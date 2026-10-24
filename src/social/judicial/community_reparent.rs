//! Judicial community re-parent: two-phase human jury over moving community and subtree union.

use rust_decimal::Decimal;
use sqlx::{MySql, MySqlPool, Transaction};

use crate::ref_key_values;
use crate::ref_keys;
use crate::social::community_graph;
use crate::social::judicial;
use crate::social::judicial::types::{JurorCandidate, OpenCaseRow, VerdictOutcome};
use crate::social::judicial_constitutional;

pub fn juror_min_rating_primary() -> Decimal {
    judicial_constitutional::juror_min_rating_primary()
}

pub fn juror_min_rating_fallback() -> Decimal {
    judicial_constitutional::juror_min_rating_fallback()
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CommunityReparentRow {
    pub proposed_parent_community_id: u64,
    pub phase: String,
    pub impacted_pool_size_frozen: Option<u32>,
    pub impacted_jury_size_frozen: Option<u8>,
    pub constitution_diff_json: Option<String>,
}

pub async fn fetch_reparent_for_case(
    pool: &MySqlPool,
    case_id: u64,
) -> Result<Option<CommunityReparentRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT
            proposed_parent_community_id,
            phase,
            impacted_pool_size_frozen,
            impacted_jury_size_frozen,
            constitution_diff_json
        FROM judicial_case_community_reparent
        WHERE judicial_case_id = ?
        "#,
    )
    .bind(case_id)
    .fetch_optional(pool)
    .await
}

pub async fn fetch_reparent_for_case_conn(
    conn: &mut sqlx::MySqlConnection,
    case_id: u64,
) -> Result<Option<CommunityReparentRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT
            proposed_parent_community_id,
            phase,
            impacted_pool_size_frozen,
            impacted_jury_size_frozen,
            constitution_diff_json
        FROM judicial_case_community_reparent
        WHERE judicial_case_id = ?
        "#,
    )
    .bind(case_id)
    .fetch_optional(conn)
    .await
}

pub async fn fetch_reparent_for_case_tx(
    tx: &mut Transaction<'_, MySql>,
    case_id: u64,
) -> Result<Option<CommunityReparentRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT
            proposed_parent_community_id,
            phase,
            impacted_pool_size_frozen,
            impacted_jury_size_frozen,
            constitution_diff_json
        FROM judicial_case_community_reparent
        WHERE judicial_case_id = ?
        "#,
    )
    .bind(case_id)
    .fetch_optional(&mut **tx)
    .await
}

pub async fn count_origin_member_pool(
    pool: &MySqlPool,
    moving_community_id: u64,
) -> Result<usize, sqlx::Error> {
    let mut conn = pool.acquire().await?;
    count_origin_member_pool_conn(&mut *conn, moving_community_id).await
}

pub async fn count_origin_member_pool_conn(
    conn: &mut sqlx::MySqlConnection,
    moving_community_id: u64,
) -> Result<usize, sqlx::Error> {
    let n: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(DISTINCT ser.from_social_entity_id)
        FROM social_entity_relationship ser
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = ser.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` = ?
        INNER JOIN ref_key_values rk_status
            ON rk_status.id = ser.status_id
           AND rk_status.`group` = ?
           AND rk_status.`key` = ?
        INNER JOIN social_entity se ON se.id = ser.from_social_entity_id
        INNER JOIN social_human sh ON sh.social_entity_id = se.id
        WHERE ser.to_social_entity_id = ?
          AND ser.ended_at IS NULL
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_VERIFIED)
    .bind(moving_community_id)
    .fetch_one(conn)
    .await?;
    Ok(n.max(0) as usize)
}

pub async fn count_impacted_subtree_humans(
    pool: &MySqlPool,
    moving_community_id: u64,
) -> Result<usize, sqlx::Error> {
    let mut conn = pool.acquire().await?;
    count_impacted_subtree_humans_conn(&mut *conn, moving_community_id).await
}

pub async fn count_impacted_subtree_humans_conn(
    conn: &mut sqlx::MySqlConnection,
    moving_community_id: u64,
) -> Result<usize, sqlx::Error> {
    let n: i64 = sqlx::query_scalar(
        r#"
        WITH RECURSIVE subtree AS (
            SELECT social_entity_id AS id
            FROM social_community
            WHERE social_entity_id = ?
            UNION ALL
            SELECT sc.social_entity_id
            FROM social_community sc
            INNER JOIN subtree s ON sc.parent_community_id = s.id
        )
        SELECT COUNT(DISTINCT ser.from_social_entity_id)
        FROM subtree st
        INNER JOIN social_entity_relationship ser
            ON ser.to_social_entity_id = st.id
           AND ser.ended_at IS NULL
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = ser.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` = ?
        INNER JOIN ref_key_values rk_status
            ON rk_status.id = ser.status_id
           AND rk_status.`group` = ?
           AND rk_status.`key` = ?
        INNER JOIN social_entity se ON se.id = ser.from_social_entity_id
        INNER JOIN social_human sh ON sh.social_entity_id = se.id
        "#,
    )
    .bind(moving_community_id)
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_VERIFIED)
    .fetch_one(conn)
    .await?;
    Ok(n.max(0) as usize)
}

