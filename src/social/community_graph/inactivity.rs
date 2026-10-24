//! Post-activity touch helpers and human login-idle inactivity penalties.

use rust_decimal::Decimal;
use sqlx::{MySql, MySqlPool, Transaction};

use crate::ref_keys;
use crate::social::audit;
use crate::social::judicial;

use super::immune::is_human_death_immune;
use super::inactivity_batch::{
    inactivity_decrement_days_to_apply, penalty_for_decrement_day,
    INACTIVITY_DECREMENT_START_IDLE_DAYS, INACTIVITY_PHASE_1_DECREMENT_DAYS,
    INACTIVITY_PHASE_2_DECREMENT_DAYS, INACTIVITY_WARNING_IDLE_DAYS,
};

/// Bump `last_post_activity_at` for any community entities in `entity_ids`.
pub async fn touch_post_activity_for_entities_tx(
    tx: &mut Transaction<'_, MySql>,
    entity_ids: &[u64],
) -> Result<(), sqlx::Error> {
    if entity_ids.is_empty() {
        return Ok(());
    }
    let placeholders = entity_ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
    let sql = format!(
        r#"
        UPDATE social_community sc
        INNER JOIN social_entity se ON se.id = sc.social_entity_id
        INNER JOIN ref_key_values rk
            ON rk.id = se.type_id
           AND rk.`group` = ?
           AND rk.`key` = ?
        SET sc.last_post_activity_at = GREATEST(
            COALESCE(sc.last_post_activity_at, UTC_TIMESTAMP(6)),
            UTC_TIMESTAMP(6)
        )
        WHERE sc.social_entity_id IN ({placeholders})
        "#
    );
    let mut q = sqlx::query(&sql);
    q = q.bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE);
    q = q.bind(ref_keys::KEY_ENTITY_TYPE_COMMUNITY);
    for id in entity_ids {
        q = q.bind(id);
    }
    q.execute(&mut **tx).await?;
    Ok(())
}

