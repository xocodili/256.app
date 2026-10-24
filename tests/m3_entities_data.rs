//! Module 3 — Entity creation & maintenance · Data tier.
//!
//! Asserts the persisted invariants behind creating and managing **non-human**
//! social entities (brands, non-profits, gatherings): the neutral **2.50 rating
//! floor** applied at create, the creator/owner-role bookkeeping, and that a
//! voluntary delete is a soft **death** (status flips to `dead`, row retained).

mod common;

use common::api::TestApp;

const PW: &str = "correct-horse-battery-staple";

/// Wallet balance needed for two 16-Nom entity creates (non-profit + gathering).
fn wallet_for_two_cheap_entities() -> u16 {
    use app::pricing::BillableService;
    BillableService::EntityCreateNonProfit.nom_cost()
        + BillableService::EntityCreateGathering.nom_cost()
}

async fn entity_type_id(app: &TestApp, key: &str) -> String {
    let id: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = 'social_entity_type' AND `key` = ? LIMIT 1",
    )
    .bind(key)
    .fetch_one(app.pool())
    .await
    .unwrap_or_else(|e| panic!("social_entity_type {key}: {e}"));
    id.to_string()
}

/// Create a non-profit through the real API and return its (entity_id, handle).
async fn create_non_profit(app: &TestApp, sid: &str, loc: u64, handle: &str) -> (u64, String) {
    let type_id = entity_type_id(app, "non_profit").await;
    let resp = app
        .post_json_auth(
            "/social-entities",
            &serde_json::json!({
                "handle": handle,
                "type_id": type_id,
                "location_id": loc.to_string(),
                "non_profit": {
                    "label": "Ocean Trust",
                    "legal_name": "Ocean Trust Foundation",
                    "founded_at": "2015-06-01",
                },
            }),
            sid,
        )
        .await;
    assert_eq!(
        resp.status,
        axum::http::StatusCode::CREATED,
        "non_profit create failed: {}",
        resp.text()
    );
    let json = resp.json();
    let id: u64 = json["id"].as_str().unwrap().parse().unwrap();
    let saved_handle = json["handle"].as_str().unwrap().to_string();
    (id, saved_handle)
}

async fn entity_rating(app: &TestApp, entity_id: u64) -> String {
    sqlx::query_scalar("SELECT CAST(rating AS CHAR) FROM social_entity WHERE id = ?")
        .bind(entity_id)
        .fetch_one(app.pool())
        .await
        .expect("entity rating")
}

async fn entity_status_key(app: &TestApp, entity_id: u64) -> String {
    sqlx::query_scalar(
        r#"SELECT r.`key`
           FROM social_entity se
           INNER JOIN ref_key_values r ON r.id = COALESCE(se.entity_status_id, se.status_id)
           WHERE se.id = ?"#,
    )
    .bind(entity_id)
    .fetch_one(app.pool())
    .await
    .expect("entity status key")
}

#[tokio::test]
async fn brand_entities_get_neutral_rating_floor() {
    let app = TestApp::new().await;
    let uid = app.seed_full_member("founder@example.com", PW).await;
    let sid = app.dev_session("founder@example.com").await;
    common::api::ensure_test_human_wallet(app.pool(), uid, wallet_for_two_cheap_entities()).await;
    let loc = app.seed_location().await;

    let (np_id, _) = create_non_profit(&app, &sid, loc, "ocean_trust").await;

    // A gathering: the handle is date-suffixed by the server, so read it back.
    let gathering_type = entity_type_id(&app, "gathering").await;
    let g_resp = app
        .post_json_auth(
            "/social-entities",
            &serde_json::json!({
                "handle": "tide_pool_meet",
                "type_id": gathering_type,
                "location_id": loc.to_string(),
                "gathering": { "label": "Tide Pool Meetup" },
            }),
            &sid,
        )
        .await;
    assert_eq!(
        g_resp.status,
        axum::http::StatusCode::CREATED,
        "gathering create failed: {}",
        g_resp.text()
    );
    let g_id: u64 = g_resp.json()["id"].as_str().unwrap().parse().unwrap();

    assert_eq!(entity_rating(&app, np_id).await, "2.50", "non_profit floor");
    assert_eq!(entity_rating(&app, g_id).await, "2.50", "gathering floor");

    app.cleanup().await;
}

#[tokio::test]
async fn create_records_creator_and_owner_role() {
    let app = TestApp::new().await;
    let uid = app.seed_full_member("owner@example.com", PW).await;
    let sid = app.dev_session("owner@example.com").await;
    let loc = app.seed_location().await;

    let (entity_id, _) = create_non_profit(&app, &sid, loc, "reef_guild").await;

    let created_by: u64 =
        sqlx::query_scalar("SELECT created_by FROM social_entity WHERE id = ?")
            .bind(entity_id)
            .fetch_one(app.pool())
            .await
            .expect("created_by");
    assert_eq!(created_by, uid, "creator recorded on the entity");

    let owner_rows: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM user_social_entity_role usr
           INNER JOIN ref_key_values r ON r.id = usr.role_id
           WHERE usr.user_id = ?
             AND usr.social_entity_id = ?
             AND r.`group` = 'user_social_entity_role'
             AND r.`key` = 'owner'"#,
    )
    .bind(uid)
    .bind(entity_id)
    .fetch_one(app.pool())
    .await
    .expect("owner role count");
    assert_eq!(owner_rows, 1, "creator gets exactly one owner role row");

    app.cleanup().await;
}

#[tokio::test]
async fn voluntary_delete_soft_kills_entity() {
    let app = TestApp::new().await;
    app.seed_full_member("steward@example.com", PW).await;
    let sid = app.dev_session("steward@example.com").await;
    let loc = app.seed_location().await;

    let (entity_id, handle) = create_non_profit(&app, &sid, loc, "kelp_fund").await;
    assert_ne!(entity_status_key(&app, entity_id).await, "dead");

    let resp = app
        .post_json_csrf(
            &format!("/me/entities/{handle}/delete"),
            &serde_json::json!({ "password": PW, "confirm": true }),
            &sid,
        )
        .await;
    assert_eq!(
        resp.status,
        axum::http::StatusCode::OK,
        "delete failed: {}",
        resp.text()
    );

    // Soft death: the row survives but status flips to dead.
    let still_present: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM social_entity WHERE id = ?")
            .bind(entity_id)
            .fetch_one(app.pool())
            .await
            .expect("row count");
    assert_eq!(still_present, 1, "delete is a soft death, not a hard delete");
    assert_eq!(entity_status_key(&app, entity_id).await, "dead");

    app.cleanup().await;
}
