//! Module 1 — Public surface · Data tier.
//!
//! Asserts the *data* the public surface produces and depends on is correct:
//! registration persistence (deliverable vs canonical email), the unique-canonical
//! constraint that defeats duplicate-account vectors, and the disposable-domain
//! denylist seed. Uses the same in-process harness so seeding goes through the real
//! registration path where it matters.

mod common;

use common::api::TestApp;

const PW: &str = "correct-horse-battery-staple";

#[tokio::test]
async fn register_persists_deliverable_and_canonical_email() {
    let app = TestApp::new().await;

    let resp = app.register("Grace.Hopper+navy@Gmail.com", PW).await;
    assert_eq!(resp.status, axum::http::StatusCode::CREATED);

    let row: (String, Option<String>, i64) = sqlx::query_as(
        "SELECT email, email_canonical, (email_verified_at IS NULL) AS unverified
         FROM users WHERE email_canonical = ?",
    )
    .bind("gracehopper@gmail.com")
    .fetch_one(app.pool())
    .await
    .expect("registered user row");

    // Deliverable form is normalized (lowercased) but keeps dots and +tag.
    assert_eq!(row.0, "grace.hopper+navy@gmail.com");
    // Canonical form collapses Gmail dots + subaddressing for uniqueness.
    assert_eq!(row.1.as_deref(), Some("gracehopper@gmail.com"));
    // Fresh registration is unverified.
    assert_eq!(row.2, 1, "new account must start unverified");

    app.cleanup().await;
}

#[tokio::test]
async fn duplicate_vector_yields_single_account() {
    let app = TestApp::new().await;

    assert_eq!(
        app.register("ada@gmail.com", PW).await.status,
        axum::http::StatusCode::CREATED
    );
    assert_eq!(
        app.register("A.D.A+work@googlemail.com", PW).await.status,
        axum::http::StatusCode::CONFLICT
    );

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE email_canonical = ?")
            .bind("ada@gmail.com")
            .fetch_one(app.pool())
            .await
            .expect("count");
    assert_eq!(count, 1, "dup-vector registrations must collapse to one row");

    app.cleanup().await;
}

#[tokio::test]
async fn unique_canonical_index_blocks_direct_duplicate() {
    let app = TestApp::new().await;
    app.seed_verified_user("dupe@example.com", PW).await;

    // A second row with the same canonical must violate the unique index.
    let err = sqlx::query(
        "INSERT INTO users (email, email_canonical, password_hash, status_id, user_type_id, verification_tier_id)
         SELECT 'dupe2@example.com', 'dupe@example.com', password_hash, status_id, user_type_id, verification_tier_id
         FROM users WHERE email_canonical = 'dupe@example.com'",
    )
    .execute(app.pool())
    .await
    .expect_err("unique canonical index should reject the duplicate");

    assert!(
        app::social::db::is_unique_violation(&err),
        "expected a duplicate-key (unique constraint) violation, got: {err}"
    );

    app.cleanup().await;
}

#[tokio::test]
async fn disposable_denylist_is_seeded() {
    let app = TestApp::new().await;

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM blocked_email_domain")
        .fetch_one(app.pool())
        .await
        .expect("count blocked domains");
    assert!(total > 0, "migration should seed disposable domains");

    assert!(
        app::auth::email_address::domain_is_disposable(app.pool(), "mailinator.com")
            .await
            .expect("disposable check"),
        "mailinator.com must be on the denylist"
    );
    assert!(
        !app::auth::email_address::domain_is_disposable(app.pool(), "gmail.com")
            .await
            .expect("disposable check"),
        "gmail.com must not be on the denylist"
    );

    app.cleanup().await;
}
