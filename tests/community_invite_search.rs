//! Community invite member search (`exclude_community_member` query path).

mod common;

use app::social::community_invite;
use common::TestDb;

#[tokio::test]
async fn search_invitable_humans_excludes_any_active_edge() {
    let db = TestDb::new().await;
    let pool = &db.pool;

    let community_id: u64 = sqlx::query_scalar(
        r#"
        SELECT se.id
        FROM social_entity se
        INNER JOIN social_community sc ON sc.social_entity_id = se.id
        LIMIT 1
        "#,
    )
    .fetch_one(pool)
    .await
    .expect("seed community");

    let needle = "seed";
    let prefix_like = format!("{needle}%");
    let broad_like = format!("%{needle}%");

    let rows = community_invite::search_invitable_humans(
        pool,
        community_id,
        needle,
        &prefix_like,
        &broad_like,
        None,
    )
    .await
    .expect("search query");

    for (human_id, handle, ..) in &rows {
        let linked = community_invite::human_has_active_edge_to_community(pool, *human_id, community_id)
            .await
            .expect("edge check");
        assert!(
            !linked,
            "handle @{handle} should not have an active edge to community {community_id}"
        );
    }

    db.cleanup().await;
}
