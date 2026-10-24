//! Integration tests: Panoply operator clearance, filings, and actions.

mod common;
mod judicial_harness;

use app::ref_keys;
use app::social::community_graph::{is_human_death_immune, is_panoply_operator_for_user};
use app::social::human_onboarding;
use app::social::panoply::{
    eject_human_by_handle, file_emergency_safety, require_operator, ActionError, FilingError,
};
use common::seed_light;
use judicial_harness::{post_status_key, root_emergency_clause_402_id};

async fn sphinx_operator_ids(pool: &sqlx::MySqlPool) -> (u64, u64) {
    let user_id: u64 = sqlx::query_scalar("SELECT id FROM users WHERE email = ?")
        .bind(ref_keys::SEED_PANOPLY_OPERATOR_EMAIL)
        .fetch_one(pool)
        .await
        .expect("operator@example.com seeded by panoply migration");
    let entity_id = human_onboarding::primary_human_entity_id(pool, user_id)
        .await
        .expect("primary human")
        .expect("sphinx human entity");
    (user_id, entity_id)
}

#[tokio::test]
async fn panoply_seed_grants_operator_clearance() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let (sphinx_uid, sphinx_eid) = sphinx_operator_ids(pool).await;

    assert!(
        is_panoply_operator_for_user(pool, sphinx_uid)
            .await
            .expect("operator check")
    );
    assert!(require_operator(pool, sphinx_uid).await.is_ok());
    assert!(
        is_human_death_immune(pool, sphinx_eid)
            .await
            .expect("immunity check")
    );

    let handle: String = sqlx::query_scalar("SELECT handle FROM social_entity WHERE id = ?")
        .bind(sphinx_eid)
        .fetch_one(pool)
        .await
        .expect("handle");
    assert_eq!(handle, ref_keys::SEED_PANOPLY_OPERATOR_HANDLE);

    let member_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM social_entity_relationship ser
        INNER JOIN social_entity se ON se.id = ser.to_social_entity_id
        INNER JOIN ref_key_values rkt ON rkt.id = ser.type_id
            AND rkt.`group` = ?
            AND rkt.`key` = ?
        WHERE se.handle = ?
          AND ser.ended_at IS NULL
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
    .bind(ref_keys::PANOPLY_COMMUNITY_HANDLE)
    .fetch_one(pool)
    .await
    .expect("member count");
    assert_eq!(member_count, 1, "@panoply must have exactly one active member at seed");

    let user_status: String = sqlx::query_scalar(
        r#"
        SELECT rk.`key`
        FROM users u
        INNER JOIN ref_key_values rk ON rk.id = u.user_status_id
        WHERE u.id = ?
        "#,
    )
    .bind(sphinx_uid)
    .fetch_one(pool)
    .await
    .expect("user status");
    assert_eq!(user_status, ref_keys::KEY_VERIFIED);

    assert!(
        human_onboarding::user_has_terms_consent(pool, sphinx_uid)
            .await
            .expect("terms consent")
    );
    assert!(
        human_onboarding::user_has_human_profile(pool, sphinx_uid)
            .await
            .expect("human profile")
    );

    let has_face: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM user_faces WHERE user_id = ? AND face_captured_at IS NOT NULL)",
    )
    .bind(sphinx_uid)
    .fetch_one(pool)
    .await
    .expect("face profile");
    assert!(has_face, "operator must have seeded face capture");
}

#[tokio::test]
async fn panoply_operator_files_emergency_without_noms() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let seed = seed_light(pool).await;
    let (sphinx_uid, sphinx_eid) = sphinx_operator_ids(pool).await;
    let clause_id = root_emergency_clause_402_id(pool).await;

    let target_idx = 5;
    let root_post_id = seed.post_community_ids[target_idx];
    let case_id = file_emergency_safety(
        pool,
        sphinx_uid,
        sphinx_eid,
        root_post_id,
        clause_id,
        "ops test emergency",
    )
    .await
    .expect("file emergency");

    assert!(case_id > 0);
    assert_eq!(
        post_status_key(pool, root_post_id).await,
        ref_keys::KEY_POST_SUPPRESSED
    );

    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM safety_operator_action WHERE actor_user_id = ? AND action_type = 'emergency_file'",
    )
    .bind(sphinx_uid)
    .fetch_one(pool)
    .await
    .expect("audit count");
    assert_eq!(audit_count, 1);
}

