use rust_decimal::Decimal;
use sqlx::{MySql, MySqlPool};

use crate::ref_key_values;
use crate::ref_keys;
use crate::social::audit;

use super::case_access::{community_context_for_case_tx, fetch_open_case_row_tx};
use super::community_reparent;
use super::emergency::{platform_safety_ban_human_tx, restore_post_thread_tx};
use super::membership_review;
use super::triggers::{trigger_is_civic_dispute, trigger_is_emergency_safety, trigger_is_post_dispute_like};
use super::rating::{
    apply_community_feed_moderation_rating_feedback_on_resolved_case_tx,
    apply_community_jury_resolved_bonus_tx,
    apply_jury_civic_rating_settlement_tx,
};
use super::types::{AuthorWithdrawPostUnderReviewOutcome, VerdictOutcome};
use super::verdict_eval;
async fn apply_post_moderation_subject_status_after_verdict_tx(
    tx: &mut sqlx::Transaction<'_, MySql>,
    case_id: u64,
    verdict: VerdictOutcome,
) -> Result<(), sqlx::Error> {
    let system_actor: Option<u64> = sqlx::query_scalar(
        r#"SELECT id FROM users WHERE email = ? LIMIT 1"#,
    )
    .bind(ref_keys::SYSTEM_USER_EMAIL)
    .fetch_optional(&mut **tx)
    .await?;

    let row: Option<(Option<u64>, String)> = sqlx::query_as(
        r#"
        SELECT jc.subject_post_id, rk_tr.`key`
        FROM judicial_case jc
        INNER JOIN ref_key_values rk_tr
            ON rk_tr.id = jc.trigger_type_id
           AND rk_tr.`group` = ?
           AND rk_tr.is_active = 1
        WHERE jc.id = ?
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_TRIGGER)
    .bind(case_id)
    .fetch_optional(&mut **tx)
    .await?;

    let Some((Some(subject_post_id), trigger_key)) = row else {
        return Ok(());
    };
    if !trigger_is_post_dispute_like(&trigger_key) {
        return Ok(());
    }

    let root_id: u64 = sqlx::query_scalar(
        r#"SELECT COALESCE(p.root_post_id, p.id) FROM post p WHERE p.id = ? LIMIT 1"#,
    )
    .bind(subject_post_id)
    .fetch_one(&mut **tx)
    .await?;

    let status_key = match verdict {
        VerdictOutcome::Overturned | VerdictOutcome::Hung => ref_keys::KEY_VERIFIED,
        VerdictOutcome::Upheld => ref_keys::KEY_FLAGGED,
    };
    let Some(new_status_id): Option<u64> = sqlx::query_scalar(
        r#"
        SELECT id
        FROM ref_key_values
        WHERE `group` = ? AND `key` = ? AND is_active = 1
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_STATUS)
    .bind(status_key)
    .fetch_optional(&mut **tx)
    .await?
    else {
        tracing::warn!(
            case_id,
            status_key,
            "ref_key_values missing post lifecycle status; skipping post status update after verdict"
        );
        return Ok(());
    };

    // Human jury **upheld** moves the thread to `flagged`; council **overturn** on appeal must
    // restore from either `under_judicial_review` or `flagged`. Jury **clears** (overturn) and
    // council **uphold** on appeal both target `under_judicial_review` only.
    let from_status_keys: &[&str] = match (verdict, trigger_key.as_str()) {
        (VerdictOutcome::Overturned | VerdictOutcome::Hung, ref_keys::KEY_JUDICIAL_TRIGGER_CASE_APPEAL) => {
            &[ref_keys::KEY_POST_UNDER_JUDICIAL_REVIEW, ref_keys::KEY_FLAGGED][..]
        }
        (VerdictOutcome::Overturned | VerdictOutcome::Hung, _) => &[ref_keys::KEY_POST_UNDER_JUDICIAL_REVIEW],
        (VerdictOutcome::Upheld, ref_keys::KEY_JUDICIAL_TRIGGER_CASE_APPEAL) => {
            &[ref_keys::KEY_POST_UNDER_JUDICIAL_REVIEW]
        }
        (VerdictOutcome::Upheld, _) => &[ref_keys::KEY_POST_UNDER_JUDICIAL_REVIEW],
    };

    let mut total_updated = 0u64;
    for from_key in from_status_keys {
        let res = sqlx::query(
            r#"
            UPDATE post p
            INNER JOIN ref_key_values rk_cur
                ON rk_cur.id = p.status_id
               AND rk_cur.`group` = ?
               AND rk_cur.is_active = 1
            SET p.status_id = ?,
                p.modified_by = ?
            WHERE (p.id = ? OR p.root_post_id = ?)
              AND rk_cur.`key` = ?
            "#,
        )
        .bind(ref_keys::GROUP_STATUS)
        .bind(new_status_id)
        .bind(system_actor)
        .bind(root_id)
        .bind(root_id)
        .bind(from_key)
        .execute(&mut **tx)
        .await?;
        total_updated += res.rows_affected();
    }

    if total_updated == 0 {
        tracing::warn!(
            case_id,
            root_id,
            ?from_status_keys,
            status_key,
            "post-moderation verdict did not update any post rows"
        );
    }
    Ok(())
}

