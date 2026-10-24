use rust_decimal::Decimal;
use sqlx::{MySqlConnection, MySqlPool};

use crate::ref_keys;

use super::case_access::community_context_for_case_conn;
use super::community_reparent;
use super::membership_review;
use super::triggers::trigger_requires_full_community_jury_pool;
use super::types::{JurorCandidate, JurorCandidateRanked, OpenCaseRow};
#[derive(Debug, Clone, Copy)]
enum CommunityJurorMinRating {
    Gte(Decimal),
    Gt(Decimal),
}

pub(crate) async fn juror_pool_for_case_conn(
    conn: &mut MySqlConnection,
    case: &OpenCaseRow,
    rating_basis_community_id: u64,
) -> Result<Vec<JurorCandidate>, sqlx::Error> {
    if case.trigger_key == ref_keys::KEY_JUDICIAL_TRIGGER_CASE_APPEAL {
        return Ok(Vec::new());
    }

    if case.trigger_key == ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_REPARENT {
        let Some(reparent) =
            community_reparent::fetch_reparent_for_case_conn(conn, case.id).await?
        else {
            return Ok(Vec::new());
        };
        if reparent.phase == "impacted_vote" {
            return community_reparent::impacted_subtree_juror_pool_conn(
                conn,
                case.originator_social_entity_id,
                case.originator_social_entity_id,
                case.target_social_entity_id,
                case.created_by,
                rating_basis_community_id,
            )
            .await;
        }
        use community_reparent::{juror_min_rating_fallback, juror_min_rating_primary};
        let primary = community_member_juror_pool_conn(
            conn,
            case.originator_social_entity_id,
            case.originator_social_entity_id,
            case.target_social_entity_id,
            case.created_by,
            Some(CommunityJurorMinRating::Gt(juror_min_rating_primary())),
            rating_basis_community_id,
        )
        .await?;
        let fallback = community_member_juror_pool_conn(
            conn,
            case.originator_social_entity_id,
            case.originator_social_entity_id,
            case.target_social_entity_id,
            case.created_by,
            Some(CommunityJurorMinRating::Gte(juror_min_rating_fallback())),
            rating_basis_community_id,
        )
        .await?;
        return Ok(if fallback.len() > primary.len() {
            fallback
        } else {
            primary
        });
    }

    if super::triggers::trigger_is_civic_dispute(&case.trigger_key)
        || super::triggers::trigger_is_emergency_safety(&case.trigger_key)
    {
        if let Some(post_id) = case.subject_post_id {
            return post_audience_juror_pool_conn(
                conn,
                post_id,
                case.originator_social_entity_id,
                case.target_social_entity_id,
                case.created_by,
            )
            .await;
        }
    }

    if let Some(community_id) = community_context_for_case_conn(conn, case).await? {
        if trigger_requires_full_community_jury_pool(&case.trigger_key) {
            let (primary_min, fallback_min) =
                if case.trigger_key == ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_PROFILE_AMENDMENT {
                    use crate::social::judicial_community_profile::{
                        juror_min_rating_fallback, juror_min_rating_primary,
                    };
                    (juror_min_rating_primary(), juror_min_rating_fallback())
                } else {
                    use crate::social::judicial_constitutional::{
                        juror_min_rating_fallback, juror_min_rating_primary,
                    };
                    (juror_min_rating_primary(), juror_min_rating_fallback())
                };
            let primary = community_member_juror_pool_conn(
                conn,
                community_id,
                case.originator_social_entity_id,
                case.target_social_entity_id,
                case.created_by,
                Some(CommunityJurorMinRating::Gt(primary_min)),
                rating_basis_community_id,
            )
            .await?;
            let fallback = community_member_juror_pool_conn(
                conn,
                community_id,
                case.originator_social_entity_id,
                case.target_social_entity_id,
                case.created_by,
                Some(CommunityJurorMinRating::Gte(fallback_min)),
                rating_basis_community_id,
            )
            .await?;
            return Ok(if fallback.len() > primary.len() {
                fallback
            } else {
                primary
            });
        }
        if case.trigger_key == ref_keys::KEY_JUDICIAL_TRIGGER_MEMBERSHIP_REVIEW {
            use crate::social::judicial::membership_review::{
                juror_min_rating_fallback, juror_min_rating_primary,
            };
            let primary = community_member_juror_pool_conn(
                conn,
                community_id,
                case.originator_social_entity_id,
                case.target_social_entity_id,
                case.created_by,
                Some(CommunityJurorMinRating::Gt(juror_min_rating_primary())),
                rating_basis_community_id,
            )
            .await?;
            let fallback = community_member_juror_pool_conn(
                conn,
                community_id,
                case.originator_social_entity_id,
                case.target_social_entity_id,
                case.created_by,
                Some(CommunityJurorMinRating::Gte(juror_min_rating_fallback())),
                rating_basis_community_id,
            )
            .await?;
            return Ok(if fallback.len() > primary.len() {
                fallback
            } else {
                primary
            });
        }
        return community_member_juror_pool_conn(
            conn,
            community_id,
            case.originator_social_entity_id,
            case.target_social_entity_id,
            case.created_by,
            None,
            rating_basis_community_id,
        )
        .await;
    }

    Ok(Vec::new())
}
async fn community_member_juror_pool_conn(
    conn: &mut MySqlConnection,
    community_id: u64,
    originator_id: u64,
    target_id: u64,
    filer_user_id: Option<u64>,
    min_rating: Option<CommunityJurorMinRating>,
    rating_basis_community_id: u64,
) -> Result<Vec<JurorCandidate>, sqlx::Error> {
    // Community cases use the **connection record** rating
    // (`social_entity_relationship.rating`) rather than the juror's global
    // `social_entity.rating`. New `member_of` edges default to 2.50 (neutral standing).
    let rating_clause = match min_rating {
        Some(CommunityJurorMinRating::Gte(_)) => "AND COALESCE(ser.rating, 2.50) >= ?",
        Some(CommunityJurorMinRating::Gt(_)) => "AND COALESCE(ser.rating, 2.50) > ?",
        None => "AND (? IS NULL OR TRUE)",
    };

    let sql = format!(
        r#"
        SELECT DISTINCT
            se.id AS juror_social_entity_id,
            se.created_by AS juror_user_id,
            COALESCE(ser.rating, 2.50) AS juror_rating_at_invite
        FROM social_entity_relationship ser
        INNER JOIN ref_key_values rk_rel_type
            ON rk_rel_type.id = ser.type_id
           AND rk_rel_type.`group` = ?
           AND rk_rel_type.`key` = ?
        INNER JOIN ref_key_values rk_rel_status
            ON rk_rel_status.id = ser.status_id
           AND rk_rel_status.`group` = ?
           AND rk_rel_status.`key` = ?
           AND rk_rel_status.is_active = 1
        INNER JOIN social_entity se
            ON se.id = ser.from_social_entity_id
        INNER JOIN social_human sh
            ON sh.social_entity_id = se.id
        INNER JOIN ref_key_values rk_entity_status
            ON rk_entity_status.id = COALESCE(se.entity_status_id, se.status_id)
           AND rk_entity_status.`key` = ?
        WHERE ser.to_social_entity_id = ?
          AND ser.ended_at IS NULL
          AND se.id NOT IN (?, ?)
          {rating_clause}
          AND (
              ? IS NULL
              OR (
                  (se.created_by IS NULL OR se.created_by <> ?)
                  AND NOT EXISTS (
                      SELECT 1
                      FROM user_social_entity_role ur_filer
                      WHERE ur_filer.user_id = ?
                        AND ur_filer.social_entity_id = se.id
                        AND ur_filer.ends_at IS NULL
                  )
              )
          )
        ORDER BY se.id ASC
        "#
    );

    let rating_bind = match min_rating {
        Some(CommunityJurorMinRating::Gte(v)) | Some(CommunityJurorMinRating::Gt(v)) => v,
        None => Decimal::ZERO,
    };

    let rows: Vec<(u64, Option<u64>, Decimal)> = sqlx::query_as(&sql)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
        .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
        .bind(ref_keys::GROUP_STATUS)
        .bind(ref_keys::KEY_VERIFIED)
        .bind(ref_keys::KEY_VERIFIED)
        .bind(community_id)
        .bind(originator_id)
        .bind(target_id)
        .bind(rating_bind)
        .bind(filer_user_id)
        .bind(filer_user_id)
        .bind(filer_user_id)
        .fetch_all(&mut *conn)
        .await?;

    Ok(rows
        .into_iter()
        .map(|(jid, uid, rating)| JurorCandidate {
            juror_social_entity_id: jid,
            juror_user_id: uid,
            juror_rating_at_invite: rating,
            rating_basis_id: rating_basis_community_id,
            rating_community_entity_id: Some(community_id),
        })
        .collect())
}

