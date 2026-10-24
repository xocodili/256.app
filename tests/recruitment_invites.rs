//! Platform recruitment invites — permissions and bulk API contract.

mod common;

use axum::http::StatusCode;
use common::api::TestApp;

const PW: &str = "correct-horse-battery-staple";

async fn entity_type_id(app: &TestApp, key: &str) -> String {
    let id: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = 'social_entity_type' AND `key` = ? LIMIT 1",
    )
    .bind(key)
    .fetch_one(app.pool())
    .await
    .expect("entity type id");
    id.to_string()
}

async fn create_non_profit(app: &TestApp, sid: &str, loc: u64, handle: &str) -> String {
    let type_id = entity_type_id(app, "non_profit").await;
    let body = serde_json::json!({
        "handle": handle,
        "type_id": type_id,
        "location_id": loc.to_string(),
        "non_profit": {
            "label": "River Keepers",
            "legal_name": "River Keepers Trust",
            "founded_at": "2015-06-01",
        },
    });
    let resp = app
        .post_json_auth("/social-entities", &body, sid)
        .await;
    assert_eq!(resp.status, StatusCode::CREATED, "{}", resp.text());
    handle.to_string()
}

#[tokio::test]
async fn bulk_recruitment_rejects_empty_list() {
    let app = TestApp::new().await;
    app.seed_full_member("owner@example.com", PW).await;
    let sid = app.dev_session("owner@example.com").await;
    let loc = app.seed_location().await;
    let handle = create_non_profit(&app, &sid, loc, "bulk_empty").await;

    let resp = app
        .post_json_csrf(
            &format!("/me/entities/{handle}/recruitment-invites"),
            &serde_json::json!({ "emails": [], "emails_text": "   " }),
            &sid,
        )
        .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST, "{}", resp.text());

    app.cleanup().await;
}

#[tokio::test]
async fn bulk_recruitment_requires_owner() {
    let app = TestApp::new().await;
    app.seed_full_member("owner@example.com", PW).await;
    app.seed_full_member("stranger@example.com", PW).await;
    let owner_sid = app.dev_session("owner@example.com").await;
    let stranger_sid = app.dev_session("stranger@example.com").await;
    let loc = app.seed_location().await;
    let handle = create_non_profit(&app, &owner_sid, loc, "bulk_perm").await;

    let resp = app
        .post_json_csrf(
            &format!("/me/entities/{handle}/recruitment-invites"),
            &serde_json::json!({ "emails": [], "emails_text": "client@example.com" }),
            &stranger_sid,
        )
        .await;
    assert_eq!(resp.status, StatusCode::NOT_FOUND, "{}", resp.text());

    app.cleanup().await;
}

#[tokio::test]
async fn bulk_recruitment_without_smtp_is_bad_request() {
    let app = TestApp::new().await;
    app.seed_full_member("owner@example.com", PW).await;
    let sid = app.dev_session("owner@example.com").await;
    let loc = app.seed_location().await;
    let handle = create_non_profit(&app, &sid, loc, "bulk_smtp").await;

    let resp = app
        .post_json_csrf(
            &format!("/me/entities/{handle}/recruitment-invites"),
            &serde_json::json!({ "emails": [], "emails_text": "client@example.com" }),
            &sid,
        )
        .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST, "{}", resp.text());
    assert!(
        resp.text().contains("smtp") || resp.text().contains("configured"),
        "expected smtp-unconfigured message: {}",
        resp.text()
    );

    app.cleanup().await;
}

#[tokio::test]
async fn owner_profile_card_includes_recruitment_block() {
    let app = TestApp::new().await;
    app.seed_full_member("owner@example.com", PW).await;
    let sid = app.dev_session("owner@example.com").await;
    let loc = app.seed_location().await;
    let handle = create_non_profit(&app, &sid, loc, "bulk_ui").await;

    let resp = app
        .get_auth(&format!("/home/private-profile-card/{handle}"), &sid)
        .await;
    assert_eq!(resp.status, StatusCode::OK, "{}", resp.text());
    let html = resp.text();
    assert!(
        html.contains("profile-recruitment-invite-block"),
        "owner profit/non-profit card should expose recruitment invite UI"
    );
    assert!(html.contains("profile-recruitment-invite-emails"));

    app.cleanup().await;
}

