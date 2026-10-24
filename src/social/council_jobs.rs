//! Council AI batch job handlers: reply / post / invite ticks.

use chrono::{Duration, Timelike, Utc};
use serde_json::json;
use sqlx::MySqlPool;

use crate::ai::business_functions::{
    accrue_reply_credit, daily_reply_budget_b_live, parse_thread_reply_response,
    reply_credit_available, thread_reply_parse_diagnosis,
    run_council_text_bf, spend_reply, ThreadReplyDecision,
};
use crate::config::Config;
use crate::content::portfolio::COUNCIL_POST_MEMBER_STAGGER_MINS;
use crate::jobs::enqueue_job_at;
use crate::social::audit;
use crate::social::council_members::{load_active_council_members, load_council_member_by_slug, CouncilMember};
use crate::social::council_portfolio::load_steward_context;
use crate::social::council_publish::publish_reply_tx;
use crate::social::council_thread_dossier::build_council_reply_briefing;
use crate::agent_specs::{
    compose_council_thread_reply_prompt, CouncilThreadReplyContext,
};
use crate::ai::business_functions::runner::COUNCIL_THREAD_REPLY_AGENT;
use rust_decimal::Decimal;
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct CouncilTickSummary {
    pub members_examined: u64,
    pub actions_taken: u64,
    pub declines: u64,
    pub errors: u64,
    /// Set when this worker returned early because the period claim was already held.
    pub skipped_period_claim: bool,
}

/// Seconds between per-councillor reply jobs. Reply members carry a 15-minute
/// `period_key` and are purged/skipped once the bucket passes, so all 12 must
/// land inside one bucket (12 × 60s = 12 min). Do NOT reuse the 5-minute daily
/// share stagger here — that starves councillors past index 2.
pub const COUNCIL_REPLY_MEMBER_STAGGER_SECS: i64 = 60;

pub async fn run_reply_tick(pool: &MySqlPool, _config: &Config) -> Result<CouncilTickSummary, String> {
    let period_key = council_reply_period_key();

    let members = load_active_council_members(pool)
        .await
        .map_err(|e| e.to_string())?;

    let mut summary = CouncilTickSummary::default();
    summary.members_examined = members.len() as u64;

    if members.is_empty() {
        tracing::warn!(
            period_key = %period_key,
            "council_reply_tick skipped — no active council members"
        );
        return Ok(summary);
    }

    if !crate::jobs::claim_ops_period_once(pool, "council_reply_tick", &period_key)
        .await
        .map_err(|e| e.to_string())?
    {
        tracing::info!(
            period_key = %period_key,
            "council_reply_tick skipped — fan-out already claimed for period"
        );
        summary.skipped_period_claim = true;
        return Ok(summary);
    }

    let base = Utc::now();
    let stagger = Duration::seconds(COUNCIL_REPLY_MEMBER_STAGGER_SECS);

    for (idx, member) in members.iter().enumerate() {
        let next_run = (base + stagger * idx as i32).naive_utc();
        let payload = json!({
            "period_key": period_key,
            "ai_actor_slug": member.slug,
        });
        match enqueue_job_at(
            pool,
            None,
            "council_reply_member",
            &payload,
            Some("council_reply_tick"),
            80,
            next_run,
        )
        .await
        {
            Ok(result) => {
                if result.job_id == 0 {
                    tracing::warn!(
                        slug = %member.slug,
                        "council_reply_member job type missing — run migrations"
                    );
                    summary.errors += 1;
                } else if result.deduplicated {
                    summary.declines += 1;
                } else {
                    summary.actions_taken += 1;
                }
            }
            Err(e) => {
                tracing::warn!(slug = %member.slug, error = %e, "council reply member enqueue failed");
                summary.errors += 1;
            }
        }
    }
    Ok(summary)
}

