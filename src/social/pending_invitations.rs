//! Inbound pending social invitations for a verified user (home feed panel).

use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::MySqlPool;

use crate::ref_keys;
use crate::social::public_profile;

#[derive(Debug, Clone)]
pub struct PendingInvitation {
    pub relationship_id: u64,
    pub kind: PendingInvitationKind,
    pub counterparty_handle: String,
    pub counterparty_label: String,
    pub counterparty_type_key: String,
    pub counterparty_rating: f32,
    pub counterparty_status_key: Option<String>,
    pub counterparty_country_code: Option<String>,
    pub counterparty_avatar_path: Option<String>,
    pub relationship_type_key: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingInvitationKind {
    HumanConnection,
    CommunityMembership,
}

#[derive(Debug, FromRow)]
struct Row {
    relationship_id: u64,
    kind_key: String,
    counterparty_handle: String,
    counterparty_label: Option<String>,
    counterparty_type_key: String,
    counterparty_rating: Decimal,
    counterparty_status_key: Option<String>,
    counterparty_country_code: Option<String>,
    counterparty_avatar_path: Option<String>,
    relationship_type_key: Option<String>,
}

/// Oldest-first pending invitations addressed to the viewer's primary human entity.
pub async fn list_for_user(
    pool: &MySqlPool,
    viewer_user_id: u64,
) -> Result<Vec<PendingInvitation>, sqlx::Error> {
    let human_entity_id: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT se.id
        FROM social_entity se
        INNER JOIN social_human sh ON sh.social_entity_id = se.id
        WHERE se.created_by = ?
        ORDER BY se.id ASC
        LIMIT 1
        "#,
    )
    .bind(viewer_user_id)
    .fetch_optional(pool)
    .await?;

    let Some(human_entity_id) = human_entity_id else {
        return Ok(Vec::new());
    };

    let avatar = public_profile::sql_avatar_path_expr("counterparty");
    let sql = format!(
        r#"
        SELECT
            ser.id AS relationship_id,
            CASE
                WHEN rk_type.`key` = ? THEN 'community_membership'
                ELSE 'human_connection'
            END AS kind_key,
            counterparty.handle AS counterparty_handle,
            COALESCE(
                NULLIF(TRIM(sc.label), ''),
                NULLIF(TRIM(CONCAT_WS(' ', sh.given_name, sh.family_name)), ''),
                counterparty.handle
            ) AS counterparty_label,
            rk_et.`key` AS counterparty_type_key,
            COALESCE(counterparty.rating, 0.00) AS counterparty_rating,
            (
                SELECT r.`key`
                FROM ref_key_values r
                WHERE r.id = COALESCE(counterparty.entity_status_id, counterparty.status_id)
                LIMIT 1
            ) AS counterparty_status_key,
            loc.country_code AS counterparty_country_code,
            COALESCE(
                NULLIF(TRIM(cl_plogo.url_canonical), ''),
                NULLIF(TRIM(cl_snplogo.url_canonical), ''),
                ({avatar})
            ) AS counterparty_avatar_path,
            rk_type.`key` AS relationship_type_key
        FROM social_entity_relationship ser
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = ser.type_id
           AND rk_type.`group` = ?
        INNER JOIN ref_key_values rk_status
            ON rk_status.id = ser.status_id
           AND rk_status.`group` = ?
           AND rk_status.`key` = ?
        INNER JOIN social_entity counterparty
            ON counterparty.id = CASE
                WHEN rk_type.`key` = ? THEN ser.to_social_entity_id
                ELSE ser.from_social_entity_id
            END
        INNER JOIN ref_key_values rk_et
            ON rk_et.id = counterparty.type_id
           AND rk_et.`group` = ?
        LEFT JOIN locations loc ON loc.id = counterparty.location_id
        LEFT JOIN social_community sc ON sc.social_entity_id = counterparty.id
        LEFT JOIN social_human sh ON sh.social_entity_id = counterparty.id
        LEFT JOIN social_profit sp ON sp.social_entity_id = counterparty.id
        LEFT JOIN social_non_profit snp ON snp.social_entity_id = counterparty.id
        LEFT JOIN content_link cl_plogo ON cl_plogo.id = sp.logo_content_link_id
        LEFT JOIN content_link cl_snplogo ON cl_snplogo.id = snp.logo_content_link_id
        WHERE ser.ended_at IS NULL
          AND (
                (
                    rk_type.`key` = ?
                    AND ser.from_social_entity_id = ?
                )
                OR (
                    rk_type.`key` IN (?, ?, ?)
                    AND ser.to_social_entity_id = ?
                    AND EXISTS (
                        SELECT 1
                        FROM social_entity se_from
                        INNER JOIN ref_key_values rk_from_type
                            ON rk_from_type.id = se_from.type_id
                           AND rk_from_type.`group` = ?
                           AND rk_from_type.`key` = ?
                        WHERE se_from.id = ser.from_social_entity_id
                    )
                )
          )
        ORDER BY ser.id ASC
        "#,
        avatar = avatar,
    );

    let rows = sqlx::query_as::<_, Row>(&sql)
        .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
        .bind(ref_keys::GROUP_STATUS)
        .bind(ref_keys::KEY_PENDING_VERIFICATION)
        .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
        .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
        .bind(human_entity_id)
        .bind(ref_keys::KEY_RELATIONSHIP_FRIEND)
        .bind(ref_keys::KEY_RELATIONSHIP_FAMILY)
        .bind(ref_keys::KEY_RELATIONSHIP_COLLEAGUE)
        .bind(human_entity_id)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
        .bind(ref_keys::KEY_ENTITY_TYPE_HUMAN)
        .fetch_all(pool)
        .await?;

    Ok(rows
        .into_iter()
        .map(|r| PendingInvitation {
            relationship_id: r.relationship_id,
            kind: if r.kind_key == "community_membership" {
                PendingInvitationKind::CommunityMembership
            } else {
                PendingInvitationKind::HumanConnection
            },
            counterparty_handle: r.counterparty_handle.clone(),
            counterparty_label: r
                .counterparty_label
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| r.counterparty_handle),
            counterparty_type_key: r.counterparty_type_key,
            counterparty_rating: r.counterparty_rating.to_string().parse::<f32>().unwrap_or(0.0),
            counterparty_status_key: r.counterparty_status_key,
            counterparty_country_code: r
                .counterparty_country_code
                .map(|s| s.trim().to_ascii_uppercase())
                .filter(|s| !s.is_empty()),
            counterparty_avatar_path: r.counterparty_avatar_path,
            relationship_type_key: r.relationship_type_key,
        })
        .collect())
}

pub async fn pending_exists_for_user(pool: &MySqlPool, viewer_user_id: u64) -> Result<bool, sqlx::Error> {
    Ok(!list_for_user(pool, viewer_user_id).await?.is_empty())
}
