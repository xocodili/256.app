//! Council invite pipeline unit and integration checks.

mod common;

use app::ai::business_functions::response::{
    parse_invite_article_shortlist_response, parse_invite_person_discover_response,
    parse_invite_response_with_verified_email, InviteArticleShortlistDecision,
    InvitePersonDiscoverDecision,
};
use app::config::{Config, WebSearchProvider};
use app::content::web_search::{sanitize_query, search};
use app::social::council_invite_contact::{
    email_verifiable_in_blob, extract_emails_from_html, profile_url_is_allowed,
    profile_url_on_article_host, url_matches_search_hits,
};
use app::content::web_search::SearchHit;
use app::social::council_invite_pipeline::{
    load_steward_invite_article_pool, StewardInviteArticle, INVITE_MAX_CHAINS_PER_DAY,
};
use common::{RefKeys, TestDb};
use sqlx::MySqlPool;

#[test]
fn web_search_empty_without_api_key() {
    let config = Config {
        council_web_search_api_key: None,
        council_web_search_provider: WebSearchProvider::None,
        council_web_search_cx: None,
        council_web_search_timeout_ms: 8000,
        ..minimal_config()
    };
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let hits = rt.block_on(search(&config, "Jane Doe author", 5));
    assert!(hits.is_empty());
}

#[test]
fn sanitize_query_collapses_whitespace() {
    assert_eq!(sanitize_query("  Jane   Doe  "), "Jane Doe");
}

#[test]
fn email_verifiable_in_article_blob() {
    let article = StewardInviteArticle {
        post_id: 1,
        content_link_id: 2,
        body_plain: "Reach author@news.test".into(),
        link_url: "https://news.test/story".into(),
        og_title: Some("Interview".into()),
        og_description: None,
        og_site_name: None,
    };
    let mut blob = article.link_url.clone();
    blob.push(' ');
    blob.push_str(&article.body_plain);
    assert!(email_verifiable_in_blob("author@news.test", &blob));
}

