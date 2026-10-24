use rand::seq::SliceRandom;
use sqlx::{MySql, MySqlPool};

use crate::ref_key_values;
use crate::ref_keys;
use crate::social::audit;

use super::deadlines::{close_full_community_jury_pool_insufficient_tx, close_jury_cases_past_no_jury_deadline};
use super::pool::juror_pool_for_case_conn;
use super::sizing::jury_size_for_case;
use super::triggers::trigger_requires_full_community_jury_pool;
use super::types::{CreatedJurorInvite, JuryNominationSummary, JurorCandidate, OpenCaseRow, VerdictResolutionRetrySummary};
use super::verdict::check_and_resolve_verdict;
pub async fn nominate_jurors_for_open_cases(
    pool: &MySqlPool,
    limit: u64,
) -> Result<JuryNominationSummary, sqlx::Error> {
    let Some(pending_invite_status_id) = ref_key_values::lookup_active_id(
        pool,
        ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS,
        ref_keys::KEY_JCJI_PENDING,
    )
    .await?
    else {
        return Err(sqlx::Error::Configuration(
            "ref_key_values missing judicial_case_juror_invite_status.pending".into(),
        ));
    };

    let Some(rating_basis_community_id) = ref_key_values::lookup_active_id(
        pool,
        ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_RATING_BASIS,
        ref_keys::KEY_JCJI_RB_COMMUNITY,
    )
    .await?
    else {
        return Err(sqlx::Error::Configuration(
            "ref_key_values missing judicial_case_juror_invite_rating_basis.community".into(),
        ));
    };

    let cap = limit.clamp(1, 256);
    let cases_closed_no_jury =
        close_jury_cases_past_no_jury_deadline(pool, cap).await?;
    let rows = cases_needing_jurors(pool, cap).await?;
    let mut summary = JuryNominationSummary {
        cases_examined: rows.len() as u64,
        cases_closed_no_jury,
        ..JuryNominationSummary::default()
    };

    let system_actor = audit::system_user_id(pool).await?;

    for case in rows {
        let mut tx = pool.begin().await?;

        let Some(locked_case) = lock_case_if_active(&mut tx, case.id).await? else {
            tx.rollback().await?;
            continue;
        };

        // Pool snapshot on the same connection as the case lock (see `juror_pool_for_case_conn`).
        let pool_candidates =
            juror_pool_for_case_conn(&mut *tx, &locked_case, rating_basis_community_id).await?;

        let mut target_seats = locked_case.jury_size as usize;
        if target_seats == 0 {
            // Legacy rows filed before `jury_size` was persisted: freeze from pool once.
            target_seats = jury_size_for_case(&locked_case.trigger_key, pool_candidates.len());
            if target_seats > 0 {
                sqlx::query(
                    r#"UPDATE judicial_case SET jury_size = ?, modified_by = ? WHERE id = ? AND jury_size = 0"#,
                )
                .bind(target_seats as u8)
                .bind(system_actor)
                .bind(locked_case.id)
                .execute(&mut *tx)
                .await?;
            }
        }

        let jury_size = target_seats.min(pool_candidates.len());
        if jury_size == 0
            || (trigger_requires_full_community_jury_pool(&locked_case.trigger_key)
                && pool_candidates.len() < target_seats)
        {
            if trigger_requires_full_community_jury_pool(&locked_case.trigger_key) {
                match close_full_community_jury_pool_insufficient_tx(
                    &mut tx,
                    locked_case.id,
                    &locked_case.trigger_key,
                )
                .await
                {
                    Ok(true) => summary.cases_closed_no_jury += 1,
                    Ok(false) => {}
                    Err(err) => tracing::warn!(
                        error = %err,
                        case_id = locked_case.id,
                        "close_full_community_jury_pool_insufficient_tx failed"
                    ),
                }
            }
            tx.commit().await?;
            continue;
        }

        void_disqualified_invites(&mut tx, &locked_case, system_actor).await?;
        let occupied_seats = occupied_juror_seat_entity_ids(&mut tx, locked_case.id).await?;
        let blocked_from_renomination =
            all_invited_juror_entity_ids_for_case(&mut tx, locked_case.id).await?;
        let was_open = locked_case.status_key == ref_keys::KEY_JUDICIAL_CASE_OPEN;
        if occupied_seats.len() >= jury_size {
            // Already at full strength. Promote a still-`open` case to
            // `jury_active`; otherwise no state change is needed.
            if was_open {
                activate_jury(&mut tx, locked_case.id, system_actor).await?;
                summary.cases_filled += 1;
            }
            tx.commit().await?;
            continue;
        }

        let needed = jury_size - occupied_seats.len();
        let mut candidates: Vec<JurorCandidate> = pool_candidates
            .into_iter()
            .filter(|c| !blocked_from_renomination.contains(&c.juror_social_entity_id))
            .collect();
        candidates.shuffle(&mut rand::thread_rng());

        let mut created = 0u64;
        for candidate in candidates.into_iter().take(needed) {
            // `ON DUPLICATE KEY UPDATE` handles voided / expired rows for the same
            // `(case, juror)` pair. Abstained / declined jurors stay in
            // `blocked_from_renomination` so they are never picked again.
            let res = sqlx::query(
                r#"
                INSERT INTO judicial_case_juror_invite (
                    judicial_case_id,
                    juror_social_entity_id,
                    juror_user_id,
                    juror_rating_at_invite,
                    rating_basis_id,
                    rating_community_entity_id,
                    invite_status_id,
                    selected_at,
                    responded_at,
                    created_by,
                    modified_by
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, UTC_TIMESTAMP(6), NULL, ?, ?)
                ON DUPLICATE KEY UPDATE
                    juror_user_id = VALUES(juror_user_id),
                    juror_rating_at_invite = VALUES(juror_rating_at_invite),
                    rating_basis_id = VALUES(rating_basis_id),
                    rating_community_entity_id = VALUES(rating_community_entity_id),
                    invite_status_id = VALUES(invite_status_id),
                    selected_at = VALUES(selected_at),
                    responded_at = NULL,
                    modified_by = VALUES(modified_by)
                "#,
            )
            .bind(locked_case.id)
            .bind(candidate.juror_social_entity_id)
            .bind(candidate.juror_user_id)
            .bind(candidate.juror_rating_at_invite)
            .bind(candidate.rating_basis_id)
            .bind(candidate.rating_community_entity_id)
            .bind(pending_invite_status_id)
            .bind(system_actor)
            .bind(system_actor)
            .execute(&mut *tx)
            .await?;
            if res.rows_affected() > 0 {
                created += 1;
                if let Some(juror_user_id) = candidate.juror_user_id {
                    summary.new_invites.push(CreatedJurorInvite {
                        invite_id: res.last_insert_id(),
                        juror_user_id,
                    });
                }
            }
        }

        let final_count = occupied_seats.len() + created as usize;
        if final_count >= jury_size && was_open {
            activate_jury(&mut tx, locked_case.id, system_actor).await?;
            summary.cases_filled += 1;
        }

        if created > 0 {
            if let Some(actor) = system_actor {
                audit::touch_judicial_case_modified_by_tx(&mut tx, locked_case.id, actor).await?;
            }
        }

        tx.commit().await?;
        summary.invites_created += created;
    }

    // If `check_and_resolve_verdict` failed after the last vote committed (e.g. transient DB
    // decode errors), the case can sit `open`/`jury_active` with zero pending invites. Retry
    // resolution here so the recurring nomination tick self-heals.
    let _ = retry_ready_verdict_resolutions(pool, cap).await?;

    Ok(summary)
}

