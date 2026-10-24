//! Module 3 — Entity creation & maintenance · API tier.
//!
//! Drives the HTTP contract for managed (non-human) social entities through the
//! full middleware stack: the posting-rights gate on create, ownership scoping on
//! edit/delete, the CSRF requirement on mutations, and password confirmation on
//! delete. Brands are modelled as **non-profits** (stable, non-suffixed handle).

mod common;

use axum::http::StatusCode;
use common::api::TestApp;

const PW: &str = "correct-horse-battery-staple";

/// `dev_session` runs a real sign-in (+1 Nom on first login each UTC day).
fn noms_after_dev_login(base: u16) -> u16 {
    base.saturating_add(app::social::noms::DAILY_LOGIN_NOM_GRANT)
}

async fn non_profit_type_id(app: &TestApp) -> String {
    let id: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = 'social_entity_type' AND `key` = 'non_profit' LIMIT 1",
    )
    .fetch_one(app.pool())
    .await
    .expect("non_profit type id");
    id.to_string()
}

fn non_profit_body(type_id: &str, loc: u64, handle: &str) -> serde_json::Value {
    serde_json::json!({
        "handle": handle,
        "type_id": type_id,
        "location_id": loc.to_string(),
        "non_profit": {
            "label": "River Keepers",
            "legal_name": "River Keepers Trust",
            "founded_at": "2015-06-01",
        },
    })
}

#[tokio::test]
async fn create_without_session_is_unauthorized() {
    let app = TestApp::new().await;
    let type_id = non_profit_type_id(&app).await;
    let loc = app.seed_location().await;

    let resp = app
        .post_json("/social-entities", &non_profit_body(&type_id, loc, "no_sess"))
        .await;
    assert_eq!(resp.status, StatusCode::UNAUTHORIZED);

    app.cleanup().await;
}

#[tokio::test]
async fn create_requires_posting_rights() {
    let app = TestApp::new().await;
    // Email-verified but NOT fully onboarded → no posting rights.
    app.seed_verified_user("pending@example.com", PW).await;
    let sid = app.dev_session("pending@example.com").await;
    let type_id = non_profit_type_id(&app).await;
    let loc = app.seed_location().await;

    let resp = app
        .post_json_auth(
            "/social-entities",
            &non_profit_body(&type_id, loc, "too_early"),
            &sid,
        )
        .await;
    assert_eq!(
        resp.status,
        StatusCode::BAD_REQUEST,
        "un-onboarded user must be blocked from creating entities: {}",
        resp.text()
    );

    app.cleanup().await;
}

#[tokio::test]
async fn full_member_creates_non_profit() {
    let app = TestApp::new().await;
    app.seed_full_member("maker@example.com", PW).await;
    let sid = app.dev_session("maker@example.com").await;
    let type_id = non_profit_type_id(&app).await;
    let loc = app.seed_location().await;

    let resp = app
        .post_json_auth(
            "/social-entities",
            &non_profit_body(&type_id, loc, "river_keepers"),
            &sid,
        )
        .await;
    assert_eq!(resp.status, StatusCode::CREATED, "{}", resp.text());
    assert_eq!(resp.json()["handle"].as_str(), Some("river_keepers"));

    app.cleanup().await;
}