#[test]
fn extract_emails_from_simple_html() {
    let emails = extract_emails_from_html(r#"Contact <a href="mailto:writer@press.org">us</a>"#);
    assert!(emails.iter().any(|e| e.contains("writer@press.org")));
}

#[test]
fn snippet_only_email_verifies_in_combined_blob() {
    let article = StewardInviteArticle {
        post_id: 1,
        content_link_id: 2,
        body_plain: "Story text without addresses.".into(),
        link_url: "https://news.test/story".into(),
        og_title: None,
        og_description: None,
        og_site_name: None,
    };
    let mut blob = article.link_url.clone();
    blob.push(' ');
    blob.push_str(&article.body_plain);
    blob.push_str(" writer@press.org said more.");
    assert!(email_verifiable_in_blob("writer@press.org", &blob));
    assert!(!email_verifiable_in_blob("fake@hallucinated.test", &blob));
}

#[test]
fn profile_url_grounding_with_search_hits() {
    let article = StewardInviteArticle {
        post_id: 1,
        content_link_id: 2,
        body_plain: "body".into(),
        link_url: "https://news.test/story".into(),
        og_title: None,
        og_description: None,
        og_site_name: None,
    };
    let hits = vec![SearchHit {
        title: "Author".into(),
        url: "https://news.test/writers/jane".into(),
        snippet: String::new(),
    }];
    assert!(url_matches_search_hits("https://news.test/writers/jane/", &hits));
    assert!(!url_matches_search_hits("https://evil.test/fake", &hits));
    assert!(profile_url_on_article_host(
        "https://news.test/writers/jane",
        &article
    ));
}

#[test]
fn invite_parsers_contract() {
    assert!(matches!(
        parse_invite_article_shortlist_response(
            r#"{"action":"shortlist","post_ids":[3,1]}"#,
            &[1, 2, 3]
        ),
        Some(InviteArticleShortlistDecision::Shortlist { .. })
    ));
    assert!(matches!(
        parse_invite_person_discover_response(
            r#"{"action":"candidates","people":[{"post_id":2,"intended_label":"A","profile_url":"https://a.test"}]}"#,
            &[2]
        ),
        Some(InvitePersonDiscoverDecision::Candidates { .. })
    ));
    assert!(
        parse_invite_response_with_verified_email(
            r#"{"action":"invite","intended_email":"other@test.com"}"#,
            Some("fixed@test.com")
        )
        .is_none()
    );
}

#[test]
fn compose_send_only_uses_review_delivery() {
    let src = include_str!("../src/social/council_invite_pipeline.rs");
    assert!(src.contains("deliver_platform_invite_for_review"));
    assert!(!src.contains("dispatch::send(config, &intended_email"));
    assert!(!src.contains("dispatch::send(config, intended_email"));
}

#[test]
fn review_inbox_module_targets_config_address() {
    let src = include_str!("../src/email/categories/council_invite.rs");
    assert!(src.contains("config.council_invite_review_email"));
    assert!(src.contains("dispatch::send(config, review_inbox"));
}

#[test]
fn invite_max_chains_constant_is_positive() {
    assert!(INVITE_MAX_CHAINS_PER_DAY >= 1);
}

struct CouncillorRow {
    slug: String,
    entity_id: u64,
    steward_community_id: u64,
}

async fn load_councillor(pool: &MySqlPool, slug: &str) -> CouncillorRow {
    let row: (String, u64, u64) = sqlx::query_as(
        r#"
        SELECT
            aa.slug,
            se.id AS entity_id,
            cse.id AS steward_community_id
        FROM ai_actor aa
        INNER JOIN social_entity se ON se.id = aa.social_entity_id
        INNER JOIN social_entity_relationship modr
            ON modr.from_social_entity_id = se.id
           AND modr.ended_at IS NULL
        INNER JOIN ref_key_values rt
            ON rt.id = modr.type_id
           AND rt.`group` = 'social_entity_relationship_type'
           AND rt.`key` = 'moderator'
        INNER JOIN social_entity cse ON cse.id = modr.to_social_entity_id
        WHERE aa.slug = ?
        LIMIT 1
        "#,
    )
    .bind(slug)
    .fetch_one(pool)
    .await
    .unwrap_or_else(|_| panic!("councillor {slug}"));
    CouncillorRow {
        slug: row.0,
        entity_id: row.1,
        steward_community_id: row.2,
    }
}

async fn insert_invite_link_post(
    pool: &MySqlPool,
    rk: &RefKeys,
    author_entity_id: u64,
    steward_community_id: u64,
    user_id: u64,
    url: &str,
) -> u64 {
    let domain = "invite-pool.test";
    let status_id = rk.id("status", "pending_verification");
    sqlx::query(
        r#"
        INSERT INTO content_domain (domain, status_id, rating, safety_state, first_seen_at, last_seen_at)
        VALUES (?, ?, 2.50, 'safe', UTC_TIMESTAMP(6), UTC_TIMESTAMP(6))
        ON DUPLICATE KEY UPDATE id = LAST_INSERT_ID(id)
        "#,
    )
    .bind(domain)
    .bind(rk.id("status", "verified"))
    .execute(pool)
    .await
    .expect("domain");
    let domain_id: u64 = sqlx::query_scalar("SELECT id FROM content_domain WHERE domain = ? LIMIT 1")
        .bind(domain)
        .fetch_one(pool)
        .await
        .expect("domain id");

    sqlx::query(
        r#"
        INSERT INTO content_link (
            domain_id, url_original, url_canonical, url_canonical_sha,
            status_id, rating, fetch_status, fetch_attempts, next_recrawl_at,
            og_title
        )
        VALUES (?, ?, ?, UNHEX(SHA2(?, 256)), ?, 2.50, 'ok', 1, UTC_TIMESTAMP(6), 'Invite pool title')
        "#,
    )
    .bind(domain_id)
    .bind(url)
    .bind(url)
    .bind(url)
    .bind(status_id)
    .execute(pool)
    .await
    .expect("link");
    let link_id: u64 = sqlx::query_scalar(
        "SELECT id FROM content_link WHERE url_canonical = ? LIMIT 1",
    )
    .bind(url)
    .fetch_one(pool)
    .await
    .expect("link id");

    let verified = rk.id("status", "verified");
    let res = sqlx::query(
        r#"
        INSERT INTO post (
            entity_id, content_link_id, status_id, body_plain, body_markup, body_html_sanitized,
            published_at, created_by, modified_by
        )
        VALUES (?, ?, ?, ?, ?, ?, UTC_TIMESTAMP(6), ?, ?)
        "#,
    )
    .bind(author_entity_id)
    .bind(link_id)
    .bind(verified)
    .bind(format!("share {url}"))
    .bind("<p>share</p>")
    .bind("<p>share</p>")
    .bind(user_id)
    .bind(user_id)
    .execute(pool)
    .await
    .expect("post");
    let post_id = res.last_insert_id();
    sqlx::query(
        "INSERT INTO post_audience_entity (post_id, audience_entity_id) VALUES (?, ?)",
    )
    .bind(post_id)
    .bind(steward_community_id)
    .execute(pool)
    .await
    .expect("audience");
    post_id
}

#[tokio::test]
async fn steward_invite_pool_excludes_peer_mesh_shares() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let rk = RefKeys::load(pool).await;
    let system_user: u64 = sqlx::query_scalar(
        "SELECT id FROM users WHERE email = 'system@256.app' LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .expect("system user");

    let epic = load_councillor(pool, "epic").await;
    let milo = load_councillor(pool, "milo").await;

    let own_post = insert_invite_link_post(
        pool,
        &rk,
        epic.entity_id,
        epic.steward_community_id,
        system_user,
        "https://invite-pool.test/own-epic-story",
    )
    .await;

    let peer_post = insert_invite_link_post(
        pool,
        &rk,
        milo.entity_id,
        epic.steward_community_id,
        system_user,
        "https://invite-pool.test/peer-milo-in-epic-steward",
    )
    .await;

    let pool_rows = load_steward_invite_article_pool(
        pool,
        epic.entity_id,
        epic.steward_community_id,
    )
    .await
    .expect("pool query");

    let post_ids: Vec<u64> = pool_rows.iter().map(|r| r.post_id).collect();
    assert!(
        post_ids.contains(&own_post),
        "own-steward share should appear in invite pool"
    );
    assert!(
        !post_ids.contains(&peer_post),
        "peer-authored mesh share must not appear in invite pool"
    );
}

#[tokio::test]
async fn profile_url_allowed_rejects_ungrounded_when_hits_exist() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let article = StewardInviteArticle {
        post_id: 99,
        content_link_id: 1,
        body_plain: "body".into(),
        link_url: "https://invite-pool.test/story".into(),
        og_title: None,
        og_description: None,
        og_site_name: None,
    };
    let hits = vec![SearchHit {
        title: "Jane".into(),
        url: "https://invite-pool.test/authors/jane".into(),
        snippet: String::new(),
    }];
    let ok = profile_url_is_allowed(
        pool,
        "https://invite-pool.test/authors/jane",
        &article,
        &hits,
        true,
    )
    .await
    .expect("allowed check");
    assert!(ok);

    let bad = profile_url_is_allowed(
        pool,
        "https://unrelated.test/not-in-hits",
        &article,
        &hits,
        true,
    )
    .await
    .expect("reject check");
    assert!(!bad);
}

