//! Council content share funnel: shortlist → read → publish (replaces subtractive random loop).

use std::collections::HashSet;

use chrono::Utc;
use serde_json::{json, Value};
use sqlx::MySqlPool;
use uuid::Uuid;

use crate::agent_specs::{
    compose_council_content_share_prompt, compose_council_content_shortlist_prompt,
    CouncilContentShareContext, CouncilContentShortlistContext,
};
use crate::ai::business_functions::{
    parse_content_share_response, parse_content_shortlist_response, run_council_text_bf,
    ContentShareDecision, ContentShortlistDecision,
};
use crate::ai::business_functions::runner::{
    COUNCIL_CONTENT_SHARE_AGENT, COUNCIL_CONTENT_SHORTLIST_AGENT,
};
use crate::config::Config;
use crate::social::council_jobs::COUNCIL_SHARE_JOB_PRIORITY;
use crate::content::article_extract::{fetch_open_article_summary, mark_link_extract_status, ExtractFailure};
use crate::content::domain_paywall;
use crate::content::og_fetch;
use crate::content::portfolio::{
    COUNCIL_SHARE_GOAL, COUNCIL_SHORTLIST_MAX_ATTEMPTS, COUNCIL_SHORTLIST_POOL,
};
use crate::jobs::queue::enqueue_job_at;
use crate::social::audit;
use crate::ai::business_functions::daily_budget_b_live;
use crate::social::council_members::load_council_member_by_slug;
use crate::social::council_portfolio::{
    format_id_list, format_str_list, load_share_exclusions, load_steward_context,
    run_portfolio_hygiene_replacements, sample_shortlist_candidates, try_publish_share,
    NovelLinkRow, PendingShare, ShareExclusions, StewardContext,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OpenCandidate {
    pub content_link_id: u64,
    pub summary: String,
}

#[derive(Debug, Clone)]
struct ShareChainPayload {
    utc_date: String,
    ai_actor_slug: String,
    correlation_id: String,
    published_count: u32,
    excluded_link_ids: HashSet<u64>,
    excluded_domains: HashSet<String>,
    shortlist_attempt: u32,
    shortlist_ids: Vec<u64>,
    open_candidates: Vec<OpenCandidate>,
}

impl ShareChainPayload {
    fn from_value(v: &Value) -> Result<Self, String> {
        let utc_date = v
            .get("utc_date")
            .and_then(|x| x.as_str())
            .ok_or("missing utc_date")?
            .to_string();
        let ai_actor_slug = v
            .get("ai_actor_slug")
            .and_then(|x| x.as_str())
            .ok_or("missing ai_actor_slug")?
            .to_string();
        let correlation_id = v
            .get("correlation_id")
            .and_then(|x| x.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let published_count = v
            .get("published_count")
            .and_then(|x| x.as_u64())
            .unwrap_or(0) as u32;
        let shortlist_attempt = v
            .get("shortlist_attempt")
            .and_then(|x| x.as_u64())
            .unwrap_or(0) as u32;
        let excluded_link_ids = parse_id_set(v.get("excluded_link_ids"));
        let excluded_domains = parse_str_set(v.get("excluded_domains"));
        let shortlist_ids = v
            .get("shortlist_ids")
            .and_then(|x| x.as_array())
            .map(|arr| arr.iter().filter_map(|x| x.as_u64()).collect::<Vec<_>>())
            .unwrap_or_default();
        let open_candidates = v
            .get("open_candidates")
            .and_then(|x| x.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|row| {
                        let id = row.get("content_link_id")?.as_u64()?;
                        let summary = row.get("summary")?.as_str()?.to_string();
                        Some(OpenCandidate {
                            content_link_id: id,
                            summary,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(Self {
            utc_date,
            ai_actor_slug,
            correlation_id,
            published_count,
            excluded_link_ids,
            excluded_domains,
            shortlist_attempt,
            shortlist_ids,
            open_candidates,
        })
    }

    fn base_json(&self) -> Value {
        json!({
            "utc_date": self.utc_date,
            "ai_actor_slug": self.ai_actor_slug,
            "correlation_id": self.correlation_id,
            "published_count": self.published_count,
            "excluded_link_ids": self.excluded_link_ids.iter().copied().collect::<Vec<_>>(),
            "excluded_domains": self.excluded_domains.iter().cloned().collect::<Vec<_>>(),
            "shortlist_attempt": self.shortlist_attempt,
        })
    }
}

fn parse_id_set(v: Option<&Value>) -> HashSet<u64> {
    v.and_then(|x| x.as_array())
        .map(|arr| arr.iter().filter_map(|x| x.as_u64()).collect())
        .unwrap_or_default()
}

fn parse_str_set(v: Option<&Value>) -> HashSet<String> {
    v.and_then(|x| x.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

async fn load_member_context(
    pool: &MySqlPool,
    slug: &str,
) -> Result<(crate::social::council_members::CouncilMember, StewardContext, ShareExclusions, u64, u64), String>
{
    let member = load_council_member_by_slug(pool, slug)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("council member not found: {slug}"))?;
    let steward = load_steward_context(pool, member.community_entity_id)
        .await
        .map_err(|e| e.to_string())?;
    let exclusions = load_share_exclusions(pool, member.community_entity_id)
        .await
        .map_err(|e| e.to_string())?;
    let b = daily_budget_b_live(pool).await.map_err(|e| e.to_string())?;
    let post_budget = (b / 3).max(1);
    let system_user = audit::system_user_id(pool)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "system user not configured".to_string())?;
    Ok((member, steward, exclusions, post_budget, system_user))
}

fn merge_payload_exclusions(payload: &ShareChainPayload, exclusions: &mut ShareExclusions) {
    for id in &payload.excluded_link_ids {
        exclusions.link_ids.insert(*id);
    }
    for domain in &payload.excluded_domains {
        exclusions.domains.insert(domain.clone());
    }
}

fn build_catalogue_lines(links: &[NovelLinkRow]) -> String {
    links
        .iter()
        .map(|l| {
            format!(
                "id={} url={} domain={} paywall={} title={} description={}",
                l.link_id,
                l.url_canonical,
                l.domain,
                l.domain_paywall,
                l.title.as_deref().unwrap_or("").trim(),
                l.description
                    .as_deref()
                    .unwrap_or("")
                    .trim()
                    .chars()
                    .take(240)
                    .collect::<String>()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_candidate_summaries_block(candidates: &[OpenCandidate], links: &[NovelLinkRow]) -> String {
    let mut out = String::new();
    for (idx, cand) in candidates.iter().enumerate() {
        let meta = links.iter().find(|l| l.link_id == cand.content_link_id);
        let title = meta.and_then(|l| l.title.as_deref()).unwrap_or("").trim();
        let url = meta.map(|l| l.url_canonical.as_str()).unwrap_or("");
        let site = meta
            .and_then(|l| l.site_name.as_deref())
            .or_else(|| meta.map(|l| l.domain.as_str()))
            .unwrap_or("")
            .trim();
        out.push_str(&format!(
            "<candidate index=\"{}\" content_link_id=\"{}\">\n<title>{}</title>\n<url>{}</url>\n<site>{}</site>\n<summary>{}</summary>\n</candidate>\n",
            idx + 1,
            cand.content_link_id,
            title,
            url,
            site,
            cand.summary.chars().take(1200).collect::<String>()
        ));
    }
    out
}

pub async fn run_share_shortlist_tick(
    pool: &MySqlPool,
    config: &Config,
    payload: &Value,
) -> Result<(), String> {
    let chain = ShareChainPayload::from_value(payload)?;
    let (member, steward, mut exclusions, post_budget, system_user) =
        load_member_context(pool, &chain.ai_actor_slug).await?;
    merge_payload_exclusions(&chain, &mut exclusions);

    let target = COUNCIL_SHARE_GOAL.min(post_budget as u32);
    let mut published = chain.published_count;

    if published < target {
        published += run_portfolio_hygiene_replacements(
            pool,
            config,
            &member,
            system_user,
            post_budget,
            target.saturating_sub(published),
            &steward,
            &mut exclusions,
        )
        .await?;
    }

    if published >= target {
        tracing::info!(
            slug = %member.slug,
            published,
            "council share chain complete — daily goal met via hygiene"
        );
        return Ok(());
    }

    let candidates =
        sample_shortlist_candidates(pool, member.ai_actor_id, &exclusions, COUNCIL_SHORTLIST_POOL)
            .await
            .map_err(|e| e.to_string())?;

    if candidates.is_empty() {
        tracing::info!(slug = %member.slug, "council share shortlist — empty pool");
        return Ok(());
    }

    let allowed_ids: Vec<u64> = candidates.iter().map(|l| l.link_id).collect();
    let catalogue = build_catalogue_lines(&candidates);
    let agent = crate::agent_specs::load_council_agent(&member.slug).map_err(|e| e.to_string())?;
    let prompt = compose_council_content_shortlist_prompt(
        &agent,
        &CouncilContentShortlistContext {
            display_name: &member.display_name,
            steward_handle: &member.community_handle,
            steward_label: &steward.label,
            steward_intention: &steward.intention,
            link_catalogue: &catalogue,
            candidate_link_ids: &allowed_ids
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(", "),
            excluded_content_link_ids: &format_id_list(&exclusions.link_ids),
            excluded_url_canonical: &format_str_list(&exclusions.url_canonical),
            excluded_domains: &format_str_list(&exclusions.domains),
            excluded_domains_evicted: &format_str_list(&exclusions.evicted_domains),
        },
    );

    let run = run_council_text_bf(
        config,
        &member.slug,
        COUNCIL_CONTENT_SHORTLIST_AGENT,
        system_user,
        prompt,
        std::collections::HashMap::new(),
    )
    .await
    .map_err(|e| e.to_string())?;

    let shortlist_ids = match parse_content_shortlist_response(&run.response_text, &allowed_ids) {
        Some(ContentShortlistDecision::Shortlist { content_link_ids }) => content_link_ids,
        Some(ContentShortlistDecision::Decline { reason }) => {
            tracing::info!(
                slug = %member.slug,
                reason = reason.as_deref().unwrap_or(""),
                "council share shortlist declined"
            );
            return Ok(());
        }
        None => {
            tracing::warn!(
                slug = %member.slug,
                response_preview = %run.response_text.chars().take(240).collect::<String>(),
                "council share shortlist parse failed"
            );
            return Ok(());
        }
    };

    if shortlist_ids.is_empty() {
        return Ok(());
    }

    let mut next_payload = chain.base_json();
    next_payload["published_count"] = json!(published);
    next_payload["shortlist_ids"] = json!(shortlist_ids);
    enqueue_job_at(
        pool,
        None,
        "council_share_read",
        &next_payload,
        Some("council_share_shortlist"),
        COUNCIL_SHARE_JOB_PRIORITY,
        Utc::now().naive_utc(),
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn run_share_read_tick(
    pool: &MySqlPool,
    _config: &Config,
    payload: &Value,
) -> Result<(), String> {
    let chain = ShareChainPayload::from_value(payload)?;
    let (member, _steward, _exclusions, _post_budget, system_user) =
        load_member_context(pool, &chain.ai_actor_slug).await?;

    let mut open_candidates = Vec::new();
    for link_id in &chain.shortlist_ids {
        let row = sqlx::query_as::<_, (String,)>(
            "SELECT url_canonical FROM content_link WHERE id = ? LIMIT 1",
        )
        .bind(link_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;
        let Some((url,)) = row else {
            tracing::info!(
                slug = %member.slug,
                link_id,
                "council share read skipped — link row missing"
            );
            continue;
        };

        let _ = og_fetch::ensure_shareable_og_for_link(pool, *link_id, Some(system_user), true).await;

        match fetch_open_article_summary(&url).await {
            Ok(extract) => {
                let _ = mark_link_extract_status(pool, *link_id, "ok").await;
                open_candidates.push(OpenCandidate {
                    content_link_id: *link_id,
                    summary: extract.summary,
                });
            }
            Err(ExtractFailure::Paywalled) => {
                let _ = mark_link_extract_status(pool, *link_id, "paywalled").await;
                if let Ok(Some(domain)) = domain_paywall::record_paywall_hit(pool, *link_id).await {
                    tracing::info!(
                        slug = %member.slug,
                        link_id,
                        domain = %domain,
                        paywall = "partial",
                        "content domain paywall profile updated"
                    );
                }
                tracing::info!(
                    slug = %member.slug,
                    link_id,
                    url = %url,
                    "council share read skipped — paywalled"
                );
            }
            Err(reason) => {
                let _ = mark_link_extract_status(pool, *link_id, "extract_failed").await;
                tracing::info!(
                    slug = %member.slug,
                    link_id,
                    url = %url,
                    reason = ?reason,
                    "council share read skipped — extract failed"
                );
            }
        }
    }

    if open_candidates.is_empty() {
        tracing::info!(
            slug = %member.slug,
            shortlist_attempt = chain.shortlist_attempt,
            shortlisted = chain.shortlist_ids.len(),
            "council share read — no open candidates"
        );
        if chain.shortlist_attempt + 1 < COUNCIL_SHORTLIST_MAX_ATTEMPTS {
            let mut retry = chain.base_json();
            retry["shortlist_attempt"] = json!(chain.shortlist_attempt + 1);
            retry["published_count"] = json!(chain.published_count);
            enqueue_job_at(
                pool,
                None,
                "council_share_shortlist",
                &retry,
                Some("council_share_read"),
                COUNCIL_SHARE_JOB_PRIORITY,
                Utc::now().naive_utc(),
            )
            .await
            .map_err(|e| e.to_string())?;
        }
        return Ok(());
    }

    let mut next = chain.base_json();
    next["open_candidates"] = json!(open_candidates);
    next["published_count"] = json!(chain.published_count);
    enqueue_job_at(
        pool,
        None,
        "council_share_publish",
        &next,
        Some("council_share_read"),
        COUNCIL_SHARE_JOB_PRIORITY,
        Utc::now().naive_utc(),
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn run_share_publish_tick(
    pool: &MySqlPool,
    config: &Config,
    payload: &Value,
) -> Result<(), String> {
    let chain = ShareChainPayload::from_value(payload)?;
    let (member, steward, mut exclusions, post_budget, system_user) =
        load_member_context(pool, &chain.ai_actor_slug).await?;
    merge_payload_exclusions(&chain, &mut exclusions);

    let target = COUNCIL_SHARE_GOAL.min(post_budget as u32);
    if chain.published_count >= target {
        return Ok(());
    }

    let mut link_rows = Vec::new();
    for cand in &chain.open_candidates {
        if let Ok(Some(row)) = sqlx::query_as::<_, NovelLinkRow>(
            r#"
            SELECT cl.id AS link_id, cl.url_canonical,
                   cl.og_title AS title, cl.og_description AS description,
                   cl.og_site_name AS site_name, cd.domain,
                   cd.paywall AS domain_paywall
            FROM content_link cl
            INNER JOIN content_domain cd ON cd.id = cl.domain_id
            WHERE cl.id = ?
            LIMIT 1
            "#,
        )
        .bind(cand.content_link_id)
        .fetch_optional(pool)
        .await
        {
            link_rows.push(row);
        }
    }

    let allowed_ids: Vec<u64> = chain
        .open_candidates
        .iter()
        .map(|c| c.content_link_id)
        .collect();
    let summaries_block = build_candidate_summaries_block(&chain.open_candidates, &link_rows);
    let agent = crate::agent_specs::load_council_agent(&member.slug).map_err(|e| e.to_string())?;

    let first = link_rows.first();
    let prompt = compose_council_content_share_prompt(
        &agent,
        &CouncilContentShareContext {
            display_name: &member.display_name,
            steward_handle: &member.community_handle,
            steward_label: &steward.label,
            steward_intention: &steward.intention,
            link_url: first.map(|l| l.url_canonical.as_str()).unwrap_or(""),
            link_title: first
                .and_then(|l| l.title.as_deref())
                .unwrap_or("")
                .trim(),
            link_description: first
                .and_then(|l| l.description.as_deref())
                .unwrap_or("")
                .trim(),
            link_site_name: first
                .and_then(|l| l.site_name.as_deref())
                .or_else(|| first.map(|l| l.domain.as_str()))
                .unwrap_or("")
                .trim(),
            link_catalogue: &summaries_block,
            candidate_link_ids: &allowed_ids
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(", "),
            candidate_summaries: &summaries_block,
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
    .map_err(|e| e.to_string())?;

    let mut published = chain.published_count;
    if let Some(decision) = parse_content_share_response(&run.response_text, &allowed_ids) {
        match decision {
            ContentShareDecision::Share {
                content_link_id,
                body_markup,
            } => {
                let share = PendingShare {
                    content_link_id,
                    body_markup,
                };
                if try_publish_share(
                    pool,
                    &member,
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
            ContentShareDecision::Decline { reason } => {
                tracing::info!(
                    slug = %member.slug,
                    reason = reason.as_deref().unwrap_or(""),
                    "council share publish declined"
                );
            }
        }
    } else {
        tracing::warn!(
            slug = %member.slug,
            response_preview = %run.response_text.chars().take(240).collect::<String>(),
            "council share publish parse failed"
        );
    }

    if published < target && chain.shortlist_attempt + 1 < COUNCIL_SHORTLIST_MAX_ATTEMPTS {
        let mut next = chain.base_json();
        next["published_count"] = json!(published);
        next["shortlist_attempt"] = json!(chain.shortlist_attempt + 1);
        let mut ex_ids: Vec<u64> = exclusions.link_ids.iter().copied().collect();
        ex_ids.sort_unstable();
        next["excluded_link_ids"] = json!(ex_ids);
        let mut ex_domains: Vec<String> = exclusions.domains.iter().cloned().collect();
        ex_domains.sort_unstable();
        next["excluded_domains"] = json!(ex_domains);
        enqueue_job_at(
            pool,
            None,
            "council_share_shortlist",
            &next,
            Some("council_share_publish"),
            COUNCIL_SHARE_JOB_PRIORITY,
            Utc::now().naive_utc(),
        )
        .await
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Legacy `council_post_member` entry: run shortlist stage with a fresh correlation id.
pub async fn run_post_member_compat(
    pool: &MySqlPool,
    config: &Config,
    ai_actor_slug: &str,
    utc_date: &str,
) -> Result<(), String> {
    let payload = json!({
        "utc_date": utc_date,
        "ai_actor_slug": ai_actor_slug,
        "correlation_id": Uuid::new_v4().to_string(),
        "published_count": 0,
        "shortlist_attempt": 0,
        "excluded_link_ids": [],
        "excluded_domains": [],
    });
    run_share_shortlist_tick(pool, config, &payload).await
}
