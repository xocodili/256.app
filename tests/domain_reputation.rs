//! Domain preview reputation — bands, inline URL enforcement, withheld OG markup.

mod common;

use app::content::domain_reputation::{self, PreviewBand};
use app::content::links;
use axum::http::StatusCode;
use common::api::{seed_full_member, TestApp};
use common::{RefKeys, TestDb};
use sqlx::MySqlPool;

const PW: &str = "TestPass123!";

async fn insert_domain(
    pool: &MySqlPool,
    rk: &RefKeys,
    domain: &str,
    status_key: &str,
    rating: &str,
    safety_state: &str,
    first_seen_at_sql: &str,
) -> u64 {
    let status_id = rk.id("status", status_key);
    sqlx::query(&format!(
        r#"
        INSERT INTO content_domain (domain, status_id, rating, safety_state, first_seen_at, last_seen_at)
        VALUES (?, ?, ?, ?, {first_seen_at_sql}, UTC_TIMESTAMP(6))
        "#
    ))
    .bind(domain)
    .bind(status_id)
    .bind(rating)
    .bind(safety_state)
    .execute(pool)
    .await
    .expect("insert domain");
    sqlx::query_scalar("SELECT id FROM content_domain WHERE domain = ? LIMIT 1")
        .bind(domain)
        .fetch_one(pool)
        .await
        .expect("domain id")
}

async fn insert_link(pool: &MySqlPool, rk: &RefKeys, domain_id: u64, url: &str) -> u64 {
    let pending = rk.id("status", "pending_verification");
    sqlx::query(
        r#"
        INSERT INTO content_link (
            domain_id, url_original, url_canonical, url_canonical_sha,
            status_id, rating, fetch_status, fetch_attempts, next_recrawl_at,
            og_title, og_image_url
        )
        VALUES (?, ?, ?, UNHEX(SHA2(?, 256)), ?, 2.50, 'ok', 1, UTC_TIMESTAMP(6),
                'Test title', 'https://cdn.example.test/preview.jpg')
        "#,
    )
    .bind(domain_id)
    .bind(url)
    .bind(url)
    .bind(url)
    .bind(pending)
    .execute(pool)
    .await
    .expect("insert link");
    sqlx::query_scalar("SELECT id FROM content_link WHERE url_canonical = ? LIMIT 1")
        .bind(url)
        .fetch_one(pool)
        .await
        .expect("link id")
}

async fn attach_post_with_link(
    pool: &MySqlPool,
    entity_id: u64,
    user_id: u64,
    domain_id: u64,
    link_id: u64,
) {
    let verified = RefKeys::load(pool)
        .await
        .id("status", "verified");
    sqlx::query(
        r#"
        INSERT INTO post (
            entity_id, created_by, status_id, body_plain, body_markup, body_html_sanitized,
            content_link_id, domain_id, published_at
        )
        VALUES (?, ?, ?, 'link post', 'link post', 'link post', ?, ?, UTC_TIMESTAMP(6))
        "#,
    )
    .bind(entity_id)
    .bind(user_id)
    .bind(verified)
    .bind(link_id)
    .bind(domain_id)
    .execute(pool)
    .await
    .expect("insert post");
}

#[tokio::test]
async fn unknown_domain_withholds_preview_image() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let rk = RefKeys::load(pool).await;
    let domain_id = insert_domain(
        pool,
        &rk,
        "unknown-preview.test",
        "pending_verification",
        "2.50",
        "safe",
        "UTC_TIMESTAMP(6)",
    )
    .await;

    let band = domain_reputation::resolve_preview_band_for_domain(pool, domain_id)
        .await
        .expect("band");
    assert_eq!(band, PreviewBand::Unknown);
    assert!(!band.preview_image_allowed());
}

#[tokio::test]
async fn seeded_trusted_domain_allows_preview() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let domain_id: u64 = sqlx::query_scalar("SELECT id FROM content_domain WHERE domain = 'bbc.com' LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("bbc.com seeded");
    let band = domain_reputation::resolve_preview_band_for_domain(pool, domain_id)
        .await
        .expect("band");
    assert_eq!(band, PreviewBand::Trusted);
    assert!(band.preview_image_allowed());
}