/// Marks the subject post thread `under_judicial_review` (aggregate feed hides from non-authors).
pub async fn set_subject_post_under_judicial_review_tx(
    tx: &mut sqlx::Transaction<'_, MySql>,
    root_post_id: u64,
    actor: u64,
) -> Result<(), sqlx::Error> {
    let Some(status_id) = ref_key_values::lookup_active_id_conn(
        &mut **tx,
        ref_keys::GROUP_STATUS,
        ref_keys::KEY_POST_UNDER_JUDICIAL_REVIEW,
    )
    .await?
    else {
        return Ok(());
    };
    sqlx::query(
        r#"
        UPDATE post
        SET status_id = ?, modified_by = ?
        WHERE id = ? OR root_post_id = ?
        "#,
    )
    .bind(status_id)
    .bind(actor)
    .bind(root_post_id)
    .bind(root_post_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Post dispute `no_jury`: community is penalized elsewhere; thread stays `flagged` (hidden).
pub async fn settle_post_moderation_subject_after_no_jury_tx(
    tx: &mut sqlx::Transaction<'_, MySql>,
    case_id: u64,
    actor: Option<u64>,
) -> Result<(), sqlx::Error> {
    let Some(flagged_status_id) = ref_key_values::lookup_active_id_conn(
        &mut **tx,
        ref_keys::GROUP_STATUS,
        ref_keys::KEY_FLAGGED,
    )
    .await?
    else {
        return Ok(());
    };

    let root_id: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT COALESCE(p.root_post_id, p.id)
        FROM judicial_case jc
        INNER JOIN post p ON p.id = jc.subject_post_id
        WHERE jc.id = ?
        LIMIT 1
        "#,
    )
    .bind(case_id)
    .fetch_optional(&mut **tx)
    .await?;

    let Some(root_id) = root_id else {
        return Ok(());
    };

    let _ = sqlx::query(
        r#"
        UPDATE post p
        INNER JOIN ref_key_values rk_cur
            ON rk_cur.id = p.status_id
           AND rk_cur.`group` = ?
           AND rk_cur.is_active = 1
           AND rk_cur.`key` = ?
        SET p.status_id = ?,
            p.modified_by = ?
        WHERE p.id = ? OR p.root_post_id = ?
        "#,
    )
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_POST_UNDER_JUDICIAL_REVIEW)
    .bind(flagged_status_id)
    .bind(actor)
    .bind(root_id)
    .bind(root_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Back-compat alias.
pub async fn restore_post_moderation_subject_after_no_jury_tx(
    tx: &mut sqlx::Transaction<'_, MySql>,
    case_id: u64,
    actor: Option<u64>,
) -> Result<(), sqlx::Error> {
    settle_post_moderation_subject_after_no_jury_tx(tx, case_id, actor).await
}

/// Check whether a case is ready for verdict and resolve it if so.
///
/// A case is ready when it has no `pending` invites remaining and at least one
/// juror has voted. **Seat fill is not required** for resolution: abstentions and
/// unfilled seats are allowed; the outcome is decided only by **weighted** votes
/// (`rating / 2.5` → 0–2 weight per juror). Upheld when
/// `normalized_for >= normalized_total * 2/3` (constitutional cases also allow a
/// hung band when neither side clears that bar).
///
/// **Transactions:** always opens its **own** transaction on `pool` (independent of
/// [`cast_vote`] / [`abstain_from_invite`]). Idempotent when the case is already `resolved`
/// or still has pending invites.
pub async fn check_and_resolve_verdict(
    pool: &MySqlPool,
    case_id: u64,
) -> Result<Option<VerdictOutcome>, sqlx::Error> {
    let system_actor = audit::system_user_id(pool).await?;
    let mut tx = pool.begin().await?;

    let already_resolved: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM judicial_case jc
            INNER JOIN ref_key_values rk ON rk.id = jc.status_id
                AND rk.`group` = 'judicial_case_status'
                AND rk.`key` = 'resolved'
            WHERE jc.id = ?
        )
        "#,
    )
    .bind(case_id)
    .fetch_one(&mut *tx)
    .await?;

    if already_resolved {
        tx.rollback().await?;
        return Ok(None);
    }

    let pending_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM judicial_case_juror_invite jcji
        INNER JOIN ref_key_values rk
            ON rk.id = jcji.invite_status_id
           AND rk.`group` = ?
           AND rk.`key` = ?
           AND rk.is_active = 1
        WHERE jcji.judicial_case_id = ?
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
    .bind(ref_keys::KEY_JCJI_PENDING)
    .bind(case_id)
    .fetch_one(&mut *tx)
    .await?;

    if pending_count > 0 {
        tx.rollback().await?;
        return Ok(None);
    }

    let votes: Vec<(String, Decimal)> = sqlx::query_as(
        r#"
        SELECT rk_vote.`key`, jcji.juror_rating_at_invite
        FROM judicial_case_juror_invite jcji
        INNER JOIN ref_key_values rk_inv
            ON rk_inv.id = jcji.invite_status_id
           AND rk_inv.`group` = ?
           AND rk_inv.`key` = ?
           AND rk_inv.is_active = 1
        INNER JOIN ref_key_values rk_vote
            ON rk_vote.id = jcji.vote_direction_id
           AND rk_vote.`group` = ?
           AND rk_vote.is_active = 1
        WHERE jcji.judicial_case_id = ?
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
    .bind(ref_keys::KEY_JCJI_ACCEPTED)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_VOTE_DIRECTION)
    .bind(case_id)
    .fetch_all(&mut *tx)
    .await?;

    if votes.is_empty() {
        tx.rollback().await?;
        return Ok(None);
    }

    let trigger_key: String = sqlx::query_scalar(
        r#"
        SELECT rk_tr.`key`
        FROM judicial_case jc
        INNER JOIN ref_key_values rk_tr
            ON rk_tr.id = jc.trigger_type_id
           AND rk_tr.`group` = ?
           AND rk_tr.is_active = 1
        WHERE jc.id = ?
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_TRIGGER)
    .bind(case_id)
    .fetch_one(&mut *tx)
    .await?;

    let (normalized_for, normalized_against) = verdict_eval::normalized_vote_totals(&votes);

    let weighted = verdict_eval::evaluate_weighted_verdict(
        &trigger_key,
        normalized_for,
        normalized_against,
    );
    let verdict = weighted.outcome;
    let hung_band = weighted.hung_band;

    let case_row_for_settlement = fetch_open_case_row_tx(&mut tx, case_id).await?;
    let mut close_case = true;
    if trigger_key == ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_REPARENT {
        if let Some(case_row) = fetch_open_case_row_tx(&mut tx, case_id).await? {
            close_case = community_reparent::on_verdict_before_close_tx(
                &mut tx,
                case_id,
                &case_row,
                verdict,
                system_actor,
            )
            .await?;
        }
    }

    if close_case {
        sqlx::query(
            r#"
            UPDATE judicial_case jc
            INNER JOIN ref_key_values rk
                ON rk.`group` = ?
               AND rk.`key` = ?
               AND rk.is_active = 1
            SET jc.status_id = rk.id,
                jc.closed_at = UTC_TIMESTAMP(),
                jc.modified_by = ?
            WHERE jc.id = ?
            "#,
        )
        .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
        .bind(ref_keys::KEY_JUDICIAL_CASE_RESOLVED)
        .bind(system_actor)
        .bind(case_id)
        .execute(&mut *tx)
        .await?;
    }

    if trigger_key == ref_keys::KEY_JUDICIAL_TRIGGER_CONSTITUTIONAL_AMENDMENT {
        if verdict == VerdictOutcome::Upheld {
            let system_actor = audit::system_user_id(pool).await?;
            if let Some(case_row) = case_row_for_settlement.as_ref() {
                if let Some(community_id) =
                    community_context_for_case_tx(&mut tx, case_row).await?
                {
                    crate::social::judicial_constitutional::apply_constitutional_amendment_on_upheld_tx(
                        &mut tx,
                        case_id,
                        community_id,
                        system_actor,
                    )
                    .await?;
                }
            }
        }
        crate::social::judicial_constitutional::apply_constitutional_verdict_rating_settlement_tx(
            &mut tx, case_id, verdict, hung_band,
        )
        .await?;
        if let Some(ref case_row) = case_row_for_settlement {
            apply_community_jury_resolved_bonus_tx(&mut tx, case_id, case_row).await?;
        }
    } else if trigger_key == ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_PROFILE_AMENDMENT {
        if verdict == VerdictOutcome::Upheld {
            let system_actor = audit::system_user_id(pool).await?;
            if let Some(case_row) = case_row_for_settlement.as_ref() {
                if let Some(community_id) =
                    community_context_for_case_tx(&mut tx, case_row).await?
                {
                    crate::social::judicial_community_profile::apply_profile_amendment_on_upheld_tx(
                        &mut tx,
                        case_id,
                        community_id,
                        system_actor,
                    )
                    .await?;
                }
            }
        }
        crate::social::judicial_community_profile::apply_profile_amendment_verdict_rating_settlement_tx(
            &mut tx, case_id, verdict, hung_band,
        )
        .await?;
        if let Some(ref case_row) = case_row_for_settlement {
            apply_community_jury_resolved_bonus_tx(&mut tx, case_id, case_row).await?;
        }
    } else if trigger_key == ref_keys::KEY_JUDICIAL_TRIGGER_MEMBERSHIP_REVIEW {
        if let Some(ref case_row) = case_row_for_settlement {
            membership_review::apply_verdict_settlement_tx(
                &mut tx,
                case_id,
                case_row,
                verdict,
                system_actor,
            )
            .await?;
            apply_jury_civic_rating_settlement_tx(
                &mut tx,
                case_id,
                case_row,
                verdict,
                hung_band,
            )
            .await?;
            apply_community_jury_resolved_bonus_tx(&mut tx, case_id, case_row).await?;
            apply_community_feed_moderation_rating_feedback_on_resolved_case_tx(&mut tx, case_id)
                .await?;
        }
    } else if trigger_key == ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_REPARENT && close_case {
        if let Some(ref case_row) = case_row_for_settlement {
            apply_jury_civic_rating_settlement_tx(
                &mut tx,
                case_id,
                case_row,
                verdict,
                hung_band,
            )
            .await?;
            apply_community_jury_resolved_bonus_tx(&mut tx, case_id, case_row).await?;
        }
    } else if trigger_is_emergency_safety(&trigger_key) {
        if verdict == VerdictOutcome::Overturned {
            let system_actor = audit::system_user_id(pool).await?;
            if let Some(root_id) = sqlx::query_scalar::<_, Option<u64>>(
                r#"
                SELECT COALESCE(p.root_post_id, p.id)
                FROM judicial_case jc
                INNER JOIN post p ON p.id = jc.subject_post_id
                WHERE jc.id = ?
                "#,
            )
            .bind(case_id)
            .fetch_optional(&mut *tx)
            .await?
            .flatten()
            {
                restore_post_thread_tx(&mut tx, root_id, system_actor.unwrap_or(0)).await?;
                super::acute_harm_content::apply_or_refund_acute_harm_content_rating_tx(
                    &mut tx,
                    case_id,
                    root_id,
                    super::acute_harm_content::AcuteHarmDirection::Refund,
                )
                .await?;
            }
        } else if verdict == VerdictOutcome::Upheld {
            if let Some(root_id) = sqlx::query_scalar::<_, Option<u64>>(
                r#"
                SELECT COALESCE(p.root_post_id, p.id)
                FROM judicial_case jc
                INNER JOIN post p ON p.id = jc.subject_post_id
                WHERE jc.id = ?
                "#,
            )
            .bind(case_id)
            .fetch_optional(&mut *tx)
            .await?
            .flatten()
            {
                super::acute_harm_content::apply_or_refund_acute_harm_content_rating_tx(
                    &mut tx,
                    case_id,
                    root_id,
                    super::acute_harm_content::AcuteHarmDirection::Apply,
                )
                .await?;
            }
        }

        if let Some(ref case_row) = case_row_for_settlement {
            apply_jury_civic_rating_settlement_tx(
                &mut tx,
                case_id,
                case_row,
                verdict,
                hung_band,
            )
            .await?;
            apply_community_jury_resolved_bonus_tx(&mut tx, case_id, case_row).await?;
        }
    } else {
        apply_post_moderation_subject_status_after_verdict_tx(&mut tx, case_id, verdict).await?;

        if let Some(ref case_row) = case_row_for_settlement {
            apply_jury_civic_rating_settlement_tx(
                &mut tx,
                case_id,
                case_row,
                verdict,
                hung_band,
            )
            .await?;
            apply_community_jury_resolved_bonus_tx(&mut tx, case_id, case_row).await?;
        }
        apply_community_feed_moderation_rating_feedback_on_resolved_case_tx(&mut tx, case_id)
            .await?;
    }

    tx.commit().await?;

    if trigger_is_emergency_safety(&trigger_key) && verdict == VerdictOutcome::Upheld {
        if let Some(ref case_row) = case_row_for_settlement {
            if let Err(err) = platform_safety_ban_human_tx(
                pool,
                case_row.target_social_entity_id,
                system_actor,
            )
            .await
            {
                tracing::warn!(
                    error = %err,
                    case_id,
                    human_entity_id = case_row.target_social_entity_id,
                    "emergency contest loss safety ban failed"
                );
            }
        }
    }

    if trigger_is_emergency_safety(&trigger_key) {
        if let Ok(config) = crate::config::Config::from_env() {
            crate::email::notify_emergency_contest_verdict_outcomes(
                pool,
                &config,
                case_id,
                verdict,
            )
            .await;
        }
    }

    if !close_case && trigger_key == ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_REPARENT {
        if let Err(err) = crate::jobs::enqueue_job(
            pool,
            None,
            "judicial_jury_nomination",
            &serde_json::json!({ "limit": 32 }),
            Some(&format!("judicial-case:{case_id}")),
            30,
        )
        .await
        {
            tracing::warn!(
                error = %err,
                case_id,
                "failed to enqueue judicial_jury_nomination after reparent phase-1 upheld"
            );
        }
    }

    Ok(Some(verdict))
}

