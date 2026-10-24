//! Integration tests: emergency safety vs civic dispute moderation.

mod common;
mod judicial_harness;

use app::ref_keys;
use app::social::judicial::{self, ContestOutcome, EmergencyFilingError};
use common::seed_light;
use judicial_harness::{
    backdate_emergency_case_for_standing, drive_human_jury_case_to_resolved,
    file_emergency_safety_case, load_emergency_ref_ids, post_status_key,
    root_emergency_clause_402_id, run_nomination_until_quiescent,
};

#[tokio::test]
async fn emergency_contest_opens_jury() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let seed = seed_light(pool).await;
    let refs = load_emergency_ref_ids(pool).await;
    let clause_id = root_emergency_clause_402_id(pool).await;

    let author_idx = 20;
    let filer_idx = 21;
    let root_post_id = seed.post_community_ids[author_idx];
    let case_id = file_emergency_safety_case(
        pool,
        &refs,
        seed.entity_ids[filer_idx],
        seed.entity_ids[author_idx],
        root_post_id,
        seed.user_ids[filer_idx],
        clause_id,
    )
    .await;

    judicial_harness::grant_noms(pool, seed.entity_ids[author_idx], 8).await;

    let outcome = judicial::contest_emergency_case(
        pool,
        seed.user_ids[author_idx],
        seed.entity_ids[author_idx],
        root_post_id,
    )
    .await
    .expect("contest");

    assert!(matches!(outcome, ContestOutcome::Opened { case_id: opened } if opened == case_id));

    let status_key: String = sqlx::query_scalar(
        r#"
        SELECT rk.`key`
        FROM judicial_case jc
        INNER JOIN ref_key_values rk ON rk.id = jc.status_id
        WHERE jc.id = ?
        "#,
    )
    .bind(case_id)
    .fetch_one(pool)
    .await
    .expect("status");

    assert_eq!(status_key, ref_keys::KEY_JUDICIAL_CASE_OPEN);
    assert!(
        sqlx::query_scalar::<_, Option<chrono::DateTime<chrono::Utc>>>(
            "SELECT contested_at FROM judicial_case WHERE id = ?"
        )
        .bind(case_id)
        .fetch_one(pool)
        .await
        .expect("contested_at")
        .is_some()
    );

    judicial_harness::run_nomination_until_quiescent(pool, 32).await;
    assert!(
        judicial_harness::pending_count_for_case(pool, case_id).await > 0,
        "contested emergency should seat jurors"
    );
}

#[tokio::test]
async fn emergency_standing_settlement_after_three_days() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let seed = seed_light(pool).await;
    let refs = load_emergency_ref_ids(pool).await;
    let clause_id = root_emergency_clause_402_id(pool).await;

    let author_idx = 30;
    let filer_idx = 31;
    let author_entity = seed.entity_ids[author_idx];
    let root_post_id = seed.post_community_ids[author_idx];
    let case_id = file_emergency_safety_case(
        pool,
        &refs,
        seed.entity_ids[filer_idx],
        author_entity,
        root_post_id,
        seed.user_ids[filer_idx],
        clause_id,
    )
    .await;

    let rating_before: String = sqlx::query_scalar(
        "SELECT CAST(rating AS CHAR) FROM social_entity WHERE id = ?",
    )
    .bind(author_entity)
    .fetch_one(pool)
    .await
    .expect("rating before");

    backdate_emergency_case_for_standing(pool, case_id, 4).await;

    let notified = judicial::run_standing_settlement_batch(pool, 8)
        .await
        .expect("standing batch");
    assert_eq!(notified.len(), 1);
    assert_eq!(notified[0].0, seed.user_ids[author_idx]);
    assert_eq!(notified[0].1, root_post_id);

    let standing_applied: bool = sqlx::query_scalar(
        "SELECT standing_applied_at IS NOT NULL FROM judicial_case WHERE id = ?",
    )
    .bind(case_id)
    .fetch_one(pool)
    .await
    .expect("standing flag");

    assert!(standing_applied);
    assert_eq!(
        post_status_key(pool, root_post_id).await,
        ref_keys::KEY_POST_SUPPRESSED
    );

    let rating_after: String = sqlx::query_scalar(
        "SELECT CAST(rating AS CHAR) FROM social_entity WHERE id = ?",
    )
    .bind(author_entity)
    .fetch_one(pool)
    .await
    .expect("rating after");

    assert!(
        rating_after < rating_before,
        "standing batch should debit author global rating"
    );
}

