//! Idempotent DB inserts for Playwright front-end personas.

use sqlx::MySqlPool;

use crate::auth::password;
use crate::ref_keys;
use crate::social::human_onboarding;

use super::manifest::{E2eManifest, FixturesEntry, PersonaEntry, Personas, BASE_PASSWORD};
use super::personas::{
    CONSENT_EMAIL, FULL_EMAIL, FULL_HANDLE, GODMOTHER_AI_HANDLE, PARTIAL_EMAIL, PUBLIC_HANDLE,
    SAMPLE_POST_BODY,
};

struct RefIds {
    status_verified: u64,
    status_pending: u64,
    user_status_verified: u64,
    user_status_pending: u64,
    user_type_human: u64,
    tier_basic: u64,
    entity_type_human: u64,
    entity_status_verified: u64,
    join_policy_open: u64,
    role_owner: u64,
    rel_godmother: u64,
    rel_member_of: u64,
    rel_moderator: u64,
    post_status_verified: u64,
}

async fn load_ref_ids(pool: &MySqlPool) -> Result<RefIds, String> {
    async fn id(pool: &MySqlPool, group: &str, key: &str) -> Result<u64, String> {
        sqlx::query_scalar(
            "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? AND is_active = 1 LIMIT 1",
        )
        .bind(group)
        .bind(key)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("ref_key_values lookup ({group}, {key}): {e}"))?
        .ok_or_else(|| format!("ref_key_values missing: ({group}, {key})"))
    }

    Ok(RefIds {
        status_verified: id(pool, ref_keys::GROUP_STATUS, ref_keys::KEY_VERIFIED).await?,
        status_pending: id(
            pool,
            ref_keys::GROUP_STATUS,
            ref_keys::KEY_PENDING_VERIFICATION,
        )
        .await?,
        user_status_verified: id(pool, ref_keys::GROUP_USER_STATUS, ref_keys::KEY_VERIFIED).await?,
        user_status_pending: id(
            pool,
            ref_keys::GROUP_USER_STATUS,
            ref_keys::KEY_PENDING_VERIFICATION,
        )
        .await?,
        user_type_human: id(pool, ref_keys::GROUP_USER_TYPE, ref_keys::KEY_USER_TYPE_HUMAN).await?,
        tier_basic: id(
            pool,
            ref_keys::GROUP_VERIFICATION_TIER,
            ref_keys::KEY_VERIFICATION_TIER_BASIC,
        )
        .await?,
        entity_type_human: id(
            pool,
            ref_keys::GROUP_SOCIAL_ENTITY_TYPE,
            ref_keys::KEY_ENTITY_TYPE_HUMAN,
        )
        .await?,
        entity_status_verified: id(
            pool,
            ref_keys::GROUP_SOCIAL_ENTITY_STATUS,
            ref_keys::KEY_VERIFIED,
        )
        .await?,
        join_policy_open: id(pool, ref_keys::GROUP_SOCIAL_JOIN_POLICY, "open").await?,
        role_owner: id(pool, ref_keys::GROUP_USER_SOCIAL_ENTITY_ROLE, "owner").await?,
        rel_godmother: id(
            pool,
            ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE,
            ref_keys::KEY_RELATIONSHIP_GODMOTHER,
        )
        .await?,
        rel_member_of: id(
            pool,
            ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE,
            ref_keys::KEY_RELATIONSHIP_MEMBER_OF,
        )
        .await?,
        rel_moderator: id(
            pool,
            ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE,
            ref_keys::KEY_RELATIONSHIP_MODERATOR,
        )
        .await?,
        post_status_verified: id(pool, ref_keys::GROUP_STATUS, ref_keys::KEY_VERIFIED).await?,
    })
}

async fn location_id(pool: &MySqlPool) -> Result<u64, String> {
    if let Some(id) = sqlx::query_scalar("SELECT id FROM locations ORDER BY id ASC LIMIT 1")
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
    {
        return Ok(id);
    }

    let lifecycle_id: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = 'location_lifecycle' AND `key` = 'active' LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .map_err(|e| format!("location_lifecycle active: {e}"))?;

    let result = sqlx::query(
        r#"INSERT INTO locations (country_code, locality, display_label, validation_source, validation_ref, canonical_fingerprint, location_lifecycle_id)
           VALUES ('ZZ', 'E2E City', 'E2E City', 'seed', 'e2e-seed', UNHEX(SHA2('e2e-seed-loc', 256)), ?)"#,
    )
    .bind(lifecycle_id)
    .execute(pool)
    .await
    .map_err(|e| format!("insert location: {e}"))?;

    Ok(result.last_insert_id())
}

