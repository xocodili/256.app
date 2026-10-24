//! Godmother outreach facility — welcome, global failing/recovery, idempotency, no-edge skip.

mod common;

use app::config::Config;
use app::jobs::{enqueue_job, run_conductor_tick};
use app::ref_keys;
use app::social::feed;
use app::social::godmother::{flush_recovery_candidates, GodmotherMoment};
use common::RefKeys;
use serde_json::json;
use sqlx::MySqlPool;

struct GodmotherFixture {
    uid: u64,
    human_entity_id: u64,
    godmother_entity_id: u64,
}

async fn seed_human_with_godmother(pool: &MySqlPool, handle: &str) -> GodmotherFixture {
    let rk = RefKeys::load(pool).await;

    let godmother_entity_id: u64 = sqlx::query_scalar(
        r#"
        SELECT se.id
        FROM ai_actor aa
        INNER JOIN social_entity se ON se.id = aa.social_entity_id
        WHERE aa.slug = 'epic'
        LIMIT 1
        "#,
    )
    .fetch_one(pool)
    .await
    .expect("epic council entity");

    let location_id: u64 = sqlx::query_scalar("SELECT id FROM locations ORDER BY id ASC LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("location");

    let email = format!("godmother.outreach.{handle}@256.test");
    sqlx::query(
        r#"
        INSERT INTO users (
            email, password_hash, email_verified_at, status_id, user_status_id, user_type_id, verification_tier_id
        ) VALUES (?, NULL, UTC_TIMESTAMP(), ?, ?, ?, ?)
        "#,
    )
    .bind(&email)
    .bind(rk.id("status", "verified"))
    .bind(rk.id("user_status", "verified"))
    .bind(rk.id("user_type", "human"))
    .bind(rk.id("verification_tier", "basic"))
    .execute(pool)
    .await
    .expect("user");

    let uid: u64 = sqlx::query_scalar("SELECT id FROM users WHERE email = ? LIMIT 1")
        .bind(&email)
        .fetch_one(pool)
        .await
        .expect("uid");

    sqlx::query(
        r#"
        INSERT INTO social_entity (
            handle, type_id, status_id, entity_status_id, location_id, rating, created_by, modified_by
        ) VALUES (?, ?, ?, ?, ?, '2.50', ?, ?)
        "#,
    )
    .bind(handle)
    .bind(rk.id("social_entity_type", "human"))
    .bind(rk.id("status", "verified"))
    .bind(rk.id("social_entity_status", "verified"))
    .bind(location_id)
    .bind(uid)
    .bind(uid)
    .execute(pool)
    .await
    .expect("human entity");

    let human_entity_id: u64 =
        sqlx::query_scalar("SELECT id FROM social_entity WHERE handle = ? LIMIT 1")
            .bind(handle)
            .fetch_one(pool)
            .await
            .expect("human id");

    sqlx::query(
        r#"
        INSERT INTO user_social_entity_role (
            user_id, social_entity_id, role_id, is_primary, starts_at, created_by, modified_by
        ) VALUES (?, ?, ?, 1, UTC_DATE(), ?, ?)
        "#,
    )
    .bind(uid)
    .bind(human_entity_id)
    .bind(rk.id("user_social_entity_role", "owner"))
    .bind(uid)
    .bind(uid)
    .execute(pool)
    .await
    .expect("owner role");

    sqlx::query(
        r#"
        INSERT INTO social_entity_relationship (
            from_social_entity_id, to_social_entity_id, type_id, status_id, relationship_status_id,
            started_at, ended_at, created_by, modified_by
        ) VALUES (?, ?, ?, ?, ?, UTC_DATE(), NULL, ?, ?)
        "#,
    )
    .bind(human_entity_id)
    .bind(godmother_entity_id)
    .bind(rk.id("social_entity_relationship_type", "godmother"))
    .bind(rk.id("status", "verified"))
    .bind(rk.id("social_relationship_status", "verified"))
    .bind(uid)
    .bind(uid)
    .execute(pool)
    .await
    .expect("godmother edge");

    GodmotherFixture {
        uid,
        human_entity_id,
        godmother_entity_id,
    }
}

async fn private_post_count(
    pool: &MySqlPool,
    godmother_entity_id: u64,
    human_entity_id: u64,
) -> i64 {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM post p
        INNER JOIN post_audience_entity pae ON pae.post_id = p.id
        INNER JOIN post_audience_scope pas ON pas.post_id = p.id AND pas.scope_key = 'private'
        WHERE p.entity_id = ?
          AND pae.audience_entity_id = ?
        "#,
    )
    .bind(godmother_entity_id)
    .bind(human_entity_id)
    .fetch_one(pool)
    .await
    .expect("count")
}

