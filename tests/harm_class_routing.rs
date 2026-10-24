//! Harm-class routing — Class A/B never open jury cases; Class C emergency; non-acute civic.

mod common;

use app::social::safety::{
    route_filing_for_clause_points, FilingRoute, SafetyHarmClass,
};

#[test]
fn class_ab_never_emergency_or_civic_route() {
    for point in [401u16, 403, 407] {
        let route = route_filing_for_clause_points(&[point]).expect("route");
        assert!(
            matches!(route, FilingRoute::SanitationOnly(_)),
            "point {point} should be sanitation-only"
        );
    }
}

#[test]
fn class_c_is_emergency_not_sanitation() {
    assert!(matches!(
        route_filing_for_clause_points(&[402]),
        Ok(FilingRoute::EmergencySafety)
    ));
}

#[test]
fn non_acute_safety_is_civic() {
    assert!(matches!(
        route_filing_for_clause_points(&[405]),
        Ok(FilingRoute::CivicDispute)
    ));
}

#[tokio::test]
async fn sanitation_filing_log_persisted() {
    use app::social::safety::{log_filing_sanitation_routed, SafetyHarmClass};

    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let user_id: u64 = sqlx::query_scalar("SELECT id FROM users LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("user");
    let post_id: u64 = sqlx::query_scalar("SELECT id FROM post LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("post");

    log_filing_sanitation_routed(
        pool,
        "test_harm_class_routing",
        SafetyHarmClass::A.as_str(),
        user_id,
        post_id,
        &[401],
    )
    .await;

    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM safety_sanitation_event
        WHERE action_class = 'filing_routed_sanitation'
          AND harm_class = 'A'
          AND actor_user_id = ?
          AND target_post_id = ?
        "#,
    )
    .bind(user_id)
    .bind(post_id)
    .fetch_one(pool)
    .await
    .expect("count");

    assert_eq!(count, 1);
}