#[tokio::test]
async fn edit_requires_csrf_then_persists() {
    let app = TestApp::new().await;
    app.seed_full_member("editor@example.com", PW).await;
    let sid = app.dev_session("editor@example.com").await;
    let type_id = non_profit_type_id(&app).await;
    let loc = app.seed_location().await;

    let created = app
        .post_json_auth(
            "/social-entities",
            &non_profit_body(&type_id, loc, "lake_guild"),
            &sid,
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.text());
    let entity_id: u64 = created.json()["id"].as_str().unwrap().parse().unwrap();

    let edit_body = serde_json::json!({
        "label": "Lake Guild Renamed",
        "legal_name": "Lake Guild Trust",
        "founded_at": "2015-06-01",
        "location_id": loc.to_string(),
    });

    // Cookie present but no CSRF token → rejected.
    let no_csrf = app
        .patch_json_auth("/me/entities/lake_guild", &edit_body, &sid)
        .await;
    assert_eq!(
        no_csrf.status,
        StatusCode::UNAUTHORIZED,
        "edit without CSRF must be rejected"
    );

    // With a valid CSRF token → succeeds and persists.
    let ok = app
        .patch_json_csrf("/me/entities/lake_guild", &edit_body, &sid)
        .await;
    assert_eq!(ok.status, StatusCode::OK, "{}", ok.text());

    let label: String =
        sqlx::query_scalar("SELECT label FROM social_non_profit WHERE social_entity_id = ?")
            .bind(entity_id)
            .fetch_one(app.pool())
            .await
            .expect("non_profit label");
    assert_eq!(label, "Lake Guild Renamed", "edit must persist the new label");

    app.cleanup().await;
}

#[tokio::test]
async fn non_owner_cannot_manage_entity() {
    let app = TestApp::new().await;
    app.seed_full_member("alice@example.com", PW).await;
    app.seed_full_member("mallory@example.com", PW).await;
    let alice = app.dev_session("alice@example.com").await;
    let mallory = app.dev_session("mallory@example.com").await;
    let type_id = non_profit_type_id(&app).await;
    let loc = app.seed_location().await;

    let created = app
        .post_json_auth(
            "/social-entities",
            &non_profit_body(&type_id, loc, "alices_org"),
            &alice,
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.text());

    // Mallory is a legitimate member (full CSRF) but does not own the entity.
    let edit = app
        .patch_json_csrf(
            "/me/entities/alices_org",
            &serde_json::json!({
                "label": "Hijacked",
                "legal_name": "Hijacked",
                "founded_at": "2015-06-01",
                "location_id": loc.to_string(),
            }),
            &mallory,
        )
        .await;
    assert_eq!(edit.status, StatusCode::NOT_FOUND, "non-owner edit must 404");

    let delete = app
        .post_json_csrf(
            "/me/entities/alices_org/delete",
            &serde_json::json!({ "password": PW, "confirm": true }),
            &mallory,
        )
        .await;
    assert_eq!(
        delete.status,
        StatusCode::NOT_FOUND,
        "non-owner delete must 404"
    );

    app.cleanup().await;
}

#[tokio::test]
async fn delete_requires_correct_password() {
    let app = TestApp::new().await;
    app.seed_full_member("closer@example.com", PW).await;
    let sid = app.dev_session("closer@example.com").await;
    let type_id = non_profit_type_id(&app).await;
    let loc = app.seed_location().await;

    let created = app
        .post_json_auth(
            "/social-entities",
            &non_profit_body(&type_id, loc, "sunset_fund"),
            &sid,
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.text());

    let wrong = app
        .post_json_csrf(
            "/me/entities/sunset_fund/delete",
            &serde_json::json!({ "password": "not-the-password", "confirm": true }),
            &sid,
        )
        .await;
    assert_eq!(
        wrong.status,
        StatusCode::BAD_REQUEST,
        "wrong password must block delete"
    );

    let confirmed = app
        .post_json_csrf(
            "/me/entities/sunset_fund/delete",
            &serde_json::json!({ "password": PW, "confirm": true }),
            &sid,
        )
        .await;
    assert_eq!(confirmed.status, StatusCode::OK, "{}", confirmed.text());
    assert_eq!(confirmed.json()["redirect"].as_str(), Some("/home"));

    app.cleanup().await;
}