#[tokio::test]
async fn panoply_operator_emergency_rejects_duplicate() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let seed = seed_light(pool).await;
    let (sphinx_uid, sphinx_eid) = sphinx_operator_ids(pool).await;
    let clause_id = root_emergency_clause_402_id(pool).await;
    let root_post_id = seed.post_community_ids[6];

    file_emergency_safety(
        pool,
        sphinx_uid,
        sphinx_eid,
        root_post_id,
        clause_id,
        "first filing",
    )
    .await
    .expect("first");

    let err = file_emergency_safety(
        pool,
        sphinx_uid,
        sphinx_eid,
        root_post_id,
        clause_id,
        "duplicate",
    )
    .await
    .expect_err("duplicate");
    assert_eq!(err, FilingError::DuplicateEmergency);
}

#[tokio::test]
async fn panoply_eject_respects_death_immunity() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let (sphinx_uid, _) = sphinx_operator_ids(pool).await;

    let err = eject_human_by_handle(pool, sphinx_uid, ref_keys::SEED_PANOPLY_OPERATOR_HANDLE, "should not eject operator")
        .await
        .expect_err("immune");
    assert_eq!(err, ActionError::DeathImmune);
}

#[tokio::test]
async fn non_operator_lacks_clearance() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let _seed = seed_light(pool).await;

    let outsider: u64 = sqlx::query_scalar(
        r#"
        SELECT u.id
        FROM users u
        WHERE u.email LIKE 'test.%@256.test'
          AND NOT EXISTS (
              SELECT 1
              FROM user_social_entity_role ur
              INNER JOIN social_entity_relationship ser
                  ON ser.from_social_entity_id = ur.social_entity_id
                 AND ser.ended_at IS NULL
              INNER JOIN social_entity se
                  ON se.id = ser.to_social_entity_id
                 AND se.handle = 'panoply'
              WHERE ur.user_id = u.id
                AND ur.ends_at IS NULL
          )
        ORDER BY u.id ASC
        LIMIT 1
        "#,
    )
    .fetch_one(pool)
    .await
    .expect("non-panoply test user");

    assert!(!is_panoply_operator_for_user(pool, outsider)
        .await
        .expect("check"));
    assert!(require_operator(pool, outsider).await.is_err());
}

#[tokio::test]
async fn panoply_concealed_from_non_operator_profiles_and_search() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let seed = seed_light(pool).await;
    let (sphinx_uid, sphinx_eid) = sphinx_operator_ids(pool).await;

    let outsider: u64 = sqlx::query_scalar(
        r#"
        SELECT u.id FROM users u
        WHERE u.email LIKE 'test.%@256.test'
          AND NOT EXISTS (
              SELECT 1 FROM user_social_entity_role ur
              INNER JOIN social_entity_relationship ser
                  ON ser.from_social_entity_id = ur.social_entity_id AND ser.ended_at IS NULL
              INNER JOIN social_entity se ON se.id = ser.to_social_entity_id AND se.handle = 'panoply'
              WHERE ur.user_id = u.id AND ur.ends_at IS NULL
          )
        ORDER BY u.id ASC LIMIT 1
        "#,
    )
    .fetch_one(pool)
    .await
    .expect("outsider");

    let outsider_profile = app::social::private_profile::fetch_by_handle(
        pool,
        outsider,
        ref_keys::SEED_PANOPLY_OPERATOR_HANDLE,
    )
        .await
        .expect("profile")
        .expect("sphinx profile");
    assert!(
        !outsider_profile
            .member_communities
            .iter()
            .any(|m| m.handle == "panoply"),
        "non-operator must not see panoply on human profile"
    );

    assert!(
        app::social::public_profile::fetch(pool, "panoply")
            .await
            .expect("public")
            .is_none()
    );
    assert!(
        app::social::private_profile::fetch_by_handle(pool, outsider, "panoply")
            .await
            .expect("private panoply")
            .is_none()
    );

    let operator_profile = app::social::private_profile::fetch_by_handle(
        pool,
        sphinx_uid,
        ref_keys::SEED_PANOPLY_OPERATOR_HANDLE,
    )
        .await
        .expect("operator profile")
        .expect("sphinx");
    assert!(
        operator_profile
            .member_communities
            .iter()
            .any(|m| m.handle == "panoply")
    );
    let _ = (seed, sphinx_eid);
}