/// Record post activity for a community (author or audience).
pub async fn touch_community_post_activity_tx(
    tx: &mut Transaction<'_, MySql>,
    community_entity_id: u64,
    at: chrono::NaiveDateTime,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE social_community
        SET last_post_activity_at = GREATEST(COALESCE(last_post_activity_at, ?), ?)
        WHERE social_entity_id = ?
        "#,
    )
    .bind(at)
    .bind(at)
    .bind(community_entity_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct HumanInactivityCandidate {
    user_id: u64,
    email: String,
    human_entity_id: u64,
    handle: String,
    display_label: Option<String>,
    last_login_at: chrono::DateTime<chrono::Utc>,
    idle_days: i64,
    decrement_days_applied: u32,
    rating: Decimal,
    inactivity_rating_debited: Decimal,
}

/// Apply phased login-inactivity penalties and notification emails for verified humans.
pub async fn run_human_inactivity_penalty_batch(
    pool: &MySqlPool,
    config: &crate::config::Config,
    limit: u64,
) -> Result<u64, sqlx::Error> {
    let lim = limit.clamp(1, 64) as i64;
    let candidates: Vec<HumanInactivityCandidate> = sqlx::query_as(
        r#"
        SELECT
            u.id AS user_id,
            u.email,
            se.id AS human_entity_id,
            se.handle,
            COALESCE(
                NULLIF(TRIM(CONCAT_WS(' ', sh.given_name, sh.additional_name, sh.family_name)), ''),
                se.handle
            ) AS display_label,
            COALESCE(u.last_login_at, u.created_at) AS last_login_at,
            TIMESTAMPDIFF(
                DAY,
                COALESCE(u.last_login_at, u.created_at),
                UTC_TIMESTAMP(6)
            ) AS idle_days,
            u.inactivity_penalties_applied AS decrement_days_applied,
            se.rating,
            u.inactivity_rating_debited
        FROM users u
        INNER JOIN user_social_entity_role ur
            ON ur.user_id = u.id
           AND ur.is_primary = 1
           AND (ur.ends_at IS NULL OR ur.ends_at >= UTC_DATE())
        INNER JOIN ref_key_values rk_role
            ON rk_role.id = ur.role_id
           AND rk_role.`group` = 'user_social_entity_role'
           AND rk_role.`key` = 'owner'
        INNER JOIN social_entity se ON se.id = ur.social_entity_id
        INNER JOIN social_human sh ON sh.social_entity_id = se.id
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = se.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` = ?
        INNER JOIN ref_key_values rk_st
            ON rk_st.id = se.entity_status_id
           AND rk_st.`group` = ?
           AND rk_st.`key` = ?
        WHERE u.email_verified_at IS NOT NULL
          AND TRIM(u.email) <> ''
          AND sh.died_at IS NULL
          AND se.rating > 0.00
        HAVING idle_days >= ?
        ORDER BY idle_days DESC, u.id ASC
        LIMIT ?
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(ref_keys::KEY_ENTITY_TYPE_HUMAN)
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_STATUS)
    .bind(ref_keys::KEY_VERIFIED)
    .bind(INACTIVITY_WARNING_IDLE_DAYS)
    .bind(lim)
    .fetch_all(pool)
    .await?;

    let system_actor = audit::system_user_id(pool).await?;
    let mut applied_total = 0u64;

    for row in candidates {
        if is_human_death_immune(pool, row.human_entity_id).await? {
            continue;
        }

        let label = row
            .display_label
            .clone()
            .unwrap_or_else(|| row.handle.clone());

        if row.idle_days >= INACTIVITY_WARNING_IDLE_DAYS {
            let claimed: u64 = sqlx::query(
                r#"
                UPDATE users
                SET inactivity_warning_notified_at = UTC_TIMESTAMP(6)
                WHERE id = ?
                  AND inactivity_warning_notified_at IS NULL
                "#,
            )
            .bind(row.user_id)
            .execute(pool)
            .await?
            .rows_affected();
            if claimed > 0 {
                crate::email::notify_human_inactivity_warning(config, &row.email, &label).await;
            }
        }

        if row.idle_days >= INACTIVITY_DECREMENT_START_IDLE_DAYS {
            let claimed: u64 = sqlx::query(
                r#"
                UPDATE users
                SET inactivity_decrement_started_notified_at = UTC_TIMESTAMP(6)
                WHERE id = ?
                  AND inactivity_decrement_started_notified_at IS NULL
                "#,
            )
            .bind(row.user_id)
            .execute(pool)
            .await?
            .rows_affected();
            if claimed > 0 {
                crate::email::notify_human_inactivity_decrement_started(config, &row.email, &label)
                    .await;
            }
        }

        let to_apply =
            inactivity_decrement_days_to_apply(row.idle_days, row.decrement_days_applied);
        if to_apply == 0 {
            continue;
        }

        let mut tx = pool.begin().await?;
        let mut debited_total = row.inactivity_rating_debited;
        let mut next_decrement_day = row.decrement_days_applied.saturating_add(1);
        let mut notify_accel_3x = false;
        let mut notify_accel_6x = false;

        for _ in 0..to_apply {
            if next_decrement_day
                == INACTIVITY_PHASE_1_DECREMENT_DAYS.saturating_add(1)
            {
                let claimed: u64 = sqlx::query(
                    r#"
                    UPDATE users
                    SET inactivity_acceleration_3x_notified_at = UTC_TIMESTAMP(6)
                    WHERE id = ?
                      AND inactivity_acceleration_3x_notified_at IS NULL
                    "#,
                )
                .bind(row.user_id)
                .execute(&mut *tx)
                .await?
                .rows_affected();
                if claimed > 0 {
                    notify_accel_3x = true;
                }
            }
            let phase_3_start = INACTIVITY_PHASE_1_DECREMENT_DAYS
                .saturating_add(INACTIVITY_PHASE_2_DECREMENT_DAYS)
                .saturating_add(1);
            if next_decrement_day == phase_3_start {
                let claimed: u64 = sqlx::query(
                    r#"
                    UPDATE users
                    SET inactivity_acceleration_6x_notified_at = UTC_TIMESTAMP(6)
                    WHERE id = ?
                      AND inactivity_acceleration_6x_notified_at IS NULL
                    "#,
                )
                .bind(row.user_id)
                .execute(&mut *tx)
                .await?
                .rows_affected();
                if claimed > 0 {
                    notify_accel_6x = true;
                }
            }

            let penalty = penalty_for_decrement_day(next_decrement_day);
            judicial::bump_global_entity_rating_tx(
                &mut tx,
                row.human_entity_id,
                penalty,
                system_actor,
            )
            .await?;
            debited_total -= penalty;

            sqlx::query(
                r#"
                INSERT INTO human_graph_event (
                    event_type,
                    human_social_entity_id,
                    detail_json,
                    created_by
                )
                VALUES ('inactivity_penalty', ?, JSON_OBJECT('idle_days', ?, 'decrement_day', ?), ?)
                "#,
            )
            .bind(row.human_entity_id)
            .bind(row.idle_days)
            .bind(next_decrement_day)
            .bind(system_actor)
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                r#"
                UPDATE users
                SET inactivity_penalties_applied = inactivity_penalties_applied + 1,
                    inactivity_rating_debited = ?
                WHERE id = ?
                "#,
            )
            .bind(debited_total)
            .bind(row.user_id)
            .execute(&mut *tx)
            .await?;

            next_decrement_day = next_decrement_day.saturating_add(1);
            applied_total += 1;
        }

        tx.commit().await?;

        if notify_accel_3x {
            crate::email::notify_human_inactivity_acceleration_3x(config, &row.email, &label).await;
        }
        if notify_accel_6x {
            crate::email::notify_human_inactivity_acceleration_6x(config, &row.email, &label).await;
        }
    }

    Ok(applied_total)
}
