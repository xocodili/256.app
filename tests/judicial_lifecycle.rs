//! Integration test: full judicial lifecycle simulation.
//!
//! Seeds 4096 users across 12 communities, files 16 moderation cases,
//! and drives the jury nomination → abstention/vote → refill → verdict
//! loop using the real business logic from `app::social::judicial`.
//!
//! Verdict settlement (`jury_civic_rating_applied_at`, community feed feedback,
//! `closed_at`) is asserted on every resolved case. Focused scenarios live in
//! `judicial_verdict.rs` (`seed_light`, faster).
//!
//! Requires `TEST_DATABASE_URL` pointing at a MySQL server with
//! CREATE/DROP DATABASE privileges.

mod common;
mod judicial_harness;

use app::ref_keys;
use app::social::judicial;
use judicial_harness::{
    assert_case_fully_resolved, assert_community_feed_feedback_applied, fetch_case_settlement,
    file_community_post_dispute, load_case_ref_ids, respond_to_pending_invites_deterministic,
    verdict_from_weights, CaseRefIds,
};
use sqlx::MySqlPool;
use std::time::Instant;

struct CaseSetup {
    case_id: u64,
    originator_entity_id: u64,
    target_entity_id: u64,
    trigger_label: String,
    /// Community-audience post disputes get feed-scoped community rating feedback.
    expects_community_feedback: bool,
}

async fn file_private_post_dispute(
    pool: &MySqlPool,
    refs: &CaseRefIds,
    originator_entity_id: u64,
    target_entity_id: u64,
    post_id: u64,
    originator_user_id: u64,
) -> u64 {
    let pool_size: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(DISTINCT
            CASE WHEN ser.from_social_entity_id = ? THEN ser.to_social_entity_id
                 ELSE ser.from_social_entity_id END
        )
        FROM post_audience_scope pas
        INNER JOIN post p ON p.id = pas.post_id
        INNER JOIN social_entity_relationship ser
            ON ser.ended_at IS NULL
           AND (ser.from_social_entity_id = p.entity_id OR ser.to_social_entity_id = p.entity_id)
        INNER JOIN ref_key_values rk_type ON rk_type.id = ser.type_id
            AND rk_type.`group` = ?
            AND rk_type.`key` IN (?, ?)
        INNER JOIN ref_key_values rk_status ON rk_status.id = ser.status_id
            AND rk_status.`group` = ?
            AND rk_status.`key` = ?
        WHERE pas.post_id = ?
          AND pas.scope_key IN ('friends', 'family')
        "#,
    )
    .bind(target_entity_id)
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_FRIEND)
    .bind(ref_keys::KEY_RELATIONSHIP_FAMILY)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_VERIFIED)
    .bind(post_id)
    .fetch_one(pool)
    .await
    .expect("private pool size");

    let jury_size = judicial::jury_size_for_case(
        ref_keys::KEY_JUDICIAL_TRIGGER_CIVIC_DISPUTE,
        pool_size.max(0) as usize,
    );

    let result = sqlx::query(
        r#"
        INSERT INTO judicial_case (
            originator_social_entity_id, target_social_entity_id,
            trigger_type_id, status_id, subject_post_id, jury_size,
            created_by
        ) VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(originator_entity_id)
    .bind(target_entity_id)
    .bind(refs.trigger_id)
    .bind(refs.status_open_id)
    .bind(post_id)
    .bind(jury_size as u8)
    .bind(originator_user_id)
    .execute(pool)
    .await
    .expect("insert private case");

    let case_id = result.last_insert_id();
    judicial_harness::mark_post_thread_under_judicial_review(pool, post_id, originator_user_id).await;
    case_id
}

