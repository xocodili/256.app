//! AI council votes on `escalated_to_ai` **post-dispute appeals**.
//!
//! `judicial_council_appeal` is a **coordinator** that enqueues one
//! `judicial_council_vote_member` job per missing (case, councillor) pair.
//! Each member job runs a single LLM moderation review and records one vote.

use std::collections::HashMap;

use sqlx::MySqlPool;

use crate::agent_specs::{
    self, compose_council_moderation_review_prompt, CouncilModerationReviewContext,
};
use crate::ai::business_functions::parse_moderation_review_response;
use crate::ai::business_functions::response::ModerationVote;
use crate::ai::grpc::{self, COUNCIL_MODERATION_REVIEW_AGENT};
use crate::ref_key_values;
use crate::ref_keys;
use crate::social::audit;
use crate::social::judicial::{self, VerdictOutcome};

const COUNCIL_QUORUM: usize = 12;

#[derive(Debug, Clone, Default)]
pub struct CouncilAppealJobSummary {
    pub cases_examined: u64,
    pub votes_recorded: u64,
    pub cases_resolved: u64,
}

#[derive(Debug, Clone, Default)]
pub struct CouncilAppealCoordinatorSummary {
    pub cases_examined: u64,
    pub vote_jobs_enqueued: u64,
    pub vote_jobs_deduplicated: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CouncilMemberVoteOutcome {
    Recorded,
    AlreadyVoted,
    CaseNotOpen,
    GrpcFailed,
}

/// Coordinator: enqueue one `judicial_council_vote_member` job per missing (case, councillor) pair.
pub async fn enqueue_escalated_appeal_vote_jobs(
    pool: &MySqlPool,
    case_limit: u64,
    focus_case_id: Option<u64>,
) -> Result<CouncilAppealCoordinatorSummary, sqlx::Error> {
    use chrono::Duration;
    use crate::content::portfolio::COUNCIL_POST_MEMBER_STAGGER_MINS;
    use crate::jobs::queue::enqueue_job_at;

    let mut summary = CouncilAppealCoordinatorSummary::default();
    let cases = load_escalated_appeal_cases(pool, case_limit, focus_case_id).await?;
    summary.cases_examined = cases.len() as u64;

    let members = load_active_council_members(pool).await?;
    if members.is_empty() {
        tracing::warn!("judicial council appeal coordinator: no active council members");
        return Ok(summary);
    }

    let base = chrono::Utc::now();
    let stagger = Duration::minutes(COUNCIL_POST_MEMBER_STAGGER_MINS);
    let mut stagger_idx = 0i32;

    for case in cases {
        for member in &members {
            let already: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM judicial_case_council_vote
                    WHERE judicial_case_id = ? AND ai_actor_id = ?
                )
                "#,
            )
            .bind(case.id)
            .bind(member.ai_actor_id)
            .fetch_one(pool)
            .await?;
            if already {
                continue;
            }

            let next_run = (base + stagger * stagger_idx).naive_utc();
            stagger_idx += 1;

            let payload = serde_json::json!({
                "judicial_case_id": case.id,
                "ai_actor_slug": member.slug,
            });
            let corr = format!("council-appeal-{}-{}", case.id, member.slug);
            match enqueue_job_at(
                pool,
                None,
                "judicial_council_vote_member",
                &payload,
                Some(&corr),
                55,
                next_run,
            )
            .await
            {
                Ok(result) if result.job_id == 0 => {
                    tracing::warn!(
                        case_id = case.id,
                        slug = %member.slug,
                        "judicial_council_vote_member job type missing — run migrations"
                    );
                }
                Ok(result) if result.deduplicated => {
                    summary.vote_jobs_deduplicated += 1;
                }
                Ok(_result) => {
                    summary.vote_jobs_enqueued += 1;
                }
                Err(e) => {
                    tracing::warn!(
                        case_id = case.id,
                        slug = %member.slug,
                        error = %e,
                        "judicial council vote member enqueue failed"
                    );
                }
            }
        }

        let _ = try_resolve_council_case(pool, case.id).await?;
    }

    Ok(summary)
}

