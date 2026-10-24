//! Integration test: `scope_recalc_drain` aligns `entity_read_scope` with `vw_entity_read_scope_live`.
//!
//! Requires `TEST_DATABASE_URL` or `DATABASE_URL` (see `tests/common`).

mod common;

use app::config::Config;
use app::jobs::{enqueue_job, run_conductor_tick};
use app::social::scope;
use common::RefKeys;
use serde_json::json;

#[tokio::test]
async fn scope_recalc_drain_matches_live_view_for_queued_subjects() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;

    // Bootstrap migration queues every existing entity; clear so this test is deterministic.
    sqlx::query("DELETE FROM entity_scope_recalc_queue")
        .execute(pool)
        .await
        .expect("clear scope recalc queue");
    sqlx::query("DELETE FROM entity_read_scope")
        .execute(pool)
        .await
        .expect("clear materialized read scope");

    let rk = RefKeys::load(pool).await;
    let location_id: u64 = sqlx::query_scalar(
        "SELECT id FROM locations ORDER BY id ASC LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .expect("location id");

    let account_status_id = rk.id("status", "verified");
    let user_status_id = rk.id("user_status", "verified");
    let user_type_id = rk.id("user_type", "human");
    let tier_id = rk.id("verification_tier", "basic");

    sqlx::query(
        r#"
        INSERT INTO users (
            email, password_hash, email_verified_at, status_id, user_status_id, user_type_id, verification_tier_id
        ) VALUES ('scope.recalc.test@256.test', NULL, UTC_TIMESTAMP(), ?, ?, ?, ?)
        "#,
    )
    .bind(account_status_id)
    .bind(user_status_id)
    .bind(user_type_id)
    .bind(tier_id)
    .execute(pool)
    .await
    .expect("insert user");

    let uid: u64 = sqlx::query_scalar(
        "SELECT id FROM users WHERE email = 'scope.recalc.test@256.test' LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .expect("user id");

    let human_type = rk.id("social_entity_type", "human");
    let status_id = rk.id("status", "verified");
    let entity_status_id = rk.id("social_entity_status", "verified");

    for handle in ["scope_a", "scope_b"] {
        sqlx::query(
            r#"
            INSERT INTO social_entity (
                handle, type_id, status_id, entity_status_id, location_id, rating, created_by, modified_by
            ) VALUES (?, ?, ?, ?, ?, '2.50', ?, ?)
            "#,
        )
        .bind(handle)
        .bind(human_type)
        .bind(status_id)
        .bind(entity_status_id)
        .bind(location_id)
        .bind(uid)
        .bind(uid)
        .execute(pool)
        .await
        .expect("insert social_entity");
    }

    let ea: u64 = sqlx::query_scalar(
        "SELECT id FROM social_entity WHERE handle = 'scope_a' LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .expect("entity a");
    let eb: u64 = sqlx::query_scalar(
        "SELECT id FROM social_entity WHERE handle = 'scope_b' LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .expect("entity b");

    let friend_type = rk.id("social_entity_relationship_type", "friend");
    let rel_status_id = rk.id("social_relationship_status", "verified");

    sqlx::query(
        r#"
        INSERT INTO social_entity_relationship (
            from_social_entity_id, to_social_entity_id, type_id, status_id, relationship_status_id, started_at
        ) VALUES (?, ?, ?, ?, ?, UTC_TIMESTAMP(6))
        "#,
    )
    .bind(ea)
    .bind(eb)
    .bind(friend_type)
    .bind(status_id)
    .bind(rel_status_id)
    .execute(pool)
    .await
    .expect("insert relationship");

    scope::mark_entity_dirty(pool, ea, "relationship_changed")
        .await
        .expect("mark ea dirty");
    scope::mark_entity_dirty(pool, eb, "relationship_changed")
        .await
        .expect("mark eb dirty");

    let count_before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM entity_read_scope WHERE subject_entity_id IN (?, ?)",
    )
    .bind(ea)
    .bind(eb)
    .fetch_one(pool)
    .await
    .expect("count before");
    assert_eq!(count_before, 0);

    let enq = enqueue_job(
        pool,
        None,
        "scope_recalc_drain",
        &json!({ "limit": 32 }),
        Some("scope-recalc-test"),
        50,
    )
    .await
    .expect("enqueue scope recalc drain");
    assert!(
        enq.job_id > 0,
        "scope_recalc_drain job type must exist in ref_key_values (ops_job_type)"
    );

    let config = Config::from_env().expect("config from env");
    let summary = run_conductor_tick(pool, &config, 8)
        .await
        .expect("conductor tick");
    assert!(
        summary.completed >= 1,
        "expected at least one completed job, got {summary:?}"
    );

    for subject in [ea, eb] {
        let live: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM vw_entity_read_scope_live WHERE subject_entity_id = ?",
        )
        .bind(subject)
        .fetch_one(pool)
        .await
        .expect("live count");

        let mat: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM entity_read_scope WHERE subject_entity_id = ?",
        )
        .bind(subject)
        .fetch_one(pool)
        .await
        .expect("materialized count");

        assert_eq!(
            mat, live,
            "subject {subject}: materialized row count should match live view"
        );
    }

    let queue_remaining: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM entity_scope_recalc_queue WHERE subject_entity_id IN (?, ?)",
    )
    .bind(ea)
    .bind(eb)
    .fetch_one(pool)
    .await
    .expect("queue remaining");
    assert_eq!(queue_remaining, 0);

    db.cleanup().await;
}
