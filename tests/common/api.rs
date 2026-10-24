//! In-process API-contract harness — the "A" (API) tier of the test pyramid.
//!
//! This boots the **real** Axum application (`app::build_app`, the same router +
//! middleware stack production serves) on top of a throwaway [`TestDb`], and drives
//! it with `tower`'s `oneshot` — no TCP socket, no live server. It is deliberately
//! self-contained ("its own little panoply"): everything an API test needs (config,
//! request builders, cookie/session handling, user seeding) lives here so module
//! test files stay terse and declarative.
//!
//! Per-IP rate limiters key off `X-Forwarded-For` (via `SmartIpKeyExtractor`), so the
//! harness assigns each [`TestApp`] a unique synthetic client IP. That keeps the
//! process-wide UI-surface limiter bucket isolated per test, while still letting a
//! focused test pin a single IP and intentionally trip a `429`.
//!
//! Shared across many test binaries; each uses only a subset of the surface.
#![allow(dead_code)]

use std::collections::HashSet;
use std::net::IpAddr;
use std::sync::atomic::{AtomicU32, Ordering};

use axum::body::Body;
use axum::http::{header, HeaderMap, Method, Request, StatusCode};
use axum::Router;
use sqlx::MySqlPool;
use tower::ServiceExt;

use app::app_state::AppState;
use app::config::Config;
use app::ref_keys;

use super::TestDb;

static CLIENT_IP_SEQ: AtomicU32 = AtomicU32::new(1);

/// Server pepper used by the harness config; tests derive CSRF tokens from it.
pub const TEST_CSRF_PEPPER: &str = "test-harness-csrf-pepper";

/// A booted application instance backed by a dedicated throwaway database.
pub struct TestApp {
    pub db: TestDb,
    router: Router,
    client_ip: String,
}

impl TestApp {
    /// Spin up a fresh database (migrations + ref-key seeds applied) and assemble
    /// the production router/middleware stack against it.
    pub async fn new() -> Self {
        let db = TestDb::new().await;
        let n = CLIENT_IP_SEQ.fetch_add(1, Ordering::Relaxed);
        let client_ip = format!("198.18.{}.{}", (n >> 8) & 0xff, n & 0xff);
        let state = AppState::new(db.pool.clone(), test_config());
        let router = app::build_app(state);
        Self {
            db,
            router,
            client_ip,
        }
    }

    pub fn pool(&self) -> &MySqlPool {
        &self.db.pool
    }

    /// Default synthetic client IP sent as `X-Forwarded-For` on harness requests.
    pub fn client_ip(&self) -> &str {
        &self.client_ip
    }

    pub async fn cleanup(self) {
        self.db.cleanup().await;
    }

    // ---- Low-level send ----

    /// Drive one request through the full stack. Adds the harness `X-Forwarded-For`
    /// when the caller has not set one (so rate-limit key extraction succeeds).
    pub async fn send(&self, mut req: Request<Body>) -> TestResponse {
        if !req.headers().contains_key("x-forwarded-for") {
            req.headers_mut().insert(
                "x-forwarded-for",
                self.client_ip.parse().expect("valid synthetic ip header"),
            );
        }
        let resp = self
            .router
            .clone()
            .oneshot(req)
            .await
            .expect("router oneshot");
        let status = resp.status();
        let headers = resp.headers().clone();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("collect response body");
        TestResponse {
            status,
            headers,
            body: bytes.to_vec(),
        }
    }

    // ---- Request builders ----

    pub async fn get(&self, path: &str) -> TestResponse {
        self.send(self.builder(Method::GET, path).body(Body::empty()).unwrap())
            .await
    }

    pub async fn get_html(&self, path: &str) -> TestResponse {
        self.send(
            self.builder(Method::GET, path)
                .header(header::ACCEPT, "text/html")
                .body(Body::empty())
                .unwrap(),
        )
        .await
    }

