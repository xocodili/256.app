//! Voluntary human account deletion uses the same death ritual as rating-zero removal.

mod common;

use app::auth::session;
use app::ref_keys;
use app::social::community_graph;
use common::TestDb;
use sqlx::MySqlPool;

async fn insert_test_human(pool: &MySqlPool, handle: &str, rating: &str) -> (u64, u64) {
    let human_type: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? LIMIT 1",
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(ref_keys::KEY_ENTITY_TYPE_HUMAN)
    .fetch_one(pool)
    .await
    .expect("human type");
    let verified_status: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? LIMIT 1",
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_STATUS)
    .bind(ref_keys::KEY_VERIFIED)
    .fetch_one(pool)
    .await
    .expect("verified status");
    let join_open: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? LIMIT 1",
    )
    .bind(ref_keys::GROUP_SOCIAL_JOIN_POLICY)
    .bind(ref_keys::KEY_JOIN_POLICY_OPEN)
    .fetch_one(pool)
    .await
    .expect("join open");
    let loc: u64 = sqlx::query_scalar("SELECT id FROM locations LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("location");
    let system_user: u64 = sqlx::query_scalar("SELECT id FROM users WHERE email = ? LIMIT 1")
        .bind(ref_keys::SYSTEM_USER_EMAIL)
        .fetch_one(pool)
        .await
        .expect("system user");

    sqlx::query(
        r#"
        INSERT INTO social_entity (handle, type_id, status_id, entity_status_id, location_id, rating, created_by, modified_by)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(handle)
    .bind(human_type)
    .bind(verified_status)
    .bind(verified_status)
    .bind(loc)
    .bind(rating)
    .bind(system_user)
    .bind(system_user)
    .execute(pool)
    .await
    .expect("entity");
    let entity_id: u64 = sqlx::query_scalar("SELECT id FROM social_entity WHERE handle = ?")
        .bind(handle)
        .fetch_one(pool)
        .await
        .expect("id");
    sqlx::query(
        r#"
        INSERT INTO social_human (
            social_entity_id, family_name, given_name, name_sort_key, birth_date, join_policy_id
        )
        VALUES (?, 'Test', 'User', 'test user', '1990-01-01', ?)
        "#,
    )
    .bind(entity_id)
    .bind(join_open)
    .execute(pool)
    .await
    .expect("human");

    let user_status: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? LIMIT 1",
    )
    .bind(ref_keys::GROUP_USER_STATUS)
    .bind(ref_keys::KEY_VERIFIED)
    .fetch_one(pool)
    .await
    .expect("user verified");
    let user_type: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? LIMIT 1",
    )
    .bind(ref_keys::GROUP_USER_TYPE)
    .bind(ref_keys::KEY_USER_TYPE_HUMAN)
    .fetch_one(pool)
    .await
    .expect("user type human");
    let tier: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? LIMIT 1",
    )
    .bind(ref_keys::GROUP_VERIFICATION_TIER)
    .bind(ref_keys::KEY_VERIFICATION_TIER_BASIC)
    .fetch_one(pool)
    .await
    .expect("tier");

    let email = format!("{handle}@voluntary-death.test");
    sqlx::query(
        r#"
        INSERT INTO users (email, password_hash, status_id, user_status_id, user_type_id, verification_tier_id, email_verified_at)
        VALUES (?, 'x', ?, ?, ?, ?, UTC_TIMESTAMP())
        "#,
    )
    .bind(&email)
    .bind(user_status)
    .bind(user_status)
    .bind(user_type)
    .bind(tier)
    .execute(pool)
    .await
    .expect("user");
    let user_id: u64 = sqlx::query_scalar("SELECT id FROM users WHERE email = ?")
        .bind(&email)
        .fetch_one(pool)
        .await
        .expect("user id");
    let owner_role: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = 'owner' LIMIT 1",
    )
    .bind(ref_keys::GROUP_USER_SOCIAL_ENTITY_ROLE)
    .fetch_one(pool)
    .await
    .expect("owner role");
    sqlx::query(
        r#"
        INSERT INTO user_social_entity_role (user_id, social_entity_id, role_id, created_by, modified_by)
        VALUES (?, ?, ?, ?, ?)
        "#,
    )
    .bind(user_id)
    .bind(entity_id)
    .bind(owner_role)
    .bind(system_user)
    .bind(system_user)
    .execute(pool)
    .await
    .expect("role");

    (entity_id, user_id)
}

#[tokio::test]
async fn human_voluntary_death_at_nonzero_rating() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let (human_id, user_id) = insert_test_human(pool, "graph_voluntary_human", "2.50").await;

    let _ = session::create_session(pool, user_id).await.expect("session");

    let mut tx = pool.begin().await.expect("tx");
    assert!(
        community_graph::human_death_voluntary_tx(&mut tx, human_id, user_id)
            .await
            .expect("voluntary death tx")
    );
    tx.commit().await.expect("commit");

    let rating: String = sqlx::query_scalar(
        r#"SELECT CAST(se.rating AS CHAR) FROM social_entity se WHERE se.id = ?"#,
    )
    .bind(human_id)
    .fetch_one(pool)
    .await
    .expect("rating");
    assert_eq!(rating, "0.00");

    let dead_key: Option<String> = sqlx::query_scalar(
        r#"
        SELECT rk.`key`
        FROM social_entity se
        INNER JOIN ref_key_values rk ON rk.id = se.entity_status_id AND rk.`group` = ?
        WHERE se.id = ?
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_STATUS)
    .bind(human_id)
    .fetch_optional(pool)
    .await
    .expect("status");
    assert_eq!(dead_key.as_deref(), Some(ref_keys::KEY_ENTITY_STATUS_DEAD));

    let death_source: Option<String> = sqlx::query_scalar(
        r#"
        SELECT rk.`key`
        FROM social_human sh
        INNER JOIN ref_key_values rk
            ON rk.id = sh.death_source_id
           AND rk.`group` = ?
        WHERE sh.social_entity_id = ?
        "#,
    )
    .bind(ref_keys::GROUP_GRAPH_DEATH_SOURCE)
    .bind(human_id)
    .fetch_optional(pool)
    .await
    .expect("death source");
    assert_eq!(death_source.as_deref(), Some(ref_keys::KEY_GRAPH_DEATH_SOURCE_USER_REQUESTED));

    let user_key: Option<String> = sqlx::query_scalar(
        r#"
        SELECT rk.`key`
        FROM users u
        INNER JOIN ref_key_values rk ON rk.id = u.user_status_id AND rk.`group` = ?
        WHERE u.id = ?
        "#,
    )
    .bind(ref_keys::GROUP_USER_STATUS)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .expect("user status");
    assert_eq!(user_key.as_deref(), Some(ref_keys::KEY_TO_BE_DELETED));

    let sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE user_id = ?")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("sessions");
    assert_eq!(sessions, 0);
}