/// Per-councillor worker: one LLM moderation review vote on an escalated appeal.
pub async fn record_council_member_vote(
    pool: &MySqlPool,
    config: &crate::config::Config,
    judicial_case_id: u64,
    ai_actor_slug: &str,
) -> Result<CouncilMemberVoteOutcome, sqlx::Error> {
    let case = load_escalated_appeal_case_by_id(pool, judicial_case_id).await?;
    let Some(case) = case else {
        return Ok(CouncilMemberVoteOutcome::CaseNotOpen);
    };

    let members = load_active_council_members(pool).await?;
    let Some(member) = members.into_iter().find(|m| m.slug == ai_actor_slug) else {
        return Ok(CouncilMemberVoteOutcome::CaseNotOpen);
    };

    let already: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM judicial_case_council_vote
            WHERE judicial_case_id = ? AND ai_actor_id = ?
        )
        "#,
    )
    .bind(case.id)
    .bind(member.ai_actor_id)
    .fetch_one(pool)
    .await?;
    if already {
        let _ = try_resolve_council_case(pool, case.id).await?;
        return Ok(CouncilMemberVoteOutcome::AlreadyVoted);
    }

    let recorded = record_one_council_vote(pool, config, &case, &member).await?;
    if recorded {
        let _ = try_resolve_council_case(pool, case.id).await?;
        Ok(CouncilMemberVoteOutcome::Recorded)
    } else {
        Ok(CouncilMemberVoteOutcome::GrpcFailed)
    }
}

/// Legacy batch entry — forwards to the coordinator (no inline LLM loop).
pub async fn process_escalated_appeals(
    pool: &MySqlPool,
    _config: &crate::config::Config,
    case_limit: u64,
    _vote_limit: u64,
) -> Result<CouncilAppealJobSummary, sqlx::Error> {
    let coord = enqueue_escalated_appeal_vote_jobs(pool, case_limit, None).await?;
    Ok(CouncilAppealJobSummary {
        cases_examined: coord.cases_examined,
        votes_recorded: coord.vote_jobs_enqueued,
        cases_resolved: 0,
    })
}

#[derive(Debug, sqlx::FromRow)]
struct EscalatedAppealCase {
    id: u64,
    subject_post_id: Option<u64>,
    appealed_from_judicial_case_id: Option<u64>, // used in build_review_context via case ref
}

