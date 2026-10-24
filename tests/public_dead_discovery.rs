//! Dead entities must not appear on anonymous public discovery surfaces.

mod common;

use app::ref_keys;
use app::social::public_profile;
use common::TestDb;

#[tokio::test]
async fn public_profile_fetch_excludes_dead_entity() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let verified_status: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? LIMIT 1",
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_STATUS)
    .bind(ref_keys::KEY_VERIFIED)
    .fetch_one(pool)
    .await
    .expect("verified");
    let dead_status: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? LIMIT 1",
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_STATUS)
    .bind(ref_keys::KEY_ENTITY_STATUS_DEAD)
    .fetch_one(pool)
    .await
    .expect("dead");
    let community_type: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? LIMIT 1",
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(ref_keys::KEY_ENTITY_TYPE_COMMUNITY)
    .fetch_one(pool)
    .await
    .expect("community type");
    let join_open: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? LIMIT 1",
    )
    .bind(ref_keys::GROUP_SOCIAL_JOIN_POLICY)
    .bind(ref_keys::KEY_JOIN_POLICY_OPEN)
    .fetch_one(pool)
    .await
    .expect("join");
    let loc: u64 = sqlx::query_scalar("SELECT id FROM locations LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("loc");
    let system_user: u64 = sqlx::query_scalar("SELECT id FROM users WHERE email = ? LIMIT 1")
        .bind(ref_keys::SYSTEM_USER_EMAIL)
        .fetch_one(pool)
        .await
        .expect("system");

    sqlx::query(
        r#"
        INSERT INTO social_entity (handle, type_id, status_id, entity_status_id, location_id, rating, created_by, modified_by)
        VALUES ('public_dead_comm_x', ?, ?, ?, ?, 0.00, ?, ?)
        "#,
    )
    .bind(community_type)
    .bind(verified_status)
    .bind(verified_status)
    .bind(loc)
    .bind(system_user)
    .bind(system_user)
    .execute(pool)
    .await
    .expect("insert");
    sqlx::query(
        r#"
        INSERT INTO social_community (social_entity_id, label, join_policy_id)
        SELECT id, 'Public Dead Test', ? FROM social_entity WHERE handle = 'public_dead_comm_x'
        "#,
    )
    .bind(join_open)
    .execute(pool)
    .await
    .expect("community row");

    let alive = public_profile::fetch(pool, "public_dead_comm_x")
        .await
        .expect("fetch")
        .is_some();
    assert!(alive, "verified community should be public");

    sqlx::query(
        r#"
        UPDATE social_entity
        SET entity_status_id = ?, rating = 0.00, modified_by = ?
        WHERE handle = 'public_dead_comm_x'
        "#,
    )
    .bind(dead_status)
    .bind(system_user)
    .execute(pool)
    .await
    .expect("mark dead");

    let gone = public_profile::fetch(pool, "public_dead_comm_x")
        .await
        .expect("fetch dead");
    assert!(gone.is_none(), "dead community must not resolve on public profile fetch");
}
