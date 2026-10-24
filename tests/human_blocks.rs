//! Human block list — bi-directional mute and profile invisibility.

mod common;

use app::ref_keys;
use app::social::{blocks, private_profile};
use common::{seed_light, TestDb};

#[tokio::test]
async fn block_hides_profiles_both_ways_and_unblock_resets() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let seed = seed_light(pool).await;

    let blocker_user = seed.user_ids[0];
    let target_user = seed.user_ids[1];
    let blocker_entity = seed.entity_ids[0];
    let target_entity = seed.entity_ids[1];

    let target_handle: String = sqlx::query_scalar("SELECT handle FROM social_entity WHERE id = ?")
        .bind(target_entity)
        .fetch_one(pool)
        .await
        .expect("target handle");

    blocks::block_human(pool, blocker_entity, target_entity, blocker_user)
        .await
        .expect("block");

    let (edge_from, edge_to): (u64, u64) = sqlx::query_as(
        r#"
        SELECT ser.from_social_entity_id, ser.to_social_entity_id
        FROM social_entity_relationship ser
        INNER JOIN ref_key_values rk_st ON rk_st.id = ser.status_id AND rk_st.`key` = ?
        WHERE ser.ended_at IS NULL
          AND (
              (ser.from_social_entity_id = ? AND ser.to_social_entity_id = ?)
              OR (ser.from_social_entity_id = ? AND ser.to_social_entity_id = ?)
          )
        LIMIT 1
        "#,
    )
    .bind(ref_keys::KEY_BLOCKED)
    .bind(target_entity)
    .bind(blocker_entity)
    .bind(blocker_entity)
    .bind(target_entity)
    .fetch_one(pool)
    .await
    .expect("blocked edge");
    assert_eq!(edge_from, target_entity, "blockee is from");
    assert_eq!(edge_to, blocker_entity, "blocker is to");

    assert!(
        blocks::is_pair_blocked(pool, blocker_entity, target_entity)
            .await
            .expect("blocked check")
    );

    let blocker_view = private_profile::fetch_by_handle(pool, blocker_user, &target_handle)
        .await
        .expect("blocker fetch");
    assert!(blocker_view.is_none(), "blocker should not see blocked profile");

    let target_handle_blocker: String =
        sqlx::query_scalar("SELECT handle FROM social_entity WHERE id = ?")
            .bind(blocker_entity)
            .fetch_one(pool)
            .await
            .expect("blocker handle");

    let target_view =
        private_profile::fetch_by_handle(pool, target_user, &target_handle_blocker)
            .await
            .expect("target fetch");
    assert!(target_view.is_none(), "blocked user should not see blocker profile");

    let listed = blocks::list_blocked_by(pool, blocker_entity)
        .await
        .expect("list blocked");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].entity_id, target_entity);

    blocks::block_human(pool, blocker_entity, target_entity, blocker_user)
        .await
        .expect("block again");
    let blocked_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM social_entity_relationship ser
        INNER JOIN ref_key_values rk_st ON rk_st.id = ser.status_id AND rk_st.`key` = ?
        WHERE ser.ended_at IS NULL
          AND ser.from_social_entity_id = ?
          AND ser.to_social_entity_id = ?
        "#,
    )
    .bind(ref_keys::KEY_BLOCKED)
    .bind(target_entity)
    .bind(blocker_entity)
    .fetch_one(pool)
    .await
    .expect("blocked count");
    assert_eq!(blocked_count, 1, "repeat block must not stack active edges");

    // Someone blocked you: blockee (you) = from, blocker (them) = to.
    blocks::unblock_human(pool, blocker_entity, target_entity, blocker_user)
        .await
        .expect("unblock");
    let type_id: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? LIMIT 1",
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_FRIEND)
    .fetch_one(pool)
    .await
    .expect("friend type");
    let status_id: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? LIMIT 1",
    )
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_BLOCKED)
    .fetch_one(pool)
    .await
    .expect("blocked status");
    let rel_status_id: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? LIMIT 1",
    )
    .bind("social_relationship_status")
    .bind(ref_keys::KEY_BLOCKED)
    .fetch_one(pool)
    .await
    .expect("blocked rel status");
    sqlx::query(
        r#"
        INSERT INTO social_entity_relationship (
            from_social_entity_id, to_social_entity_id,
            type_id, status_id, relationship_status_id,
            created_by, modified_by
        )
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(blocker_entity)
    .bind(target_entity)
    .bind(type_id)
    .bind(status_id)
    .bind(rel_status_id)
    .bind(target_user)
    .bind(target_user)
    .execute(pool)
    .await
    .expect("inbound blocked edge");
    let blocker_list_after_inbound = blocks::list_blocked_by(pool, blocker_entity)
        .await
        .expect("blocker list after inbound edge");
    assert!(
        blocker_list_after_inbound.is_empty(),
        "being blocked must not appear on your manage list"
    );
    let target_list = blocks::list_blocked_by(pool, target_entity)
        .await
        .expect("target list after inbound edge");
    assert_eq!(target_list.len(), 1);
    assert_eq!(target_list[0].entity_id, blocker_entity);

    blocks::unblock_human(pool, target_entity, blocker_entity, target_user)
        .await
        .expect("unblock inbound");

    assert!(
        !blocks::is_pair_blocked(pool, blocker_entity, target_entity)
            .await
            .expect("unblocked check")
    );

    let blocker_view_after = private_profile::fetch_by_handle(pool, blocker_user, &target_handle)
        .await
        .expect("blocker fetch after unblock");
    assert!(
        blocker_view_after.is_some(),
        "profiles visible again after unblock"
    );

    db.cleanup().await;
}