#[derive(Debug, sqlx::FromRow)]
struct CouncilMember {
    ai_actor_id: u64,
    slug: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CouncilVoteDirection {
    Uphold,
    Overturn,
}

/// Process pending council votes for AI-escalated appeals (coordinator only).
pub async fn process_escalated_appeals_coordinator(
    pool: &MySqlPool,
    case_limit: u64,
    focus_case_id: Option<u64>,
) -> Result<CouncilAppealCoordinatorSummary, sqlx::Error> {
    enqueue_escalated_appeal_vote_jobs(pool, case_limit, focus_case_id).await
}

async fn load_escalated_appeal_cases(
    pool: &MySqlPool,
    limit: u64,
    focus_case_id: Option<u64>,
) -> Result<Vec<EscalatedAppealCase>, sqlx::Error> {
    if let Some(case_id) = focus_case_id {
        let row: Option<EscalatedAppealCase> = sqlx::query_as(
            r#"
            SELECT jc.id, jc.subject_post_id, jc.appealed_from_judicial_case_id
            FROM judicial_case jc
            INNER JOIN ref_key_values rk_status
                ON rk_status.id = jc.status_id
               AND rk_status.`group` = ?
               AND rk_status.`key` = ?
               AND rk_status.is_active = 1
            INNER JOIN ref_key_values rk_trigger
                ON rk_trigger.id = jc.trigger_type_id
               AND rk_trigger.`group` = ?
               AND rk_trigger.`key` = ?
               AND rk_trigger.is_active = 1
            INNER JOIN ref_key_values rk_tier
                ON rk_tier.id = jc.resolution_tier_id
               AND rk_tier.`group` = ?
               AND rk_tier.`key` = ?
               AND rk_tier.is_active = 1
            WHERE jc.closed_at IS NULL
              AND jc.id = ?
            LIMIT 1
            "#,
        )
        .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
        .bind(ref_keys::KEY_JUDICIAL_CASE_ESCALATED_TO_AI)
        .bind(ref_keys::GROUP_JUDICIAL_CASE_TRIGGER)
        .bind(ref_keys::KEY_JUDICIAL_TRIGGER_CASE_APPEAL)
        .bind(ref_keys::GROUP_JUDICIAL_RESOLUTION_TIER)
        .bind(ref_keys::KEY_JUDICIAL_RESOLUTION_TIER_AI_COUNCIL)
        .bind(case_id)
        .fetch_optional(pool)
        .await?;
        return Ok(row.into_iter().collect());
    }

    sqlx::query_as(
        r#"
        SELECT jc.id, jc.subject_post_id, jc.appealed_from_judicial_case_id
        FROM judicial_case jc
        INNER JOIN ref_key_values rk_status
            ON rk_status.id = jc.status_id
           AND rk_status.`group` = ?
           AND rk_status.`key` = ?
           AND rk_status.is_active = 1
        INNER JOIN ref_key_values rk_trigger
            ON rk_trigger.id = jc.trigger_type_id
           AND rk_trigger.`group` = ?
           AND rk_trigger.`key` = ?
           AND rk_trigger.is_active = 1
        INNER JOIN ref_key_values rk_tier
            ON rk_tier.id = jc.resolution_tier_id
           AND rk_tier.`group` = ?
           AND rk_tier.`key` = ?
           AND rk_tier.is_active = 1
        WHERE jc.closed_at IS NULL
        ORDER BY jc.opened_at ASC, jc.id ASC
        LIMIT ?
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(ref_keys::KEY_JUDICIAL_CASE_ESCALATED_TO_AI)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_TRIGGER)
    .bind(ref_keys::KEY_JUDICIAL_TRIGGER_CASE_APPEAL)
    .bind(ref_keys::GROUP_JUDICIAL_RESOLUTION_TIER)
    .bind(ref_keys::KEY_JUDICIAL_RESOLUTION_TIER_AI_COUNCIL)
    .bind(limit.max(1))
    .fetch_all(pool)
    .await
}

async fn load_escalated_appeal_case_by_id(
    pool: &MySqlPool,
    case_id: u64,
) -> Result<Option<EscalatedAppealCase>, sqlx::Error> {
    Ok(load_escalated_appeal_cases(pool, 1, Some(case_id))
        .await?
        .into_iter()
        .next())
}

async fn record_one_council_vote(
    pool: &MySqlPool,
    config: &crate::config::Config,
    case: &EscalatedAppealCase,
    member: &CouncilMember,
) -> Result<bool, sqlx::Error> {
    let ctx_owned = build_review_context(pool, case).await?;
    let ctx = CouncilModerationReviewContext {
        community_name: &ctx_owned.community_name,
        target_content: &ctx_owned.target_content,
        constitution_context: &ctx_owned.constitution_context,
        jury_deliberation: &ctx_owned.jury_deliberation,
        scraped_evidence_summary: &ctx_owned.scraped_evidence_summary,
    };
    let system_user = audit::system_user_id(pool)
        .await?
        .ok_or_else(|| sqlx::Error::Protocol("system user not configured".into()))?;
    let vote_for_id = ref_key_values::lookup_active_id(
        pool,
        ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_VOTE_DIRECTION,
        ref_keys::KEY_JCJI_VOTE_FOR,
    )
    .await?
    .ok_or_else(|| sqlx::Error::Protocol("vote direction for missing".into()))?;
    let vote_against_id = ref_key_values::lookup_active_id(
        pool,
        ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_VOTE_DIRECTION,
        ref_keys::KEY_JCJI_VOTE_AGAINST,
    )
    .await?
    .ok_or_else(|| sqlx::Error::Protocol("vote direction against missing".into()))?;

    let agent = agent_specs::load_council_agent(&member.slug).map_err(sqlx::Error::Protocol)?;
    let prompt = compose_council_moderation_review_prompt(&agent, &ctx);

    let mut metadata = HashMap::new();
    metadata.insert("ai_actor_slug".to_string(), member.slug.clone());
    metadata.insert("text_prompt".to_string(), prompt);
    metadata.insert("judicial_case_id".to_string(), case.id.to_string());

    let vote_result = grpc::run_text_agent(
        &config.ai_grpc_endpoint,
        config.ai_grpc_timeout_ms,
        COUNCIL_MODERATION_REVIEW_AGENT,
        system_user,
        vec![],
        metadata,
    )
    .await;

    let (response_text, model_id) = match vote_result {
        Ok(pair) => pair,
        Err(e) => {
            tracing::warn!(
                case_id = case.id,
                council_slug = %member.slug,
                error = %e,
                "council moderation review grpc failed"
            );
            return Ok(false);
        }
    };

    let parsed = parse_moderation_review_response(&response_text);
    let direction = match parsed.vote {
        ModerationVote::Uphold => CouncilVoteDirection::Uphold,
        ModerationVote::Overturn => CouncilVoteDirection::Overturn,
    };
    let (vote_direction_id, decision_slug) = match direction {
        CouncilVoteDirection::Uphold => (vote_for_id, "council_uphold"),
        CouncilVoteDirection::Overturn => (vote_against_id, "council_overturn"),
    };

    let mut tx = pool.begin().await?;
    sqlx::query(
        r#"
        INSERT INTO judicial_case_council_vote (
            judicial_case_id,
            ai_actor_id,
            vote_direction_id,
            rationale,
            model_ref,
            created_by,
            modified_by
        )
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(case.id)
    .bind(member.ai_actor_id)
    .bind(vote_direction_id)
    .bind(response_text.as_str())
    .bind(model_id.as_str())
    .bind(system_user)
    .bind(system_user)
    .execute(&mut *tx)
    .await?;

    let (for_delta, against_delta) = match direction {
        CouncilVoteDirection::Uphold => (1i16, 0i16),
        CouncilVoteDirection::Overturn => (0i16, 1i16),
    };
    sqlx::query(
        r#"
        UPDATE judicial_case
        SET ai_votes_for = ai_votes_for + ?,
            ai_votes_against = ai_votes_against + ?,
            modified_by = ?
        WHERE id = ?
        "#,
    )
    .bind(for_delta)
    .bind(against_delta)
    .bind(system_user)
    .bind(case.id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO ai_judicial_decision_log (
            ai_actor_id,
            subject_ref,
            decision,
            rationale,
            model_ref
        )
        VALUES (?, ?, ?, ?, ?)
        "#,
    )
    .bind(member.ai_actor_id)
    .bind(format!("judicial_case:{}", case.id))
    .bind(decision_slug)
    .bind(response_text.as_str())
    .bind(model_id.as_str())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    tracing::info!(
        case_id = case.id,
        council_slug = %member.slug,
        decision = decision_slug,
        "council member vote recorded"
    );
    Ok(true)
}

async fn load_active_council_members(pool: &MySqlPool) -> Result<Vec<CouncilMember>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT aa.id AS ai_actor_id, aa.slug
        FROM ai_council_membership acm
        INNER JOIN ai_actor aa ON aa.id = acm.ai_actor_id AND aa.is_active = 1
        WHERE acm.is_active = 1
          AND (acm.ends_at IS NULL OR acm.ends_at >= UTC_DATE())
        ORDER BY aa.slug ASC
        "#,
    )
    .fetch_all(pool)
    .await
}