async fn run_moment(pool: &MySqlPool, config: &Config, uid: u64, human_entity_id: u64, moment: GodmotherMoment) {
    let enqueued = enqueue_job(
        pool,
        Some(uid),
        "godmother_outreach",
        &json!({
            "human_entity_id": human_entity_id,
            "moment": moment.key(),
        }),
        Some("test"),
        40,
    )
    .await
    .expect("enqueue");
    assert!(enqueued.job_id > 0);

    let summary = run_conductor_tick(pool, config, 8)
        .await
        .expect("conductor");
    assert!(summary.completed >= 1);
}

#[tokio::test]
async fn godmother_outreach_welcome_publishes_private_post() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let fx = seed_human_with_godmother(pool, "godmother_outreach_welcome").await;
    let config = Config::from_env().expect("config from env");

    run_moment(pool, &config, fx.uid, fx.human_entity_id, GodmotherMoment::Welcome).await;

    assert_eq!(
        private_post_count(pool, fx.godmother_entity_id, fx.human_entity_id).await,
        1
    );
}

#[tokio::test]
async fn godmother_outreach_welcome_is_idempotent() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let fx = seed_human_with_godmother(pool, "godmother_outreach_idempotent").await;
    let config = Config::from_env().expect("config from env");

    run_moment(pool, &config, fx.uid, fx.human_entity_id, GodmotherMoment::Welcome).await;
    run_moment(pool, &config, fx.uid, fx.human_entity_id, GodmotherMoment::Welcome).await;

    assert_eq!(
        private_post_count(pool, fx.godmother_entity_id, fx.human_entity_id).await,
        1
    );
}

#[tokio::test]
async fn godmother_outreach_skips_without_godmother_edge() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let rk = RefKeys::load(pool).await;
    let config = Config::from_env().expect("config from env");

    let location_id: u64 = sqlx::query_scalar("SELECT id FROM locations ORDER BY id ASC LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("location");

    sqlx::query(
        r#"
        INSERT INTO users (
            email, password_hash, email_verified_at, status_id, user_status_id, user_type_id, verification_tier_id
        ) VALUES ('godmother.noedge@256.test', NULL, UTC_TIMESTAMP(), ?, ?, ?, ?)
        "#,
    )
    .bind(rk.id("status", "verified"))
    .bind(rk.id("user_status", "verified"))
    .bind(rk.id("user_type", "human"))
    .bind(rk.id("verification_tier", "basic"))
    .execute(pool)
    .await
    .expect("user");

    let uid: u64 = sqlx::query_scalar("SELECT id FROM users WHERE email = 'godmother.noedge@256.test' LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("uid");

    sqlx::query(
        r#"
        INSERT INTO social_entity (
            handle, type_id, status_id, entity_status_id, location_id, rating, created_by, modified_by
        ) VALUES ('godmother_no_edge_human', ?, ?, ?, ?, '2.50', ?, ?)
        "#,
    )
    .bind(rk.id("social_entity_type", "human"))
    .bind(rk.id("status", "verified"))
    .bind(rk.id("social_entity_status", "verified"))
    .bind(location_id)
    .bind(uid)
    .bind(uid)
    .execute(pool)
    .await
    .expect("human");

    let human_entity_id: u64 = sqlx::query_scalar(
        "SELECT id FROM social_entity WHERE handle = 'godmother_no_edge_human' LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .expect("human id");

    run_moment(pool, &config, uid, human_entity_id, GodmotherMoment::Welcome).await;

    let post_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM post WHERE entity_id IS NOT NULL")
        .fetch_one(pool)
        .await
        .expect("posts");
    let _ = post_count;
    let ledger: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM godmother_outreach WHERE human_entity_id = ?",
    )
    .bind(human_entity_id)
    .fetch_one(pool)
    .await
    .expect("ledger");
    assert_eq!(ledger, 0);
}

#[tokio::test]
async fn godmother_outreach_global_failing_publishes_private_post() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let fx = seed_human_with_godmother(pool, "godmother_outreach_failing").await;
    let config = Config::from_env().expect("config from env");

    sqlx::query(
        r#"
        UPDATE social_entity
        SET rating = '0.50', rating_failing_notified_at = UTC_TIMESTAMP(6)
        WHERE id = ?
        "#,
    )
    .bind(fx.human_entity_id)
    .execute(pool)
    .await
    .expect("failing rating");

    run_moment(
        pool,
        &config,
        fx.uid,
        fx.human_entity_id,
        GodmotherMoment::GlobalFailing,
    )
    .await;

    assert_eq!(
        private_post_count(pool, fx.godmother_entity_id, fx.human_entity_id).await,
        1
    );

    let moment_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM godmother_outreach
        WHERE human_entity_id = ? AND moment_key = 'global_failing'
        "#,
    )
    .bind(fx.human_entity_id)
    .fetch_one(pool)
    .await
    .expect("ledger");
    assert_eq!(moment_count, 1);
}

