//! Entity recommendation engine — graph-walk tiers for Search/Add.

mod common;

const PW: &str = "correct-horse-battery-staple";

use app::ref_keys;
use app::social::recommendations::{recommend_for_user, SECTION_DIRECT, SECTION_INFERRED};
use common::api::{seed_full_member, seed_location, TestApp};
use common::TestDb;

async fn lookup_ref_id(pool: &sqlx::MySqlPool, group: &str, key: &str) -> u64 {
    sqlx::query_scalar("SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? LIMIT 1")
        .bind(group)
        .bind(key)
        .fetch_one(pool)
        .await
        .expect("ref key")
}

async fn insert_verified_edge(
    pool: &sqlx::MySqlPool,
    from_id: u64,
    to_id: u64,
    rel_type_key: &str,
) {
    let type_id = lookup_ref_id(pool, ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE, rel_type_key).await;
    let status_id = lookup_ref_id(pool, ref_keys::GROUP_STATUS, ref_keys::KEY_VERIFIED).await;
    let rel_status_id =
        lookup_ref_id(pool, "social_relationship_status", ref_keys::KEY_VERIFIED).await;
    sqlx::query(
        r#"
        INSERT INTO social_entity_relationship
            (from_social_entity_id, to_social_entity_id, type_id, status_id, relationship_status_id, started_at)
        VALUES (?, ?, ?, ?, ?, UTC_TIMESTAMP())
        "#,
    )
    .bind(from_id)
    .bind(to_id)
    .bind(type_id)
    .bind(status_id)
    .bind(rel_status_id)
    .execute(pool)
    .await
    .expect("insert edge");
}

async fn create_test_community(
    pool: &sqlx::MySqlPool,
    owner_user_id: u64,
    handle: &str,
    join_policy_key: &str,
) -> u64 {
    let loc = seed_location(pool).await;
    let type_id = lookup_ref_id(pool, ref_keys::GROUP_SOCIAL_ENTITY_TYPE, ref_keys::KEY_ENTITY_TYPE_COMMUNITY).await;
    let status_id = lookup_ref_id(pool, ref_keys::GROUP_STATUS, ref_keys::KEY_VERIFIED).await;
    let entity_status_id =
        lookup_ref_id(pool, ref_keys::GROUP_SOCIAL_ENTITY_STATUS, ref_keys::KEY_VERIFIED).await;
    let join_policy_id = lookup_ref_id(pool, ref_keys::GROUP_SOCIAL_JOIN_POLICY, join_policy_key).await;

    let entity_id = sqlx::query(
        r#"
        INSERT INTO social_entity (handle, type_id, status_id, entity_status_id, location_id, created_by, rating)
        VALUES (?, ?, ?, ?, ?, ?, 3.00)
        "#,
    )
    .bind(handle)
    .bind(type_id)
    .bind(status_id)
    .bind(entity_status_id)
    .bind(loc)
    .bind(owner_user_id)
    .execute(pool)
    .await
    .expect("insert community entity")
    .last_insert_id();

    sqlx::query(
        r#"
        INSERT INTO social_community (social_entity_id, label, description, intention, join_policy_id)
        VALUES (?, ?, 'Test community', 'Testing recommendations', ?)
        "#,
    )
    .bind(entity_id)
    .bind(format!("Test {handle}"))
    .bind(join_policy_id)
    .execute(pool)
    .await
    .expect("insert social_community");

    let owner_role = lookup_ref_id(pool, ref_keys::GROUP_USER_SOCIAL_ENTITY_ROLE, ref_keys::KEY_USER_SOCIAL_ENTITY_ROLE_OWNER).await;
    sqlx::query(
        r#"
        INSERT INTO user_social_entity_role (user_id, social_entity_id, role_id, is_primary, starts_at)
        VALUES (?, ?, ?, 1, UTC_TIMESTAMP())
        "#,
    )
    .bind(owner_user_id)
    .bind(entity_id)
    .bind(owner_role)
    .execute(pool)
    .await
    .expect("owner role");

    entity_id
}

