//! Server-rendered private profile data for verified sessions only — not exposed via `/api/public/...`.

use chrono::NaiveDate;
use serde::Serialize;
use sqlx::FromRow;
use sqlx::MySqlPool;

use crate::db::TimestampUtc;

use crate::ref_keys;
use crate::pricing::BillableService;
use crate::social::blocks;
use crate::social::community_constitution::{self, ConstitutionSection};
use crate::social::community_graph;
use crate::social::community_invite;
use crate::social::connections::{self, MAX_CONNECTION_SLOTS};
use crate::social::entity_profile_edit;
use crate::social::human_onboarding;
use crate::social::ownership;
use crate::social::public_profile;
use crate::social::user_faces;

/// Capability tokens mirrored in the private profile card (`data-profile-actions` JSON).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileAction {
    SendConnectionInvite,
    RespondConnectionInvite,
    CancelConnectionInvite,
    Disconnect,
    BlockHuman,
    LeaveCommunity,
    InviteCommunityMember,
    RespondCommunityInvite,
    FollowBrand,
    UnfollowBrand,
    GatheringRsvp,
    EditHumanProfile,
    EditEntityProfile,
    DeleteEntity,
    AvatarRedraw,
    ProposeConstitutionalAmendment,
    ProposeCommunityProfileAmendment,
    ProposeCommunityReparent,
    SendRecruitmentInvites,
}

impl PrivateEntityProfile {
    /// Allowed profile-card actions for the current viewer (server source of truth for UI + handlers).
    #[must_use]
    pub fn allowed_actions(&self) -> Vec<ProfileAction> {
        let mut out = Vec::new();
        if self.can_send_connection_invite {
            out.push(ProfileAction::SendConnectionInvite);
        }
        if self.can_respond_to_connection_invite {
            out.push(ProfileAction::RespondConnectionInvite);
        }
        if self.invite_already_sent {
            out.push(ProfileAction::CancelConnectionInvite);
        }
        if self.can_disconnect {
            out.push(ProfileAction::Disconnect);
        }
        if self.can_block_human {
            out.push(ProfileAction::BlockHuman);
        }
        if self.can_leave_community {
            out.push(ProfileAction::LeaveCommunity);
        }
        if self.can_invite_community_members {
            out.push(ProfileAction::InviteCommunityMember);
        }
        if self.can_respond_to_community_invite {
            out.push(ProfileAction::RespondCommunityInvite);
        }
        if self.can_follow_brand {
            out.push(ProfileAction::FollowBrand);
        }
        if self.is_following_brand {
            out.push(ProfileAction::UnfollowBrand);
        }
        if self.can_use_gathering_rsvp {
            out.push(ProfileAction::GatheringRsvp);
        }
        if self.human_profile_edit.is_some() {
            out.push(ProfileAction::EditHumanProfile);
        }
        if self.entity_profile_edit.is_some() {
            out.push(ProfileAction::EditEntityProfile);
            out.push(ProfileAction::DeleteEntity);
        }
        if self.is_self_profile && self.type_label_key == ref_keys::KEY_ENTITY_TYPE_HUMAN {
            out.push(ProfileAction::AvatarRedraw);
        }
        if self.can_propose_constitutional_amendment && !self.community_is_failing {
            out.push(ProfileAction::ProposeConstitutionalAmendment);
        }
        if self.can_propose_community_profile_amendment && !self.community_is_failing {
            out.push(ProfileAction::ProposeCommunityProfileAmendment);
        }
        if self.can_propose_community_reparent && !self.community_is_failing {
            out.push(ProfileAction::ProposeCommunityReparent);
        }
        if self.can_send_recruitment_invites {
            out.push(ProfileAction::SendRecruitmentInvites);
        }
        out
    }
}

#[derive(Debug, Clone)]
pub struct MemberCommunityStanding {
    pub handle: String,
    pub edge_rating: f32,
}

