//! Council AI portfolio seed: 32 outbound hosts per councillor + seed catalogue links.

mod common;

const COUNCIL_SLUGS: &[&str] = &[
    "milo", "soka", "hypa", "marc", "dogo", "aspa", "pele", "theo", "augi", "ciro", "atol", "epic",
];

#[tokio::test]
async fn each_councillor_has_32_portfolio_domains() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;

    for slug in COUNCIL_SLUGS {
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM ai_actor_content_domain pac
            INNER JOIN ai_actor aa ON aa.id = pac.ai_actor_id AND aa.is_active = 1
            WHERE aa.slug = ?
              AND pac.is_active = 1
              AND pac.removed_at IS NULL
            "#,
        )
        .bind(slug)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("{slug}: {e}"));
        assert_eq!(
            count, 32,
            "expected 32 portfolio domains for @{slug}, got {count}"
        );
    }
}

#[tokio::test]
async fn portfolio_domains_have_seed_catalogue_links() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;

    for slug in COUNCIL_SLUGS {
        let link_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(DISTINCT cl.id)
            FROM ai_actor aa
            INNER JOIN ai_actor_content_domain pac
                ON pac.ai_actor_id = aa.id
               AND pac.is_active = 1
               AND pac.removed_at IS NULL
            INNER JOIN content_domain cd ON cd.id = pac.content_domain_id
            INNER JOIN content_link cl ON cl.domain_id = cd.id
            WHERE aa.slug = ?
              AND cl.rating >= 2.50
              AND cl.fetch_status = 'ok'
            "#,
        )
        .bind(slug)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("{slug}: {e}"));
        assert!(
            link_count >= 32,
            "@{slug} expected at least 32 catalogue links, got {link_count}"
        );
    }
}

#[tokio::test]
async fn shared_domains_appear_in_multiple_portfolios() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;

    let actors: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(DISTINCT pac.ai_actor_id)
        FROM ai_actor_content_domain pac
        INNER JOIN content_domain cd ON cd.id = pac.content_domain_id
        WHERE cd.domain = 'theguardian.com'
          AND pac.is_active = 1
          AND pac.removed_at IS NULL
        "#,
    )
    .fetch_one(pool)
    .await
    .expect("theguardian overlap count");
    assert!(
        actors >= 2,
        "expected theguardian.com in multiple councillor portfolios, got {actors}"
    );
}
