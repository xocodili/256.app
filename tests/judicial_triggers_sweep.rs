//! Integration sweep: every active `judicial_case_trigger` ref key has a focused path test.
//!
//! | Trigger (`ref_keys`)              | Human jury | Covered by |
//! |-----------------------------------|------------|------------|
//! | `civic_dispute`                   | yes        | this file + `judicial_verdict.rs` / `judicial_lifecycle.rs` |
//! | `emergency_safety`                | after contest | `emergency_moderation.rs` |
//! | `membership_review`               | yes        | `membership_review_judicial.rs` |
//! | `constitutional_amendment`        | yes        | `constitutional_amendment_upheld_adds_clause` (this file) |
//! | `moderation_appeal`               | AI council | `moderation_appeal_escalates_without_human_jurors` (this file) |

mod common;
mod judicial_harness;

use app::ref_keys;
use app::social::judicial::{self, trigger_uses_ai_council_review, trigger_uses_human_jury};
use common::seed_light;
use judicial_harness::{
    assert_case_fully_resolved, case_trigger_key, drive_human_jury_case_to_resolved,
    fetch_case_settlement, file_community_post_dispute, file_constitutional_new_clause_case,
    file_moderation_appeal_case, load_case_ref_ids, pending_count_for_case,
    run_nomination_until_quiescent, set_entity_rating,
};
use sqlx::MySqlPool;

const CANONICAL_TRIGGERS: &[&str] = &[
    ref_keys::KEY_JUDICIAL_TRIGGER_CIVIC_DISPUTE,
    ref_keys::KEY_JUDICIAL_TRIGGER_EMERGENCY_SAFETY,
    ref_keys::KEY_JUDICIAL_TRIGGER_MEMBERSHIP_REVIEW,
    ref_keys::KEY_JUDICIAL_TRIGGER_CONSTITUTIONAL_AMENDMENT,
    ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_REPARENT,
    ref_keys::KEY_JUDICIAL_TRIGGER_CASE_APPEAL,
];

#[tokio::test]
async fn active_ref_keys_include_all_canonical_judicial_triggers() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;

    let keys: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT `key`
        FROM ref_key_values
        WHERE `group` = ?
          AND is_active = 1
        ORDER BY `key`
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_TRIGGER)
    .fetch_all(pool)
    .await
    .expect("trigger keys");

    for expected in CANONICAL_TRIGGERS {
        assert!(
            keys.iter().any(|k| k == expected),
            "missing active judicial_case_trigger ref key: {expected}"
        );
    }

    for key in &keys {
        if trigger_uses_human_jury(key) {
            assert!(
                !trigger_uses_ai_council_review(key),
                "{key} must not be both human jury and AI council"
            );
        }
        if trigger_uses_ai_council_review(key) {
            assert!(
                !trigger_uses_human_jury(key),
                "{key} must not be both AI council and human jury"
            );
        }
    }
}

#[tokio::test]
async fn civic_dispute_trigger_seats_human_jury() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let seed = seed_light(pool).await;
    let refs = load_case_ref_ids(pool).await;

    let author_idx = 10;
    let filer_idx = 11;
    let case_id = file_community_post_dispute(
        pool,
        &refs,
        seed.entity_ids[filer_idx],
        seed.entity_ids[author_idx],
        seed.post_community_ids[author_idx],
        seed.user_ids[filer_idx],
    )
    .await;

    assert_eq!(
        case_trigger_key(pool, case_id).await,
        ref_keys::KEY_JUDICIAL_TRIGGER_CIVIC_DISPUTE
    );

    run_nomination_until_quiescent(pool, 32).await;
    assert!(
        pending_count_for_case(pool, case_id).await > 0,
        "civic_dispute should create human juror invites"
    );
}

#[tokio::test]
async fn emergency_safety_uncontested_skips_jury_nomination() {
    use judicial_harness::{
        file_emergency_safety_case, load_emergency_ref_ids, post_status_key, root_emergency_clause_402_id,
    };

    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let seed = seed_light(pool).await;
    let refs = load_emergency_ref_ids(pool).await;
    let clause_id = root_emergency_clause_402_id(pool).await;

    let author_idx = 10;
    let filer_idx = 11;
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
        case_trigger_key(pool, case_id).await,
        ref_keys::KEY_JUDICIAL_TRIGGER_EMERGENCY_SAFETY
    );
    assert_eq!(
        post_status_key(pool, root_post_id).await,
        ref_keys::KEY_POST_SUPPRESSED
    );

    run_nomination_until_quiescent(pool, 32).await;
    assert_eq!(
        pending_count_for_case(pool, case_id).await,
        0,
        "contained emergency should not seat jurors until contested"
    );
}

