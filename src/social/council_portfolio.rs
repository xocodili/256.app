//! Council post tick — portfolio hygiene (Algorithm 1) and subtractive link selection (Algorithm 2).

use std::collections::HashSet;

use rand::seq::SliceRandom;
use rand::Rng;
use sqlx::MySqlPool;

use crate::agent_specs::{
    compose_council_content_share_prompt, compose_council_portfolio_replace_prompt,
    CouncilContentShareContext, CouncilPortfolioReplaceContext,
};
use crate::ai::business_functions::{
    parse_content_share_response, parse_portfolio_replace_response, run_council_text_bf,
    spend_post, ContentShareDecision, PortfolioReplaceDecision,
};
use crate::ai::business_functions::runner::{
    COUNCIL_CONTENT_SHARE_AGENT, COUNCIL_PORTFOLIO_REPLACE_AGENT,
};
use crate::config::Config;
use crate::content::links::{canonicalize_url, ensure_domain_allowed, upsert_canonical_link};
use crate::content::og_fetch::{self, is_homepage_url};
use crate::content::portfolio::{
    COUNCIL_SHARE_GOAL, MAX_PORTFOLIO_REPLACE_ATTEMPTS, PORTFOLIO_REPLACE_CATALOGUE_LIMIT,
};
use crate::ref_keys;
use crate::social::council_members::CouncilMember;
use crate::social::council_publish::publish_community_share_tx;

#[derive(Debug, Clone)]
pub struct StewardContext {
    pub label: String,
    pub intention: String,
}

