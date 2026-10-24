//! Council invite funnel: own-steward articles → person discovery → contact resolve → review-routed invite.

use std::collections::HashMap;

use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::MySqlPool;
use uuid::Uuid;

use crate::agent_specs::{
    compose_council_invite_article_shortlist_prompt, compose_council_invite_person_discover_prompt,
    compose_council_invite_prompt, CouncilInviteArticleShortlistContext,
    CouncilInviteContext, CouncilInvitePersonDiscoverContext,
};
use crate::ai::business_functions::{
    daily_budget_b_live, parse_invite_article_shortlist_response,
    parse_invite_person_discover_response, parse_invite_response_with_verified_email,
    run_council_text_bf, spend_invite, InviteArticleShortlistDecision, InviteDecision,
    InvitePersonDiscoverDecision,
};
use crate::ai::business_functions::runner::{
    COUNCIL_INVITE_AGENT, COUNCIL_INVITE_ARTICLE_SHORTLIST_AGENT,
    COUNCIL_INVITE_PERSON_DISCOVER_AGENT,
};
use crate::config::Config;
use crate::content::web_search::{self, WebSearchLimits};
use crate::jobs::queue::enqueue_job_at;
use crate::ref_keys;
use crate::social::audit;
use crate::social::council_invite::deliver_platform_invite_for_review;
use crate::social::council_invite_contact::{
    profile_url_is_allowed, resolve_email_for_candidate,
};
use crate::social::council_members::{load_council_member_by_slug, CouncilMember};
use crate::social::council_portfolio::load_steward_context;
use crate::social::recruitment_outreach;

