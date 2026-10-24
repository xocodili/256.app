//! One-time civic rating feedback when a post/reply conversation closes.

use sqlx::{MySql, MySqlPool, Transaction};

use crate::social::engagement::{self, PostFeel};
use crate::social::judicial::{
    bump_community_member_rating_tx, bump_global_entity_rating_tx, civic_rating_delta_times,
    RATING_MULT_CONVERSATION_LOATHED_EDGE, RATING_MULT_CONVERSATION_LOATHED_GLOBAL,
    RATING_MULT_CONVERSATION_LOVED_EDGE, RATING_MULT_CONVERSATION_LOVED_GLOBAL,
};

pub const CONVERSATION_INACTIVITY_DAYS: i64 = 3;
pub const CONVERSATION_MIN_VOTES: i64 = 3;

#[derive(Debug, sqlx::FromRow)]
struct SettlementRow {
    post_id: u64,
    root_post_id: u64,
    author_human_entity_id: Option<u64>,
}

pub async fn run_settlement_batch(pool: &MySqlPool, limit: u32) -> Result<u32, sqlx::Error> {
    let lim = limit.clamp(1, 256) as i64;
    let rows: Vec<SettlementRow> = sqlx::query_as(
        r#"
        SELECT
            p.id AS post_id,
            COALESCE(p.root_post_id, p.id) AS root_post_id,
            sh_author.social_entity_id AS author_human_entity_id
        FROM post p
        LEFT JOIN social_human sh_author ON sh_author.social_entity_id = p.entity_id
        WHERE p.conv_rating_applied_at IS NULL
          AND p.conv_opened_at IS NOT NULL
          AND p.conv_last_activity_at IS NOT NULL
          AND p.is_moderation_mirror = 0
          AND UTC_TIMESTAMP(6) >= p.conv_last_activity_at + INTERVAL ? DAY
          AND (
                SELECT COUNT(*)
                FROM post_engagement_vote pev
                WHERE pev.target_post_id = p.id
                  AND pev.voter_user_id <> p.created_by
          ) >= ?
        ORDER BY p.conv_last_activity_at ASC
        LIMIT ?
        "#,
    )
    .bind(CONVERSATION_INACTIVITY_DAYS)
    .bind(CONVERSATION_MIN_VOTES)
    .bind(lim)
    .fetch_all(pool)
    .await?;

    let mut applied = 0u32;
    for row in rows {
        if apply_settlement_tx(pool, &row).await? {
            applied += 1;
        }
    }
    Ok(applied)
}

async fn apply_settlement_tx(pool: &MySqlPool, row: &SettlementRow) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let locked: Option<(u64,)> = sqlx::query_as(
        r#"
        SELECT id
        FROM post
        WHERE id = ?
          AND conv_rating_applied_at IS NULL
          AND conv_opened_at IS NOT NULL
          AND conv_last_activity_at IS NOT NULL
          AND UTC_TIMESTAMP(6) >= conv_last_activity_at + INTERVAL ? DAY
          AND (
                SELECT COUNT(*)
                FROM post_engagement_vote pev
                WHERE pev.target_post_id = post.id
                  AND pev.voter_user_id <> post.created_by
          ) >= ?
        FOR UPDATE
        "#,
    )
    .bind(row.post_id)
    .bind(CONVERSATION_INACTIVITY_DAYS)
    .bind(CONVERSATION_MIN_VOTES)
    .fetch_optional(&mut *tx)
    .await?;
    if locked.is_none() {
        return Ok(false);
    }

    let stars = engagement::recompute_post_engagement_rating_tx(&mut tx, row.post_id).await?;
    let feel = PostFeel::from_stars(stars);

    let community_id =
        crate::social::post_community::community_context_for_root_post_tx(&mut tx, row.root_post_id)
            .await?;
    if let (Some(human_id), Some(community_id)) = (row.author_human_entity_id, community_id) {
        let community_delta = match feel {
            PostFeel::Loved => civic_rating_delta_times(RATING_MULT_CONVERSATION_LOVED_EDGE),
            PostFeel::Controversial => civic_rating_delta_times(1),
            PostFeel::Loathed => civic_rating_delta_times(RATING_MULT_CONVERSATION_LOATHED_EDGE),
        };
        if !community_delta.is_zero() {
            bump_community_member_rating_tx(
                &mut tx,
                human_id,
                community_id,
                community_delta,
                None,
            )
            .await?;
        }
    }

    if let Some(human_id) = row.author_human_entity_id {
        let global_mult = match feel {
            PostFeel::Loved => RATING_MULT_CONVERSATION_LOVED_GLOBAL,
            PostFeel::Controversial => 0i64,
            PostFeel::Loathed => RATING_MULT_CONVERSATION_LOATHED_GLOBAL,
        };
        if global_mult != 0 {
            bump_global_entity_rating_tx(
                &mut tx,
                human_id,
                civic_rating_delta_times(global_mult),
                None,
            )
            .await?;
        }
    }

    sqlx::query(
        r#"
        UPDATE post
        SET conv_rating_applied_at = UTC_TIMESTAMP(6)
        WHERE id = ?
        "#,
    )
    .bind(row.post_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(true)
}

pub async fn init_conversation_tx(
    tx: &mut Transaction<'_, MySql>,
    post_id: u64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE post
        SET conv_opened_at = UTC_TIMESTAMP(6),
            conv_last_activity_at = UTC_TIMESTAMP(6)
        WHERE id = ?
        "#,
    )
    .bind(post_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn touch_conversation_activity_tx(
    tx: &mut Transaction<'_, MySql>,
    post_id: u64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE post
        SET conv_last_activity_at = UTC_TIMESTAMP(6)
        WHERE id = ?
        "#,
    )
    .bind(post_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