    pub async fn get_auth(&self, path: &str, sid: &str) -> TestResponse {
        self.send(
            self.builder(Method::GET, path)
                .header(header::COOKIE, format!("sid={sid}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
    }

    pub async fn get_auth_html(&self, path: &str, sid: &str) -> TestResponse {
        self.send(
            self.builder(Method::GET, path)
                .header(header::ACCEPT, "text/html")
                .header(header::COOKIE, format!("sid={sid}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
    }

    pub async fn post_json(&self, path: &str, body: &serde_json::Value) -> TestResponse {
        self.send(self.json_request(Method::POST, path, body, None))
            .await
    }

    pub async fn post_json_auth(
        &self,
        path: &str,
        body: &serde_json::Value,
        sid: &str,
    ) -> TestResponse {
        self.send(self.json_request(Method::POST, path, body, Some(sid)))
            .await
    }

    /// PATCH JSON as an authenticated user **without** a CSRF token — used to assert
    /// that CSRF-gated mutations reject cookie-only requests.
    pub async fn patch_json_auth(
        &self,
        path: &str,
        body: &serde_json::Value,
        sid: &str,
    ) -> TestResponse {
        let req = self
            .builder(Method::PATCH, path)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::COOKIE, format!("sid={sid}"))
            .body(Body::from(serde_json::to_vec(body).unwrap()))
            .unwrap();
        self.send(req).await
    }

    /// Derive the session-bound CSRF token the way the middleware expects it
    /// (`X-CSRF-Token` header value for cookie-authenticated mutations).
    pub fn csrf_token(&self, sid: &str) -> String {
        app::auth::csrf::token_from_session(sid, TEST_CSRF_PEPPER)
    }

    /// PATCH JSON as an authenticated user with a valid CSRF token (the gate used
    /// by managed-entity profile edits via `require_profile_mutation`).
    pub async fn patch_json_csrf(
        &self,
        path: &str,
        body: &serde_json::Value,
        sid: &str,
    ) -> TestResponse {
        self.send(self.csrf_json_request(Method::PATCH, path, body, sid))
            .await
    }

    /// POST JSON as an authenticated user with a valid CSRF token (e.g. entity delete).
    pub async fn post_json_csrf(
        &self,
        path: &str,
        body: &serde_json::Value,
        sid: &str,
    ) -> TestResponse {
        self.send(self.csrf_json_request(Method::POST, path, body, sid))
            .await
    }

    /// POST `multipart/form-data` as an authenticated user (face capture / avatar redraw).
    pub async fn post_multipart_auth(&self, path: &str, body: Vec<u8>, boundary: &str, sid: &str) -> TestResponse {
        let content_type = format!("multipart/form-data; boundary={boundary}");
        self.send(
            self.builder(Method::POST, path)
                .header(header::CONTENT_TYPE, content_type)
                .header(header::COOKIE, format!("sid={sid}"))
                .body(Body::from(body))
                .unwrap(),
        )
        .await
    }

    /// Minimal empty multipart body — enough for handlers that gate before reading fields.
    pub fn empty_multipart_body(boundary: &str) -> Vec<u8> {
        format!("--{boundary}--\r\n").into_bytes()
    }

    /// POST JSON while pinning a specific client IP — used to exercise per-IP limits.
    pub async fn post_json_from_ip(
        &self,
        path: &str,
        body: &serde_json::Value,
        ip: &str,
    ) -> TestResponse {
        let req = Request::builder()
            .method(Method::POST)
            .uri(path)
            .header(header::CONTENT_TYPE, "application/json")
            .header("x-forwarded-for", ip)
            .body(Body::from(serde_json::to_vec(body).unwrap()))
            .unwrap();
        self.send(req).await
    }

    fn builder(&self, method: Method, path: &str) -> axum::http::request::Builder {
        Request::builder().method(method).uri(path)
    }

    fn json_request(
        &self,
        method: Method,
        path: &str,
        body: &serde_json::Value,
        sid: Option<&str>,
    ) -> Request<Body> {
        let mut b = self
            .builder(method, path)
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(sid) = sid {
            b = b.header(header::COOKIE, format!("sid={sid}"));
        }
        b.body(Body::from(serde_json::to_vec(body).unwrap())).unwrap()
    }

    fn csrf_json_request(
        &self,
        method: Method,
        path: &str,
        body: &serde_json::Value,
        sid: &str,
    ) -> Request<Body> {
        self.builder(method, path)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::COOKIE, format!("sid={sid}"))
            .header(app::auth::csrf::CSRF_HEADER, self.csrf_token(sid))
            .body(Body::from(serde_json::to_vec(body).unwrap()))
            .unwrap()
    }

    // ---- High-level flows ----

    /// Register through the public JSON API (reCAPTCHA bypass enabled in test config).
    pub async fn register(&self, email: &str, password: &str) -> TestResponse {
        self.post_json(
            "/auth/register",
            &serde_json::json!({
                "email": email,
                "password": password,
                "captcha_token": "test-bypass",
            }),
        )
        .await
    }

    /// Log in through the public JSON API; returns the response (inspect the `sid`).
    pub async fn login(&self, email: &str, password: &str) -> TestResponse {
        self.post_json(
            "/auth/login",
            &serde_json::json!({
                "email": email,
                "password": password,
                "captcha_token": "test-bypass",
            }),
        )
        .await
    }

    /// Record current Root Charter / age consent for a session holder.
    pub async fn accept_terms_consent(&self, sid: &str) -> TestResponse {
        self.post_json_auth(
            "/me/terms-consent",
            &serde_json::json!({
                "adult_ack": true,
                "constitution_ack": true,
            }),
            sid,
        )
        .await
    }

    /// Insert an email-verified (but un-onboarded) human user directly.
    pub async fn seed_verified_user(&self, email: &str, password: &str) -> u64 {
        seed_verified_user(&self.db.pool, email, password).await
    }

    /// Insert a fully onboarded member (`user_status = verified`) who holds posting
    /// rights — the prerequisite for creating and managing social entities.
    pub async fn seed_full_member(&self, email: &str, password: &str) -> u64 {
        seed_full_member(&self.db.pool, email, password).await
    }

    /// Insert (or reuse) a throwaway location row and return its id, for entity
    /// creation that requires an explicit `location_id`.
    pub async fn seed_location(&self) -> u64 {
        seed_location(&self.db.pool).await
    }

    /// Obtain a session cookie for a verified user via the dev bypass endpoint.
    pub async fn dev_session(&self, email: &str) -> String {
        let resp = self
            .post_json("/dev/auth/login-as", &serde_json::json!({ "email": email }))
            .await
        ;
        assert_eq!(
            resp.status,
            StatusCode::OK,
            "dev login-as failed: {}",
            resp.text()
        );
        resp.session_cookie()
            .expect("dev login-as should set a session cookie")
    }
}

/// Collected response: status, headers, and fully-buffered body.
pub struct TestResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl TestResponse {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body)
            .unwrap_or_else(|e| panic!("response body was not JSON ({e}): {}", self.text()))
    }

    /// Value of the `sid` session cookie set by this response, if any.
    pub fn session_cookie(&self) -> Option<String> {
        for v in self.headers.get_all(header::SET_COOKIE) {
            let Ok(s) = v.to_str() else { continue };
            let first = s.split(';').next().unwrap_or("");
            if let Some((name, val)) = first.split_once('=') {
                if name.trim() == "sid" && !val.trim().is_empty() {
                    return Some(val.trim().to_string());
                }
            }
        }
        None
    }

    pub fn location(&self) -> Option<String> {
        self.headers
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    }
}

/// Hermetic test configuration — does not read the process environment, so API tests
/// behave identically regardless of the developer's `.env`. The live pool comes from
/// [`TestDb`]; `database_url` here only backs `csrf_pepper`-style derivations.
fn test_config() -> Config {
    Config {
        database_url: "mysql://test-harness/itest".to_string(),
        host: "127.0.0.1".parse::<IpAddr>().unwrap(),
        port: 8080,
        nominatim_user_agent: "256.app-tests".to_string(),
        secure_cookies: false,
        public_base_url: "http://127.0.0.1:8080".to_string(),
        dev_auth_bypass: true,
        trusted_proxy: false,
        smtp: None,
        face_vision_mock: true,
        skip_face_vision_validate: false,
        ai_grpc_endpoint: "http://127.0.0.1:50051".to_string(),
        ai_grpc_timeout_ms: 3000,
        verbose_ai_errors: true,
        portrait_raster_model: "test-model".to_string(),
        portrait_raster_timeout_ms: 30_000,
        generated_faces_dir: "static/generated/user-faces".to_string(),
        council_agent_handle: "epic".to_string(),
        council_invite_review_email: "owner@example.com".to_string(),
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
        csrf_pepper: TEST_CSRF_PEPPER.to_string(),
        media_hash_blocklist: HashSet::new(),
        recaptcha: Some(app::config::RecaptchaConfig {
            site_key: "test-site-key".to_string(),
            secret_key: "test-secret-key".to_string(),
            min_score: 0.5,
            bypass: true,
        }),
        oidc_google: None,
        stripe: None,
        platform_safety_contact_email: "safety@example.com".to_string(),
        council_web_search_api_key: None,
        council_web_search_provider: app::config::WebSearchProvider::None,
        council_web_search_cx: None,
        council_web_search_timeout_ms: 8000,
    }
}

/// Insert an **email-verified** human user with a hashed password and canonical email.
///
/// This models a real account immediately after email verification but *before*
/// onboarding: `status = pending_verification`, `user_status_id = NULL` (exactly what
/// the registration path produces). Full `verified` status is only earned by completing
/// the composite onboarding pipeline, so seeded users correctly flow through onboarding.
pub async fn seed_verified_user(pool: &MySqlPool, email: &str, password: &str) -> u64 {
    let hash = app::auth::password::hash_password(password).expect("hash password");
    let canonical = app::auth::email_address::canonical(email);

    let pending_status =
        lookup_ref_id(pool, ref_keys::GROUP_STATUS, ref_keys::KEY_PENDING_VERIFICATION).await;
    let user_type =
        lookup_ref_id(pool, ref_keys::GROUP_USER_TYPE, ref_keys::KEY_USER_TYPE_HUMAN).await;
    let tier = lookup_ref_id(
        pool,
        ref_keys::GROUP_VERIFICATION_TIER,
        ref_keys::KEY_VERIFICATION_TIER_BASIC,
    )
    .await;

    let res = sqlx::query(
        r#"
        INSERT INTO users
            (email, email_canonical, password_hash, status_id, user_type_id, verification_tier_id, email_verified_at)
        VALUES (?, ?, ?, ?, ?, ?, UTC_TIMESTAMP())
        "#,
    )
    .bind(email)
    .bind(&canonical)
    .bind(hash.as_bytes())
    .bind(pending_status)
    .bind(user_type)
    .bind(tier)
    .execute(pool)
    .await
    .expect("insert verified user");

    res.last_insert_id()
}

/// Insert a **fully onboarded** human user: email verified and `user_status = verified`,
/// which is what `status::user_has_verified_status` (the posting-rights gate) checks.
/// Also ensures a human `social_entity` + wallet exist (required for entity founding).
/// Use this for entity creation/management flows that require posting rights.
pub async fn seed_full_member(pool: &MySqlPool, email: &str, password: &str) -> u64 {
    let id = seed_verified_user(pool, email, password).await;
    let verified_user_status =
        lookup_ref_id(pool, ref_keys::GROUP_USER_STATUS, ref_keys::KEY_VERIFIED).await;
    sqlx::query("UPDATE users SET user_status_id = ? WHERE id = ?")
        .bind(verified_user_status)
        .bind(id)
        .execute(pool)
        .await
        .expect("promote user to verified status");
    ensure_test_human_wallet(pool, id, app::social::noms::SIGNUP_NOM_GRANT).await;
    id
}

/// Minimal human profile + Nom wallet for API tests (idempotent per user).
pub async fn ensure_test_human_wallet(pool: &MySqlPool, user_id: u64, ai_tokens: u16) {
    if let Ok(Some(entity_id)) =
        app::social::human_onboarding::primary_human_entity_id(pool, user_id).await
    {
        sqlx::query("UPDATE social_human SET ai_tokens = ? WHERE social_entity_id = ?")
            .bind(ai_tokens)
            .bind(entity_id)
            .execute(pool)
            .await
            .expect("set test human noms");
        return;
    }

    let loc = seed_location(pool).await;
    let type_id = lookup_ref_id(
        pool,
        ref_keys::GROUP_SOCIAL_ENTITY_TYPE,
        ref_keys::KEY_ENTITY_TYPE_HUMAN,
    )
    .await;
    let status_id =
        lookup_ref_id(pool, ref_keys::GROUP_STATUS, ref_keys::KEY_VERIFIED).await;
    let entity_status_id = lookup_ref_id(
        pool,
        ref_keys::GROUP_SOCIAL_ENTITY_STATUS,
        ref_keys::KEY_VERIFIED,
    )
    .await;
    let join_open =
        lookup_ref_id(pool, ref_keys::GROUP_SOCIAL_JOIN_POLICY, ref_keys::KEY_JOIN_POLICY_OPEN)
            .await;
    let owner_role = lookup_ref_id(
        pool,
        ref_keys::GROUP_USER_SOCIAL_ENTITY_ROLE,
        ref_keys::KEY_USER_SOCIAL_ENTITY_ROLE_OWNER,
    )
    .await;

    let handle = format!("test_human_{user_id}");
    let entity_id = sqlx::query(
        r#"
        INSERT INTO social_entity (handle, type_id, status_id, entity_status_id, location_id, created_by, rating)
        VALUES (?, ?, ?, ?, ?, ?, 2.50)
        "#,
    )
    .bind(&handle)
    .bind(type_id)
    .bind(status_id)
    .bind(entity_status_id)
    .bind(loc)
    .bind(user_id)
    .execute(pool)
    .await
    .expect("insert test human entity")
    .last_insert_id();

    sqlx::query(
        r#"
        INSERT INTO social_human (social_entity_id, family_name, given_name, birth_date, birth_location_id, join_policy_id, ai_tokens)
        VALUES (?, 'Member', 'Test', '1990-01-01', ?, ?, ?)
        "#,
    )
    .bind(entity_id)
    .bind(loc)
    .bind(join_open)
    .bind(ai_tokens)
    .execute(pool)
    .await
    .expect("insert test social_human");

    sqlx::query(
        r#"
        INSERT INTO user_social_entity_role (user_id, social_entity_id, role_id, is_primary, starts_at)
        VALUES (?, ?, ?, 1, UTC_TIMESTAMP())
        "#,
    )
    .bind(user_id)
    .bind(entity_id)
    .bind(owner_role)
    .execute(pool)
    .await
    .expect("insert test owner role");
}

/// Insert (or reuse) a single throwaway location row, mirroring the shape used by
/// the data-tier graph seeder so entity creation has a valid `location_id` FK target.
pub async fn seed_location(pool: &MySqlPool) -> u64 {
    if let Some(id) = sqlx::query_scalar("SELECT id FROM locations ORDER BY id ASC LIMIT 1")
        .fetch_optional(pool)
        .await
        .expect("query locations")
    {
        return id;
    }
    let lifecycle_id: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = 'location_lifecycle' AND `key` = 'active' LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .expect("location_lifecycle active");

    sqlx::query(
        r#"INSERT INTO locations
           (country_code, locality, display_label, validation_source, validation_ref, canonical_fingerprint, location_lifecycle_id)
           VALUES ('ZZ', 'Test City', 'Test City', 'seed', 'test-harness', UNHEX(SHA2('test-harness-loc', 256)), ?)"#,
    )
    .bind(lifecycle_id)
    .execute(pool)
    .await
    .expect("insert location")
    .last_insert_id()
}

async fn lookup_ref_id(pool: &MySqlPool, group: &str, key: &str) -> u64 {
    sqlx::query_scalar("SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? LIMIT 1")
        .bind(group)
        .bind(key)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("ref_key_values ({group}, {key}) not found: {e}"))
}