/// Per-councillor reply worker: one LLM attempt for a single steward mesh.
pub async fn run_reply_member_tick(
    pool: &MySqlPool,
    config: &Config,
    ai_actor_slug: &str,
    period_key: &str,
    lease_job_id: Option<u64>,
) -> Result<CouncilTickSummary, String> {
    let member_period = format!("{period_key}:{ai_actor_slug}");
    if !crate::jobs::claim_ops_period_once(
        pool,
        "council_reply_member",
        &member_period,
    )
    .await
    .map_err(|e| e.to_string())?
    {
        tracing::info!(
            slug = %ai_actor_slug,
            period_key = %period_key,
            "council_reply_member skipped — period claim already held"
        );
        let mut summary = CouncilTickSummary::default();
        summary.members_examined = 1;
        summary.declines = 1;
        summary.skipped_period_claim = true;
        return Ok(summary);
    }

    let b = daily_reply_budget_b_live(pool)
        .await
        .map_err(|e| e.to_string())?;
    let member = load_council_member_by_slug(pool, ai_actor_slug)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("council member not found: {ai_actor_slug}"))?;
    let system_user = audit::system_user_id(pool)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "system user not configured".to_string())?;

    let mut summary = CouncilTickSummary::default();
    summary.members_examined = 1;

    if let Some(job_id) = lease_job_id {
        let _ = crate::jobs::extend_job_lease(pool, job_id).await;
    }

    if let Err(e) = accrue_reply_credit(pool, member.ai_actor_id, b).await {
        tracing::warn!(slug = %member.slug, error = %e, "council reply credit accrual failed");
        summary.errors = 1;
        return Ok(summary);
    }

    let credit = match reply_credit_available(pool, member.ai_actor_id).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(slug = %member.slug, error = %e, "council reply credit read failed");
            summary.errors = 1;
            return Ok(summary);
        }
    };
    if credit < Decimal::ONE {
        tracing::info!(
            slug = %member.slug,
            reply_credit = %credit,
            "council reply skipped — credit below 1"
        );
        summary.declines = 1;
        return Ok(summary);
    }

    match attempt_reply(pool, config, &member, system_user, b).await {
        Ok(true) => summary.actions_taken = 1,
        Ok(false) => summary.declines = 1,
        Err(e) => {
            tracing::warn!(slug = %member.slug, error = %e, "council reply tick failed");
            summary.errors = 1;
        }
    }
    Ok(summary)
}

async fn attempt_reply(
    pool: &MySqlPool,
    config: &Config,
    member: &CouncilMember,
    system_user: u64,
    b: u64,
) -> Result<bool, String> {
    let (thread_doc, parent_ids) = build_council_reply_briefing(pool, member.entity_id)
        .await
        .map_err(|e| e.to_string())?;
    if parent_ids.is_empty() {
        tracing::info!(
            slug = %member.slug,
            dossier_chars = thread_doc.len(),
            "council reply skipped — no eligible parent posts in thread dossiers"
        );
        return Ok(false);
    }

    tracing::info!(
        slug = %member.slug,
        dossier_chars = thread_doc.len(),
        candidate_parents = parent_ids.len(),
        candidate_parent_ids = %parent_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(","),
        "council reply dossier assembled"
    );

    let id_list = parent_ids
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(", ");

    let steward = load_steward_context(pool, member.community_entity_id)
        .await
        .map_err(|e| e.to_string())?;

    let agent = crate::agent_specs::load_council_agent(&member.slug)?;
    let prompt = compose_council_thread_reply_prompt(
        &agent,
        &CouncilThreadReplyContext {
            display_name: &member.display_name,
            steward_handle: &member.community_handle,
            steward_label: &steward.label,
            steward_intention: &steward.intention,
            thread_document: &thread_doc,
            candidate_parent_ids: &id_list,
        },
    );

    let run = run_council_text_bf(
        config,
        &member.slug,
        COUNCIL_THREAD_REPLY_AGENT,
        system_user,
        prompt,
        HashMap::new(),
    )
    .await
    .map_err(|e| e.to_string())?;

    let decision = parse_thread_reply_response(&run.response_text, &parent_ids);
    let Some(decision) = decision else {
        let diagnosis = thread_reply_parse_diagnosis(&run.response_text, &parent_ids);
        tracing::info!(
            slug = %member.slug,
            parse_diagnosis = diagnosis,
            response_preview = %run.response_text.chars().take(240).collect::<String>(),
            "council reply skipped — unparseable model response"
        );
        return Ok(false);
    };

    match decision {
        ThreadReplyDecision::Decline { reason } => {
            tracing::info!(
                slug = %member.slug,
                reason = ?reason,
                "council reply declined by model"
            );
            Ok(false)
        }
        ThreadReplyDecision::Reply {
            parent_post_id,
            body_markup,
        } => {
            let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
            if !spend_reply(pool, &mut tx, member.ai_actor_id, b)
                .await
                .map_err(|e| e.to_string())?
            {
                tx.rollback().await.ok();
                tracing::info!(
                    slug = %member.slug,
                    daily_budget_b = b,
                    "council reply skipped — reply budget exhausted or credit race"
                );
                return Ok(false);
            }
            let post_id = publish_reply_tx(
                &mut tx,
                member.entity_id,
                system_user,
                parent_post_id,
                &body_markup,
            )
            .await?;
            tx.commit().await.map_err(|e| e.to_string())?;
            tracing::info!(
                slug = %member.slug,
                post_id,
                parent_post_id,
                "council reply published"
            );
            Ok(true)
        }
    }
}

