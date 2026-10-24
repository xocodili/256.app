//! Juror abstention quota — severe/acute cases bypass the annual cap.

mod common;
mod judicial_harness;

use app::social::judicial::{self, AbstainOutcome, MAX_FREE_ABSTENTIONS_PER_YEAR};
use common::seed_light;
use judicial_harness::{
    file_community_post_dispute, file_emergency_safety_case, load_case_ref_ids,
    load_emergency_ref_ids, pending_invites_for_active_cases, root_emergency_clause_402_id,
    run_nomination_until_quiescent, seed_recent_abstentions_for_user,
};

fn pending_invite_on_case(
    pending: &[(u64, u64, Option<u64>, u64)],
    case_id: u64,
) -> (u64, u64) {
    let (invite_id, _, juror_user_id, _) = pending
        .iter()
        .find(|(_, cid, uid, _)| *cid == case_id && uid.is_some())
        .unwrap_or_else(|| {
            panic!(
                "expected at least one pending invite on case {case_id}, got {pending:?}"
            )
        });
    (*invite_id, juror_user_id.expect("juror user id"))
}

#[tokio::test]
async fn civic_abstention_hits_quota_when_exhausted() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let seed = seed_light(pool).await;
    let refs = load_case_ref_ids(pool).await;

    let filer_idx = 6;
    let author_idx = 7;

    let case_id = file_community_post_dispute(
        pool,
        &refs,
        seed.entity_ids[filer_idx],
        seed.entity_ids[author_idx],
        seed.post_community_ids[author_idx],
        seed.user_ids[filer_idx],
    )
    .await;

    run_nomination_until_quiescent(pool, 32).await;
    let pending = pending_invites_for_active_cases(pool).await;
    let (invite_id, juror_user) = pending_invite_on_case(&pending, case_id);

    let juror_entity: u64 = sqlx::query_scalar(
        "SELECT social_entity_id FROM user_social_entity_role WHERE user_id = ? AND ends_at IS NULL LIMIT 1",
    )
    .bind(juror_user)
    .fetch_one(pool)
    .await
    .expect("juror entity");

    seed_recent_abstentions_for_user(
        pool,
        juror_user,
        juror_entity,
        MAX_FREE_ABSTENTIONS_PER_YEAR,
    )
    .await;

    let outcome = judicial::abstain_from_invite(pool, juror_user, invite_id)
        .await
        .expect("abstain civic");

    assert!(
        matches!(
            outcome,
            AbstainOutcome::OverFreeQuota {
                prior_in_window,
                cap,
            } if prior_in_window == MAX_FREE_ABSTENTIONS_PER_YEAR && cap == MAX_FREE_ABSTENTIONS_PER_YEAR
        ),
        "regular civic case should report quota exhaustion: {outcome:?}"
    );

    db.cleanup().await;
}

#[tokio::test]
async fn emergency_abstention_stays_free_when_quota_exhausted() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let seed = seed_light(pool).await;
    let refs = load_emergency_ref_ids(pool).await;
    let clause_id = root_emergency_clause_402_id(pool).await;

    let filer_idx = 9;
    let author_idx = 10;

    judicial_harness::grant_noms(pool, seed.entity_ids[author_idx], 8).await;

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

    judicial::contest_emergency_case(
        pool,
        seed.user_ids[author_idx],
        seed.entity_ids[author_idx],
        root_post_id,
    )
    .await
    .expect("contest emergency");

    run_nomination_until_quiescent(pool, 32).await;
    let pending = pending_invites_for_active_cases(pool).await;
    let (invite_id, juror_user) = pending_invite_on_case(&pending, case_id);

    let juror_entity: u64 = sqlx::query_scalar(
        "SELECT social_entity_id FROM user_social_entity_role WHERE user_id = ? AND ends_at IS NULL LIMIT 1",
    )
    .bind(juror_user)
    .fetch_one(pool)
    .await
    .expect("juror entity");

    seed_recent_abstentions_for_user(
        pool,
        juror_user,
        juror_entity,
        MAX_FREE_ABSTENTIONS_PER_YEAR,
    )
    .await;

    let outcome = judicial::abstain_from_invite(pool, juror_user, invite_id)
        .await
        .expect("abstain emergency");

    assert!(
        matches!(
            outcome,
            AbstainOutcome::Free {
                prior_in_window,
            } if prior_in_window == MAX_FREE_ABSTENTIONS_PER_YEAR
        ),
        "emergency case abstention must bypass quota: {outcome:?}"
    );

    let still_pending = judicial::find_pending_human_jury_invite_for_user(pool, juror_user)
        .await
        .expect("fetch pending");
    assert!(
        still_pending.is_none(),
        "abstained invite should no longer be pending"
    );

    db.cleanup().await;
}

#[tokio::test]
async fn pending_invite_marks_emergency_as_severe() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let seed = seed_light(pool).await;
    let refs = load_emergency_ref_ids(pool).await;
    let clause_id = root_emergency_clause_402_id(pool).await;

    let filer_idx = 12;
    let author_idx = 13;

    judicial_harness::grant_noms(pool, seed.entity_ids[author_idx], 8).await;

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

    judicial::contest_emergency_case(
        pool,
        seed.user_ids[author_idx],
        seed.entity_ids[author_idx],
        root_post_id,
    )
    .await
    .expect("contest");

    run_nomination_until_quiescent(pool, 32).await;
    let pending = pending_invites_for_active_cases(pool).await;
    let (_, juror_user) = pending_invite_on_case(&pending, case_id);

    let summons = judicial::find_pending_human_jury_invite_for_user(pool, juror_user)
        .await
        .expect("fetch pending")
        .expect("nominated juror should have emergency summons");

    assert!(
        summons.is_severe_case,
        "contested emergency invite must be marked severe"
    );

    db.cleanup().await;
}