async fn post_audience_juror_pool_conn(
    conn: &mut MySqlConnection,
    post_id: u64,
    originator_id: u64,
    target_id: u64,
    filer_user_id: Option<u64>,
) -> Result<Vec<JurorCandidate>, sqlx::Error> {
    // Each sub-pool carries the rating that best reflects *how* the candidate
    // is connected to the case:
    //
    //   priority 1 — member of a community that the post was addressed to:
    //                use `social_entity_relationship.rating` (the community
    //                connection rating). This is a community case for that
    //                juror even though it's filed as a post dispute.
    //   priority 2 — directly named on the post's audience: use the global
    //                `social_entity.rating`. They were addressed personally,
    //                not through a community lens.
    //   priority 3 — friend or family of the post author: use the global
    //                `social_entity.rating`. Per product canon, "general
    //                items" (friends/family) use the core rating.
    //
    // When a single candidate appears in multiple sub-pools, the lowest
    // priority number wins (community-context rating beats personal-context
    // rating), and we resolve that tie in application code below so the SQL
    // stays portable.
    let rows: Vec<JurorCandidateRanked> = sqlx::query_as(
        r#"
        SELECT
            candidates.candidate_id AS juror_social_entity_id,
            se.created_by AS juror_user_id,
            candidates.candidate_rating AS juror_rating_at_invite,
            candidates.source_priority AS source_priority,
            (SELECT rk_rb.id
             FROM ref_key_values rk_rb
             WHERE rk_rb.`group` = ?
               AND rk_rb.`key` = CASE
                   WHEN candidates.source_priority = 1 THEN ?
                   ELSE ?
               END
               AND rk_rb.is_active = 1
             LIMIT 1) AS rating_basis_id,
            candidates.rating_community_entity_id AS rating_community_entity_id
        FROM (
            SELECT
                ser_member.from_social_entity_id AS candidate_id,
                COALESCE(ser_member.rating, 2.50) AS candidate_rating,
                1 AS source_priority,
                sc.social_entity_id AS rating_community_entity_id
            FROM post_audience_entity pae
            INNER JOIN social_community sc
                ON sc.social_entity_id = pae.audience_entity_id
            INNER JOIN social_entity_relationship ser_member
                ON ser_member.to_social_entity_id = sc.social_entity_id
               AND ser_member.ended_at IS NULL
            INNER JOIN ref_key_values rk_member_type
                ON rk_member_type.id = ser_member.type_id
               AND rk_member_type.`group` = ?
               AND rk_member_type.`key` = ?
            INNER JOIN ref_key_values rk_member_status
                ON rk_member_status.id = ser_member.status_id
               AND rk_member_status.`group` = ?
               AND rk_member_status.`key` = ?
               AND rk_member_status.is_active = 1
            WHERE pae.post_id = ?

            UNION ALL

            SELECT
                pae.audience_entity_id AS candidate_id,
                COALESCE(se_direct.rating, 0.00) AS candidate_rating,
                2 AS source_priority,
                NULL AS rating_community_entity_id
            FROM post_audience_entity pae
            INNER JOIN social_human sh_direct
                ON sh_direct.social_entity_id = pae.audience_entity_id
            INNER JOIN social_entity se_direct
                ON se_direct.id = pae.audience_entity_id
            WHERE pae.post_id = ?

            UNION ALL

            SELECT
                CASE
                    WHEN ser_scope.from_social_entity_id = p.entity_id THEN ser_scope.to_social_entity_id
                    ELSE ser_scope.from_social_entity_id
                END AS candidate_id,
                COALESCE(se_scope.rating, 0.00) AS candidate_rating,
                3 AS source_priority,
                NULL AS rating_community_entity_id
            FROM post p
            INNER JOIN post_audience_scope pas
                ON pas.post_id = p.id
               AND pas.scope_key IN ('friends', 'family')
            INNER JOIN social_entity_relationship ser_scope
                ON ser_scope.ended_at IS NULL
               AND (
                    ser_scope.from_social_entity_id = p.entity_id
                 OR ser_scope.to_social_entity_id = p.entity_id
               )
            INNER JOIN ref_key_values rk_scope_type
                ON rk_scope_type.id = ser_scope.type_id
               AND rk_scope_type.`group` = ?
               AND (
                    (pas.scope_key = 'friends' AND rk_scope_type.`key` = ?)
                 OR (pas.scope_key = 'family' AND rk_scope_type.`key` = ?)
               )
            INNER JOIN ref_key_values rk_scope_status
                ON rk_scope_status.id = ser_scope.status_id
               AND rk_scope_status.`group` = ?
               AND rk_scope_status.`key` = ?
               AND rk_scope_status.is_active = 1
            INNER JOIN social_entity se_scope
                ON se_scope.id = CASE
                    WHEN ser_scope.from_social_entity_id = p.entity_id THEN ser_scope.to_social_entity_id
                    ELSE ser_scope.from_social_entity_id
                END
            WHERE p.id = ?
        ) candidates
        INNER JOIN social_entity se
            ON se.id = candidates.candidate_id
        INNER JOIN social_human sh
            ON sh.social_entity_id = se.id
        INNER JOIN ref_key_values rk_entity_status
            ON rk_entity_status.id = COALESCE(se.entity_status_id, se.status_id)
           AND rk_entity_status.`key` = ?
        WHERE se.id NOT IN (?, ?)
          AND (
              ? IS NULL
              OR (
                  (se.created_by IS NULL OR se.created_by <> ?)
                  AND NOT EXISTS (
                      SELECT 1
                      FROM user_social_entity_role ur_filer
                      WHERE ur_filer.user_id = ?
                        AND ur_filer.social_entity_id = se.id
                        AND ur_filer.ends_at IS NULL
                  )
              )
          )
        ORDER BY candidates.source_priority ASC, candidates.candidate_id ASC
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_RATING_BASIS)
    .bind(ref_keys::KEY_JCJI_RB_COMMUNITY)
    .bind(ref_keys::KEY_JCJI_RB_GLOBAL)
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_VERIFIED)
    .bind(post_id)
    .bind(post_id)
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_FRIEND)
    .bind(ref_keys::KEY_RELATIONSHIP_FAMILY)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_VERIFIED)
    .bind(post_id)
    .bind(ref_keys::KEY_VERIFIED)
    .bind(originator_id)
    .bind(target_id)
    .bind(filer_user_id)
    .bind(filer_user_id)
    .bind(filer_user_id)
    .fetch_all(&mut *conn)
    .await?;

    Ok(dedupe_preferring_lowest_priority(rows))
}

/// Collapse duplicate candidates from a `UNION ALL` query, keeping the row
/// whose `source_priority` is lowest. Input is expected to be sorted by
/// `(source_priority ASC, candidate_id ASC)` so a single pass suffices.
pub(crate) fn dedupe_preferring_lowest_priority(rows: Vec<JurorCandidateRanked>) -> Vec<JurorCandidate> {
    let mut seen: std::collections::HashSet<u64> = std::collections::HashSet::new();
    let mut out: Vec<JurorCandidate> = Vec::with_capacity(rows.len());
    for r in rows {
        if seen.insert(r.juror_social_entity_id) {
            out.push(JurorCandidate {
                juror_social_entity_id: r.juror_social_entity_id,
                juror_user_id: r.juror_user_id,
                juror_rating_at_invite: r.juror_rating_at_invite,
                rating_basis_id: r.rating_basis_id,
                rating_community_entity_id: r.rating_community_entity_id,
            });
        }
    }
    out
}