/// Priority for council article-share pipeline jobs (shortlist / read / publish).
/// Sits above recurring settlement work and below reply tick (80) so fair batch
/// planning reaches shares before lower-priority daily coordinators.
pub const COUNCIL_SHARE_JOB_PRIORITY: i16 = 76;

/// Daily coordinator: enqueue one staggered `council_share_shortlist` job per active councillor.
pub async fn run_post_tick(pool: &MySqlPool, _config: &Config) -> Result<CouncilTickSummary, String> {
    let utc_date = council_daily_period_key();

    let members = load_active_council_members(pool)
        .await
        .map_err(|e| e.to_string())?;

    let mut summary = CouncilTickSummary::default();
    summary.members_examined = members.len() as u64;

    if members.is_empty() {
        tracing::warn!(
            utc_date = %utc_date,
            "council_post_tick skipped — no active council members"
        );
        return Ok(summary);
    }

    if !crate::jobs::claim_ops_period_once(pool, "council_post_tick", &utc_date)
        .await
        .map_err(|e| e.to_string())?
    {
        tracing::info!(
            utc_date = %utc_date,
            "council_post_tick skipped — fan-out already claimed for day"
        );
        summary.skipped_period_claim = true;
        return Ok(summary);
    }

    let base = Utc::now();
    let stagger = Duration::minutes(COUNCIL_POST_MEMBER_STAGGER_MINS);

    for (idx, member) in members.iter().enumerate() {
        let next_run = (base + stagger * idx as i32).naive_utc();
        let payload = json!({
            "utc_date": utc_date,
            "ai_actor_slug": member.slug,
            "published_count": 0,
            "shortlist_attempt": 0,
            "excluded_link_ids": [],
            "excluded_domains": [],
        });
        match enqueue_job_at(
            pool,
            None,
            "council_share_shortlist",
            &payload,
            Some("council_post_tick"),
            COUNCIL_SHARE_JOB_PRIORITY,
            next_run,
        )
        .await
        {
            Ok(result) => {
                if result.job_id == 0 {
                    tracing::warn!(
                        slug = %member.slug,
                        "council_share_shortlist job type missing — run migrations"
                    );
                    summary.errors += 1;
                } else if result.deduplicated {
                    summary.declines += 1;
                } else {
                    summary.actions_taken += 1;
                }
            }
            Err(e) => {
                tracing::warn!(slug = %member.slug, error = %e, "council share shortlist enqueue failed");
                summary.errors += 1;
            }
        }
    }
    Ok(summary)
}

