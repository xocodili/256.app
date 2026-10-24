//! Engagement thread integration: seeded users post comments on shared-community posts.

mod common;

use app::social::engagement;
use app::social::thread::{self, THREAD_L1_PAGE_SIZE};
use common::{
    expected_engagement_stats, insert_engagement_vote, insert_thread_comment,
    pick_shared_community_author_idx, seed_light, simulate_engagement_comment_threads, TestDb,
};
use rand::rngs::StdRng;
use rand::SeedableRng;

const SIM_RNG_SEED: u64 = 256_0420;

fn max_comment_depth(comments: &[thread::ThreadComment]) -> u8 {
    let mut max = 0u8;
    for c in comments {
        max = max.max(c.depth);
        if !c.children.is_empty() {
            max = max.max(max_comment_depth(&c.children));
        }
    }
    max
}

#[tokio::test]
async fn seeded_users_comment_on_shared_community_posts() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let seed = seed_light(pool).await;

    let stats = simulate_engagement_comment_threads(pool, &seed, SIM_RNG_SEED).await;
    let expected = expected_engagement_stats(&seed, SIM_RNG_SEED);

    assert_eq!(stats, expected);
    assert_eq!(stats.users_processed, seed.entity_ids.len() as u32);
    assert_eq!(stats.l1, seed.entity_ids.len() as u32);
    assert!(stats.l2 > 0, "expected some L2 replies at 64% probability");
    assert!(
        stats.l3 <= stats.l2,
        "L3 replies only happen under L2 branches"
    );

    let mut rng = StdRng::seed_from_u64(SIM_RNG_SEED);
    let author_idx =
        pick_shared_community_author_idx(&seed, 0, &mut rng).expect("shared community peer");
    let root = seed.post_community_ids[author_idx];
    let viewer = seed.user_ids[0];

    let thread = thread::load_thread(pool, root, viewer, 0)
        .await
        .expect("load thread")
        .expect("root post");
    assert!(thread.can_view_discussion);
    assert!(thread.viewer_vote.is_some());
    assert!(thread.comment_count >= stats.l1 as u64 / seed.entity_ids.len() as u64);
    assert!(!thread.comments.is_empty());

    if stats.l3 > 0 {
        let l3_chains: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM post p3
            INNER JOIN post p2 ON p2.id = p3.parent_post_id
            INNER JOIN post p1 ON p1.id = p2.parent_post_id
            WHERE p3.root_post_id = p1.root_post_id
              AND p1.parent_post_id = p3.root_post_id
            "#,
        )
        .fetch_one(pool)
        .await
        .expect("count L3 chains");
        assert_eq!(
            l3_chains, stats.l3 as i64,
            "L3 rows in post table should match simulation stats"
        );
    }

    let comment = thread
        .comments
        .iter()
        .find(|c| c.handle.starts_with("test_"))
        .expect("seeded comment with test handle");
    assert!(comment.profile_linkable);
    assert_eq!(
        comment.profile_href,
        Some(format!("/home/profile/{}", comment.handle))
    );
    let normalized = app::social::entity::normalize_handle_from_path_segment(&comment.handle)
        .expect("comment handle normalizes");
    let profile = app::social::private_profile::fetch_by_handle(pool, viewer, &normalized)
        .await
        .expect("profile lookup");
    assert!(profile.is_some(), "comment author profile must load for viewer");
}

#[tokio::test]
async fn thread_hidden_until_viewer_votes() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let seed = seed_light(pool).await;

    let root = seed.post_community_ids[0];
    let viewer = seed.user_ids[1];

    let before = thread::load_thread(pool, root, viewer, 0)
        .await
        .expect("load")
        .expect("root");
    assert!(!before.can_view_discussion);
    assert!(before.comments.is_empty());
    assert!(!before.preview_only);
    assert!(engagement::viewer_vote_direction(pool, root, viewer)
        .await
        .expect("vote query")
        .is_none());

    insert_engagement_vote(pool, root, viewer).await;

    let after = thread::load_thread(pool, root, viewer, 0)
        .await
        .expect("load")
        .expect("root");
    assert!(after.can_view_discussion);
    assert!(!after.preview_only);
    assert!(engagement::viewer_vote_direction(pool, root, viewer)
        .await
        .expect("vote query")
        .is_some());
}