pub async fn reset_e2e_rows(pool: &MySqlPool) -> Result<(), String> {
    let user_ids: Vec<u64> = sqlx::query_scalar(
        "SELECT id FROM users WHERE email LIKE 'e2e.%@256.test' ORDER BY id ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let entity_ids: Vec<u64> = sqlx::query_scalar(
        "SELECT id FROM social_entity WHERE handle IN ('e2efull', 'e2econsent') ORDER BY id ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    if !user_ids.is_empty() {
        sqlx::query(
            "DELETE pa FROM post_audience_entity pa
             INNER JOIN post p ON p.id = pa.post_id
             WHERE p.body_plain = ?",
        )
        .bind(SAMPLE_POST_BODY)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

        sqlx::query(
            "DELETE FROM post WHERE body_plain = ?",
        )
        .bind(SAMPLE_POST_BODY)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

        for uid in &user_ids {
            sqlx::query("DELETE FROM user_faces WHERE user_id = ?")
                .bind(uid)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    for eid in &entity_ids {
        sqlx::query(
            "DELETE FROM social_entity_relationship WHERE from_social_entity_id = ? OR to_social_entity_id = ?",
        )
        .bind(eid)
        .bind(eid)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

        sqlx::query("DELETE FROM user_social_entity_role WHERE social_entity_id = ?")
            .bind(eid)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;

        sqlx::query("DELETE FROM social_human WHERE social_entity_id = ?")
            .bind(eid)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;

        sqlx::query("DELETE FROM social_entity WHERE id = ?")
            .bind(eid)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
    }

    if !user_ids.is_empty() {
        for uid in &user_ids {
            sqlx::query("DELETE FROM users WHERE id = ?")
                .bind(uid)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

struct UserSeed {
    user_id: u64,
}

async fn upsert_user(
    pool: &MySqlPool,
    refs: &RefIds,
    password_hash: &[u8],
    email: &str,
    status_id: u64,
    user_status_id: u64,
    terms_consent: Option<&str>,
) -> Result<u64, String> {
    if let Some(existing) = sqlx::query_scalar::<_, u64>(
        "SELECT id FROM users WHERE email = ? LIMIT 1",
    )
    .bind(email)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    {
        sqlx::query(
            r#"
            UPDATE users
            SET password_hash = ?,
                email_verified_at = COALESCE(email_verified_at, UTC_TIMESTAMP(6)),
                status_id = ?,
                user_status_id = ?,
                user_type_id = ?,
                verification_tier_id = ?,
                terms_consent_version = ?,
                modified_by = ?
            WHERE id = ?
            "#,
        )
        .bind(password_hash)
        .bind(status_id)
        .bind(user_status_id)
        .bind(refs.user_type_human)
        .bind(refs.tier_basic)
        .bind(terms_consent)
        .bind(existing)
        .bind(existing)
        .execute(pool)
        .await
        .map_err(|e| format!("update user {email}: {e}"))?;
        return Ok(existing);
    }

    let result = sqlx::query(
        r#"
        INSERT INTO users (
            email, email_canonical, password_hash, email_verified_at,
            status_id, user_status_id, user_type_id, verification_tier_id,
            terms_consent_version, created_by, modified_by
        )
        VALUES (?, ?, ?, UTC_TIMESTAMP(6), ?, ?, ?, ?, ?, NULL, NULL)
        "#,
    )
    .bind(email)
    .bind(crate::auth::email_address::canonical(email))
    .bind(password_hash)
    .bind(status_id)
    .bind(user_status_id)
    .bind(refs.user_type_human)
    .bind(refs.tier_basic)
    .bind(terms_consent)
    .execute(pool)
    .await
    .map_err(|e| format!("insert user {email}: {e}"))?;

    let user_id = result.last_insert_id();
    sqlx::query("UPDATE users SET created_by = ?, modified_by = ? WHERE id = ?")
        .bind(user_id)
        .bind(user_id)
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| format!("self-ref user {email}: {e}"))?;

    Ok(user_id)
}

async fn upsert_human_entity(
    pool: &MySqlPool,
    refs: &RefIds,
    location_id: u64,
    user_id: u64,
    handle: &str,
) -> Result<u64, String> {
    if let Some(existing) = sqlx::query_scalar::<_, u64>(
        "SELECT id FROM social_entity WHERE handle = ? LIMIT 1",
    )
    .bind(handle)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    {
        sqlx::query(
            "UPDATE social_entity SET created_by = ?, modified_by = ? WHERE id = ?",
        )
        .bind(user_id)
        .bind(user_id)
        .bind(existing)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
        return Ok(existing);
    }

    let result = sqlx::query(
        r#"
        INSERT INTO social_entity (
            handle, type_id, status_id, entity_status_id, location_id,
            rating, created_by, modified_by
        )
        VALUES (?, ?, ?, ?, ?, '2.50', ?, ?)
        "#,
    )
    .bind(handle)
    .bind(refs.entity_type_human)
    .bind(refs.status_verified)
    .bind(refs.entity_status_verified)
    .bind(location_id)
    .bind(user_id)
    .bind(user_id)
    .execute(pool)
    .await
    .map_err(|e| format!("insert entity {handle}: {e}"))?;

    Ok(result.last_insert_id())
}

async fn ensure_human_profile(
    pool: &MySqlPool,
    refs: &RefIds,
    location_id: u64,
    entity_id: u64,
) -> Result<(), String> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM social_human WHERE social_entity_id = ?)",
    )
    .bind(entity_id)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    if exists {
        return Ok(());
    }

    sqlx::query(
        r#"
        INSERT INTO social_human (
            social_entity_id, family_name, given_name, birth_date,
            birth_location_id, join_policy_id
        )
        VALUES (?, 'E2E', 'Full', '1990-06-15', ?, ?)
        "#,
    )
    .bind(entity_id)
    .bind(location_id)
    .bind(refs.join_policy_open)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

async fn ensure_owner_role(
    pool: &MySqlPool,
    refs: &RefIds,
    user_id: u64,
    entity_id: u64,
) -> Result<(), String> {
    let exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM user_social_entity_role
            WHERE user_id = ? AND social_entity_id = ? AND ends_at IS NULL
        )
        "#,
    )
    .bind(user_id)
    .bind(entity_id)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    if exists {
        return Ok(());
    }

    sqlx::query(
        r#"
        INSERT INTO user_social_entity_role (user_id, social_entity_id, role_id, is_primary, starts_at)
        VALUES (?, ?, ?, 1, UTC_TIMESTAMP())
        "#,
    )
    .bind(user_id)
    .bind(entity_id)
    .bind(refs.role_owner)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

async fn ensure_face_row(pool: &MySqlPool, user_id: u64) -> Result<(), String> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM user_faces WHERE user_id = ?)",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    if exists {
        return Ok(());
    }

    sqlx::query(
        r#"
        INSERT INTO user_faces (
            user_id, generated_artwork_path, ai_model_id, quality_score,
            captured_at, face_captured_at, created_by, modified_by
        )
        VALUES (?, NULL, 'e2e-seed-v1', NULL, UTC_TIMESTAMP(), UTC_TIMESTAMP(), ?, ?)
        "#,
    )
    .bind(user_id)
    .bind(user_id)
    .bind(user_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

async fn milo_community_id(pool: &MySqlPool, refs: &RefIds) -> Result<(u64, u64), String> {
    let milo_eid: u64 = sqlx::query_scalar(
        "SELECT id FROM social_entity WHERE handle = ? LIMIT 1",
    )
    .bind(GODMOTHER_AI_HANDLE)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| {
        format!(
            "council AI handle '{GODMOTHER_AI_HANDLE}' not found — run migrations first"
        )
    })?;

    let community_eid: u64 = sqlx::query_scalar(
        r#"
        SELECT modr.to_social_entity_id
        FROM social_entity_relationship modr
        WHERE modr.from_social_entity_id = ?
          AND modr.type_id = ?
          AND modr.ended_at IS NULL
        ORDER BY modr.id ASC
        LIMIT 1
        "#,
    )
    .bind(milo_eid)
    .bind(refs.rel_moderator)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| format!("moderator community for '{GODMOTHER_AI_HANDLE}' not found"))?;

    Ok((milo_eid, community_eid))
}

