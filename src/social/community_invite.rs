//! Invite-only community membership invitations (`member_of`, pending).

use sqlx::MySqlPool;

use crate::ref_keys;
use crate::social::blocks;
use crate::social::public_profile;

use super::connections;
use super::ownership;
use super::relationship::{self, CreateRequest};
use super::scope;
use crate::error::{AppError, AppResult};
use crate::i18n;
use crate::ref_key_values;
use crate::status;

/// Join policies where a viewer may request membership from the community profile card.
pub fn community_allows_self_join(join_policy_key: Option<&str>) -> bool {
    matches!(
        join_policy_key,
        Some(ref_keys::KEY_JOIN_POLICY_OPEN) | Some(ref_keys::KEY_JOIN_POLICY_REVIEWED)
    )
}

/// Viewer may send membership invites when they hold an active role on an invite-only community.
pub async fn viewer_can_invite_members(
    pool: &MySqlPool,
    viewer_user_id: u64,
    community_entity_id: u64,
    join_policy_key: Option<&str>,
) -> Result<bool, sqlx::Error> {
    if !join_policy_key.is_some_and(|k| k == ref_keys::KEY_JOIN_POLICY_INVITE_ONLY) {
        return Ok(false);
    }
    ownership::user_has_active_role_for_entity(pool, viewer_user_id, community_entity_id).await
}

/// Create a pending `member_of` edge from `invitee_human_id` → `community_entity_id`.
pub async fn send_membership_invite(
    pool: &MySqlPool,
    acting_user_id: u64,
    community_entity_id: u64,
    invitee_human_id: u64,
) -> AppResult<()> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();

    if invitee_human_id == community_entity_id {
        return Err(AppError::BadRequest(
            c.t(&loc, "social_entity.community_invite_self"),
        ));
    }

    if !viewer_can_invite_members(
        pool,
        acting_user_id,
        community_entity_id,
        Some(ref_keys::KEY_JOIN_POLICY_INVITE_ONLY),
    )
    .await
    .map_err(AppError::Db)?
    {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.community_invite_unavailable"),
        ));
    }

    if human_already_linked_to_community(pool, invitee_human_id, community_entity_id).await? {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.community_invite_already_member"),
        ));
    }

    if !connections::human_can_accept_new_peer(pool, invitee_human_id, community_entity_id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.community_invite_target_full"),
        ));
    }

    let member_of_type_id = ref_key_values::lookup_active_id(
        pool,
        ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE,
        ref_keys::KEY_RELATIONSHIP_MEMBER_OF,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| {
        AppError::BadRequest(c.t(&loc, "ui.entity_private.community_invite_unconfigured"))
    })?;
    let status_pending = ref_key_values::lookup_active_id(
        pool,
        ref_keys::GROUP_STATUS,
        ref_keys::KEY_PENDING_VERIFICATION,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| {
        AppError::BadRequest(c.t(&loc, "ui.entity_private.pending_status_unconfigured"))
    })?;

    relationship::create(
        pool,
        CreateRequest {
            from_social_entity_id: invitee_human_id.to_string(),
            to_social_entity_id: community_entity_id.to_string(),
            type_id: member_of_type_id.to_string(),
            status_id: Some(status_pending.to_string()),
            started_at: None,
            ended_at: None,
        },
        acting_user_id,
    )
    .await?;

    scope::mark_relationship_endpoints_dirty(pool, invitee_human_id, community_entity_id)
        .await
        .map_err(AppError::Db)?;

    Ok(())
}