async fn insert_owned_community(app: &TestApp, user_id: u64, handle: &str) -> u64 {
    let loc = app.seed_location().await;
    let type_id: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = 'social_entity_type' AND `key` = 'community' LIMIT 1",
    )
    .fetch_one(app.pool())
    .await
    .expect("community type");
    let status_id: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = 'status' AND `key` = 'verified' LIMIT 1",
    )
    .fetch_one(app.pool())
    .await
    .expect("verified");
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
    .expect("insert community")
    .last_insert_id();

    sqlx::query(
        r#"
        INSERT INTO social_community (social_entity_id, label, description, intention, join_policy_id)
        VALUES (?, ?, 'Test', 'Testing', ?)
        "#,
    )
    .bind(entity_id)
    .bind(format!("Test {handle}"))
    .bind(join_open)
    .execute(app.pool())
    .await
    .expect("social_community");

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
    .expect("owner role");

    entity_id
}

#[tokio::test]
async fn posting_entities_excludes_community_but_includes_branded() {
    let app = TestApp::new().await;
    let user_id = app.seed_full_member("poster@example.com", PW).await;
    let sid = app.dev_session("poster@example.com").await;
    let type_id = non_profit_type_id(&app).await;
    let loc = app.seed_location().await;

    insert_owned_community(&app, user_id, "my_community").await;
    let created = app
        .post_json_auth(
            "/social-entities",
            &non_profit_body(&type_id, loc, "my_brand"),
            &sid,
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.text());

    let resp = app.get_auth("/me/posting-entities", &sid).await;
    assert_eq!(resp.status, StatusCode::OK, "{}", resp.text());
    let json = resp.json();
    let options = json["entities"].as_array().expect("posting entities array");
    let type_keys: Vec<&str> = options
        .iter()
        .filter_map(|o| o["type_key"].as_str())
        .collect();
    assert!(
        !type_keys.iter().any(|k| *k == "community"),
        "communities must not appear in Post as picker: {type_keys:?}"
    );
    assert!(
        type_keys.iter().any(|k| *k == "non_profit"),
        "owned non-profit must appear in Post as picker: {type_keys:?}"
    );
    let np = options
        .iter()
        .find(|o| o["type_key"].as_str() == Some("non_profit"))
        .expect("non_profit row");
    assert_eq!(
        np["post_nom_cost"].as_u64(),
        Some(app::pricing::BillableService::BrandedPostNonProfit.nom_cost() as u64),
        "posting entities must expose branded post cost: {np:?}"
    );
    assert_eq!(
        np["can_afford_post"].as_bool(),
        Some(true),
        "funded user must afford non-profit post: {np:?}"
    );
    assert!(json["wallet_balance"].is_number(), "wallet_balance required");

    app.cleanup().await;
}

async fn community_type_id(app: &TestApp) -> String {
    let id: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = 'social_entity_type' AND `key` = 'community' LIMIT 1",
    )
    .fetch_one(app.pool())
    .await
    .expect("community type id");
    id.to_string()
}

fn community_body(type_id: &str, handle: &str) -> serde_json::Value {
    serde_json::json!({
        "handle": handle,
        "type_id": type_id,
        "location_id": "",
        "community": {
            "label": "Nom Test Community",
            "parent_community_handle": "operating_charter_256",
            "location": { "country_code": "AU" }
        }
    })
}

async fn human_noms(app: &TestApp, email: &str) -> u16 {
    let user_id: u64 = sqlx::query_scalar(
        "SELECT id FROM users WHERE email_canonical = ? LIMIT 1",
    )
    .bind(app::auth::email_address::canonical(email))
    .fetch_one(app.pool())
    .await
    .expect("user id");
    let entity_id = app::social::human_onboarding::primary_human_entity_id(app.pool(), user_id)
        .await
        .expect("human lookup")
        .expect("human entity");
    sqlx::query_scalar("SELECT ai_tokens FROM social_human WHERE social_entity_id = ?")
        .bind(entity_id)
        .fetch_one(app.pool())
        .await
        .expect("noms")
}

