//! Domain paywall profile — defaults, learning, and rating isolation.

mod common;

use app::content::domain_paywall::{self, PAYWALL_FREE, PAYWALL_PARTIAL, PAYWALL_WHOLE};
use common::{RefKeys, TestDb};
use sqlx::MySqlPool;

async fn insert_domain(
    pool: &MySqlPool,
    rk: &RefKeys,
    domain: &str,
    rating: &str,
    paywall: Option<&str>,
) -> u64 {
    let pending = rk.id("status", "pending_verification");
    if let Some(tier) = paywall {
        sqlx::query(
            r#"
            INSERT INTO content_domain (
                domain, status_id, rating, safety_state, paywall, first_seen_at, last_seen_at
            )
            VALUES (?, ?, ?, 'safe', ?, UTC_TIMESTAMP(6), UTC_TIMESTAMP(6))
            "#,
        )
        .bind(domain)
        .bind(pending)
        .bind(rating)
        .bind(tier)
        .execute(pool)
        .await
        .expect("insert domain");
    } else {
        sqlx::query(
            r#"
            INSERT INTO content_domain (
                domain, status_id, rating, safety_state, first_seen_at, last_seen_at
            )
            VALUES (?, ?, ?, 'safe', UTC_TIMESTAMP(6), UTC_TIMESTAMP(6))
            "#,
        )
        .bind(domain)
        .bind(pending)
        .bind(rating)
        .execute(pool)
        .await
        .expect("insert domain");
    }
    sqlx::query_scalar("SELECT id FROM content_domain WHERE domain = ? LIMIT 1")
        .bind(domain)
        .fetch_one(pool)
        .await
        .expect("domain id")
}

async fn insert_link(pool: &MySqlPool, rk: &RefKeys, domain_id: u64, url: &str) -> u64 {
    let pending = rk.id("status", "pending_verification");
    sqlx::query(
        r#"
        INSERT INTO content_link (
            domain_id, url_original, url_canonical, url_canonical_sha,
            status_id, rating, fetch_status, fetch_attempts, next_recrawl_at
        )
        VALUES (?, ?, ?, UNHEX(SHA2(?, 256)), ?, 2.50, 'ok', 1, UTC_TIMESTAMP(6))
        "#,
    )
    .bind(domain_id)
    .bind(url)
    .bind(url)
    .bind(url)
    .bind(pending)
    .execute(pool)
    .await
    .expect("insert link");
    sqlx::query_scalar("SELECT id FROM content_link WHERE url_canonical = ? LIMIT 1")
        .bind(url)
        .fetch_one(pool)
        .await
        .expect("link id")
}

async fn domain_paywall_tier(pool: &MySqlPool, domain_id: u64) -> String {
    sqlx::query_scalar("SELECT paywall FROM content_domain WHERE id = ? LIMIT 1")
        .bind(domain_id)
        .fetch_one(pool)
        .await
        .expect("paywall tier")
}

async fn domain_rating(pool: &MySqlPool, domain_id: u64) -> String {
    sqlx::query_scalar("SELECT CAST(rating AS CHAR) FROM content_domain WHERE id = ? LIMIT 1")
        .bind(domain_id)
        .fetch_one(pool)
        .await
        .expect("rating")
}

#[tokio::test]
async fn new_domain_defaults_to_free_paywall() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let rk = RefKeys::load(pool).await;

    let domain_id = insert_domain(pool, &rk, "default-free.test", "3.25", None).await;
    assert_eq!(domain_paywall_tier(pool, domain_id).await, PAYWALL_FREE);
}

#[tokio::test]
async fn record_paywall_hit_promotes_free_to_partial() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let rk = RefKeys::load(pool).await;

    let domain_id = insert_domain(pool, &rk, "learn-partial.test", "2.50", Some(PAYWALL_FREE)).await;
    let link_id = insert_link(
        pool,
        &rk,
        domain_id,
        "https://learn-partial.test/story",
    )
    .await;

    let observed = domain_paywall::record_paywall_hit(pool, link_id)
        .await
        .expect("record");
    assert_eq!(observed.as_deref(), Some("learn-partial.test"));
    assert_eq!(domain_paywall_tier(pool, domain_id).await, PAYWALL_PARTIAL);
}

