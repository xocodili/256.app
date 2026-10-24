//! Module 1 — Public surface · API-contract tier.
//!
//! Exercises the public HTTP boundary the way a real client hits it: registration,
//! login, session issuance, the protected-route auth gate, and public page shells.
//! Drives the production router in-process via the harness in `common::api`.

mod common;

use common::api::TestApp;

const PW: &str = "correct-horse-battery-staple";

#[tokio::test]
async fn register_succeeds_and_returns_user_id() {
    let app = TestApp::new().await;

    let resp = app.register("Grace.Hopper+navy@Gmail.com", PW).await;

    assert_eq!(resp.status, axum::http::StatusCode::CREATED, "{}", resp.text());
    let body = resp.json();
    assert!(
        body.get("user_id").and_then(|v| v.as_str()).is_some(),
        "expected string user_id, got {body}"
    );

    app.cleanup().await;
}

#[tokio::test]
async fn register_rejects_duplicate_canonical_dup_vector() {
    let app = TestApp::new().await;

    let first = app.register("ada@gmail.com", PW).await;
    assert_eq!(first.status, axum::http::StatusCode::CREATED);

    // Same canonical address via Gmail dots + `+tag` + googlemail alias.
    let dup = app.register("A.D.A+work@googlemail.com", PW).await;
    assert_eq!(
        dup.status,
        axum::http::StatusCode::CONFLICT,
        "dup vector should collapse to one account: {}",
        dup.text()
    );

    app.cleanup().await;
}

#[tokio::test]
async fn register_rejects_disposable_domain() {
    let app = TestApp::new().await;

    let resp = app.register("throwaway@mailinator.com", PW).await;

    assert_eq!(resp.status, axum::http::StatusCode::BAD_REQUEST);
    assert!(
        resp.text().to_lowercase().contains("disposable"),
        "expected disposable-domain message, got {}",
        resp.text()
    );

    app.cleanup().await;
}

#[tokio::test]
async fn terms_consent_requires_both_acknowledgements() {
    let app = TestApp::new().await;
    let _uid = app.seed_verified_user("ack@example.com", PW).await;
    let sid = app.dev_session("ack@example.com").await;

    let no_constitution = app
        .post_json_auth(
            "/me/terms-consent",
            &serde_json::json!({ "adult_ack": true, "constitution_ack": false }),
            &sid,
        )
        .await;
    assert_eq!(no_constitution.status, axum::http::StatusCode::BAD_REQUEST);

    let no_adult = app
        .post_json_auth(
            "/me/terms-consent",
            &serde_json::json!({ "adult_ack": false, "constitution_ack": true }),
            &sid,
        )
        .await;
    assert_eq!(no_adult.status, axum::http::StatusCode::BAD_REQUEST);

    app.cleanup().await;
}

#[tokio::test]
async fn register_rejects_honeypot_and_invalid_email() {
    let app = TestApp::new().await;

    let honeypot = app
        .post_json(
            "/auth/register",
            &serde_json::json!({
                "email": "bot@example.com",
                "password": PW,
                "captcha_token": "test-bypass",
                "website": "http://spam.example",
            }),
        )
        .await;
    assert_eq!(honeypot.status, axum::http::StatusCode::BAD_REQUEST);

    let invalid = app.register("not-an-email", PW).await;
    assert_eq!(invalid.status, axum::http::StatusCode::BAD_REQUEST);

    app.cleanup().await;
}

#[tokio::test]
async fn login_rejects_wrong_password() {
    let app = TestApp::new().await;
    app.seed_verified_user("login-wrong@example.com", PW).await;

    let resp = app.login("login-wrong@example.com", "the-wrong-password").await;

    assert_eq!(resp.status, axum::http::StatusCode::UNAUTHORIZED);
    assert!(resp.session_cookie().is_none());

    app.cleanup().await;
}

#[tokio::test]
async fn login_rejects_unverified_user() {
    let app = TestApp::new().await;
    // Registration leaves the account pending (email not verified).
    assert_eq!(
        app.register("pending@example.com", PW).await.status,
        axum::http::StatusCode::CREATED
    );

    let resp = app.login("pending@example.com", PW).await;

    assert_eq!(
        resp.status,
        axum::http::StatusCode::FORBIDDEN,
        "unverified login should be forbidden: {}",
        resp.text()
    );

    app.cleanup().await;
}

