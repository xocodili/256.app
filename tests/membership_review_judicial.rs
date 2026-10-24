//! Membership-review judicial flow: file → jury → upheld → verified `member_of`.

mod common;
mod judicial_harness;

use app::ref_keys;
use app::social::judicial::{self, CastVoteOutcome, DeclineOutcome};
use app::social::judicial::membership_review;
use common::{seed_light, SeedIds};
use judicial_harness::{pending_invites_for_active_cases, run_nomination_until_quiescent};
use sqlx::MySqlPool;

async fn find_membership_review_fixture(
    pool: &MySqlPool,
    seed: &SeedIds,
) -> (u64, u64, u64) {
    let community_id = seed
        .community_entity_ids
        .get(1)
        .copied()
        .or_else(|| seed.community_entity_ids.first().copied())
        .expect("seed_light council communities");

    let reviewed_policy_id: u64 = sqlx::query_scalar(
        r#"
        SELECT id FROM ref_key_values
        WHERE `group` = ? AND `key` = ? AND is_active = 1
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_JOIN_POLICY)
    .bind(ref_keys::KEY_JOIN_POLICY_REVIEWED)
    .fetch_one(pool)
    .await
    .expect("reviewed join policy id");

    sqlx::query(
        r#"
        UPDATE social_community
        SET join_policy_id = ?
        WHERE social_entity_id = ?
        "#,
    )
    .bind(reviewed_policy_id)
    .bind(community_id)
    .execute(pool)
    .await
    .expect("set reviewed join policy");

    let applicant_idx = 1usize.min(seed.entity_ids.len().saturating_sub(1));
    let applicant_id = seed.entity_ids[applicant_idx];
    let acting_user_id = seed.user_ids[applicant_idx];

    sqlx::query(
        r#"
        UPDATE social_entity_relationship ser
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = ser.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` = ?
        SET ser.ended_at = UTC_TIMESTAMP(6)
        WHERE ser.from_social_entity_id = ?
          AND ser.to_social_entity_id = ?
          AND ser.ended_at IS NULL
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
    .bind(applicant_id)
    .bind(community_id)
    .execute(pool)
    .await
    .expect("clear applicant membership edge");

    (community_id, applicant_id, acting_user_id)
}

async fn vote_all_pending_for_case(pool: &MySqlPool, case_id: u64, vote_for: bool) {
    let direction = if vote_for {
        ref_keys::KEY_JCJI_VOTE_FOR
    } else {
        ref_keys::KEY_JCJI_VOTE_AGAINST
    };
    for (invite_id, cid, juror_user_id, _) in pending_invites_for_active_cases(pool).await {
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
}

async fn drive_case_to_resolution(pool: &MySqlPool, case_id: u64) {
    for _ in 0..16 {
        run_nomination_until_quiescent(pool, 32).await;
        vote_all_pending_for_case(pool, case_id, true).await;

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

#[tokio::test]
async fn membership_review_upheld_creates_verified_member_of() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let seed = seed_light(pool).await;
    let (community_id, applicant_id, acting_user_id) =
        find_membership_review_fixture(pool, &seed).await;

    let case_id = membership_review::file_application(
        pool,
        acting_user_id,
        applicant_id,
        community_id,
        "I would like to join this community.",
        "<p>I would like to join this community.</p>",
        "<p>I would like to join this community.</p>",
    )
    .await
    .expect("file_application");

    drive_case_to_resolution(pool, case_id).await;

    let member_edge: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT ser.id
        FROM social_entity_relationship ser
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = ser.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` = ?
        INNER JOIN ref_key_values rk_status
            ON rk_status.id = ser.status_id
           AND rk_status.`group` = ?
           AND rk_status.`key` = ?
        WHERE ser.from_social_entity_id = ?
          AND ser.to_social_entity_id = ?
          AND ser.ended_at IS NULL
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_VERIFIED)
    .bind(applicant_id)
    .bind(community_id)
    .fetch_optional(pool)
    .await
    .expect("member_of");

    assert!(
        member_edge.is_some(),
        "upheld membership review should create verified member_of"
    );
}

#[tokio::test]
async fn find_pending_human_jury_invite_includes_membership_review() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let seed = seed_light(pool).await;
    let (community_id, applicant_id, acting_user_id) =
        find_membership_review_fixture(pool, &seed).await;

    let case_id = membership_review::file_application(
        pool,
        acting_user_id,
        applicant_id,
        community_id,
        "Statement for jury summons test.",
        "<p>Statement for jury summons test.</p>",
        "<p>Statement for jury summons test.</p>",
    )
    .await
    .expect("file_application");

    run_nomination_until_quiescent(pool, 32).await;

    let pending = pending_invites_for_active_cases(pool).await;
    let case_invites: Vec<_> = pending
        .iter()
        .filter(|(_, cid, _, _)| *cid == case_id)
        .collect();
    assert!(
        !case_invites.is_empty(),
        "expected juror invites for membership review case {case_id}"
    );

    let Some((_, _, Some(juror_user_id), _)) = case_invites.first().copied() else {
        panic!("invite missing juror_user_id");
    };

    let invite = judicial::find_pending_human_jury_invite_for_user(pool, *juror_user_id)
        .await
        .expect("find_pending")
        .expect("pending invite for juror");

    assert_eq!(invite.case_id, case_id);
    assert_eq!(
        invite.trigger_key,
        ref_keys::KEY_JUDICIAL_TRIGGER_MEMBERSHIP_REVIEW
    );
}

#[tokio::test]
async fn juror_can_decline_membership_review_invite() {
    let db = common::TestDb::new().await;
    let pool = &db.pool;
    let seed = seed_light(pool).await;
    let (community_id, applicant_id, acting_user_id) =
        find_membership_review_fixture(pool, &seed).await;

    let case_id = membership_review::file_application(
        pool,
        acting_user_id,
        applicant_id,
        community_id,
        "Decline path test.",
        "<p>Decline path test.</p>",
        "<p>Decline path test.</p>",
    )
    .await
    .expect("file_application");

    run_nomination_until_quiescent(pool, 32).await;

    let Some((invite_id, _, Some(juror_user_id), _)) = pending_invites_for_active_cases(pool)
        .await
        .into_iter()
        .find(|(_, cid, _, _)| *cid == case_id)
    else {
        panic!("no pending invite");
    };

    let outcome = judicial::decline_from_invite(pool, juror_user_id, invite_id)
        .await
        .expect("decline");
    assert_eq!(outcome, DeclineOutcome::Recorded);

    let status_key: String = sqlx::query_scalar(
        r#"
        SELECT rk.`key`
        FROM judicial_case_juror_invite jcji
        INNER JOIN ref_key_values rk ON rk.id = jcji.invite_status_id
        WHERE jcji.id = ?
        "#,
    )
    .bind(invite_id)
    .fetch_one(pool)
    .await
    .expect("invite status");

    assert_eq!(status_key, ref_keys::KEY_JCJI_DECLINED);
}