#[tokio::test]
async fn emergency_daily_filing_cap_enforced() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let seed = seed_light(pool).await;
    let refs = load_emergency_ref_ids(pool).await;
    let clause_id = root_emergency_clause_402_id(pool).await;
    let filer_idx = 40;

    for i in 0..3 {
        let author_idx = 41 + i;
        file_emergency_safety_case(
            pool,
            &refs,
            seed.entity_ids[filer_idx],
            seed.entity_ids[author_idx],
            seed.post_community_ids[author_idx],
            seed.user_ids[filer_idx],
            clause_id,
        )
        .await;
    }

    let mut tx = pool.begin().await.expect("tx");
    let err = judicial::check_daily_emergency_filing_cap_tx(&mut tx, seed.user_ids[filer_idx])
        .await
        .expect_err("cap should trip");
    assert_eq!(err, EmergencyFilingError::DailyCap);
}

async fn contest_emergency_as_author(
    pool: &sqlx::MySqlPool,
    seed: &common::SeedIds,
    author_idx: usize,
    root_post_id: u64,
) -> u64 {
    judicial_harness::grant_noms(pool, seed.entity_ids[author_idx], 8).await;
    let outcome = judicial::contest_emergency_case(
        pool,
        seed.user_ids[author_idx],
        seed.entity_ids[author_idx],
        root_post_id,
    )
    .await
    .expect("contest");
    let judicial::ContestOutcome::Opened { case_id } = outcome else {
        panic!("expected contest to open jury");
    };
    run_nomination_until_quiescent(pool, 32).await;
    case_id
}

async fn user_is_blocked(pool: &sqlx::MySqlPool, user_id: u64) -> bool {
    sqlx::query_scalar::<_, String>(
        r#"
        SELECT rk.`key`
        FROM users u
        INNER JOIN ref_key_values rk
            ON rk.id = COALESCE(u.user_status_id, u.status_id)
           AND rk.`group` IN (?, ?)
           AND rk.is_active = 1
        WHERE u.id = ?
        "#,
    )
    .bind(ref_keys::GROUP_USER_STATUS)
    .bind(ref_keys::GROUP_STATUS)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .expect("user status")
        == ref_keys::KEY_BLOCKED
}

#[tokio::test]
async fn emergency_contest_verdict_overturn_restores_post() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let seed = seed_light(pool).await;
    let refs = load_emergency_ref_ids(pool).await;
    let clause_id = root_emergency_clause_402_id(pool).await;

    let author_idx = 50;
    let filer_idx = 51;
    let root_post_id = seed.post_community_ids[author_idx];
    let case_id = file_emergency_safety_case(
        pool,
        &refs,
        seed.entity_ids[filer_idx],
        seed.entity_ids[author_idx],
        root_post_id,
        seed.user_ids[filer_idx],
        clause_id,
    )
    .await;

    assert_eq!(
        post_status_key(pool, root_post_id).await,
        ref_keys::KEY_POST_SUPPRESSED
    );

    contest_emergency_as_author(pool, &seed, author_idx, root_post_id).await;

    drive_human_jury_case_to_resolved(pool, case_id, false).await;

    assert_eq!(
        post_status_key(pool, root_post_id).await,
        ref_keys::KEY_VERIFIED
    );
}

#[tokio::test]
async fn emergency_contest_verdict_upheld_bans_author() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let seed = seed_light(pool).await;
    let refs = load_emergency_ref_ids(pool).await;
    let clause_id = root_emergency_clause_402_id(pool).await;

    let author_idx = 60;
    let filer_idx = 61;
    let root_post_id = seed.post_community_ids[author_idx];
    let case_id = file_emergency_safety_case(
        pool,
        &refs,
        seed.entity_ids[filer_idx],
        seed.entity_ids[author_idx],
        root_post_id,
        seed.user_ids[filer_idx],
        clause_id,
    )
    .await;

    contest_emergency_as_author(pool, &seed, author_idx, root_post_id).await;

    drive_human_jury_case_to_resolved(pool, case_id, true).await;

    assert!(
        user_is_blocked(pool, seed.user_ids[author_idx]).await,
        "author should be platform-blocked after losing emergency contest"
    );
    assert_eq!(
        post_status_key(pool, root_post_id).await,
        ref_keys::KEY_POST_SUPPRESSED
    );
}