#[derive(Debug, Clone)]
pub struct PrivateEntityProfile {
    pub handle: String,
    pub type_label_key: String,
    pub entity_rating: f32,
    pub show_entity_rating: bool,
    pub display_name: Option<String>,
    pub country_code: Option<String>,
    pub joined_on: Option<NaiveDate>,
    /// Human profile: active `member_of` communities with edge standing (self-view only in UI).
    pub member_communities: Vec<MemberCommunityStanding>,
    pub admired_non_profit_handles: Vec<String>,
    pub admired_profit_handles: Vec<String>,
    /// Human profile: for-profit entities this user founded or owns (viewer-visible).
    pub owned_profit_handles: Vec<String>,
    /// Human profile: non-profit entities this user founded or owns.
    pub owned_non_profit_handles: Vec<String>,
    /// Human profile: gathering (event) entities this user founded or owns.
    pub owned_gathering_handles: Vec<String>,
    pub can_send_connection_invite: bool,
    pub invite_already_sent: bool,
    pub can_respond_to_connection_invite: bool,
    /// Active friend/family/colleague edge with status verified or flagged (either direction).
    pub can_disconnect: bool,
    /// Another human profile: viewer may block this person (pre-emptive or from connect UI).
    pub can_block_human: bool,
    /// Community profile: viewer's human has an active `member_of` edge to this community.
    pub can_leave_community: bool,
    /// Profit / non-profit: viewer can start a verified `follow` edge (human → entity).
    pub can_follow_brand: bool,
    /// Profit / non-profit: active `follow` from viewer’s human to this entity.
    pub is_following_brand: bool,
    /// Gathering: viewer may set RSVP (`going` / `not_going` / `interested` form values).
    pub can_use_gathering_rsvp: bool,
    /// Current RSVP form value when an active RSVP row exists.
    pub gathering_rsvp: Option<String>,
    /// Viewer has a human profile and fewer than [`MAX_CONNECTION_SLOTS`] distinct peers (same basis as the connection rail).
    pub viewer_has_connection_capacity: bool,
    pub location_display_label: Option<String>,
    /// Self human only: Google Place ID when location was validated via Places on profile save.
    pub location_google_place_id: Option<String>,
    /// Human profile: optional website from `profile_content_link_id`.
    pub website_url: Option<String>,
    pub avatar_path: Option<String>,
    /// Lifecycle key from `social_entity_status` / `status` ref group.
    pub entity_status_key: Option<String>,
    pub verified_member_count: Option<i64>,
    /// Community only: `social_join_policy` ref key (`open`, `reviewed`, …).
    pub community_join_policy_key: Option<String>,
    /// Profit / non-profit extension copy (from `social_profit` / `social_non_profit`).
    pub description: Option<String>,
    pub intention: Option<String>,
    /// Active `follow` edges from human entities (verified or flagged), for-profit / non-profit only.
    pub brand_base_follower_count: u64,
    /// Gathering: verified/flagged RSVP counts per response type; zero for other entity types.
    pub gathering_rsvp_accepted: u64,
    pub gathering_rsvp_not_going: u64,
    pub gathering_rsvp_interested: u64,
    /// Community only: merged constitution from this community and all ancestors (leaf-first).
    pub effective_constitution: Vec<ConstitutionSection>,
    /// Community only: verified member may file a constitutional amendment case.
    pub can_propose_constitutional_amendment: bool,
    pub can_propose_community_profile_amendment: bool,
    pub can_propose_community_reparent: bool,
    pub community_is_failing: bool,
    /// Human profile: `rating < 1` and not dead (visible in handle styling for everyone).
    pub human_is_failing: bool,
    /// Viewer is the account owner of this human profile.
    pub is_self_profile: bool,
    /// Owner-only warning before rating-zero removal.
    pub show_human_rating_warning: bool,
    /// Owner-only: login inactivity decay is active (21+ days without sign-in).
    pub show_human_inactivity_notice: bool,
    /// Nom cost shown on the avatar redraw affordance (`CouncilAvatarRefresh`).
    pub avatar_redraw_nom_cost: u16,
    /// Self human profile: viewer has enough Noms for a paid avatar redraw.
    pub avatar_redraw_can_afford: bool,
    /// Self human profile: current Noms balance (for preflight UX).
    pub avatar_redraw_noms_balance: u16,
    /// UTC date when inactivity penalties would reach rating zero (owner-only).
    pub human_estimated_deletion_display: Option<String>,
    /// Invite-only community: viewer holds an owner role and may invite humans.
    pub can_invite_community_members: bool,
    /// Pending `member_of` invitation to this community (viewer is invitee; invite-only).
    pub can_respond_to_community_invite: bool,
    /// Open community: viewer may self-join via the connect dropdown.
    pub can_request_community_membership: bool,
    /// Membership-review community: viewer may file a judicial application.
    pub can_apply_for_membership_review: bool,
    /// Open membership-review application (viewer is applicant).
    pub has_open_membership_review_application: bool,
    /// Closed application outcome when not admitted (`denied` | `no_jury`).
    pub membership_review_application_outcome: Option<String>,
    /// Self human profile: editable name/location snapshot for the profile edit panel.
    pub human_profile_edit: Option<human_onboarding::HumanProfileEditSnapshot>,
    /// Profit / non-profit / gathering owner: editable fields for the entity edit panel.
    pub entity_profile_edit: Option<entity_profile_edit::EntityProfileEditSnapshot>,
    /// Self profit / non-profit owner: may send platform recruitment invites (single or bulk).
    pub can_send_recruitment_invites: bool,
}

#[derive(Debug, FromRow)]
struct MemberCommunityRow {
    handle: String,
    edge_rating: rust_decimal::Decimal,
}

#[derive(Debug, FromRow)]
struct OwnedEntityRow {
    handle: String,
    type_key: String,
}