struct OwnedReviewContext {
    community_name: String,
    target_content: String,
    constitution_context: String,
    jury_deliberation: String,
    scraped_evidence_summary: String,
}

async fn build_review_context(
    pool: &MySqlPool,
    case: &EscalatedAppealCase,
) -> Result<OwnedReviewContext, sqlx::Error> {
    Ok(OwnedReviewContext {
        community_name: community_name_for_case(pool, case.id)
            .await?
            .unwrap_or_else(|| "the community".to_string()),
        target_content: post_content_for_case(pool, case.subject_post_id)
            .await?
            .unwrap_or_else(|| "(post content unavailable)".to_string()),
        constitution_context: constitution_text_for_case(pool, case.id).await?,
        jury_deliberation: human_jury_deliberation_text(
            pool,
            case.appealed_from_judicial_case_id,
            case.id,
        )
        .await?,
        scraped_evidence_summary: evidence_summary_for_case(pool, case.id).await?,
    })
}

async fn community_name_for_case(
    pool: &MySqlPool,
    case_id: u64,
) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(Option<u64>,)> = sqlx::query_as(
        r#"
        SELECT jc.subject_post_id
        FROM judicial_case jc
        WHERE jc.id = ?
        LIMIT 1
        "#,
    )
    .bind(case_id)
    .fetch_optional(pool)
    .await?;

    let Some((Some(post_id),)) = row else {
        return Ok(None);
    };

    sqlx::query_scalar(
        r#"
        SELECT COALESCE(NULLIF(TRIM(sc.label), ''), se.handle)
        FROM post p
        INNER JOIN post_audience_entity pae ON pae.post_id = p.id
        INNER JOIN social_entity se ON se.id = pae.audience_entity_id
        INNER JOIN social_community sc ON sc.social_entity_id = se.id
        WHERE p.id = ? OR p.root_post_id = ?
        ORDER BY pae.audience_entity_id ASC
        LIMIT 1
        "#,
    )
    .bind(post_id)
    .bind(post_id)
    .fetch_optional(pool)
    .await
}