#[tokio::test]
async fn constitutional_amendment_upheld_adds_clause() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let seed = seed_light(pool).await;

    let community_id = seed
        .community_entity_ids
        .get(2)
        .copied()
        .expect("council community");
    let proposer_idx = 12;
    let proposer_id = seed.entity_ids[proposer_idx];
    let acting_user_id = seed.user_ids[proposer_idx];

    set_entity_rating(pool, proposer_id, "4.00").await;

    let before: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM community_constitution_item WHERE community_id = ?"#,
    )
    .bind(community_id)
    .fetch_one(pool)
    .await
    .expect("count clauses");

    let proposed = "Integration sweep: new constitutional clause.";
    let case_id = file_constitutional_new_clause_case(
        pool,
        proposer_id,
        community_id,
        acting_user_id,
        proposed,
    )
    .await;

    assert_eq!(
        case_trigger_key(pool, case_id).await,
        ref_keys::KEY_JUDICIAL_TRIGGER_CONSTITUTIONAL_AMENDMENT
    );

    drive_human_jury_case_to_resolved(pool, case_id, true).await;

    let snap = fetch_case_settlement(pool, case_id).await;
    assert_case_fully_resolved(&snap, "constitutional_amendment");

    let after: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM community_constitution_item WHERE community_id = ?"#,
    )
    .bind(community_id)
    .fetch_one(pool)
    .await
    .expect("count clauses after");

    assert!(
        after > before,
        "upheld constitutional amendment should insert a community clause"
    );

    let stored: Option<String> = sqlx::query_scalar(
        r#"
        SELECT item FROM community_constitution_item
        WHERE community_id = ?
        ORDER BY id DESC
        LIMIT 1
        "#,
    )
    .bind(community_id)
    .fetch_optional(pool)
    .await
    .expect("latest clause");

    assert_eq!(stored.as_deref(), Some(proposed));
}

async fn resolve_community_post_dispute(pool: &MySqlPool) -> (u64, u64, u64, u64) {
    let seed = seed_light(pool).await;
    let refs = load_case_ref_ids(pool).await;
    let author_idx = 20;
    let filer_idx = 21;
    let post_id = seed.post_community_ids[author_idx];
    let case_id = file_community_post_dispute(
        pool,
        &refs,
        seed.entity_ids[filer_idx],
        seed.entity_ids[author_idx],
        post_id,
        seed.user_ids[filer_idx],
    )
    .await;
    drive_human_jury_case_to_resolved(pool, case_id, true).await;
    (
        case_id,
        post_id,
        seed.entity_ids[filer_idx],
        seed.user_ids[filer_idx],
    )
}

#[tokio::test]
async fn moderation_appeal_escalates_without_human_jurors() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;

    let (prior_case_id, post_id, appeal_origin, acting_user_id) =
        resolve_community_post_dispute(pool).await;

    let appeal_target: u64 = sqlx::query_scalar(
        r#"SELECT target_social_entity_id FROM judicial_case WHERE id = ?"#,
    )
    .bind(prior_case_id)
    .fetch_one(pool)
    .await
    .expect("prior target");

    let appeal_case_id = file_moderation_appeal_case(
        pool,
        prior_case_id,
        appeal_origin,
        appeal_target,
        post_id,
        acting_user_id,
    )
    .await;

    assert_eq!(
        case_trigger_key(pool, appeal_case_id).await,
        ref_keys::KEY_JUDICIAL_TRIGGER_CASE_APPEAL
    );

    let status_key: String = sqlx::query_scalar(
        r#"
        SELECT rk.`key`
        FROM judicial_case jc
        INNER JOIN ref_key_values rk ON rk.id = jc.status_id
        WHERE jc.id = ?
        "#,
    )
    .bind(appeal_case_id)
    .fetch_one(pool)
    .await
    .expect("appeal status");

    assert_eq!(status_key, ref_keys::KEY_JUDICIAL_CASE_ESCALATED_TO_AI);

    let jury_size: u8 = sqlx::query_scalar(r#"SELECT jury_size FROM judicial_case WHERE id = ?"#)
        .bind(appeal_case_id)
        .fetch_one(pool)
        .await
        .expect("jury_size");
    assert_eq!(jury_size, 0);

    judicial::nominate_jurors_for_open_cases(pool, 64)
        .await
        .expect("nominate");

    assert_eq!(
        pending_count_for_case(pool, appeal_case_id).await,
        0,
        "moderation_appeal must not receive human juror invites"
    );
}