#[derive(Debug, Default)]
pub(crate) struct ShareExclusions {
    pub(crate) link_ids: HashSet<u64>,
    pub(crate) url_canonical: HashSet<String>,
    pub(crate) domains: HashSet<String>,
    pub(crate) evicted_domains: HashSet<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingShare {
    pub(crate) content_link_id: u64,
    pub(crate) body_markup: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct EvictedHost {
    domain_id: u64,
    domain: String,
}

#[derive(Debug, sqlx::FromRow)]
struct PostedLinkRow {
    link_id: u64,
    url_canonical: String,
    domain: String,
}

#[derive(Debug, sqlx::FromRow)]
struct DomainCatalogueRow {
    domain_id: u64,
    domain: String,
    rating: rust_decimal::Decimal,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct NovelLinkRow {
    pub(crate) link_id: u64,
    pub(crate) url_canonical: String,
    pub(crate) title: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) site_name: Option<String>,
    pub(crate) domain: String,
    pub(crate) domain_paywall: String,
}

#[derive(Debug, sqlx::FromRow)]
struct PortfolioDomainRow {
    domain_id: u64,
    domain: String,
    paywall: String,
}

/// Run Algorithms 1–2 for one councillor; returns successful publish count.
pub async fn run_council_post_share_tick(
    pool: &MySqlPool,
    config: &Config,
    member: &CouncilMember,
    system_user: u64,
    post_budget: u64,
) -> Result<u32, String> {
    let target = COUNCIL_SHARE_GOAL.min(post_budget as u32);
    if target == 0 {
        return Ok(0);
    }

    let steward = load_steward_context(pool, member.community_entity_id)
        .await
        .map_err(|e| e.to_string())?;
    let mut exclusions = load_share_exclusions(pool, member.community_entity_id)
        .await
        .map_err(|e| e.to_string())?;

    let evicted = evict_ineligible_portfolio_hosts(pool, member.ai_actor_id, system_user)
        .await
        .map_err(|e| e.to_string())?;

    let mut pending_shares = Vec::new();
    for host in evicted {
        exclusions.evicted_domains.insert(host.domain.clone());
        exclusions.domains.insert(host.domain.clone());
        if let Some(share) = try_replace_evicted_host(
            pool,
            config,
            member,
            system_user,
            &steward,
            &host,
            &mut exclusions,
        )
        .await
        {
            pending_shares.push(share);
        }
    }

    let mut published = 0u32;
    for share in pending_shares {
        if published >= target {
            break;
        }
        if try_publish_share(
            pool,
            member,
            system_user,
            post_budget,
            &share,
            &mut exclusions,
        )
        .await?
        {
            published += 1;
        }
    }

    if published < target {
        published += run_subtractive_share_loop(
            pool,
            config,
            member,
            system_user,
            post_budget,
            target,
            &steward,
            &mut exclusions,
            published,
        )
        .await?;
    }

    Ok(published)
}

pub async fn load_steward_context(
    pool: &MySqlPool,
    community_entity_id: u64,
) -> Result<StewardContext, sqlx::Error> {
    let row: (String, String) = sqlx::query_as(
        r#"
        SELECT
            COALESCE(NULLIF(TRIM(sc.label), ''), se.handle) AS label,
            COALESCE(NULLIF(TRIM(sc.intention), ''), '(none)') AS intention
        FROM social_entity se
        LEFT JOIN social_community sc ON sc.social_entity_id = se.id
        WHERE se.id = ?
        LIMIT 1
        "#,
    )
    .bind(community_entity_id)
    .fetch_one(pool)
    .await?;
    Ok(StewardContext {
        label: row.0,
        intention: row.1,
    })
}

pub(crate) async fn load_share_exclusions(
    pool: &MySqlPool,
    community_entity_id: u64,
) -> Result<ShareExclusions, sqlx::Error> {
    let posted = sqlx::query_as::<_, PostedLinkRow>(
        r#"
        SELECT DISTINCT
            cl.id AS link_id,
            cl.url_canonical,
            cd.domain
        FROM post p
        INNER JOIN post_audience_entity pae ON pae.post_id = p.id
        INNER JOIN content_link cl ON cl.id = p.content_link_id
        INNER JOIN content_domain cd ON cd.id = cl.domain_id
        WHERE pae.audience_entity_id = ?
          AND p.content_link_id IS NOT NULL
        "#,
    )
    .bind(community_entity_id)
    .fetch_all(pool)
    .await?;

    let mut exclusions = ShareExclusions::default();
    for row in posted {
        exclusions.link_ids.insert(row.link_id);
        exclusions.url_canonical.insert(row.url_canonical);
        exclusions.domains.insert(row.domain);
    }
    Ok(exclusions)
}

/// Active portfolio hostnames — for replacement/discovery prompts only (not shortlist).
async fn load_portfolio_domain_names(
    pool: &MySqlPool,
    ai_actor_id: u64,
) -> Result<HashSet<String>, sqlx::Error> {
    let rows: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT cd.domain
        FROM ai_actor_content_domain pac
        INNER JOIN content_domain cd ON cd.id = pac.content_domain_id
        WHERE pac.ai_actor_id = ?
          AND pac.is_active = 1
          AND pac.removed_at IS NULL
        "#,
    )
    .bind(ai_actor_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().collect())
}

/// Replacement LLM must not re-pick hosts already in portfolio; shortlist uses [`format_str_list`] on `exclusions.domains` only.
pub(crate) fn format_excluded_domains_for_replacement(
    exclusions: &ShareExclusions,
    portfolio_domains: &HashSet<String>,
) -> String {
    let mut merged = exclusions.domains.clone();
    merged.extend(portfolio_domains.iter().cloned());
    format_str_list(&merged)
}

async fn evict_ineligible_portfolio_hosts(
    pool: &MySqlPool,
    ai_actor_id: u64,
    system_user: u64,
) -> Result<Vec<EvictedHost>, sqlx::Error> {
    let rows: Vec<EvictedHost> = sqlx::query_as(
        r#"
        SELECT pac.content_domain_id AS domain_id, cd.domain
        FROM ai_actor_content_domain pac
        INNER JOIN content_domain cd ON cd.id = pac.content_domain_id
        INNER JOIN ref_key_values rk
            ON rk.id = cd.status_id
           AND rk.`group` = ?
           AND rk.is_active = 1
        WHERE pac.ai_actor_id = ?
          AND pac.is_active = 1
          AND pac.removed_at IS NULL
          AND (
              cd.rating < 2.50
              OR cd.safety_state <> 'safe'
              OR rk.`key` = ?
          )
        "#,
    )
    .bind(ref_keys::GROUP_STATUS)
    .bind(ai_actor_id)
    .bind(ref_keys::KEY_BLOCKED)
    .fetch_all(pool)
    .await?;

    for host in &rows {
        sqlx::query(
            r#"
            UPDATE ai_actor_content_domain
            SET is_active = 0,
                removed_at = UTC_TIMESTAMP(6),
                modified_by = ?
            WHERE ai_actor_id = ?
              AND content_domain_id = ?
              AND is_active = 1
            "#,
        )
        .bind(system_user)
        .bind(ai_actor_id)
        .bind(host.domain_id)
        .execute(pool)
        .await?;
    }
    Ok(rows)
}

async fn load_replacement_catalogue(
    pool: &MySqlPool,
    ai_actor_id: u64,
) -> Result<Vec<DomainCatalogueRow>, sqlx::Error> {
    let lim = PORTFOLIO_REPLACE_CATALOGUE_LIMIT;
    let sql = format!(
        r#"
        SELECT cd.id AS domain_id, cd.domain, cd.rating
        FROM content_domain cd
        INNER JOIN ref_key_values rk
            ON rk.id = cd.status_id
           AND rk.`group` = ?
           AND rk.`key` <> ?
           AND rk.is_active = 1
        WHERE cd.rating >= 2.50
          AND cd.safety_state = 'safe'
          AND NOT EXISTS (
              SELECT 1
              FROM ai_actor_content_domain pac
              WHERE pac.ai_actor_id = ?
                AND pac.content_domain_id = cd.id
                AND pac.is_active = 1
                AND pac.removed_at IS NULL
          )
        ORDER BY cd.rating DESC, cd.id ASC
        LIMIT {lim}
        "#
    );
    sqlx::query_as(&sql)
        .bind(ref_keys::GROUP_STATUS)
        .bind(ref_keys::KEY_BLOCKED)
        .bind(ai_actor_id)
        .fetch_all(pool)
        .await
}

async fn activate_portfolio_domain(
    pool: &MySqlPool,
    ai_actor_id: u64,
    domain_id: u64,
    system_user: u64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO ai_actor_content_domain (
            ai_actor_id, content_domain_id, is_active, created_by, modified_by
        )
        VALUES (?, ?, 1, ?, ?)
        ON DUPLICATE KEY UPDATE
            is_active = 1,
            removed_at = NULL,
            modified_by = VALUES(modified_by)
        "#,
    )
    .bind(ai_actor_id)
    .bind(domain_id)
    .bind(system_user)
    .bind(system_user)
    .execute(pool)
    .await?;
    Ok(())
}

