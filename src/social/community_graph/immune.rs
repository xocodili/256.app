//! Death-immune communities and humans (seeded council, product roots, system actors).

use sqlx::MySqlPool;

use crate::ref_keys;

/// Seeded council domain communities (death-immune).
pub const COUNCIL_COMMUNITY_HANDLES: &[&str] = &[
    "creativity_leisure_adventure",
    "fitness_health_wellbeing",
    "intellectual_life_education",
    "emotional_life_wellbeing",
    "character_self_mastery",
    "spiritual_life_meaning",
    "love_romantic_relationships",
    "parenting_family",
    "social_life_community_justice",
    "career_mission",
    "financial_life_abundance",
    "quality_of_life_environment",
];

pub(crate) const IMMUNE_PRODUCT_HANDLES: &[&str] = &[
    ref_keys::SEED_COMMUNITY_256_HANDLE,
    "operating_charter_256",
    ref_keys::PANOPLY_COMMUNITY_HANDLE,
];

/// Platform and seeded identities that never auto-die at rating zero.
pub(crate) const IMMUNE_HUMAN_HANDLES: &[&str] = &[ref_keys::SYSTEM_ENTITY_HANDLE];

/// Sync check for feed/profile rendering when parent tree is not loaded.
#[must_use]
pub fn is_death_immune_handle(handle: &str) -> bool {
    IMMUNE_PRODUCT_HANDLES.contains(&handle)
        || COUNCIL_COMMUNITY_HANDLES.contains(&handle)
        || IMMUNE_HUMAN_HANDLES.contains(&handle)
}

#[must_use]
pub fn is_human_death_immune_handle(handle: &str) -> bool {
    IMMUNE_HUMAN_HANDLES.contains(&handle)
}

/// Root charter, product communities, and council domains never auto-die.
pub async fn is_death_immune(pool: &MySqlPool, community_entity_id: u64) -> Result<bool, sqlx::Error> {
    let mut conn = pool.acquire().await?;
    is_death_immune_conn(&mut *conn, community_entity_id).await
}

pub async fn is_death_immune_conn(
    conn: &mut sqlx::MySqlConnection,
    community_entity_id: u64,
) -> Result<bool, sqlx::Error> {
    let row: Option<(String, Option<u64>)> = sqlx::query_as(
        r#"
        SELECT se.handle, sc.parent_community_id
        FROM social_entity se
        INNER JOIN social_community sc ON sc.social_entity_id = se.id
        WHERE se.id = ?
        LIMIT 1
        "#,
    )
    .bind(community_entity_id)
    .fetch_optional(&mut *conn)
    .await?;

    let Some((handle, parent_id)) = row else {
        return Ok(false);
    };

    if parent_id.is_none() {
        return Ok(true);
    }
    if IMMUNE_PRODUCT_HANDLES.contains(&handle.as_str())
        || COUNCIL_COMMUNITY_HANDLES.contains(&handle.as_str())
    {
        return Ok(true);
    }
    Ok(false)
}

pub async fn is_human_death_immune(pool: &MySqlPool, human_entity_id: u64) -> Result<bool, sqlx::Error> {
    let mut conn = pool.acquire().await?;
    is_human_death_immune_conn(&mut *conn, human_entity_id).await
}

pub async fn is_human_death_immune_conn(
    conn: &mut sqlx::MySqlConnection,
    human_entity_id: u64,
) -> Result<bool, sqlx::Error> {
    let row: Option<(String, Option<String>)> = sqlx::query_as(
        r#"
        SELECT se.handle, rk_ut.`key`
        FROM social_entity se
        INNER JOIN social_human sh ON sh.social_entity_id = se.id
        LEFT JOIN user_social_entity_role ur ON ur.social_entity_id = se.id AND ur.ends_at IS NULL
        LEFT JOIN users u ON u.id = ur.user_id
        LEFT JOIN ref_key_values rk_ut
            ON rk_ut.id = u.user_type_id
           AND rk_ut.`group` = ?
        WHERE se.id = ?
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_USER_TYPE)
    .bind(human_entity_id)
    .fetch_optional(&mut *conn)
    .await?;

    let Some((handle, user_type_key)) = row else {
        return Ok(false);
    };
    if IMMUNE_HUMAN_HANDLES.contains(&handle.as_str()) {
        return Ok(true);
    }
    if user_type_key.as_deref() == Some(ref_keys::KEY_USER_TYPE_AI) {
        return Ok(true);
    }
    if panoply_member_immunity_conn(conn, human_entity_id).await? {
        return Ok(true);
    }
    Ok(false)
}

async fn panoply_member_immunity_conn(
    conn: &mut sqlx::MySqlConnection,
    human_entity_id: u64,
) -> Result<bool, sqlx::Error> {
    let active: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM social_entity_relationship ser
        INNER JOIN social_entity se ON se.id = ser.to_social_entity_id
        INNER JOIN ref_key_values rk_type ON rk_type.id = ser.type_id
            AND rk_type.`group` = ?
            AND rk_type.`key` = ?
        INNER JOIN ref_key_values rk_status ON rk_status.id = ser.status_id
            AND rk_status.`group` = ?
            AND rk_status.`key` = ?
        WHERE ser.from_social_entity_id = ?
          AND se.handle = ?
          AND ser.ended_at IS NULL
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_VERIFIED)
    .bind(human_entity_id)
    .bind(ref_keys::PANOPLY_COMMUNITY_HANDLE)
    .fetch_one(conn)
    .await?;
    Ok(active > 0)
}

/// Active verified `member_of` @panoply (ops clearance + human death immunity).
pub async fn is_panoply_operator_conn(
    conn: &mut sqlx::MySqlConnection,
    human_entity_id: u64,
) -> Result<bool, sqlx::Error> {
    panoply_member_immunity_conn(conn, human_entity_id).await
}

pub async fn is_panoply_operator_for_user(
    pool: &MySqlPool,
    user_id: u64,
) -> Result<bool, sqlx::Error> {
    let Some(human_id) = crate::social::human_onboarding::primary_human_entity_id(pool, user_id).await?
    else {
        return Ok(false);
    };
    let mut conn = pool.acquire().await?;
    is_panoply_operator_conn(&mut *conn, human_id).await
}
