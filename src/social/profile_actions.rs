//! Human connection invite actions from the private profile card.

use sqlx::MySqlPool;

use crate::error::{AppError, AppResult};
use crate::i18n;
use crate::ref_keys;
use crate::social::{blocks, connections};
use crate::status;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionInviteResponse {
    Accept,
    Decline,
    Block,
}

/// Pending human connection invite from `inviter` → `invitee` (friend/family/colleague, pending).
pub async fn find_pending_human_invite_rel_id(
    pool: &MySqlPool,
    inviter_entity_id: u64,
    invitee_entity_id: u64,
) -> Result<Option<u64>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT ser.id
        FROM social_entity_relationship ser
        INNER JOIN ref_key_values rk_type
          ON rk_type.id = ser.type_id
         AND rk_type.`group` = ?
         AND rk_type.`key` IN (?, ?, ?)
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
    .bind(ref_keys::KEY_RELATIONSHIP_FRIEND)
    .bind(ref_keys::KEY_RELATIONSHIP_FAMILY)
    .bind(ref_keys::KEY_RELATIONSHIP_COLLEAGUE)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_PENDING_VERIFICATION)
    .bind(inviter_entity_id)
    .bind(invitee_entity_id)
    .fetch_optional(pool)
    .await
}

pub async fn respond_to_connection_invite(
    pool: &MySqlPool,
    invitee_entity_id: u64,
    inviter_entity_id: u64,
    rel_id: u64,
    response: ConnectionInviteResponse,
    actor_user_id: u64,
) -> AppResult<()> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();

    match response {
        ConnectionInviteResponse::Accept => {
            if !connections::human_can_accept_new_peer(pool, invitee_entity_id, inviter_entity_id)
                .await
                .map_err(AppError::Db)?
            {
                return Err(AppError::BadRequest(
                    c.t(&loc, "ui.entity_private.connection_slots_full"),
                ));
            }
            if !connections::human_can_accept_new_peer(pool, inviter_entity_id, invitee_entity_id)
                .await
                .map_err(AppError::Db)?
            {
                return Err(AppError::BadRequest(
                    c.t(&loc, "ui.entity_private.connection_inviter_slots_full"),
                ));
            }
            status::transition_relationship(pool, rel_id, ref_keys::KEY_VERIFIED, Some(actor_user_id))
                .await?;
        }
        ConnectionInviteResponse::Decline => {
            status::end_relationship(pool, rel_id, actor_user_id).await?;
        }
        ConnectionInviteResponse::Block => {
            blocks::block_human(
                pool,
                invitee_entity_id,
                inviter_entity_id,
                actor_user_id,
            )
            .await
            .map_err(AppError::Db)?;
        }
    }
    Ok(())
}
