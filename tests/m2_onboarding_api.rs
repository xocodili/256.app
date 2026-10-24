//! Module 2 — Onboarding · API-contract tier.
//!
//! Exercises the onboarding HTTP surface: the auth gate, consent → face → profile
//! routing for not-yet-verified users, human-profile creation + validation, and the
//! godmother step's prerequisite gates — all through the real router in-process.

mod common;

use axum::http::StatusCode;
use common::api::TestApp;

const PW: &str = "correct-horse-battery-staple";

fn valid_profile_body(handle: &str) -> serde_json::Value {
    serde_json::json!({
        "handle": handle,
        "given_name": "Ada",
        "family_name": "Lovelace",
        "birth_date": "1990-01-01",
        "country_code": "AU",
    })
}

#[tokio::test]
async fn onboarding_endpoints_require_session() {
    let app = TestApp::new().await;

    assert_eq!(app.get("/onboarding").await.status, StatusCode::UNAUTHORIZED);

    let no_session = app.post_json("/me/human-profile", &valid_profile_body("nobody")).await;
    assert_eq!(no_session.status, StatusCode::UNAUTHORIZED);

    app.cleanup().await;
}

#[tokio::test]
async fn fresh_user_home_redirects_to_consent() {
    let app = TestApp::new().await;
    app.seed_verified_user("fresh@example.com", PW).await;
    let sid = app.dev_session("fresh@example.com").await;

    let home = app.get_auth("/home", &sid).await;
    assert_eq!(home.status, StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        home.location().as_deref(),
        Some("/home?onboarding=consent")
    );

    app.cleanup().await;
}