async fn post_content_for_case(
    pool: &MySqlPool,
    post_id: Option<u64>,
) -> Result<Option<String>, sqlx::Error> {
    let Some(post_id) = post_id else {
        return Ok(None);
    };
    let row: Option<(String, String)> = sqlx::query_as(
        r#"
        SELECT COALESCE(NULLIF(TRIM(p.body_plain), ''), ''),
               COALESCE(NULLIF(TRIM(p.body_html_sanitized), ''), '')
        FROM post p
        WHERE p.id = ?
        LIMIT 1
        "#,
    )
    .bind(post_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(plain, html)| {
        if !plain.trim().is_empty() {
            plain
        } else {
            strip_tags_plain(&html)
        }
    }))
}

async fn constitution_text_for_case(pool: &MySqlPool, case_id: u64) -> Result<String, sqlx::Error> {
    let rows: Vec<(u16, String)> = sqlx::query_as(
        r#"
        SELECT cci.point, cci.item
        FROM judicial_case_constitution_item jcci
        INNER JOIN community_constitution_item cci ON cci.id = jcci.constitution_item_id
        WHERE jcci.judicial_case_id = ?
        ORDER BY cci.point ASC, cci.id ASC
        "#,
    )
    .bind(case_id)
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        return Ok("(no constitution clauses selected)".to_string());
    }
    Ok(rows
        .into_iter()
        .map(|(point, item)| format!("#{point}: {item}"))
        .collect::<Vec<_>>()
        .join("\n"))
}