/// Close an AI-escalated moderation appeal after the full council has voted (simple majority).
pub async fn resolve_council_appeal_verdict(
    pool: &MySqlPool,
    case_id: u64,
    verdict: VerdictOutcome,
) -> Result<(), sqlx::Error> {
    let system_actor = audit::system_user_id(pool).await?;
    let mut tx = pool.begin().await?;

    let already_resolved: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM judicial_case jc
            INNER JOIN ref_key_values rk ON rk.id = jc.status_id
                AND rk.`group` = ?
                AND rk.`key` = ?
            WHERE jc.id = ?
        )
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(ref_keys::KEY_JUDICIAL_CASE_RESOLVED)
    .bind(case_id)
    .fetch_one(&mut *tx)
    .await?;

    if already_resolved {
        tx.rollback().await?;
        return Ok(());
    }

    sqlx::query(
        r#"
        UPDATE judicial_case jc
        INNER JOIN ref_key_values rk
            ON rk.`group` = ?
           AND rk.`key` = ?
           AND rk.is_active = 1
        SET jc.status_id = rk.id,
            jc.closed_at = UTC_TIMESTAMP(),
            jc.modified_by = ?
        WHERE jc.id = ?
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(ref_keys::KEY_JUDICIAL_CASE_RESOLVED)
    .bind(system_actor)
    .bind(case_id)
    .execute(&mut *tx)
    .await?;

    apply_post_moderation_subject_status_after_verdict_tx(&mut tx, case_id, verdict).await?;

    tx.commit().await?;
    Ok(())
}

