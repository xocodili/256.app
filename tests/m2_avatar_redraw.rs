//! Module 2 — Avatar redraw · API-contract tier.
//!
//! Covers the in-feed face-capture card fragment and redraw POST gates.

mod common;

use axum::http::StatusCode;
use common::api::TestApp;

const PW: &str = "correct-horse-battery-staple";

/// Verified member with human profile (via `seed_full_member`) and face capture on file.
async fn seed_avatar_redraw_eligible(app: &TestApp, email: &str) -> (u64, String) {
    let uid = app.seed_full_member(email, PW).await;
    let sid = app.dev_session(email).await;
    app::social::user_faces::insert_face_capture(app.pool(), uid, None, "test-model", None, uid)
        .await
        .expect("insert face capture");
    (uid, sid)
}

#[tokio::test]
async fn face_capture_card_requires_session() {
    let app = TestApp::new().await;
    // JSON clients get 401; HTML GET would redirect to login instead.
    let resp = app.get("/home/face-capture-card?mode=avatar_redraw").await;
    assert_eq!(resp.status, StatusCode::UNAUTHORIZED);
    app.cleanup().await;
}

#[tokio::test]
async fn face_capture_card_rejects_unknown_mode() {
    let app = TestApp::new().await;
    let (_uid, sid) = seed_avatar_redraw_eligible(&app, "cardmode@example.com").await;
    let resp = app
        .get_auth_html("/home/face-capture-card?mode=onboarding", &sid)
        .await;
    assert_eq!(resp.status, StatusCode::NOT_FOUND);
    app.cleanup().await;
}

#[tokio::test]
async fn face_capture_card_avatar_redraw_returns_fragment() {
    let app = TestApp::new().await;
    let (_uid, sid) = seed_avatar_redraw_eligible(&app, "redrawcard@example.com").await;
    let resp = app
        .get_auth_html("/home/face-capture-card?mode=avatar_redraw", &sid)
        .await;
    assert_eq!(resp.status, StatusCode::OK, "{}", resp.text());
    let html = resp.text();
    assert!(
        html.contains(r#"data-face-capture-mode="avatar_redraw""#),
        "fragment must tag avatar redraw mode"
    );
    assert!(
        html.contains("face-capture-i18n"),
        "fragment must ship i18n JSON for face-capture.js"
    );
    assert!(
        html.contains("face-btn-start"),
        "fragment must include start camera control"
    );
    app.cleanup().await;
}

#[tokio::test]
async fn avatar_redraw_post_rejects_mock_mode() {
    let app = TestApp::new().await;
    let (_uid, sid) = seed_avatar_redraw_eligible(&app, "redrawmock@example.com").await;
    let boundary = "redraw-mock-test";
    let resp = app
        .post_multipart_auth(
            "/me/avatar-redraw",
            TestApp::empty_multipart_body(boundary),
            boundary,
            &sid,
        )
        .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);
    assert!(
        resp.text().to_lowercase().contains("mock"),
        "mock mode must block paid redraw: {}",
        resp.text()
    );
    app.cleanup().await;
}