#[tokio::test]
async fn judicial_lifecycle_simulation() {
    let t_total = Instant::now();

    let db = common::TestDb::new().await;
    let pool = &db.pool;

    let t_seed = Instant::now();
    let seed = common::seed_all(pool).await;
    eprintln!("Seeding completed in {}ms", t_seed.elapsed().as_millis());

    let case_refs = load_case_ref_ids(pool).await;
    let mut cases: Vec<CaseSetup> = Vec::with_capacity(16);

    for (ci, &_comm_id) in seed.community_entity_ids.iter().take(12).enumerate() {
        let author_idx = ci * 50 + 10;
        let filer_idx = ci * 50 + 11;
        let case_id = file_community_post_dispute(
            pool,
            &case_refs,
            seed.entity_ids[filer_idx],
            seed.entity_ids[author_idx],
            seed.post_community_ids[author_idx],
            seed.user_ids[filer_idx],
        )
        .await;

        cases.push(CaseSetup {
            case_id,
            originator_entity_id: seed.entity_ids[filer_idx],
            target_entity_id: seed.entity_ids[author_idx],
            trigger_label: format!("community_{ci}"),
            expects_community_feedback: true,
        });
    }

    for fi in 0..4u32 {
        let author_idx = 1000 + (fi as usize) * 100;
        let filer_idx = author_idx + 1;
        let case_id = file_private_post_dispute(
            pool,
            &case_refs,
            seed.entity_ids[filer_idx],
            seed.entity_ids[author_idx],
            seed.post_private_ids[author_idx],
            seed.user_ids[filer_idx],
        )
        .await;

        cases.push(CaseSetup {
            case_id,
            originator_entity_id: seed.entity_ids[filer_idx],
            target_entity_id: seed.entity_ids[author_idx],
            trigger_label: format!("private_{fi}"),
            expects_community_feedback: false,
        });
    }

    eprintln!("Filed {} cases", cases.len());

    let t_sim = Instant::now();
    let max_rounds = 20;
    let mut round = 0;

    loop {
        round += 1;
        if round > max_rounds {
            eprintln!("WARNING: max rounds ({max_rounds}) reached");
            break;
        }

        let summary = judicial::nominate_jurors_for_open_cases(pool, 256)
            .await
            .expect("nomination");

        let pending_processed = respond_to_pending_invites_deterministic(pool).await;

        if summary.cases_examined == 0 && pending_processed == 0 && round > 1 {
            break;
        }

        eprintln!(
            "  round {round}: examined={}, filled={}, invites_created={}, pending_processed={}",
            summary.cases_examined,
            summary.cases_filled,
            summary.invites_created,
            pending_processed,
        );
    }

    eprintln!(
        "Simulation completed in {}ms ({round} rounds)",
        t_sim.elapsed().as_millis()
    );

    let mut resolved_count = 0u32;
    let mut upheld_count = 0u32;
    let mut overturned_count = 0u32;

    for cs in &cases {
        let snap = fetch_case_settlement(pool, cs.case_id).await;

        let verdict_label = if snap.status_key == ref_keys::KEY_JUDICIAL_CASE_RESOLVED {
            resolved_count += 1;
            let v = verdict_from_weights(&snap);
            match v {
                judicial::VerdictOutcome::Upheld => {
                    upheld_count += 1;
                    "UPHELD"
                }
                judicial::VerdictOutcome::Overturned => {
                    overturned_count += 1;
                    "OVERTURNED"
                }
                judicial::VerdictOutcome::Hung => {
                    upheld_count += 1;
                    "UPHELD (hung→safety)"
                }
            }
        } else {
            "PENDING"
        };

        let threshold = snap.normalized_total * 2.0 / 3.0;
        eprintln!(
            "{:<14} {:<10} {:<10} {:<10} {:<10.2} {:<10.2} {:<10.2} {:<10} {:<10}",
            cs.trigger_label,
            snap.jury_size,
            snap.votes_for,
            snap.votes_against,
            snap.normalized_for,
            snap.normalized_total,
            threshold,
            verdict_label,
            snap.status_key,
        );

        if snap.status_key == ref_keys::KEY_JUDICIAL_CASE_RESOLVED {
            assert_case_fully_resolved(&snap, &cs.trigger_label);
            if cs.expects_community_feedback {
                assert_community_feed_feedback_applied(&snap, &cs.trigger_label);
            }
        }

        let bad_jurors: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM judicial_case_juror_invite jcji
            INNER JOIN ref_key_values rk_inv
                ON rk_inv.id = jcji.invite_status_id
               AND rk_inv.`group` = ?
               AND rk_inv.`key` <> ?
               AND rk_inv.is_active = 1
            WHERE jcji.judicial_case_id = ?
              AND jcji.juror_social_entity_id IN (?, ?)
            "#,
        )
        .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
        .bind(ref_keys::KEY_JCJI_VOIDED)
        .bind(cs.case_id)
        .bind(cs.originator_entity_id)
        .bind(cs.target_entity_id)
        .fetch_one(pool)
        .await
        .expect("juror exclusion check");

        assert_eq!(
            bad_jurors, 0,
            "case {} has originator or target as juror",
            cs.trigger_label
        );
    }

    eprintln!("\nResolved: {resolved_count}/16, Upheld: {upheld_count}, Overturned: {overturned_count}");
    assert!(
        resolved_count >= 14,
        "expected at least 14 cases resolved, got {resolved_count}"
    );

    eprintln!("Total test time: {}ms", t_total.elapsed().as_millis());
    db.cleanup().await;
}