#[tokio::test]
async fn record_paywall_hit_noop_when_already_partial_or_whole() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let rk = RefKeys::load(pool).await;

    for (domain, tier) in [
        ("already-partial.test", PAYWALL_PARTIAL),
        ("already-whole.test", PAYWALL_WHOLE),
    ] {
        let domain_id = insert_domain(pool, &rk, domain, "2.50", Some(tier)).await;
        let link_id = insert_link(
            pool,
            &rk,
            domain_id,
            &format!("https://{domain}/story"),
        )
        .await;

        let observed = domain_paywall::record_paywall_hit(pool, link_id)
            .await
            .expect("record");
        assert!(observed.is_none(), "expected no change for {tier}");
        assert_eq!(domain_paywall_tier(pool, domain_id).await, tier);
    }
}

#[tokio::test]
async fn record_paywall_hit_does_not_change_domain_rating() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let rk = RefKeys::load(pool).await;

    let domain_id = insert_domain(pool, &rk, "rating-stable.test", "4.00", Some(PAYWALL_FREE)).await;
    let link_id = insert_link(
        pool,
        &rk,
        domain_id,
        "https://rating-stable.test/article",
    )
    .await;

    domain_paywall::record_paywall_hit(pool, link_id)
        .await
        .expect("record");
    assert_eq!(domain_rating(pool, domain_id).await, "4.00");
    assert_eq!(domain_paywall_tier(pool, domain_id).await, PAYWALL_PARTIAL);
}

#[tokio::test]
async fn whole_domains_excluded_from_council_portfolio_query_shape() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let rk = RefKeys::load(pool).await;

    insert_domain(pool, &rk, "whole-host.test", "2.50", Some(PAYWALL_WHOLE)).await;
    insert_domain(pool, &rk, "free-host.test", "2.50", Some(PAYWALL_FREE)).await;
    insert_domain(pool, &rk, "partial-host.test", "2.50", Some(PAYWALL_PARTIAL)).await;

    let rows: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT cd.domain, cd.paywall
        FROM content_domain cd
        WHERE cd.domain IN ('whole-host.test', 'free-host.test', 'partial-host.test')
          AND cd.paywall <> 'whole'
        ORDER BY CASE cd.paywall
            WHEN 'free' THEN 0
            WHEN 'partial' THEN 1
            ELSE 2
        END, cd.domain
        "#,
    )
    .fetch_all(pool)
    .await
    .expect("select");

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].0, "free-host.test");
    assert_eq!(rows[0].1, PAYWALL_FREE);
    assert_eq!(rows[1].0, "partial-host.test");
    assert_eq!(rows[1].1, PAYWALL_PARTIAL);
}

#[tokio::test]
async fn paywalled_links_excluded_from_novel_link_pool() {
    let db = TestDb::new().await;
    let pool = &db.pool;
    let rk = RefKeys::load(pool).await;

    let domain_id = insert_domain(pool, &rk, "link-filter.test", "2.50", Some(PAYWALL_FREE)).await;
    let open_id = insert_link(
        pool,
        &rk,
        domain_id,
        "https://link-filter.test/open-story",
    )
    .await;
    let walled_id = insert_link(
        pool,
        &rk,
        domain_id,
        "https://link-filter.test/walled-story",
    )
    .await;
    sqlx::query("UPDATE content_link SET extract_status = 'paywalled' WHERE id = ?")
        .bind(walled_id)
        .execute(pool)
        .await
        .expect("mark paywalled");

    let ids: Vec<u64> = sqlx::query_scalar(
        r#"
        SELECT cl.id
        FROM content_link cl
        INNER JOIN content_domain cd ON cd.id = cl.domain_id
        WHERE cl.domain_id = ?
          AND cl.fetch_status = 'ok'
          AND cd.paywall <> 'whole'
          AND (cl.extract_status IS NULL OR cl.extract_status <> 'paywalled')
        "#,
    )
    .bind(domain_id)
    .fetch_all(pool)
    .await
    .expect("select links");

    assert_eq!(ids, vec![open_id]);
}

#[test]
fn paywall_sort_key_orders_free_before_partial_before_whole() {
    use app::content::domain_paywall::paywall_sort_key;

    assert!(paywall_sort_key(PAYWALL_FREE) < paywall_sort_key(PAYWALL_PARTIAL));
    assert!(paywall_sort_key(PAYWALL_PARTIAL) < paywall_sort_key(PAYWALL_WHOLE));
}