pub const INVITE_ARTICLE_POOL: i64 = 32;
pub const INVITE_ARTICLE_SHORTLIST: usize = 8;
pub const INVITE_MAX_PEOPLE: usize = 3;
pub const INVITE_PIPELINE_TTL_HOURS: i64 = 24;
/// Cap distinct `council_invite_article_pick` chain starts per councillor per UTC day (runaway guard).
/// Successful sends remain limited by `invite_budget` via `invites_spent` in the DB.
pub const INVITE_MAX_CHAINS_PER_DAY: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct StewardInviteArticle {
    pub post_id: u64,
    pub content_link_id: u64,
    pub body_plain: String,
    pub link_url: String,
    pub og_title: Option<String>,
    pub og_description: Option<String>,
    pub og_site_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvitePersonCandidate {
    pub post_id: u64,
    pub intended_label: String,
    pub profile_url: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedInviteCandidate {
    pub post_id: u64,
    pub content_link_id: u64,
    pub intended_label: Option<String>,
    pub verified_email: String,
    pub profile_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PipelineScratch {
    shortlisted_posts: Vec<StewardInviteArticle>,
    candidates: Vec<InvitePersonCandidate>,
    search_hits_by_post_id: HashMap<String, Vec<web_search::SearchHit>>,
    resolved: Option<ResolvedInviteCandidate>,
}

#[derive(Debug, Clone)]
struct InviteChainPayload {
    utc_date: String,
    ai_actor_slug: String,
    correlation_id: String,
    invites_sent: u32,
    /// Chain starts this UTC day (1 = first `article_pick` from daily tick or re-chain).
    chain_starts: u32,
    excluded_post_ids: Vec<u64>,
}

impl InviteChainPayload {
    fn from_value(v: &Value) -> Result<Self, String> {
        Ok(Self {
            utc_date: v
                .get("utc_date")
                .and_then(|x| x.as_str())
                .ok_or("missing utc_date")?
                .to_string(),
            ai_actor_slug: v
                .get("ai_actor_slug")
                .and_then(|x| x.as_str())
                .ok_or("missing ai_actor_slug")?
                .to_string(),
            correlation_id: v
                .get("correlation_id")
                .and_then(|x| x.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
            invites_sent: v
                .get("invites_sent")
                .and_then(|x| x.as_u64())
                .unwrap_or(0) as u32,
            chain_starts: v
                .get("chain_starts")
                .and_then(|x| x.as_u64())
                .unwrap_or(1) as u32,
            excluded_post_ids: v
                .get("excluded_post_ids")
                .and_then(|x| x.as_array())
                .map(|arr| arr.iter().filter_map(|x| x.as_u64()).collect())
                .unwrap_or_default(),
        })
    }

    fn base_json(&self) -> Value {
        json!({
            "utc_date": self.utc_date,
            "ai_actor_slug": self.ai_actor_slug,
            "correlation_id": self.correlation_id,
            "invites_sent": self.invites_sent,
            "chain_starts": self.chain_starts,
            "excluded_post_ids": self.excluded_post_ids,
        })
    }

    fn fresh_rechain(&self) -> Self {
        Self {
            correlation_id: Uuid::new_v4().to_string(),
            ..self.clone()
        }
    }
}

pub async fn load_steward_invite_article_pool(
    pool: &MySqlPool,
    councillor_entity_id: u64,
    steward_community_entity_id: u64,
) -> Result<Vec<StewardInviteArticle>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT
            p.id AS post_id,
            p.content_link_id AS content_link_id,
            p.body_plain,
            cl.url_canonical AS link_url,
            cl.og_title,
            cl.og_description,
            cl.og_site_name
        FROM post p
        INNER JOIN post_audience_entity pae ON pae.post_id = COALESCE(p.root_post_id, p.id)
        INNER JOIN content_link cl ON cl.id = p.content_link_id
        WHERE p.entity_id = ?
          AND pae.audience_entity_id = ?
          AND p.content_link_id IS NOT NULL
          AND p.published_at >= UTC_TIMESTAMP(6) - INTERVAL 30 DAY
          AND p.is_moderation_mirror = 0
          AND NOT EXISTS (
              SELECT 1
              FROM post root
              INNER JOIN ref_key_values rk_root
                  ON rk_root.id = root.status_id
                 AND rk_root.`group` = ?
                 AND rk_root.`key` IN (?, ?, ?)
              WHERE root.id = COALESCE(p.root_post_id, p.id)
          )
        ORDER BY p.published_at DESC
        LIMIT ?
        "#,
    )
    .bind(councillor_entity_id)
    .bind(steward_community_entity_id)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_POST_SUPPRESSED)
    .bind(ref_keys::KEY_POST_UNDER_JUDICIAL_REVIEW)
    .bind(ref_keys::KEY_FLAGGED)
    .bind(INVITE_ARTICLE_POOL)
    .fetch_all(pool)
    .await
}

async fn save_scratch(
    pool: &MySqlPool,
    chain: &InviteChainPayload,
    stage: &str,
    scratch: &PipelineScratch,
) -> Result<(), String> {
    let payload = serde_json::to_value(scratch).map_err(|e| e.to_string())?;
    let expires = (Utc::now() + Duration::hours(INVITE_PIPELINE_TTL_HOURS)).naive_utc();
    sqlx::query(
        r#"
        INSERT INTO council_invite_pipeline_state
            (correlation_id, ai_actor_slug, `utc_date`, stage, payload, expires_at)
        VALUES (?, ?, ?, ?, ?, ?)
        ON DUPLICATE KEY UPDATE stage = VALUES(stage), payload = VALUES(payload), expires_at = VALUES(expires_at)
        "#,
    )
    .bind(&chain.correlation_id)
    .bind(&chain.ai_actor_slug)
    .bind(&chain.utc_date)
    .bind(stage)
    .bind(payload)
    .bind(expires)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

async fn load_scratch(pool: &MySqlPool, correlation_id: &str) -> Result<PipelineScratch, String> {
    let row: Option<(Value,)> = sqlx::query_as(
        "SELECT payload FROM council_invite_pipeline_state WHERE correlation_id = ? LIMIT 1",
    )
    .bind(correlation_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;
    let Some((payload,)) = row else {
        return Err("invite pipeline scratch missing".to_string());
    };
    serde_json::from_value(payload).map_err(|e| e.to_string())
}

async fn delete_scratch(pool: &MySqlPool, correlation_id: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM council_invite_pipeline_state WHERE correlation_id = ?")
        .bind(correlation_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn load_member_and_budget(
    pool: &MySqlPool,
    slug: &str,
) -> Result<(CouncilMember, crate::social::council_portfolio::StewardContext, u64, u64, u64), String>
{
    let member = load_council_member_by_slug(pool, slug)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("council member not found: {slug}"))?;
    let steward = load_steward_context(pool, member.community_entity_id)
        .await
        .map_err(|e| e.to_string())?;
    let b = daily_budget_b_live(pool).await.map_err(|e| e.to_string())?;
    let invite_budget = (b / 3).max(1);
    let system_user = audit::system_user_id(pool)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "system user not configured".to_string())?;
    Ok((member, steward, b, invite_budget, system_user))
}

async fn invite_budget_remaining(
    pool: &MySqlPool,
    ai_actor_id: u64,
    invite_budget: u64,
) -> Result<bool, String> {
    let spent: Option<u32> = sqlx::query_scalar(
        r#"SELECT invites_spent FROM council_ai_daily_budget WHERE ai_actor_id = ? AND utc_date = CURDATE()"#,
    )
    .bind(ai_actor_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(spent.unwrap_or(0) < invite_budget as u32)
}

fn build_article_catalogue(posts: &[StewardInviteArticle]) -> String {
    posts
        .iter()
        .enumerate()
        .map(|(idx, post)| {
            format!(
                "post_{idx}: post_id={} content_link_id={} url=\"{}\" title=\"{}\" description=\"{}\" site=\"{}\" body=\"{}\"",
                post.post_id,
                post.content_link_id,
                post.link_url,
                post.og_title.as_deref().unwrap_or(""),
                post.og_description.as_deref().unwrap_or(""),
                post.og_site_name.as_deref().unwrap_or(""),
                post.body_plain.chars().take(400).collect::<String>(),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn article_by_post_id(posts: &[StewardInviteArticle], post_id: u64) -> Option<&StewardInviteArticle> {
    posts.iter().find(|p| p.post_id == post_id)
}

fn build_article_context_block(article: &StewardInviteArticle) -> String {
    format!(
        "post_id={} content_link_id={} url=\"{}\" title=\"{}\" description=\"{}\" site=\"{}\" body=\"{}\"",
        article.post_id,
        article.content_link_id,
        article.link_url,
        article.og_title.as_deref().unwrap_or(""),
        article.og_description.as_deref().unwrap_or(""),
        article.og_site_name.as_deref().unwrap_or(""),
        article.body_plain.chars().take(400).collect::<String>(),
    )
}

fn build_search_block(
    posts: &[StewardInviteArticle],
    hits: &HashMap<String, Vec<web_search::SearchHit>>,
) -> String {
    let mut out = String::new();
    for post in posts {
        out.push_str(&format!("\n### post_id={}\n", post.post_id));
        if let Some(list) = hits.get(&post.post_id.to_string()) {
            for (i, hit) in list.iter().enumerate() {
                out.push_str(&format!(
                    "hit_{}: title=\"{}\" url=\"{}\" snippet=\"{}\"\n",
                    i + 1,
                    hit.title,
                    hit.url,
                    hit.snippet.chars().take(300).collect::<String>()
                ));
            }
        } else {
            out.push_str("(no search hits — API disabled or no results)\n");
        }
    }
    out
}

async fn enqueue_next(
    pool: &MySqlPool,
    job_type: &str,
    payload: &Value,
    source: &str,
) -> Result<(), String> {
    let result = enqueue_job_at(
        pool,
        None,
        job_type,
        payload,
        Some(source),
        83,
        Utc::now().naive_utc(),
    )
    .await
    .map_err(|e| e.to_string())?;
    if result.job_id == 0 {
        tracing::warn!(
            job_type,
            source,
            "council invite enqueue skipped — job type missing (run migrations)"
        );
        return Err(format!("job type not registered: {job_type}"));
    }
    if result.deduplicated {
        tracing::debug!(
            job_type,
            job_id = result.job_id,
            source,
            "council invite enqueue deduplicated"
        );
    }
    Ok(())
}

pub async fn run_invite_article_pick(
    pool: &MySqlPool,
    config: &Config,
    payload: &Value,
) -> Result<(), String> {
    let chain = InviteChainPayload::from_value(payload)?;
    let (member, steward, _b, invite_budget, system_user) =
        load_member_and_budget(pool, &chain.ai_actor_slug).await?;

    if !invite_budget_remaining(pool, member.ai_actor_id, invite_budget).await? {
        tracing::info!(slug = %member.slug, "council invite article_pick — daily invite budget exhausted");
        return Ok(());
    }

    if chain.chain_starts > INVITE_MAX_CHAINS_PER_DAY {
        tracing::info!(
            slug = %member.slug,
            chain_starts = chain.chain_starts,
            cap = INVITE_MAX_CHAINS_PER_DAY,
            "council invite article_pick — daily chain start cap reached"
        );
        return Ok(());
    }

    let pool_posts: Vec<StewardInviteArticle> = load_steward_invite_article_pool(
        pool,
        member.entity_id,
        member.community_entity_id,
    )
    .await
    .map_err(|e| e.to_string())?
    .into_iter()
    .filter(|p| !chain.excluded_post_ids.contains(&p.post_id))
    .collect();

    if pool_posts.is_empty() {
        tracing::info!(slug = %member.slug, "council invite article_pick — empty steward pool");
        return Ok(());
    }

    let allowed_ids: Vec<u64> = pool_posts.iter().map(|p| p.post_id).collect();
    let catalogue = build_article_catalogue(&pool_posts);
    let agent = crate::agent_specs::load_council_agent(&member.slug).map_err(|e| e.to_string())?;
    let prompt = compose_council_invite_article_shortlist_prompt(
        &agent,
        &CouncilInviteArticleShortlistContext {
            display_name: &member.display_name,
            steward_handle: &member.community_handle,
            steward_label: &steward.label,
            steward_intention: &steward.intention,
            article_catalogue: &catalogue,
            candidate_post_ids: &allowed_ids
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        },
    );

    let run = run_council_text_bf(
        config,
        &member.slug,
        COUNCIL_INVITE_ARTICLE_SHORTLIST_AGENT,
        system_user,
        prompt,
        HashMap::new(),
    )
    .await
    .map_err(|e| e.to_string())?;

    let shortlist_ids = match parse_invite_article_shortlist_response(&run.response_text, &allowed_ids)
    {
        Some(InviteArticleShortlistDecision::Shortlist { post_ids }) => post_ids,
        Some(InviteArticleShortlistDecision::Decline { reason }) => {
            tracing::info!(
                slug = %member.slug,
                reason = reason.as_deref().unwrap_or(""),
                "council invite article shortlist declined"
            );
            return Ok(());
        }
        None => {
            tracing::warn!(slug = %member.slug, "council invite article shortlist parse failed");
            return Ok(());
        }
    };

    let shortlisted: Vec<StewardInviteArticle> = shortlist_ids
        .iter()
        .filter_map(|id| article_by_post_id(&pool_posts, *id).cloned())
        .collect();

    if shortlisted.is_empty() {
        return Ok(());
    }

    save_scratch(
        pool,
        &chain,
        "article_pick",
        &PipelineScratch {
            shortlisted_posts: shortlisted,
            candidates: Vec::new(),
            search_hits_by_post_id: HashMap::new(),
            resolved: None,
        },
    )
    .await?;

    enqueue_next(
        pool,
        "council_invite_person_discover",
        &chain.base_json(),
        "council_invite_article_pick",
    )
    .await
}

pub async fn run_invite_person_discover(
    pool: &MySqlPool,
    config: &Config,
    payload: &Value,
) -> Result<(), String> {
    let chain = InviteChainPayload::from_value(payload)?;
    let (member, steward, _b, _invite_budget, system_user) =
        load_member_and_budget(pool, &chain.ai_actor_slug).await?;
    let mut scratch = load_scratch(pool, &chain.correlation_id).await?;

    let limits = WebSearchLimits::default();
    let mut queries_run = 0u8;
    let mut hits_map: HashMap<String, Vec<web_search::SearchHit>> = HashMap::new();

    for post in &scratch.shortlisted_posts {
        if queries_run >= limits.max_queries {
            break;
        }
        let title = post
            .og_title
            .as_deref()
            .unwrap_or("")
            .trim()
            .to_string();
        if title.is_empty() {
            continue;
        }
        let q1 = web_search::sanitize_query(&format!("{title} author contact"));
        if !q1.is_empty() {
            let hits = web_search::search(config, &q1, limits.hits_per_query).await;
            queries_run += 1;
            hits_map
                .entry(post.post_id.to_string())
                .or_default()
                .extend(hits);
        }
        if queries_run >= limits.max_queries {
            break;
        }
        if let Some(site) = post.og_site_name.as_deref().filter(|s| !s.is_empty()) {
            let q2 = web_search::sanitize_query(&format!("{title} {site} interview"));
            if !q2.is_empty() {
                let hits = web_search::search(config, &q2, limits.hits_per_query).await;
                queries_run += 1;
                hits_map
                    .entry(post.post_id.to_string())
                    .or_default()
                    .extend(hits);
            }
        }
    }

    if !config.web_search_configured() {
        tracing::info!(
            slug = %member.slug,
            "council invite person_discover — web search not configured; OG/byline signals only"
        );
    }

    scratch.search_hits_by_post_id = hits_map.clone();
    let search_block = build_search_block(&scratch.shortlisted_posts, &hits_map);
    let articles_block = build_article_catalogue(&scratch.shortlisted_posts);

    let agent = crate::agent_specs::load_council_agent(&member.slug).map_err(|e| e.to_string())?;
    let prompt = compose_council_invite_person_discover_prompt(
        &agent,
        &CouncilInvitePersonDiscoverContext {
            display_name: &member.display_name,
            steward_handle: &member.community_handle,
            steward_label: &steward.label,
            steward_intention: &steward.intention,
            shortlisted_articles: &articles_block,
            search_hits: &search_block,
        },
    );

    let run = run_council_text_bf(
        config,
        &member.slug,
        COUNCIL_INVITE_PERSON_DISCOVER_AGENT,
        system_user,
        prompt,
        HashMap::new(),
    )
    .await
    .map_err(|e| e.to_string())?;

    let allowed_post_ids: Vec<u64> = scratch
        .shortlisted_posts
        .iter()
        .map(|p| p.post_id)
        .collect();

    let candidates = match parse_invite_person_discover_response(&run.response_text, &allowed_post_ids)
    {
        Some(InvitePersonDiscoverDecision::Candidates { people }) => people
            .into_iter()
            .map(|p| InvitePersonCandidate {
                post_id: p.post_id,
                intended_label: p.intended_label,
                profile_url: p.profile_url,
                reason: p.reason,
            })
            .collect::<Vec<_>>(),
        Some(InvitePersonDiscoverDecision::Decline { reason }) => {
            tracing::info!(
                slug = %member.slug,
                reason = reason.as_deref().unwrap_or(""),
                "council invite person discover declined"
            );
            delete_scratch(pool, &chain.correlation_id).await.ok();
            return Ok(());
        }
        None => {
            tracing::warn!(slug = %member.slug, "council invite person discover parse failed");
            delete_scratch(pool, &chain.correlation_id).await.ok();
            return Ok(());
        }
    };

    scratch.candidates = candidates;
    save_scratch(pool, &chain, "person_discover", &scratch).await?;

    enqueue_next(
        pool,
        "council_invite_contact_resolve",
        &chain.base_json(),
        "council_invite_person_discover",
    )
    .await
}

pub async fn run_invite_contact_resolve(
    pool: &MySqlPool,
    config: &Config,
    payload: &Value,
) -> Result<(), String> {
    let chain = InviteChainPayload::from_value(payload)?;
    let (member, _steward, _b, _invite_budget, _system_user) =
        load_member_and_budget(pool, &chain.ai_actor_slug).await?;
    let mut scratch = load_scratch(pool, &chain.correlation_id).await?;
    let search_configured = config.web_search_configured();

    for candidate in scratch.candidates.iter().take(INVITE_MAX_PEOPLE) {
        let Some(article) = article_by_post_id(&scratch.shortlisted_posts, candidate.post_id) else {
            continue;
        };
        let hits: Vec<web_search::SearchHit> = scratch
            .search_hits_by_post_id
            .get(&candidate.post_id.to_string())
            .cloned()
            .unwrap_or_default();

        if !profile_url_is_allowed(
            pool,
            &candidate.profile_url,
            article,
            &hits,
            search_configured,
        )
        .await
        .map_err(|e| e.to_string())?
        {
            tracing::info!(
                slug = %member.slug,
                post_id = candidate.post_id,
                profile_url = %candidate.profile_url,
                "council invite contact_resolve — profile_url not grounded"
            );
            continue;
        }

        let snippets: Vec<String> = hits.iter().map(|h| h.snippet.clone()).collect();

        let resolved = resolve_email_for_candidate(
            pool,
            &candidate.profile_url,
            article,
            &snippets,
        )
        .await
        .map_err(|e| e.to_string())?;

        if let Some(contact) = resolved {
            scratch.resolved = Some(ResolvedInviteCandidate {
                post_id: candidate.post_id,
                content_link_id: article.content_link_id,
                intended_label: Some(candidate.intended_label.clone()),
                verified_email: contact.email,
                profile_url: candidate.profile_url.clone(),
            });
            save_scratch(pool, &chain, "contact_resolve", &scratch).await?;
            enqueue_next(
                pool,
                "council_invite_compose_send",
                &chain.base_json(),
                "council_invite_contact_resolve",
            )
            .await?;
            return Ok(());
        }
    }

    tracing::info!(
        slug = %member.slug,
        "council invite contact_resolve — no verifiable email for candidates"
    );
    delete_scratch(pool, &chain.correlation_id).await.ok();
    Ok(())
}

pub async fn run_invite_compose_send(
    pool: &MySqlPool,
    config: &Config,
    payload: &Value,
) -> Result<(), String> {
    let chain = InviteChainPayload::from_value(payload)?;
    let (member, steward, _b, invite_budget, system_user) =
        load_member_and_budget(pool, &chain.ai_actor_slug).await?;
    let scratch = load_scratch(pool, &chain.correlation_id).await?;

    let Some(resolved) = scratch.resolved else {
        delete_scratch(pool, &chain.correlation_id).await.ok();
        return Ok(());
    };

    let Some(article) = article_by_post_id(&scratch.shortlisted_posts, resolved.post_id) else {
        delete_scratch(pool, &chain.correlation_id).await.ok();
        return Ok(());
    };

    let article_context = build_article_context_block(article);
    let agent = crate::agent_specs::load_council_agent(&member.slug).map_err(|e| e.to_string())?;
    let prompt = compose_council_invite_prompt(
        &agent,
        &CouncilInviteContext {
            display_name: &member.display_name,
            steward_handle: &member.community_handle,
            steward_label: &steward.label,
            steward_intention: &steward.intention,
            verified_email: &resolved.verified_email,
            intended_label: resolved.intended_label.as_deref(),
            article_context: &article_context,
        },
    );

    let run = run_council_text_bf(
        config,
        &member.slug,
        COUNCIL_INVITE_AGENT,
        system_user,
        prompt,
        HashMap::new(),
    )
    .await
    .map_err(|e| e.to_string())?;

    let decision = parse_invite_response_with_verified_email(
        &run.response_text,
        Some(&resolved.verified_email),
    );
    let Some(decision) = decision else {
        tracing::warn!(slug = %member.slug, "council invite compose parse failed");
        delete_scratch(pool, &chain.correlation_id).await.ok();
        return Ok(());
    };

    match decision {
        InviteDecision::Decline { reason } => {
            tracing::info!(
                slug = %member.slug,
                reason = reason.as_deref().unwrap_or(""),
                "council invite compose declined"
            );
            delete_scratch(pool, &chain.correlation_id).await.ok();
            Ok(())
        }
        InviteDecision::Invite {
            intended_email,
            intended_label,
            personal_note,
        } => {
            let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
            if !recruitment_outreach::claim_outreach_tx(
                &mut tx,
                &intended_email,
                member.entity_id,
                Some(member.ai_actor_id),
                Some(resolved.post_id),
                Some(resolved.content_link_id),
                intended_label.as_deref(),
                "review_routed",
            )
            .await
            .map_err(|e| e.to_string())?
            {
                tx.rollback().await.ok();
                delete_scratch(pool, &chain.correlation_id).await.ok();
                return Ok(());
            }
            if !spend_invite(pool, &mut tx, member.ai_actor_id, invite_budget)
                .await
                .map_err(|e| e.to_string())?
            {
                tx.rollback().await.ok();
                delete_scratch(pool, &chain.correlation_id).await.ok();
                return Ok(());
            }
            tx.commit().await.map_err(|e| e.to_string())?;

            // Council AI: ALWAYS review-routed — never direct SMTP to the intended recipient.
            deliver_platform_invite_for_review(
                pool,
                config,
                member.entity_id,
                &intended_email,
                intended_label.as_deref(),
                personal_note.as_deref(),
            )
            .await?;

            delete_scratch(pool, &chain.correlation_id).await.ok();

            let invites_sent = chain.invites_sent + 1;
            let rechain =
                invite_budget_remaining(pool, member.ai_actor_id, invite_budget).await?;

            tracing::info!(
                slug = %member.slug,
                intended_email = %intended_email,
                post_id = resolved.post_id,
                invites_sent,
                invite_budget,
                chain_starts = chain.chain_starts,
                rechain,
                "council invite routed to review inbox"
            );

            if rechain {
                let mut next = chain.fresh_rechain();
                next.invites_sent = invites_sent;
                next.chain_starts = chain.chain_starts + 1;
                next.excluded_post_ids.push(resolved.post_id);
                enqueue_next(
                    pool,
                    "council_invite_article_pick",
                    &next.base_json(),
                    "council_invite_compose_send",
                )
                .await?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::social::council_invite_contact::verification_blob_for_article;

    #[test]
    fn verification_blob_from_article_includes_url_and_body() {
        let article = StewardInviteArticle {
            post_id: 5,
            content_link_id: 9,
            body_plain: "hello".into(),
            link_url: "https://x.test/a".into(),
            og_title: Some("Title".into()),
            og_description: None,
            og_site_name: None,
        };
        let blob = verification_blob_for_article(&article);
        assert!(blob.contains("https://x.test/a"));
        assert!(blob.contains("hello"));
    }

    #[test]
    fn invite_chain_payload_roundtrip_with_exclusions() {
        let v = json!({
            "utc_date": "2026-08-15",
            "ai_actor_slug": "epic",
            "correlation_id": "abc",
            "invites_sent": 1,
            "chain_starts": 2,
            "excluded_post_ids": [10, 20],
        });
        let chain = InviteChainPayload::from_value(&v).expect("parse");
        assert_eq!(chain.invites_sent, 1);
        assert_eq!(chain.excluded_post_ids, vec![10, 20]);
        let next = chain.fresh_rechain();
        assert_ne!(next.correlation_id, chain.correlation_id);
        assert_eq!(next.invites_sent, 1);
        let out = next.base_json();
        assert_eq!(out["invites_sent"], 1);
        assert_eq!(out["excluded_post_ids"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn invite_chain_payload_includes_chain_starts() {
        let v = json!({
            "utc_date": "2026-08-15",
            "ai_actor_slug": "epic",
            "correlation_id": "abc",
            "invites_sent": 1,
            "chain_starts": 2,
            "excluded_post_ids": [10, 20],
        });
        let chain = InviteChainPayload::from_value(&v).expect("parse");
        assert_eq!(chain.chain_starts, 2);
        assert_eq!(chain.invites_sent, 1);
    }

    #[test]
    fn chain_starts_defaults_to_one() {
        let v = json!({
            "utc_date": "2026-08-15",
            "ai_actor_slug": "epic",
            "correlation_id": "abc",
            "invites_sent": 0,
        });
        let chain = InviteChainPayload::from_value(&v).expect("parse");
        assert_eq!(chain.chain_starts, 1);
    }
}