/// For `open` / `jury_active` cases with no `pending` juror invites, run [`check_and_resolve_verdict`].
///
/// Covers votes fully cast but case not yet `resolved` (failed resolution pass, manual SQL,
/// or rating drift). Invoked from [`nominate_jurors_for_open_cases`] and from
/// [`crate::jobs::queue::run_conductor_tick`] so dev **Run Jobs** advances these without waiting
/// for a `judicial_jury_nomination` queue row.
pub async fn retry_ready_verdict_resolutions(
    pool: &MySqlPool,
    limit: u64,
) -> Result<VerdictResolutionRetrySummary, sqlx::Error> {
    let retry_case_ids = case_ids_with_no_pending_invites(pool, limit).await?;
    let candidates = retry_case_ids.len() as u64;
    let mut resolved = 0u64;
    for case_id in retry_case_ids {
        match check_and_resolve_verdict(pool, case_id).await {
            Ok(Some(_)) => resolved += 1,
            Ok(None) => {}
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    case_id,
                    "check_and_resolve_verdict (retry_ready_verdict_resolutions) failed"
                );
            }
        }
    }
    Ok(VerdictResolutionRetrySummary {
        candidates,
        resolved,
    })
}

/// Cases in `open` or `jury_active` with no `pending` juror invites (verdict may be ready to close).
async fn case_ids_with_no_pending_invites(
    pool: &MySqlPool,
    limit: u64,
) -> Result<Vec<u64>, sqlx::Error> {
    let lim = limit.clamp(1, 256) as i64;
    sqlx::query_scalar(
        r#"
        SELECT jc.id
        FROM judicial_case jc
        INNER JOIN ref_key_values rk_st
            ON rk_st.id = jc.status_id
           AND rk_st.`group` = ?
           AND rk_st.`key` IN (?, ?)
           AND rk_st.is_active = 1
        WHERE NOT EXISTS (
            SELECT 1
            FROM judicial_case_juror_invite jcji
            INNER JOIN ref_key_values rk_pend
                ON rk_pend.id = jcji.invite_status_id
               AND rk_pend.`group` = ?
               AND rk_pend.`key` = ?
               AND rk_pend.is_active = 1
            WHERE jcji.judicial_case_id = jc.id
        )
          AND EXISTS (
            SELECT 1
            FROM judicial_case_juror_invite jcji_v
            INNER JOIN ref_key_values rk_acc
                ON rk_acc.id = jcji_v.invite_status_id
               AND rk_acc.`group` = ?
               AND rk_acc.`key` = ?
               AND rk_acc.is_active = 1
            WHERE jcji_v.judicial_case_id = jc.id
              AND jcji_v.vote_direction_id IS NOT NULL
        )
        ORDER BY jc.opened_at ASC, jc.id ASC
        LIMIT ?
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(ref_keys::KEY_JUDICIAL_CASE_OPEN)
    .bind(ref_keys::KEY_JUDICIAL_CASE_JURY_ACTIVE)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
    .bind(ref_keys::KEY_JCJI_PENDING)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
    .bind(ref_keys::KEY_JCJI_ACCEPTED)
    .bind(lim)
    .fetch_all(pool)
    .await
}

async fn cases_needing_jurors(
    pool: &MySqlPool,
    limit: u64,
) -> Result<Vec<OpenCaseRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT
            jc.id,
            jc.originator_social_entity_id,
            jc.target_social_entity_id,
            jc.created_by,
            rk_trigger.`key` AS trigger_key,
            jc.subject_post_id,
            jc.jury_size,
            rk_status.`key` AS status_key
        FROM judicial_case jc
        INNER JOIN ref_key_values rk_status
            ON rk_status.id = jc.status_id
           AND rk_status.`group` = ?
           AND rk_status.is_active = 1
           AND rk_status.`key` IN (?, ?)
        INNER JOIN ref_key_values rk_trigger
            ON rk_trigger.id = jc.trigger_type_id
           AND rk_trigger.`group` = ?
           AND rk_trigger.is_active = 1
        ORDER BY jc.opened_at ASC, jc.id ASC
        LIMIT ?
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(ref_keys::KEY_JUDICIAL_CASE_OPEN)
    .bind(ref_keys::KEY_JUDICIAL_CASE_JURY_ACTIVE)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_TRIGGER)
    .bind(limit)
    .fetch_all(pool)
    .await
}

