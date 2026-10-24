use sqlx::{MySql, MySqlPool};

use crate::ref_keys;

use super::types::OpenCaseRow;
async fn community_context_for_case(pool: &MySqlPool, case: &OpenCaseRow) -> Result<Option<u64>, sqlx::Error> {
    let mut conn = pool.acquire().await?;
    community_context_for_case_conn(&mut *conn, case).await
}

pub(crate) async fn community_context_for_case_tx(
    tx: &mut sqlx::Transaction<'_, MySql>,
    case: &OpenCaseRow,
) -> Result<Option<u64>, sqlx::Error> {
    community_context_for_case_conn(&mut **tx, case).await
}

pub(crate) async fn community_context_for_case_conn(
    conn: &mut sqlx::MySqlConnection,
    case: &OpenCaseRow,
) -> Result<Option<u64>, sqlx::Error> {
    if case.trigger_key == ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_REPARENT {
        let moving: Option<u64> = sqlx::query_scalar(
            r#"
            SELECT sc.social_entity_id
            FROM social_community sc
            WHERE sc.social_entity_id = ?
            LIMIT 1
            "#,
        )
        .bind(case.originator_social_entity_id)
        .fetch_optional(&mut *conn)
        .await?;
        return Ok(moving);
    }

    let target_community: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT sc.social_entity_id
        FROM social_community sc
        WHERE sc.social_entity_id = ?
        LIMIT 1
        "#,
    )
    .bind(case.target_social_entity_id)
    .fetch_optional(&mut *conn)
    .await?;
    if target_community.is_some() {
        return Ok(target_community);
    }

    let Some(post_id) = case.subject_post_id else {
        return Ok(None);
    };

    crate::social::post_community::community_context_for_root_post_conn(conn, post_id).await
}
pub(crate) async fn fetch_open_case_row_tx(
    tx: &mut sqlx::Transaction<'_, MySql>,
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
        INNER JOIN ref_key_values rk_trigger
            ON rk_trigger.id = jc.trigger_type_id
           AND rk_trigger.`group` = ?
           AND rk_trigger.is_active = 1
        WHERE jc.id = ?
        FOR UPDATE
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_TRIGGER)
    .bind(case_id)
    .fetch_optional(&mut **tx)
    .await
}

pub(crate) async fn is_community_social_entity_tx(
    tx: &mut sqlx::Transaction<'_, MySql>,
    entity_id: u64,
) -> Result<bool, sqlx::Error> {
    let hit: Option<i8> = sqlx::query_scalar(
        r#"
        SELECT 1
        FROM social_entity se
        INNER JOIN ref_key_values rk
            ON rk.id = se.type_id
           AND rk.`group` = ?
           AND rk.`key` = ?
           AND rk.is_active = 1
        WHERE se.id = ?
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(ref_keys::KEY_ENTITY_TYPE_COMMUNITY)
    .bind(entity_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(hit.is_some())
}

