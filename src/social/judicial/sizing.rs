use sqlx::{MySql, MySqlConnection, MySqlPool};

use crate::ref_key_values;
use crate::ref_keys;

use super::pool::juror_pool_for_case_conn;
use super::triggers::trigger_requires_full_community_jury_pool;
use super::types::OpenCaseRow;
pub fn jury_size_for_case(trigger_key: &str, pool_size: usize) -> usize {
    if trigger_key == ref_keys::KEY_JUDICIAL_TRIGGER_CASE_APPEAL {
        return 0;
    }
    if pool_size == 0 {
        return 0;
    }
    let root = (pool_size as f64).sqrt().ceil() as usize;
    let root = root.max(1);
    if trigger_requires_full_community_jury_pool(trigger_key) {
        root.min(pool_size)
    } else {
        root.min(12).min(pool_size)
    }
}

/// Snapshot the eligible juror pool at filing and return the square-root target seat count
/// to store on `judicial_case.jury_size` (see [`jury_size_for_case`]), using the **same**
/// `MySqlConnection` as the surrounding work (e.g. filing transaction).
async fn compute_jury_size_to_freeze_at_filing_conn(
    conn: &mut MySqlConnection,
    originator_social_entity_id: u64,
    target_social_entity_id: u64,
    created_by: Option<u64>,
    subject_post_id: Option<u64>,
    trigger_key: &str,
) -> Result<u8, sqlx::Error> {
    let Some(rating_basis_community_id) = ref_key_values::lookup_active_id_conn(
        conn,
        ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_RATING_BASIS,
        ref_keys::KEY_JCJI_RB_COMMUNITY,
    )
    .await?
    else {
        return Err(sqlx::Error::Configuration(
            "ref_key_values missing judicial_case_juror_invite_rating_basis.community".into(),
        ));
    };

    let probe = OpenCaseRow {
        id: 0,
        originator_social_entity_id,
        target_social_entity_id,
        created_by,
        trigger_key: trigger_key.to_string(),
        subject_post_id,
        jury_size: 0,
        status_key: String::new(),
    };
    let pool_vec = juror_pool_for_case_conn(conn, &probe, rating_basis_community_id).await?;
    let n = jury_size_for_case(trigger_key, pool_vec.len());
    u8::try_from(n).map_err(|_| {
        sqlx::Error::Configuration("computed judicial jury_size does not fit u8".into())
    })
}

/// Same as [`compute_jury_size_to_freeze_at_filing`] but reads `ref_key_values` and the juror
/// pool on **`tx`** so the snapshot matches uncommitted filing state.
pub async fn compute_jury_size_to_freeze_at_filing_tx(
    tx: &mut sqlx::Transaction<'_, MySql>,
    originator_social_entity_id: u64,
    target_social_entity_id: u64,
    created_by: Option<u64>,
    subject_post_id: Option<u64>,
    trigger_key: &str,
) -> Result<u8, sqlx::Error> {
    compute_jury_size_to_freeze_at_filing_conn(
        &mut **tx,
        originator_social_entity_id,
        target_social_entity_id,
        created_by,
        subject_post_id,
        trigger_key,
    )
    .await
}

/// Snapshot the eligible juror pool at filing and return the square-root target seat count
/// to store on `judicial_case.jury_size` (see [`jury_size_for_case`]). Prefer
/// [`compute_jury_size_to_freeze_at_filing_tx`] when filing inside a transaction.
pub async fn compute_jury_size_to_freeze_at_filing(
    pool: &MySqlPool,
    originator_social_entity_id: u64,
    target_social_entity_id: u64,
    created_by: Option<u64>,
    subject_post_id: Option<u64>,
    trigger_key: &str,
) -> Result<u8, sqlx::Error> {
    let mut conn = pool.acquire().await?;
    compute_jury_size_to_freeze_at_filing_conn(
        &mut *conn,
        originator_social_entity_id,
        target_social_entity_id,
        created_by,
        subject_post_id,
        trigger_key,
    )
    .await
}

