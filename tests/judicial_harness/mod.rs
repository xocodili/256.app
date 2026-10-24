//! Shared helpers for judicial integration simulations.

use app::ref_keys;
use app::social::judicial::{self, AbstainOutcome, CastVoteOutcome, VerdictOutcome};
use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::MySqlPool;

pub struct CaseRefIds {
    pub trigger_id: u64,
    pub status_open_id: u64,
}

pub async fn load_case_ref_ids(pool: &MySqlPool) -> CaseRefIds {
    let trigger_id: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? AND is_active = 1 LIMIT 1",
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_TRIGGER)
    .bind(ref_keys::KEY_JUDICIAL_TRIGGER_CIVIC_DISPUTE)
    .fetch_one(pool)
    .await
    .expect("civic_dispute trigger id");

    let status_open_id: u64 = sqlx::query_scalar(
        "SELECT id FROM ref_key_values WHERE `group` = ? AND `key` = ? AND is_active = 1 LIMIT 1",
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(ref_keys::KEY_JUDICIAL_CASE_OPEN)
    .fetch_one(pool)
    .await
    .expect("open status id");

    CaseRefIds {
        trigger_id,
        status_open_id,
    }
}

/// Mirror production filing: root thread enters `under_judicial_review`.
pub async fn mark_post_thread_under_judicial_review(
    pool: &MySqlPool,
    post_id: u64,
    modified_by: u64,
) {
    let root_id: u64 = sqlx::query_scalar(
        r#"SELECT COALESCE(p.root_post_id, p.id) FROM post p WHERE p.id = ? LIMIT 1"#,
    )
    .bind(post_id)
    .fetch_one(pool)
    .await
    .expect("post root id");

    let status_id: u64 = sqlx::query_scalar(
        r#"
        SELECT id FROM ref_key_values
        WHERE `group` = ? AND `key` = ? AND is_active = 1
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_POST_UNDER_JUDICIAL_REVIEW)
    .fetch_one(pool)
    .await
    .expect("under_judicial_review status id");

    sqlx::query(
        r#"
        UPDATE post
        SET status_id = ?, modified_by = ?
        WHERE id = ? OR root_post_id = ?
        "#,
    )
    .bind(status_id)
    .bind(modified_by)
    .bind(root_id)
    .bind(root_id)
    .execute(pool)
    .await
    .expect("mark post under judicial review");
}

/// File a community-audience post dispute with `jury_size` frozen at filing time.
pub async fn file_community_post_dispute(
    pool: &MySqlPool,
    refs: &CaseRefIds,
    originator_entity_id: u64,
    target_entity_id: u64,
    post_id: u64,
    originator_user_id: u64,
) -> u64 {
    let pool_size: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(DISTINCT ser.from_social_entity_id)
        FROM post_audience_entity pae
        INNER JOIN social_community sc ON sc.social_entity_id = pae.audience_entity_id
        INNER JOIN social_entity_relationship ser
            ON ser.to_social_entity_id = sc.social_entity_id
           AND ser.ended_at IS NULL
        INNER JOIN ref_key_values rk_type ON rk_type.id = ser.type_id
            AND rk_type.`group` = ?
            AND rk_type.`key` = ?
        INNER JOIN ref_key_values rk_status ON rk_status.id = ser.status_id
            AND rk_status.`group` = ?
            AND rk_status.`key` = ?
        INNER JOIN social_entity se ON se.id = ser.from_social_entity_id
        INNER JOIN social_human sh ON sh.social_entity_id = se.id
        WHERE pae.post_id = ?
          AND se.id NOT IN (?, ?)
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_VERIFIED)
    .bind(post_id)
    .bind(originator_entity_id)
    .bind(target_entity_id)
    .fetch_one(pool)
    .await
    .expect("community pool size");

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
    .expect("insert judicial_case");

    let case_id = result.last_insert_id();
    mark_post_thread_under_judicial_review(pool, post_id, originator_user_id).await;
    case_id
}

