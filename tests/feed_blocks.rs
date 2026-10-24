//! Feed query smoke test with block SQL fragment.

mod common;

use app::social::feed;
use common::{seed_light, TestDb};

#[tokio::test]
async fn aggregate_feed_loads_with_block_filter_sql() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let seed = seed_light(pool).await;
    let user_id = seed.user_ids[0];

    let page = feed::fetch_page_rows(pool, user_id, 0, 10, None, false)
        .await
        .expect("aggregate feed query");

    assert!(page.rows.len() <= 10);

    let personal = feed::fetch_page_rows(pool, user_id, 0, 10, None, true)
        .await
        .expect("personal feed filter query");

    assert!(personal.rows.len() <= 10);

    db.cleanup().await;
}