#[tokio::test]
async fn godmother_outreach_global_recovery_flush_enqueues_and_publishes() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let fx = seed_human_with_godmother(pool, "godmother_outreach_recovery").await;
    let config = Config::from_env().expect("config from env");

    sqlx::query(
        r#"
        UPDATE social_entity
        SET rating = '2.50',
            rating_failing_notified_at = UTC_TIMESTAMP(6),
            global_recovery_notified_at = NULL
        WHERE id = ?
        "#,
    )
    .bind(fx.human_entity_id)
    .execute(pool)
    .await
    .expect("recovery candidate");

    flush_recovery_candidates(pool, &config, 8).await;

    let summary = run_conductor_tick(pool, &config, 8)
        .await
        .expect("conductor");
    assert!(summary.completed >= 1);

    assert_eq!(
        private_post_count(pool, fx.godmother_entity_id, fx.human_entity_id).await,
        1
    );

    let recovery_notified: bool = sqlx::query_scalar(
        "SELECT global_recovery_notified_at IS NOT NULL FROM social_entity WHERE id = ?",
    )
    .bind(fx.human_entity_id)
    .fetch_one(pool)
    .await
    .expect("recovery ts");
    assert!(recovery_notified);
}

async fn seed_human_only(pool: &MySqlPool, handle: &str) -> (u64, u64) {
    let rk = RefKeys::load(pool).await;
    let location_id: u64 = sqlx::query_scalar("SELECT id FROM locations ORDER BY id ASC LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("location");

    let email = format!("godmother.feed.{handle}@256.test");
    sqlx::query(
        r#"
        INSERT INTO users (
            email, password_hash, email_verified_at, status_id, user_status_id, user_type_id, verification_tier_id
        ) VALUES (?, NULL, UTC_TIMESTAMP(), ?, ?, ?, ?)
        "#,
    )
    .bind(&email)
    .bind(rk.id("status", "verified"))
    .bind(rk.id("user_status", "verified"))
    .bind(rk.id("user_type", "human"))
    .bind(rk.id("verification_tier", "basic"))
    .execute(pool)
    .await
    .expect("user");

    let uid: u64 = sqlx::query_scalar("SELECT id FROM users WHERE email = ? LIMIT 1")
        .bind(&email)
        .fetch_one(pool)
        .await
        .expect("uid");

    sqlx::query(
        r#"
        INSERT INTO social_entity (
            handle, type_id, status_id, entity_status_id, location_id, rating, created_by, modified_by
        ) VALUES (?, ?, ?, ?, ?, '2.50', ?, ?)
        "#,
    )
    .bind(handle)
    .bind(rk.id("social_entity_type", "human"))
    .bind(rk.id("status", "verified"))
    .bind(rk.id("social_entity_status", "verified"))
    .bind(location_id)
    .bind(uid)
    .bind(uid)
    .execute(pool)
    .await
    .expect("human entity");

    let human_entity_id: u64 =
        sqlx::query_scalar("SELECT id FROM social_entity WHERE handle = ? LIMIT 1")
            .bind(handle)
            .fetch_one(pool)
            .await
            .expect("human id");

    sqlx::query(
        r#"
        INSERT INTO social_human (
            social_entity_id, family_name, given_name, birth_date, birth_location_id, join_policy_id, ai_tokens
        ) VALUES (?, 'Test', 'Human', '1990-01-01', ?, ?, 32)
        "#,
    )
    .bind(human_entity_id)
    .bind(location_id)
    .bind(rk.id("social_join_policy", "open"))
    .execute(pool)
    .await
    .expect("social_human");

    sqlx::query(
        r#"
        INSERT INTO user_social_entity_role (
            user_id, social_entity_id, role_id, is_primary, starts_at, created_by, modified_by
        ) VALUES (?, ?, ?, 1, UTC_DATE(), ?, ?)
        "#,
    )
    .bind(uid)
    .bind(human_entity_id)
    .bind(rk.id("user_social_entity_role", "owner"))
    .bind(uid)
    .bind(uid)
    .execute(pool)
    .await
    .expect("owner role");

    (uid, human_entity_id)
}

async fn insert_verified_friendship(pool: &MySqlPool, from_id: u64, to_id: u64) {
    let rk = RefKeys::load(pool).await;
    sqlx::query(
        r#"
        INSERT INTO social_entity_relationship (
            from_social_entity_id, to_social_entity_id, type_id, status_id, relationship_status_id,
            started_at, ended_at, created_by, modified_by
        ) VALUES (?, ?, ?, ?, ?, UTC_DATE(), NULL, 1, 1)
        "#,
    )
    .bind(from_id)
    .bind(to_id)
    .bind(rk.id("social_entity_relationship_type", ref_keys::KEY_RELATIONSHIP_FRIEND))
    .bind(rk.id("status", ref_keys::KEY_VERIFIED))
    .bind(rk.id("social_relationship_status", ref_keys::KEY_VERIFIED))
    .execute(pool)
    .await
    .expect("friend edge");
}

#[tokio::test]
async fn private_godmother_welcome_hidden_from_friend_feed() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let fx = seed_human_with_godmother(pool, "godmother_feed_target").await;
    let (friend_uid, friend_entity_id) = seed_human_only(pool, "godmother_feed_friend").await;
    insert_verified_friendship(pool, fx.human_entity_id, friend_entity_id).await;

    let config = Config::from_env().expect("config from env");
    run_moment(pool, &config, fx.uid, fx.human_entity_id, GodmotherMoment::Welcome).await;

    assert_eq!(
        private_post_count(pool, fx.godmother_entity_id, fx.human_entity_id).await,
        1
    );

    let private_post_id: u64 = sqlx::query_scalar(
        r#"
        SELECT p.id
        FROM post p
        INNER JOIN post_audience_scope pas ON pas.post_id = p.id AND pas.scope_key = 'private'
        WHERE p.entity_id = ?
        LIMIT 1
        "#,
    )
    .bind(fx.godmother_entity_id)
    .fetch_one(pool)
    .await
    .expect("private post id");

    let target_feed = feed::fetch_page_rows(pool, fx.uid, 0, 50, None, false)
        .await
        .expect("target feed");
    assert!(
        target_feed.rows.iter().any(|r| r.id == private_post_id),
        "target human should see their godmother welcome post"
    );

    let friend_feed = feed::fetch_page_rows(pool, friend_uid, 0, 50, None, false)
        .await
        .expect("friend feed");
    assert!(
        !friend_feed.rows.iter().any(|r| r.id == private_post_id),
        "verified friend must not see @private godmother post addressed to peer"
    );

    db.cleanup().await;
}