#[tokio::test]
async fn three_human_attachers_promotes_to_known() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let rk = RefKeys::load(pool).await;
    let domain_id = insert_domain(
        pool,
        &rk,
        "known-by-humans.test",
        "pending_verification",
        "2.50",
        "safe",
        "UTC_TIMESTAMP(6)",
    )
    .await;
    let link_id = insert_link(
        pool,
        &rk,
        domain_id,
        "https://known-by-humans.test/article",
    )
    .await;

    for i in 0..3 {
        let email = format!("known_human_{i}@example.com");
        let user_id = seed_full_member(pool, &email, PW).await;
        let entity_id: u64 = sqlx::query_scalar(
            "SELECT id FROM social_entity WHERE created_by = ? AND type_id = (SELECT id FROM ref_key_values WHERE `group` = 'social_entity_type' AND `key` = 'human') LIMIT 1",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("entity");
        attach_post_with_link(pool, entity_id, user_id, domain_id, link_id).await;
    }

    let band = domain_reputation::resolve_preview_band_for_domain(pool, domain_id)
        .await
        .expect("band");
    assert_eq!(band, PreviewBand::Known);
}

#[tokio::test]
async fn blocked_domain_rejected_on_ingest() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let blocked = links::registrable_domain_is_blocked(pool, "4chan.org")
        .await
        .expect("query");
    assert!(blocked);
}

#[tokio::test]
async fn inline_blocked_url_in_post_body_returns_400() {
    let app = TestApp::new().await;
    let user_id = app.seed_full_member("inline_block@example.com", PW).await;
    let sid = app.dev_session("inline_block@example.com").await;
    let entity_id: u64 = sqlx::query_scalar(
        "SELECT id FROM social_entity WHERE created_by = ? LIMIT 1",
    )
    .bind(user_id)
    .fetch_one(app.pool())
    .await
    .expect("entity");

    let resp = app
        .post_json_csrf(
            "/posts",
            &serde_json::json!({
                "entity_id": entity_id.to_string(),
                "audience_handles": "For: @public",
                "body_markup": "Check out https://4chan.org/b/ for gossip",
            }),
            &sid,
        )
        .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST, "{}", resp.text());
    app.cleanup().await;
}

#[tokio::test]
async fn inline_allowed_url_in_post_body_ok_without_link_attach() {
    let app = TestApp::new().await;
    let user_id = app.seed_full_member("inline_ok@example.com", PW).await;
    let sid = app.dev_session("inline_ok@example.com").await;
    let entity_id: u64 = sqlx::query_scalar(
        "SELECT id FROM social_entity WHERE created_by = ? LIMIT 1",
    )
    .bind(user_id)
    .fetch_one(app.pool())
    .await
    .expect("entity");

    let resp = app
        .post_json_csrf(
            "/posts",
            &serde_json::json!({
                "entity_id": entity_id.to_string(),
                "audience_handles": "For: @public",
                "body_markup": "BBC story https://www.bbc.com/news — discuss",
            }),
            &sid,
        )
        .await;
    assert_eq!(resp.status, StatusCode::CREATED, "{}", resp.text());
    app.cleanup().await;
}

#[tokio::test]
async fn preview_og_feed_withholds_image_url_for_unknown_domain() {
    let app = TestApp::new().await;
    let pool = app.pool();
    let rk = RefKeys::load(pool).await;
    let _user_id = app.seed_full_member("preview_feed@example.com", PW).await;
    let sid = app.dev_session("preview_feed@example.com").await;

    let domain_id = insert_domain(
        pool,
        &rk,
        "preview-feed-unknown.test",
        "pending_verification",
        "2.50",
        "safe",
        "UTC_TIMESTAMP(6)",
    )
    .await;
    let _link_id = insert_link(
        pool,
        &rk,
        domain_id,
        "https://preview-feed-unknown.test/story",
    )
    .await;

    let resp = app
        .post_json_csrf(
            "/content-links/preview-og",
            &serde_json::json!({
                "url": "https://preview-feed-unknown.test/story",
            }),
            &sid,
        )
        .await;
    assert_eq!(resp.status, StatusCode::OK, "{}", resp.text());
    let body: serde_json::Value = serde_json::from_str(&resp.text()).expect("json");
    assert_eq!(body["preview_image_allowed"], false);
    assert!(body["og_image_url"].is_null());
    assert!(body["og_image_reveal_url"]
        .as_str()
        .is_some_and(|u| u.contains("preview.jpg")));

    let composer = app
        .post_json_csrf(
            "/content-links/preview-og",
            &serde_json::json!({
                "url": "https://preview-feed-unknown.test/story",
                "for_composer": true,
            }),
            &sid,
        )
        .await;
    assert_eq!(composer.status, StatusCode::OK, "{}", composer.text());
    let composer_body: serde_json::Value = serde_json::from_str(&composer.text()).expect("json");
    assert!(composer_body["og_image_url"]
        .as_str()
        .is_some_and(|u| u.contains("preview.jpg")));
    assert!(composer_body["og_image_reveal_url"].is_null());

    app.cleanup().await;
}
