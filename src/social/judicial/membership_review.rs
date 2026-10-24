//! Membership-review applications: judicial filing, jury pool tiers, verdict settlement.

use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use sqlx::{MySql, MySqlPool, Transaction};

use crate::error::{AppError, AppResult};
use crate::i18n;
use crate::ref_key_values;
use crate::ref_keys;
use crate::social::community_invite;
use crate::social::connections;
use crate::social::judicial;
use crate::social::judicial::types::{
    MembershipReviewApplicantOutcome, OpenCaseRow, VerdictOutcome,
};
use crate::social::scope;

pub fn juror_min_rating_primary() -> Decimal {
    Decimal::new(300, 2)
}

pub fn juror_min_rating_fallback() -> Decimal {
    Decimal::new(250, 2)
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MembershipReviewApplicationRow {
    pub application_statement_plain: String,
    pub application_statement_html: String,
}

pub async fn fetch_application_for_case(
    pool: &MySqlPool,
    case_id: u64,
) -> Result<Option<MembershipReviewApplicationRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT application_statement_plain, application_statement_html
        FROM judicial_case_membership_review
        WHERE judicial_case_id = ?
        "#,
    )
    .bind(case_id)
    .fetch_optional(pool)
    .await
}

pub async fn open_application_case_for_applicant(
    pool: &MySqlPool,
    applicant_human_id: u64,
    community_entity_id: u64,
) -> Result<Option<u64>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT jc.id
        FROM judicial_case jc
        INNER JOIN ref_key_values rk_tr
            ON rk_tr.id = jc.trigger_type_id
           AND rk_tr.`group` = ?
           AND rk_tr.`key` = ?
        INNER JOIN ref_key_values rk_st
            ON rk_st.id = jc.status_id
           AND rk_st.`group` = ?
           AND rk_st.`key` IN (?, ?)
        WHERE jc.originator_social_entity_id = ?
          AND jc.target_social_entity_id = ?
        ORDER BY jc.opened_at DESC, jc.id DESC
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_TRIGGER)
    .bind(ref_keys::KEY_JUDICIAL_TRIGGER_MEMBERSHIP_REVIEW)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(ref_keys::KEY_JUDICIAL_CASE_OPEN)
    .bind(ref_keys::KEY_JUDICIAL_CASE_JURY_ACTIVE)
    .bind(applicant_human_id)
    .bind(community_entity_id)
    .fetch_optional(pool)
    .await
}

pub async fn is_reapply_barred(
    pool: &MySqlPool,
    applicant_human_id: u64,
    community_entity_id: u64,
) -> Result<bool, sqlx::Error> {
    let found: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT human_social_entity_id
        FROM community_membership_reapply_barrier
        WHERE human_social_entity_id = ?
          AND community_social_entity_id = ?
          AND barred_until > UTC_TIMESTAMP(6)
        LIMIT 1
        "#,
    )
    .bind(applicant_human_id)
    .bind(community_entity_id)
    .fetch_optional(pool)
    .await?;
    Ok(found.is_some())
}