#[tokio::test]
async fn entity_create_charges_noms_for_non_profit() {
    let app = TestApp::new().await;
    app.seed_full_member("nom_np@example.com", PW).await;
    let sid = app.dev_session("nom_np@example.com").await;
    let type_id = non_profit_type_id(&app).await;
    let loc = app.seed_location().await;

    let start = noms_after_dev_login(app::social::noms::SIGNUP_NOM_GRANT);
    assert_eq!(human_noms(&app, "nom_np@example.com").await, start);

    let cost = app::pricing::BillableService::EntityCreateNonProfit.nom_cost();
    let resp = app
        .post_json_auth(
            "/social-entities",
            &non_profit_body(&type_id, loc, "nom_np_org"),
            &sid,
        )
        .await;
    assert_eq!(resp.status, StatusCode::CREATED, "{}", resp.text());
    assert_eq!(human_noms(&app, "nom_np@example.com").await, start - cost);

    app.cleanup().await;
}

#[tokio::test]
async fn entity_create_community_blocked_when_underfunded() {
    let app = TestApp::new().await;
    let user_id = app.seed_full_member("nom_comm_fail@example.com", PW).await;
    // Below community founding cost (32) even after daily login credit.
    common::api::ensure_test_human_wallet(app.pool(), user_id, 16).await;
    let sid = app.dev_session("nom_comm_fail@example.com").await;
    let type_id = community_type_id(&app).await;

    assert_eq!(
        human_noms(&app, "nom_comm_fail@example.com").await,
        noms_after_dev_login(16)
    );

    let resp = app
        .post_json_auth(
            "/social-entities",
            &community_body(&type_id, "nom_comm_fail"),
            &sid,
        )
        .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST, "{}", resp.text());

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM social_entity WHERE handle = ?",
    )
    .bind("nom_comm_fail")
    .fetch_one(app.pool())
    .await
    .expect("count");
    assert_eq!(count, 0, "underfunded create must not insert entity row");

    app.cleanup().await;
}

#[tokio::test]
async fn entity_create_community_succeeds_with_enough_noms() {
    let app = TestApp::new().await;
    let user_id = app.seed_full_member("nom_comm_ok@example.com", PW).await;
    common::api::ensure_test_human_wallet(app.pool(), user_id, 48).await;
    let sid = app.dev_session("nom_comm_ok@example.com").await;
    let type_id = community_type_id(&app).await;

    let resp = app
        .post_json_auth(
            "/social-entities",
            &community_body(&type_id, "nom_comm_ok"),
            &sid,
        )
        .await;
    assert_eq!(resp.status, StatusCode::CREATED, "{}", resp.text());
    let community_cost = app::pricing::BillableService::EntityCreateCommunity.nom_cost();
    assert_eq!(
        human_noms(&app, "nom_comm_ok@example.com").await,
        noms_after_dev_login(48) - community_cost
    );

    app.cleanup().await;
}

#[tokio::test]
async fn entity_create_rejects_logo_on_foreign_domain() {
    let app = TestApp::new().await;
    app.seed_full_member("logo_foreign@example.com", PW).await;
    let sid = app.dev_session("logo_foreign@example.com").await;
    let type_id = non_profit_type_id(&app).await;
    let loc = app.seed_location().await;

    let body = serde_json::json!({
        "handle": "logo_mismatch_np",
        "type_id": type_id,
        "location_id": loc.to_string(),
        "non_profit": {
            "label": "Mismatch Org",
            "legal_name": "Mismatch Org Ltd",
            "founded_at": "2015-06-01",
            "website_url": "https://www.riverkeepers.org/",
            "logo_url": "https://cdn.evil.example/logo.png",
        },
    });

    let resp = app
        .post_json_auth("/social-entities", &body, &sid)
        .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST, "{}", resp.text());
    let text = resp.text();
    assert!(
        text.contains("same domain") || text.to_ascii_lowercase().contains("domain"),
        "expected domain mismatch message: {text}"
    );

    app.cleanup().await;
}