async fn insert_human_scoped_post(
    pool: &MySqlPool,
    author_entity_id: u64,
    author_user_id: u64,
    scopes: &[&str],
) -> u64 {
    let res = sqlx::query(
        r#"
        INSERT INTO post (
            entity_id, content_link_id, parent_post_id, root_post_id, status_id, rating,
            body_plain, body_markup, body_html_sanitized, published_at,
            created_by, modified_by, domain_id, country_code, is_moderation_mirror
        )
        VALUES (?, NULL, NULL, NULL, NULL, 0.00, ?, ?, ?, UTC_TIMESTAMP(6), ?, ?, NULL, NULL, 0)
        "#,
    )
    .bind(author_entity_id)
    .bind("scoped feed test")
    .bind("scoped feed test")
    .bind("scoped feed test")
    .bind(author_user_id)
    .bind(author_user_id)
    .execute(pool)
    .await
    .expect("post");
    let post_id = res.last_insert_id();
    sqlx::query(
        r#"INSERT INTO post_audience_entity (post_id, audience_entity_id, created_by) VALUES (?, ?, ?)"#,
    )
    .bind(post_id)
    .bind(author_entity_id)
    .bind(author_user_id)
    .execute(pool)
    .await
    .expect("audience entity");
    for scope in scopes {
        sqlx::query(
            r#"INSERT INTO post_audience_scope (post_id, scope_key, created_by) VALUES (?, ?, ?)"#,
        )
        .bind(post_id)
        .bind(*scope)
        .bind(author_user_id)
        .execute(pool)
        .await
        .expect("audience scope");
    }
    post_id
}

#[tokio::test]
async fn scoped_private_friends_post_visible_to_friend_not_colleague_only() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let author = seed_human_only(pool, "scoped_feed_author").await;
    let friend = seed_human_only(pool, "scoped_feed_friend").await;
    let other = seed_human_only(pool, "scoped_feed_other").await;
    insert_verified_friendship(pool, author.1, friend.1).await;

    let friends_post =
        insert_human_scoped_post(pool, author.1, author.0, &["private", "friends", "family"]).await;
    let colleague_post =
        insert_human_scoped_post(pool, author.1, author.0, &["private", "colleague"]).await;

    let friend_feed = feed::fetch_page_rows(pool, friend.0, 0, 50, None, false)
        .await
        .expect("friend feed");
    assert!(
        friend_feed.rows.iter().any(|r| r.id == friends_post),
        "friend should see @private @friends post"
    );
    assert!(
        !friend_feed.rows.iter().any(|r| r.id == colleague_post),
        "friend should not see @private @colleagues-only post"
    );

    let other_feed = feed::fetch_page_rows(pool, other.0, 0, 50, None, false)
        .await
        .expect("other feed");
    assert!(
        !other_feed.rows.iter().any(|r| r.id == friends_post),
        "unconnected viewer should not see scoped friend post"
    );

    db.cleanup().await;
}
