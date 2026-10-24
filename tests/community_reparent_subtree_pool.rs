//! Re-parent phase-2 juror pool counts distinct humans across the moving community subtree.

mod common;

use app::ref_keys;
use app::social::community_graph;
use app::social::judicial::community_reparent;
use common::TestDb;
use sqlx::MySqlPool;

async fn insert_member(
    pool: &MySqlPool,
    handle: &str,
    community_id: u64,
    member_type: u64,
    rel_status: u64,
    human_type: u64,
    loc: u64,
    system_user: u64,
) -> u64 {
    sqlx::query(
        r#"
        INSERT INTO social_entity (handle, type_id, status_id, entity_status_id, location_id, created_by, modified_by)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(handle)
    .bind(human_type)
    .bind(rel_status)
    .bind(rel_status)
    .bind(loc)
    .bind(system_user)
    .bind(system_user)
    .execute(pool)
    .await
    .expect("human entity");
    let human_id: u64 = sqlx::query_scalar("SELECT id FROM social_entity WHERE handle = ?")
        .bind(handle)
        .fetch_one(pool)
        .await
        .expect("id");
    let join_open: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? LIMIT 1",
    )
    .bind(ref_keys::GROUP_SOCIAL_JOIN_POLICY)
    .bind(ref_keys::KEY_JOIN_POLICY_OPEN)
    .fetch_one(pool)
    .await
    .expect("join");
    sqlx::query(
        r#"
        INSERT INTO social_human (social_entity_id, family_name, given_name, name_sort_key, birth_date, join_policy_id)
        VALUES (?, 'Test', 'User', 'test', '1990-01-01', ?)
        "#,
    )
    .bind(human_id)
    .bind(join_open)
    .execute(pool)
    .await
    .expect("human");
    sqlx::query(
        r#"
        INSERT INTO social_entity_relationship (
            from_social_entity_id, to_social_entity_id, type_id, status_id, rating, created_by, modified_by
        )
        VALUES (?, ?, ?, ?, 3.50, ?, ?)
        "#,
    )
    .bind(human_id)
    .bind(community_id)
    .bind(member_type)
    .bind(rel_status)
    .bind(system_user)
    .bind(system_user)
    .execute(pool)
    .await
    .expect("member_of");
    human_id
}

#[tokio::test]
async fn impacted_subtree_pool_counts_union_of_subtree_members() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let verified_type: u64 = sqlx::query_scalar(
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
    .expect("join");
    let loc: u64 = sqlx::query_scalar("SELECT id FROM locations LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("loc");
    let system_user: u64 = sqlx::query_scalar("SELECT id FROM users WHERE email = ? LIMIT 1")
        .bind(ref_keys::SYSTEM_USER_EMAIL)
        .fetch_one(pool)
        .await
        .expect("system");
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
    .expect("rel verified");
    let human_type: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? LIMIT 1",
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(ref_keys::KEY_ENTITY_TYPE_HUMAN)
    .fetch_one(pool)
    .await
    .expect("human type");

    async fn mk_community(
        pool: &MySqlPool,
        handle: &str,
        parent: Option<u64>,
        type_id: u64,
        status_id: u64,
        join_id: u64,
        loc_id: u64,
        actor: u64,
    ) -> u64 {
        sqlx::query(
            r#"
            INSERT INTO social_entity (handle, type_id, status_id, entity_status_id, location_id, rating, created_by, modified_by)
            VALUES (?, ?, ?, ?, ?, 2.50, ?, ?)
            "#,
        )
        .bind(handle)
        .bind(type_id)
        .bind(status_id)
        .bind(status_id)
        .bind(loc_id)
        .bind(actor)
        .bind(actor)
        .execute(pool)
        .await
        .expect("entity");
        let id = sqlx::query_scalar("SELECT id FROM social_entity WHERE handle = ?")
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
        .bind(join_id)
        .bind(parent)
        .execute(pool)
        .await
        .expect("community");
        id
    }

    let root = mk_community(
        pool,
        "reparent_pool_root",
        None,
        verified_type,
        verified_status,
        join_open,
        loc,
        system_user,
    )
    .await;
    let moving = mk_community(
        pool,
        "reparent_pool_moving",
        Some(root),
        verified_type,
        verified_status,
        join_open,
        loc,
        system_user,
    )
    .await;
    let child = mk_community(
        pool,
        "reparent_pool_child",
        Some(moving),
        verified_type,
        verified_status,
        join_open,
        loc,
        system_user,
    )
    .await;

    let _ = insert_member(
        pool,
        "reparent_pool_h_m",
        moving,
        member_type,
        rel_status,
        human_type,
        loc,
        system_user,
    )
    .await;
    let _ = insert_member(
        pool,
        "reparent_pool_h_child",
        child,
        member_type,
        rel_status,
        human_type,
        loc,
        system_user,
    )
    .await;

    let origin_only = community_reparent::count_origin_member_pool(pool, moving)
        .await
        .expect("origin pool");
    assert_eq!(origin_only, 1, "phase 1 pool is moving community members only");

    let impacted = community_reparent::count_impacted_subtree_humans(pool, moving)
        .await
        .expect("impacted pool");
    assert_eq!(
        impacted, 2,
        "phase 2 pool is distinct humans in subtree (moving + child)"
    );

    assert!(
        community_graph::would_create_cycle(pool, moving, child)
            .await
            .expect("cycle check"),
        "child in subtree cannot become parent of moving community"
    );
}