async fn human_jury_deliberation_text(
    pool: &MySqlPool,
    prior_case_id: Option<u64>,
    appeal_case_id: u64,
) -> Result<String, sqlx::Error> {
    let mut parts = Vec::new();

    if let Some(prior_id) = prior_case_id {
        let tally: Option<(u8, u8, Option<String>)> = sqlx::query_as(
            r#"
            SELECT jc.jury_votes_for, jc.jury_votes_against, rk_status.`key`
            FROM judicial_case jc
            INNER JOIN ref_key_values rk_status ON rk_status.id = jc.status_id
            WHERE jc.id = ?
            LIMIT 1
            "#,
        )
        .bind(prior_id)
        .fetch_optional(pool)
        .await?;

        if let Some((for_v, against_v, status)) = tally {
            parts.push(format!(
                "Prior human case #{prior_id} status={} jury_votes_for={for_v} jury_votes_against={against_v}",
                status.unwrap_or_default()
            ));
        }

        let comments: Vec<(String, String, String)> = sqlx::query_as(
            r#"
            SELECT rk.`key`, COALESCE(jcc.body_plain, ''), COALESCE(jcc.body_html_sanitized, '')
            FROM judicial_case_comment jcc
            INNER JOIN ref_key_values rk ON rk.id = jcc.type_id
            WHERE jcc.judicial_case_id = ?
            ORDER BY jcc.id ASC
            "#,
        )
        .bind(prior_id)
        .fetch_all(pool)
        .await?;

        for (type_key, plain, html) in comments {
            let body = if !plain.trim().is_empty() {
                plain
            } else {
                strip_tags_plain(&html)
            };
            parts.push(format!("[prior {type_key}] {body}"));
        }
    }

    let appeal_comments: Vec<(String, String, String)> = sqlx::query_as(
        r#"
        SELECT rk.`key`, COALESCE(jcc.body_plain, ''), COALESCE(jcc.body_html_sanitized, '')
        FROM judicial_case_comment jcc
        INNER JOIN ref_key_values rk ON rk.id = jcc.type_id
        WHERE jcc.judicial_case_id = ?
        ORDER BY jcc.id ASC
        "#,
    )
    .bind(appeal_case_id)
    .fetch_all(pool)
    .await?;

    for (type_key, plain, html) in appeal_comments {
        let body = if !plain.trim().is_empty() {
            plain
        } else {
            strip_tags_plain(&html)
        };
        parts.push(format!("[appeal {type_key}] {body}"));
    }

    if parts.is_empty() {
        Ok("(no human jury deliberation on record)".to_string())
    } else {
        Ok(parts.join("\n\n"))
    }
}

async fn evidence_summary_for_case(pool: &MySqlPool, case_id: u64) -> Result<String, sqlx::Error> {
    let rows: Vec<(String, Option<String>)> = sqlx::query_as(
        r#"
        SELECT source_url, captured_text
        FROM judicial_case_evidence
        WHERE judicial_case_id = ?
        ORDER BY id ASC
        LIMIT 8
        "#,
    )
    .bind(case_id)
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        return Ok("None".to_string());
    }

    Ok(rows
        .into_iter()
        .map(|(url, text)| {
            let excerpt = text
                .as_deref()
                .unwrap_or("")
                .chars()
                .take(400)
                .collect::<String>();
            format!("URL: {url}\nExcerpt: {excerpt}")
        })
        .collect::<Vec<_>>()
        .join("\n---\n"))
}

fn parse_council_verdict(text: &str) -> CouncilVoteDirection {
    let upper = text.to_ascii_uppercase();
    if upper.contains("OVERTURN") {
        return CouncilVoteDirection::Overturn;
    }
    if upper.contains("UPHOLD") {
        return CouncilVoteDirection::Uphold;
    }
    if upper.contains("VIOLAT") || upper.contains("FLAG") {
        return CouncilVoteDirection::Uphold;
    }
    CouncilVoteDirection::Uphold
}

async fn try_resolve_council_case(pool: &MySqlPool, case_id: u64) -> Result<bool, sqlx::Error> {
    let members = load_active_council_members(pool).await?;
    let expected = members.len().min(COUNCIL_QUORUM);
    if expected == 0 {
        return Ok(false);
    }

    let vote_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM judicial_case_council_vote WHERE judicial_case_id = ?"#,
    )
    .bind(case_id)
    .fetch_one(pool)
    .await?;

    if vote_count < expected as i64 {
        return Ok(false);
    }

    let (for_votes, against_votes): (u8, u8) = sqlx::query_as(
        r#"
        SELECT jc.ai_votes_for, jc.ai_votes_against
        FROM judicial_case jc
        WHERE jc.id = ?
        "#,
    )
    .bind(case_id)
    .fetch_one(pool)
    .await?;

    let verdict = if for_votes > against_votes {
        VerdictOutcome::Upheld
    } else if against_votes > for_votes {
        VerdictOutcome::Overturned
    } else {
        tracing::info!(
            case_id,
            for_votes,
            against_votes,
            "council appeal tied; safety default upholds original violation assertion"
        );
        VerdictOutcome::Upheld
    };

    judicial::resolve_council_appeal_verdict(pool, case_id, verdict).await?;
    Ok(true)
}

fn strip_tags_plain(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}
