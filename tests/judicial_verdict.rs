//! Focused judicial simulations: final-vote resolution, settlement columns, and
//! nomination-tick retry when votes are in but the case is still open.
//!
//! Uses `seed_light` (128 users) for speed. Full multi-case load remains in
//! `judicial_lifecycle.rs`.

mod common;
mod judicial_harness;

use app::ref_keys;
use app::social::judicial::{self, CastVoteOutcome, VerdictOutcome};
use judicial_harness::{
    accept_all_invites_with_votes_sql, assert_case_fully_resolved,
    assert_community_feed_feedback_applied, fetch_case_settlement, file_community_post_dispute,
    load_case_ref_ids, pending_count_for_case, pending_invites_for_active_cases,
    run_nomination_until_quiescent, verdict_from_weights, CaseRefIds,
};
use sqlx::MySqlPool;

async fn root_post_status_key_for_case(pool: &MySqlPool, case_id: u64) -> String {
    let subject: Option<u64> = sqlx::query_scalar(
        r#"SELECT subject_post_id FROM judicial_case WHERE id = ?"#,
    )
    .bind(case_id)
    .fetch_optional(pool)
    .await
    .expect("subject_post_id query")
    .flatten();
    let subject = subject.expect("case subject_post_id");
    let root_id: u64 = sqlx::query_scalar(
        r#"SELECT COALESCE(p.root_post_id, p.id) FROM post p WHERE p.id = ? LIMIT 1"#,
    )
    .bind(subject)
    .fetch_one(pool)
    .await
    .expect("root id");
    sqlx::query_scalar(
        r#"
        SELECT rk.`key`
        FROM post p
        INNER JOIN ref_key_values rk
            ON rk.id = p.status_id
           AND rk.`group` = ?
           AND rk.is_active = 1
        WHERE p.id = ?
        "#,
    )
    .bind(ref_keys::GROUP_STATUS)
    .bind(root_id)
    .fetch_one(pool)
    .await
    .expect("post status key")
}

async fn seat_jury_for_case(pool: &MySqlPool, case_id: u64) {
    for _ in 0..12 {
        judicial::nominate_jurors_for_open_cases(pool, 64)
            .await
            .expect("nominate");
        let pending = pending_count_for_case(pool, case_id).await;
        if pending > 0 {
            return;
        }
        let status: String = sqlx::query_scalar(
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
        .expect("case status");
        if status == ref_keys::KEY_JUDICIAL_CASE_JURY_ACTIVE {
            return;
        }
    }
    panic!("case {case_id} never received juror invites");
}

async fn file_fixture_community_case(pool: &MySqlPool) -> (u64, CaseRefIds) {
    let seed = common::seed_light(pool).await;
    let refs = load_case_ref_ids(pool).await;

    let author_idx = 10;
    let filer_idx = 11;
    let author_entity = seed.entity_ids[author_idx];
    let filer_entity = seed.entity_ids[filer_idx];
    let filer_user = seed.user_ids[filer_idx];
    let post_id = seed.post_community_ids[author_idx];

    let case_id = file_community_post_dispute(
        pool,
        &refs,
        filer_entity,
        author_entity,
        post_id,
        filer_user,
    )
    .await;

    (case_id, refs)
}

/// Last `cast_vote` must run `check_and_resolve_verdict` internally (no manual resolve call).
#[tokio::test]
async fn final_cast_vote_resolves_case_and_settlement() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let (case_id, _refs) = file_fixture_community_case(pool).await;

    seat_jury_for_case(pool, case_id).await;

    let mut invites = pending_invites_for_active_cases(pool).await;
    invites.retain(|(_, cid, _, _)| *cid == case_id);
    assert!(
        !invites.is_empty(),
        "expected pending juror invites for case {case_id}"
    );

    let last_idx = invites.len() - 1;
    for (i, (invite_id, _, juror_user_id, _)) in invites.iter().enumerate() {
        let user_id = juror_user_id.expect("juror user");
        let outcome = judicial::cast_vote(pool, user_id, *invite_id, ref_keys::KEY_JCJI_VOTE_FOR)
            .await
            .expect("cast_vote");
        assert_eq!(outcome, CastVoteOutcome::Recorded);

        if i < last_idx {
            let snap = fetch_case_settlement(pool, case_id).await;
            assert_ne!(
                snap.status_key,
                ref_keys::KEY_JUDICIAL_CASE_RESOLVED,
                "case should not resolve before final vote"
            );
        }
    }

    let snap = fetch_case_settlement(pool, case_id).await;
    assert_case_fully_resolved(&snap, "final_cast_vote");
    assert_community_feed_feedback_applied(&snap, "final_cast_vote");
    assert_eq!(verdict_from_weights(&snap), VerdictOutcome::Upheld);
    assert_eq!(
        root_post_status_key_for_case(pool, case_id).await,
        ref_keys::KEY_FLAGGED,
        "upheld post-moderation verdict should mark the thread flagged"
    );

    db.cleanup().await;
}

/// Votes recorded in SQL but case left open; `nominate_jurors_for_open_cases` retry closes it.
#[tokio::test]
async fn nomination_tick_retries_verdict_when_no_pending_invites() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let (case_id, _refs) = file_fixture_community_case(pool).await;

    seat_jury_for_case(pool, case_id).await;
    accept_all_invites_with_votes_sql(pool, case_id, ref_keys::KEY_JCJI_VOTE_FOR).await;

    assert_eq!(pending_count_for_case(pool, case_id).await, 0);

    let pre = fetch_case_settlement(pool, case_id).await;
    assert_ne!(pre.status_key, ref_keys::KEY_JUDICIAL_CASE_RESOLVED);

    run_nomination_until_quiescent(pool, 3).await;

    let snap = fetch_case_settlement(pool, case_id).await;
    assert_case_fully_resolved(&snap, "nomination_retry");
    assert_community_feed_feedback_applied(&snap, "nomination_retry");
    assert_eq!(
        root_post_status_key_for_case(pool, case_id).await,
        ref_keys::KEY_FLAGGED
    );

    db.cleanup().await;
}

/// Explicit `check_and_resolve_verdict` after votes (exercises TIMESTAMP decode on `opened_at`/`closed_at`).
#[tokio::test]
async fn check_and_resolve_verdict_applies_community_rating_feedback() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let (case_id, _refs) = file_fixture_community_case(pool).await;

    seat_jury_for_case(pool, case_id).await;
    accept_all_invites_with_votes_sql(pool, case_id, ref_keys::KEY_JCJI_VOTE_AGAINST).await;

    let resolved = judicial::check_and_resolve_verdict(pool, case_id)
        .await
        .expect("check_and_resolve_verdict must not error on TIMESTAMP columns");
    assert_eq!(resolved, Some(VerdictOutcome::Overturned));

    let snap = fetch_case_settlement(pool, case_id).await;
    assert_case_fully_resolved(&snap, "explicit_resolve");
    assert_community_feed_feedback_applied(&snap, "explicit_resolve");
    assert_eq!(verdict_from_weights(&snap), VerdictOutcome::Overturned);
    assert_eq!(
        root_post_status_key_for_case(pool, case_id).await,
        ref_keys::KEY_VERIFIED,
        "cleared post should return to verified so it appears in the aggregate feed"
    );

    db.cleanup().await;
}