pub async fn file_application_tx(
    tx: &mut Transaction<'_, MySql>,
    applicant_human_id: u64,
    community_entity_id: u64,
    acting_user_id: u64,
    statement_plain: &str,
    statement_markup: &str,
    statement_html: &str,
    jury_size_frozen: u8,
) -> Result<u64, sqlx::Error> {
    let trigger_id = ref_key_values::lookup_active_id_conn(
        &mut **tx,
        ref_keys::GROUP_JUDICIAL_CASE_TRIGGER,
        ref_keys::KEY_JUDICIAL_TRIGGER_MEMBERSHIP_REVIEW,
    )
    .await?
    .ok_or_else(|| {
        sqlx::Error::Configuration("membership_review trigger ref missing".into())
    })?;
    let status_open_id = ref_key_values::lookup_active_id_conn(
        &mut **tx,
        ref_keys::GROUP_JUDICIAL_CASE_STATUS,
        ref_keys::KEY_JUDICIAL_CASE_OPEN,
    )
    .await?
    .ok_or_else(|| {
        sqlx::Error::Configuration("judicial_case_status.open ref missing".into())
    })?;
    let type_originator_id = ref_key_values::lookup_active_id_conn(
        &mut **tx,
        ref_keys::GROUP_JUDICIAL_CASE_COMMENT_TYPE,
        ref_keys::KEY_JCC_ORIGINATOR,
    )
    .await?
    .ok_or_else(|| {
        sqlx::Error::Configuration("judicial_case_comment_type.originator ref missing".into())
    })?;

    let originator_rating: Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(se.rating, 0) FROM social_entity se WHERE se.id = ? LIMIT 1"#,
    )
    .bind(applicant_human_id)
    .fetch_one(&mut **tx)
    .await?;

    let res = sqlx::query(
        r#"
        INSERT INTO judicial_case (
            originator_social_entity_id,
            target_social_entity_id,
            trigger_type_id,
            status_id,
            jury_size,
            created_by,
            modified_by
        )
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(applicant_human_id)
    .bind(community_entity_id)
    .bind(trigger_id)
    .bind(status_open_id)
    .bind(jury_size_frozen)
    .bind(acting_user_id)
    .bind(acting_user_id)
    .execute(&mut **tx)
    .await?;
    let case_id = res.last_insert_id();

    sqlx::query(
        r#"
        INSERT INTO judicial_case_membership_review (
            judicial_case_id, application_statement_plain, application_statement_html
        )
        VALUES (?, ?, ?)
        "#,
    )
    .bind(case_id)
    .bind(statement_plain)
    .bind(statement_html)
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO judicial_case_comment (
            judicial_case_id, type_id, author_user_id, author_social_entity_id,
            author_entity_rating_at_create, body_plain, body_markup, body_html_sanitized,
            created_by, modified_by
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(case_id)
    .bind(type_originator_id)
    .bind(acting_user_id)
    .bind(applicant_human_id)
    .bind(originator_rating)
    .bind(statement_plain)
    .bind(statement_markup)
    .bind(statement_html)
    .bind(acting_user_id)
    .bind(acting_user_id)
    .execute(&mut **tx)
    .await?;

    Ok(case_id)
}

/// File a membership-review judicial case (free; no pending `member_of` edge).
pub async fn file_application(
    pool: &MySqlPool,
    acting_user_id: u64,
    applicant_human_id: u64,
    community_entity_id: u64,
    statement_plain: &str,
    statement_raw_html: &str,
    statement_html: &str,
) -> AppResult<u64> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();

    if statement_plain.trim().is_empty() {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.membership_review_statement_required"),
        ));
    }

    if community_invite::human_has_active_edge_to_community(
        pool,
        applicant_human_id,
        community_entity_id,
    )
    .await
    .map_err(AppError::Db)?
    {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.community_invite_already_member"),
        ));
    }

    if is_reapply_barred(pool, applicant_human_id, community_entity_id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.membership_review_reapply_barred"),
        ));
    }

    if open_application_case_for_applicant(pool, applicant_human_id, community_entity_id)
        .await
        .map_err(AppError::Db)?
        .is_some()
    {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.membership_review_already_open"),
        ));
    }

    if !connections::human_can_accept_new_peer(pool, applicant_human_id, community_entity_id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.community_invite_target_full"),
        ));
    }

    let markup_trimmed = statement_raw_html.trim();
    let statement_markup = if markup_trimmed.is_empty() {
        format!(
            "<p>{}</p>",
            crate::html_escape::escape_html(statement_plain.trim())
        )
    } else {
        markup_trimmed.to_string()
    };

    let mut tx = pool.begin().await.map_err(AppError::Db)?;
    let jury_size_frozen = judicial::compute_jury_size_to_freeze_at_filing_tx(
        &mut tx,
        applicant_human_id,
        community_entity_id,
        Some(acting_user_id),
        None,
        ref_keys::KEY_JUDICIAL_TRIGGER_MEMBERSHIP_REVIEW,
    )
    .await
    .map_err(AppError::Db)?;

    let case_id = file_application_tx(
        &mut tx,
        applicant_human_id,
        community_entity_id,
        acting_user_id,
        statement_plain,
        &statement_markup,
        statement_html,
        jury_size_frozen,
    )
    .await
    .map_err(AppError::Db)?;

    tx.commit().await.map_err(AppError::Db)?;
    Ok(case_id)
}

/// Void the applicant's open membership-review case (withdraw application).
pub async fn cancel_application(
    pool: &MySqlPool,
    acting_user_id: u64,
    applicant_human_id: u64,
    community_entity_id: u64,
) -> AppResult<()> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let Some(case_id) =
        open_application_case_for_applicant(pool, applicant_human_id, community_entity_id)
            .await
            .map_err(AppError::Db)?
    else {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.membership_review_cancel_unavailable"),
        ));
    };

    let mut tx = pool.begin().await.map_err(AppError::Db)?;
    void_open_application_tx(&mut tx, case_id, acting_user_id)
        .await
        .map_err(AppError::Db)?;
    tx.commit().await.map_err(AppError::Db)?;
    Ok(())
}