async fn ensure_relationship(
    pool: &MySqlPool,
    refs: &RefIds,
    from_eid: u64,
    to_eid: u64,
    type_id: u64,
    actor: u64,
) -> Result<(), String> {
    let exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM social_entity_relationship
            WHERE from_social_entity_id = ?
              AND to_social_entity_id = ?
              AND type_id = ?
              AND ended_at IS NULL
        )
        "#,
    )
    .bind(from_eid)
    .bind(to_eid)
    .bind(type_id)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    if exists {
        return Ok(());
    }

    sqlx::query(
        r#"
        INSERT INTO social_entity_relationship (
            from_social_entity_id, to_social_entity_id, type_id, status_id,
            created_by, modified_by
        )
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(from_eid)
    .bind(to_eid)
    .bind(type_id)
    .bind(refs.status_verified)
    .bind(actor)
    .bind(actor)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

async fn ensure_sample_post(
    pool: &MySqlPool,
    refs: &RefIds,
    author_entity_id: u64,
    author_user_id: u64,
    audience_community_id: u64,
) -> Result<(), String> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM post WHERE body_plain = ? LIMIT 1)",
    )
    .bind(SAMPLE_POST_BODY)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    if exists {
        return Ok(());
    }

    let result = sqlx::query(
        r#"
        INSERT INTO post (
            entity_id, content_link_id, status_id,
            body_plain, body_markup, body_html_sanitized,
            published_at, created_by, modified_by
        )
        VALUES (?, NULL, ?, ?, ?, ?, UTC_TIMESTAMP(6), ?, ?)
        "#,
    )
    .bind(author_entity_id)
    .bind(refs.post_status_verified)
    .bind(SAMPLE_POST_BODY)
    .bind(SAMPLE_POST_BODY)
    .bind(format!("<p>{SAMPLE_POST_BODY}</p>"))
    .bind(author_user_id)
    .bind(author_user_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    let post_id = result.last_insert_id();

    sqlx::query(
        "INSERT INTO post_audience_entity (post_id, audience_entity_id) VALUES (?, ?)",
    )
    .bind(post_id)
    .bind(audience_community_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

async fn seed_full_persona(
    pool: &MySqlPool,
    refs: &RefIds,
    location_id: u64,
    password_hash: &[u8],
) -> Result<UserSeed, String> {
    let terms = human_onboarding::CURRENT_TERMS_CONSENT_VERSION;
    let user_id = upsert_user(
        pool,
        refs,
        password_hash,
        FULL_EMAIL,
        refs.status_verified,
        refs.user_status_verified,
        Some(terms),
    )
    .await?;

    let entity_id = upsert_human_entity(pool, refs, location_id, user_id, FULL_HANDLE).await?;
    ensure_human_profile(pool, refs, location_id, entity_id).await?;
    ensure_owner_role(pool, refs, user_id, entity_id).await?;
    ensure_face_row(pool, user_id).await?;

    let (milo_eid, community_eid) = milo_community_id(pool, refs).await?;
    ensure_relationship(pool, refs, entity_id, milo_eid, refs.rel_godmother, user_id).await?;
    ensure_relationship(
        pool,
        refs,
        entity_id,
        community_eid,
        refs.rel_member_of,
        user_id,
    )
    .await?;

    ensure_sample_post(
        pool,
        refs,
        entity_id,
        user_id,
        community_eid,
    )
    .await?;

    Ok(UserSeed { user_id })
}

async fn seed_consent_persona(
    pool: &MySqlPool,
    refs: &RefIds,
    password_hash: &[u8],
) -> Result<UserSeed, String> {
    let user_id = upsert_user(
        pool,
        refs,
        password_hash,
        CONSENT_EMAIL,
        refs.status_pending,
        refs.user_status_pending,
        None,
    )
    .await?;

    Ok(UserSeed { user_id })
}

async fn seed_partial_persona(
    pool: &MySqlPool,
    refs: &RefIds,
    password_hash: &[u8],
) -> Result<UserSeed, String> {
    let terms = human_onboarding::CURRENT_TERMS_CONSENT_VERSION;
    let user_id = upsert_user(
        pool,
        refs,
        password_hash,
        PARTIAL_EMAIL,
        refs.status_pending,
        refs.user_status_pending,
        Some(terms),
    )
    .await?;

    Ok(UserSeed { user_id })
}

pub async fn seed_all(pool: &MySqlPool, reset: bool) -> Result<E2eManifest, String> {
    if reset {
        reset_e2e_rows(pool).await?;
    }

    let refs = load_ref_ids(pool).await?;
    let location_id = location_id(pool).await?;

    let phc = password::hash_password(BASE_PASSWORD).map_err(|e| e.to_string())?;
    let password_hash = phc.as_bytes();

    let full = seed_full_persona(pool, &refs, location_id, password_hash).await?;
    let consent = seed_consent_persona(pool, &refs, password_hash).await?;
    let partial = seed_partial_persona(pool, &refs, password_hash).await?;

    Ok(E2eManifest {
        base_password: BASE_PASSWORD.to_string(),
        terms_consent_version: human_onboarding::CURRENT_TERMS_CONSENT_VERSION.to_string(),
        personas: Personas {
            full: PersonaEntry {
                email: FULL_EMAIL.to_string(),
                handle: Some(FULL_HANDLE.to_string()),
                user_id: full.user_id,
            },
            consent: PersonaEntry {
                email: CONSENT_EMAIL.to_string(),
                handle: None,
                user_id: consent.user_id,
            },
            partial: PersonaEntry {
                email: PARTIAL_EMAIL.to_string(),
                handle: None,
                user_id: partial.user_id,
            },
        },
        fixtures: FixturesEntry {
            public_handle: PUBLIC_HANDLE.to_string(),
            sample_post_body: SAMPLE_POST_BODY.to_string(),
        },
    })
}

pub fn database_url_allowed(raw: &str, allow_flag: bool) -> bool {
    if allow_flag {
        return true;
    }
    let lower = raw.to_ascii_lowercase();
    if lower.contains("localhost") || lower.contains("127.0.0.1") {
        return true;
    }
    if lower.contains("/test") || lower.contains("_test") || lower.contains("_e2e") {
        return true;
    }
    false
}
