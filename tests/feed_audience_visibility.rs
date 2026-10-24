//! Aggregate feed audience: relationship scopes, @private-only direct posts, godmother privacy.

mod common;

use app::ref_keys;
use app::social::feed;
use common::RefKeys;
use sqlx::MySqlPool;

struct HumanFixture {
    uid: u64,
    entity_id: u64,
}

async fn seed_human(pool: &MySqlPool, handle: &str) -> HumanFixture {
    let rk = RefKeys::load(pool).await;
    let location_id: u64 = sqlx::query_scalar("SELECT id FROM locations ORDER BY id ASC LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("location");

    let email = format!("feed.vis.{handle}@256.test");
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
    .expect("entity");

    let entity_id: u64 = sqlx::query_scalar("SELECT id FROM social_entity WHERE handle = ? LIMIT 1")
        .bind(handle)
        .fetch_one(pool)
        .await
        .expect("entity id");

    sqlx::query(
        r#"
        INSERT INTO social_human (
            social_entity_id, family_name, given_name, birth_date, birth_location_id, join_policy_id, ai_tokens
        ) VALUES (?, 'Test', 'Human', '1990-01-01', ?, ?, 32)
        "#,
    )
    .bind(entity_id)
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
    .bind(entity_id)
    .bind(rk.id("user_social_entity_role", "owner"))
    .bind(uid)
    .bind(uid)
    .execute(pool)
    .await
    .expect("owner role");

    HumanFixture { uid, entity_id }
}

async fn insert_verified_edge(pool: &MySqlPool, from_id: u64, to_id: u64, rel_key: &str) {
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
    .bind(rk.id("social_entity_relationship_type", rel_key))
    .bind(rk.id("status", ref_keys::KEY_VERIFIED))
    .bind(rk.id("social_relationship_status", ref_keys::KEY_VERIFIED))
    .execute(pool)
    .await
    .expect("edge");
}

async fn insert_scoped_post(
    pool: &MySqlPool,
    author_entity_id: u64,
    author_user_id: u64,
    body: &str,
    scopes: &[&str],
    audience_entity_ids: &[u64],
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
    .bind(body)
    .bind(body)
    .bind(body)
    .bind(author_user_id)
    .bind(author_user_id)
    .execute(pool)
    .await
    .expect("post");
    let post_id = res.last_insert_id();
    for eid in audience_entity_ids {
        sqlx::query(
            r#"INSERT INTO post_audience_entity (post_id, audience_entity_id, created_by) VALUES (?, ?, ?)"#,
        )
        .bind(post_id)
        .bind(*eid)
        .bind(author_user_id)
        .execute(pool)
        .await
        .expect("audience entity");
    }
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

fn feed_contains(pool_rows: &[feed::HomeFeedRow], post_id: u64) -> bool {
    pool_rows.iter().any(|r| r.id == post_id)
}

#[tokio::test]
async fn relationship_scoped_private_posts_respect_edge_types() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;

    let author = seed_human(pool, "feed_vis_author").await;
    let friend = seed_human(pool, "feed_vis_friend").await;
    let colleague = seed_human(pool, "feed_vis_colleague").await;
    let stranger = seed_human(pool, "feed_vis_stranger").await;

    insert_verified_edge(pool, author.entity_id, friend.entity_id, ref_keys::KEY_RELATIONSHIP_FRIEND).await;
    insert_verified_edge(
        pool,
        author.entity_id,
        colleague.entity_id,
        ref_keys::KEY_RELATIONSHIP_COLLEAGUE,
    )
    .await;

    let friends_post = insert_scoped_post(
        pool,
        author.entity_id,
        author.uid,
        "feed vis friends family private",
        &["private", "friends", "family"],
        &[author.entity_id],
    )
    .await;
    let colleagues_post = insert_scoped_post(
        pool,
        author.entity_id,
        author.uid,
        "feed vis colleagues private",
        &["private", "colleague"],
        &[author.entity_id],
    )
    .await;
    let private_direct = insert_scoped_post(
        pool,
        author.entity_id,
        author.uid,
        "feed vis private direct to friend",
        &["private"],
        &[friend.entity_id],
    )
    .await;
    let private_peer_leak = insert_scoped_post(
        pool,
        author.entity_id,
        author.uid,
        "feed vis private direct to colleague",
        &["private"],
        &[colleague.entity_id],
    )
    .await;

    let friend_feed = feed::fetch_page_rows(pool, friend.uid, 0, 100, None, false)
        .await
        .expect("friend feed");
    assert!(
        feed_contains(&friend_feed.rows, friends_post),
        "verified friend sees @private @friends post"
    );
    assert!(
        !feed_contains(&friend_feed.rows, colleagues_post),
        "friend must not see colleagues-only post"
    );
    assert!(
        feed_contains(&friend_feed.rows, private_direct),
        "recipient sees @private-only post addressed to them"
    );
    assert!(
        !feed_contains(&friend_feed.rows, private_peer_leak),
        "friend must not see @private-only post for a connected peer"
    );

    let colleague_feed = feed::fetch_page_rows(pool, colleague.uid, 0, 100, None, false)
        .await
        .expect("colleague feed");
    assert!(
        feed_contains(&colleague_feed.rows, colleagues_post),
        "verified colleague sees colleagues-only post"
    );
    assert!(
        !feed_contains(&colleague_feed.rows, friends_post),
        "colleague must not see friends-only post"
    );
    assert!(
        !feed_contains(&colleague_feed.rows, private_direct),
        "non-recipient must not see @private-only direct post"
    );

    let stranger_feed = feed::fetch_page_rows(pool, stranger.uid, 0, 100, None, false)
        .await
        .expect("stranger feed");
    assert!(
        !feed_contains(&stranger_feed.rows, friends_post),
        "stranger sees no scoped author posts"
    );
    assert!(
        !feed_contains(&stranger_feed.rows, colleagues_post),
        "stranger sees no scoped author posts"
    );

    let author_feed = feed::fetch_page_rows(pool, author.uid, 0, 100, None, false)
        .await
        .expect("author feed");
    assert!(feed_contains(&author_feed.rows, friends_post));
    assert!(feed_contains(&author_feed.rows, colleagues_post));

    db.cleanup().await;
}