#[tokio::test]
async fn login_issues_session_and_unlocks_protected_route() {
    let app = TestApp::new().await;
    app.seed_verified_user("verified@example.com", PW).await;

    let login = app.login("verified@example.com", PW).await;
    assert_eq!(login.status, axum::http::StatusCode::OK, "{}", login.text());
    let sid = login
        .session_cookie()
        .expect("login should set a session cookie");

    let authed = app.get_auth("/account", &sid).await;
    assert_eq!(authed.status, axum::http::StatusCode::OK);
    assert_eq!(authed.json()["ok"], serde_json::json!(true));

    app.cleanup().await;
}

#[tokio::test]
async fn protected_route_requires_session() {
    let app = TestApp::new().await;

    let no_cookie = app.get("/account").await;
    assert_eq!(no_cookie.status, axum::http::StatusCode::UNAUTHORIZED);
    assert_eq!(no_cookie.json()["code"], serde_json::json!("auth_required"));

    let bogus = app
        .get_auth("/account", "deadbeefdeadbeefdeadbeefdeadbeef")
        .await;
    assert_eq!(bogus.status, axum::http::StatusCode::UNAUTHORIZED);

    app.cleanup().await;
}

#[tokio::test]
async fn protected_html_routes_redirect_to_login() {
    let app = TestApp::new().await;

    for path in ["/home", "/account"] {
        let resp = app.get_html(path).await;
        assert_eq!(
            resp.status,
            axum::http::StatusCode::TEMPORARY_REDIRECT,
            "{path} should redirect HTML clients to login"
        );
        assert_eq!(
            resp.location().as_deref(),
            Some("/login?login=1"),
            "{path} redirect target"
        );
    }

    let json = app.get("/home").await;
    assert_eq!(json.status, axum::http::StatusCode::UNAUTHORIZED);
    assert_eq!(json.json()["code"], serde_json::json!("auth_required"));

    app.cleanup().await;
}

#[tokio::test]
async fn protected_html_routes_redirect_unverified_session_to_resend() {
    let app = TestApp::new().await;
    assert_eq!(
        app.register("pending-html@example.com", PW).await.status,
        axum::http::StatusCode::CREATED
    );

    let user_id: u64 = sqlx::query_scalar(
        "SELECT id FROM users WHERE email_canonical = ? LIMIT 1",
    )
    .bind("pending-html@example.com")
    .fetch_one(app.pool())
    .await
    .expect("registered user");

    let token = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
    sqlx::query(
        r#"INSERT INTO sessions (user_id, session_token, password_reset_only, expires_at, created_by, modified_by)
           VALUES (?, ?, 0, UTC_TIMESTAMP() + INTERVAL 1 DAY, ?, ?)"#,
    )
    .bind(user_id)
    .bind(token)
    .bind(user_id)
    .bind(user_id)
    .execute(app.pool())
    .await
    .expect("session insert");

    let resp = app.get_auth_html("/home", token).await;
    assert_eq!(resp.status, axum::http::StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(resp.location().as_deref(), Some("/login?resend=1"));

    let json = app.get_auth("/home", token).await;
    assert_eq!(json.status, axum::http::StatusCode::FORBIDDEN);
    assert_eq!(json.json()["code"], serde_json::json!("email_not_verified"));

    app.cleanup().await;
}

#[tokio::test]
async fn public_pages_render_for_guests() {
    let app = TestApp::new().await;

    let login = app.get("/login").await;
    assert_eq!(login.status, axum::http::StatusCode::OK);
    let login_html = login.text();
    assert!(
        login_html.contains("guest-split-auth"),
        "login shell should render the guest auth split"
    );
    assert!(
        login_html.contains("guest-welcome-benefits"),
        "login shell should render foreground benefit tiles"
    );
    assert!(
        login_html.contains("guest-pantheon-card"),
        "login shell should render the council pantheon card"
    );
    assert!(
        login_html.contains("Socials done right"),
        "login shell should render the welcome headline"
    );
    assert!(
        login_html.contains("Twelve AI stewards assist each community."),
        "login shell should render the pantheon title"
    );

    // Legal routes are canonicalized onto the login shell's legal card.
    for (path, legal) in [
        ("/terms", "terms"),
        ("/privacy", "privacy"),
        ("/conditions", "terms"),
        ("/safety", "safety"),
    ] {
        let resp = app.get(path).await;
        assert_eq!(
            resp.status,
            axum::http::StatusCode::TEMPORARY_REDIRECT,
            "{path} should redirect to the login legal card"
        );
        assert_eq!(
            resp.location().as_deref(),
            Some(format!("/login?legal={legal}").as_str()),
            "{path} should point at its legal card"
        );
    }

    let health = app.get("/health/status").await;
    assert_eq!(health.status, axum::http::StatusCode::OK);

    app.cleanup().await;
}
