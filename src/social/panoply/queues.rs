//! Read-only Panoply operator queues.

use chrono::{DateTime, Utc};
use sqlx::MySqlPool;

use crate::ref_keys;

#[derive(Debug, Clone)]
pub struct ContainedEmergencyRow {
    pub case_id: u64,
    pub root_post_id: u64,
    pub target_handle: String,
    pub opened_at: DateTime<Utc>,
}

pub async fn list_contained_emergencies(
    pool: &MySqlPool,
    limit: u32,
) -> Result<Vec<ContainedEmergencyRow>, sqlx::Error> {
    let cap = limit.clamp(1, 50);
    sqlx::query_as::<_, (u64, u64, String, DateTime<Utc>)>(
        r#"
        SELECT jc.id, jc.subject_post_id, se.handle, jc.opened_at
        FROM judicial_case jc
        INNER JOIN ref_key_values rk_tr ON rk_tr.id = jc.trigger_type_id
            AND rk_tr.`group` = ?
            AND rk_tr.`key` = ?
        INNER JOIN ref_key_values rk_st ON rk_st.id = jc.status_id
            AND rk_st.`group` = ?
            AND rk_st.`key` = ?
        INNER JOIN social_entity se ON se.id = jc.target_social_entity_id
        WHERE jc.contested_at IS NULL
        ORDER BY jc.opened_at ASC
        LIMIT ?
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_TRIGGER)
    .bind(ref_keys::KEY_JUDICIAL_TRIGGER_EMERGENCY_SAFETY)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(ref_keys::KEY_JUDICIAL_CASE_CONTAINED)
    .bind(cap)
    .fetch_all(pool)
    .await
    .map(|rows| {
        rows.into_iter()
            .map(|(case_id, root_post_id, target_handle, opened_at)| ContainedEmergencyRow {
                case_id,
                root_post_id,
                target_handle,
                opened_at,
            })
            .collect()
    })
}
