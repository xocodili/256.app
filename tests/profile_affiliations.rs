//! Other-user human profile — affiliations block on private profile card fragment.

mod common;

use axum::http::StatusCode;
use common::api::TestApp;

const PW: &str = "correct-horse-battery-staple";

async fn entity_type_id(app: &TestApp, key: &str) -> u64 {
    sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = 'social_entity_type' AND `key` = ? LIMIT 1",
    )
    .bind(key)
    .fetch_one(app.pool())
    .await
    .expect("entity type id")
}

async fn human_handle(app: &TestApp, user_id: u64) -> String {
    sqlx::query_scalar(
        r#"
        SELECT se.handle
        FROM social_entity se
        INNER JOIN social_human sh ON sh.social_entity_id = se.id
        WHERE se.created_by = ?
        ORDER BY se.id ASC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .fetch_one(app.pool())
    .await
    .expect("human handle")
}

async fn human_entity_id(app: &TestApp, user_id: u64) -> u64 {
    app::social::human_onboarding::primary_human_entity_id(app.pool(), user_id)
        .await
        .expect("human entity id")
        .expect("human entity id missing")
}

async fn seed_verified_friendship(app: &TestApp, viewer_user_id: u64, target_user_id: u64) {
    let viewer_entity = human_entity_id(app, viewer_user_id).await;
    let target_entity = human_entity_id(app, target_user_id).await;
    let friend_type: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = 'social_entity_relationship_type' AND `key` = 'friend' LIMIT 1",
    )
    .fetch_one(app.pool())
    .await
    .expect("friend type");
    let verified_status: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = 'status' AND `key` = 'verified' LIMIT 1",
    )
    .fetch_one(app.pool())
    .await
    .expect("verified status");
    let rel_verified_status: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = 'social_relationship_status' AND `key` = 'verified' LIMIT 1",
    )
    .fetch_one(app.pool())
    .await
    .expect("relationship verified status");
    sqlx::query(
        r#"
        INSERT INTO social_entity_relationship
            (from_social_entity_id, to_social_entity_id, type_id, status_id, relationship_status_id, rating, started_at)
        VALUES (?, ?, ?, ?, ?, 2.50, UTC_TIMESTAMP(6))
        ON DUPLICATE KEY UPDATE ended_at = NULL, status_id = VALUES(status_id), relationship_status_id = VALUES(relationship_status_id)
        "#,
    )
    .bind(viewer_entity)
    .bind(target_entity)
    .bind(friend_type)
    .bind(verified_status)
    .bind(rel_verified_status)
    .execute(app.pool())
    .await
    .expect("friendship insert");
}

async fn create_profit(app: &TestApp, sid: &str, loc: u64, handle: &str) -> String {
    let type_id = entity_type_id(app, "profit").await;
    let body = serde_json::json!({
        "handle": handle,
        "type_id": type_id.to_string(),
        "location_id": loc.to_string(),
        "profit": {
            "label": "Affiliation Test Co",
            "legal_name": "Affiliation Test Co Pty Ltd",
            "founded_at": "2020-01-01",
        },
    });
    let resp = app.post_json_auth("/social-entities", &body, sid).await;
    assert_eq!(resp.status, StatusCode::CREATED, "{}", resp.text());
    handle.to_string()
}

#[tokio::test]
async fn other_human_profile_card_loads_with_affiliations() {
    let app = TestApp::new().await;
    let viewer_id = app.seed_full_member("viewer@example.com", PW).await;
    let target_id = app.seed_full_member("target@example.com", PW).await;
    let viewer_sid = app.dev_session("viewer@example.com").await;
    let target_sid = app.dev_session("target@example.com").await;
    let loc = app.seed_location().await;

    let target_handle = human_handle(&app, target_id).await;
    seed_verified_friendship(&app, viewer_id, target_id).await;
    create_profit(&app, &target_sid, loc, "affil_profit").await;

    let resp = app
        .get_auth(
            &format!("/home/private-profile-card/{target_handle}"),
            &viewer_sid,
        )
        .await;
    assert_eq!(
        resp.status,
        StatusCode::OK,
        "profile card should load for connected viewer: {}",
        resp.text()
    );
    let html = resp.text();
    assert!(
        html.contains("profile-human-affiliations-block"),
        "expected affiliations section for other human profile"
    );
    assert!(
        html.contains("affil_profit"),
        "expected owned profit handle in affiliations"
    );
    assert!(
        html.contains("profile-civic-standing-label"),
        "expected civic-standing label styling on affiliation headings"
    );

    assert!(
        html.contains("profile-disconnect-form"),
        "expected disconnect control visible in collapsed other-user profile"
    );
    assert!(
        html.contains("profile-block-connect-block"),
        "expected block control visible in collapsed other-user profile"
    );
    assert!(
        html.contains("profile-view-toggle-view"),
        "expected view profile control in collapsed profile"
    );

    let _ = viewer_id;
    app.cleanup().await;
}
