//! Community/human login-post inactivity penalties and failing-rating notification claims.

mod common;

use app::auth::session;
use app::config::Config;
use app::ref_keys;
use app::social::community_graph;
use app::social::rating_notify;
use common::TestDb;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::MySqlPool;
use std::net::{IpAddr, Ipv4Addr};

/// `social_entity.rating` is `DECIMAL(3,2)`; 2.50 − 0.035 persists as 2.47.
fn rating_after_one_inactivity_penalty() -> Decimal {
    Decimal::new(247, 2)
}

fn test_config(database_url: String) -> Config {
    Config {
        database_url,
        host: IpAddr::V4(Ipv4Addr::LOCALHOST),
        port: 8080,
        nominatim_user_agent: "256.app-test".into(),
        secure_cookies: false,
        public_base_url: "http://127.0.0.1:8080".into(),
        dev_auth_bypass: false,
        trusted_proxy: false,
        smtp: None,
        face_vision_mock: true,
        skip_face_vision_validate: false,
        ai_grpc_endpoint: "http://127.0.0.1:50051".into(),
        ai_grpc_timeout_ms: 3000,
        verbose_ai_errors: false,
        portrait_raster_model: "test".into(),
        portrait_raster_timeout_ms: 30_000,
        generated_faces_dir: "generated_faces".into(),
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
        csrf_pepper: "test-csrf-pepper".to_string(),
        media_hash_blocklist: std::collections::HashSet::new(),
        recaptcha: None,
        oidc_google: None,
        stripe: None,
        platform_safety_contact_email: "safety@example.com".to_string(),
        council_web_search_api_key: None,
        council_web_search_provider: app::config::WebSearchProvider::None,
        council_web_search_cx: None,
        council_web_search_timeout_ms: 8000,
    }
}

