//! Deleted-account reactivation eligibility (email dispatch entry point).

use sqlx::MySqlPool;

use crate::ref_keys;

/// Lookup for reactivation email dispatch (anti-enumeration: caller always returns 202).
///
/// `email_canonical` is the canonical uniqueness key (see [`crate::auth::email_address::canonical`]);
/// the returned address is the account's deliverable `users.email`.
pub async fn lookup_eligible_reactivation_user(
    pool: &MySqlPool,
    email_canonical: &str,
) -> Result<Option<(u64, String)>, sqlx::Error> {
    let row: Option<(u64, String)> = sqlx::query_as(
        r#"
        SELECT u.id, u.email
        FROM users u
        INNER JOIN ref_key_values usr
            ON usr.id = COALESCE(u.user_status_id, u.status_id)
           AND usr.`group` = ?
           AND usr.`key` = ?
        INNER JOIN user_social_entity_role ur
            ON ur.user_id = u.id
           AND ur.ends_at IS NULL
        INNER JOIN social_entity se
            ON se.id = ur.social_entity_id
        INNER JOIN ref_key_values se_st
            ON se_st.id = se.entity_status_id
           AND se_st.`group` = ?
           AND se_st.`key` = ?
        INNER JOIN social_human sh
            ON sh.social_entity_id = se.id
        WHERE u.email_canonical = ?
          AND u.email_verified_at IS NOT NULL
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_USER_STATUS)
    .bind(ref_keys::KEY_TO_BE_DELETED)
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_STATUS)
    .bind(ref_keys::KEY_ENTITY_STATUS_DEAD)
    .bind(email_canonical)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}