pub async fn void_open_application_tx(
    tx: &mut Transaction<'_, MySql>,
    case_id: u64,
    acting_user_id: u64,
) -> Result<(), sqlx::Error> {
    let void_status_id = ref_key_values::lookup_active_id_conn(
        &mut **tx,
        ref_keys::GROUP_JUDICIAL_CASE_STATUS,
        ref_keys::KEY_JUDICIAL_CASE_VOIDED,
    )
    .await?
    .ok_or_else(|| sqlx::Error::Configuration("judicial_case_status.voided ref missing".into()))?;

    sqlx::query(
        r#"
        UPDATE judicial_case jc
        SET jc.status_id = ?,
            jc.closed_at = UTC_TIMESTAMP(),
            jc.modified_by = ?
        WHERE jc.id = ?
        "#,
    )
    .bind(void_status_id)
    .bind(acting_user_id)
    .bind(case_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn apply_verdict_settlement_tx(
    tx: &mut Transaction<'_, MySql>,
    case_id: u64,
    case_row: &OpenCaseRow,
    verdict: VerdictOutcome,
    system_actor: Option<u64>,
) -> Result<(), sqlx::Error> {
    let applicant_id = case_row.originator_social_entity_id;
    let community_id = case_row.target_social_entity_id;
    let actor = system_actor.or(case_row.created_by).unwrap_or(0);
    if actor == 0 {
        return Ok(());
    }

    match verdict {
        VerdictOutcome::Upheld => {
            let member_of_type_id = ref_key_values::lookup_active_id_conn(
                &mut **tx,
                ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE,
                ref_keys::KEY_RELATIONSHIP_MEMBER_OF,
            )
            .await?
            .ok_or_else(|| {
                sqlx::Error::Configuration("member_of relationship type ref missing".into())
            })?;
            let status_verified_id = ref_key_values::lookup_active_id_conn(
                &mut **tx,
                ref_keys::GROUP_STATUS,
                ref_keys::KEY_VERIFIED,
            )
            .await?
            .ok_or_else(|| sqlx::Error::Configuration("verified status ref missing".into()))?;

            let existing: Option<u64> = sqlx::query_scalar(
                r#"
                SELECT ser.id
                FROM social_entity_relationship ser
                WHERE ser.from_social_entity_id = ?
                  AND ser.to_social_entity_id = ?
                  AND ser.ended_at IS NULL
                LIMIT 1
                "#,
            )
            .bind(applicant_id)
            .bind(community_id)
            .fetch_optional(&mut **tx)
            .await?;

            if existing.is_none() {
                sqlx::query(
                    r#"
                    INSERT INTO social_entity_relationship (
                        from_social_entity_id, to_social_entity_id, type_id, status_id,
                        relationship_status_id, rating, started_at, ended_at, created_by, modified_by
                    )
                    VALUES (
                        ?, ?, ?, ?,
                        (
                            SELECT scoped.id
                            FROM ref_key_values old_status
                            INNER JOIN ref_key_values scoped
                                ON scoped.`group` = 'social_relationship_status'
                               AND scoped.`key` = old_status.`key`
                            WHERE old_status.id = ?
                            LIMIT 1
                        ),
                        2.50, UTC_DATE(), NULL, ?, ?
                    )
                    "#,
                )
                .bind(applicant_id)
                .bind(community_id)
                .bind(member_of_type_id)
                .bind(status_verified_id)
                .bind(status_verified_id)
                .bind(actor)
                .bind(actor)
                .execute(&mut **tx)
                .await?;
                scope::mark_relationship_endpoints_dirty_in_tx(tx, applicant_id, community_id)
                    .await?;
            }

            sqlx::query(
                r#"
                DELETE FROM community_membership_reapply_barrier
                WHERE human_social_entity_id = ?
                  AND community_social_entity_id = ?
                "#,
            )
            .bind(applicant_id)
            .bind(community_id)
            .execute(&mut **tx)
            .await?;
        }
        VerdictOutcome::Overturned | VerdictOutcome::Hung => {
            let barred_until = Utc::now() + Duration::days(365);
            sqlx::query(
                r#"
                INSERT INTO community_membership_reapply_barrier (
                    human_social_entity_id, community_social_entity_id,
                    barred_until, source_judicial_case_id
                )
                VALUES (?, ?, ?, ?)
                ON DUPLICATE KEY UPDATE
                    barred_until = VALUES(barred_until),
                    source_judicial_case_id = VALUES(source_judicial_case_id),
                    updated_at = UTC_TIMESTAMP(6)
                "#,
            )
            .bind(applicant_id)
            .bind(community_id)
            .bind(barred_until.naive_utc())
            .bind(case_id)
            .execute(&mut **tx)
            .await?;
        }
    }
    Ok(())
}

/// After a 21-day `no_jury` close: applicant is not admitted and may reapply (no barrier).
pub(crate) async fn apply_no_jury_settlement_tx(
    _tx: &mut Transaction<'_, MySql>,
    _case_id: u64,
    _case_row: &OpenCaseRow,
) -> Result<(), sqlx::Error> {
    Ok(())
}

#[derive(Debug, Clone)]
pub struct MembershipReviewApplicantOutcomeFeed {
    pub case_id: u64,
    pub community_handle: String,
    pub community_label: String,
    pub outcome: MembershipReviewApplicantOutcome,
}

/// Most recent closed membership-review application for this applicant → community (no open case).
pub async fn applicant_outcome_for_community(
    pool: &MySqlPool,
    applicant_human_id: u64,
    community_entity_id: u64,
) -> Result<Option<MembershipReviewApplicantOutcome>, sqlx::Error> {
    let row = latest_closed_application_row(pool, applicant_human_id, community_entity_id).await?;
    Ok(row.map(|r| r.outcome))
}

/// Most recent closed application across all communities (for home feed card).
pub async fn latest_applicant_outcome_for_viewer(
    pool: &MySqlPool,
    viewer_user_id: u64,
) -> Result<Option<MembershipReviewApplicantOutcomeFeed>, sqlx::Error> {
    let applicant_human_id: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT se.id
        FROM social_entity se
        INNER JOIN social_human sh ON sh.social_entity_id = se.id
        WHERE se.created_by = ?
        ORDER BY se.id ASC
        LIMIT 1
        "#,
    )
    .bind(viewer_user_id)
    .fetch_optional(pool)
    .await?;

    let Some(applicant_human_id) = applicant_human_id else {
        return Ok(None);
    };

    let row: Option<(u64, String, String, String)> = sqlx::query_as(
        r#"
        SELECT jc.id, rk_st.`key`, se.handle,
               COALESCE(NULLIF(TRIM(sc.label), ''), se.handle)
        FROM judicial_case jc
        INNER JOIN ref_key_values rk_tr
            ON rk_tr.id = jc.trigger_type_id
           AND rk_tr.`group` = ?
           AND rk_tr.`key` = ?
        INNER JOIN ref_key_values rk_st
            ON rk_st.id = jc.status_id
           AND rk_st.`group` = ?
           AND rk_st.`key` IN (?, ?)
        INNER JOIN social_entity se ON se.id = jc.target_social_entity_id
        LEFT JOIN social_community sc ON sc.social_entity_id = se.id
        WHERE jc.originator_social_entity_id = ?
          AND NOT EXISTS (
              SELECT 1
              FROM judicial_case jc_open
              INNER JOIN ref_key_values rk_open
                  ON rk_open.id = jc_open.status_id
                 AND rk_open.`group` = ?
                 AND rk_open.`key` IN (?, ?)
              WHERE jc_open.originator_social_entity_id = jc.originator_social_entity_id
                AND jc_open.target_social_entity_id = jc.target_social_entity_id
                AND jc_open.trigger_type_id = jc.trigger_type_id
          )
        ORDER BY jc.closed_at DESC, jc.id DESC
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_TRIGGER)
    .bind(ref_keys::KEY_JUDICIAL_TRIGGER_MEMBERSHIP_REVIEW)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(ref_keys::KEY_JUDICIAL_CASE_RESOLVED)
    .bind(ref_keys::KEY_JUDICIAL_CASE_NO_JURY)
    .bind(applicant_human_id)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(ref_keys::KEY_JUDICIAL_CASE_OPEN)
    .bind(ref_keys::KEY_JUDICIAL_CASE_JURY_ACTIVE)
    .fetch_optional(pool)
    .await?;

    let Some((case_id, status_key, handle, label)) = row else {
        return Ok(None);
    };

    let community_entity_id: u64 = sqlx::query_scalar(
        r#"SELECT target_social_entity_id FROM judicial_case WHERE id = ? LIMIT 1"#,
    )
    .bind(case_id)
    .fetch_one(pool)
    .await?;

    let outcome = outcome_from_closed_case(pool, applicant_human_id, community_entity_id, &status_key)
        .await?;
    let Some(outcome) = outcome else {
        return Ok(None);
    };

    Ok(Some(MembershipReviewApplicantOutcomeFeed {
        case_id,
        community_handle: handle,
        community_label: label,
        outcome,
    }))
}

struct ClosedApplicationRow {
    outcome: MembershipReviewApplicantOutcome,
}

async fn latest_closed_application_row(
    pool: &MySqlPool,
    applicant_human_id: u64,
    community_entity_id: u64,
) -> Result<Option<ClosedApplicationRow>, sqlx::Error> {
    let status_key: Option<String> = sqlx::query_scalar(
        r#"
        SELECT rk_st.`key`
        FROM judicial_case jc
        INNER JOIN ref_key_values rk_tr
            ON rk_tr.id = jc.trigger_type_id
           AND rk_tr.`group` = ?
           AND rk_tr.`key` = ?
        INNER JOIN ref_key_values rk_st
            ON rk_st.id = jc.status_id
           AND rk_st.`group` = ?
           AND rk_st.`key` IN (?, ?)
        WHERE jc.originator_social_entity_id = ?
          AND jc.target_social_entity_id = ?
          AND NOT EXISTS (
              SELECT 1
              FROM judicial_case jc_open
              INNER JOIN ref_key_values rk_open
                  ON rk_open.id = jc_open.status_id
                 AND rk_open.`group` = ?
                 AND rk_open.`key` IN (?, ?)
              INNER JOIN ref_key_values rk_tr2
                  ON rk_tr2.id = jc_open.trigger_type_id
                 AND rk_tr2.`group` = ?
                 AND rk_tr2.`key` = ?
              WHERE jc_open.originator_social_entity_id = ?
                AND jc_open.target_social_entity_id = ?
          )
        ORDER BY jc.closed_at DESC, jc.id DESC
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_TRIGGER)
    .bind(ref_keys::KEY_JUDICIAL_TRIGGER_MEMBERSHIP_REVIEW)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(ref_keys::KEY_JUDICIAL_CASE_RESOLVED)
    .bind(ref_keys::KEY_JUDICIAL_CASE_NO_JURY)
    .bind(applicant_human_id)
    .bind(community_entity_id)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(ref_keys::KEY_JUDICIAL_CASE_OPEN)
    .bind(ref_keys::KEY_JUDICIAL_CASE_JURY_ACTIVE)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_TRIGGER)
    .bind(ref_keys::KEY_JUDICIAL_TRIGGER_MEMBERSHIP_REVIEW)
    .bind(applicant_human_id)
    .bind(community_entity_id)
    .fetch_optional(pool)
    .await?;

    let Some(status_key) = status_key else {
        return Ok(None);
    };

    let outcome =
        outcome_from_closed_case(pool, applicant_human_id, community_entity_id, &status_key)
            .await?;
    Ok(outcome.map(|outcome| ClosedApplicationRow { outcome }))
}