async fn insert_child_community(
    pool: &MySqlPool,
    handle: &str,
    parent_id: u64,
    rating: &str,
) -> u64 {
    let community_type: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? LIMIT 1",
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(ref_keys::KEY_ENTITY_TYPE_COMMUNITY)
    .fetch_one(pool)
    .await
    .expect("community type");
    let verified_status: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? LIMIT 1",
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_STATUS)
    .bind(ref_keys::KEY_VERIFIED)
    .fetch_one(pool)
    .await
    .expect("verified");
    let join_open: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? LIMIT 1",
    )
    .bind(ref_keys::GROUP_SOCIAL_JOIN_POLICY)
    .bind(ref_keys::KEY_JOIN_POLICY_OPEN)
    .fetch_one(pool)
    .await
    .expect("join open");
    let loc: u64 = sqlx::query_scalar("SELECT id FROM locations LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("location");
    let system_user: u64 = sqlx::query_scalar("SELECT id FROM users WHERE email = ? LIMIT 1")
        .bind(ref_keys::SYSTEM_USER_EMAIL)
        .fetch_one(pool)
        .await
        .expect("system user");

    sqlx::query(
        r#"
        INSERT INTO social_entity (handle, type_id, status_id, entity_status_id, location_id, rating, created_by, modified_by)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(handle)
    .bind(community_type)
    .bind(verified_status)
    .bind(verified_status)
    .bind(loc)
    .bind(rating)
    .bind(system_user)
    .bind(system_user)
    .execute(pool)
    .await
    .expect("entity");
    let id: u64 = sqlx::query_scalar("SELECT id FROM social_entity WHERE handle = ?")
        .bind(handle)
        .fetch_one(pool)
        .await
        .expect("id");
    sqlx::query(
        r#"
        INSERT INTO social_community (social_entity_id, label, join_policy_id, parent_community_id)
        VALUES (?, ?, ?, ?)
        "#,
    )
    .bind(id)
    .bind(handle)
    .bind(join_open)
    .bind(parent_id)
    .execute(pool)
    .await
    .expect("community");
    id
}

async fn insert_test_human_with_user(
    pool: &MySqlPool,
    handle: &str,
    rating: &str,
) -> (u64, u64) {
    let human_type: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? LIMIT 1",
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(ref_keys::KEY_ENTITY_TYPE_HUMAN)
    .fetch_one(pool)
    .await
    .expect("human type");
    let verified_status: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? LIMIT 1",
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_STATUS)
    .bind(ref_keys::KEY_VERIFIED)
    .fetch_one(pool)
    .await
    .expect("verified");
    let join_open: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? LIMIT 1",
    )
    .bind(ref_keys::GROUP_SOCIAL_JOIN_POLICY)
    .bind(ref_keys::KEY_JOIN_POLICY_OPEN)
    .fetch_one(pool)
    .await
    .expect("join open");
    let loc: u64 = sqlx::query_scalar("SELECT id FROM locations LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("location");
    let system_user: u64 = sqlx::query_scalar("SELECT id FROM users WHERE email = ? LIMIT 1")
        .bind(ref_keys::SYSTEM_USER_EMAIL)
        .fetch_one(pool)
        .await
        .expect("system user");

    sqlx::query(
        r#"
        INSERT INTO social_entity (handle, type_id, status_id, entity_status_id, location_id, rating, created_by, modified_by)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(handle)
    .bind(human_type)
    .bind(verified_status)
    .bind(verified_status)
    .bind(loc)
    .bind(rating)
    .bind(system_user)
    .bind(system_user)
    .execute(pool)
    .await
    .expect("entity");
    let entity_id: u64 = sqlx::query_scalar("SELECT id FROM social_entity WHERE handle = ?")
        .bind(handle)
        .fetch_one(pool)
        .await
        .expect("entity id");
    sqlx::query(
        r#"
        INSERT INTO social_human (social_entity_id, family_name, given_name, name_sort_key, birth_date, join_policy_id)
        VALUES (?, 'Test', 'User', 'test user', '1990-01-01', ?)
        "#,
    )
    .bind(entity_id)
    .bind(join_open)
    .execute(pool)
    .await
    .expect("human row");

    let user_status: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? LIMIT 1",
    )
    .bind(ref_keys::GROUP_USER_STATUS)
    .bind(ref_keys::KEY_VERIFIED)
    .fetch_one(pool)
    .await
    .expect("user verified");
    let user_type: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? LIMIT 1",
    )
    .bind(ref_keys::GROUP_USER_TYPE)
    .bind(ref_keys::KEY_USER_TYPE_HUMAN)
    .fetch_one(pool)
    .await
    .expect("user type");
    let tier: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? LIMIT 1",
    )
    .bind(ref_keys::GROUP_VERIFICATION_TIER)
    .bind(ref_keys::KEY_VERIFICATION_TIER_BASIC)
    .fetch_one(pool)
    .await
    .expect("tier");

    let email = format!("{handle}@inactivity.test");
    sqlx::query(
        r#"
        INSERT INTO users (
            email, password_hash, status_id, user_status_id, user_type_id,
            verification_tier_id, email_verified_at, last_login_at, inactivity_penalties_applied
        )
        VALUES (?, 'x', ?, ?, ?, ?, UTC_TIMESTAMP(), DATE_SUB(UTC_TIMESTAMP(6), INTERVAL 28 DAY), 0)
        "#,
    )
    .bind(&email)
    .bind(user_status)
    .bind(user_status)
    .bind(user_type)
    .bind(tier)
    .execute(pool)
    .await
    .expect("user");
    let user_id: u64 = sqlx::query_scalar("SELECT id FROM users WHERE email = ?")
        .bind(&email)
        .fetch_one(pool)
        .await
        .expect("user id");
    let owner_role: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = 'owner' LIMIT 1",
    )
    .bind(ref_keys::GROUP_USER_SOCIAL_ENTITY_ROLE)
    .fetch_one(pool)
    .await
    .expect("owner role");
    sqlx::query(
        r#"
        INSERT INTO user_social_entity_role (user_id, social_entity_id, role_id, is_primary, created_by, modified_by)
        VALUES (?, ?, ?, 1, ?, ?)
        "#,
    )
    .bind(user_id)
    .bind(entity_id)
    .bind(owner_role)
    .bind(system_user)
    .bind(system_user)
    .execute(pool)
    .await
    .expect("owner role row");

    (entity_id, user_id)
}

async fn council_parent_id(pool: &MySqlPool) -> u64 {
    sqlx::query_scalar(
        r#"
        SELECT se.id
        FROM social_entity se
        WHERE se.handle = 'creativity_leisure_adventure'
        LIMIT 1
        "#,
    )
    .fetch_one(pool)
    .await
    .expect("council parent")
}

