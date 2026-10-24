//! In-process recurring job scheduler.
//!
//! Spawns a single tokio task that, on a configurable interval:
//!  1. Enqueues recurring system jobs (`judicial_jury_nomination`,
//!     `emergency_standing_settlement`, `conversation_rating_settlement`,
//!     `community_graph_lifecycle`, `judicial_council_appeal`, council AI ticks,
//!     `content_domain_lifecycle`, `user_engagement_digest`, `scope_recalc_drain`). Enqueues are deduped on
//!     `(job_type_id, payload_hash)`, so calling them every tick is safe —
//!     work doesn't pile up when a previous job is still pending.
//!  2. Runs [`super::queue::run_conductor_tick`] to drain the queue (up to
//!     [`crate::config::Config::job_conductor_parallelism`] handlers in parallel per tick).
//!
//! The loop is intentionally simple: any error from a single tick is logged
//! and the loop continues. The runtime tearing down at shutdown aborts the
//! task; the conductor's 64s lease reaper picks up anything that was mid-flight
//! on the next process start.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::json;
use sqlx::MySqlPool;
use tokio::time::{interval, MissedTickBehavior};

use crate::config::Config;

const SCHEDULER_CORR_ID: &str = "scheduler";
const JUDICIAL_JURY_NOMINATION: &str = "judicial_jury_nomination";
const JUDICIAL_COUNCIL_APPEAL: &str = "judicial_council_appeal";
const SCOPE_RECALC_DRAIN: &str = "scope_recalc_drain";
const COMMUNITY_GRAPH_LIFECYCLE: &str = "community_graph_lifecycle";
const CONVERSATION_RATING_SETTLEMENT: &str = "conversation_rating_settlement";
const EMERGENCY_STANDING_SETTLEMENT: &str = "emergency_standing_settlement";
const COUNCIL_REPLY_TICK: &str = "council_reply_tick";
const COUNCIL_POST_TICK: &str = "council_post_tick";
const COUNCIL_INVITE_TICK: &str = "council_invite_tick";
const COUNCIL_PORTFOLIO_FEED_POLL: &str = "council_portfolio_feed_poll";
const CONTENT_DOMAIN_LIFECYCLE: &str = "content_domain_lifecycle";
const USER_ENGAGEMENT_DIGEST: &str = "user_engagement_digest";
const PANOPLY_DAILY_STATUS: &str = "panoply_daily_status";
const FACE_PORTRAIT_PIPELINE: &str = "face_portrait_pipeline";
const FACE_CAPTURE_PENDING_CLEANUP: &str = "face_capture_pending_cleanup";
const SYSTEM_JOURNAL_RETENTION: &str = "system_journal_retention";

static SCHEDULER_TICK_COUNT: AtomicU64 = AtomicU64::new(0);

/// Last conductor tick claim count (for health/ops probes).
#[must_use]
pub fn scheduler_last_claimed() -> u64 {
    SCHEDULER_LAST_CLAIMED.load(Ordering::Relaxed)
}

/// Total scheduler loop iterations since process start.
#[must_use]
pub fn scheduler_tick_count() -> u64 {
    SCHEDULER_TICK_COUNT.load(Ordering::Relaxed)
}

static SCHEDULER_LAST_CLAIMED: AtomicU64 = AtomicU64::new(0);

/// Spawn the scheduler. Returns immediately; the loop runs in the background.
///
/// `tick_interval == Duration::ZERO` disables the scheduler entirely, which is
/// the right behaviour when callers want the queue advanced only via the ops
/// endpoint (e.g. tests, one-shot CLI runs, or production deployments that
/// pair the app with an external orchestrator).
pub fn spawn(pool: MySqlPool, config: Config, tick_interval: Duration, batch_size: usize) {
    if tick_interval.is_zero() {
        tracing::info!("job scheduler disabled (tick_interval=0)");
        return;
    }
    let batch_size = batch_size.clamp(1, 256);
    tokio::spawn(async move {
        tracing::warn!(
            interval_secs = tick_interval.as_secs(),
            batch_size,
            "job scheduler starting (sequential per-job lease)"
        );
        let mut tick = interval(tick_interval);
        // Don't fire a burst of catch-up ticks if the runtime was paused
        // (e.g. the host was suspended); just resume cadence.
        tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            if let Err(err) = run_cycle(&pool, &config, batch_size).await {
                tracing::warn!(error = %err, "scheduler tick failed");
            }
        }
    });
}

