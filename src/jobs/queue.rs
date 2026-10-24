//! Job queue: enqueue, cancel, lease reap, fair batch claim, execute, archive, terminal purge.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use tokio::sync::{Mutex, Semaphore};
use tokio::task::JoinSet;

use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::Value;
use sqlx::types::Json;
use sqlx::{MySql, MySqlPool, Transaction};

use super::payload::payload_hash;
const OPS_JOB_TYPE_GROUP: &str = "ops_job_type";

static CONDUCTOR_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

fn conductor_mutex() -> &'static Mutex<()> {
    CONDUCTOR_MUTEX.get_or_init(|| Mutex::new(()))
}
/// Council AI ticks (reply/post member) can run many sequential LLM calls; keep well above
/// a single Vertex round-trip budget. Override via `JOB_QUEUE_LEASE_SECS` on the VM.
fn job_queue_lease_secs() -> i64 {
    std::env::var("JOB_QUEUE_LEASE_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&n| n >= 64)
        .unwrap_or(1800)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EnqueueResult {
    pub job_id: u64,
    /// True when an identical `(job_type_id, payload_hash)` row already existed.
    pub deduplicated: bool,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ConductorSummary {
    pub archive_rows_purged: u64,
    pub leases_reaped: u64,
    /// Rows removed after `attempt_count >= max_attempts` (reap or post-handler pass).
    pub terminal_purged: u64,
    /// Pending rows dropped because their period/date bucket is stale (safe no-op work).
    pub stale_purged: u64,
    pub claimed: u64,
    pub completed: u64,
    pub failed_retry: u64,
    /// `open`/`jury_active` cases with no pending juror invites examined for verdict closure.
    pub judicial_verdict_retry_candidates: u64,
    /// Cases moved to `resolved` by the post-conductor judicial verdict sweep.
    pub judicial_verdicts_resolved: u64,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct JobQueueSnapshot {
    pub pending: u64,
    pub pending_due: u64,
    pub leased: u64,
    /// Pending rows grouped by job type key (largest buckets first).
    pub pending_by_type: Vec<(String, u64)>,
    /// Most recent handler error among pending rows that have failed at least once.
    pub sample_last_error: Option<String>,
    /// Job type key for [`JobQueueSnapshot::sample_last_error`], when present.
    pub sample_last_error_type: Option<String>,
    /// Job type key of the single leased row, when exactly one is active.
    pub leased_job_type: Option<String>,
}

const COUNCIL_PIPELINE_JOB_TYPES: &[&str] = &[
    "council_share_shortlist",
    "council_share_read",
    "council_share_publish",
    "council_portfolio_feed_poll",
    "council_invite_article_pick",
    "council_invite_person_discover",
    "council_invite_contact_resolve",
    "council_invite_compose_send",
];

/// Lightweight queue depth for `/health/status` and ops probes.
pub async fn job_queue_snapshot(pool: &MySqlPool) -> Result<JobQueueSnapshot, sqlx::Error> {
    let pending: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM job_queue WHERE status = 'pending'",
    )
    .fetch_one(pool)
    .await?;
    let pending_due: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM job_queue
        WHERE status = 'pending' AND next_run_at <= UTC_TIMESTAMP(6)
        "#,
    )
    .fetch_one(pool)
    .await?;
    let leased: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM job_queue WHERE status = 'leased'",
    )
    .fetch_one(pool)
    .await?;

    let pending_by_type: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT rk.`key`, COUNT(*) AS n
        FROM job_queue jq
        INNER JOIN ref_key_values rk ON rk.id = jq.job_type_id
        WHERE jq.status = 'pending'
        GROUP BY rk.`key`
        ORDER BY n DESC
        LIMIT 12
        "#,
    )
    .fetch_all(pool)
    .await?;

    let sample_row: Option<(String, String)> = sqlx::query_as(
        r#"
        SELECT rk.`key`, LEFT(COALESCE(jq.last_error_message, ''), 200)
        FROM job_queue jq
        INNER JOIN ref_key_values rk ON rk.id = jq.job_type_id
        WHERE jq.status = 'pending'
          AND jq.attempt_count > 0
          AND jq.last_error_message IS NOT NULL
          AND TRIM(jq.last_error_message) <> ''
        ORDER BY jq.last_attempt_at DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await?;

    let leased_job_type: Option<String> = sqlx::query_scalar(
        r#"
        SELECT rk.`key`
        FROM job_queue jq
        INNER JOIN ref_key_values rk ON rk.id = jq.job_type_id
        WHERE jq.status = 'leased'
        GROUP BY rk.`key`
        HAVING COUNT(*) = 1
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await?;

    Ok(JobQueueSnapshot {
        pending: pending.max(0) as u64,
        pending_due: pending_due.max(0) as u64,
        leased: leased.max(0) as u64,
        pending_by_type: pending_by_type
            .into_iter()
            .map(|(k, n)| (k, n.max(0) as u64))
            .collect(),
        sample_last_error: sample_row.as_ref().map(|(_, msg)| msg.clone()),
        sample_last_error_type: sample_row.map(|(key, _)| key),
        leased_job_type,
    })
}

/// True when all council share/invite pipeline job types are registered in ref_key_values.
pub async fn council_pipeline_job_types_ready(pool: &MySqlPool) -> Result<bool, sqlx::Error> {
    for key in COUNCIL_PIPELINE_JOB_TYPES {
        if resolve_job_type_id(pool, key).await? == 0 {
            return Ok(false);
        }
    }
    Ok(true)
}

#[derive(Debug, sqlx::FromRow)]
struct EligibleRow {
    id: u64,
    job_type_id: u64,
}

#[derive(Debug, Clone)]
struct ClaimedJob {
    id: u64,
    user_id: Option<u64>,
    job_type_id: u64,
    type_key: String,
    payload: Value,
    payload_hash: String,
    correlation_id: Option<String>,
    attempt_count: u8,
}

pub async fn resolve_job_type_id(pool: &MySqlPool, job_type_key: &str) -> Result<u64, sqlx::Error> {
    let id: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT id FROM ref_key_values
        WHERE `group` = ? AND `key` = ? AND is_active = 1
        LIMIT 1
        "#,
    )
    .bind(OPS_JOB_TYPE_GROUP)
    .bind(job_type_key)
    .fetch_optional(pool)
    .await?;
    Ok(id.unwrap_or(0))
}

/// Insert a job due immediately, or return the existing row id on `(job_type_id, payload_hash)` conflict.
pub async fn enqueue_job(
    pool: &MySqlPool,
    user_id: Option<u64>,
    job_type_key: &str,
    payload: &Value,
    correlation_id: Option<&str>,
    priority: i16,
) -> Result<EnqueueResult, sqlx::Error> {
    enqueue_job_at(
        pool,
        user_id,
        job_type_key,
        payload,
        correlation_id,
        priority,
        Utc::now().naive_utc(),
    )
    .await
}

/// Insert a job scheduled for `next_run_at`, with the same dedupe semantics as [`enqueue_job`].
pub async fn enqueue_job_at(
    pool: &MySqlPool,
    user_id: Option<u64>,
    job_type_key: &str,
    payload: &Value,
    correlation_id: Option<&str>,
    priority: i16,
    next_run_at: NaiveDateTime,
) -> Result<EnqueueResult, sqlx::Error> {
    let job_type_id = resolve_job_type_id(pool, job_type_key).await?;
    if job_type_id == 0 {
        return Ok(EnqueueResult {
            job_id: 0,
            deduplicated: false,
        });
    }

    let hash = payload_hash(payload);
    let corr = correlation_id
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let insert = sqlx::query(
        r#"
        INSERT INTO job_queue (
            user_id, job_type_id, payload, payload_hash, correlation_id,
            status, priority, next_run_at, lease_until,
            attempt_count, max_attempts
        ) VALUES (?, ?, ?, ?, ?, 'pending', ?, ?, NULL, 0, 3)
        "#,
    )
    .bind(user_id)
    .bind(job_type_id)
    .bind(Json(payload))
    .bind(&hash)
    .bind(corr.as_deref())
    .bind(priority)
    .bind(next_run_at)
    .execute(pool)
    .await;

    match insert {
        Ok(r) => Ok(EnqueueResult {
            job_id: r.last_insert_id(),
            deduplicated: false,
        }),
        Err(e) => {
            if crate::social::db::is_unique_violation(&e) {
                let existing: u64 = sqlx::query_scalar(
                    "SELECT id FROM job_queue WHERE job_type_id = ? AND payload_hash = ? LIMIT 1",
                )
                .bind(job_type_id)
                .bind(&hash)
                .fetch_one(pool)
                .await?;
                return Ok(EnqueueResult {
                    job_id: existing,
                    deduplicated: true,
                });
            }
            Err(e)
        }
    }
}

/// User cancel: only `pending` jobs they own.
pub async fn cancel_job(pool: &MySqlPool, job_id: u64, user_id: u64) -> Result<bool, sqlx::Error> {
    let r = sqlx::query(
        r#"DELETE FROM job_queue WHERE id = ? AND user_id = ? AND status = 'pending'"#,
    )
    .bind(job_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(r.rows_affected() > 0)
}

fn round_robin_claim_ids(candidates: &[EligibleRow], batch_size: usize) -> Vec<u64> {
    if candidates.is_empty() || batch_size == 0 {
        return Vec::new();
    }
    let mut type_order: Vec<u64> = Vec::new();
    let mut buckets: HashMap<u64, VecDeque<u64>> = HashMap::new();
    for r in candidates {
        if !buckets.contains_key(&r.job_type_id) {
            type_order.push(r.job_type_id);
            buckets.insert(r.job_type_id, VecDeque::new());
        }
        buckets
            .get_mut(&r.job_type_id)
            .expect("bucket")
            .push_back(r.id);
    }

    let mut out: Vec<u64> = Vec::new();
    let mut chosen: HashSet<u64> = HashSet::new();

    loop {
        if out.len() >= batch_size {
            break;
        }
        let mut progressed = false;
        for tid in &type_order {
            if let Some(q) = buckets.get_mut(tid) {
                if let Some(id) = q.pop_front() {
                    out.push(id);
                    chosen.insert(id);
                    progressed = true;
                    if out.len() >= batch_size {
                        break;
                    }
                }
            }
        }
        if !progressed {
            break;
        }
    }

    for r in candidates {
        if out.len() >= batch_size {
            break;
        }
        if chosen.contains(&r.id) {
            continue;
        }
        out.push(r.id);
        chosen.insert(r.id);
    }

    out
}

async fn purge_old_archives(pool: &MySqlPool) -> Result<u64, sqlx::Error> {
    let r = sqlx::query(
        r#"DELETE FROM job_queue_archive WHERE archived_at < (UTC_TIMESTAMP(6) - INTERVAL 30 DAY)"#,
    )
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

async fn reap_expired_leases(pool: &MySqlPool) -> Result<u64, sqlx::Error> {
    let r = sqlx::query(
        r#"
        UPDATE job_queue
        SET
            attempt_count = attempt_count + 1,
            last_error_code = 'lease_expired',
            last_error_message = 'lease expired without completion',
            last_attempt_at = UTC_TIMESTAMP(6),
            status = 'pending',
            lease_until = NULL,
            next_run_at = UTC_TIMESTAMP(6)
        WHERE status = 'leased' AND lease_until IS NOT NULL AND lease_until < UTC_TIMESTAMP(6)
        "#,
    )
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

async fn purge_terminal_failures(pool: &MySqlPool) -> Result<u64, sqlx::Error> {
    let rows: Vec<(u64, Option<u64>, String, Option<String>, u8, String)> = sqlx::query_as(
        r#"
        SELECT jq.id, jq.user_id, rk.`key`, jq.correlation_id, jq.attempt_count, COALESCE(jq.last_error_message, '')
        FROM job_queue jq
        INNER JOIN ref_key_values rk ON rk.id = jq.job_type_id
        WHERE jq.status = 'pending'
          AND jq.lease_until IS NULL
          AND jq.attempt_count >= jq.max_attempts
        "#,
    )
    .fetch_all(pool)
    .await?;

    let n = rows.len() as u64;
    for (id, user_id, type_key, correlation_id, attempts, last_msg) in &rows {
        tracing::error!(
            job_id = id,
            user_id = user_id,
            job_type = %type_key,
            correlation_id = correlation_id.as_deref().unwrap_or(""),
            attempt_count = attempts,
            last_error = %last_msg,
            "job_queue terminal failure (purged)"
        );
    }

    if !rows.is_empty() {
        let ids: Vec<u64> = rows.iter().map(|r| r.0).collect();
        let mut qb = sqlx::QueryBuilder::new("DELETE FROM job_queue WHERE id IN (");
        let mut sep = qb.separated(", ");
        for id in ids {
            sep.push_bind(id);
        }
        qb.push(")");
        qb.build().execute(pool).await?;
    }

    Ok(n)
}

/// Drop pending jobs whose scheduling bucket is obsolete (daily `utc_date` or 15-min `period_key`).
/// Handler no-ops remain as a safety net; purging avoids claiming thousands of stale rows.
async fn purge_stale_period_jobs(pool: &MySqlPool) -> Result<u64, sqlx::Error> {
    let today = crate::social::council_jobs::council_daily_period_key();
    let reply_period = crate::social::council_jobs::council_reply_period_key();

    const DAILY_JOB_TYPES: &[&str] = &[
        "council_post_member",
        "council_share_shortlist",
        "council_share_read",
        "council_share_publish",
        "council_invite_article_pick",
        "council_invite_person_discover",
        "council_invite_contact_resolve",
        "council_invite_compose_send",
    ];

    let mut total = 0u64;
    for job_type in DAILY_JOB_TYPES {
        let r = sqlx::query(
            r#"
            DELETE jq FROM job_queue jq
            INNER JOIN ref_key_values rk
                ON rk.id = jq.job_type_id
               AND rk.`group` = ?
               AND rk.is_active = 1
               AND rk.`key` = ?
            WHERE jq.status = 'pending'
              AND JSON_UNQUOTE(JSON_EXTRACT(jq.payload, '$.utc_date')) < ?
            "#,
        )
        .bind(OPS_JOB_TYPE_GROUP)
        .bind(*job_type)
        .bind(&today)
        .execute(pool)
        .await?;
        total += r.rows_affected();
    }

    let r = sqlx::query(
        r#"
        DELETE jq FROM job_queue jq
        INNER JOIN ref_key_values rk
            ON rk.id = jq.job_type_id
           AND rk.`group` = ?
           AND rk.is_active = 1
           AND rk.`key` = 'council_reply_tick'
        WHERE jq.status = 'pending'
          AND JSON_UNQUOTE(JSON_EXTRACT(jq.payload, '$.period_key')) < ?
        "#,
    )
    .bind(OPS_JOB_TYPE_GROUP)
    .bind(&reply_period)
    .execute(pool)
    .await?;
    total += r.rows_affected();

    for job_type in ["council_reply_member"] {
        let r = sqlx::query(
            r#"
            DELETE jq FROM job_queue jq
            INNER JOIN ref_key_values rk
                ON rk.id = jq.job_type_id
               AND rk.`group` = ?
               AND rk.is_active = 1
               AND rk.`key` = ?
            WHERE jq.status = 'pending'
              AND JSON_UNQUOTE(JSON_EXTRACT(jq.payload, '$.period_key')) < ?
            "#,
        )
        .bind(OPS_JOB_TYPE_GROUP)
        .bind(job_type)
        .bind(&reply_period)
        .execute(pool)
        .await?;
        total += r.rows_affected();
    }

    const DAILY_COORDINATOR_TYPES: &[&str] = &[
        "council_post_tick",
        "council_invite_tick",
        "panoply_daily_status",
        "user_engagement_digest",
        "system_journal_retention",
    ];
    for job_type in DAILY_COORDINATOR_TYPES {
        let r = sqlx::query(
            r#"
            DELETE jq FROM job_queue jq
            INNER JOIN ref_key_values rk
                ON rk.id = jq.job_type_id
               AND rk.`group` = ?
               AND rk.is_active = 1
               AND rk.`key` = ?
            WHERE jq.status = 'pending'
              AND JSON_UNQUOTE(JSON_EXTRACT(jq.payload, '$.utc_date')) < ?
            "#,
        )
        .bind(OPS_JOB_TYPE_GROUP)
        .bind(*job_type)
        .bind(&today)
        .execute(pool)
        .await?;
        total += r.rows_affected();
    }

    if total > 0 {
        tracing::info!(
            stale_purged = total,
            daily_cutoff = %today,
            reply_cutoff = %reply_period,
            "job_queue stale period purge"
        );
    }

    Ok(total)
}

async fn try_claim_job(pool: &MySqlPool, job_id: u64, lease_secs: i64) -> Result<bool, sqlx::Error> {
    let lease_until =
        Utc::now() + Duration::from_secs(lease_secs.try_into().unwrap_or(1800));
    let r = sqlx::query(
        r#"
        UPDATE job_queue
        SET status = 'leased', lease_until = ?, updated_at = UTC_TIMESTAMP(6)
        WHERE id = ?
          AND status = 'pending'
          AND next_run_at <= UTC_TIMESTAMP(6)
        "#,
    )
    .bind(lease_until.naive_utc())
    .bind(job_id)
    .execute(pool)
    .await?;
    Ok(r.rows_affected() > 0)
}

async fn release_orphan_lease(pool: &MySqlPool, job_id: u64) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE job_queue
        SET status = 'pending', lease_until = NULL, updated_at = UTC_TIMESTAMP(6)
        WHERE id = ? AND status = 'leased'
        "#,
    )
    .bind(job_id)
    .execute(pool)
    .await?;
    Ok(())
}

fn handler_timeout_for_lease(lease_secs: i64) -> Duration {
    Duration::from_secs(lease_secs.saturating_sub(30).clamp(120, 3600) as u64)
}

fn handler_timeout_for_job(
    type_key: &str,
    lease_secs: i64,
    config: &crate::config::Config,
) -> Duration {
    match type_key {
        "panoply_daily_status"
        | "user_engagement_digest"
        | "council_post_tick"
        | "council_invite_tick"
        | "content_domain_lifecycle"
        | "face_capture_pending_cleanup"
        | "system_journal_retention"
        | "scope_recalc_drain"
        | "community_graph_lifecycle"
        | "judicial_jury_nomination" => Duration::from_secs(300),
        "council_portfolio_feed_poll" => Duration::from_secs(
            config
                .feed_poll_wall_budget_secs
                .saturating_add(30)
                .min(300),
        ),
        "council_reply_tick" => Duration::from_secs(120),
        "judicial_council_appeal" => Duration::from_secs(120),
        "council_reply_member"
        | "judicial_council_vote_member"
        | "council_share_shortlist"
        | "council_share_read"
        | "council_share_publish"
        | "council_invite_article_pick"
        | "council_invite_person_discover"
        | "council_invite_contact_resolve"
        | "council_invite_compose_send"
        | "council_post_member" => Duration::from_secs(600),
        "face_portrait_pipeline" => Duration::from_secs(900),
        _ => handler_timeout_for_lease(lease_secs),
    }
}

/// Gateway-heavy handlers share a tighter sub-cap within [`Config::job_conductor_parallelism`].
fn job_type_uses_llm_gateway(type_key: &str) -> bool {
    matches!(
        type_key,
        "council_reply_member"
            | "judicial_council_vote_member"
            | "council_share_shortlist"
            | "council_share_read"
            | "council_share_publish"
            | "council_invite_article_pick"
            | "council_invite_person_discover"
            | "council_invite_contact_resolve"
            | "council_invite_compose_send"
            | "council_post_member"
            | "judicial_council_appeal"
            | "face_portrait_pipeline"
    )
}

#[derive(Debug)]
struct JobRunOutcome {
    completed: bool,
    failed_retry: bool,
    job_id: u64,
    type_key: String,
    correlation_id: String,
}

async fn execute_claimed_job_with_timeout(
    pool: &MySqlPool,
    config: &crate::config::Config,
    job: &ClaimedJob,
    lease_secs: i64,
) -> JobRunOutcome {
    let cid = job.correlation_id.as_deref().unwrap_or("").to_string();
    let job_id = job.id;
    let type_key = job.type_key.clone();
    let handler_timeout = handler_timeout_for_job(&type_key, lease_secs, config);
    let pool_dispatch = pool.clone();
    let config_dispatch = config.clone();
    let job_dispatch = job.clone();
    let mut dispatch_task = tokio::spawn(async move {
        dispatch_job(&pool_dispatch, &config_dispatch, &job_dispatch).await
    });
    let res = match tokio::time::timeout(handler_timeout, &mut dispatch_task).await {
        Ok(Ok(result)) => result,
        Ok(Err(join_err)) => Err(format!("dispatch task join failed: {join_err}")),
        Err(_) => {
            dispatch_task.abort();
            let _ = mark_job_failed(
                pool,
                job_id,
                "handler_timeout",
                &format!(
                    "handler exceeded {}s wall clock",
                    handler_timeout.as_secs()
                ),
            )
            .await;
            tracing::warn!(
                job_id,
                job_type = %type_key,
                correlation_id = %cid,
                timeout_secs = handler_timeout.as_secs(),
                "job_queue handler timed out"
            );
            return JobRunOutcome {
                completed: false,
                failed_retry: true,
                job_id,
                type_key,
                correlation_id: cid,
            };
        }
    };

    match res {
        Ok(()) => {
            let mut tx = match pool.begin().await {
                Ok(tx) => tx,
                Err(e) => {
                    tracing::warn!(
                        job_id,
                        job_type = %type_key,
                        correlation_id = %cid,
                        error = %e,
                        "job_queue archive begin failed"
                    );
                    let _ = mark_job_failed(pool, job_id, "archive_failed", "archive begin failed").await;
                    return JobRunOutcome {
                        completed: false,
                        failed_retry: true,
                        job_id,
                        type_key,
                        correlation_id: cid,
                    };
                }
            };
            if archive_success(&mut tx, job).await.is_ok() {
                if tx.commit().await.is_ok() {
                    tracing::warn!(
                        job_id = job.id,
                        job_type = %job.type_key,
                        correlation_id = %cid,
                        "job_queue completed"
                    );
                    JobRunOutcome {
                        completed: true,
                        failed_retry: false,
                        job_id,
                        type_key,
                        correlation_id: cid,
                    }
                } else {
                    let _ = mark_job_failed(pool, job.id, "archive_failed", "archive commit failed").await;
                    JobRunOutcome {
                        completed: false,
                        failed_retry: true,
                        job_id,
                        type_key,
                        correlation_id: cid,
                    }
                }
            } else {
                let _ = tx.rollback().await;
                let _ = mark_job_failed(pool, job.id, "archive_failed", "archive insert/delete failed").await;
                JobRunOutcome {
                    completed: false,
                    failed_retry: true,
                    job_id,
                    type_key,
                    correlation_id: cid,
                }
            }
        }
        Err(msg) => {
            let _ = mark_job_failed(pool, job.id, "handler_error", &msg).await;
            tracing::warn!(
                job_id = job.id,
                job_type = %job.type_key,
                correlation_id = %cid,
                error = %msg,
                "job_queue handler failed"
            );
            JobRunOutcome {
                completed: false,
                failed_retry: true,
                job_id,
                type_key,
                correlation_id: cid,
            }
        }
    }
}

fn apply_job_run_outcome(summary: &mut ConductorSummary, outcome: JobRunOutcome) {
    if outcome.completed {
        summary.completed += 1;
    }
    if outcome.failed_retry {
        summary.failed_retry += 1;
    }
}

async fn fetch_eligible_job_rows(pool: &MySqlPool) -> Result<Vec<EligibleRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT jq.id, jq.job_type_id
        FROM job_queue jq
        INNER JOIN ref_key_values rk ON rk.id = jq.job_type_id
            AND rk.`group` = ?
            AND rk.is_active = 1
        WHERE jq.status = 'pending'
          AND jq.next_run_at <= UTC_TIMESTAMP(6)
        ORDER BY jq.priority ASC, jq.next_run_at ASC, jq.id ASC
        LIMIT 500
        "#,
    )
    .bind(OPS_JOB_TYPE_GROUP)
    .fetch_all(pool)
    .await
}