/// Self-serve join or membership request (`member_of`) for open / membership-review communities.
pub async fn request_membership(
    pool: &MySqlPool,
    acting_user_id: u64,
    community_entity_id: u64,
    join_policy_key: Option<&str>,
) -> AppResult<()> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();

    let policy = join_policy_key.unwrap_or(ref_keys::KEY_JOIN_POLICY_OPEN);
    if !community_allows_self_join(Some(policy)) {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.community_join_unavailable"),
        ));
    }

    let viewer_human_id: u64 = sqlx::query_scalar(
        r#"
        SELECT se.id
        FROM social_entity se
        INNER JOIN social_human sh ON sh.social_entity_id = se.id
        WHERE se.created_by = ?
        ORDER BY se.id ASC
        LIMIT 1
        "#,
    )
    .bind(acting_user_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| {
        AppError::BadRequest(c.t(&loc, "ui.entity_private.human_profile_required"))
    })?;

    if human_has_active_edge_to_community(pool, viewer_human_id, community_entity_id).await? {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.community_invite_already_member"),
        ));
    }

    if !connections::human_can_accept_new_peer(pool, viewer_human_id, community_entity_id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.community_invite_target_full"),
        ));
    }

    let member_of_type_id = ref_key_values::lookup_active_id(
        pool,
        ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE,
        ref_keys::KEY_RELATIONSHIP_MEMBER_OF,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| {
        AppError::BadRequest(c.t(&loc, "ui.entity_private.community_invite_unconfigured"))
    })?;

    if policy != ref_keys::KEY_JOIN_POLICY_OPEN {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.community_join_unavailable"),
        ));
    }
    let status_key = ref_keys::KEY_VERIFIED;
    let status_id = ref_key_values::lookup_active_id(
        pool,
        ref_keys::GROUP_STATUS,
        status_key,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| {
        AppError::BadRequest(c.t(&loc, "ui.entity_private.pending_status_unconfigured"))
    })?;

    relationship::create(
        pool,
        CreateRequest {
            from_social_entity_id: viewer_human_id.to_string(),
            to_social_entity_id: community_entity_id.to_string(),
            type_id: member_of_type_id.to_string(),
            status_id: Some(status_id.to_string()),
            started_at: None,
            ended_at: None,
        },
        acting_user_id,
    )
    .await?;

    scope::mark_relationship_endpoints_dirty(pool, viewer_human_id, community_entity_id)
        .await
        .map_err(AppError::Db)?;

    Ok(())
}

/// Cancel the viewer's pending self-serve `member_of` request to a community.
pub async fn cancel_pending_membership_request(
    pool: &MySqlPool,
    acting_user_id: u64,
    community_entity_id: u64,
) -> AppResult<()> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();

    let viewer_human_id = sqlx::query_scalar::<_, u64>(
        r#"
        SELECT se.id
        FROM social_entity se
        INNER JOIN social_human sh ON sh.social_entity_id = se.id
        WHERE se.created_by = ?
        ORDER BY se.id ASC
        LIMIT 1
        "#,
    )
    .bind(acting_user_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| {
        AppError::BadRequest(c.t(&loc, "ui.entity_private.human_profile_required"))
    })?;

    let rel_id: Option<u64> = sqlx::query_scalar(
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
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_PENDING_VERIFICATION)
    .bind(viewer_human_id)
    .bind(community_entity_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Db)?;

    let Some(rel_id) = rel_id else {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.connection_invite_unavailable"),
        ));
    };

    status::end_relationship(pool, rel_id, acting_user_id).await?;
    scope::mark_relationship_endpoints_dirty(pool, viewer_human_id, community_entity_id)
        .await
        .map_err(AppError::Db)?;
    Ok(())
}

/// Resolve a community `social_entity.id` from its handle (ASCII lowercase).
pub async fn community_entity_id_for_handle(
    pool: &MySqlPool,
    community_handle: &str,
) -> Result<Option<u64>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT se.id
        FROM social_entity se
        INNER JOIN social_community sc ON sc.social_entity_id = se.id
        WHERE se.handle = ?
        LIMIT 1
        "#,
    )
    .bind(community_handle)
    .fetch_optional(pool)
    .await
}

/// True when the human already has any active edge to the community (any relationship type or status).
pub async fn human_has_active_edge_to_community(
    pool: &MySqlPool,
    human_entity_id: u64,
    community_entity_id: u64,
) -> Result<bool, sqlx::Error> {
    let found: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT ser.id
        FROM social_entity_relationship ser
        WHERE ser.ended_at IS NULL
          AND (
              (ser.from_social_entity_id = ? AND ser.to_social_entity_id = ?)
              OR (ser.from_social_entity_id = ? AND ser.to_social_entity_id = ?)
          )
        LIMIT 1
        "#,
    )
    .bind(human_entity_id)
    .bind(community_entity_id)
    .bind(community_entity_id)
    .bind(human_entity_id)
    .fetch_optional(pool)
    .await?;
    Ok(found.is_some())
}

