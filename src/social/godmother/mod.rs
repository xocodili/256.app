//! Unified godmother → human private outreach facility.

mod compose;
mod flush;
mod ledger;
mod moment;
mod moments;
mod notify;
mod publish;
mod resolve;

pub use flush::flush_recovery_candidates;
pub use moment::GodmotherMoment;
pub use publish::publish_private_post_tx;

use sqlx::MySqlPool;

use crate::config::Config;
use crate::social::audit;

#[derive(Debug, Clone, Default)]
pub struct OutreachSummary {
    pub delivered: u64,
    pub skipped: u64,
    pub llm_generated: bool,
}

/// Enqueue a godmother outreach job (system-only; deduped on payload hash).
pub async fn enqueue_moment(
    pool: &MySqlPool,
    user_id: Option<u64>,
    human_entity_id: u64,
    moment: GodmotherMoment,
    correlation_id: Option<&str>,
) -> Result<bool, sqlx::Error> {
    let payload = serde_json::json!({
        "human_entity_id": human_entity_id,
        "moment": moment.key(),
    });
    let enqueued = crate::jobs::enqueue_job(
        pool,
        user_id,
        "godmother_outreach",
        &payload,
        correlation_id,
        40,
    )
    .await?;
    Ok(enqueued.job_id > 0)
}

async fn rollback_unpublished_claim(
    pool: &MySqlPool,
    human_entity_id: u64,
    moment: GodmotherMoment,
) {
    if let Err(del_err) = ledger::delete_unpublished_claim(pool, human_entity_id, moment).await {
        tracing::warn!(
            error = %del_err,
            human_entity_id,
            moment = moment.key(),
            "failed to roll back godmother outreach ledger claim"
        );
    }
}

async fn maybe_finalize_global_recovery(
    pool: &MySqlPool,
    config: &Config,
    human_entity_id: u64,
    moment: GodmotherMoment,
) {
    if moment != GodmotherMoment::GlobalRecovery {
        return;
    }
    notify::finalize_global_recovery_notification(pool, config, human_entity_id).await;
}

pub async fn run_outreach(
    pool: &MySqlPool,
    config: &Config,
    human_entity_id: u64,
    moment: GodmotherMoment,
) -> Result<OutreachSummary, String> {
    if resolve::should_skip_human(pool, human_entity_id)
        .await
        .map_err(|e| e.to_string())?
    {
        tracing::info!(
            human_entity_id,
            moment = moment.key(),
            "godmother outreach skipped (death immune)"
        );
        return Ok(OutreachSummary {
            delivered: 0,
            skipped: 1,
            llm_generated: false,
        });
    }

    let resolved = resolve::resolve_active_godmother(pool, human_entity_id)
        .await
        .map_err(|e| e.to_string())?;
    let Some(resolved) = resolved else {
        tracing::info!(
            human_entity_id,
            moment = moment.key(),
            "godmother outreach skipped (no active godmother edge)"
        );
        maybe_finalize_global_recovery(pool, config, human_entity_id, moment).await;
        return Ok(OutreachSummary {
            delivered: 0,
            skipped: 1,
            llm_generated: false,
        });
    };

    match ledger::try_claim(
        pool,
        human_entity_id,
        resolved.godmother_entity_id,
        moment,
    )
    .await
    {
        Ok(ledger::ClaimOutcome::AlreadyDelivered) => {
            maybe_finalize_global_recovery(pool, config, human_entity_id, moment).await;
            return Ok(OutreachSummary {
                delivered: 0,
                skipped: 1,
                llm_generated: false,
            });
        }
        Ok(ledger::ClaimOutcome::Claimed { .. }) => {}
        Err(e) => return Err(e.to_string()),
    }

    let system_user = match audit::system_user_id(pool).await.map_err(|e| e.to_string())? {
        Some(id) => id,
        None => {
            rollback_unpublished_claim(pool, human_entity_id, moment).await;
            return Err("system user not configured".to_string());
        }
    };

    let (markup, llm_generated) = match moments::compose_moment_markup(
        pool,
        config,
        moment,
        resolved.godmother_entity_id,
        human_entity_id,
        &resolved.human_handle,
        system_user,
    )
    .await
    {
        Ok(v) => v,
        Err(e) => {
            rollback_unpublished_claim(pool, human_entity_id, moment).await;
            return Err(e);
        }
    };

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let post_id = match publish::publish_private_post_tx(
        &mut tx,
        resolved.godmother_entity_id,
        human_entity_id,
        system_user,
        &markup,
    )
    .await
    {
        Ok(id) => id,
        Err(e) => {
            tx.rollback().await.ok();
            rollback_unpublished_claim(pool, human_entity_id, moment).await;
            return Err(e);
        }
    };

    if let Err(e) = ledger::record_post_id_tx(&mut tx, human_entity_id, moment, post_id).await {
        tx.rollback().await.ok();
        rollback_unpublished_claim(pool, human_entity_id, moment).await;
        return Err(e.to_string());
    }

    if let Err(e) = tx.commit().await {
        rollback_unpublished_claim(pool, human_entity_id, moment).await;
        return Err(e.to_string());
    }

    maybe_finalize_global_recovery(pool, config, human_entity_id, moment).await;

    tracing::info!(
        post_id,
        human_entity_id,
        godmother_entity_id = resolved.godmother_entity_id,
        moment = moment.key(),
        llm_generated,
        "godmother outreach post published"
    );

    Ok(OutreachSummary {
        delivered: 1,
        skipped: 0,
        llm_generated,
    })
}