/// Distinct verified humans with `member_of` to any community in the moving subtree.
pub(crate) async fn impacted_subtree_juror_pool_conn(
    conn: &mut sqlx::MySqlConnection,
    moving_community_id: u64,
    originator_social_entity_id: u64,
    target_social_entity_id: u64,
    created_by: Option<u64>,
    rating_basis_community_id: u64,
) -> Result<Vec<JurorCandidate>, sqlx::Error> {
    let primary_min = juror_min_rating_primary();
    let fallback_min = juror_min_rating_fallback();
    let primary = impacted_subtree_juror_pool_with_min_conn(
        conn,
        moving_community_id,
        originator_social_entity_id,
        target_social_entity_id,
        created_by,
        primary_min,
        true,
        rating_basis_community_id,
    )
    .await?;
    let fallback = impacted_subtree_juror_pool_with_min_conn(
        conn,
        moving_community_id,
        originator_social_entity_id,
        target_social_entity_id,
        created_by,
        fallback_min,
        false,
        rating_basis_community_id,
    )
    .await?;
    Ok(if fallback.len() > primary.len() {
        fallback
    } else {
        primary
    })
}

async fn impacted_subtree_juror_pool_with_min_conn(
    conn: &mut sqlx::MySqlConnection,
    moving_community_id: u64,
    originator_social_entity_id: u64,
    target_social_entity_id: u64,
    created_by: Option<u64>,
    min_rating: Decimal,
    primary_tier: bool,
    rating_basis_community_id: u64,
) -> Result<Vec<JurorCandidate>, sqlx::Error> {
    let rating_clause = if primary_tier {
        "AND COALESCE(ser.rating, 2.50) > ?"
    } else {
        "AND COALESCE(ser.rating, 2.50) >= ?"
    };
    let sql = format!(
        r#"
        WITH RECURSIVE subtree AS (
            SELECT social_entity_id AS id
            FROM social_community
            WHERE social_entity_id = ?
            UNION ALL
            SELECT sc.social_entity_id
            FROM social_community sc
            INNER JOIN subtree s ON sc.parent_community_id = s.id
        )
        SELECT DISTINCT
            se.id AS juror_social_entity_id,
            se.created_by AS juror_user_id,
            COALESCE(ser.rating, 2.50) AS juror_rating_at_invite,
            ser.to_social_entity_id AS rating_community_entity_id
        FROM subtree st
        INNER JOIN social_entity_relationship ser
            ON ser.to_social_entity_id = st.id
           AND ser.ended_at IS NULL
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = ser.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` = ?
        INNER JOIN ref_key_values rk_status
            ON rk_status.id = ser.status_id
           AND rk_status.`group` = ?
           AND rk_status.`key` = ?
        INNER JOIN social_entity se ON se.id = ser.from_social_entity_id
        INNER JOIN social_human sh ON sh.social_entity_id = se.id
        INNER JOIN ref_key_values rk_entity_status
            ON rk_entity_status.id = COALESCE(se.entity_status_id, se.status_id)
           AND rk_entity_status.`key` = ?
        WHERE se.id NOT IN (?, ?)
          {rating_clause}
          AND (
              ? IS NULL
              OR (
                  (se.created_by IS NULL OR se.created_by <> ?)
                  AND NOT EXISTS (
                      SELECT 1
                      FROM user_social_entity_role ur_filer
                      WHERE ur_filer.user_id = ?
                        AND ur_filer.social_entity_id = se.id
                        AND ur_filer.ends_at IS NULL
                  )
              )
          )
        ORDER BY se.id ASC, ser.to_social_entity_id ASC
        "#
    );
    let rows: Vec<(u64, Option<u64>, Decimal, u64)> = sqlx::query_as(&sql)
        .bind(moving_community_id)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
        .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
        .bind(ref_keys::GROUP_STATUS)
        .bind(ref_keys::KEY_VERIFIED)
        .bind(ref_keys::KEY_VERIFIED)
        .bind(originator_social_entity_id)
        .bind(target_social_entity_id)
        .bind(min_rating)
        .bind(created_by)
        .bind(created_by)
        .bind(created_by)
        .fetch_all(conn)
        .await?;

    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for (juror_social_entity_id, juror_user_id, rating, rating_community) in rows {
        if !seen.insert(juror_social_entity_id) {
            continue;
        }
        out.push(JurorCandidate {
            juror_social_entity_id,
            juror_user_id,
            juror_rating_at_invite: rating,
            rating_basis_id: rating_basis_community_id,
            rating_community_entity_id: Some(rating_community),
        });
    }
    Ok(out)
}

/// Insert case + sidecar after caller validated preconditions and spent Noms.
pub async fn file_reparent_case_tx(
    tx: &mut Transaction<'_, MySql>,
    moving_community_id: u64,
    proposed_parent_id: u64,
    filer_user_id: u64,
    jury_size_frozen: u8,
    constitution_diff_json: Option<&str>,
) -> Result<u64, sqlx::Error> {
    let trigger_id = ref_key_values::lookup_active_id_conn(
        &mut **tx,
        ref_keys::GROUP_JUDICIAL_CASE_TRIGGER,
        ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_REPARENT,
    )
    .await?
    .ok_or_else(|| {
        sqlx::Error::Configuration("ref_key_values missing judicial_case_trigger.community_reparent".into())
    })?;
    let status_open_id = ref_key_values::lookup_active_id_conn(
        &mut **tx,
        ref_keys::GROUP_JUDICIAL_CASE_STATUS,
        ref_keys::KEY_JUDICIAL_CASE_OPEN,
    )
    .await?
    .ok_or_else(|| {
        sqlx::Error::Configuration("ref_key_values missing judicial_case_status.open".into())
    })?;

    let res = sqlx::query(
        r#"
        INSERT INTO judicial_case (
            originator_social_entity_id,
            target_social_entity_id,
            trigger_type_id,
            status_id,
            jury_size,
            jury_votes_for,
            jury_votes_against,
            ai_votes_for,
            ai_votes_against,
            created_by,
            modified_by
        )
        VALUES (?, ?, ?, ?, ?, 0, 0, 0, 0, ?, ?)
        "#,
    )
    .bind(moving_community_id)
    .bind(proposed_parent_id)
    .bind(trigger_id)
    .bind(status_open_id)
    .bind(jury_size_frozen)
    .bind(filer_user_id)
    .bind(filer_user_id)
    .execute(&mut **tx)
    .await?;
    let case_id = res.last_insert_id();

    sqlx::query(
        r#"
        INSERT INTO judicial_case_community_reparent (
            judicial_case_id,
            proposed_parent_community_id,
            constitution_diff_json
        )
        VALUES (?, ?, ?)
        "#,
    )
    .bind(case_id)
    .bind(proposed_parent_id)
    .bind(constitution_diff_json)
    .execute(&mut **tx)
    .await?;

    Ok(case_id)
}

pub async fn open_reparent_case_for_community(
    pool: &MySqlPool,
    moving_community_id: u64,
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
        ORDER BY jc.opened_at DESC, jc.id DESC
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_TRIGGER)
    .bind(ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_REPARENT)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(ref_keys::KEY_JUDICIAL_CASE_OPEN)
    .bind(ref_keys::KEY_JUDICIAL_CASE_JURY_ACTIVE)
    .bind(moving_community_id)
    .fetch_optional(pool)
    .await
}

/// After phase-1 **upheld**: freeze phase-2 pool size, reset jury, stay open for new invites.
pub async fn advance_to_impacted_phase_tx(
    tx: &mut Transaction<'_, MySql>,
    case_id: u64,
    moving_community_id: u64,
    system_actor: Option<u64>,
) -> Result<(), sqlx::Error> {
    let pool_n = count_impacted_subtree_humans_conn(&mut **tx, moving_community_id).await?;
    let jury_size = judicial::jury_size_for_case(
        ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_REPARENT,
        pool_n,
    );
    let jury_u8 = u8::try_from(jury_size).map_err(|_| {
        sqlx::Error::Configuration("impacted reparent jury_size overflow u8".into())
    })?;
    let pool_u32 = u32::try_from(pool_n).map_err(|_| {
        sqlx::Error::Configuration("impacted reparent pool size overflow u32".into())
    })?;

    let void_invite_id = ref_key_values::lookup_active_id_conn(
        &mut **tx,
        ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS,
        ref_keys::KEY_JCJI_VOIDED,
    )
    .await?
    .ok_or_else(|| {
        sqlx::Error::Configuration(
            "ref_key_values missing judicial_case_juror_invite_status.voided".into(),
        )
    })?;
    let open_status_id = ref_key_values::lookup_active_id_conn(
        &mut **tx,
        ref_keys::GROUP_JUDICIAL_CASE_STATUS,
        ref_keys::KEY_JUDICIAL_CASE_OPEN,
    )
    .await?
    .ok_or_else(|| {
        sqlx::Error::Configuration("ref_key_values missing judicial_case_status.open".into())
    })?;

    sqlx::query(
        r#"
        UPDATE judicial_case_juror_invite
        SET invite_status_id = ?,
            modified_by = ?
        WHERE judicial_case_id = ?
        "#,
    )
    .bind(void_invite_id)
    .bind(system_actor)
    .bind(case_id)
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE judicial_case
        SET status_id = ?,
            jury_size = ?,
            jury_votes_for = 0,
            jury_votes_against = 0,
            modified_by = ?
        WHERE id = ?
        "#,
    )
    .bind(open_status_id)
    .bind(jury_u8)
    .bind(system_actor)
    .bind(case_id)
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE judicial_case_community_reparent
        SET phase = 'impacted_vote',
            origin_phase_resolved_at = UTC_TIMESTAMP(6),
            impacted_pool_size_frozen = ?,
            impacted_jury_size_frozen = ?
        WHERE judicial_case_id = ?
        "#,
    )
    .bind(pool_u32)
    .bind(jury_u8)
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
    let Some(reparent) = fetch_reparent_for_case_tx(tx, case_id).await? else {
        return Ok(());
    };

    if reparent.phase == "origin_vote" {
        if verdict == VerdictOutcome::Upheld {
            advance_to_impacted_phase_tx(tx, case_id, case_row.originator_social_entity_id, system_actor)
                .await?;
        }
        return Ok(());
    }

    if reparent.phase == "impacted_vote" && verdict == VerdictOutcome::Upheld {
        community_graph::apply_judicial_reparent_tx(
            tx,
            case_id,
            case_row.originator_social_entity_id,
            reparent.proposed_parent_community_id,
            system_actor,
        )
        .await?;
    }
    Ok(())
}

/// Phase-aware settlement hook from [`judicial::check_and_resolve_verdict`].
pub(crate) async fn on_verdict_before_close_tx(
    tx: &mut Transaction<'_, MySql>,
    case_id: u64,
    case_row: &OpenCaseRow,
    verdict: VerdictOutcome,
    system_actor: Option<u64>,
) -> Result<bool, sqlx::Error> {
    let Some(reparent) = fetch_reparent_for_case_tx(tx, case_id).await? else {
        return Ok(true);
    };
    if reparent.phase == "origin_vote" && verdict == VerdictOutcome::Upheld {
        advance_to_impacted_phase_tx(tx, case_id, case_row.originator_social_entity_id, system_actor)
            .await?;
        return Ok(false);
    }
    if reparent.phase == "impacted_vote" && verdict == VerdictOutcome::Upheld {
        community_graph::apply_judicial_reparent_tx(
            tx,
            case_id,
            case_row.originator_social_entity_id,
            reparent.proposed_parent_community_id,
            system_actor,
        )
        .await?;
    }
    Ok(true)
}

pub(crate) async fn apply_no_jury_settlement_tx(
    tx: &mut Transaction<'_, MySql>,
    case_id: u64,
    case_row: &OpenCaseRow,
) -> Result<(), sqlx::Error> {
    let Some(reparent) = fetch_reparent_for_case_tx(tx, case_id).await? else {
        return Ok(());
    };
    if reparent.phase == "origin_vote" {
        return Ok(());
    }
    let _ = (case_id, case_row);
    Ok(())
}