#[tokio::test]
async fn entity_create_rejects_logo_without_website() {
    let app = TestApp::new().await;
    app.seed_full_member("logo_no_web@example.com", PW).await;
    let sid = app.dev_session("logo_no_web@example.com").await;
    let type_id = non_profit_type_id(&app).await;
    let loc = app.seed_location().await;

    let body = serde_json::json!({
        "handle": "logo_no_web_np",
        "type_id": type_id,
        "location_id": loc.to_string(),
        "non_profit": {
            "label": "No Web Org",
            "legal_name": "No Web Org Ltd",
            "founded_at": "2015-06-01",
            "logo_url": "https://www.example.org/logo.png",
        },
    });

    let resp = app
        .post_json_auth("/social-entities", &body, &sid)
        .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST, "{}", resp.text());

    app.cleanup().await;
}

#[tokio::test]
async fn posting_entities_marks_unaffordable_branded_options() {
    let app = TestApp::new().await;
    let user_id = app.seed_full_member("post_poor@example.com", PW).await;
    let sid = app.dev_session("post_poor@example.com").await;
    let type_id = non_profit_type_id(&app).await;
    let loc = app.seed_location().await;

    let created = app
        .post_json_auth(
            "/social-entities",
            &non_profit_body(&type_id, loc, "post_poor_org"),
            &sid,
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.text());

    common::api::ensure_test_human_wallet(app.pool(), user_id, 1).await;

    let resp = app.get_auth("/me/posting-entities", &sid).await;
    assert_eq!(resp.status, StatusCode::OK, "{}", resp.text());
    let json = resp.json();
    assert_eq!(json["wallet_balance"].as_u64(), Some(1));
    let np = json["entities"]
        .as_array()
        .expect("entities")
        .iter()
        .find(|o| o["type_key"].as_str() == Some("non_profit"))
        .expect("non_profit row");
    assert_eq!(
        np["can_afford_post"].as_bool(),
        Some(false),
        "2-Nom post must be unaffordable at 1 Nom: {np:?}"
    );

    app.cleanup().await;
}

#[tokio::test]
async fn branded_post_charges_noms_for_non_profit() {
    let app = TestApp::new().await;
    app.seed_full_member("brand_post@example.com", PW).await;
    let sid = app.dev_session("brand_post@example.com").await;
    let type_id = non_profit_type_id(&app).await;
    let loc = app.seed_location().await;

    let created = app
        .post_json_auth(
            "/social-entities",
            &non_profit_body(&type_id, loc, "brand_post_org"),
            &sid,
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.text());
    let brand_id = created.json()["id"]
        .as_str()
        .expect("entity id")
        .to_string();

    let post_cost = app::pricing::BillableService::BrandedPostNonProfit.nom_cost();
    let before = human_noms(&app, "brand_post@example.com").await;
    let post = app
        .post_json_auth(
            "/posts",
            &serde_json::json!({
                "entity_id": brand_id,
                "audience_handles": "For: @public",
                "body_markup": "Hello from the org",
            }),
            &sid,
        )
        .await;
    assert_eq!(post.status, StatusCode::CREATED, "{}", post.text());
    assert_eq!(
        human_noms(&app, "brand_post@example.com").await,
        before.saturating_sub(post_cost)
    );

    app.cleanup().await;
}

#[tokio::test]
async fn branded_post_blocked_when_underfunded() {
    let app = TestApp::new().await;
    let user_id = app.seed_full_member("brand_post_fail@example.com", PW).await;
    let sid = app.dev_session("brand_post_fail@example.com").await;
    let type_id = non_profit_type_id(&app).await;
    let loc = app.seed_location().await;

    let created = app
        .post_json_auth(
            "/social-entities",
            &non_profit_body(&type_id, loc, "brand_post_fail_org"),
            &sid,
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.text());
    let brand_id = created.json()["id"]
        .as_str()
        .expect("entity id")
        .to_string();

    common::api::ensure_test_human_wallet(app.pool(), user_id, 0).await;

    let post = app
        .post_json_auth(
            "/posts",
            &serde_json::json!({
                "entity_id": brand_id,
                "audience_handles": "For: @public",
                "body_markup": "Should not publish",
            }),
            &sid,
        )
        .await;
    assert_eq!(post.status, StatusCode::BAD_REQUEST, "{}", post.text());

    app.cleanup().await;
}