async fn try_replace_evicted_host(
    pool: &MySqlPool,
    config: &Config,
    member: &CouncilMember,
    system_user: u64,
    steward: &StewardContext,
    evicted: &EvictedHost,
    exclusions: &mut ShareExclusions,
) -> Option<PendingShare> {
    let catalogue_rows = load_replacement_catalogue(pool, member.ai_actor_id)
        .await
        .ok()?;
    let portfolio_domains = load_portfolio_domain_names(pool, member.ai_actor_id)
        .await
        .ok()?;
    let replacement_excluded =
        format_excluded_domains_for_replacement(exclusions, &portfolio_domains);
    let catalogue = catalogue_rows
        .iter()
        .map(|r| format!("id={} domain={} rating={}", r.domain_id, r.domain, r.rating))
        .collect::<Vec<_>>()
        .join("\n");
    let agent = crate::agent_specs::load_council_agent(&member.slug).ok()?;

    for _ in 0..MAX_PORTFOLIO_REPLACE_ATTEMPTS {
        let prompt = compose_council_portfolio_replace_prompt(
            &agent,
            &CouncilPortfolioReplaceContext {
                display_name: &member.display_name,
                steward_handle: &member.community_handle,
                steward_label: &steward.label,
                steward_intention: &steward.intention,
                evicted_domain: &evicted.domain,
                replacement_catalogue: &catalogue,
                excluded_content_link_ids: &format_id_list(&exclusions.link_ids),
                excluded_url_canonical: &format_str_list(&exclusions.url_canonical),
                excluded_domains: &replacement_excluded,
                excluded_domains_evicted: &format_str_list(&exclusions.evicted_domains),
            },
        );

        let run = run_council_text_bf(
            config,
            &member.slug,
            COUNCIL_PORTFOLIO_REPLACE_AGENT,
            system_user,
            prompt,
            std::collections::HashMap::new(),
        )
        .await
        .ok()?;

        let decision = parse_portfolio_replace_response(&run.response_text)?;
        let PortfolioReplaceDecision::Replace { proof_url, .. } = decision
        else {
            return None;
        };

        let canonical = match canonicalize_url(&proof_url) {
            Ok(c) => c,
            Err(_) => {
                exclusions.domains.insert(proof_url.clone());
                continue;
            }
        };
        if exclusions.domains.contains(&canonical.registrable_domain) {
            continue;
        }
        if ensure_domain_allowed(
            pool,
            canonical.registrable_domain.as_str(),
            "council_portfolio_replace",
            Some(system_user),
            None,
        )
        .await
        .is_err()
        {
            exclusions
                .domains
                .insert(canonical.registrable_domain.clone());
            continue;
        }

        if is_homepage_url(&canonical.url_canonical) {
            exclusions.domains.insert(canonical.registrable_domain.clone());
            continue;
        }

        let upserted = match upsert_canonical_link(pool, &canonical, Some(system_user)).await {
            Ok(u) => u,
            Err(_) => continue,
        };
        if !og_fetch::ensure_shareable_og_for_link(pool, upserted.link_id, Some(system_user), true)
            .await
            .unwrap_or(false)
        {
            tracing::debug!(
                slug = %member.slug,
                link_id = upserted.link_id,
                url = %canonical.url_canonical,
                "council portfolio replace skipped — OG metadata insufficient"
            );
            continue;
        }

        if activate_portfolio_domain(pool, member.ai_actor_id, upserted.domain_id, system_user)
            .await
            .is_err()
        {
            continue;
        }

        exclusions
            .domains
            .insert(canonical.registrable_domain.clone());

        if exclusions.link_ids.contains(&upserted.link_id) {
            return None;
        }

        let Some(link) = load_novel_link_by_id(pool, upserted.link_id).await.ok().flatten() else {
            return None;
        };
        let body = run_share_llm_for_link(
            pool,
            config,
            member,
            &agent,
            steward,
            &link,
            exclusions,
            system_user,
        )
        .await?;

        return Some(PendingShare {
            content_link_id: upserted.link_id,
            body_markup: body,
        });
    }
    None
}