async fn run_cycle(pool: &MySqlPool, config: &Config, batch_size: usize) -> Result<(), sqlx::Error> {
    enqueue_recurring_system_jobs(pool, SCHEDULER_CORR_ID).await;
    let summary = super::queue::run_conductor_tick(pool, config, batch_size).await?;
    SCHEDULER_LAST_CLAIMED.store(summary.claimed, Ordering::Relaxed);
    let tick = SCHEDULER_TICK_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    if summary.claimed > 0
        || summary.completed > 0
        || summary.failed_retry > 0
        || tick % 60 == 0
    {
        tracing::info!(
            tick,
            claimed = summary.claimed,
            completed = summary.completed,
            failed_retry = summary.failed_retry,
            terminal_purged = summary.terminal_purged,
            stale_purged = summary.stale_purged,
            "scheduler heartbeat"
        );
    }
    Ok(())
}

/// Enqueue all recurring system jobs (deduped on payload hash). Used by the
/// in-process scheduler and the dev **Run Jobs** button.
pub async fn enqueue_recurring_system_jobs(pool: &MySqlPool, correlation_id: &str) {
    let corr = correlation_id.trim();
    let corr = if corr.is_empty() { SCHEDULER_CORR_ID } else { corr };
    // Jury nomination: the inner job is cheap when there's nothing to do
    // (one indexed `SELECT` over judicial cases), so enqueueing every tick
    // is fine. Dedup collapses concurrent enqueues to a single pending row.
    if let Err(err) = super::queue::enqueue_job(
        pool,
        None,
        JUDICIAL_JURY_NOMINATION,
        &json!({ "limit": 32 }),
        Some(corr),
        50,
    )
    .await
    {
        tracing::warn!(
            error = %err,
            job_type = JUDICIAL_JURY_NOMINATION,
            "scheduler failed to enqueue recurring job"
        );
    }

    if let Err(err) = super::queue::enqueue_job(
        pool,
        None,
        JUDICIAL_COUNCIL_APPEAL,
        &json!({ "case_limit": 4 }),
        Some(corr),
        55,
    )
    .await
    {
        tracing::warn!(
            error = %err,
            job_type = JUDICIAL_COUNCIL_APPEAL,
            "scheduler failed to enqueue recurring job"
        );
    }

    if let Err(err) = super::queue::enqueue_job(
        pool,
        None,
        SCOPE_RECALC_DRAIN,
        &json!({ "limit": 256 }),
        Some(corr),
        60,
    )
    .await
    {
        tracing::warn!(
            error = %err,
            job_type = SCOPE_RECALC_DRAIN,
            "scheduler failed to enqueue recurring job"
        );
    }

    if let Err(err) = super::queue::enqueue_job(
        pool,
        None,
        COMMUNITY_GRAPH_LIFECYCLE,
        &json!({
            "death_batch": 8,
            "human_inactivity_batch": 16
        }),
        Some(corr),
        65,
    )
    .await
    {
        tracing::warn!(
            error = %err,
            job_type = COMMUNITY_GRAPH_LIFECYCLE,
            "scheduler failed to enqueue recurring job"
        );
    }

    if let Err(err) = super::queue::enqueue_job(
        pool,
        None,
        CONVERSATION_RATING_SETTLEMENT,
        &json!({ "limit": 32 }),
        Some(corr),
        70,
    )
    .await
    {
        tracing::warn!(
            error = %err,
            job_type = CONVERSATION_RATING_SETTLEMENT,
            "scheduler failed to enqueue recurring job"
        );
    }

    if let Err(err) = super::queue::enqueue_job(
        pool,
        None,
        EMERGENCY_STANDING_SETTLEMENT,
        &json!({ "limit": 32 }),
        Some(corr),
        75,
    )
    .await
    {
        tracing::warn!(
            error = %err,
            job_type = EMERGENCY_STANDING_SETTLEMENT,
            "scheduler failed to enqueue recurring job"
        );
    }

    let reply_period = crate::social::council_jobs::council_reply_period_key();
    enqueue_period_job_if_open(
        pool,
        COUNCIL_REPLY_TICK,
        &reply_period,
        COUNCIL_REPLY_TICK,
        &json!({ "period_key": reply_period }),
        corr,
        80,
    )
    .await;

    let feed_period = crate::content::feed_poll::feed_poll_period_key();
    enqueue_period_job_if_open(
        pool,
        COUNCIL_PORTFOLIO_FEED_POLL,
        &feed_period,
        COUNCIL_PORTFOLIO_FEED_POLL,
        &json!({ "period_key": feed_period }),
        corr,
        81,
    )
    .await;

    let daily_period = crate::social::council_jobs::council_daily_period_key();
    // Daily coordinators dedupe on live queue rows; period idempotency is enforced in each handler.
    if let Err(err) = super::queue::enqueue_job(
        pool,
        None,
        COUNCIL_POST_TICK,
        &json!({ "utc_date": daily_period }),
        Some(corr),
        72,
    )
    .await
    {
        tracing::warn!(
            error = %err,
            job_type = COUNCIL_POST_TICK,
            "scheduler failed to enqueue recurring job"
        );
    }

    if let Err(err) = super::queue::enqueue_job(
        pool,
        None,
        COUNCIL_INVITE_TICK,
        &json!({ "utc_date": daily_period }),
        Some(corr),
        73,
    )
    .await
    {
        tracing::warn!(
            error = %err,
            job_type = COUNCIL_INVITE_TICK,
            "scheduler failed to enqueue recurring job"
        );
    }

    if let Err(err) = super::queue::enqueue_job(
        pool,
        None,
        CONTENT_DOMAIN_LIFECYCLE,
        &json!({ "limit": 32 }),
        Some(corr),
        86,
    )
    .await
    {
        tracing::warn!(
            error = %err,
            job_type = CONTENT_DOMAIN_LIFECYCLE,
            "scheduler failed to enqueue recurring job"
        );
    }

    if let Err(err) = super::queue::enqueue_job(
        pool,
        None,
        USER_ENGAGEMENT_DIGEST,
        &json!({
            "utc_date": daily_period,
            "limit": 64
        }),
        Some(corr),
        88,
    )
    .await
    {
        tracing::warn!(
            error = %err,
            job_type = USER_ENGAGEMENT_DIGEST,
            "scheduler failed to enqueue recurring job"
        );
    }

    if let Err(err) = super::queue::enqueue_job(
        pool,
        None,
        PANOPLY_DAILY_STATUS,
        &json!({ "utc_date": daily_period }),
        Some(corr),
        74,
    )
    .await
    {
        tracing::warn!(
            error = %err,
            job_type = PANOPLY_DAILY_STATUS,
            "scheduler failed to enqueue recurring job"
        );
    }

    if let Err(err) = super::queue::enqueue_job(
        pool,
        None,
        FACE_CAPTURE_PENDING_CLEANUP,
        &json!({ "limit": 256 }),
        Some(corr),
        92,
    )
    .await
    {
        tracing::warn!(
            error = %err,
            job_type = FACE_CAPTURE_PENDING_CLEANUP,
            "scheduler failed to enqueue recurring job"
        );
    }

    if let Err(err) = super::queue::enqueue_job(
        pool,
        None,
        SYSTEM_JOURNAL_RETENTION,
        &json!({ "utc_date": daily_period }),
        Some(corr),
        93,
    )
    .await
    {
        tracing::warn!(
            error = %err,
            job_type = SYSTEM_JOURNAL_RETENTION,
            "scheduler failed to enqueue recurring job"
        );
    }
}

/// Skip enqueue when the period is already claimed — live queue rows are removed on
/// archive, so otherwise the scheduler would re-add the same payload every tick.
async fn enqueue_period_job_if_open(
    pool: &MySqlPool,
    digest_key: &str,
    period_key: &str,
    job_type: &str,
    payload: &serde_json::Value,
    correlation_id: &str,
    priority: i16,
) {
    match super::ops_period_already_claimed(pool, digest_key, period_key).await {
        Ok(true) => return,
        Ok(false) => {}
        Err(err) => {
            tracing::warn!(
                error = %err,
                digest_key,
                job_type,
                "scheduler period claim probe failed"
            );
            return;
        }
    }

    if let Err(err) = super::queue::enqueue_job(
        pool,
        None,
        job_type,
        payload,
        Some(correlation_id),
        priority,
    )
    .await
    {
        tracing::warn!(
            error = %err,
            job_type,
            "scheduler failed to enqueue recurring job"
        );
    }
}