async fn ensure_human_entity(app: &TestApp, user_id: u64, handle: &str) -> u64 {
    if let Ok(Some(entity_id)) =
        app::social::human_onboarding::primary_human_entity_id(app.pool(), user_id).await
    {
        sqlx::query("UPDATE social_entity SET handle = ? WHERE id = ?")
            .bind(handle)
            .bind(entity_id)
            .execute(app.pool())
            .await
            .expect("update existing human handle");
        return entity_id;
    }

    let loc = app.seed_location().await;
    let type_id: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = 'social_entity_type' AND `key` = 'human' LIMIT 1",
    )
    .fetch_one(app.pool())
    .await
    .expect("human type");
    let status_id: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = 'status' AND `key` = 'verified' LIMIT 1",
    )
    .fetch_one(app.pool())
    .await
    .expect("verified status");
    let entity_status_id: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = 'social_entity_status' AND `key` = 'verified' LIMIT 1",
    )
    .fetch_one(app.pool())
    .await
    .expect("entity verified");
    let join_open: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = 'social_join_policy' AND `key` = 'open' LIMIT 1",
    )
    .fetch_one(app.pool())
    .await
    .expect("join open");

    let entity_id = sqlx::query(
        r#"
        INSERT INTO social_entity (handle, type_id, status_id, entity_status_id, location_id, created_by, rating)
        VALUES (?, ?, ?, ?, ?, ?, 2.50)
        "#,
    )
    .bind(handle)
    .bind(type_id)
    .bind(status_id)
    .bind(entity_status_id)
    .bind(loc)
    .bind(user_id)
    .execute(app.pool())
    .await
    .expect("insert human")
    .last_insert_id();

    sqlx::query(
        r#"
        INSERT INTO social_human (social_entity_id, family_name, given_name, birth_date, birth_location_id, join_policy_id)
        VALUES (?, 'Invite', 'Tester', '1990-01-01', ?, ?)
        "#,
    )
    .bind(entity_id)
    .bind(loc)
    .bind(join_open)
    .execute(app.pool())
    .await
    .expect("insert social_human");

    let owner_role: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = 'user_social_entity_role' AND `key` = 'owner' LIMIT 1",
    )
    .fetch_one(app.pool())
    .await
    .expect("owner role");
    sqlx::query(
        r#"
        INSERT INTO user_social_entity_role (user_id, social_entity_id, role_id, is_primary, starts_at)
        VALUES (?, ?, ?, 1, UTC_TIMESTAMP())
        "#,
    )
    .bind(user_id)
    .bind(entity_id)
    .bind(owner_role)
    .execute(app.pool())
    .await
    .expect("owner role row");

    entity_id
}

#[tokio::test]
async fn friend_invite_returns_handle_for_verified_member() {
    let app = TestApp::new().await;
    app.seed_full_member("inviter@example.com", PW).await;
    app.seed_full_member("peer@example.com", PW).await;
    let inviter_sid = app.dev_session("inviter@example.com").await;
    let inviter_user = sqlx::query_scalar("SELECT id FROM users WHERE email = ?")
        .bind("inviter@example.com")
        .fetch_one(app.pool())
        .await
        .expect("inviter user");
    let peer_user = sqlx::query_scalar("SELECT id FROM users WHERE email = ?")
        .bind("peer@example.com")
        .fetch_one(app.pool())
        .await
        .expect("peer user");
    ensure_human_entity(&app, inviter_user, "inviter_member").await;
    ensure_human_entity(&app, peer_user, "peer_member").await;

    let resp = app
        .post_json_csrf(
            "/me/friend-invite",
            &serde_json::json!({ "email": "peer@example.com", "personal_message": "" }),
            &inviter_sid,
        )
        .await;
    assert_eq!(resp.status, StatusCode::OK, "{}", resp.text());
    let json = resp.json();
    assert_eq!(json["ok"], true);
    assert_eq!(json["already_member"], true);
    assert_eq!(json["handle"].as_str(), Some("@peer_member"));

    app.cleanup().await;
}

#[tokio::test]
async fn friend_invite_rejects_registered_unverified_email() {
    let app = TestApp::new().await;
    app.seed_full_member("inviter2@example.com", PW).await;
    app.seed_verified_user("pending@example.com", PW).await;
    let inviter_sid = app.dev_session("inviter2@example.com").await;

    let resp = app
        .post_json_csrf(
            "/me/friend-invite",
            &serde_json::json!({ "email": "pending@example.com" }),
            &inviter_sid,
        )
        .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST, "{}", resp.text());

    app.cleanup().await;
}