pub(crate) async fn try_publish_share(
    pool: &MySqlPool,
    member: &CouncilMember,
    system_user: u64,
    post_budget: u64,
    share: &PendingShare,
    exclusions: &mut ShareExclusions,
) -> Result<bool, String> {
    if exclusions.link_ids.contains(&share.content_link_id) {
        tracing::info!(
            slug = %member.slug,
            content_link_id = share.content_link_id,
            "council share publish skipped — link already posted to steward"
        );
        return Ok(false);
    }

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    if !spend_post(pool, &mut tx, member.ai_actor_id, post_budget)
        .await
        .map_err(|e| e.to_string())?
    {
        tx.rollback().await.ok();
        tracing::info!(
            slug = %member.slug,
            post_budget,
            "council share publish skipped — post budget exhausted"
        );
        return Ok(false);
    }

    let post_id = match publish_community_share_tx(
        &mut tx,
        member.entity_id,
        member.community_entity_id,
        member.ai_actor_id,
        system_user,
        share.content_link_id,
        &share.body_markup,
    )
    .await
    {
        Ok(id) => id,
        Err(e) => {
            tx.rollback().await.ok();
            tracing::info!(
                slug = %member.slug,
                content_link_id = share.content_link_id,
                error = %e,
                "council share publish skipped — publish gate"
            );
            return Ok(false);
        }
    };

    tx.commit().await.map_err(|e| e.to_string())?;

    if let Ok(Some((url, domain))) = sqlx::query_as::<_, (String, String)>(
        r#"
        SELECT cl.url_canonical, cd.domain
        FROM content_link cl
        INNER JOIN content_domain cd ON cd.id = cl.domain_id
        WHERE cl.id = ?
        "#,
    )
    .bind(share.content_link_id)
    .fetch_optional(pool)
    .await
    {
        exclusions.link_ids.insert(share.content_link_id);
        exclusions.url_canonical.insert(url);
        exclusions.domains.insert(domain);
    }

    tracing::info!(
        slug = %member.slug,
        post_id,
        content_link_id = share.content_link_id,
        "council content share published"
    );
    Ok(true)
}

