//! Account security: password change and session rotation.

mod common;

use app::auth::{account, password, session};
use app::ref_keys;
use common::TestDb;
use sqlx::MySqlPool;

async fn insert_verified_user(pool: &MySqlPool, email: &str, password_plain: &str) -> u64 {
    let hash = password::hash_password(password_plain).expect("hash password");
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

    sqlx::query(
        r#"
        INSERT INTO users (email, password_hash, status_id, user_status_id, user_type_id, verification_tier_id, email_verified_at)
        VALUES (?, ?, ?, ?, ?, ?, UTC_TIMESTAMP())
        "#,
    )
    .bind(email)
    .bind(hash.as_bytes())
    .bind(user_status)
    .bind(user_status)
    .bind(user_type)
    .bind(tier)
    .execute(pool)
    .await
    .expect("insert user");

    sqlx::query_scalar("SELECT id FROM users WHERE email = ?")
        .bind(email)
        .fetch_one(pool)
        .await
        .expect("user id")
}

#[tokio::test]
async fn change_password_updates_hash_and_rejects_old_password() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let user_id = insert_verified_user(pool, "pw-change@256.test", "old-password-12").await;

    account::change_user_password(pool, user_id, "old-password-12", "new-password-12")
        .await
        .expect("change password");

    assert!(
        !account::verify_user_password(pool, user_id, "old-password-12")
            .await
            .expect("verify old")
    );
    assert!(
        account::verify_user_password(pool, user_id, "new-password-12")
            .await
            .expect("verify new")
    );
}

#[tokio::test]
async fn rotate_session_after_credential_change_revokes_old_sessions() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let user_id = insert_verified_user(pool, "session-rotate@256.test", "password-twelve").await;

    session::create_session(pool, user_id).await.expect("session 1");
    session::create_session(pool, user_id).await.expect("session 2");

    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE user_id = ?")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("count before");
    assert_eq!(before, 2);

    let cookie = session::rotate_session_after_credential_change(pool, user_id, false)
        .await
        .expect("rotate");
    assert!(!cookie.value().is_empty());

    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE user_id = ?")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("count after");
    assert_eq!(after, 1);
}

#[tokio::test]
async fn password_reset_token_issue_and_consume() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let user_id = insert_verified_user(pool, "reset-flow@256.test", "password-twelve").await;

    let token = app::auth::password_reset::issue_password_reset_token(pool, user_id)
        .await
        .expect("issue token");
    let outcome = app::auth::password_reset::consume_password_reset_token(pool, &token)
        .await
        .expect("consume");
    assert_eq!(
        outcome,
        app::auth::password_reset::ResetConsumeOutcome::Valid(user_id)
    );

    let again = app::auth::password_reset::consume_password_reset_token(pool, &token)
        .await
        .expect("consume again");
    assert_eq!(
        again,
        app::auth::password_reset::ResetConsumeOutcome::Invalid
    );
}

#[tokio::test]
async fn password_reset_session_is_reset_only_until_completion() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let user_id = insert_verified_user(pool, "reset-session@256.test", "password-twelve").await;

    let cookie = session::start_password_reset_session(pool, user_id, false)
        .await
        .expect("reset session");
    let token = cookie.value();
    let reset_only: Option<bool> = sqlx::query_scalar(
        "SELECT password_reset_only FROM sessions WHERE session_token = ?",
    )
    .bind(token)
    .fetch_optional(pool)
    .await
    .expect("lookup")
    .flatten();
    assert_eq!(reset_only, Some(true));

    let (_, _) = session::complete_password_reset_session(pool, user_id, false)
        .await
        .expect("complete reset");
    let reset_only_after: Option<bool> = sqlx::query_scalar(
        "SELECT password_reset_only FROM sessions WHERE user_id = ? LIMIT 1",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .expect("row");
    assert_eq!(reset_only_after, Some(false));
}

#[tokio::test]
async fn set_user_password_rejects_unchanged_password() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let user_id = insert_verified_user(pool, "same-pw@256.test", "password-twelve").await;

    let err = account::set_user_password(pool, user_id, "password-twelve")
        .await
        .expect_err("same password");
    assert!(matches!(err, app::error::AppError::BadRequest(_)));
}