#[tokio::test]
async fn consent_shell_renders_on_home_with_onboarding_query() {
    let app = TestApp::new().await;
    app.seed_verified_user("consent-shell@example.com", PW).await;
    let sid = app.dev_session("consent-shell@example.com").await;

    let home = app.get_auth("/home?onboarding=consent", &sid).await;
    assert_eq!(home.status, StatusCode::OK, "{}", home.text());
    let body = home.text();
    assert!(body.contains(r#"data-needs-consent="1""#));
    assert!(body.contains(r#"data-face-capture-ready="0""#));
    assert!(body.contains(r#"id="app-home-consent-panel""#));
    assert!(
        !body.contains("window.location.replace(\"/onboarding\")"),
        "home shell must not client-redirect away from consent"
    );

    app.cleanup().await;
}

#[tokio::test]
async fn onboarding_gate_redirects_face_capture_until_consent() {
    let app = TestApp::new().await;
    app.seed_verified_user("gate-consent@example.com", PW).await;
    let sid = app.dev_session("gate-consent@example.com").await;

    let onboarding = app.get_auth("/onboarding", &sid).await;
    assert_eq!(onboarding.status, StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        onboarding.location().as_deref(),
        Some("/home?onboarding=consent")
    );

    let plain_home = app.get_auth("/home", &sid).await;
    assert_eq!(plain_home.status, StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        plain_home.location().as_deref(),
        Some("/home?onboarding=consent")
    );

    app.cleanup().await;
}

#[tokio::test]
async fn onboarding_page_redirects_without_consent() {
    let app = TestApp::new().await;
    app.seed_verified_user("capture@example.com", PW).await;
    let sid = app.dev_session("capture@example.com").await;

    let page = app.get_auth("/onboarding", &sid).await;
    assert_eq!(page.status, StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        page.location().as_deref(),
        Some("/home?onboarding=consent")
    );

    app.cleanup().await;
}

#[tokio::test]
async fn home_with_onboarding_query_flags_needs_profile() {
    let app = TestApp::new().await;
    let uid = app.seed_verified_user("needsprofile@example.com", PW).await;
    let sid = app.dev_session("needsprofile@example.com").await;
    assert_eq!(
        app.accept_terms_consent(&sid).await.status,
        StatusCode::OK
    );

    let blocked = app.get_auth("/home?onboarding=profile", &sid).await;
    assert_eq!(blocked.status, StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(blocked.location().as_deref(), Some("/onboarding"));

    app::social::user_faces::insert_face_capture(app.pool(), uid, None, "test-model", None, uid)
        .await
        .expect("insert face capture");

    let home = app.get_auth("/home?onboarding=profile", &sid).await;
    assert_eq!(home.status, StatusCode::OK);
    assert!(
        home.text().contains(r#"data-needs-profile="1""#),
        "home shell should flag that a profile is still required"
    );

    app.cleanup().await;
}

#[tokio::test]
async fn create_profile_succeeds_then_stays_in_onboarding() {
    let app = TestApp::new().await;
    app.seed_verified_user("builder@example.com", PW).await;
    let sid = app.dev_session("builder@example.com").await;
    assert_eq!(
        app.accept_terms_consent(&sid).await.status,
        StatusCode::OK
    );

    let created = app
        .post_json_auth("/me/human-profile", &valid_profile_body("buildhandle"), &sid)
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.text());

    let home = app.get_auth("/home", &sid).await;
    assert_eq!(home.status, StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(home.location().as_deref(), Some("/onboarding"));

    app.cleanup().await;
}

#[tokio::test]
async fn create_profile_rejects_underage() {
    let app = TestApp::new().await;
    app.seed_verified_user("young@example.com", PW).await;
    let sid = app.dev_session("young@example.com").await;
    assert_eq!(
        app.accept_terms_consent(&sid).await.status,
        StatusCode::OK
    );

    let mut body = valid_profile_body("younghandle");
    body["birth_date"] = serde_json::json!("2015-01-01");
    let resp = app.post_json_auth("/me/human-profile", &body, &sid).await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST, "{}", resp.text());

    app.cleanup().await;
}

#[tokio::test]
async fn duplicate_profile_conflicts() {
    let app = TestApp::new().await;
    app.seed_verified_user("twice@example.com", PW).await;
    let sid = app.dev_session("twice@example.com").await;
    assert_eq!(
        app.accept_terms_consent(&sid).await.status,
        StatusCode::OK
    );

    let first = app
        .post_json_auth("/me/human-profile", &valid_profile_body("firsthandle"), &sid)
        .await;
    assert_eq!(first.status, StatusCode::CREATED);

    let second = app
        .post_json_auth("/me/human-profile", &valid_profile_body("secondhandle"), &sid)
        .await;
    assert_eq!(
        second.status,
        StatusCode::CONFLICT,
        "a user may only complete one human profile: {}",
        second.text()
    );

    app.cleanup().await;
}

#[tokio::test]
async fn godmother_requires_profile_then_face_capture() {
    let app = TestApp::new().await;
    let uid = app.seed_verified_user("godm@example.com", PW).await;
    let sid = app.dev_session("godm@example.com").await;
    assert_eq!(
        app.accept_terms_consent(&sid).await.status,
        StatusCode::OK
    );

    let no_profile = app
        .post_json_auth("/me/godmother", &serde_json::json!({ "handle": "epic" }), &sid)
        .await;
    assert_eq!(no_profile.status, StatusCode::BAD_REQUEST);
    assert!(no_profile.text().to_lowercase().contains("human profile"));

    let created = app
        .post_json_auth("/me/human-profile", &valid_profile_body("godmhandle"), &sid)
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.text());

    let no_face = app
        .post_json_auth("/me/godmother", &serde_json::json!({ "handle": "epic" }), &sid)
        .await;
    assert_eq!(no_face.status, StatusCode::BAD_REQUEST);
    assert!(no_face.text().to_lowercase().contains("face capture"));

    app::social::user_faces::insert_face_capture(app.pool(), uid, None, "test-model", None, uid)
        .await
        .expect("insert face capture");
    let bad_handle = app
        .post_json_auth(
            "/me/godmother",
            &serde_json::json!({ "handle": "definitely-not-a-real-godmother" }),
            &sid,
        )
        .await;
    assert_eq!(bad_handle.status, StatusCode::BAD_REQUEST);
    let msg = bad_handle.text().to_lowercase();
    assert!(
        !msg.contains("human profile") && !msg.contains("face capture"),
        "prerequisite gates should now pass; got: {msg}"
    );

    app.cleanup().await;
}

#[tokio::test]
async fn godmother_options_returns_council_json() {
    let app = TestApp::new().await;
    app.seed_verified_user("council@example.com", PW).await;
    let sid = app.dev_session("council@example.com").await;

    let resp = app.get_auth("/me/godmother-options", &sid).await;
    assert_eq!(
        resp.status,
        StatusCode::OK,
        "godmother-options failed: {}",
        resp.text()
    );
    let data = resp.json();
    let arr = data.as_array().expect("godmother-options must be a JSON array");
    assert!(
        !arr.is_empty(),
        "migrated seed data should expose at least one AI council moderator"
    );
    assert!(arr[0].get("handle").is_some());
    assert!(arr[0].get("community_label").is_some());

    app.cleanup().await;
}