/// Root post must be `under_judicial_review`; case must be post dispute and not yet
/// `resolved` / `voided`. Deletes the root thread when every post in the tree is authored by
/// `viewer_user_id`, applies civic rating settlement, voids pending/accepted juror invites, and removes unreferenced `content_link` rows.
pub async fn author_withdraw_post_under_review(
    pool: &MySqlPool,
    viewer_user_id: u64,
    post_id_param: u64,
) -> Result<AuthorWithdrawPostUnderReviewOutcome, sqlx::Error> {
    let Some(root_id): Option<u64> = sqlx::query_scalar(
        r#"SELECT COALESCE(p.root_post_id, p.id) FROM post p WHERE p.id = ? LIMIT 1"#,
    )
    .bind(post_id_param)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(AuthorWithdrawPostUnderReviewOutcome::NotFound);
    };

    let mut tx = pool.begin().await?;

    let Some((post_author, post_status_key)): Option<(Option<u64>, Option<String>)> = sqlx::query_as(
        r#"
        SELECT p.created_by, rk.`key`
        FROM post p
        LEFT JOIN ref_key_values rk
            ON rk.id = p.status_id
           AND rk.`group` = ?
           AND rk.is_active = 1
        WHERE p.id = ?
        FOR UPDATE
        "#,
    )
    .bind(ref_keys::GROUP_STATUS)
    .bind(root_id)
    .fetch_optional(&mut *tx)
    .await?
    else {
        tx.rollback().await?;
        return Ok(AuthorWithdrawPostUnderReviewOutcome::NotFound);
    };

    if post_author != Some(viewer_user_id) {
        tx.rollback().await?;
        return Ok(AuthorWithdrawPostUnderReviewOutcome::Forbidden);
    }
    if post_status_key.as_deref() != Some(ref_keys::KEY_POST_UNDER_JUDICIAL_REVIEW) {
        tx.rollback().await?;
        return Ok(AuthorWithdrawPostUnderReviewOutcome::NotUnderReview);
    }

    let other_authors: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM post p2
        WHERE (p2.id = ? OR p2.root_post_id = ?)
          AND COALESCE(p2.created_by, 0) <> ?
        "#,
    )
    .bind(root_id)
    .bind(root_id)
    .bind(viewer_user_id)
    .fetch_one(&mut *tx)
    .await?;
    if other_authors > 0 {
        tx.rollback().await?;
        return Ok(AuthorWithdrawPostUnderReviewOutcome::ThreadHasOtherAuthors);
    }

    let case_row: Option<(u64, String, String)> = sqlx::query_as(
        r#"
        SELECT jc.id, rk_st.`key`, rt.`key`
        FROM judicial_case jc
        INNER JOIN ref_key_values rk_st
            ON rk_st.id = jc.status_id
           AND rk_st.`group` = ?
           AND rk_st.is_active = 1
        INNER JOIN ref_key_values rt
            ON rt.id = jc.trigger_type_id
           AND rt.`group` = ?
           AND rt.is_active = 1
        WHERE jc.subject_post_id = ?
        ORDER BY jc.id DESC
        LIMIT 1
        FOR UPDATE
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_TRIGGER)
    .bind(root_id)
    .fetch_optional(&mut *tx)
    .await?;

    let Some((case_id, case_status_key, trigger_key)) = case_row else {
        tx.rollback().await?;
        return Ok(AuthorWithdrawPostUnderReviewOutcome::NotFound);
    };
    if !trigger_is_civic_dispute(&trigger_key) {
        tx.rollback().await?;
        return Ok(AuthorWithdrawPostUnderReviewOutcome::WrongTrigger);
    }
    if matches!(
        case_status_key.as_str(),
        ref_keys::KEY_JUDICIAL_CASE_RESOLVED
            | ref_keys::KEY_JUDICIAL_CASE_VOIDED
            | ref_keys::KEY_JUDICIAL_CASE_NO_JURY
    ) {
        tx.rollback().await?;
        return Ok(AuthorWithdrawPostUnderReviewOutcome::CaseTerminal);
    }

    let void_status_id: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT id FROM ref_key_values
        WHERE `group` = ? AND `key` = ? AND is_active = 1
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(ref_keys::KEY_JUDICIAL_CASE_VOIDED)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(void_status_id) = void_status_id else {
        tx.rollback().await?;
        return Ok(AuthorWithdrawPostUnderReviewOutcome::NotFound);
    };

    sqlx::query(
        r#"
        UPDATE judicial_case
        SET status_id = ?,
            modified_by = ?
        WHERE id = ?
        "#,
    )
    .bind(void_status_id)
    .bind(viewer_user_id)
    .bind(case_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE judicial_case_juror_invite jcji
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
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
    .bind(ref_keys::KEY_JCJI_VOIDED)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
    .bind(ref_keys::KEY_JCJI_PENDING)
    .bind(ref_keys::KEY_JCJI_ACCEPTED)
    .bind(viewer_user_id)
    .bind(case_id)
    .execute(&mut *tx)
    .await?;

    let thread_posts: Vec<(u64, u64)> = sqlx::query_as(
        r#"SELECT id, content_link_id FROM post WHERE id = ? OR root_post_id = ?"#,
    )
    .bind(root_id)
    .bind(root_id)
    .fetch_all(&mut *tx)
    .await?;

    let mut link_ids: Vec<u64> = thread_posts.iter().map(|(_, lid)| *lid).collect();
    link_ids.sort_unstable();
    link_ids.dedup();

    sqlx::query(r#"DELETE FROM post WHERE root_post_id = ?"#)
        .bind(root_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(r#"DELETE FROM post WHERE id = ?"#)
        .bind(root_id)
        .execute(&mut *tx)
        .await?;

    for link_id in link_ids {
        let remaining: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM post WHERE content_link_id = ?"#,
        )
        .bind(link_id)
        .fetch_one(&mut *tx)
        .await?;
        if remaining == 0 {
            sqlx::query(r#"DELETE FROM content_link WHERE id = ?"#)
                .bind(link_id)
                .execute(&mut *tx)
                .await?;
        }
    }

    tx.commit().await?;
    Ok(AuthorWithdrawPostUnderReviewOutcome::ThreadDeleted)
}