fn minimal_config() -> Config {
    use std::collections::HashSet;
    use std::net::IpAddr;
    Config {
        database_url: "mysql://test/itest".into(),
        host: "127.0.0.1".parse::<IpAddr>().unwrap(),
        port: 8080,
        nominatim_user_agent: "256.app-tests".into(),
        secure_cookies: false,
        public_base_url: "http://127.0.0.1:8080".into(),
        dev_auth_bypass: true,
        trusted_proxy: false,
        smtp: None,
        face_vision_mock: true,
        skip_face_vision_validate: false,
        ai_grpc_endpoint: "http://127.0.0.1:50051".into(),
        ai_grpc_timeout_ms: 3000,
        verbose_ai_errors: true,
        portrait_raster_model: "test".into(),
        portrait_raster_timeout_ms: 30_000,
        generated_faces_dir: "static/generated/user-faces".into(),
        council_agent_handle: "epic".into(),
        council_invite_review_email: "owner@example.com".into(),
        platform_recruitment_review_only: true,
        google_maps_api_key: None,
        google_places_timeout_ms: 12_000,
        ops_location_invalidate_token: None,
        ops_job_queue_token: None,
        request_timeout_secs: 60,
        scheduler_tick_secs: 0,
        scheduler_batch_size: 32,
        job_conductor_parallelism: 4,
        job_conductor_llm_parallelism: 2,
        feed_poll_wall_budget_secs: 120,
        ops_digest_live: true,
        journal_retention_days: 3,
        csrf_pepper: "test".into(),
        media_hash_blocklist: HashSet::new(),
        recaptcha: None,
        oidc_google: None,
        stripe: None,
        platform_safety_contact_email: "safety@example.com".into(),
        council_web_search_api_key: None,
        council_web_search_provider: WebSearchProvider::None,
        council_web_search_cx: None,
        council_web_search_timeout_ms: 8000,
    }
}