async fn ensure_human_profile(pool: &sqlx::MySqlPool, user_id: u64, handle: &str) -> u64 {
    if let Ok(existing) = sqlx::query_scalar::<_, u64>(
        r#"
        SELECT se.id
        FROM social_entity se
        INNER JOIN user_social_entity_role ur
          ON ur.social_entity_id = se.id
         AND ur.user_id = ?
         AND ur.ends_at IS NULL
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    {
        if let Some(id) = existing {
            return id;
        }
    }
    let loc = seed_location(pool).await;
    let type_id = lookup_ref_id(pool, ref_keys::GROUP_SOCIAL_ENTITY_TYPE, ref_keys::KEY_ENTITY_TYPE_HUMAN).await;
    let status_id = lookup_ref_id(pool, ref_keys::GROUP_STATUS, ref_keys::KEY_VERIFIED).await;
    let entity_status_id =
        lookup_ref_id(pool, ref_keys::GROUP_SOCIAL_ENTITY_STATUS, ref_keys::KEY_VERIFIED).await;
    let join_open = lookup_ref_id(pool, "social_join_policy", "open").await;

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
    .execute(pool)
    .await
    .expect("insert human entity")
    .last_insert_id();

    sqlx::query(
        r#"
        INSERT INTO social_human (social_entity_id, family_name, given_name, birth_date, birth_location_id, join_policy_id)
        VALUES (?, 'Tester', 'Rec', '1990-01-01', ?, ?)
        "#,
    )
    .bind(entity_id)
    .bind(loc)
    .bind(join_open)
    .execute(pool)
    .await
    .expect("insert social_human");

    let owner_role = lookup_ref_id(pool, ref_keys::GROUP_USER_SOCIAL_ENTITY_ROLE, ref_keys::KEY_USER_SOCIAL_ENTITY_ROLE_OWNER).await;
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
    .expect("owner role");

    entity_id
}

#[tokio::test]
async fn direct_tier_suggests_community_from_friend_member_of() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let viewer_user = seed_full_member(pool, "viewer@example.com", PW).await;
    let friend_user = seed_full_member(pool, "friend@example.com", PW).await;
    let owner_user = seed_full_member(pool, "owner@example.com", PW).await;

    let viewer_entity = ensure_human_profile(pool, viewer_user, "rec_viewer").await;
    let friend_entity = ensure_human_profile(pool, friend_user, "rec_friend").await;
    let community_id =
        create_test_community(pool, owner_user, "rec_gardening", ref_keys::KEY_JOIN_POLICY_OPEN).await;

    insert_verified_edge(pool, viewer_entity, friend_entity, ref_keys::KEY_RELATIONSHIP_FRIEND).await;
    insert_verified_edge(pool, friend_entity, community_id, ref_keys::KEY_RELATIONSHIP_MEMBER_OF).await;

    let payload = recommend_for_user(pool, viewer_user, 6)
        .await
        .expect("recommendations");

    let direct = payload
        .sections
        .iter()
        .find(|s| s.key == SECTION_DIRECT)
        .expect("direct section");
    let community_bucket = direct
        .types
        .iter()
        .find(|t| t.type_key == ref_keys::KEY_ENTITY_TYPE_COMMUNITY)
        .expect("community bucket");
    let handles: Vec<&str> = community_bucket
        .items
        .iter()
        .map(|i| i.handle.as_str())
        .collect();
    assert!(
        handles.contains(&"rec_gardening"),
        "expected rec_gardening in direct community suggestions, got {handles:?}"
    );

    db.cleanup().await;
}

#[tokio::test]
async fn excludes_entities_viewer_already_has_edge_to() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let viewer_user = seed_full_member(pool, "viewer2@example.com", PW).await;
    let friend_user = seed_full_member(pool, "friend2@example.com", PW).await;
    let owner_user = seed_full_member(pool, "owner2@example.com", PW).await;

    let viewer_entity = ensure_human_profile(pool, viewer_user, "rec_viewer2").await;
    let friend_entity = ensure_human_profile(pool, friend_user, "rec_friend2").await;
    let community_id =
        create_test_community(pool, owner_user, "rec_joined", ref_keys::KEY_JOIN_POLICY_OPEN).await;

    insert_verified_edge(pool, viewer_entity, friend_entity, ref_keys::KEY_RELATIONSHIP_FRIEND).await;
    insert_verified_edge(pool, friend_entity, community_id, ref_keys::KEY_RELATIONSHIP_MEMBER_OF).await;
    insert_verified_edge(pool, viewer_entity, community_id, ref_keys::KEY_RELATIONSHIP_MEMBER_OF).await;

    let payload = recommend_for_user(pool, viewer_user, 6)
        .await
        .expect("recommendations");

    for section in &payload.sections {
        for bucket in &section.types {
            for item in &bucket.items {
                assert_ne!(
                    item.handle, "rec_joined",
                    "viewer already member_of rec_joined — must not recommend"
                );
            }
        }
    }

    db.cleanup().await;
}