async fn lock_case_if_active(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    case_id: u64,
) -> Result<Option<OpenCaseRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT
            jc.id,
            jc.originator_social_entity_id,
            jc.target_social_entity_id,
            jc.created_by,
            rk_trigger.`key` AS trigger_key,
            jc.subject_post_id,
            jc.jury_size,
            rk_status.`key` AS status_key
        FROM judicial_case jc
        INNER JOIN ref_key_values rk_status
            ON rk_status.id = jc.status_id
           AND rk_status.`group` = ?
           AND rk_status.is_active = 1
           AND rk_status.`key` IN (?, ?)
        INNER JOIN ref_key_values rk_trigger
            ON rk_trigger.id = jc.trigger_type_id
           AND rk_trigger.`group` = ?
           AND rk_trigger.is_active = 1
        WHERE jc.id = ?
        FOR UPDATE
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(ref_keys::KEY_JUDICIAL_CASE_OPEN)
    .bind(ref_keys::KEY_JUDICIAL_CASE_JURY_ACTIVE)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_TRIGGER)
    .bind(case_id)
    .fetch_optional(&mut **tx)
    .await
}

async fn void_disqualified_invites(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    case: &OpenCaseRow,
    system_actor: Option<u64>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE judicial_case_juror_invite jcji
        INNER JOIN social_entity se
            ON se.id = jcji.juror_social_entity_id
        INNER JOIN ref_key_values rk_void
            ON rk_void.`group` = ?
           AND rk_void.`key` = ?
           AND rk_void.is_active = 1
        INNER JOIN ref_key_values rk_cur
            ON rk_cur.id = jcji.invite_status_id
           AND rk_cur.`group` = ?
           AND rk_cur.`key` IN (?, ?)
           AND rk_cur.is_active = 1
        SET jcji.invite_status_id = rk_void.id,
            jcji.modified_by = ?
        WHERE jcji.judicial_case_id = ?
          AND (
              jcji.juror_social_entity_id IN (?, ?)
              OR (
                  ? IS NOT NULL
                  AND (
                      se.created_by = ?
                      OR EXISTS (
                          SELECT 1
                          FROM user_social_entity_role ur_filer
                          WHERE ur_filer.user_id = ?
                            AND ur_filer.social_entity_id = se.id
                            AND ur_filer.ends_at IS NULL
                      )
                  )
              )
          )
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
    .bind(ref_keys::KEY_JCJI_VOIDED)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
    .bind(ref_keys::KEY_JCJI_PENDING)
    .bind(ref_keys::KEY_JCJI_ACCEPTED)
    .bind(system_actor)
    .bind(case.id)
    .bind(case.originator_social_entity_id)
    .bind(case.target_social_entity_id)
    .bind(case.created_by)
    .bind(case.created_by)
    .bind(case.created_by)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Social entity ids that still **occupy a jury seat** (pending invite or accepted / voted).
///
/// `abstained`, `declined`, `expired`, and `voided` are excluded so
/// [`nominate_jurors_for_open_cases`] can refill seats after a juror vacates.
async fn occupied_juror_seat_entity_ids(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    case_id: u64,
) -> Result<Vec<u64>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT jcji.juror_social_entity_id
        FROM judicial_case_juror_invite jcji
        INNER JOIN ref_key_values rk
            ON rk.id = jcji.invite_status_id
           AND rk.`group` = ?
           AND rk.`key` IN (?, ?)
           AND rk.is_active = 1
        WHERE jcji.judicial_case_id = ?
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
    .bind(ref_keys::KEY_JCJI_PENDING)
    .bind(ref_keys::KEY_JCJI_ACCEPTED)
    .bind(case_id)
    .fetch_all(&mut **tx)
    .await
}

/// Every juror entity that already has an invite row on this case (any status).
///
/// Used so abstainers / decliners are never re-selected; the unique key
/// `(judicial_case_id, juror_social_entity_id)` would otherwise allow
/// `ON DUPLICATE KEY UPDATE` to resurrect a vacated seat for the same person.
async fn all_invited_juror_entity_ids_for_case(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    case_id: u64,
) -> Result<Vec<u64>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT jcji.juror_social_entity_id
        FROM judicial_case_juror_invite jcji
        WHERE jcji.judicial_case_id = ?
        "#,
    )
    .bind(case_id)
    .fetch_all(&mut **tx)
    .await
}

async fn activate_jury(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    case_id: u64,
    system_actor: Option<u64>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE judicial_case jc
        INNER JOIN ref_key_values rk
            ON rk.`group` = ?
           AND rk.`key` = ?
           AND rk.is_active = 1
        SET jc.status_id = rk.id,
            jc.modified_by = ?
        WHERE jc.id = ?
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(ref_keys::KEY_JUDICIAL_CASE_JURY_ACTIVE)
    .bind(system_actor)
    .bind(case_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