async fn run_subtractive_share_loop(
    pool: &MySqlPool,
    config: &Config,
    member: &CouncilMember,
    system_user: u64,
    post_budget: u64,
    target: u32,
    steward: &StewardContext,
    exclusions: &mut ShareExclusions,
    already_published: u32,
) -> Result<u32, String> {
    let domain_pool = load_active_portfolio_domains(pool, member.ai_actor_id)
        .await
        .map_err(|e| e.to_string())?;

    let agent = crate::agent_specs::load_council_agent(&member.slug).map_err(|e| e.to_string())?;
    let mut added = 0u32;

    for domain_row in domain_pool {
        if already_published + added >= target {
            break;
        }
        exclusions.domains.insert(domain_row.domain.clone());

        let mut novel_links = rank_share_candidates(load_novel_links_for_domain(
            pool,
            domain_row.domain_id,
            &exclusions.link_ids,
        )
        .await
        .map_err(|e| e.to_string())?);
        if novel_links.is_empty() {
            if let Some(discovered) = try_discover_domain_article(
                pool,
                config,
                member,
                system_user,
                steward,
                &domain_row,
                exclusions,
            )
            .await
            {
                novel_links.push(discovered);
            } else {
                continue;
            }
        }
        if novel_links.is_empty() {
            continue;
        }

        let link_idx = rand::thread_rng().gen_range(0..novel_links.len());
        let link = &novel_links[link_idx];

        let body = match run_share_llm_for_link(
            pool,
            config,
            member,
            &agent,
            steward,
            link,
            exclusions,
            system_user,
        )
        .await
        {
            Some(b) => b,
            None => continue,
        };

        let share = PendingShare {
            content_link_id: link.link_id,
            body_markup: body,
        };
        if try_publish_share(
            pool,
            member,
            system_user,
            post_budget,
            &share,
            exclusions,
        )
        .await?
        {
            added += 1;
        }
    }

    Ok(added)
}