async fn outcome_from_closed_case(
    pool: &MySqlPool,
    applicant_human_id: u64,
    community_entity_id: u64,
    status_key: &str,
) -> Result<Option<MembershipReviewApplicantOutcome>, sqlx::Error> {
    if status_key == ref_keys::KEY_JUDICIAL_CASE_NO_JURY {
        return Ok(Some(MembershipReviewApplicantOutcome::NoJury));
    }
    if status_key != ref_keys::KEY_JUDICIAL_CASE_RESOLVED {
        return Ok(None);
    }

    let is_member: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT ser.id
        FROM social_entity_relationship ser
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = ser.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` = ?
        INNER JOIN ref_key_values rk_status
            ON rk_status.id = ser.status_id
           AND rk_status.`group` = ?
           AND rk_status.`key` IN (?, ?, ?)
        WHERE ser.from_social_entity_id = ?
          AND ser.to_social_entity_id = ?
          AND ser.ended_at IS NULL
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_VERIFIED)
    .bind(ref_keys::KEY_FLAGGED)
    .bind(applicant_human_id)
    .bind(community_entity_id)
    .fetch_optional(pool)
    .await?;

    if is_member.is_some() {
        return Ok(None);
    }

    Ok(Some(MembershipReviewApplicantOutcome::Denied))
}
