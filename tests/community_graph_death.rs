//! Community death at rating zero: memberships ended, children re-parented, status dead.

mod common;

use app::ref_keys;
use app::social::community_graph;
use common::TestDb;
use sqlx::MySqlPool;

async fn insert_test_community_tree(pool: &MySqlPool) -> (u64, u64, u64) {
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
    .expect("verified status");
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
    let system_user: u64 = sqlx::query_scalar(
        "SELECT id FROM users WHERE email = ? LIMIT 1",
    )
    .bind(ref_keys::SYSTEM_USER_EMAIL)
    .fetch_one(pool)
    .await
    .expect("system user");

    async fn mk(
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
            VALUES (?, ?, ?, ?, ?, 0.00, ?, ?)
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

    let a = mk(
        pool,
        "graph_death_parent_a",
        None,
        verified_type,
        verified_status,
        join_open,
        loc,
        system_user,
    )
    .await;
    let b = mk(
        pool,
        "graph_death_child_b",
        Some(a),
        verified_type,
        verified_status,
        join_open,
        loc,
        system_user,
    )
    .await;
    let c = mk(
        pool,
        "graph_death_grandchild_c",
        Some(b),
        verified_type,
        verified_status,
        join_open,
        loc,
        system_user,
    )
    .await;
    (a, b, c)
}

#[tokio::test]
async fn rating_zero_death_reparents_children_and_ends_memberships() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let (a, b, c) = insert_test_community_tree(pool).await;

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
    .expect("verified rel");
    let human_type: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? LIMIT 1",
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(ref_keys::KEY_ENTITY_TYPE_HUMAN)
    .fetch_one(pool)
    .await
    .expect("human");
    let system_user: u64 = sqlx::query_scalar(
        "SELECT id FROM users WHERE email = ? LIMIT 1",
    )
    .bind(ref_keys::SYSTEM_USER_EMAIL)
    .fetch_one(pool)
    .await
    .expect("system");
    let loc: u64 = sqlx::query_scalar("SELECT id FROM locations LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("loc");

    sqlx::query(
        r#"
        INSERT INTO social_entity (handle, type_id, status_id, entity_status_id, location_id, created_by, modified_by)
        VALUES ('graph_death_member_h', ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(human_type)
    .bind(rel_status)
    .bind(rel_status)
    .bind(loc)
    .bind(system_user)
    .bind(system_user)
    .execute(pool)
    .await
    .expect("human");
    let human_id: u64 = sqlx::query_scalar(
        "SELECT id FROM social_entity WHERE handle = 'graph_death_member_h'",
    )
    .fetch_one(pool)
    .await
    .expect("human id");
    let human_join_open: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? LIMIT 1",
    )
    .bind(ref_keys::GROUP_SOCIAL_JOIN_POLICY)
    .bind(ref_keys::KEY_JOIN_POLICY_OPEN)
    .fetch_one(pool)
    .await
    .expect("jp");
    sqlx::query(
        r#"
        INSERT INTO social_human (social_entity_id, join_policy_id, given_name, family_name, birth_date)
        VALUES (?, ?, 'Test', 'Member', '1990-01-01')
        "#,
    )
    .bind(human_id)
    .bind(human_join_open)
    .execute(pool)
    .await
    .expect("sh");

    sqlx::query(
        r#"
        INSERT INTO social_entity_relationship (
            from_social_entity_id, to_social_entity_id, type_id, status_id,
            relationship_status_id, rating, started_at, created_by, modified_by
        ) VALUES (?, ?, ?, ?, ?, 2.50, UTC_TIMESTAMP(6), ?, ?)
        "#,
    )
    .bind(human_id)
    .bind(b)
    .bind(member_type)
    .bind(rel_status)
    .bind(rel_status)
    .bind(system_user)
    .bind(system_user)
    .execute(pool)
    .await
    .expect("member edge");

    community_graph::run_death_batch(pool, 8).await.expect("death batch");

    let b_dead: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM social_entity se
            INNER JOIN ref_key_values rk ON rk.id = se.entity_status_id AND rk.`key` = ?
            WHERE se.id = ?
        )
        "#,
    )
    .bind(ref_keys::KEY_ENTITY_STATUS_DEAD)
    .bind(b)
    .fetch_one(pool)
    .await
    .expect("dead");
    assert!(b_dead);

    let c_parent: Option<u64> = sqlx::query_scalar(
        "SELECT parent_community_id FROM social_community WHERE social_entity_id = ?",
    )
    .bind(c)
    .fetch_one(pool)
    .await
    .expect("c parent");
    assert_eq!(c_parent, Some(a));

    let open_edges: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM social_entity_relationship
        WHERE to_social_entity_id = ? AND ended_at IS NULL
        "#,
    )
    .bind(b)
    .fetch_one(pool)
    .await
    .expect("edges");
    assert_eq!(open_edges, 0);

    let _ = a;
}

#[tokio::test]
async fn death_is_idempotent() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let (_a, b, _c) = insert_test_community_tree(pool).await;
    community_graph::run_death_batch(pool, 8).await.expect("first");
    community_graph::run_death_batch(pool, 8).await.expect("second");
    let still_dead: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM social_entity se
            INNER JOIN ref_key_values rk ON rk.id = se.entity_status_id AND rk.`key` = ?
            WHERE se.id = ?
        )
        "#,
    )
    .bind(ref_keys::KEY_ENTITY_STATUS_DEAD)
    .bind(b)
    .fetch_one(pool)
    .await
    .expect("dead");
    assert!(still_dead);
}