async fn run_share_llm_for_link(
    pool: &MySqlPool,
    config: &Config,
    member: &CouncilMember,
    agent: &crate::agent_specs::CouncilAgentFile,
    steward: &StewardContext,
    link: &NovelLinkRow,
    exclusions: &ShareExclusions,
    system_user: u64,
) -> Option<String> {
    if !og_fetch::ensure_shareable_og_for_link(pool, link.link_id, Some(system_user), true)
        .await
        .unwrap_or(false)
    {
        tracing::debug!(
            slug = %member.slug,
            link_id = link.link_id,
            url = %link.url_canonical,
            "council share skipped — OG metadata insufficient"
        );
        return None;
    }

    let fresh = load_novel_link_by_id(pool, link.link_id).await.ok().flatten()?;
    let link_title = fresh
        .title
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_string();
    let link_description = fresh
        .description
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_string();
    let link_site_name = fresh
        .site_name
        .as_deref()
        .unwrap_or(&fresh.domain)
        .trim()
        .to_string();

    if link_description.is_empty()
        || og_fetch::text_looks_like_error_page(&link_title)
        || og_fetch::text_looks_like_error_page(&link_description)
    {
        return None;
    }

    let catalogue = format!(
        "id={} domain={} paywall={} url={} title={} description={}",
        fresh.link_id,
        fresh.domain,
        fresh.domain_paywall,
        fresh.url_canonical,
        link_title,
        link_description.chars().take(240).collect::<String>()
    );
    let candidate_ids = fresh.link_id.to_string();
    let prompt = compose_council_content_share_prompt(
        agent,
        &CouncilContentShareContext {
            display_name: &member.display_name,
            steward_handle: &member.community_handle,
            steward_label: &steward.label,
            steward_intention: &steward.intention,
            link_url: &fresh.url_canonical,
            link_title: &link_title,
            link_description: &link_description,
            link_site_name: &link_site_name,
            link_catalogue: &catalogue,
            candidate_link_ids: &candidate_ids,
            candidate_summaries: "",
            excluded_content_link_ids: &format_id_list(&exclusions.link_ids),
            excluded_url_canonical: &format_str_list(&exclusions.url_canonical),
            excluded_domains: &format_str_list(&exclusions.domains),
            excluded_domains_evicted: &format_str_list(&exclusions.evicted_domains),
        },
    );

    let run = run_council_text_bf(
        config,
        &member.slug,
        COUNCIL_CONTENT_SHARE_AGENT,
        system_user,
        prompt,
        std::collections::HashMap::new(),
    )
    .await
    .ok()?;

    let decision = parse_content_share_response(&run.response_text, &[fresh.link_id])?;
    match decision {
        ContentShareDecision::Share { body_markup, .. } => Some(body_markup),
        ContentShareDecision::Decline { .. } => None,
    }
}

async fn load_active_portfolio_domains(
    pool: &MySqlPool,
    ai_actor_id: u64,
) -> Result<Vec<PortfolioDomainRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT pac.content_domain_id AS domain_id, cd.domain, cd.paywall
        FROM ai_actor_content_domain pac
        INNER JOIN content_domain cd ON cd.id = pac.content_domain_id
        INNER JOIN ref_key_values rk
            ON rk.id = cd.status_id
           AND rk.`group` = ?
           AND rk.`key` <> ?
           AND rk.is_active = 1
        WHERE pac.ai_actor_id = ?
          AND pac.is_active = 1
          AND pac.removed_at IS NULL
          AND cd.rating >= 2.50
          AND cd.safety_state = 'safe'
          AND cd.paywall <> 'whole'
        ORDER BY CASE cd.paywall
            WHEN 'free' THEN 0
            WHEN 'partial' THEN 1
            ELSE 2
        END, cd.domain
        "#,
    )
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_BLOCKED)
    .bind(ai_actor_id)
    .fetch_all(pool)
    .await
}

async fn load_novel_links_for_domain(
    pool: &MySqlPool,
    domain_id: u64,
    excluded_link_ids: &HashSet<u64>,
) -> Result<Vec<NovelLinkRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, NovelLinkRow>(
        r#"
        SELECT
            cl.id AS link_id,
            cl.url_canonical,
            cl.og_title AS title,
            cl.og_description AS description,
            cl.og_site_name AS site_name,
            cd.domain,
            cd.paywall AS domain_paywall
        FROM content_link cl
        INNER JOIN content_domain cd ON cd.id = cl.domain_id
        INNER JOIN ref_key_values rk
            ON rk.id = cd.status_id
           AND rk.`group` = ?
           AND rk.`key` <> ?
           AND rk.is_active = 1
        WHERE cl.domain_id = ?
          AND cl.fetch_status = 'ok'
          AND cl.rating >= 2.50
          AND cd.rating >= 2.50
          AND cd.safety_state = 'safe'
          AND cd.paywall <> 'whole'
          AND (cl.extract_status IS NULL OR cl.extract_status <> 'paywalled')
        ORDER BY cl.share_count ASC, cl.id DESC
        LIMIT 64
        "#,
    )
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_BLOCKED)
    .bind(domain_id)
    .fetch_all(pool)
    .await?;

    Ok(rank_share_candidates(
        rows.into_iter()
            .filter(|r| !excluded_link_ids.contains(&r.link_id))
            .collect(),
    ))
}