pub async fn pending_invites_for_active_cases(
    pool: &MySqlPool,
) -> Vec<(u64, u64, Option<u64>, u64)> {
    sqlx::query_as(
        r#"
        SELECT jcji.id, jcji.judicial_case_id, jcji.juror_user_id, jcji.juror_social_entity_id
        FROM judicial_case_juror_invite jcji
        INNER JOIN judicial_case jc ON jc.id = jcji.judicial_case_id
        INNER JOIN ref_key_values rk
            ON rk.id = jc.status_id
           AND rk.`group` = ?
           AND rk.`key` IN (?, ?)
           AND rk.is_active = 1
        INNER JOIN ref_key_values rk_inv
            ON rk_inv.id = jcji.invite_status_id
           AND rk_inv.`group` = ?
           AND rk_inv.`key` = ?
           AND rk_inv.is_active = 1
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(ref_keys::KEY_JUDICIAL_CASE_OPEN)
    .bind(ref_keys::KEY_JUDICIAL_CASE_JURY_ACTIVE)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
    .bind(ref_keys::KEY_JCJI_PENDING)
    .fetch_all(pool)
    .await
    .expect("pending invites")
}

/// Drive pending invites to `for` / `against` / abstain using production `cast_vote` / `abstain_from_invite`.
pub async fn respond_to_pending_invites_deterministic(pool: &MySqlPool) -> usize {
    let pending = pending_invites_for_active_cases(pool).await;
    let n = pending.len();

    for (invite_id, _case_id, juror_user_id, juror_entity_id) in pending {
        let Some(user_id) = juror_user_id else {
            continue;
        };
        match juror_entity_id % 9 {
            0 | 1 | 2 => {
                let outcome = judicial::abstain_from_invite(pool, user_id, invite_id)
                    .await
                    .expect("abstain");
                assert!(
                    matches!(
                        outcome,
                        AbstainOutcome::Free { .. } | AbstainOutcome::OverFreeQuota { .. }
                    ),
                    "unexpected abstain: {outcome:?}"
                );
            }
            3 | 4 | 5 | 6 => {
                let outcome = judicial::cast_vote(
                    pool,
                    user_id,
                    invite_id,
                    ref_keys::KEY_JCJI_VOTE_FOR,
                )
                .await
                .expect("cast for");
                assert_eq!(outcome, CastVoteOutcome::Recorded);
            }
            _ => {
                let outcome = judicial::cast_vote(
                    pool,
                    user_id,
                    invite_id,
                    ref_keys::KEY_JCJI_VOTE_AGAINST,
                )
                .await
                .expect("cast against");
                assert_eq!(outcome, CastVoteOutcome::Recorded);
            }
        }
    }

    n
}

/// Seat jurors until no new pending invites appear or `max_rounds` is hit.
pub async fn run_nomination_until_quiescent(pool: &MySqlPool, max_rounds: u32) {
    for round in 1..=max_rounds {
        let summary = judicial::nominate_jurors_for_open_cases(pool, 256)
            .await
            .expect("nominate_jurors_for_open_cases");
        if summary.cases_examined == 0 && round > 1 {
            break;
        }
    }
}

#[derive(Debug, Clone)]
pub struct CaseSettlementSnapshot {
    pub status_key: String,
    pub jury_size: u8,
    pub votes_for: u8,
    pub votes_against: u8,
    pub closed_at: Option<DateTime<Utc>>,
    pub jury_civic_rating_applied_at: Option<NaiveDateTime>,
    pub community_rating_feedback_applied_at: Option<NaiveDateTime>,
    pub normalized_for: f64,
    pub normalized_total: f64,
}

pub async fn fetch_case_settlement(pool: &MySqlPool, case_id: u64) -> CaseSettlementSnapshot {
    let row: (
        u8,
        u8,
        u8,
        String,
        Option<DateTime<Utc>>,
        Option<NaiveDateTime>,
        Option<NaiveDateTime>,
    ) = sqlx::query_as(
        r#"
        SELECT
            jc.jury_size,
            jc.jury_votes_for,
            jc.jury_votes_against,
            rk.`key`,
            jc.closed_at,
            jc.jury_civic_rating_applied_at,
            jc.community_rating_feedback_applied_at
        FROM judicial_case jc
        INNER JOIN ref_key_values rk ON rk.id = jc.status_id
        WHERE jc.id = ?
        "#,
    )
    .bind(case_id)
    .fetch_one(pool)
    .await
    .expect("fetch case settlement row");

    let vote_weights: Vec<(String, rust_decimal::Decimal)> = sqlx::query_as(
        r#"
        SELECT rk_vote.`key`, jcji.juror_rating_at_invite
        FROM judicial_case_juror_invite jcji
        INNER JOIN ref_key_values rk_inv
            ON rk_inv.id = jcji.invite_status_id
           AND rk_inv.`group` = ?
           AND rk_inv.`key` = ?
           AND rk_inv.is_active = 1
        INNER JOIN ref_key_values rk_vote
            ON rk_vote.id = jcji.vote_direction_id
           AND rk_vote.`group` = ?
           AND rk_vote.is_active = 1
        WHERE jcji.judicial_case_id = ?
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
    .bind(ref_keys::KEY_JCJI_ACCEPTED)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_VOTE_DIRECTION)
    .bind(case_id)
    .fetch_all(pool)
    .await
    .expect("vote weights");

    let mut normalized_for = 0.0f64;
    let mut normalized_total = 0.0f64;
    for (dir, rating) in &vote_weights {
        let w = judicial::normalize_vote_weight(rating);
        normalized_total += w;
        if dir == ref_keys::KEY_JCJI_VOTE_FOR {
            normalized_for += w;
        }
    }

    CaseSettlementSnapshot {
        status_key: row.3,
        jury_size: row.0,
        votes_for: row.1,
        votes_against: row.2,
        closed_at: row.4,
        jury_civic_rating_applied_at: row.5,
        community_rating_feedback_applied_at: row.6,
        normalized_for,
        normalized_total,
    }
}

pub fn assert_case_fully_resolved(snap: &CaseSettlementSnapshot, case_label: &str) {
    assert_eq!(
        snap.status_key,
        ref_keys::KEY_JUDICIAL_CASE_RESOLVED,
        "case {case_label} should be resolved, got {}",
        snap.status_key
    );
    assert!(
        snap.closed_at.is_some(),
        "case {case_label} missing closed_at"
    );
    assert!(
        snap.jury_civic_rating_applied_at.is_some(),
        "case {case_label} missing jury_civic_rating_applied_at"
    );
    assert!(
        snap.normalized_total > 0.0,
        "case {case_label} has no accepted weighted votes"
    );
}

pub fn assert_community_feed_feedback_applied(snap: &CaseSettlementSnapshot, case_label: &str) {
    assert!(
        snap.community_rating_feedback_applied_at.is_some(),
        "case {case_label} missing community_rating_feedback_applied_at (feed-scoped community)"
    );
}

pub fn verdict_from_weights(snap: &CaseSettlementSnapshot) -> VerdictOutcome {
    let normalized_against = (snap.normalized_total - snap.normalized_for).max(0.0);
    judicial::evaluate_weighted_verdict(
        ref_keys::KEY_JUDICIAL_TRIGGER_CIVIC_DISPUTE,
        snap.normalized_for,
        normalized_against,
    )
    .outcome
}

/// Mark every seated invite as voted without calling `check_and_resolve_verdict` (simulates votes
/// committed but resolution not yet run).
pub async fn accept_all_invites_with_votes_sql(
    pool: &MySqlPool,
    case_id: u64,
    default_direction: &str,
) {
    assert!(
        default_direction == ref_keys::KEY_JCJI_VOTE_FOR
            || default_direction == ref_keys::KEY_JCJI_VOTE_AGAINST
    );
    sqlx::query(
        r#"
        UPDATE judicial_case_juror_invite jcji
        INNER JOIN ref_key_values rk_acc
            ON rk_acc.`group` = ?
           AND rk_acc.`key` = ?
           AND rk_acc.is_active = 1
        INNER JOIN ref_key_values rk_vote
            ON rk_vote.`group` = ?
           AND rk_vote.`key` = ?
           AND rk_vote.is_active = 1
        SET jcji.invite_status_id = rk_acc.id,
            jcji.vote_direction_id = rk_vote.id,
            jcji.responded_at = UTC_TIMESTAMP(6)
        WHERE jcji.judicial_case_id = ?
          AND jcji.vote_direction_id IS NULL
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
    .bind(ref_keys::KEY_JCJI_ACCEPTED)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_VOTE_DIRECTION)
    .bind(default_direction)
    .bind(case_id)
    .execute(pool)
    .await
    .expect("accept invites via sql");
}

pub async fn active_ref_id(pool: &MySqlPool, group: &str, key: &str) -> u64 {
    sqlx::query_scalar(
        r#"
        SELECT id FROM ref_key_values
        WHERE `group` = ? AND `key` = ? AND is_active = 1
        LIMIT 1
        "#,
    )
    .bind(group)
    .bind(key)
    .fetch_one(pool)
    .await
    .unwrap_or_else(|_| panic!("ref_key_values missing ({group}, {key})"))
}

pub async fn case_trigger_key(pool: &MySqlPool, case_id: u64) -> String {
    sqlx::query_scalar(
        r#"
        SELECT rk.`key`
        FROM judicial_case jc
        INNER JOIN ref_key_values rk ON rk.id = jc.trigger_type_id
        WHERE jc.id = ?
        "#,
    )
    .bind(case_id)
    .fetch_one(pool)
    .await
    .expect("case trigger key")
}

pub async fn grant_noms(pool: &MySqlPool, human_entity_id: u64, amount: u16) {
    sqlx::query("UPDATE social_human SET ai_tokens = ? WHERE social_entity_id = ?")
        .bind(amount)
        .bind(human_entity_id)
        .execute(pool)
        .await
        .expect("grant noms");
}

pub async fn set_entity_rating(pool: &MySqlPool, entity_id: u64, rating: &str) {
    sqlx::query("UPDATE social_entity SET rating = ? WHERE id = ?")
        .bind(rating)
        .bind(entity_id)
        .execute(pool)
        .await
        .expect("set entity rating");
}

/// File a `constitutional_amendment` case (new clause) through the same jury-size freeze as production.
pub async fn file_constitutional_new_clause_case(
    pool: &MySqlPool,
    proposer_entity_id: u64,
    community_entity_id: u64,
    acting_user_id: u64,
    proposed_plain: &str,
) -> u64 {
    let trigger_id = active_ref_id(
        pool,
        ref_keys::GROUP_JUDICIAL_CASE_TRIGGER,
        ref_keys::KEY_JUDICIAL_TRIGGER_CONSTITUTIONAL_AMENDMENT,
    )
    .await;
    let status_open_id = active_ref_id(
        pool,
        ref_keys::GROUP_JUDICIAL_CASE_STATUS,
        ref_keys::KEY_JUDICIAL_CASE_OPEN,
    )
    .await;
    let type_originator_id = active_ref_id(
        pool,
        ref_keys::GROUP_JUDICIAL_CASE_COMMENT_TYPE,
        ref_keys::KEY_JCC_ORIGINATOR,
    )
    .await;

    let mut tx = pool.begin().await.expect("begin tx");
    let jury_size_frozen = judicial::compute_jury_size_to_freeze_at_filing_tx(
        &mut tx,
        proposer_entity_id,
        community_entity_id,
        Some(acting_user_id),
        None,
        ref_keys::KEY_JUDICIAL_TRIGGER_CONSTITUTIONAL_AMENDMENT,
    )
    .await
    .expect("freeze jury size");

    let originator_rating: rust_decimal::Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(se.rating, 0) FROM social_entity se WHERE se.id = ? LIMIT 1"#,
    )
    .bind(proposer_entity_id)
    .fetch_one(&mut *tx)
    .await
    .expect("originator rating");

    let jc_res = sqlx::query(
        r#"
        INSERT INTO judicial_case (
            originator_social_entity_id, target_social_entity_id,
            trigger_type_id, status_id, jury_size,
            created_by, modified_by
        )
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(proposer_entity_id)
    .bind(community_entity_id)
    .bind(trigger_id)
    .bind(status_open_id)
    .bind(jury_size_frozen)
    .bind(acting_user_id)
    .bind(acting_user_id)
    .execute(&mut *tx)
    .await
    .expect("insert constitutional case");
    let case_id = jc_res.last_insert_id();

    let proposal_html = format!("<p>{}</p>", proposed_plain);
    sqlx::query(
        r#"
        INSERT INTO judicial_case_constitutional_amendment (
            judicial_case_id, amendment_kind,
            target_constitution_item_id,
            proposed_body_plain, proposed_body_markup, proposed_body_html_sanitized
        )
        VALUES (?, 'new_clause', NULL, ?, ?, ?)
        "#,
    )
    .bind(case_id)
    .bind(proposed_plain)
    .bind(&proposal_html)
    .bind(&proposal_html)
    .execute(&mut *tx)
    .await
    .expect("insert amendment row");

    sqlx::query(
        r#"
        INSERT INTO judicial_case_comment (
            judicial_case_id, type_id, author_user_id, author_social_entity_id,
            author_entity_rating_at_create, body_plain, body_markup, body_html_sanitized,
            created_by, modified_by
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(case_id)
    .bind(type_originator_id)
    .bind(acting_user_id)
    .bind(proposer_entity_id)
    .bind(originator_rating)
    .bind("Filing rationale for integration test.")
    .bind("<p>Filing rationale for integration test.</p>")
    .bind("<p>Filing rationale for integration test.</p>")
    .bind(acting_user_id)
    .bind(acting_user_id)
    .execute(&mut *tx)
    .await
    .expect("insert originator comment");

    tx.commit().await.expect("commit constitutional filing");
    case_id
}

/// File a `moderation_appeal` case escalated to the AI council (no human jury).
pub async fn file_moderation_appeal_case(
    pool: &MySqlPool,
    prior_case_id: u64,
    appeal_origin_entity_id: u64,
    appeal_target_entity_id: u64,
    subject_post_id: u64,
    acting_user_id: u64,
) -> u64 {
    let trigger_id = active_ref_id(
        pool,
        ref_keys::GROUP_JUDICIAL_CASE_TRIGGER,
        ref_keys::KEY_JUDICIAL_TRIGGER_CASE_APPEAL,
    )
    .await;
    let status_escalated_id = active_ref_id(
        pool,
        ref_keys::GROUP_JUDICIAL_CASE_STATUS,
        ref_keys::KEY_JUDICIAL_CASE_ESCALATED_TO_AI,
    )
    .await;
    let resolution_tier_ai_id = active_ref_id(
        pool,
        ref_keys::GROUP_JUDICIAL_RESOLUTION_TIER,
        ref_keys::KEY_JUDICIAL_RESOLUTION_TIER_AI_COUNCIL,
    )
    .await;
    let type_originator_id = active_ref_id(
        pool,
        ref_keys::GROUP_JUDICIAL_CASE_COMMENT_TYPE,
        ref_keys::KEY_JCC_ORIGINATOR,
    )
    .await;

    let originator_rating: rust_decimal::Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(se.rating, 0) FROM social_entity se WHERE se.id = ? LIMIT 1"#,
    )
    .bind(appeal_origin_entity_id)
    .fetch_one(pool)
    .await
    .expect("originator rating");

    let jc_res = sqlx::query(
        r#"
        INSERT INTO judicial_case (
            originator_social_entity_id, target_social_entity_id,
            trigger_type_id, status_id, subject_post_id,
            appealed_from_judicial_case_id, resolution_tier_id,
            jury_size, jury_votes_for, jury_votes_against,
            ai_votes_for, ai_votes_against,
            created_by, modified_by
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, 0, 0, 0, 0, 0, ?, ?)
        "#,
    )
    .bind(appeal_origin_entity_id)
    .bind(appeal_target_entity_id)
    .bind(trigger_id)
    .bind(status_escalated_id)
    .bind(subject_post_id)
    .bind(prior_case_id)
    .bind(resolution_tier_ai_id)
    .bind(acting_user_id)
    .bind(acting_user_id)
    .execute(pool)
    .await
    .expect("insert appeal case");
    let case_id = jc_res.last_insert_id();

    sqlx::query(
        r#"
        INSERT INTO judicial_case_constitution_item (judicial_case_id, constitution_item_id, created_by)
        SELECT ?, cci.constitution_item_id, ?
        FROM judicial_case_constitution_item AS cci
        WHERE cci.judicial_case_id = ?
        "#,
    )
    .bind(case_id)
    .bind(acting_user_id)
    .bind(prior_case_id)
    .execute(pool)
    .await
    .expect("copy constitution items");

    sqlx::query(
        r#"
        INSERT INTO judicial_case_comment (
            judicial_case_id, type_id, author_user_id, author_social_entity_id,
            author_entity_rating_at_create, body_plain, body_markup, body_html_sanitized,
            created_by, modified_by
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(case_id)
    .bind(type_originator_id)
    .bind(acting_user_id)
    .bind(appeal_origin_entity_id)
    .bind(originator_rating)
    .bind("Appeal rationale for integration test.")
    .bind("<p>Appeal rationale for integration test.</p>")
    .bind("<p>Appeal rationale for integration test.</p>")
    .bind(acting_user_id)
    .bind(acting_user_id)
    .execute(pool)
    .await
    .expect("insert appeal comment");

    case_id
}

pub struct EmergencyRefIds {
    pub trigger_id: u64,
    pub status_contained_id: u64,
}

pub async fn load_emergency_ref_ids(pool: &MySqlPool) -> EmergencyRefIds {
    let trigger_id = active_ref_id(
        pool,
        ref_keys::GROUP_JUDICIAL_CASE_TRIGGER,
        ref_keys::KEY_JUDICIAL_TRIGGER_EMERGENCY_SAFETY,
    )
    .await;
    let status_contained_id = active_ref_id(
        pool,
        ref_keys::GROUP_JUDICIAL_CASE_STATUS,
        ref_keys::KEY_JUDICIAL_CASE_CONTAINED,
    )
    .await;
    EmergencyRefIds {
        trigger_id,
        status_contained_id,
    }
}

/// Root charter §4.1 clause id for integration tests (migration-seeded).
pub async fn root_emergency_clause_401_id(pool: &MySqlPool) -> u64 {
    sqlx::query_scalar(
        r#"
        SELECT cci.id
        FROM community_constitution_item cci
        INNER JOIN social_entity se ON se.id = cci.community_id
        WHERE se.handle = 'operating_charter_256'
          AND cci.point = 401
          AND cci.status = 'active'
        LIMIT 1
        "#,
    )
    .fetch_one(pool)
    .await
    .expect("root §4.1 clause 401")
}

pub async fn root_emergency_clause_402_id(pool: &MySqlPool) -> u64 {
    sqlx::query_scalar(
        r#"
        SELECT cci.id
        FROM community_constitution_item cci
        INNER JOIN social_entity se ON se.id = cci.community_id
        WHERE se.handle = 'operating_charter_256'
          AND cci.point = 402
          AND cci.status = 'active'
        LIMIT 1
        "#,
    )
    .fetch_one(pool)
    .await
    .expect("root §4.2 clause 402")
}

pub async fn post_status_key(pool: &MySqlPool, root_post_id: u64) -> String {
    sqlx::query_scalar(
        r#"
        SELECT rk.`key`
        FROM post p
        INNER JOIN ref_key_values rk ON rk.id = p.status_id
        WHERE p.id = ?
        "#,
    )
    .bind(root_post_id)
    .fetch_one(pool)
    .await
    .expect("post status key")
}

/// File uncontested emergency case and suppress thread (mirrors production filing side effects).
pub async fn file_emergency_safety_case(
    pool: &MySqlPool,
    refs: &EmergencyRefIds,
    originator_entity_id: u64,
    target_entity_id: u64,
    post_id: u64,
    originator_user_id: u64,
    clause_id: u64,
) -> u64 {
    let suppressed_id = active_ref_id(pool, ref_keys::GROUP_STATUS, ref_keys::KEY_POST_SUPPRESSED).await;
    let root_id: u64 = sqlx::query_scalar(
        r#"SELECT COALESCE(p.root_post_id, p.id) FROM post p WHERE p.id = ?"#,
    )
    .bind(post_id)
    .fetch_one(pool)
    .await
    .expect("root id");

    let result = sqlx::query(
        r#"
        INSERT INTO judicial_case (
            originator_social_entity_id, target_social_entity_id,
            trigger_type_id, status_id, subject_post_id, jury_size,
            created_by
        ) VALUES (?, ?, ?, ?, ?, 0, ?)
        "#,
    )
    .bind(originator_entity_id)
    .bind(target_entity_id)
    .bind(refs.trigger_id)
    .bind(refs.status_contained_id)
    .bind(root_id)
    .bind(originator_user_id)
    .execute(pool)
    .await
    .expect("insert emergency case");
    let case_id = result.last_insert_id();

    sqlx::query(
        r#"
        INSERT INTO judicial_case_constitution_item (judicial_case_id, constitution_item_id, created_by)
        VALUES (?, ?, ?)
        "#,
    )
    .bind(case_id)
    .bind(clause_id)
    .bind(originator_user_id)
    .execute(pool)
    .await
    .expect("link emergency clause");

    sqlx::query(
        r#"
        UPDATE post SET status_id = ?, modified_by = ?
        WHERE id = ? OR root_post_id = ?
        "#,
    )
    .bind(suppressed_id)
    .bind(originator_user_id)
    .bind(root_id)
    .bind(root_id)
    .execute(pool)
    .await
    .expect("suppress thread");

    case_id
}

/// Backdate uncontested emergency case so standing settlement is eligible.
pub async fn backdate_emergency_case_for_standing(pool: &MySqlPool, case_id: u64, days: i64) {
    sqlx::query(
        r#"
        UPDATE judicial_case
        SET opened_at = UTC_TIMESTAMP(6) - INTERVAL ? DAY
        WHERE id = ?
        "#,
    )
    .bind(days)
    .bind(case_id)
    .execute(pool)
    .await
    .expect("backdate emergency case");
}

/// Cast votes on all pending invites for `case_id` until the case reaches `resolved` or rounds exhaust.
pub async fn drive_human_jury_case_to_resolved(pool: &MySqlPool, case_id: u64, vote_for: bool) {
    let direction = if vote_for {
        ref_keys::KEY_JCJI_VOTE_FOR
    } else {
        ref_keys::KEY_JCJI_VOTE_AGAINST
    };

    for _ in 0..16 {
        run_nomination_until_quiescent(pool, 32).await;
        let pending = pending_invites_for_active_cases(pool).await;
        for (invite_id, cid, juror_user_id, _) in pending {
            if cid != case_id {
                continue;
            }
            let Some(user_id) = juror_user_id else {
                continue;
            };
            let outcome = judicial::cast_vote(pool, user_id, invite_id, direction)
                .await
                .expect("cast_vote");
            assert_eq!(outcome, CastVoteOutcome::Recorded);
        }

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

        if status_key == ref_keys::KEY_JUDICIAL_CASE_RESOLVED {
            return;
        }
    }
    panic!("case {case_id} did not resolve");
}

pub async fn pending_count_for_case(pool: &MySqlPool, case_id: u64) -> i64 {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM judicial_case_juror_invite jcji
        INNER JOIN ref_key_values rk
            ON rk.id = jcji.invite_status_id
           AND rk.`group` = ?
           AND rk.`key` = ?
           AND rk.is_active = 1
        WHERE jcji.judicial_case_id = ?
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
    .bind(ref_keys::KEY_JCJI_PENDING)
    .bind(case_id)
    .fetch_one(pool)
    .await
    .expect("pending count")
}

/// Insert `count` abstained invites in the rolling window so quota tests can start at the cap.
pub async fn seed_recent_abstentions_for_user(
    pool: &MySqlPool,
    user_id: u64,
    juror_entity_id: u64,
    count: u32,
) {
    let trigger_id = active_ref_id(
        pool,
        ref_keys::GROUP_JUDICIAL_CASE_TRIGGER,
        ref_keys::KEY_JUDICIAL_TRIGGER_CIVIC_DISPUTE,
    )
    .await;
    let status_resolved_id = active_ref_id(
        pool,
        ref_keys::GROUP_JUDICIAL_CASE_STATUS,
        ref_keys::KEY_JUDICIAL_CASE_RESOLVED,
    )
    .await;
    let abstained_status_id = active_ref_id(
        pool,
        ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS,
        ref_keys::KEY_JCJI_ABSTAINED,
    )
    .await;
    let rating_basis_global_id = active_ref_id(
        pool,
        ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_RATING_BASIS,
        ref_keys::KEY_JCJI_RB_GLOBAL,
    )
    .await;

    for _ in 0..count {
        let case_id = sqlx::query(
            r#"
            INSERT INTO judicial_case (
                originator_social_entity_id, target_social_entity_id,
                trigger_type_id, status_id, jury_size, created_by
            ) VALUES (?, ?, ?, ?, 1, ?)
            "#,
        )
        .bind(juror_entity_id)
        .bind(juror_entity_id)
        .bind(trigger_id)
        .bind(status_resolved_id)
        .bind(user_id)
        .execute(pool)
        .await
        .expect("insert abstention history case")
        .last_insert_id();

        sqlx::query(
            r#"
            INSERT INTO judicial_case_juror_invite (
                judicial_case_id, juror_social_entity_id, juror_user_id,
                juror_rating_at_invite, invite_status_id, rating_basis_id,
                selected_at, responded_at, created_by
            ) VALUES (?, ?, ?, 2.50, ?, ?, UTC_TIMESTAMP(6), UTC_TIMESTAMP(6), ?)
            "#,
        )
        .bind(case_id)
        .bind(juror_entity_id)
        .bind(user_id)
        .bind(abstained_status_id)
        .bind(rating_basis_global_id)
        .bind(user_id)
        .execute(pool)
        .await
        .expect("insert abstention history invite");
    }
}

pub async fn first_pending_invite_for_user_on_case(
    pool: &MySqlPool,
    case_id: u64,
    user_id: u64,
) -> Option<u64> {
    sqlx::query_scalar(
        r#"
        SELECT jcji.id
        FROM judicial_case_juror_invite jcji
        INNER JOIN ref_key_values rk
            ON rk.id = jcji.invite_status_id
           AND rk.`group` = ?
           AND rk.`key` = ?
           AND rk.is_active = 1
        WHERE jcji.judicial_case_id = ?
          AND jcji.juror_user_id = ?
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
    .bind(ref_keys::KEY_JCJI_PENDING)
    .bind(case_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .expect("pending invite for user")
}
