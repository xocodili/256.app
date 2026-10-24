//! `content_domain_lifecycle` blocks domains at rating ≤ 0.

mod common;

use app::content::domain_lifecycle::run_domain_lifecycle_batch;
use app::jobs::{enqueue_job, run_conductor_tick};
use app::config::Config;
use common::RefKeys;
use serde_json::json;

#[tokio::test]
async fn domain_lifecycle_blocks_zero_rated_domains() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let rk = RefKeys::load(pool).await;
    let pending = rk.id("status", "pending_verification");

    sqlx::query(
        r#"
        INSERT INTO content_domain (domain, status_id, rating, safety_state, first_seen_at, last_seen_at)
        VALUES ('zero-rated-lifecycle.test', ?, 0.00, 'safe', UTC_TIMESTAMP(6), UTC_TIMESTAMP(6))
        "#,
    )
    .bind(pending)
    .execute(pool)
    .await
    .expect("insert domain");

    let summary = run_domain_lifecycle_batch(pool, 8)
        .await
        .expect("batch");
    assert_eq!(summary.domains_blocked, 1);

    let status_key: String = sqlx::query_scalar(
        r#"
        SELECT rk.`key`
        FROM content_domain d
        INNER JOIN ref_key_values rk ON rk.id = d.status_id
        WHERE d.domain = 'zero-rated-lifecycle.test'
        "#,
    )
    .fetch_one(pool)
    .await
    .expect("status");
    assert_eq!(status_key, "blocked");
}

#[tokio::test]
async fn domain_lifecycle_job_type_runs_via_conductor() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let rk = RefKeys::load(pool).await;
    let pending = rk.id("status", "pending_verification");

    sqlx::query(
        r#"
        INSERT INTO content_domain (domain, status_id, rating, safety_state, first_seen_at, last_seen_at)
        VALUES ('zero-rated-queue.test', ?, 0.00, 'safe', UTC_TIMESTAMP(6), UTC_TIMESTAMP(6))
        "#,
    )
    .bind(pending)
    .execute(pool)
    .await
    .expect("insert");

    let enq = enqueue_job(
        pool,
        None,
        "content_domain_lifecycle",
        &json!({ "limit": 8 }),
        Some("test"),
        50,
    )
    .await
    .expect("enqueue");
    assert!(enq.job_id > 0);

    let config = Config::from_env().expect("config");
    let summary = run_conductor_tick(pool, &config, 8)
        .await
        .expect("conductor");
    assert!(summary.completed >= 1);
}
