//! Module 2 — Onboarding · Data tier.
//!
//! Asserts the data/state machine that drives onboarding: terms-consent via
//! `/me/terms-consent`, human-profile presence, and the face-capture **completion signal**.

mod common;

use common::api::TestApp;

const PW: &str = "correct-horse-battery-staple";

fn user_id_of(resp: &common::api::TestResponse) -> u64 {
    resp.json()["user_id"]
        .as_str()
        .expect("user_id string")
        .parse()
        .expect("user_id parses")
}

#[tokio::test]
async fn terms_consent_recorded_via_onboarding_endpoint() {
    let app = TestApp::new().await;
    let uid = app.seed_verified_user("consenting@example.com", PW).await;
    let sid = app.dev_session("consenting@example.com").await;

    assert!(
        !app::social::human_onboarding::user_has_terms_consent(app.pool(), uid)
            .await
            .expect("consent check"),
        "fresh verified user should not have terms consent yet"
    );

    let resp = app.accept_terms_consent(&sid).await;
    assert_eq!(resp.status, axum::http::StatusCode::OK, "{}", resp.text());
    assert!(
        app::social::human_onboarding::user_has_terms_consent(app.pool(), uid)
            .await
            .expect("consent check"),
        "terms consent endpoint should record current version"
    );

    app::social::human_onboarding::record_terms_consent(app.pool(), uid)
        .await
        .expect("idempotent consent");
    assert!(
        app::social::human_onboarding::user_has_terms_consent(app.pool(), uid)
            .await
            .expect("consent check")
    );

    app.cleanup().await;
}

#[tokio::test]
async fn human_profile_presence_transitions() {
    let app = TestApp::new().await;
    app.seed_verified_user("profile@example.com", PW).await;
    let sid = app.dev_session("profile@example.com").await;

    let uid: u64 = sqlx::query_scalar("SELECT id FROM users WHERE email_canonical = ?")
        .bind("profile@example.com")
        .fetch_one(app.pool())
        .await
        .expect("user id");

    assert!(
        !app::social::human_onboarding::user_has_human_profile(app.pool(), uid)
            .await
            .expect("profile check"),
        "fresh user has no human profile"
    );

    let consent = app.accept_terms_consent(&sid).await;
    assert_eq!(consent.status, axum::http::StatusCode::OK, "{}", consent.text());

    let created = app
        .post_json_auth(
            "/me/human-profile",
            &serde_json::json!({
                "handle": "adalovelace",
                "given_name": "Ada",
                "family_name": "Lovelace",
                "birth_date": "1990-01-01",
                "country_code": "AU",
            }),
            &sid,
        )
        .await;
    assert_eq!(
        created.status,
        axum::http::StatusCode::CREATED,
        "profile create failed: {}",
        created.text()
    );

    assert!(
        app::social::human_onboarding::user_has_human_profile(app.pool(), uid)
            .await
            .expect("profile check"),
        "profile should exist after creation"
    );
    assert!(
        app::social::human_onboarding::primary_human_entity_id(app.pool(), uid)
            .await
            .expect("primary entity")
            .is_some()
    );

    let (given, family): (String, String) = sqlx::query_as(
        r#"SELECT sh.given_name, sh.family_name
           FROM social_human sh
           INNER JOIN social_entity se ON se.id = sh.social_entity_id
           WHERE se.created_by = ?"#,
    )
    .bind(uid)
    .fetch_one(app.pool())
    .await
    .expect("social_human row");
    assert_eq!((given.as_str(), family.as_str()), ("Ada", "Lovelace"));

    let noms: u16 = sqlx::query_scalar(
        r#"SELECT sh.ai_tokens
           FROM social_human sh
           INNER JOIN social_entity se ON se.id = sh.social_entity_id
           WHERE se.created_by = ?"#,
    )
    .bind(uid)
    .fetch_one(app.pool())
    .await
    .expect("signup nom balance");
    assert_eq!(
        noms,
        app::social::noms::SIGNUP_NOM_GRANT,
        "new human profiles should receive the signup Nom grant"
    );

    app.cleanup().await;
}

#[tokio::test]
async fn face_capture_completion_signal_decoupled_from_image() {
    let app = TestApp::new().await;
    let uid = app.seed_verified_user("face@example.com", PW).await;

    assert!(
        !app::social::user_faces::user_has_face_capture(app.pool(), uid)
            .await
            .expect("face check"),
        "fresh user has no face capture"
    );

    // Completion can be reached with NO stored artwork — the signal is the timestamp,
    // never a biometric encoding.
    let id = app::social::user_faces::insert_face_capture(app.pool(), uid, None, "test-model", None, uid)
        .await
        .expect("insert face capture");
    assert!(id > 0);
    assert!(
        app::social::user_faces::user_has_face_capture(app.pool(), uid)
            .await
            .expect("face check"),
        "face_captured_at should mark completion even without artwork"
    );
    assert!(
        app::social::user_faces::artwork_path_for_user(app.pool(), uid)
            .await
            .expect("artwork")
            .is_none(),
        "no artwork stored yet"
    );

    // Upsert with an artwork path updates the durable (non-biometric) artifact.
    let artwork_rel = "static/generated/user-faces/face.png";
    tokio::fs::create_dir_all("static/generated/user-faces")
        .await
        .expect("face art dir");
    tokio::fs::write(artwork_rel, b"\x89PNG\r\n\x1a\n")
        .await
        .expect("placeholder portrait file");

    app::social::user_faces::insert_face_capture(
        app.pool(),
        uid,
        Some(artwork_rel),
        "test-model",
        Some(0.9),
        uid,
    )
    .await
    .expect("upsert artwork");
    assert_eq!(
        app::social::user_faces::artwork_path_for_user(app.pool(), uid)
            .await
            .expect("artwork")
            .as_deref(),
        Some(artwork_rel)
    );

    let _ = tokio::fs::remove_file(artwork_rel).await;

    app.cleanup().await;
}

#[tokio::test]
async fn user_faces_table_stores_no_biometrics() {
    let app = TestApp::new().await;

    // Legal-compliance guard: no embeddings, biometric encodings, or appearance text.
    let biometric_cols: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM information_schema.columns
        WHERE table_schema = DATABASE()
          AND table_name = 'user_faces'
          AND (
                LOWER(column_name) LIKE '%embedding%'
             OR LOWER(column_name) LIKE '%biometric%'
             OR LOWER(column_name) = 'ai_appearance_description'
          )
        "#,
    )
    .fetch_one(app.pool())
    .await
    .expect("information_schema query");
    assert_eq!(
        biometric_cols, 0,
        "user_faces must not store any biometric/appearance columns"
    );

    // The completion signal column must exist.
    let signal_cols: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM information_schema.columns
           WHERE table_schema = DATABASE()
             AND table_name = 'user_faces'
             AND column_name = 'face_captured_at'"#,
    )
    .fetch_one(app.pool())
    .await
    .expect("information_schema query");
    assert_eq!(signal_cols, 1, "face_captured_at completion signal must exist");

    app.cleanup().await;
}
