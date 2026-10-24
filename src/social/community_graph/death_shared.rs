//! Shared death-transition helpers (judicial voiding, relationship teardown).

use crate::ref_key_values;
use crate::ref_keys;

pub(crate) async fn end_all_relationships_for_entity_tx(
    conn: &mut sqlx::MySqlConnection,
    entity_id: u64,
    actor: Option<u64>,
) -> Result<Vec<u64>, sqlx::Error> {
    let edge_ids: Vec<(u64, u64, u64)> = sqlx::query_as(
        r#"
        SELECT ser.id, ser.from_social_entity_id, ser.to_social_entity_id
        FROM social_entity_relationship ser
        WHERE ser.ended_at IS NULL
          AND (ser.from_social_entity_id = ? OR ser.to_social_entity_id = ?)
        "#,
    )
    .bind(entity_id)
    .bind(entity_id)
    .fetch_all(&mut *conn)
    .await?;

    let mut peers = Vec::new();
    for (edge_id, from_id, to_id) in edge_ids {
        sqlx::query(
            r#"
            UPDATE social_entity_relationship
            SET ended_at = UTC_TIMESTAMP(6), modified_by = ?
            WHERE id = ?
            "#,
        )
        .bind(actor)
        .bind(edge_id)
        .execute(&mut *conn)
        .await?;
        let peer = if from_id == entity_id {
            to_id
        } else {
            from_id
        };
        if !peers.contains(&peer) {
            peers.push(peer);
        }
    }
    Ok(peers)
}

pub(crate) async fn void_open_judicial_cases_for_entity_tx(
    conn: &mut sqlx::MySqlConnection,
    entity_id: u64,
    actor: Option<u64>,
) -> Result<(), sqlx::Error> {
    let voided_status_id = ref_key_values::lookup_active_id_conn(
        conn,
        ref_keys::GROUP_JUDICIAL_CASE_STATUS,
        ref_keys::KEY_JUDICIAL_CASE_VOIDED,
    )
    .await?
    .ok_or_else(|| {
        sqlx::Error::Configuration("ref_key_values missing judicial_case_status.voided".into())
    })?;
    let void_invite_id = ref_key_values::lookup_active_id_conn(
        conn,
        ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS,
        ref_keys::KEY_JCJI_VOIDED,
    )
    .await?
    .ok_or_else(|| {
        sqlx::Error::Configuration(
            "ref_key_values missing judicial_case_juror_invite_status.voided".into(),
        )
    })?;

    let case_ids: Vec<u64> = sqlx::query_scalar(
        r#"
        SELECT jc.id
        FROM judicial_case jc
        INNER JOIN ref_key_values rk_st
            ON rk_st.id = jc.status_id
           AND rk_st.`group` = ?
           AND rk_st.`key` IN (?, ?, ?)
        WHERE jc.originator_social_entity_id = ?
           OR jc.target_social_entity_id = ?
           OR EXISTS (
                SELECT 1 FROM post p
                WHERE p.id = jc.subject_post_id
                  AND p.entity_id = ?
           )
           OR EXISTS (
                SELECT 1
                FROM post_audience_entity pae
                WHERE pae.post_id = jc.subject_post_id
                  AND pae.audience_entity_id = ?
           )
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(ref_keys::KEY_JUDICIAL_CASE_OPEN)
    .bind(ref_keys::KEY_JUDICIAL_CASE_JURY_ACTIVE)
    .bind(ref_keys::KEY_JUDICIAL_CASE_ESCALATED_TO_AI)
    .bind(entity_id)
    .bind(entity_id)
    .bind(entity_id)
    .bind(entity_id)
    .fetch_all(&mut *conn)
    .await?;

    for case_id in case_ids {
        sqlx::query(
            r#"
            UPDATE judicial_case_juror_invite jcji
            INNER JOIN ref_key_values rk_pend
                ON rk_pend.id = jcji.invite_status_id
               AND rk_pend.`group` = ?
               AND rk_pend.`key` = ?
            SET jcji.invite_status_id = ?,
                jcji.modified_by = ?
            WHERE jcji.judicial_case_id = ?
            "#,
        )
        .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
        .bind(ref_keys::KEY_JCJI_PENDING)
        .bind(void_invite_id)
        .bind(actor)
        .bind(case_id)
        .execute(&mut *conn)
        .await?;

        sqlx::query(
            r#"
            UPDATE judicial_case
            SET status_id = ?,
                closed_at = UTC_TIMESTAMP(6),
                modified_by = ?
            WHERE id = ?
            "#,
        )
        .bind(voided_status_id)
        .bind(actor)
        .bind(case_id)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}