async fn human_already_linked_to_community(
    pool: &MySqlPool,
    human_entity_id: u64,
    community_entity_id: u64,
) -> Result<bool, sqlx::Error> {
    human_has_active_edge_to_community(pool, human_entity_id, community_entity_id).await
}

/// Humans matching `needle`, excluding anyone with any active relationship to `community_entity_id`.
pub async fn search_invitable_humans(
    pool: &MySqlPool,
    community_entity_id: u64,
    needle: &str,
    prefix_like: &str,
    broad_like: &str,
    viewer_human_entity_id: Option<u64>,
) -> Result<Vec<(u64, String, String, Option<String>, Option<String>, i64)>, sqlx::Error> {
    let alive_predicate = public_profile::sql_alive_entity_predicate("se");
    let block_sql = viewer_human_entity_id
        .map(|_| blocks::SQL_EXCLUDE_BLOCKED_PEERS)
        .unwrap_or("");
    let avatar_sql = public_profile::sql_avatar_path_expr("se");
    let sql = format!(
        r#"
        SELECT
            se.id,
            se.handle,
            rk_type.`key` AS type_key,
            COALESCE(
                NULLIF(TRIM(CONCAT_WS(' ', sh.given_name, sh.additional_name, sh.family_name)), ''),
                se.handle
            ) AS label,
            {avatar_sql} AS avatar_path,
            CAST(
                (
                    CASE WHEN se.handle = ? THEN 100 ELSE 0 END +
                    CASE WHEN se.handle LIKE ? THEN 60 ELSE 0 END +
                    CASE WHEN COALESCE(CONCAT_WS(' ', sh.given_name, sh.additional_name, sh.family_name), se.handle) LIKE ? THEN 30 ELSE 0 END
                ) AS SIGNED
            ) AS rank_score
        FROM social_entity se
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = se.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` = ?
        INNER JOIN social_human sh ON sh.social_entity_id = se.id
        WHERE
            (
                se.handle LIKE ?
                OR COALESCE(CONCAT_WS(' ', sh.given_name, sh.additional_name, sh.family_name), se.handle) LIKE ?
            )
            AND NOT EXISTS (
                SELECT 1
                FROM social_entity_relationship ser_ex
                WHERE ser_ex.ended_at IS NULL
                  AND (
                      (ser_ex.from_social_entity_id = se.id AND ser_ex.to_social_entity_id = ?)
                      OR (ser_ex.from_social_entity_id = ? AND ser_ex.to_social_entity_id = se.id)
                  )
            )
            AND {alive_predicate}
            {block_sql}
        ORDER BY
            rank_score DESC,
            CASE WHEN se.handle = ? THEN 0 ELSE 1 END,
            se.handle ASC
        LIMIT 16
        "#,
        alive_predicate = alive_predicate,
        block_sql = block_sql,
        avatar_sql = avatar_sql,
    );
    let mut q = sqlx::query_as::<_, (u64, String, String, Option<String>, Option<String>, i64)>(&sql)
    .bind(needle)
    .bind(prefix_like)
    .bind(prefix_like)
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(ref_keys::KEY_ENTITY_TYPE_HUMAN)
    .bind(prefix_like)
    .bind(broad_like)
    .bind(community_entity_id)
    .bind(community_entity_id)
    .bind(needle);
    if let Some(viewer_human_id) = viewer_human_entity_id {
        q = q
            .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
            .bind(ref_keys::KEY_RELATIONSHIP_FRIEND)
            .bind(ref_keys::KEY_RELATIONSHIP_FAMILY)
            .bind(ref_keys::KEY_RELATIONSHIP_COLLEAGUE)
            .bind(ref_keys::GROUP_STATUS)
            .bind(ref_keys::KEY_BLOCKED)
            .bind(viewer_human_id)
            .bind(viewer_human_id);
    }
    q.fetch_all(pool).await
}