async fn load_novel_link_by_id(
    pool: &MySqlPool,
    link_id: u64,
) -> Result<Option<NovelLinkRow>, sqlx::Error> {
    sqlx::query_as::<_, NovelLinkRow>(
        r#"
        SELECT
            cl.id AS link_id,
            cl.url_canonical,
            cl.og_title AS title,
            cl.og_description AS description,
            cl.og_site_name AS site_name,
            cd.domain,
            cd.paywall AS domain_paywall
        FROM content_link cl
        INNER JOIN content_domain cd ON cd.id = cl.domain_id
        WHERE cl.id = ?
        LIMIT 1
        "#,
    )
    .bind(link_id)
    .fetch_optional(pool)
    .await
}

#[must_use]
fn rank_share_candidates(mut links: Vec<NovelLinkRow>) -> Vec<NovelLinkRow> {
    let has_deep = links
        .iter()
        .any(|l| !is_homepage_url(&l.url_canonical));
    if has_deep {
        links.retain(|l| !is_homepage_url(&l.url_canonical));
    }
    links.sort_by(|a, b| {
        let score_a = share_candidate_score(a);
        let score_b = share_candidate_score(b);
        score_b.cmp(&score_a).then_with(|| a.link_id.cmp(&b.link_id))
    });
    links.truncate(32);
    links
}

fn share_candidate_score(link: &NovelLinkRow) -> u8 {
    let mut score = 0u8;
    if !is_homepage_url(&link.url_canonical) {
        score += 4;
    }
    if link
        .description
        .as_deref()
        .is_some_and(|d| !d.trim().is_empty())
    {
        score += 2;
    }
    if link
        .title
        .as_deref()
        .is_some_and(|t| !t.trim().is_empty() && !t.eq_ignore_ascii_case(&link.domain))
    {
        score += 1;
    }
    score
}

async fn try_discover_domain_article(
    pool: &MySqlPool,
    config: &Config,
    member: &CouncilMember,
    system_user: u64,
    steward: &StewardContext,
    domain_row: &PortfolioDomainRow,
    exclusions: &mut ShareExclusions,
) -> Option<NovelLinkRow> {
    let agent = crate::agent_specs::load_council_agent(&member.slug).ok()?;
    let portfolio_domains = load_portfolio_domain_names(pool, member.ai_actor_id)
        .await
        .ok()?;
    let replacement_excluded =
        format_excluded_domains_for_replacement(exclusions, &portfolio_domains);
    let catalogue = format!(
        "id={} domain={} rating=2.50 (seek a specific article URL on this host — not the homepage)",
        domain_row.domain_id, domain_row.domain
    );
    let prompt = compose_council_portfolio_replace_prompt(
        &agent,
        &CouncilPortfolioReplaceContext {
            display_name: &member.display_name,
            steward_handle: &member.community_handle,
            steward_label: &steward.label,
            steward_intention: &steward.intention,
            evicted_domain: &format!(
                "{} (catalogue exhausted — propose one fresh article URL on this host)",
                domain_row.domain
            ),
            replacement_catalogue: &catalogue,
            excluded_content_link_ids: &format_id_list(&exclusions.link_ids),
            excluded_url_canonical: &format_str_list(&exclusions.url_canonical),
            excluded_domains: &replacement_excluded,
            excluded_domains_evicted: &format_str_list(&exclusions.evicted_domains),
        },
    );
    let run = run_council_text_bf(
        config,
        &member.slug,
        COUNCIL_PORTFOLIO_REPLACE_AGENT,
        system_user,
        prompt,
        std::collections::HashMap::new(),
    )
    .await
    .ok()?;
    let decision = parse_portfolio_replace_response(&run.response_text)?;
    let PortfolioReplaceDecision::Replace { proof_url, .. } = decision else {
        return None;
    };
    let canonical = canonicalize_url(&proof_url).ok()?;
    if canonical.registrable_domain != domain_row.domain {
        return None;
    }
    if is_homepage_url(&canonical.url_canonical) {
        return None;
    }
    if ensure_domain_allowed(
        pool,
        canonical.registrable_domain.as_str(),
        "council_link_discovery",
        Some(system_user),
        None,
    )
    .await
    .is_err()
    {
        return None;
    }
    let upserted = upsert_canonical_link(pool, &canonical, Some(system_user))
        .await
        .ok()?;
    if !og_fetch::ensure_shareable_og_for_link(pool, upserted.link_id, Some(system_user), true)
        .await
        .unwrap_or(false)
    {
        return None;
    }
    load_novel_link_by_id(pool, upserted.link_id).await.ok().flatten()
}