#[tokio::test]
async fn thread_preview_top_three_before_vote() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let seed = seed_light(pool).await;

    let root = seed.post_community_ids[0];
    let viewer = seed.user_ids[1];
    let comm_count = seed.community_entity_ids.len();
    let author_bucket = (seed.entity_ids[0] as usize) % comm_count;

    let commenters: Vec<usize> = (1..seed.entity_ids.len())
        .filter(|&i| (seed.entity_ids[i] as usize) % comm_count == author_bucket)
        .take(4)
        .collect();
    assert!(commenters.len() >= 4, "need four peers in the same community bucket");

    for (n, &idx) in commenters.iter().enumerate() {
        insert_engagement_vote(pool, root, seed.user_ids[idx]).await;
        insert_thread_comment(
            pool,
            root,
            root,
            seed.entity_ids[idx],
            seed.user_ids[idx],
            &format!("preview probe {n}"),
        )
        .await;
    }

    let preview = thread::load_thread(pool, root, viewer, 0)
        .await
        .expect("load preview")
        .expect("root");
    assert!(!preview.can_view_discussion);
    assert!(preview.preview_only);
    assert_eq!(preview.comments.len(), THREAD_L1_PAGE_SIZE);
    assert!(preview.comments.iter().all(|c| !c.can_vote && !c.can_reply));
    assert!(preview.comments.iter().all(|c| c.children.is_empty()));

    insert_engagement_vote(pool, root, viewer).await;

    let full = thread::load_thread(pool, root, viewer, 0)
        .await
        .expect("load full")
        .expect("root");
    assert!(full.can_view_discussion);
    assert!(!full.preview_only);
    assert_eq!(full.comments.len(), THREAD_L1_PAGE_SIZE);
}

#[tokio::test]
async fn l1_thread_pagination_returns_three_per_page() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let seed = seed_light(pool).await;

    let root = seed.post_community_ids[0];
    let comm_count = seed.community_entity_ids.len();
    let author_bucket = (seed.entity_ids[0] as usize) % comm_count;

    let commenters: Vec<usize> = (1..seed.entity_ids.len())
        .filter(|&i| (seed.entity_ids[i] as usize) % comm_count == author_bucket)
        .take(5)
        .collect();
    assert_eq!(commenters.len(), 5, "need five peers in the same community bucket");

    for (n, &idx) in commenters.iter().enumerate() {
        insert_engagement_vote(pool, root, seed.user_ids[idx]).await;
        insert_thread_comment(
            pool,
            root,
            root,
            seed.entity_ids[idx],
            seed.user_ids[idx],
            &format!("pagination probe {n}"),
        )
        .await;
    }

    let viewer = seed.user_ids[commenters[0]];
    let page0 = thread::load_thread(pool, root, viewer, 0)
        .await
        .expect("page 0")
        .expect("root");
    assert_eq!(page0.comments.len(), THREAD_L1_PAGE_SIZE);
    assert!(page0.has_more_l1);
    assert_eq!(page0.l1_next_offset, Some(3));

    let page1 = thread::load_thread(pool, root, viewer, 3)
        .await
        .expect("page 1")
        .expect("root");
    assert_eq!(page1.comments.len(), 2);
    assert!(!page1.has_more_l1);
    assert_eq!(page1.l1_next_offset, None);
}

#[tokio::test]
async fn thread_load_includes_depth_three_replies() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let seed = seed_light(pool).await;

    // Post 0 belongs to user 0; engagement rejects self-votes.
    let root = seed.post_community_ids[1];
    let viewer = seed.user_ids[0];
    let entity = seed.entity_ids[0];

    insert_engagement_vote(pool, root, viewer).await;
    let l1 = insert_thread_comment(pool, root, root, entity, viewer, "depth probe L1").await;
    let l2 = insert_thread_comment(pool, root, l1, entity, viewer, "depth probe L2").await;
    let _l3 = insert_thread_comment(pool, root, l2, entity, viewer, "depth probe L3").await;

    let thread = thread::load_thread(pool, root, viewer, 0)
        .await
        .expect("load thread")
        .expect("root post");
    assert!(
        max_comment_depth(&thread.comments) >= 3,
        "load_thread should return L1→L2→L3 when the chain fits the first L1 page"
    );
}