#[tokio::test]
async fn inferred_tier_suggests_via_shared_community_cohort() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let viewer_user = seed_full_member(pool, "cohort_viewer@example.com", PW).await;
    let stranger_user = seed_full_member(pool, "cohort_stranger@example.com", PW).await;
    let owner_user = seed_full_member(pool, "cohort_owner@example.com", PW).await;

    let viewer_entity = ensure_human_profile(pool, viewer_user, "rec_cohort_viewer").await;
    let stranger_entity = ensure_human_profile(pool, stranger_user, "rec_cohort_stranger").await;
    let shared_hub = create_test_community(pool, owner_user, "rec_shared_hub", ref_keys::KEY_JOIN_POLICY_OPEN).await;
    let inferred_target = create_test_community(
        pool,
        owner_user,
        "rec_inferred_target",
        ref_keys::KEY_JOIN_POLICY_OPEN,
    )
    .await;

    insert_verified_edge(pool, viewer_entity, shared_hub, ref_keys::KEY_RELATIONSHIP_MEMBER_OF).await;
    insert_verified_edge(pool, stranger_entity, shared_hub, ref_keys::KEY_RELATIONSHIP_MEMBER_OF).await;
    insert_verified_edge(
        pool,
        stranger_entity,
        inferred_target,
        ref_keys::KEY_RELATIONSHIP_MEMBER_OF,
    )
    .await;

    let payload = recommend_for_user(pool, viewer_user, 6)
        .await
        .expect("recommendations");

    let inferred = payload
        .sections
        .iter()
        .find(|s| s.key == SECTION_INFERRED)
        .expect("inferred section");
    let community_bucket = inferred
        .types
        .iter()
        .find(|t| t.type_key == ref_keys::KEY_ENTITY_TYPE_COMMUNITY)
        .expect("community bucket");
    let handles: Vec<&str> = community_bucket
        .items
        .iter()
        .map(|i| i.handle.as_str())
        .collect();
    assert!(
        handles.contains(&"rec_inferred_target"),
        "expected rec_inferred_target via shared community cohort, got {handles:?}"
    );

    if let Some(direct) = payload.sections.iter().find(|s| s.key == SECTION_DIRECT) {
        for bucket in &direct.types {
            for item in &bucket.items {
                assert_ne!(
                    item.handle, "rec_inferred_target",
                    "cohort-only target must not appear in direct tier"
                );
            }
        }
    }

    db.cleanup().await;
}

#[tokio::test]
async fn excludes_invite_only_communities_even_when_bridged() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let viewer_user = seed_full_member(pool, "invite_viewer@example.com", PW).await;
    let friend_user = seed_full_member(pool, "invite_friend@example.com", PW).await;
    let owner_user = seed_full_member(pool, "invite_owner@example.com", PW).await;

    let viewer_entity = ensure_human_profile(pool, viewer_user, "rec_invite_viewer").await;
    let friend_entity = ensure_human_profile(pool, friend_user, "rec_invite_friend").await;
    let community_id = create_test_community(
        pool,
        owner_user,
        "rec_invite_only",
        ref_keys::KEY_JOIN_POLICY_INVITE_ONLY,
    )
    .await;

    insert_verified_edge(pool, viewer_entity, friend_entity, ref_keys::KEY_RELATIONSHIP_FRIEND).await;
    insert_verified_edge(pool, friend_entity, community_id, ref_keys::KEY_RELATIONSHIP_MEMBER_OF).await;

    let payload = recommend_for_user(pool, viewer_user, 6)
        .await
        .expect("recommendations");

    for section in &payload.sections {
        for bucket in &section.types {
            for item in &bucket.items {
                assert_ne!(
                    item.handle, "rec_invite_only",
                    "invite-only communities must not appear in recommendations"
                );
            }
        }
    }

    db.cleanup().await;
}

#[tokio::test]
async fn api_returns_sectioned_recommendations_payload() {
    let app = TestApp::new().await;
    app.seed_full_member("api_viewer@example.com", PW).await;
    let sid = app.dev_session("api_viewer@example.com").await;

    let viewer_user = sqlx::query_scalar("SELECT id FROM users WHERE email = ?")
        .bind("api_viewer@example.com")
        .fetch_one(app.pool())
        .await
        .expect("viewer user");
    let _ = ensure_human_profile(app.pool(), viewer_user, "api_viewer").await;

    let resp = app
        .get_auth("/me/entity-recommendations?per_type=6", &sid)
        .await;
    assert_eq!(resp.status, axum::http::StatusCode::OK, "{}", resp.text());
    let json = resp.json();
    assert!(json.get("sections").is_some(), "response must include sections array");
    if let Some(section) = json["sections"].as_array().and_then(|s| s.first()) {
        if let Some(item) = section["types"]
            .as_array()
            .and_then(|types| types.first())
            .and_then(|bucket| bucket["items"].as_array())
            .and_then(|items| items.first())
        {
            assert!(
                item.get("bridge_count").is_none(),
                "bridge_count must not be exposed in API responses"
            );
            assert!(
                item.get("avatar_path").is_some(),
                "recommendation items must expose avatar_path (nullable)"
            );
        }
    }

    app.cleanup().await;
}