/// Per-councillor worker (legacy): runs shortlist stage directly.
pub async fn run_post_member_tick(
    pool: &MySqlPool,
    config: &Config,
    ai_actor_slug: &str,
) -> Result<u32, String> {
    let utc_date = council_daily_period_key();
    crate::social::council_share_pipeline::run_post_member_compat(
        pool,
        config,
        ai_actor_slug,
        &utc_date,
    )
    .await?;
    Ok(0)
}

/// Daily coordinator: enqueue `council_invite_article_pick` per active councillor (staggered).
pub async fn run_invite_tick(pool: &MySqlPool, _config: &Config) -> Result<CouncilTickSummary, String> {
    let utc_date = council_daily_period_key();

    let members = load_active_council_members(pool)
        .await
        .map_err(|e| e.to_string())?;

    let mut summary = CouncilTickSummary::default();
    summary.members_examined = members.len() as u64;

    if members.is_empty() {
        tracing::warn!(
            utc_date = %utc_date,
            "council_invite_tick skipped — no active council members"
        );
        return Ok(summary);
    }

    if !crate::jobs::claim_ops_period_once(pool, "council_invite_tick", &utc_date)
        .await
        .map_err(|e| e.to_string())?
    {
        tracing::info!(
            utc_date = %utc_date,
            "council_invite_tick skipped — fan-out already claimed for day"
        );
        summary.skipped_period_claim = true;
        return Ok(summary);
    }

    let base = Utc::now();
    let stagger = Duration::minutes(COUNCIL_POST_MEMBER_STAGGER_MINS);

    for (idx, member) in members.iter().enumerate() {
        let next_run = (base + stagger * idx as i32).naive_utc();
        let payload = json!({
            "utc_date": utc_date,
            "ai_actor_slug": member.slug,
            "invites_sent": 0,
            "chain_starts": 1,
            "excluded_post_ids": [],
        });
        match enqueue_job_at(
            pool,
            None,
            "council_invite_article_pick",
            &payload,
            Some("council_invite_tick"),
            83,
            next_run,
        )
        .await
        {
            Ok(result) => {
                if result.job_id == 0 {
                    tracing::warn!(
                        slug = %member.slug,
                        "council_invite_article_pick job type missing — run migrations"
                    );
                    summary.errors += 1;
                } else if result.deduplicated {
                    summary.declines += 1;
                } else {
                    summary.actions_taken += 1;
                }
            }
            Err(e) => {
                tracing::warn!(slug = %member.slug, error = %e, "council invite article_pick enqueue failed");
                summary.errors += 1;
            }
        }
    }
    Ok(summary)
}

/// True when `period_key` is from a prior 15-minute bucket (safe to no-op stale queue rows).
#[must_use]
pub fn is_stale_reply_period(period_key: &str) -> bool {
    let current = council_reply_period_key();
    period_key.trim() < current.as_str()
}

/// True when `utc_date` is before today's UTC date (safe to no-op stale daily queue rows).
#[must_use]
pub fn is_stale_daily_period(utc_date: &str) -> bool {
    utc_date.trim() < council_daily_period_key().as_str()
}

/// 15-minute bucket key for scheduler dedupe (`YYYY-MM-DDTHH:MM` with MM ∈ {00,15,30,45}).
#[must_use]
pub fn council_reply_period_key() -> String {
    let now = Utc::now();
    let minute = (now.minute() / 15) * 15;
    format!(
        "{}T{:02}:{:02}",
        now.format("%Y-%m-%d"),
        now.hour(),
        minute
    )
}

/// UTC date key for daily council jobs.
#[must_use]
pub fn council_daily_period_key() -> String {
    Utc::now().format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_daily_period_before_today() {
        let today = council_daily_period_key();
        assert!(is_stale_daily_period("2020-01-01"));
        assert!(!is_stale_daily_period(&today));
    }

    #[test]
    fn reply_member_stagger_fits_reply_period_bucket() {
        // All 12 councillors must be due within one 15-minute reply bucket,
        // otherwise purge_stale_period_jobs deletes the late members unrun.
        assert!(12 * COUNCIL_REPLY_MEMBER_STAGGER_SECS < 15 * 60);
    }
}