#[tokio::test]
async fn human_login_inactivity_applies_penalty_and_sends_phase_emails() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let config = test_config(String::new());
    let (human_id, user_id) = insert_test_human_with_user(pool, "inact_human_idle", "2.50").await;

    let applied = community_graph::run_human_inactivity_penalty_batch(pool, &config, 16)
        .await
        .expect("batch");
    assert!(applied >= 1, "expected penalty application");

    let (rating, penalties, warning, decrement_started, debited): (
        Decimal,
        u32,
        Option<DateTime<Utc>>,
        Option<DateTime<Utc>>,
        Decimal,
    ) = sqlx::query_as(
            r#"
            SELECT se.rating, u.inactivity_penalties_applied, u.inactivity_warning_notified_at,
                   u.inactivity_decrement_started_notified_at, u.inactivity_rating_debited
            FROM social_entity se
            INNER JOIN user_social_entity_role ur ON ur.social_entity_id = se.id AND ur.is_primary = 1
            INNER JOIN users u ON u.id = ur.user_id
            WHERE se.id = ? AND u.id = ?
            "#,
        )
        .bind(human_id)
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("state");

    assert_eq!(rating, rating_after_one_inactivity_penalty());
    assert!(penalties >= 1);
    assert!(warning.is_some());
    assert!(decrement_started.is_some());
    assert!(debited > Decimal::ZERO);

    let events: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM human_graph_event
        WHERE human_social_entity_id = ? AND event_type = 'inactivity_penalty'
        "#,
    )
    .bind(human_id)
    .fetch_one(pool)
    .await
    .expect("events");
    assert!(events >= 1);
}

#[tokio::test]
async fn record_user_login_resets_inactivity_penalty_cycle() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let (human_id, user_id) = insert_test_human_with_user(pool, "inact_human_reset", "2.50").await;

    sqlx::query(
        r#"
        UPDATE users
        SET inactivity_penalties_applied = 3,
            inactivity_rating_debited = 0.10,
            inactivity_warning_notified_at = UTC_TIMESTAMP(6)
        WHERE id = ?
        "#,
    )
    .bind(user_id)
    .execute(pool)
    .await
    .expect("seed penalties");

    let rating_before: Decimal = sqlx::query_scalar(
        "SELECT rating FROM social_entity WHERE id = ?",
    )
    .bind(human_id)
    .fetch_one(pool)
    .await
    .expect("rating before");
    assert_eq!(rating_before, Decimal::new(250, 2));

    let outcome = session::record_user_login(pool, user_id)
        .await
        .expect("login record");
    assert_eq!(outcome.inactivity_refunded, Decimal::new(10, 2));

    let rating_after: Decimal = sqlx::query_scalar(
        "SELECT rating FROM social_entity WHERE id = ?",
    )
    .bind(human_id)
    .fetch_one(pool)
    .await
    .expect("rating after");
    assert_eq!(rating_after, Decimal::new(260, 2));

    let (penalties, debited, warning, last_login): (
        u32,
        Decimal,
        Option<DateTime<Utc>>,
        DateTime<Utc>,
    ) = sqlx::query_as(
            r#"
            SELECT inactivity_penalties_applied, inactivity_rating_debited,
                   inactivity_warning_notified_at, last_login_at
            FROM users WHERE id = ?
            "#,
        )
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("user row");

    assert_eq!(penalties, 0);
    assert_eq!(debited, Decimal::ZERO);
    assert!(warning.is_none());
    let age_secs = (Utc::now() - last_login).num_seconds().abs();
    assert!(age_secs < 120, "last_login_at should be fresh");
}

#[tokio::test]
async fn flush_rating_failing_claims_global_entity_once() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let config = test_config(String::new());
    let parent = council_parent_id(pool).await;
    let comm_id = insert_child_community(pool, "inact_comm_failing", parent, "0.65").await;

    rating_notify::flush_rating_failing_notifications(pool, &config, 8).await;

    let first: Option<DateTime<Utc>> = sqlx::query_scalar(
        "SELECT rating_failing_notified_at FROM social_entity WHERE id = ?",
    )
    .bind(comm_id)
    .fetch_one(pool)
    .await
    .expect("notified");
    assert!(first.is_some(), "flush should claim failing notification");

    rating_notify::flush_rating_failing_notifications(pool, &config, 8).await;

    let second: Option<DateTime<Utc>> = sqlx::query_scalar(
        "SELECT rating_failing_notified_at FROM social_entity WHERE id = ?",
    )
    .bind(comm_id)
    .fetch_one(pool)
    .await
    .expect("notified again");
    assert_eq!(first, second, "second flush must not re-claim");
}