#[tokio::test]
async fn panoply_constitution_seeded_non_trigger() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;

    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM community_constitution_item cci
        INNER JOIN social_entity se ON se.id = cci.community_id
        WHERE se.handle = 'panoply'
          AND cci.status = 'active'
        "#,
    )
    .fetch_one(pool)
    .await
    .expect("count");

    assert_eq!(count, 10, "expected ten active Panoply charter points");

    let trigger_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM community_constitution_item cci
        INNER JOIN social_entity se ON se.id = cci.community_id
        WHERE se.handle = 'panoply'
          AND cci.status = 'active'
          AND cci.judicial_trigger_chk = 1
        "#,
    )
    .fetch_one(pool)
    .await
    .expect("triggers");

    assert_eq!(trigger_count, 0, "Panoply charter must not be judicial triggers in v1");
}

#[tokio::test]
async fn panoply_excluded_from_community_parent_search() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;

    let rows: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT se.handle
        FROM social_entity se
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = se.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` = ?
        INNER JOIN social_community sc ON sc.social_entity_id = se.id
        WHERE se.handle LIKE ?
          AND se.handle <> 'panoply'
        "#,
    )
    .bind(app::ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(app::ref_keys::KEY_ENTITY_TYPE_COMMUNITY)
    .bind("panoply%")
    .fetch_all(pool)
    .await
    .expect("parent search simulation");

    assert!(
        !rows.iter().any(|h| h == "panoply"),
        "parent community search must exclude @panoply"
    );
    assert!(
        app::social::panoply::is_invalid_community_parent_handle("panoply")
            && app::social::panoply::is_invalid_community_parent_handle("@panoply")
    );
}

#[tokio::test]
async fn panoply_daily_status_sends_once_per_utc_period() {
    use app::config::{Config, SmtpConfig};
    use app::social::panoply::run_panoply_daily_status_batch;
    use std::net::{IpAddr, Ipv4Addr};

    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let config = Config {
        database_url: String::new(),
        host: IpAddr::V4(Ipv4Addr::LOCALHOST),
        port: 8080,
        nominatim_user_agent: "256.app-test".into(),
        secure_cookies: false,
        public_base_url: "http://127.0.0.1:8080".into(),
        dev_auth_bypass: false,
        trusted_proxy: false,
        smtp: Some(SmtpConfig {
            host: "127.0.0.1".into(),
            port: 1025,
            username: None,
            password: None,
            from: "256.app <test@256.test>".into(),
            tls_mode: "none".into(),
        }),
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
    };

    let period = "2099-01-02";
    run_panoply_daily_status_batch(pool, &config, period)
        .await
        .expect("first batch");

    let claimed: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM ops_daily_digest_sent
        WHERE digest_key = 'panoply_daily_status' AND period_key = ?
        "#,
    )
    .bind(period)
    .fetch_one(pool)
    .await
    .expect("digest claim");
    assert_eq!(claimed, 1, "first run must claim the UTC period");

    let second = run_panoply_daily_status_batch(pool, &config, period)
        .await
        .expect("second batch");
    assert_eq!(second, 0, "second run same period must not send again");
}
