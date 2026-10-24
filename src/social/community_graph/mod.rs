//! Social graph lifecycle: community death at rating zero, human removal at rating zero,
//! subtree helpers, judicial reparent apply, and inactivity pruning.

mod death_brand;
mod death_community;
mod death_human;
mod death_shared;
mod immune;
mod inactivity;
mod inactivity_batch;
mod reparent;
mod tree;
mod types;

#[cfg(test)]
mod inactivity_tests;

pub use death_brand::extension_entity_death_voluntary_tx;
pub use death_community::{community_death_tx, list_death_candidates};
pub use death_human::{
    human_death_tx, human_death_voluntary_tx, human_safety_ban_tx, is_human_failing,
    list_human_death_candidates,
};
pub use immune::{
    is_death_immune, is_death_immune_conn, is_death_immune_handle, is_human_death_immune,
    is_human_death_immune_conn, is_human_death_immune_handle, is_panoply_operator_conn,
    is_panoply_operator_for_user, COUNCIL_COMMUNITY_HANDLES,
};
pub use inactivity::{
    run_human_inactivity_penalty_batch, touch_community_post_activity_tx,
    touch_post_activity_for_entities_tx,
};
pub use inactivity_batch::{
    estimated_deletion_at_from_last_login, format_estimated_deletion_date,
    penalties_until_rating_zero, INACTIVITY_FIRST_PENALTY_DAYS, INACTIVITY_WARNING_IDLE_DAYS,
};
pub use reparent::{apply_judicial_reparent_tx, reparent_children_of_dead_community_tx};
pub use tree::{
    collect_subtree_community_ids, collect_subtree_community_ids_conn, community_rating,
    is_community_failing, list_immediate_children, would_create_cycle,
};
pub use types::DeathSource;

use sqlx::MySqlPool;

use crate::social::audit;

/// Process rating-zero death candidates (communities and humans) in one job tick.
pub async fn run_death_batch(pool: &MySqlPool, batch: u64) -> Result<u64, sqlx::Error> {
    let per_type = (batch / 2).max(1);
    let community_candidates = list_death_candidates(pool, per_type).await?;
    let human_candidates = list_human_death_candidates(pool, per_type).await?;
    let system_actor = audit::system_user_id(pool).await?;
    let mut processed = 0u64;
    for community_id in community_candidates {
        let mut tx = pool.begin().await?;
        if community_death_tx(
            &mut tx,
            community_id,
            DeathSource::RatingZero,
            system_actor,
        )
        .await?
        {
            tx.commit().await?;
            processed += 1;
        } else {
            tx.rollback().await?;
        }
    }
    for human_id in human_candidates {
        let mut tx = pool.begin().await?;
        if human_death_tx(
            &mut tx,
            human_id,
            DeathSource::RatingZero,
            system_actor,
        )
        .await?
        {
            tx.commit().await?;
            processed += 1;
        } else {
            tx.rollback().await?;
        }
    }
    Ok(processed)
}