/// Portfolio hygiene (Algorithm 1): evict ineligible hosts, publish replacement shares.
pub(crate) async fn run_portfolio_hygiene_replacements(
    pool: &MySqlPool,
    config: &Config,
    member: &CouncilMember,
    system_user: u64,
    post_budget: u64,
    target: u32,
    steward: &StewardContext,
    exclusions: &mut ShareExclusions,
) -> Result<u32, String> {
    let evicted = evict_ineligible_portfolio_hosts(pool, member.ai_actor_id, system_user)
        .await
        .map_err(|e| e.to_string())?;

    let mut pending_shares = Vec::new();
    for host in evicted {
        exclusions.evicted_domains.insert(host.domain.clone());
        exclusions.domains.insert(host.domain.clone());
        if let Some(share) = try_replace_evicted_host(
            pool,
            config,
            member,
            system_user,
            steward,
            &host,
            exclusions,
        )
        .await
        {
            pending_shares.push(share);
        }
    }

    let mut published = 0u32;
    for share in pending_shares {
        if published >= target {
            break;
        }
        if try_publish_share(
            pool,
            member,
            system_user,
            post_budget,
            &share,
            exclusions,
        )
        .await?
        {
            published += 1;
        }
    }
    Ok(published)
}

/// Sample up to `limit` novel links across portfolio domains (free → partial).
pub(crate) async fn sample_shortlist_candidates(
    pool: &MySqlPool,
    ai_actor_id: u64,
    exclusions: &ShareExclusions,
    limit: usize,
) -> Result<Vec<NovelLinkRow>, sqlx::Error> {
    let domain_pool = load_active_portfolio_domains(pool, ai_actor_id).await?;

    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for domain_row in domain_pool {
        if out.len() >= limit {
            break;
        }
        let links = load_novel_links_for_domain(pool, domain_row.domain_id, &exclusions.link_ids)
            .await?;
        for link in links {
            if out.len() >= limit {
                break;
            }
            if seen.insert(link.link_id) {
                out.push(link);
            }
        }
    }
    Ok(out)
}

pub(crate) fn format_id_list(ids: &HashSet<u64>) -> String {
    if ids.is_empty() {
        return "(none)".to_string();
    }
    let mut sorted: Vec<_> = ids.iter().copied().collect();
    sorted.sort_unstable();
    sorted
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn format_str_list(values: &HashSet<String>) -> String {
    if values.is_empty() {
        return "(none)".to_string();
    }
    let mut sorted: Vec<_> = values.iter().cloned().collect();
    sorted.sort_unstable();
    sorted.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_excluded_domains_include_portfolio() {
        let mut exclusions = ShareExclusions::default();
        exclusions.domains.insert("posted.example".into());
        let portfolio = HashSet::from(["guardian.com".into(), "bbc.co.uk".into()]);
        let formatted = format_excluded_domains_for_replacement(&exclusions, &portfolio);
        assert!(formatted.contains("posted.example"));
        assert!(formatted.contains("guardian.com"));
        assert!(formatted.contains("bbc.co.uk"));
    }
}