#[tokio::test]
async fn flush_rating_failing_claims_member_edge_once() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let config = test_config(String::new());
    let parent = council_parent_id(pool).await;
    let comm_id = insert_child_community(pool, "inact_comm_edge_fail", parent, "2.50").await;
    let (human_id, _user_id) = insert_test_human_with_user(pool, "inact_human_edge", "2.50").await;

    let member_type: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? LIMIT 1",
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
    .fetch_one(pool)
    .await
    .expect("member_of");
    let rel_status: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? LIMIT 1",
    )
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_VERIFIED)
    .fetch_one(pool)
    .await
    .expect("verified");
    let system_user: u64 = sqlx::query_scalar("SELECT id FROM users WHERE email = ? LIMIT 1")
        .bind(ref_keys::SYSTEM_USER_EMAIL)
        .fetch_one(pool)
        .await
        .expect("system");

    sqlx::query(
        r#"
        INSERT INTO social_entity_relationship (
            from_social_entity_id, to_social_entity_id, type_id, status_id,
            relationship_status_id, rating, started_at, created_by, modified_by
        ) VALUES (?, ?, ?, ?, ?, 0.65, UTC_TIMESTAMP(6), ?, ?)
        "#,
    )
    .bind(human_id)
    .bind(comm_id)
    .bind(member_type)
    .bind(rel_status)
    .bind(rel_status)
    .bind(system_user)
    .bind(system_user)
    .execute(pool)
    .await
    .expect("edge");

    let rel_id: u64 = sqlx::query_scalar(
        r#"
        SELECT id FROM social_entity_relationship
        WHERE from_social_entity_id = ? AND to_social_entity_id = ?
        ORDER BY id DESC LIMIT 1
        "#,
    )
    .bind(human_id)
    .bind(comm_id)
    .fetch_one(pool)
    .await
    .expect("rel id");

    rating_notify::flush_rating_failing_notifications(pool, &config, 8).await;

    let first: Option<DateTime<Utc>> = sqlx::query_scalar(
        "SELECT rating_failing_notified_at FROM social_entity_relationship WHERE id = ?",
    )
    .bind(rel_id)
    .fetch_one(pool)
    .await
    .expect("edge notified");
    assert!(first.is_some());

    rating_notify::flush_rating_failing_notifications(pool, &config, 8).await;

    let second: Option<DateTime<Utc>> = sqlx::query_scalar(
        "SELECT rating_failing_notified_at FROM social_entity_relationship WHERE id = ?",
    )
    .bind(rel_id)
    .fetch_one(pool)
    .await
    .expect("edge notified again");
    assert_eq!(first, second);
}

#[tokio::test]
async fn flush_rating_failing_skips_death_immune_panoply() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let config = test_config(String::new());

    sqlx::query(
        r#"
        UPDATE social_entity
        SET rating = 0.65, rating_failing_notified_at = NULL
        WHERE handle = ?
        "#,
    )
    .bind(ref_keys::PANOPLY_COMMUNITY_HANDLE)
    .execute(pool)
    .await
    .expect("panoply rating");

    rating_notify::flush_rating_failing_notifications(pool, &config, 8).await;

    let notified: Option<DateTime<Utc>> = sqlx::query_scalar(
        r#"
        SELECT rating_failing_notified_at
        FROM social_entity
        WHERE handle = ?
        "#,
    )
    .bind(ref_keys::PANOPLY_COMMUNITY_HANDLE)
    .fetch_one(pool)
    .await
    .expect("panoply row");
    assert!(
        notified.is_none(),
        "death-immune @panoply must not receive failing notification claims"
    );
}

#[tokio::test]
async fn flush_rating_failing_skips_panoply_operator_human() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let config = test_config(String::new());

    sqlx::query(
        r#"
        UPDATE social_entity
        SET rating = 0.65, rating_failing_notified_at = NULL
        WHERE handle = ?
        "#,
    )
    .bind(ref_keys::SEED_PANOPLY_OPERATOR_HANDLE)
    .execute(pool)
    .await
    .expect("sphinx rating");

    rating_notify::flush_rating_failing_notifications(pool, &config, 8).await;

    let notified: Option<DateTime<Utc>> = sqlx::query_scalar(
        r#"
        SELECT rating_failing_notified_at
        FROM social_entity
        WHERE handle = ?
        "#,
    )
    .bind(ref_keys::SEED_PANOPLY_OPERATOR_HANDLE)
    .fetch_one(pool)
    .await
    .expect("sphinx row");
    assert!(
        notified.is_none(),
        "Panoply operator human must not receive global failing notification claims"
    );
}