/// Reset lease clock when execution starts (and for long-running council handlers).
pub async fn extend_job_lease(pool: &MySqlPool, job_id: u64) -> Result<(), sqlx::Error> {
    let lease_secs = job_queue_lease_secs();
    let lease_until =
        Utc::now() + Duration::from_secs(lease_secs.try_into().unwrap_or(1800));
    sqlx::query(
        r#"
        UPDATE job_queue
        SET lease_until = ?, updated_at = UTC_TIMESTAMP(6)
        WHERE id = ? AND status = 'leased'
        "#,
    )
    .bind(lease_until.naive_utc())
    .bind(job_id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn fetch_claimed_job(pool: &MySqlPool, id: u64) -> Result<Option<ClaimedJob>, sqlx::Error> {
    let row: Option<(u64, Option<u64>, u64, String, Value, String, Option<String>, u8)> =
        sqlx::query_as(
            r#"
            SELECT jq.id, jq.user_id, jq.job_type_id, rk.`key` AS type_key, jq.payload,
                   jq.payload_hash, jq.correlation_id, jq.attempt_count
            FROM job_queue jq
            INNER JOIN ref_key_values rk ON rk.id = jq.job_type_id AND rk.`group` = ?
            WHERE jq.id = ? AND jq.status = 'leased'
            "#,
        )
        .bind(OPS_JOB_TYPE_GROUP)
        .bind(id)
        .fetch_optional(pool)
        .await?;

    Ok(row.map(
        |(id, user_id, job_type_id, type_key, payload, payload_hash, correlation_id, attempt_count)| {
            ClaimedJob {
                id,
                user_id,
                job_type_id,
                type_key,
                payload,
                payload_hash,
                correlation_id,
                attempt_count,
            }
        },
    ))
}

async fn archive_success(
    tx: &mut Transaction<'_, MySql>,
    job: &ClaimedJob,
) -> Result<(), sqlx::Error> {
    let completed_at: DateTime<Utc> = Utc::now();
    sqlx::query(
        r#"
        INSERT INTO job_queue_archive (
            source_job_id, user_id, job_type_id, payload, payload_hash, correlation_id,
            attempt_count, completed_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(job.id)
    .bind(job.user_id)
    .bind(job.job_type_id)
    .bind(Json(&job.payload))
    .bind(&job.payload_hash)
    .bind(job.correlation_id.as_deref())
    .bind(job.attempt_count)
    .bind(completed_at.naive_utc())
    .execute(&mut **tx)
    .await?;

    sqlx::query(r#"DELETE FROM job_queue WHERE id = ? AND status = 'leased'"#)
        .bind(job.id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

fn backoff_after_failure(attempt_after_increment: u8) -> Duration {
    let exp = u32::from(attempt_after_increment).min(6);
    Duration::from_secs(2u64.pow(exp)).min(Duration::from_secs(60))
}

async fn mark_job_failed(
    pool: &MySqlPool,
    job_id: u64,
    error_code: &str,
    error_message: &str,
) -> Result<(), sqlx::Error> {
    let r = sqlx::query(
        r#"
        UPDATE job_queue
        SET
            attempt_count = attempt_count + 1,
            last_error_code = ?,
            last_error_message = LEFT(?, 500),
            last_attempt_at = UTC_TIMESTAMP(6),
            status = 'pending',
            lease_until = NULL
        WHERE id = ? AND status = 'leased'
        "#,
    )
    .bind(error_code)
    .bind(error_message)
    .bind(job_id)
    .execute(pool)
    .await?;

    if r.rows_affected() == 0 {
        return Ok(());
    }

    let counts: Option<(u8, u8)> = sqlx::query_as(
        "SELECT attempt_count, max_attempts FROM job_queue WHERE id = ?",
    )
    .bind(job_id)
    .fetch_optional(pool)
    .await?;

    let Some((attempt_count, max_attempts)) = counts else {
        return Ok(());
    };

    if attempt_count >= max_attempts {
        let meta: Option<(Option<u64>, String, Option<String>, String)> = sqlx::query_as(
            r#"
            SELECT jq.user_id, rk.`key`, jq.correlation_id, COALESCE(jq.last_error_message, '')
            FROM job_queue jq
            INNER JOIN ref_key_values rk ON rk.id = jq.job_type_id
            WHERE jq.id = ?
            "#,
        )
        .bind(job_id)
        .fetch_optional(pool)
        .await?;

        if let Some((user_id, type_key, correlation_id, last_msg)) = meta {
            tracing::error!(
                job_id,
                user_id = user_id,
                job_type = %type_key,
                correlation_id = correlation_id.as_deref().unwrap_or(""),
                attempt_count,
                last_error = %last_msg,
                "job_queue terminal failure after handler error (purged)"
            );
        }
        sqlx::query("DELETE FROM job_queue WHERE id = ?")
            .bind(job_id)
            .execute(pool)
            .await?;
        return Ok(());
    }

    let wait = backoff_after_failure(attempt_count);
    let next = Utc::now() + wait;
    sqlx::query(
        r#"UPDATE job_queue SET next_run_at = ? WHERE id = ? AND status = 'pending'"#,
    )
    .bind(next.naive_utc())
    .bind(job_id)
    .execute(pool)
    .await?;

    Ok(())
}

async fn dispatch_job(
    pool: &MySqlPool,
    config: &crate::config::Config,
    job: &ClaimedJob,
) -> Result<(), String> {
    match job.type_key.as_str() {
        "noop" => Ok(()),
        "judicial_jury_nomination" => {
            run_judicial_jury_nomination_job(pool, config, &job.payload).await
        }
        "judicial_council_appeal" => run_judicial_council_appeal_job(pool, &job.payload).await,
        "judicial_council_vote_member" => {
            run_judicial_council_vote_member_job(pool, config, &job.payload, job.id).await
        }
        "scope_recalc_drain" => run_scope_recalc_drain_job(pool, &job.payload).await,
        "scope_rematerialize_all" => run_scope_rematerialize_all_job(pool).await,
        "community_graph_lifecycle" => {
            run_community_graph_lifecycle_job(pool, config, &job.payload).await
        }
        "conversation_rating_settlement" => {
            run_conversation_rating_settlement_job(pool, &job.payload).await
        }
        "emergency_standing_settlement" => {
            run_emergency_standing_settlement_job(pool, config, &job.payload).await
        }
        "godmother_outreach" => run_godmother_outreach_job(pool, config, &job.payload).await,
        "godmother_welcome" => run_godmother_welcome_job(pool, config, &job.payload).await,
        "council_reply_tick" => {
            run_council_reply_tick_job(pool, config, &job.payload).await
        }
        "council_reply_member" => {
            run_council_reply_member_job(pool, config, &job.payload, job.id).await
        }
        "council_post_tick" => run_council_post_tick_job(pool, config).await,
        "council_post_member" => {
            run_council_post_member_job(pool, config, &job.payload, job.id).await
        }
        "council_share_shortlist" => {
            run_council_share_shortlist_job(pool, config, &job.payload, job.id).await
        }
        "council_share_read" => {
            run_council_share_read_job(pool, config, &job.payload, job.id).await
        }
        "council_share_publish" => {
            run_council_share_publish_job(pool, config, &job.payload, job.id).await
        }
        "council_portfolio_feed_poll" => run_council_portfolio_feed_poll_job(pool, config, &job.payload).await,
        "council_invite_tick" => run_council_invite_tick_job(pool, config).await,
        "council_invite_article_pick" => {
            run_council_invite_article_pick_job(pool, config, &job.payload).await
        }
        "council_invite_person_discover" => {
            run_council_invite_person_discover_job(pool, config, &job.payload).await
        }
        "council_invite_contact_resolve" => {
            run_council_invite_contact_resolve_job(pool, config, &job.payload).await
        }
        "council_invite_compose_send" => {
            run_council_invite_compose_send_job(pool, config, &job.payload).await
        }
        "content_domain_lifecycle" => run_content_domain_lifecycle_job(pool, &job.payload).await,
        "user_engagement_digest" => {
            run_user_engagement_digest_job(pool, config, &job.payload).await
        }
        "panoply_daily_status" => run_panoply_daily_status_job(pool, config, &job.payload).await,
        "face_portrait_pipeline" => {
            run_face_portrait_pipeline_job(pool, config, &job.payload).await
        }
        "face_capture_pending_cleanup" => run_face_capture_pending_cleanup_job(pool, config).await,
        "system_journal_retention" => {
            run_system_journal_retention_job(pool, config, &job.payload).await
        }
        other => Err(format!("unknown job type: {other}")),
    }
}

fn payload_u64(payload: &Value, key: &str) -> Option<u64> {
    payload.get(key).and_then(|v| v.as_u64())
}

async fn run_scope_recalc_drain_job(pool: &MySqlPool, payload: &Value) -> Result<(), String> {
    let limit = payload_u64(payload, "limit").unwrap_or(64).clamp(1, 256) as usize;
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    let subjects: Vec<u64> = sqlx::query_scalar(
        r#"
        SELECT subject_entity_id
        FROM entity_scope_recalc_queue
        ORDER BY requested_at ASC
        LIMIT ?
        FOR UPDATE SKIP LOCKED
        "#,
    )
    .bind(limit as u64)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    for subject_entity_id in subjects {
        sqlx::query("DELETE FROM entity_read_scope WHERE subject_entity_id = ?")
            .bind(subject_entity_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;

        sqlx::query(
            r#"
            INSERT INTO entity_read_scope (
                subject_entity_id,
                visible_entity_id,
                granted_at,
                source
            )
            SELECT ?, v.visible_entity_id, UTC_TIMESTAMP(6), 'materialized'
            FROM vw_entity_read_scope_live v
            WHERE v.subject_entity_id = ?
            "#,
        )
        .bind(subject_entity_id)
        .bind(subject_entity_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

        sqlx::query("DELETE FROM entity_scope_recalc_queue WHERE subject_entity_id = ?")
            .bind(subject_entity_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

async fn run_judicial_council_appeal_job(
    pool: &MySqlPool,
    payload: &Value,
) -> Result<(), String> {
    let case_limit = payload_u64(payload, "case_limit").unwrap_or(4).clamp(1, 32);
    let focus_case_id = payload_u64(payload, "judicial_case_id");
    let summary = crate::social::judicial_council::enqueue_escalated_appeal_vote_jobs(
        pool,
        case_limit,
        focus_case_id,
    )
    .await
    .map_err(|e| e.to_string())?;
    tracing::info!(
        cases_examined = summary.cases_examined,
        vote_jobs_enqueued = summary.vote_jobs_enqueued,
        vote_jobs_deduplicated = summary.vote_jobs_deduplicated,
        focus_case_id = ?focus_case_id,
        "judicial council appeal coordinator completed"
    );
    Ok(())
}

async fn run_judicial_council_vote_member_job(
    pool: &MySqlPool,
    config: &crate::config::Config,
    payload: &Value,
    job_id: u64,
) -> Result<(), String> {
    let case_id = payload_u64(payload, "judicial_case_id")
        .ok_or_else(|| "judicial_case_id required".to_string())?;
    let slug = payload_string(payload, "ai_actor_slug")?;
    let _ = extend_job_lease(pool, job_id).await;
    let outcome = crate::social::judicial_council::record_council_member_vote(
        pool,
        config,
        case_id,
        &slug,
    )
    .await
    .map_err(|e| e.to_string())?;
    tracing::info!(
        case_id,
        council_slug = %slug,
        outcome = ?outcome,
        "judicial_council_vote_member completed"
    );
    Ok(())
}

async fn run_conversation_rating_settlement_job(
    pool: &MySqlPool,
    payload: &Value,
) -> Result<(), String> {
    let limit = payload_u64(payload, "limit").unwrap_or(32).clamp(1, 256) as u32;
    let applied = crate::social::conversation_rating::run_settlement_batch(pool, limit)
        .await
        .map_err(|e| e.to_string())?;
    if applied > 0 {
        tracing::info!(applied, "conversation_rating_settlement applied");
    }
    Ok(())
}

async fn run_emergency_standing_settlement_job(
    pool: &MySqlPool,
    config: &crate::config::Config,
    payload: &Value,
) -> Result<(), String> {
    let limit = payload_u64(payload, "limit").unwrap_or(32).clamp(1, 256) as u32;
    let notified = crate::social::judicial::run_standing_settlement_batch(pool, limit)
        .await
        .map_err(|e| e.to_string())?;
    let applied_count = notified.len();
    for (author_user_id, root_post_id) in notified {
        crate::email::notify_emergency_standing_applied(
            pool,
            config,
            author_user_id,
            root_post_id,
        )
        .await;
    }
    if applied_count > 0 {
        tracing::info!(count = applied_count, "emergency_standing_settlement applied");
    }
    Ok(())
}

async fn run_community_graph_lifecycle_job(
    pool: &MySqlPool,
    config: &crate::config::Config,
    payload: &Value,
) -> Result<(), String> {
    let inactivity_batch = payload_u64(payload, "human_inactivity_batch")
        .unwrap_or(16)
        .clamp(1, 64);
    let human_penalties = crate::social::community_graph::run_human_inactivity_penalty_batch(
        pool,
        config,
        inactivity_batch,
    )
    .await
    .map_err(|e| e.to_string())?;
    if human_penalties > 0 {
        tracing::info!(
            human_penalties,
            "community_graph_lifecycle human login-inactivity penalties applied"
        );
    }

    crate::social::rating_notify::flush_rating_failing_notifications(pool, config, 32).await;

    crate::social::godmother::flush_recovery_candidates(pool, config, 32).await;

    let batch = payload_u64(payload, "death_batch").unwrap_or(8).clamp(1, 64);
    let processed = crate::social::community_graph::run_death_batch(pool, batch)
        .await
        .map_err(|e| e.to_string())?;
    if processed > 0 {
        tracing::info!(processed, "community_graph_lifecycle death batch completed");
    }
    Ok(())
}

async fn run_judicial_jury_nomination_job(
    pool: &MySqlPool,
    config: &crate::config::Config,
    payload: &Value,
) -> Result<(), String> {
    let limit = payload_u64(payload, "limit").unwrap_or(32).clamp(1, 256);
    let summary = crate::social::judicial::nominate_jurors_for_open_cases(pool, limit)
        .await
        .map_err(|e| e.to_string())?;
    crate::email::notify_juror_invites_created(pool, config, &summary.new_invites).await;
    tracing::info!(
        cases_examined = summary.cases_examined,
        cases_filled = summary.cases_filled,
        invites_created = summary.invites_created,
        cases_closed_no_jury = summary.cases_closed_no_jury,
        juror_emails = summary.new_invites.len(),
        "judicial jury nomination completed"
    );
    Ok(())
}

async fn run_godmother_outreach_job(
    pool: &MySqlPool,
    config: &crate::config::Config,
    payload: &Value,
) -> Result<(), String> {
    let human_entity_id = payload_u64(payload, "human_entity_id")
        .ok_or_else(|| "godmother_outreach: human_entity_id required".to_string())?;
    let moment_raw = payload
        .get("moment")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "godmother_outreach: moment required".to_string())?;
    let moment = crate::social::godmother::GodmotherMoment::from_key(moment_raw)
        .ok_or_else(|| format!("godmother_outreach: unknown moment {moment_raw}"))?;
    let summary = crate::social::godmother::run_outreach(pool, config, human_entity_id, moment).await?;
    tracing::info!(
        delivered = summary.delivered,
        skipped = summary.skipped,
        llm_generated = summary.llm_generated,
        human_entity_id,
        moment = moment.key(),
        "godmother_outreach completed"
    );
    Ok(())
}

async fn run_godmother_welcome_job(
    pool: &MySqlPool,
    config: &crate::config::Config,
    payload: &Value,
) -> Result<(), String> {
    let human_entity_id = payload_u64(payload, "human_entity_id")
        .ok_or_else(|| "godmother_welcome: human_entity_id required".to_string())?;
    let summary = crate::social::godmother::run_outreach(
        pool,
        config,
        human_entity_id,
        crate::social::godmother::GodmotherMoment::Welcome,
    )
    .await?;
    tracing::info!(
        delivered = summary.delivered,
        skipped = summary.skipped,
        llm_generated = summary.llm_generated,
        human_entity_id,
        "godmother_welcome completed (legacy payload)"
    );
    Ok(())
}

async fn run_council_reply_tick_job(
    pool: &MySqlPool,
    config: &crate::config::Config,
    payload: &Value,
) -> Result<(), String> {
    if let Some(period) = payload.get("period_key").and_then(|v| v.as_str()) {
        if crate::social::council_jobs::is_stale_reply_period(period) {
            tracing::info!(
                period_key = period,
                "council_reply_tick skipped — stale period"
            );
            return Ok(());
        }
    }
    let summary = crate::social::council_jobs::run_reply_tick(pool, config).await?;
    if summary.skipped_period_claim {
        return Ok(());
    }
    if summary.members_examined == 0 {
        return Ok(());
    }
    tracing::info!(
        members = summary.members_examined,
        enqueued = summary.actions_taken,
        deduped = summary.declines,
        errors = summary.errors,
        "council_reply_tick coordinator completed"
    );
    Ok(())
}

async fn run_council_reply_member_job(
    pool: &MySqlPool,
    config: &crate::config::Config,
    payload: &Value,
    job_id: u64,
) -> Result<(), String> {
    let slug = payload_string(payload, "ai_actor_slug")?;
    let period_key = payload
        .get("period_key")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if let Some(period) = payload.get("period_key").and_then(|v| v.as_str()) {
        if crate::social::council_jobs::is_stale_reply_period(period) {
            tracing::info!(
                slug = %slug,
                period_key = period,
                "council_reply_member skipped — stale period"
            );
            return Ok(());
        }
    }
    let _ = extend_job_lease(pool, job_id).await;
    let summary = crate::social::council_jobs::run_reply_member_tick(
        pool,
        config,
        &slug,
        period_key,
        Some(job_id),
    )
    .await?;
    if summary.skipped_period_claim {
        tracing::info!(
            slug = %slug,
            period_key = %period_key,
            "council_reply_member skipped — duplicate queue row"
        );
    } else {
        tracing::info!(
            slug = %slug,
            published = summary.actions_taken,
            "council_reply_member completed"
        );
    }
    Ok(())
}

async fn run_council_post_tick_job(
    pool: &MySqlPool,
    config: &crate::config::Config,
) -> Result<(), String> {
    let summary = crate::social::council_jobs::run_post_tick(pool, config).await?;
    if summary.skipped_period_claim {
        tracing::info!(
            members = summary.members_examined,
            "council_post_tick coordinator skipped — fan-out already claimed for utc day"
        );
        return Ok(());
    }
    if summary.members_examined == 0 {
        return Ok(());
    }
    tracing::info!(
        members = summary.members_examined,
        enqueued = summary.actions_taken,
        deduped = summary.declines,
        errors = summary.errors,
        "council_post_tick coordinator completed"
    );
    Ok(())
}

fn payload_string(payload: &Value, key: &str) -> Result<String, String> {
    payload
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("missing payload.{key}"))
}

async fn run_council_post_member_job(
    pool: &MySqlPool,
    config: &crate::config::Config,
    payload: &Value,
    job_id: u64,
) -> Result<(), String> {
    let slug = payload_string(payload, "ai_actor_slug")?;
    if let Some(utc_date) = payload.get("utc_date").and_then(|v| v.as_str()) {
        let today = crate::social::council_jobs::council_daily_period_key();
        if utc_date.trim() < today.as_str() {
            tracing::info!(
                slug = %slug,
                utc_date,
                "council_post_member skipped — stale daily period"
            );
            return Ok(());
        }
    }
    let _ = extend_job_lease(pool, job_id).await;
    crate::social::council_jobs::run_post_member_tick(pool, config, &slug).await?;
    tracing::info!(slug = %slug, "council_post_member completed");
    Ok(())
}

async fn run_council_share_shortlist_job(
    pool: &MySqlPool,
    config: &crate::config::Config,
    payload: &Value,
    job_id: u64,
) -> Result<(), String> {
    if let Some(utc_date) = payload.get("utc_date").and_then(|v| v.as_str()) {
        if crate::social::council_jobs::is_stale_daily_period(utc_date) {
            tracing::info!(utc_date, "council_share_shortlist skipped — stale daily period");
            return Ok(());
        }
    }
    let _ = extend_job_lease(pool, job_id).await;
    crate::social::council_share_pipeline::run_share_shortlist_tick(pool, config, payload).await?;
    tracing::info!("council_share_shortlist completed");
    Ok(())
}

async fn run_council_share_read_job(
    pool: &MySqlPool,
    config: &crate::config::Config,
    payload: &Value,
    job_id: u64,
) -> Result<(), String> {
    if let Some(utc_date) = payload.get("utc_date").and_then(|v| v.as_str()) {
        if crate::social::council_jobs::is_stale_daily_period(utc_date) {
            tracing::info!(utc_date, "council_share_read skipped — stale daily period");
            return Ok(());
        }
    }
    let _ = extend_job_lease(pool, job_id).await;
    crate::social::council_share_pipeline::run_share_read_tick(pool, config, payload).await?;
    tracing::info!("council_share_read completed");
    Ok(())
}

async fn run_council_share_publish_job(
    pool: &MySqlPool,
    config: &crate::config::Config,
    payload: &Value,
    job_id: u64,
) -> Result<(), String> {
    if let Some(utc_date) = payload.get("utc_date").and_then(|v| v.as_str()) {
        if crate::social::council_jobs::is_stale_daily_period(utc_date) {
            tracing::info!(utc_date, "council_share_publish skipped — stale daily period");
            return Ok(());
        }
    }
    let _ = extend_job_lease(pool, job_id).await;
    crate::social::council_share_pipeline::run_share_publish_tick(pool, config, payload).await?;
    tracing::info!("council_share_publish completed");
    Ok(())
}

async fn run_council_portfolio_feed_poll_job(
    pool: &MySqlPool,
    config: &crate::config::Config,
    payload: &Value,
) -> Result<(), String> {
    let period_key = payload
        .get("period_key")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(crate::content::feed_poll::feed_poll_period_key);

    if !crate::jobs::claim_ops_period_once(pool, "council_portfolio_feed_poll", &period_key)
        .await
        .map_err(|e| e.to_string())?
    {
        tracing::info!(
            period_key = %period_key,
            "council_portfolio_feed_poll skipped — period already claimed"
        );
        return Ok(());
    }

    let summary = crate::content::feed_poll::run_portfolio_feed_poll(
        pool,
        config.feed_poll_wall_budget_secs,
    )
    .await?;
    tracing::info!(
        feeds = summary.feeds_polled,
        links = summary.links_upserted,
        crawled = summary.domains_crawled,
        errors = summary.errors,
        budget_exhausted = summary.budget_exhausted,
        feeds_skipped = summary.feeds_skipped,
        crawls_skipped = summary.crawls_skipped,
        "council_portfolio_feed_poll completed"
    );
    Ok(())
}

async fn run_council_invite_tick_job(
    pool: &MySqlPool,
    config: &crate::config::Config,
) -> Result<(), String> {
    let summary = crate::social::council_jobs::run_invite_tick(pool, config).await?;
    if summary.skipped_period_claim {
        tracing::info!(
            members = summary.members_examined,
            "council_invite_tick coordinator skipped — fan-out already claimed for utc day"
        );
        return Ok(());
    }
    if summary.members_examined == 0 {
        return Ok(());
    }
    tracing::info!(
        members = summary.members_examined,
        actions = summary.actions_taken,
        declines = summary.declines,
        errors = summary.errors,
        "council_invite_tick completed"
    );
    Ok(())
}

async fn run_council_invite_article_pick_job(
    pool: &MySqlPool,
    config: &crate::config::Config,
    payload: &Value,
) -> Result<(), String> {
    if let Some(utc_date) = payload.get("utc_date").and_then(|v| v.as_str()) {
        if crate::social::council_jobs::is_stale_daily_period(utc_date) {
            tracing::info!(utc_date, "council_invite_article_pick skipped — stale daily period");
            return Ok(());
        }
    }
    crate::social::council_invite_pipeline::run_invite_article_pick(pool, config, payload).await?;
    tracing::info!("council_invite_article_pick completed");
    Ok(())
}

async fn run_council_invite_person_discover_job(
    pool: &MySqlPool,
    config: &crate::config::Config,
    payload: &Value,
) -> Result<(), String> {
    if let Some(utc_date) = payload.get("utc_date").and_then(|v| v.as_str()) {
        if crate::social::council_jobs::is_stale_daily_period(utc_date) {
            tracing::info!(utc_date, "council_invite_person_discover skipped — stale daily period");
            return Ok(());
        }
    }
    crate::social::council_invite_pipeline::run_invite_person_discover(pool, config, payload)
        .await?;
    tracing::info!("council_invite_person_discover completed");
    Ok(())
}

async fn run_council_invite_contact_resolve_job(
    pool: &MySqlPool,
    config: &crate::config::Config,
    payload: &Value,
) -> Result<(), String> {
    if let Some(utc_date) = payload.get("utc_date").and_then(|v| v.as_str()) {
        if crate::social::council_jobs::is_stale_daily_period(utc_date) {
            tracing::info!(utc_date, "council_invite_contact_resolve skipped — stale daily period");
            return Ok(());
        }
    }
    crate::social::council_invite_pipeline::run_invite_contact_resolve(pool, config, payload)
        .await?;
    tracing::info!("council_invite_contact_resolve completed");
    Ok(())
}

async fn run_council_invite_compose_send_job(
    pool: &MySqlPool,
    config: &crate::config::Config,
    payload: &Value,
) -> Result<(), String> {
    if let Some(utc_date) = payload.get("utc_date").and_then(|v| v.as_str()) {
        if crate::social::council_jobs::is_stale_daily_period(utc_date) {
            tracing::info!(utc_date, "council_invite_compose_send skipped — stale daily period");
            return Ok(());
        }
    }
    crate::social::council_invite_pipeline::run_invite_compose_send(pool, config, payload).await?;
    tracing::info!("council_invite_compose_send completed");
    Ok(())
}

async fn run_content_domain_lifecycle_job(
    pool: &MySqlPool,
    payload: &Value,
) -> Result<(), String> {
    let limit = payload_u64(payload, "limit").unwrap_or(32).clamp(1, 256) as u32;
    let summary = crate::content::domain_lifecycle::run_domain_lifecycle_batch(pool, limit)
        .await
        .map_err(|e| e.to_string())?;
    if summary.domains_blocked > 0 {
        tracing::info!(
            domains_blocked = summary.domains_blocked,
            "content_domain_lifecycle completed"
        );
    }
    Ok(())
}

async fn run_user_engagement_digest_job(
    pool: &MySqlPool,
    config: &crate::config::Config,
    payload: &Value,
) -> Result<(), String> {
    let limit = payload_u64(payload, "limit").unwrap_or(64).clamp(1, 128);
    let sent = crate::social::engagement_digest::run_user_engagement_digest_batch(
        pool, config, limit,
    )
    .await
    .map_err(|e| e.to_string())?;
    if sent > 0 {
        tracing::info!(sent, "user_engagement_digest completed");
    }
    Ok(())
}

async fn run_panoply_daily_status_job(
    pool: &MySqlPool,
    config: &crate::config::Config,
    payload: &Value,
) -> Result<(), String> {
    let period_key = payload
        .get("utc_date")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(crate::social::council_jobs::council_daily_period_key);
    let sent =
        crate::social::panoply::run_panoply_daily_status_batch(pool, config, &period_key)
            .await
            .map_err(|e| e.to_string())?;
    tracing::info!(
        sent,
        period_key = %period_key,
        "panoply_daily_status completed"
    );
    Ok(())
}

async fn run_face_portrait_pipeline_job(
    pool: &MySqlPool,
    config: &crate::config::Config,
    payload: &Value,
) -> Result<(), String> {
    crate::social::face_capture_pending::run_face_portrait_pipeline_job(pool, config, payload).await
}

async fn run_face_capture_pending_cleanup_job(
    pool: &MySqlPool,
    config: &crate::config::Config,
) -> Result<(), String> {
    crate::social::face_capture_pending::run_face_capture_pending_cleanup_job(pool, config).await
}

async fn run_system_journal_retention_job(
    _pool: &MySqlPool,
    config: &crate::config::Config,
    payload: &Value,
) -> Result<(), String> {
    if let Some(utc_date) = payload.get("utc_date").and_then(|v| v.as_str()) {
        if crate::social::council_jobs::is_stale_daily_period(utc_date) {
            tracing::info!(
                utc_date,
                "system_journal_retention skipped — stale daily period"
            );
            return Ok(());
        }
    }

    // Re-enqueued every tick after archive; only attempt a vacuum every few hours.
    if !crate::jobs::journal_retention::should_attempt_now() {
        return Ok(());
    }

    let summary =
        crate::jobs::journal_retention::run_system_journal_retention(config.journal_retention_days);

    if let Some(reason) = &summary.skipped_reason {
        tracing::warn!(
            retention_days = summary.retention_days,
            reason = %reason,
            "system_journal_retention skipped"
        );
        return Ok(());
    }

    if !summary.vacuum_succeeded {
        return Err(format!(
            "journalctl vacuum failed after {}d retention window",
            summary.retention_days
        ));
    }

    tracing::info!(
        retention_days = summary.retention_days,
        freed_bytes = ?summary.freed_bytes,
        "system_journal_retention completed"
    );
    Ok(())
}

async fn run_scope_rematerialize_all_job(pool: &MySqlPool) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    sqlx::query("DELETE FROM entity_read_scope")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    sqlx::query(
        r#"
        INSERT INTO entity_read_scope (
            subject_entity_id,
            visible_entity_id,
            granted_at,
            source
        )
        SELECT
            v.subject_entity_id,
            v.visible_entity_id,
            UTC_TIMESTAMP(6),
            'materialized'
        FROM vw_entity_read_scope_live v
        "#,
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    sqlx::query("DELETE FROM entity_scope_recalc_queue")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

/// Ops conductor: purge archive TTL, reap leases, purge terminal rows, claim a fair batch, run handlers.
///
/// Up to [`Config::job_conductor_parallelism`] jobs run concurrently per tick; gateway-heavy types
/// also respect [`Config::job_conductor_llm_parallelism`]. Jobs are leased immediately before
/// spawn so a long handler does not leave other claimed rows expiring on the queue.
pub async fn run_conductor_tick(
    pool: &MySqlPool,
    config: &crate::config::Config,
    batch_size: usize,
) -> Result<ConductorSummary, sqlx::Error> {
    let _conductor_guard = conductor_mutex().lock().await;

    let mut summary = ConductorSummary::default();
    summary.archive_rows_purged = purge_old_archives(pool).await?;
    summary.leases_reaped = reap_expired_leases(pool).await?;
    summary.terminal_purged = purge_terminal_failures(pool).await?;
    summary.stale_purged = purge_stale_period_jobs(pool).await?;

    let cap = batch_size.max(1).min(256);
    let parallelism = config.job_conductor_parallelism.max(1);
    let llm_parallelism = config
        .job_conductor_llm_parallelism
        .max(1)
        .min(parallelism);
    let lease_secs = job_queue_lease_secs();
    let candidates = fetch_eligible_job_rows(pool).await?;
    let plan = round_robin_claim_ids(&candidates, cap);

    let global_sem = Arc::new(Semaphore::new(parallelism));
    let llm_sem = Arc::new(Semaphore::new(llm_parallelism));
    let mut join_set: JoinSet<JobRunOutcome> = JoinSet::new();
    let mut plan_idx = 0usize;

    while plan_idx < plan.len() || !join_set.is_empty() {
        while join_set.len() < parallelism && plan_idx < plan.len() {
            let id = plan[plan_idx];
            plan_idx += 1;
            if !try_claim_job(pool, id, lease_secs).await? {
                continue;
            }
            summary.claimed += 1;

            let Some(job) = fetch_claimed_job(pool, id).await? else {
                tracing::warn!(job_id = id, "job_queue claimed row missing on fetch — releasing lease");
                let _ = release_orphan_lease(pool, id).await;
                summary.failed_retry += 1;
                continue;
            };

            let cid = job.correlation_id.as_deref().unwrap_or("");
            tracing::warn!(
                job_id = job.id,
                job_type = %job.type_key,
                correlation_id = cid,
                "job_queue dispatch starting"
            );

            let pool_spawn = pool.clone();
            let config_spawn = config.clone();
            let global_sem = global_sem.clone();
            let llm_sem = llm_sem.clone();
            join_set.spawn(async move {
                let _llm = if job_type_uses_llm_gateway(&job.type_key) {
                    Some(
                        llm_sem
                            .acquire_owned()
                            .await
                            .expect("job conductor llm semaphore"),
                    )
                } else {
                    None
                };
                let _global = global_sem
                    .acquire_owned()
                    .await
                    .expect("job conductor global semaphore");
                execute_claimed_job_with_timeout(&pool_spawn, &config_spawn, &job, lease_secs).await
            });
        }

        if join_set.is_empty() {
            break;
        }

        match join_set.join_next().await {
            Some(Ok(outcome)) => apply_job_run_outcome(&mut summary, outcome),
            Some(Err(join_err)) => {
                tracing::error!(error = %join_err, "job_queue dispatch task panicked");
                summary.failed_retry += 1;
            }
            None => break,
        }
    }

    if summary.claimed > 0 && summary.completed == 0 && summary.failed_retry == 0 {
        tracing::warn!(
            claimed = summary.claimed,
            "conductor tick claimed jobs but none completed or failed — rows may have been stolen"
        );
    }

    summary.terminal_purged += purge_terminal_failures(pool).await?;

    // Every conductor tick (including dev "Run Jobs" / `POST /ops/jobs/run`): retry verdict
    // resolution for cases where all juror seats are non-pending but the case is still active.
    // Independent of whether a `judicial_jury_nomination` job was claimed this batch.
    let verdict_sweep = crate::social::judicial::retry_ready_verdict_resolutions(pool, cap as u64)
        .await?;
    summary.judicial_verdict_retry_candidates = verdict_sweep.candidates;
    summary.judicial_verdicts_resolved = verdict_sweep.resolved;

    Ok(summary)
}