async fn load_human_owned_entity_handles(
    pool: &MySqlPool,
    user_id: u64,
) -> Result<(Vec<String>, Vec<String>, Vec<String>), sqlx::Error> {
    let alive = public_profile::sql_alive_entity_predicate("se");
    let rows: Vec<OwnedEntityRow> = sqlx::query_as(&format!(
        r#"
        SELECT DISTINCT se.handle, rk_type.`key` AS type_key
        FROM social_entity se
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = se.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` IN (?, ?, ?)
        LEFT JOIN user_social_entity_role ur
            ON ur.social_entity_id = se.id
           AND ur.user_id = ?
           AND (ur.ends_at IS NULL OR ur.ends_at >= UTC_DATE())
        LEFT JOIN ref_key_values rk_role
            ON rk_role.id = ur.role_id
           AND rk_role.`group` = 'user_social_entity_role'
        LEFT JOIN social_gathering sg ON sg.social_entity_id = se.id
        WHERE {alive}
          AND (
              se.created_by = ?
              OR (
                  ur.id IS NOT NULL
                  AND rk_role.`key` IN ('owner', 'admin')
              )
          )
          AND (
              rk_type.`key` <> ?
              OR sg.ends_at IS NULL
              OR sg.ends_at >= UTC_TIMESTAMP(6)
              OR se.created_by = ?
          )
        ORDER BY se.handle ASC
        "#,
    ))
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(ref_keys::KEY_ENTITY_TYPE_PROFIT)
    .bind(ref_keys::KEY_ENTITY_TYPE_NON_PROFIT)
    .bind(ref_keys::KEY_ENTITY_TYPE_GATHERING)
    .bind(user_id)
    .bind(user_id)
    .bind(ref_keys::KEY_ENTITY_TYPE_GATHERING)
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut profit = Vec::new();
    let mut non_profit = Vec::new();
    let mut gathering = Vec::new();
    for row in rows {
        match row.type_key.as_str() {
            ref_keys::KEY_ENTITY_TYPE_PROFIT => profit.push(row.handle),
            ref_keys::KEY_ENTITY_TYPE_NON_PROFIT => non_profit.push(row.handle),
            ref_keys::KEY_ENTITY_TYPE_GATHERING => gathering.push(row.handle),
            _ => {}
        }
    }
    Ok((profit, non_profit, gathering))
}

#[derive(Debug, FromRow)]
struct Row {
    handle: String,
    type_key: String,
    entity_rating: rust_decimal::Decimal,
    display_name: Option<String>,
    country_code: Option<String>,
    entity_id: u64,
    created_by_user_id: u64,
    location_region: Option<String>,
    location_locality: Option<String>,
    display_label: Option<String>,
    location_validation_ref: Option<String>,
    location_validation_source: Option<String>,
    website_url: Option<String>,
    avatar_path: Option<String>,
    entity_status_key: Option<String>,
    joined_on: Option<NaiveDate>,
    verified_member_count: Option<i64>,
    community_join_policy_key: Option<String>,
    extension_description: Option<String>,
    extension_intention: Option<String>,
    brand_base_follower_count: i64,
    gathering_rsvp_accepted: i64,
    gathering_rsvp_not_going: i64,
    gathering_rsvp_interested: i64,
}

/// Load full entity row for private viewing (any active entity with this handle). No public-status filter.
pub async fn fetch_by_handle(
    pool: &MySqlPool,
    viewer_user_id: u64,
    handle: &str,
) -> Result<Option<PrivateEntityProfile>, sqlx::Error> {
    let conceal_panoply =
        crate::social::panoply::conceal_panoply_from_viewer(pool, viewer_user_id).await?;
    if conceal_panoply && crate::social::panoply::is_panoply_community_handle(handle) {
        return Ok(None);
    }

    let avatar = public_profile::sql_avatar_path_expr("se");
    // Verified/flagged `follow` humans to this entity, plus the owner's human profile (deduped).
    let brand_follow_count_sql = format!(
        r#"(SELECT COUNT(*) FROM (
            SELECT DISTINCT ser_fb.from_social_entity_id AS entity_id
            FROM social_entity_relationship ser_fb
            INNER JOIN ref_key_values rk_fb_type
                ON rk_fb_type.id = ser_fb.type_id
               AND rk_fb_type.`group` = '{g_rel}'
               AND rk_fb_type.`key` = '{k_follow}'
            INNER JOIN ref_key_values rk_fb_status
                ON rk_fb_status.id = ser_fb.status_id
               AND rk_fb_status.`group` = '{g_status}'
               AND rk_fb_status.is_active = 1
               AND rk_fb_status.`key` IN ('{v}', '{f}')
            WHERE ser_fb.to_social_entity_id = se.id
              AND ser_fb.ended_at IS NULL
            UNION
            SELECT oh.id AS entity_id
            FROM (
                SELECT se_owner.id
                FROM social_entity se_owner
                INNER JOIN social_human sh_owner ON sh_owner.social_entity_id = se_owner.id
                WHERE se_owner.created_by = se.created_by
                ORDER BY se_owner.id ASC
                LIMIT 1
            ) oh
        ) AS brand_base_members) AS brand_base_follower_count"#,
        g_rel = ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE,
        k_follow = ref_keys::KEY_RELATIONSHIP_FOLLOW,
        g_status = ref_keys::GROUP_STATUS,
        v = ref_keys::KEY_VERIFIED,
        f = ref_keys::KEY_FLAGGED,
    );
    let gathering_rsvp_count_sql = |rel_key: &str, alias: &str| {
        format!(
            r#"(SELECT COUNT(*)
            FROM social_entity_relationship ser_rsvp
            INNER JOIN ref_key_values rk_rsvp_type
                ON rk_rsvp_type.id = ser_rsvp.type_id
               AND rk_rsvp_type.`group` = '{g_rel}'
               AND rk_rsvp_type.`key` = '{rel_key}'
            INNER JOIN ref_key_values rk_rsvp_status
                ON rk_rsvp_status.id = ser_rsvp.status_id
               AND rk_rsvp_status.`group` = '{g_status}'
               AND rk_rsvp_status.is_active = 1
               AND rk_rsvp_status.`key` IN ('{v}', '{f}')
            WHERE ser_rsvp.to_social_entity_id = se.id
              AND ser_rsvp.ended_at IS NULL
        ) AS {alias}"#,
            g_rel = ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE,
            rel_key = rel_key,
            g_status = ref_keys::GROUP_STATUS,
            v = ref_keys::KEY_VERIFIED,
            f = ref_keys::KEY_FLAGGED,
            alias = alias,
        )
    };
    let gathering_rsvp_accepted_sql =
        gathering_rsvp_count_sql(ref_keys::KEY_RELATIONSHIP_EVENT_GOING, "gathering_rsvp_accepted");
    let gathering_rsvp_not_going_sql =
        gathering_rsvp_count_sql(ref_keys::KEY_RELATIONSHIP_EVENT_NOT_GOING, "gathering_rsvp_not_going");
    let gathering_rsvp_interested_sql =
        gathering_rsvp_count_sql(ref_keys::KEY_RELATIONSHIP_EVENT_INTERESTED, "gathering_rsvp_interested");
    let q = format!(
        r#"
        SELECT
            se.handle AS handle,
            rk_type.`key` AS type_key,
            COALESCE(se.rating, 0.00) AS entity_rating,
            COALESCE(
                NULLIF(TRIM(sp.label), ''),
                NULLIF(TRIM(snp.label), ''),
                NULLIF(TRIM(sc.label), ''),
                NULLIF(TRIM(sg.label), ''),
                NULLIF(TRIM(sd.label), ''),
                NULLIF(TRIM(CONCAT_WS(' ', sh.given_name, sh.additional_name, sh.family_name)), '')
            ) AS display_name,
            loc.country_code AS country_code,
            se.id AS entity_id,
            se.created_by AS created_by_user_id,
            loc.region AS location_region,
            loc.locality AS location_locality,
            loc.display_label AS display_label,
            loc.validation_ref AS location_validation_ref,
            loc.validation_source AS location_validation_source,
            NULLIF(TRIM(cl_profile.url_canonical), '') AS website_url,
            (
                SELECT r.`key`
                FROM ref_key_values r
                WHERE r.id = COALESCE(se.entity_status_id, se.status_id)
                LIMIT 1
            ) AS entity_status_key,
            COALESCE(
                NULLIF(TRIM(cl_plogo.url_canonical), ''),
                NULLIF(TRIM(cl_snplogo.url_canonical), ''),
                ({avatar})
            ) AS avatar_path,
            DATE(sh.created_at) AS joined_on,
            (
                SELECT COUNT(DISTINCT ser_m.from_social_entity_id)
                FROM social_entity_relationship ser_m
                INNER JOIN social_entity se_m
                    ON se_m.id = ser_m.from_social_entity_id
                INNER JOIN ref_key_values rk_m_type
                    ON rk_m_type.id = se_m.type_id
                   AND rk_m_type.`group` = 'social_entity_type'
                   AND rk_m_type.`key` = 'human'
                INNER JOIN ref_key_values rk_m_status
                    ON rk_m_status.id = COALESCE(se_m.entity_status_id, se_m.status_id)
                INNER JOIN ref_key_values rk_rel_type
                    ON rk_rel_type.id = ser_m.type_id
                   AND rk_rel_type.`group` = 'social_entity_relationship_type'
                   AND rk_rel_type.`key` = 'member_of'
                WHERE ser_m.to_social_entity_id = se.id
                  AND ser_m.ended_at IS NULL
                  AND rk_m_status.`key` = 'verified'
            ) AS verified_member_count,
            (
                SELECT rk_jp.`key`
                FROM social_community sc_jp
                INNER JOIN ref_key_values rk_jp
                    ON rk_jp.id = sc_jp.join_policy_id
                   AND rk_jp.`group` = 'social_join_policy'
                   AND rk_jp.is_active = 1
                WHERE sc_jp.social_entity_id = se.id
                LIMIT 1
            ) AS community_join_policy_key,
            NULLIF(TRIM(COALESCE(sp.description, snp.description, sc.description, sg.description, sd.description)), '') AS extension_description,
            NULLIF(TRIM(COALESCE(sp.intention, snp.intention, sc.intention, sg.intention, sd.intention)), '') AS extension_intention,
            {brand_follow_count_sql},
            {gathering_rsvp_accepted_sql},
            {gathering_rsvp_not_going_sql},
            {gathering_rsvp_interested_sql}
        FROM social_entity se
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = se.type_id
            AND rk_type.`group` = ?
            AND rk_type.is_active = 1
        LEFT JOIN locations loc ON loc.id = se.location_id
        LEFT JOIN social_profit sp ON sp.social_entity_id = se.id
        LEFT JOIN social_non_profit snp ON snp.social_entity_id = se.id
        LEFT JOIN social_community sc ON sc.social_entity_id = se.id
        LEFT JOIN social_gathering sg ON sg.social_entity_id = se.id
        LEFT JOIN social_digital sd ON sd.social_entity_id = se.id
        LEFT JOIN content_link cl_plogo ON cl_plogo.id = sp.logo_content_link_id
        LEFT JOIN content_link cl_snplogo ON cl_snplogo.id = snp.logo_content_link_id
        LEFT JOIN social_human sh ON sh.social_entity_id = se.id
        LEFT JOIN content_link cl_profile ON cl_profile.id = se.profile_content_link_id
        WHERE se.handle = ?
        LIMIT 1
        "#,
        avatar = avatar,
        brand_follow_count_sql = brand_follow_count_sql,
        gathering_rsvp_accepted_sql = gathering_rsvp_accepted_sql,
        gathering_rsvp_not_going_sql = gathering_rsvp_not_going_sql,
        gathering_rsvp_interested_sql = gathering_rsvp_interested_sql
    );

    let row = sqlx::query_as::<_, Row>(&q)
        .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
        .bind(handle)
        .fetch_optional(pool)
        .await?;

    let Some(r) = row else {
        return Ok(None);
    };
    let (member_communities, admired_non_profit_handles, admired_profit_handles, owned_profit_handles, owned_non_profit_handles, owned_gathering_handles) =
        if r.type_key == ref_keys::KEY_ENTITY_TYPE_HUMAN {
            let member_rows: Vec<MemberCommunityRow> = sqlx::query_as(
                r#"
                SELECT se_peer.handle, ser.rating AS edge_rating
                FROM social_entity_relationship ser
                INNER JOIN ref_key_values rk_rel_type
                    ON rk_rel_type.id = ser.type_id
                   AND rk_rel_type.`group` = ?
                   AND rk_rel_type.`key` = ?
                INNER JOIN ref_key_values rk_status
                    ON rk_status.id = ser.status_id
                   AND rk_status.`group` = ?
                   AND rk_status.`key` IN (?, ?, ?)
                INNER JOIN social_entity se_peer ON se_peer.id = ser.to_social_entity_id
                INNER JOIN ref_key_values rk_peer_type
                    ON rk_peer_type.id = se_peer.type_id
                   AND rk_peer_type.`group` = ?
                   AND rk_peer_type.`key` = ?
                WHERE ser.from_social_entity_id = ?
                  AND ser.ended_at IS NULL
                ORDER BY se_peer.handle ASC
                "#,
            )
            .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
            .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
            .bind(ref_keys::GROUP_STATUS)
            .bind(ref_keys::KEY_PENDING_VERIFICATION)
            .bind(ref_keys::KEY_VERIFIED)
            .bind(ref_keys::KEY_FLAGGED)
            .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
            .bind(ref_keys::KEY_ENTITY_TYPE_COMMUNITY)
            .bind(r.entity_id)
            .fetch_all(pool)
            .await?;
            let mut member_communities: Vec<MemberCommunityStanding> = member_rows
                .into_iter()
                .map(|row| MemberCommunityStanding {
                    handle: row.handle,
                    edge_rating: row.edge_rating.to_string().parse::<f32>().unwrap_or(0.0),
                })
                .collect();
            if conceal_panoply {
                member_communities.retain(|m| {
                    !crate::social::panoply::is_panoply_community_handle(&m.handle)
                });
            }

            let admired_non_profit_handles: Vec<String> = sqlx::query_scalar(
                r#"
                SELECT DISTINCT se_peer.handle
                FROM social_entity_relationship ser
                INNER JOIN ref_key_values rk_rel_type
                    ON rk_rel_type.id = ser.type_id
                   AND rk_rel_type.`group` = ?
                   AND rk_rel_type.`key` = ?
                INNER JOIN ref_key_values rk_status
                    ON rk_status.id = ser.status_id
                   AND rk_status.`group` = ?
                   AND rk_status.`key` IN (?, ?, ?)
                INNER JOIN social_entity se_peer
                    ON se_peer.id = ser.to_social_entity_id
                INNER JOIN ref_key_values rk_peer_type
                    ON rk_peer_type.id = se_peer.type_id
                   AND rk_peer_type.`group` = ?
                   AND rk_peer_type.`key` = ?
                WHERE ser.from_social_entity_id = ?
                  AND ser.ended_at IS NULL
                ORDER BY se_peer.handle ASC
                "#,
            )
            .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
            .bind(ref_keys::KEY_RELATIONSHIP_FOLLOW)
            .bind(ref_keys::GROUP_STATUS)
            .bind(ref_keys::KEY_PENDING_VERIFICATION)
            .bind(ref_keys::KEY_VERIFIED)
            .bind(ref_keys::KEY_FLAGGED)
            .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
            .bind(ref_keys::KEY_ENTITY_TYPE_NON_PROFIT)
            .bind(r.entity_id)
            .fetch_all(pool)
            .await?;

            let admired_profit_handles: Vec<String> = sqlx::query_scalar(
                r#"
                SELECT DISTINCT se_peer.handle
                FROM social_entity_relationship ser
                INNER JOIN ref_key_values rk_rel_type
                    ON rk_rel_type.id = ser.type_id
                   AND rk_rel_type.`group` = ?
                   AND rk_rel_type.`key` = ?
                INNER JOIN ref_key_values rk_status
                    ON rk_status.id = ser.status_id
                   AND rk_status.`group` = ?
                   AND rk_status.`key` IN (?, ?, ?)
                INNER JOIN social_entity se_peer
                    ON se_peer.id = ser.to_social_entity_id
                INNER JOIN ref_key_values rk_peer_type
                    ON rk_peer_type.id = se_peer.type_id
                   AND rk_peer_type.`group` = ?
                   AND rk_peer_type.`key` = ?
                WHERE ser.from_social_entity_id = ?
                  AND ser.ended_at IS NULL
                ORDER BY se_peer.handle ASC
                "#,
            )
            .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
            .bind(ref_keys::KEY_RELATIONSHIP_FOLLOW)
            .bind(ref_keys::GROUP_STATUS)
            .bind(ref_keys::KEY_PENDING_VERIFICATION)
            .bind(ref_keys::KEY_VERIFIED)
            .bind(ref_keys::KEY_FLAGGED)
            .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
            .bind(ref_keys::KEY_ENTITY_TYPE_PROFIT)
            .bind(r.entity_id)
            .fetch_all(pool)
            .await?;

            let (owned_profit_handles, owned_non_profit_handles, owned_gathering_handles) =
                load_human_owned_entity_handles(pool, r.created_by_user_id).await?;

            (
                member_communities,
                admired_non_profit_handles,
                admired_profit_handles,
                owned_profit_handles,
                owned_non_profit_handles,
                owned_gathering_handles,
            )
        } else {
            (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new())
        };

    let viewer_human_entity_id: Option<u64> = sqlx::query_scalar(
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

    let target_is_human = r.type_key == ref_keys::KEY_ENTITY_TYPE_HUMAN;
    let extension_manage_target = matches!(
        r.type_key.as_str(),
        ref_keys::KEY_ENTITY_TYPE_PROFIT
            | ref_keys::KEY_ENTITY_TYPE_NON_PROFIT
            | ref_keys::KEY_ENTITY_TYPE_GATHERING
    );
    let can_manage_entity = if extension_manage_target {
        r.created_by_user_id == viewer_user_id
            || ownership::user_has_active_role_for_entity(pool, viewer_user_id, r.entity_id)
                .await
                .unwrap_or(false)
    } else {
        r.created_by_user_id == viewer_user_id
    };
    let is_self = if extension_manage_target {
        can_manage_entity
    } else {
        r.created_by_user_id == viewer_user_id
    };

    if target_is_human && !is_self {
        if let Some(viewer_human_id) = viewer_human_entity_id {
            if blocks::is_pair_blocked(pool, viewer_human_id, r.entity_id).await? {
                return Ok(None);
            }
        }
    }

    // Surface `social_entity.rating` on avatars for org/community/event types; humans see stars on self-view only.
    let show_entity_rating = matches!(
        r.type_key.as_str(),
        ref_keys::KEY_ENTITY_TYPE_NON_PROFIT
            | ref_keys::KEY_ENTITY_TYPE_PROFIT
            | ref_keys::KEY_ENTITY_TYPE_COMMUNITY
            | ref_keys::KEY_ENTITY_TYPE_GATHERING
    ) || is_self;
    let mut can_send_connection_invite = false;
    let mut invite_already_sent = false;
    let mut can_respond_to_connection_invite = false;
    let mut can_disconnect = false;
    let viewer_peer_count: i64 = if let Some(vid) = viewer_human_entity_id {
        connections::distinct_peer_count_for_human_entity(pool, vid).await?
    } else {
        0
    };
    let viewer_has_connection_capacity = viewer_human_entity_id.is_some()
        && (viewer_peer_count as usize) < MAX_CONNECTION_SLOTS;
    if target_is_human && !is_self {
        if let Some(viewer_human_id) = viewer_human_entity_id {
            let viewer_active_connections = viewer_peer_count;

            let target_peer_count =
                connections::distinct_peer_count_for_human_entity(pool, r.entity_id).await?;
            let target_has_connection_capacity =
                (target_peer_count as usize) < MAX_CONNECTION_SLOTS;

            let outbound_pending_connection: Option<u64> = sqlx::query_scalar(
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
                WHERE (
                    (ser.from_social_entity_id = ? AND ser.to_social_entity_id = ?)
                )
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
            .bind(viewer_human_id)
            .bind(r.entity_id)
            .fetch_optional(pool)
            .await?;
            let inbound_pending_connection: Option<u64> = sqlx::query_scalar(
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
                WHERE (
                    (ser.from_social_entity_id = ? AND ser.to_social_entity_id = ?)
                )
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
            .bind(r.entity_id)
            .bind(viewer_human_id)
            .fetch_optional(pool)
            .await?;
            let existing_active_connection: Option<u64> = sqlx::query_scalar(
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
                   AND rk_status.`key` IN (?, ?)
                WHERE (
                    (ser.from_social_entity_id = ? AND ser.to_social_entity_id = ?)
                    OR (ser.from_social_entity_id = ? AND ser.to_social_entity_id = ?)
                )
                  AND ser.ended_at IS NULL
                LIMIT 1
                "#,
            )
            .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
            .bind(ref_keys::KEY_RELATIONSHIP_FRIEND)
            .bind(ref_keys::KEY_RELATIONSHIP_FAMILY)
            .bind(ref_keys::KEY_RELATIONSHIP_COLLEAGUE)
            .bind(ref_keys::GROUP_STATUS)
            .bind(ref_keys::KEY_VERIFIED)
            .bind(ref_keys::KEY_FLAGGED)
            .bind(viewer_human_id)
            .bind(r.entity_id)
            .bind(r.entity_id)
            .bind(viewer_human_id)
            .fetch_optional(pool)
            .await?;

            invite_already_sent = outbound_pending_connection.is_some();
            can_respond_to_connection_invite = inbound_pending_connection.is_some();
            can_disconnect = existing_active_connection.is_some();
            can_send_connection_invite = !invite_already_sent
                && !can_respond_to_connection_invite
                && existing_active_connection.is_none()
                && (viewer_active_connections as usize) < MAX_CONNECTION_SLOTS
                && target_has_connection_capacity;
        }
    }

    let mut can_follow_brand = false;
    let mut is_following_brand = false;
    let mut can_use_gathering_rsvp = false;
    let mut gathering_rsvp: Option<String> = None;
    let mut can_leave_community = false;
    let mut can_propose_constitutional_amendment = false;
    let mut can_propose_community_profile_amendment = false;
    let mut can_propose_community_reparent = false;
    let mut community_is_failing = false;
    let mut human_is_failing = false;
    let mut can_invite_community_members = false;
    let mut can_respond_to_community_invite = false;
    let mut can_request_community_membership = false;
    let mut can_apply_for_membership_review = false;
    let mut has_open_membership_review_application = false;
    let mut membership_review_application_outcome: Option<String> = None;

    let brand_follow_target = matches!(
        r.type_key.as_str(),
        ref_keys::KEY_ENTITY_TYPE_PROFIT | ref_keys::KEY_ENTITY_TYPE_NON_PROFIT
    );
    let gathering_target = r.type_key == ref_keys::KEY_ENTITY_TYPE_GATHERING;

    if let Some(viewer_human_id) = viewer_human_entity_id {
        if brand_follow_target && !is_self {
            let active_follow: Option<u64> = sqlx::query_scalar(
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
                   AND rk_status.`key` IN (?, ?)
                WHERE ser.from_social_entity_id = ?
                  AND ser.to_social_entity_id = ?
                  AND ser.ended_at IS NULL
                LIMIT 1
                "#,
            )
            .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
            .bind(ref_keys::KEY_RELATIONSHIP_FOLLOW)
            .bind(ref_keys::GROUP_STATUS)
            .bind(ref_keys::KEY_VERIFIED)
            .bind(ref_keys::KEY_FLAGGED)
            .bind(viewer_human_id)
            .bind(r.entity_id)
            .fetch_optional(pool)
            .await?;
            is_following_brand = active_follow.is_some();
            can_follow_brand = !is_following_brand && viewer_has_connection_capacity;
        }

        if gathering_target && !is_self {
            let rsvp_key: Option<String> = sqlx::query_scalar(
                r#"
                SELECT rk.`key`
                FROM social_entity_relationship ser
                INNER JOIN ref_key_values rk
                    ON rk.id = ser.type_id
                   AND rk.`group` = ?
                   AND rk.`key` IN (?, ?, ?)
                INNER JOIN ref_key_values rk_status
                    ON rk_status.id = ser.status_id
                   AND rk_status.`group` = ?
                   AND rk_status.`key` IN (?, ?, ?)
                WHERE ser.from_social_entity_id = ?
                  AND ser.to_social_entity_id = ?
                  AND ser.ended_at IS NULL
                ORDER BY ser.id DESC
                LIMIT 1
                "#,
            )
            .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
            .bind(ref_keys::KEY_RELATIONSHIP_EVENT_GOING)
            .bind(ref_keys::KEY_RELATIONSHIP_EVENT_NOT_GOING)
            .bind(ref_keys::KEY_RELATIONSHIP_EVENT_INTERESTED)
            .bind(ref_keys::GROUP_STATUS)
            .bind(ref_keys::KEY_PENDING_VERIFICATION)
            .bind(ref_keys::KEY_VERIFIED)
            .bind(ref_keys::KEY_FLAGGED)
            .bind(viewer_human_id)
            .bind(r.entity_id)
            .fetch_optional(pool)
            .await?;

            gathering_rsvp = rsvp_key.as_deref().and_then(|k| match k {
                ref_keys::KEY_RELATIONSHIP_EVENT_GOING => Some("going".to_string()),
                ref_keys::KEY_RELATIONSHIP_EVENT_NOT_GOING => Some("not_going".to_string()),
                ref_keys::KEY_RELATIONSHIP_EVENT_INTERESTED => Some("interested".to_string()),
                _ => None,
            });
            can_use_gathering_rsvp = gathering_rsvp.is_some() || viewer_has_connection_capacity;
        }

        if r.type_key == ref_keys::KEY_ENTITY_TYPE_COMMUNITY {
            can_invite_community_members = community_invite::viewer_can_invite_members(
                pool,
                viewer_user_id,
                r.entity_id,
                r.community_join_policy_key.as_deref(),
            )
            .await?;

            let pending_membership: Option<u64> = sqlx::query_scalar(
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
            .bind(r.entity_id)
            .fetch_optional(pool)
            .await?;
            let join_policy = r.community_join_policy_key.as_deref();
            let invite_only = join_policy == Some(ref_keys::KEY_JOIN_POLICY_INVITE_ONLY);
            let is_open_policy = join_policy == Some(ref_keys::KEY_JOIN_POLICY_OPEN);
            let is_reviewed_policy = join_policy == Some(ref_keys::KEY_JOIN_POLICY_REVIEWED);
            can_respond_to_community_invite =
                pending_membership.is_some() && invite_only;
            invite_already_sent = pending_membership.is_some() && is_open_policy;

            if is_reviewed_policy && !is_self {
                if let Some(viewer_human_id) = viewer_human_entity_id {
                    has_open_membership_review_application = crate::social::judicial::membership_review::open_application_case_for_applicant(
                        pool,
                        viewer_human_id,
                        r.entity_id,
                    )
                    .await?
                    .is_some();
                }
            }

            let active_membership: Option<u64> = sqlx::query_scalar(
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
                   AND rk_status.`key` IN (?, ?, ?)
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
            .bind(ref_keys::KEY_VERIFIED)
            .bind(ref_keys::KEY_FLAGGED)
            .bind(viewer_human_id)
            .bind(r.entity_id)
            .fetch_optional(pool)
            .await?;
            can_leave_community = active_membership.is_some();
            let membership_blocked = active_membership.is_some() || pending_membership.is_some();
            if let Some(viewer_human_id) = viewer_human_entity_id {
                if is_reviewed_policy && !is_self && !membership_blocked {
                    let barred = crate::social::judicial::membership_review::is_reapply_barred(
                        pool,
                        viewer_human_id,
                        r.entity_id,
                    )
                    .await?;
                    can_apply_for_membership_review = !has_open_membership_review_application
                        && !barred
                        && viewer_has_connection_capacity;
                }
            }
            can_request_community_membership = !is_self
                && is_open_policy
                && membership_blocked == false
                && viewer_human_entity_id.is_some()
                && viewer_has_connection_capacity;

            if let Some(viewer_human_id) = viewer_human_entity_id {
                if !has_open_membership_review_application && active_membership.is_none() {
                    if let Some(outcome) =
                        crate::social::judicial::membership_review::applicant_outcome_for_community(
                            pool,
                            viewer_human_id,
                            r.entity_id,
                        )
                        .await?
                    {
                        membership_review_application_outcome = Some(match outcome {
                            crate::social::judicial::MembershipReviewApplicantOutcome::Denied => {
                                "denied".to_string()
                            }
                            crate::social::judicial::MembershipReviewApplicantOutcome::NoJury => {
                                "no_jury".to_string()
                            }
                        });
                    }
                }
            }

            if active_membership.is_some() {
                let viewer_rating: Option<rust_decimal::Decimal> = sqlx::query_scalar(
                    r#"
                    SELECT ser.rating
                    FROM social_entity_relationship ser
                    INNER JOIN ref_key_values rk_type
                        ON rk_type.id = ser.type_id
                       AND rk_type.`group` = ?
                       AND rk_type.`key` = ?
                    INNER JOIN ref_key_values rk_status
                        ON rk_status.id = ser.status_id
                       AND rk_status.`group` = ?
                       AND rk_status.`key` IN (?, ?, ?)
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
                .bind(ref_keys::KEY_VERIFIED)
                .bind(ref_keys::KEY_FLAGGED)
                .bind(viewer_human_id)
                .bind(r.entity_id)
                .fetch_optional(pool)
                .await?;
                let quality_member = viewer_rating
                    .is_some_and(|rating| rating >= rust_decimal::Decimal::new(300, 2));
                can_propose_constitutional_amendment = quality_member;
                can_propose_community_profile_amendment = quality_member;
                let failing = crate::social::community_graph::is_community_failing(pool, r.entity_id)
                    .await
                    .unwrap_or(false);
                let dead = r.entity_status_key.as_deref()
                    == Some(ref_keys::KEY_ENTITY_STATUS_DEAD);
                community_is_failing = failing;
                can_propose_community_reparent = quality_member
                    && !failing
                    && !dead
                    && !crate::social::community_graph::is_death_immune(pool, r.entity_id)
                        .await
                        .unwrap_or(true);
            }
        }
    }

    let trim_nonempty = |s: Option<String>| {
        s.map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
    };

    let effective_constitution = if r.type_key == ref_keys::KEY_ENTITY_TYPE_COMMUNITY {
        community_constitution::fetch_effective_constitution(pool, r.entity_id).await?
    } else {
        Vec::new()
    };

    if target_is_human {
        human_is_failing = community_graph::is_human_failing(pool, r.entity_id)
            .await
            .unwrap_or(false);
    }
    let show_human_rating_warning =
        target_is_human && is_self && human_is_failing && !community_graph::is_human_death_immune(pool, r.entity_id).await.unwrap_or(true);

    let mut show_human_inactivity_notice = false;
    let mut human_estimated_deletion_display = None;
    if target_is_human
        && is_self
        && !community_graph::is_human_death_immune(pool, r.entity_id)
            .await
            .unwrap_or(true)
    {
        let inactivity: Option<(TimestampUtc, u32, i64)> = sqlx::query_as(
            r#"
            SELECT
                COALESCE(last_login_at, created_at),
                inactivity_penalties_applied,
                TIMESTAMPDIFF(
                    DAY,
                    COALESCE(last_login_at, created_at),
                    UTC_TIMESTAMP(6)
                )
            FROM users
            WHERE id = ?
            LIMIT 1
            "#,
        )
        .bind(viewer_user_id)
        .fetch_optional(pool)
        .await?;
        if let Some((last_login_at, penalties_applied, idle_days)) = inactivity {
            show_human_inactivity_notice = idle_days
                >= community_graph::INACTIVITY_FIRST_PENALTY_DAYS
                || penalties_applied > 0;
            if show_human_inactivity_notice {
                human_estimated_deletion_display =
                    community_graph::estimated_deletion_at_from_last_login(
                        last_login_at.naive_utc(),
                        r.entity_rating,
                    )
                    .map(community_graph::format_estimated_deletion_date);
            }
        }
    }

    let avatar_redraw_nom_cost = BillableService::CouncilAvatarRefresh.nom_cost();
    let avatar_redraw_noms_balance = if target_is_human && is_self {
        human_onboarding::ai_tokens_for_user(pool, viewer_user_id)
            .await
            .unwrap_or(0)
    } else {
        0
    };
    let avatar_redraw_can_afford = avatar_redraw_noms_balance >= avatar_redraw_nom_cost;

    let human_profile_edit = if target_is_human && is_self {
        match human_onboarding::fetch_human_profile_edit_snapshot(pool, viewer_user_id, r.entity_id)
            .await
        {
            Ok(snapshot) => Some(snapshot),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    entity_id = r.entity_id,
                    user_id = viewer_user_id,
                    "human profile edit snapshot unavailable"
                );
                None
            }
        }
    } else {
        None
    };

    let entity_is_alive = r.entity_status_key.as_deref() != Some(ref_keys::KEY_ENTITY_STATUS_DEAD);
    let can_send_recruitment_invites = extension_manage_target
        && can_manage_entity
        && entity_is_alive
        && matches!(
            r.type_key.as_str(),
            ref_keys::KEY_ENTITY_TYPE_PROFIT | ref_keys::KEY_ENTITY_TYPE_NON_PROFIT
        );
    let entity_profile_edit = if extension_manage_target && can_manage_entity && entity_is_alive {
        match entity_profile_edit::fetch_entity_profile_edit_snapshot(
            pool,
            viewer_user_id,
            r.entity_id,
        )
        .await
        {
            Ok(snapshot) => Some(snapshot),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    entity_id = r.entity_id,
                    user_id = viewer_user_id,
                    "entity profile edit snapshot unavailable"
                );
                None
            }
        }
    } else {
        None
    };

    let can_block_human = target_is_human && !is_self;

    let location_display_label = if r.type_key == ref_keys::KEY_ENTITY_TYPE_HUMAN {
        human_onboarding::human_profile_location_label(
            r.country_code.as_deref(),
            r.location_region.as_deref(),
            r.location_locality.as_deref(),
            r.display_label.as_deref(),
        )
    } else {
        trim_nonempty(r.display_label)
    };

    let location_google_place_id = if is_self {
        r.location_validation_source
            .as_deref()
            .map(str::trim)
            .filter(|src| crate::verification::is_google_maps_validation_source(src))
            .and(r.location_validation_ref.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| crate::google_maps::places::normalize_place_id(s))
    } else {
        None
    };

    Ok(Some(PrivateEntityProfile {
            handle: r.handle,
            type_label_key: r.type_key,
            entity_rating: r.entity_rating.to_string().parse::<f32>().unwrap_or(0.0),
            show_entity_rating,
            display_name: r
                .display_name
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            country_code: r
                .country_code
                .map(|s| s.trim().to_ascii_uppercase())
                .filter(|s| !s.is_empty()),
            joined_on: r.joined_on,
            member_communities,
            admired_non_profit_handles,
            admired_profit_handles,
            owned_profit_handles,
            owned_non_profit_handles,
            owned_gathering_handles,
            can_send_connection_invite,
            invite_already_sent,
            can_respond_to_connection_invite,
            can_disconnect,
            can_block_human,
            can_leave_community,
            can_follow_brand,
            is_following_brand,
            can_use_gathering_rsvp,
            gathering_rsvp,
            viewer_has_connection_capacity,
            location_display_label,
            location_google_place_id,
            website_url: trim_nonempty(r.website_url),
            avatar_path: r.avatar_path,
            entity_status_key: r.entity_status_key,
            verified_member_count: r.verified_member_count,
            community_join_policy_key: r.community_join_policy_key,
            description: trim_nonempty(r.extension_description),
            intention: trim_nonempty(r.extension_intention),
            brand_base_follower_count: r.brand_base_follower_count.max(0) as u64,
            gathering_rsvp_accepted: r.gathering_rsvp_accepted.max(0) as u64,
            gathering_rsvp_not_going: r.gathering_rsvp_not_going.max(0) as u64,
            gathering_rsvp_interested: r.gathering_rsvp_interested.max(0) as u64,
            effective_constitution,
            can_propose_constitutional_amendment,
            can_propose_community_profile_amendment,
            can_propose_community_reparent,
            community_is_failing,
            human_is_failing,
            is_self_profile: is_self,
            show_human_rating_warning,
            show_human_inactivity_notice,
            human_estimated_deletion_display,
            avatar_redraw_nom_cost,
            avatar_redraw_can_afford,
            avatar_redraw_noms_balance,
            can_invite_community_members,
            can_respond_to_community_invite,
            can_request_community_membership,
            can_apply_for_membership_review,
            has_open_membership_review_application,
            membership_review_application_outcome,
            human_profile_edit,
            entity_profile_edit,
            can_send_recruitment_invites,
        }))
}
