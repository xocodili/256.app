use axum::extract::{Extension, Form, Path, Query, State};
use axum::http::header;
use axum::http::{HeaderMap, HeaderValue};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::auth::post_auth;
use crate::config::Config;
use crate::content::sanitize as content_sanitize;
use crate::error::{AppError, AppResult};
use crate::html_escape::{escape_html, escape_json_for_html_script};
use crate::i18n;
use crate::middleware::VerifiedSession;
use crate::auth::password::MIN_PASSWORD_LEN;
use crate::pricing::BillableService;
use crate::ref_key_values;
use crate::ref_keys;
use crate::social::blocks;
use crate::social::community_constitution;
use crate::social::community_graph;
use crate::social::community_invite;
use crate::social::judicial::community_reparent;
use crate::social::connections;
use crate::social::pending_invitations;
use crate::social::entity;
use crate::social::feed::{self, HomeFeedRow};
use crate::social::human_onboarding;
use crate::social::judicial::{self, AuthorWithdrawPostUnderReviewOutcome};
use crate::social::judicial_link_evidence;
use crate::social::noms;
use crate::social::ownership;
use crate::social::private_profile;
use crate::social::profile_actions;
use crate::social::relationship;
use crate::social::scope;
use super::profile_gate;
use crate::social::user_faces;
use crate::status;
use crate::ui::entity_type_accent;
use crate::ui::nom_icon;
use crate::ui::outstanding_invitations;
use crate::ui::private_profile_card;
use crate::ui::rail;
use crate::ui::snippets;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/home", get(home_page))
        .route("/home/feed-day", get(home_feed_day))
        .route("/home/mentions/ack", post(home_mentions_ack))
        .route("/home/judicial/post/{post_id}", get(home_judicial_post_case_page))
        .route("/home/judicial/post/{post_id}", post(home_judicial_post_case_submit))
        .route(
            "/home/judicial/post/{post_id}/appeal",
            get(home_judicial_post_appeal_page).post(home_judicial_post_appeal_submit),
        )
        .route(
            "/home/judicial/post/{post_id}/contest",
            get(home_judicial_post_emergency_contest_page)
                .post(home_judicial_post_emergency_contest_submit),
        )
        .route(
            "/home/judicial/post/{post_id}/author-withdraw",
            post(home_judicial_post_author_withdraw_submit),
        )
        .route(
            "/home/judicial/community/{handle}",
            get(home_judicial_community_amendment_page).post(home_judicial_community_amendment_submit),
        )
        .route(
            "/home/judicial/community/{handle}/profile",
            get(home_judicial_community_profile_amendment_page)
                .post(home_judicial_community_profile_amendment_submit),
        )
        .route(
            "/home/community/{handle}/reparent",
            get(home_community_reparent_page).post(home_community_reparent_submit),
        )
        .route("/home/profile/{handle}", get(private_profile_page))
        .route(
            "/home/private-profile-card/{handle}",
            get(private_profile_card_fragment),
        )
        .route(
            "/home/private-profile-connect/{handle}",
            post(private_profile_connect_submit),
        )
        .route(
            "/home/private-profile-connect-action/{handle}",
            post(private_profile_connect_action_submit),
        )
        .route(
            "/home/private-profile-cancel-invite/{handle}",
            post(private_profile_cancel_invite_submit),
        )
        .route(
            "/home/private-profile-disconnect/{handle}",
            post(private_profile_disconnect_submit),
        )
        .route(
            "/home/private-profile-block/{handle}",
            post(private_profile_block_submit),
        )
        .route(
            "/home/private-profile-leave-community/{handle}",
            post(private_profile_leave_community_submit),
        )
        .route(
            "/home/private-profile-community-invite/{handle}",
            post(private_profile_community_invite_submit),
        )
        .route(
            "/home/private-profile-community-invite-action/{handle}",
            post(private_profile_community_invite_action_submit),
        )
        .route(
            "/home/pending-invitation/{relationship_id}/respond",
            post(pending_invitation_respond_submit),
        )
        .route(
            "/home/private-profile-follow/{handle}",
            post(private_profile_follow_submit),
        )
        .route(
            "/home/private-profile-unfollow/{handle}",
            post(private_profile_unfollow_submit),
        )
        .route(
            "/home/private-profile-gathering-rsvp/{handle}",
            post(private_profile_gathering_rsvp_submit),
        )
        .route("/onboarding", get(face_capture_page))
        .route("/home/face-capture", get(legacy_face_capture_redirect))
        .route("/home/face-capture-card", get(face_capture_card_fragment))
        .route(
            "/home/profile-completion-card",
            get(profile_completion_card_fragment),
        )
        .route(
            "/home/terms-consent-card",
            get(terms_consent_card_fragment),
        )
        .route("/home/create-post-card", get(create_post_card_fragment))
        .route("/home/search-add-card", get(search_add_card_fragment))
        .route("/home/create-entity-card", get(create_entity_card_fragment))
        .route("/home/juror-invite-panel", get(home_outstanding_panel))
        .route("/home/outstanding-panel", get(home_outstanding_panel))
        .route(
            "/home/juror-invite/{invite_id}/workspace",
            get(home_juror_invite_workspace),
        )
        .route(
            "/home/juror-invite/{invite_id}/abstain",
            post(home_juror_invite_abstain),
        )
        .route(
            "/home/juror-invite/{invite_id}/decline",
            post(home_juror_invite_decline),
        )
        .route(
            "/home/juror-invite/{invite_id}/vote",
            post(home_juror_invite_vote),
        )
}

async fn legacy_face_capture_redirect() -> Redirect {
    Redirect::temporary("/onboarding")
}

fn face_capture_i18n_json(loc: &str, artist_handle: &str) -> String {
    let c = i18n::catalog();
    let avatar_cost = BillableService::CouncilAvatarRefresh.nom_cost();
    let handle = artist_handle.trim().trim_start_matches('@');
    let at_handle = format!("@{handle}");
    serde_json::json!({
        "stage_neutral": c.t(loc, "ui.face_capture.stage_neutral"),
        "stage_smile": c.t(loc, "ui.face_capture.stage_smile"),
        "stage_frown": c.t(loc, "ui.face_capture.stage_frown"),
        "face_forward_hint": c.t(loc, "ui.face_capture.face_forward_hint"),
        "frame_head_hint": c.t(loc, "ui.face_capture.frame_head_hint"),
        "err_move_closer": c.t(loc, "ui.face_capture.err_move_closer"),
        "err_dark": c.t(loc, "ui.face_capture.err_dark"),
        "steady": c.t(loc, "ui.face_capture.steady"),
        "start_camera": c.t(loc, "ui.face_capture.start_camera"),
        "submit": c.t(loc, "ui.face_capture.submit"),
        "reset": c.t(loc, "ui.face_capture.reset"),
        "success": c.t(loc, "ui.face_capture.success"),
        "err_camera": c.t(loc, "ui.face_capture.err_camera"),
        "err_models": c.t(loc, "ui.face_capture.err_models"),
        "err_no_face": c.t(loc, "ui.face_capture.err_no_face"),
        "err_detection": c.t(loc, "ui.face_capture.err_detection"),
        "err_submit": c.t(loc, "ui.face_capture.err_submit"),
        "err_incomplete": c.t(loc, "ui.face_capture.err_incomplete"),
        "network_error": c.t(loc, "ui.shared.network_error_detail"),
        "all_captured": c.t(loc, "ui.face_capture.all_captured"),
        "pipeline_stage_uploading": c.t(loc, "ui.face_capture.pipeline_stage_uploading"),
        "pipeline_stage_queued": c.tf(loc, "ui.face_capture.pipeline_stage_queued", &[("artist_handle", &at_handle)]),
        "pipeline_stage_validating": c.t(loc, "ui.face_capture.pipeline_stage_validating"),
        "pipeline_stage_likeness": c.t(loc, "ui.face_capture.pipeline_stage_likeness"),
        "pipeline_stage_painting": c.tf(loc, "ui.face_capture.pipeline_stage_painting", &[("artist_handle", &at_handle)]),
        "submit_wait": c.t(loc, "ui.face_capture.submit_wait"),
        "pipeline_timeout": c.t(loc, "ui.face_capture.pipeline_timeout"),
        "submit_retry": c.t(loc, "ui.face_capture.submit_retry"),
        "rate_limited": c.t(loc, "ui.face_capture.rate_limited"),
        "rate_limited_retry": c.t(loc, "ui.face_capture.rate_limited_retry"),
        "capture_pause": c.t(loc, "ui.face_capture.capture_pause"),
        "err_insufficient_diversity": c.t(loc, "ui.face_capture.insufficient_diversity"),
        "err_vision_rejected": c.tf(loc, "ui.face_capture.vision_rejected", &[("artist_handle", &at_handle)]),
        "err_vision_rejected_not_live": c.t(loc, "ui.face_capture.vision_rejected_not_live"),
        "err_vision_failed": c.t(loc, "ui.face_capture.vision_failed"),
        "err_vision_rate_limited": c.t(loc, "ui.face_capture.vision_rate_limited"),
        "err_vision_timeout": c.tf(loc, "ui.face_capture.vision_timeout", &[("seconds", "15")]),
        "err_portrait_failed": c.tf(loc, "ui.face_capture.portrait_failed", &[("artist_handle", &at_handle)]),
        "err_quality_too_small": c.t(loc, "ui.face_capture.quality_too_small"),
        "err_quality_low_resolution": c.t(loc, "ui.face_capture.quality_low_resolution"),
        "err_quality_move_closer": c.t(loc, "ui.face_capture.quality_move_closer"),
        "err_media_blocked": c.t(loc, "social.media_blocked_safety"),
        "models_ready": c.t(loc, "ui.face_capture.models_ready"),
        "profile_required_before_godmother": c.t(loc, "ui.face_capture.profile_required_before_godmother"),
        "godmother_heading": c.t(loc, "ui.face_capture.godmother_heading"),
        "godmother_subtitle": c.t(loc, "ui.face_capture.godmother_subtitle"),
        "godmother_submit": c.t(loc, "ui.face_capture.godmother_submit"),
        "godmother_status_pending": c.t(loc, "ui.face_capture.godmother_status_pending"),
        "godmother_status_ready": c.t(loc, "ui.face_capture.godmother_status_ready"),
        "godmother_status_selected": c.t(loc, "ui.face_capture.godmother_status_selected"),
        "godmother_load_failed": c.t(loc, "ui.face_capture.godmother_load_failed"),
        "godmother_empty": c.t(loc, "ui.face_capture.godmother_empty"),
        "godmother_select_required": c.t(loc, "ui.face_capture.godmother_select_required"),
        "godmother_conflict_selected": c.t(loc, "ui.face_capture.godmother_conflict_selected"),
        "avatar_artist_handle": handle,
        "avatar_artist_profile_url": format!("/public/{handle}"),
        "avatar_redraw_generating_suffix": c.t(loc, "ui.face_capture.avatar_redraw_generating_suffix"),
        "avatar_artist_link_aria": c.tf(loc, "ui.face_capture.avatar_artist_link_aria", &[("artist_handle", &at_handle)]),
        "avatar_redraw_complete": c.t(loc, "ui.face_capture.avatar_redraw_complete"),
        "avatar_redraw_failed": c.t(loc, "ui.entity_private.avatar_redraw_failed"),
        "avatar_redraw_submit": nom_icon::locale_with_nom_price_paren(
            &c,
            loc,
            "ui.face_capture.avatar_redraw_submit",
            avatar_cost,
        ),
    })
    .to_string()
}

/// Slot meter for chrome: `{n}/{max}` unless at capacity, then plain `{max}` (full connection load).
fn connection_slot_meter_display(connection_count: usize) -> String {
    if connection_count >= connections::MAX_CONNECTION_SLOTS {
        connections::MAX_CONNECTION_SLOTS.to_string()
    } else {
        format!(
            "{}/{}",
            connection_count,
            connections::MAX_CONNECTION_SLOTS
        )
    }
}

/// Top bar: `{meter} - {suffix}`.
pub(crate) fn topbar_title_with_suffix_html(connection_count: usize, suffix: &str) -> String {
    format!(
        r#"<span class="app-topbar-title mono"><span class="app-topbar-slot-prefix">{}</span><span class="app-topbar-title-sep"> - </span><span class="app-topbar-display">{}</span></span>"#,
        escape_html(&connection_slot_meter_display(connection_count)),
        escape_html(suffix),
    )
}

/// Home feed: `{meter} - {tagline}` on tablet/desktop; `{meter} - {viewer_handle_at}` on phones.
pub(crate) fn topbar_title_home_feed_html(
    connection_count: usize,
    tagline: &str,
    viewer_handle_at: &str,
) -> String {
    format!(
        r#"<span class="app-topbar-title mono"><span class="app-topbar-slot-prefix">{}</span><span class="app-topbar-title-sep"> - </span><span class="app-topbar-home-tagline">{}</span><span class="app-topbar-home-handle">{}</span></span>"#,
        escape_html(&connection_slot_meter_display(connection_count)),
        escape_html(tagline),
        escape_html(viewer_handle_at),
    )
}

async fn juror_summons_pending_for_user(
    pool: &sqlx::MySqlPool,
    user_id: u64,
) -> Result<bool, sqlx::Error> {
    Ok(
        judicial::find_pending_human_jury_invite_for_user(pool, user_id)
            .await?
            .is_some(),
    )
}

async fn topbar_juror_summons_badge_html(
    pool: &sqlx::MySqlPool,
    loc: &str,
    user_id: u64,
) -> AppResult<String> {
    if !juror_summons_pending_for_user(pool, user_id)
        .await
        .map_err(AppError::Db)?
    {
        return Ok(String::new());
    }
    let c = i18n::catalog();
    Ok(snippets::topbar_juror_summons_badge_html(
        &c.t(loc, "ui.home.juror_summons_badge"),
    ))
}


pub(crate) async fn secured_shell_topbar_html(
    state: &AppState,
    user_id: u64,
    loc: &str,
    title_html: &str,
    show_create_post: bool,
    avatar_html: &str,
    ai_face: bool,
) -> AppResult<String> {
    let c = i18n::catalog();
    let juror_badge = topbar_juror_summons_badge_html(&state.db, loc, user_id).await?;
    let leading = format!("{}{}", juror_badge, title_html);
    let run_jobs = topbar_tool_buttons_html(state, user_id, loc).await?;
    let noms = topbar_noms_html_for_user(&state.db, user_id, &c, loc).await?;
    let create = if show_create_post {
        snippets::topbar_create_post_html(&c.t(loc, "ui.home.create_post_open"))
    } else {
        String::new()
    };
    let logo_aria = c.t(loc, "ui.home.logo_aria_home");
    Ok(if ai_face {
        snippets::secured_topbar_html_ai(
            &leading,
            &run_jobs,
            &noms,
            &create,
            avatar_html,
            &logo_aria,
        )
    } else {
        snippets::secured_topbar_html(
            &leading,
            &run_jobs,
            &noms,
            &create,
            avatar_html,
            &logo_aria,
        )
    })
}

pub(crate) fn csrf_meta_html(headers: &HeaderMap, config: &Config) -> String {
    profile_gate::csrf_token_for_request(headers, config)
        .map(|token| {
            format!(
                r#"<meta name="csrf-token" content="{}">"#,
                escape_html(&token)
            )
        })
        .unwrap_or_default()
}

pub(crate) async fn topbar_tool_buttons_html(
    state: &AppState,
    user_id: u64,
    loc: &str,
) -> AppResult<String> {
    let c = i18n::catalog();
    let mut html = crate::social::panoply::topbar_ops_button_html(
        &state.db,
        user_id,
        &c.t(loc, "ui.home.ops"),
    )
    .await
    .map_err(AppError::Db)?;
    html.push_str(&snippets::topbar_run_jobs_html(
        state.config.dev_auth_bypass || cfg!(debug_assertions),
        &c.t(loc, "ui.home.run_jobs"),
    ));
    Ok(html)
}

fn home_shell_i18n_json(loc: &str, connection_count: usize, viewer_handle: Option<&str>) -> String {
    let c = i18n::catalog();
    let feed_personal_filter = viewer_handle
        .map(|h| format!("@{h}"))
        .unwrap_or_else(|| c.t(loc, "ui.home.feed_show_mentions"));
    serde_json::json!({
        "profile_load_failed": c.t(loc, "ui.home_profile.load_failed"),
        "terms_step_failed": c.t(loc, "ui.home_profile.terms_step_failed"),
        "terms_consent_failed": c.t(loc, "ui.home_profile.terms_consent_failed"),
        "must_confirm_adult": c.t(loc, "ui.register.must_confirm_adult"),
        "must_confirm_constitution": c.t(loc, "ui.register.must_confirm_constitution"),
        "request_failed": c.t(loc, "ui.shared.request_failed"),
        "network_error_detail": c.t(loc, "ui.shared.network_error_detail"),
        "card_rate_limited": c.t(loc, "ui.login.card_rate_limited"),
        "feed_no_posts_yet": c.t(loc, "ui.home.feed_no_posts_yet"),
        "feed_range_default": c.t(loc, "ui.home.feed_range_default"),
        "feed_page_label": c.t(loc, "ui.home.feed_page_label"),
        "feed_nav_first_page": c.t(loc, "ui.home.feed_nav_first_page"),
        "feed_nav_newer": c.t(loc, "ui.home.feed_nav_newer"),
        "feed_nav_older": c.t(loc, "ui.home.feed_nav_older"),
        "feed_controls_aria": c.t(loc, "ui.home.feed_controls_aria"),
        "feed_loading_aria": c.t(loc, "ui.home.feed_loading_aria"),
        "feed_load_failed": c.t(loc, "ui.home.feed_load_failed"),
        "feed_scroll_to_top": c.t(loc, "ui.home.feed_scroll_to_top"),
        "feed_posted_from": c.t(loc, "ui.home.feed_posted_from"),
        "feed_show_mentions": feed_personal_filter,
        "mentions_login_notice": c.t(loc, "ui.home.mentions_login_notice"),
        "mention_autocomplete_aria": c.t(loc, "ui.home.mention_autocomplete_aria"),
        "mention_show_all": c.t(loc, "ui.home.mention_show_all"),
        "audience_autocomplete_aria": c.t(loc, "ui.home.audience_autocomplete_aria"),
        "create_post_domain_blocked": c.t(loc, "ui.home.create_post_domain_blocked"),
        "create_post_og_loading": c.t(loc, "ui.home.create_post_og_loading"),
        "create_post_og_loaded": c.t(loc, "ui.home.create_post_og_loaded"),
        "create_post_og_failed": c.t(loc, "ui.home.create_post_og_failed"),
        "create_post_to": c.t(loc, "ui.home.create_post_to"),
        "create_post_characters_left": c.t(loc, "ui.home.create_post_characters_left"),
        "create_post_character_limit_reached": c.t(loc, "ui.home.create_post_character_limit_reached"),
        "create_post_load_failed": c.t(loc, "ui.home.create_post_load_failed"),
        "create_post_success": c.t(loc, "ui.home.create_post_success"),
        "post_engagement_feel_loved": c.t(loc, "ui.post_engagement.feel_loved"),
        "post_engagement_feel_controversial": c.t(loc, "ui.post_engagement.feel_controversial"),
        "post_engagement_feel_loathed": c.t(loc, "ui.post_engagement.feel_loathed"),
        "post_engagement_vote_aria": c.t(loc, "ui.post_engagement.vote_aria"),
        "post_engagement_comment_placeholder": c.t(loc, "ui.post_engagement.comment_placeholder"),
        "post_engagement_share_link_aria": c.t(loc, "ui.post_engagement.share_link_aria"),
        "post_engagement_share_audience_prompt": c.t(loc, "ui.post_engagement.share_audience_prompt"),
        "post_engagement_reply": c.t(loc, "ui.post_engagement.reply"),
        "post_engagement_reply_placeholder": c.t(loc, "ui.post_engagement.reply_placeholder"),
        "post_engagement_open_case": c.t(loc, "ui.post_engagement.open_case"),
        "post_engagement_show_more": c.t(loc, "ui.post_engagement.show_more"),
        "post_engagement_show_less": c.t(loc, "ui.post_engagement.show_less"),
        "post_engagement_load_more": c.t(loc, "ui.post_engagement.load_more"),
        "search_add_load_failed": c.t(loc, "ui.home.search_add_load_failed"),
        "search_add_enter_handle": c.t(loc, "ui.home.search_add_enter_handle"),
        "search_add_create_success": c.t(loc, "ui.home.search_add_create_success"),
        "search_no_results": c.t(loc, "ui.home.search_no_results"),
        "search_recommendations_empty": c.t(loc, "ui.home.search_recommendations_empty"),
        "search_recommendations_load_failed": c.t(loc, "ui.home.search_recommendations_load_failed"),
        "search_recommendations_direct_title": c.t(loc, "ui.home.search_recommendations_direct_title"),
        "search_recommendations_inferred_title": c.t(loc, "ui.home.search_recommendations_inferred_title"),
        "search_recommendations_type_community": c.t(loc, "ui.home.search_recommendations_type_community"),
        "search_recommendations_type_non_profit": c.t(loc, "ui.home.search_recommendations_type_non_profit"),
        "search_add_results_aria": c.t(loc, "ui.home.search_add_results_aria"),
        "create_entity_load_failed": c.t(loc, "ui.home.create_entity_load_failed"),
        "create_entity_location_required": c.t(loc, "ui.home.create_entity_location_required"),
        "create_entity_business_location_required": c.t(loc, "ui.home.create_entity_business_location_required"),
        "create_entity_place_id_required": c.t(loc, "ui.home.create_entity_place_id_required"),
        "create_entity_place_query_min": c.t(loc, "ui.home.create_entity_place_query_min"),
        "create_entity_place_search_failed": c.t(loc, "ui.home.create_entity_place_search_failed"),
        "create_entity_place_searching": c.t(loc, "ui.home.create_entity_place_searching"),
        "create_entity_place_verifying": c.t(loc, "ui.home.create_entity_place_verifying"),
        "create_entity_place_confirmed_prefix": c.t(loc, "ui.home.create_entity_place_confirmed_prefix"),
        "create_entity_founded_required": c.t(loc, "ui.home.create_entity_founded_required"),
        "create_entity_nom_cost": c.t(loc, "ui.home.create_entity_nom_cost"),
        "handle_checking": c.t(loc, "ui.home.handle_checking"),
        "handle_taken": c.t(loc, "ui.home.handle_taken"),
        "handle_ready": c.t(loc, "ui.home.handle_ready"),
        "handle_reserved": c.t(loc, "ui.home.handle_reserved"),
        "handle_invalid": c.t(loc, "ui.home.handle_invalid"),
        "handle_min_chars": c.t(loc, "ui.home.handle_min_chars"),
        "handle_check_failed": c.t(loc, "ui.home.handle_check_failed"),
        "min_age_years": crate::social::human_onboarding::MIN_AGE_YEARS,
        "search_add_search_failed": c.t(loc, "ui.home.search_add_search_failed"),
        "friend_invite_sent": c.t(loc, "ui.home.friend_invite_sent"),
        "friend_invite_invalid_email": c.t(loc, "ui.home.friend_invite_invalid_email"),
        "friend_invite_failed": c.t(loc, "ui.home.friend_invite_failed"),
        "friend_invite_smtp_unconfigured": c.t(loc, "ui.home.friend_invite_smtp_unconfigured"),
        "friend_invite_email_registered": c.t(loc, "ui.home.friend_invite_email_registered"),
        "friend_invite_already_on_platform": c.t(loc, "ui.home.friend_invite_already_on_platform"),
        "recruitment_invite_bulk_result": c.t(loc, "ui.entity_private.recruitment_invite_bulk_result"),
        "recruitment_invite_send_cta": c.t(loc, "ui.home.search_add_invite_send_cta"),
        "private_profile_load_failed": c.t(loc, "ui.home.private_profile_load_failed"),
        "connection_slots_used": connection_count,
        "connection_slots_max": connections::MAX_CONNECTION_SLOTS,
        "private_profile_doc_title_suffix": c.t(loc, "ui.home.private_profile_doc_title_suffix"),
        "run_jobs": c.t(loc, "ui.home.run_jobs"),
        "run_jobs_running": c.t(loc, "ui.home.run_jobs_running"),
        "run_jobs_done": c.t(loc, "ui.home.run_jobs_done"),
        "run_jobs_idle": c.t(loc, "ui.home.run_jobs_idle"),
        "run_jobs_failed": c.t(loc, "ui.home.run_jobs_failed"),
        "judicial_case_filed_notice": c.t(loc, "ui.home.judicial_case_filed_notice"),
        "inactivity_standing_restored_notice": c.t(
            loc,
            "ui.home.inactivity_standing_restored_notice",
        ),
        "constitutional_amendment_filed_notice": c.t(
            loc,
            "ui.home.constitutional_amendment_filed_notice",
        ),
        "community_profile_amendment_filed_notice": c.t(
            loc,
            "ui.home.community_profile_amendment_filed_notice",
        ),
        "judicial_appeal_filed_notice": c.t(loc, "ui.home.judicial_appeal_filed_notice"),
        "emergency_contest_opened_notice": c.t(loc, "ui.home.emergency_contest_opened_notice"),
        "connect_sending": c.t(loc, "ui.home.connect_sending"),
        "connect_send_failed": c.t(loc, "ui.home.connect_send_failed"),
        "connect_invitation_sent": c.t(loc, "ui.entity_private.connection_invitation_sent"),
        "invite_response_accepted": c.t(loc, "ui.entity_private.connection_invitation_accepted"),
        "invite_response_declined": c.t(loc, "ui.entity_private.connection_invitation_declined"),
        "invite_response_blocked": c.t(loc, "ui.entity_private.connection_invitation_blocked"),
        "invitation_cancelled_notice": c.t(loc, "ui.entity_private.invitation_cancelled_notice"),
        "disconnected_notice": c.t(loc, "ui.entity_private.disconnected_notice"),
        "avatar_redraw_success": c.t(loc, "ui.entity_private.avatar_redraw_success"),
        "avatar_redraw_failed": c.t(loc, "ui.entity_private.avatar_redraw_failed"),
        "avatar_redraw_insufficient_noms": c.t(loc, "noms.insufficient_for_avatar_redraw"),
        "create_post_insufficient_noms": c.t(loc, "noms.insufficient_for_branded_post"),
        "nom_icon_sm": nom_icon::nom_icon_sm(&nom_icon::currency_tooltip(&c, loc)),
        "profile_edit_success": c.t(loc, "ui.entity_private.profile_edit_success"),
        "profile_edit_failed": c.t(loc, "ui.entity_private.profile_edit_failed"),
        "change_password_success": c.t(loc, "ui.entity_private.change_password_success"),
        "change_password_failed": c.t(loc, "ui.entity_private.change_password_failed"),
        "change_password_mismatch": c.t(loc, "ui.entity_private.change_password_mismatch"),
        "account_password_same_as_current": c.t(loc, "ui.entity_private.account_password_same_as_current"),
        "account_password_required": c.t(loc, "ui.entity_private.account_password_required"),
        "password_too_short": c.tf(
            loc,
            "auth.password_too_short",
            &[("min_len", &MIN_PASSWORD_LEN.to_string())],
        ),
        "delete_account_success": c.t(loc, "ui.entity_private.delete_account_success"),
        "delete_account_failed": c.t(loc, "ui.entity_private.delete_account_failed"),
        "delete_account_confirm_required": c.t(loc, "ui.entity_private.delete_account_confirm_required"),
        "delete_entity_success": c.t(loc, "ui.entity_private.delete_entity_success"),
        "delete_entity_failed": c.t(loc, "ui.entity_private.delete_entity_failed"),
        "delete_entity_confirm_required": c.t(loc, "ui.entity_private.delete_entity_confirm_required"),
        "block_success": c.t(loc, "ui.entity_private.block_success"),
        "manage_blocks_load_failed": c.t(loc, "ui.entity_private.manage_blocks_load_failed"),
        "manage_blocks_empty": c.t(loc, "ui.entity_private.manage_blocks_empty"),
        "unblock_cta": c.t(loc, "ui.entity_private.unblock_cta"),
        "unblock_success": c.t(loc, "ui.entity_private.unblock_success"),
        "unblock_failed": c.t(loc, "ui.entity_private.unblock_failed"),
        "community_left_notice": c.t(loc, "ui.entity_private.community_left_notice"),
        "community_invite_sent": c.t(loc, "ui.entity_private.community_invite_sent"),
        "community_invite_search_no_results": c.t(loc, "ui.entity_private.community_invite_search_no_results"),
        "community_joined_notice": c.t(loc, "ui.entity_private.community_joined_notice"),
        "open_membership_review_notice": c.t(loc, "ui.entity_private.open_membership_review_notice"),
        "membership_review_statement_required": c.t(
            loc,
            "ui.entity_private.membership_review_statement_required",
        ),
        "noms_wallet_aria": c.tf(
            loc,
            "noms.wallet_aria",
            &[
                ("balance", "{balance}"),
                ("cap", &noms::NOMS_MAX.to_string()),
            ],
        ),
        "following_saved_notice": c.t(loc, "ui.entity_private.following_saved_notice"),
        "unfollowed_notice": c.t(loc, "ui.entity_private.unfollowed_notice"),
        "gathering_rsvp_saved_notice": c.t(loc, "ui.entity_private.gathering_rsvp_saved_notice"),
        "constitution_show_more": c.t(loc, "ui.entity_private.constitution_show_more"),
        "constitution_show_less": c.t(loc, "ui.entity_private.constitution_show_less"),
        "view_full_profile_cta": c.t(loc, "ui.entity_private.view_full_profile_cta"),
        "view_summary_profile_cta": c.t(loc, "ui.entity_private.view_summary_profile_cta"),
        "create_entity_community_location_global_hint": c.t(
            loc,
            "ui.home.create_entity_community_location_global_hint",
        ),
        "create_entity_community_location_country_hint": c.t(
            loc,
            "ui.home.create_entity_community_location_country_hint",
        ),
        "create_entity_community_confirm_global": c.t(
            loc,
            "ui.home.create_entity_community_confirm_global",
        ),
        "create_entity_community_confirm_country_lock": c.t(
            loc,
            "ui.home.create_entity_community_confirm_country_lock",
        ),
        "create_entity_community_join_open_hint": c.t(
            loc,
            "ui.home.create_entity_community_join_open_hint",
        ),
        "create_entity_community_join_reviewed_hint": c.t(
            loc,
            "ui.home.create_entity_community_join_reviewed_hint",
        ),
        "create_entity_community_join_invite_only_hint": c.t(
            loc,
            "ui.home.create_entity_community_join_invite_only_hint",
        ),
        "create_entity_community_join_closed_hint": c.t(
            loc,
            "ui.home.create_entity_community_join_closed_hint",
        ),
        "create_entity_community_parent_required": c.t(
            loc,
            "ui.home.create_entity_community_parent_required",
        ),
        "create_entity_community_location_required": c.t(
            loc,
            "ui.home.create_entity_community_location_required",
        ),
        "constitution_loading": c.t(loc, "ui.home.create_entity_community_constitution_loading"),
        "constitution_load_failed": c.t(loc, "ui.home.create_entity_community_constitution_load_failed"),
        "judicial_withdraw_success": c.t(loc, "ui.judicial_post_case.withdraw_success_toast"),
        "judicial_withdraw_failed": c.t(loc, "ui.judicial_post_case.withdraw_failed"),
        "juror_summons_badge": c.t(loc, "ui.home.juror_summons_badge"),
        "juror_workspace_failed": c.t(loc, "ui.juror_workspace.workspace_failed"),
        "juror_vote_recorded": c.t(loc, "ui.juror_workspace.vote_recorded"),
        "juror_abstain_recorded": c.t(loc, "ui.juror_workspace.abstain_recorded"),
        "juror_decline_recorded": c.t(loc, "ui.juror_summons.decline_recorded"),
        "juror_law_notice": c.t(loc, "ui.juror_workspace.law_notice"),
        "emergency_confirm": c.t(loc, "ui.judicial_post_case.emergency_confirm"),
        "sanitation_clause_blocked": c.t(loc, "ui.judicial_post_case.sanitation_clause_blocked"),
        "sanitation_notice_visible": c.t(loc, "ui.judicial_post_case.sanitation_notice_visible"),
    })
    .to_string()
}

#[derive(Debug, Deserialize)]
struct OnboardingQuery {
    consented: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HomeQuery {
    onboarding: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FeedDayQuery {
    page: Option<u32>,
    #[serde(default)]
    profile_handle: Option<String>,
    /// Query strings use `1` / `true` (see `home-shell.js`); not serde's bool tokens alone.
    #[serde(default)]
    mentions_only: Option<String>,
}

fn query_flag_is_truthy(raw: Option<&str>) -> bool {
    matches!(
        raw.map(str::trim),
        Some("1" | "true" | "yes" | "on")
    )
}

#[derive(Debug, Deserialize)]
struct CreateEntityCardQuery {
    /// `social_entity_type` ref key: `community`, `gathering`, `profit`, `non_profit`.
    #[serde(rename = "type")]
    entity_type: String,
}

#[derive(Debug, Serialize)]
struct FeedDayResponse {
    page: u32,
    range_label: String,
    has_newer: bool,
    has_older: bool,
    cards_html: String,
    noms: u16,
    juror_summons_pending: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    unseen_mention_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct JudicialPostCaseFormBody {
    /// Comma-separated clause ids (`1,2,3`). Axum's `Form` uses `serde_urlencoded`, which does not
    /// deserialize repeated keys into `Vec` for structs Ã¢â‚¬â€ multiple checkboxes would yield 422.
    #[serde(default)]
    violated_clause_ids: String,
    #[serde(default)]
    rationale: Option<String>,
    #[serde(default)]
    return_to: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JudicialAuthorWithdrawForm {
    withdraw_action: String,
}

#[derive(Debug, Deserialize)]
struct JudicialPostCasePageQuery {
    #[serde(default)]
    return_to: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JudicialAppealPageQuery {
    #[serde(default)]
    from_case: Option<String>,
    #[serde(default)]
    return_to: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JudicialAppealFormBody {
    pub prior_case_id: String,
    #[serde(default)]
    pub rationale: Option<String>,
    #[serde(default)]
    pub return_to: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JudicialEmergencyContestPageQuery {
    #[serde(default)]
    return_to: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JudicialEmergencyContestForm {
    #[serde(default)]
    return_to: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PrivateConnectForm {
    connect_as: String,
    #[serde(default)]
    application_statement: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PrivateConnectActionForm {
    response_action: String,
}

#[derive(Debug, Deserialize)]
struct CommunityInviteForm {
    invitee_handle: String,
}

#[derive(Debug, Deserialize)]
struct GatheringRsvpForm {
    gathering_rsvp: String,
}

#[derive(Debug, sqlx::FromRow)]
struct PriorResolvedPostCaseRow {
    id: u64,
    subject_post_id: Option<u64>,
    originator_social_entity_id: u64,
    target_social_entity_id: u64,
}

#[derive(Debug, sqlx::FromRow)]
struct JudicialClauseRow {
    id: u64,
    community_handle: String,
    community_label: String,
    point: u16,
    item: String,
    judicial_trigger_chk: i8,
}

#[derive(Debug, Deserialize)]
struct JudicialCommunityAmendmentPageQuery {
    return_to: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JudicialCommunityAmendmentForm {
    amendment_kind: String,
    target_constitution_item_id: Option<String>,
    proposal: Option<String>,
    rationale: Option<String>,
    return_to: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JudicialCommunityProfileAmendmentPageQuery {
    return_to: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JudicialCommunityProfileAmendmentForm {
    description: Option<String>,
    intention: Option<String>,
    join_policy: Option<String>,
    rationale: Option<String>,
    return_to: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct CommunityAmendmentTargetRow {
    entity_id: u64,
    handle: String,
    display_label: String,
}

#[derive(Debug, sqlx::FromRow)]
struct CommunityProfileAmendmentTargetRow {
    entity_id: u64,
    handle: String,
    display_label: String,
    description: Option<String>,
    intention: Option<String>,
    join_policy_key: Option<String>,
}

const AMEND_KIND_NEW: &str = "new_clause";
const AMEND_KIND_AMEND: &str = "amend_clause";
const AMEND_KIND_REMOVE: &str = "remove_clause";

fn juror_redaction_labels(c: &i18n::Catalog, loc: &str) -> (String, String) {
    (
        c.t(loc, "ui.juror_workspace.party_redacted"),
        c.t(loc, "ui.juror_workspace.link_redacted"),
    )
}

fn juror_facing_submission_html(
    c: &i18n::Catalog,
    loc: &str,
    html: &str,
    plain: &str,
) -> String {
    let (party, link) = juror_redaction_labels(c, loc);
    crate::content::juror_anonymize::juror_facing_body(html, plain, &party, &link)
}

fn feed_post_card_html(
    r: &HomeFeedRow,
    viewer_user_id: u64,
    viewer_human_entity_id: Option<u64>,
    show_judicial_ctas: bool,
    show_engagement_bar: bool,
    viewer_country_code: Option<&str>,
    preview_image_allowed: bool,
    public_base_url: &str,
    c: &i18n::Catalog,
    loc: &str,
) -> String {
    crate::ui::post_card::render_authenticated_feed_card(
        r,
        &crate::ui::post_card::FeedCardOptions {
            viewer_user_id,
            viewer_human_entity_id,
            show_judicial_ctas,
            show_engagement_bar,
            viewer_country_code: viewer_country_code.map(str::to_string),
            preview_image_allowed,
            og_preview_reveal_label: c.t(loc, "ui.post.og_preview_reveal"),
            og_preview_withheld_hint: Some(c.t(loc, "ui.post.og_preview_withheld_hint")),
            public_base_url: Some(public_base_url.to_string()),
        },
        c,
        loc,
    )
}

async fn preview_image_allowed_for_feed_rows(
    pool: &sqlx::MySqlPool,
    rows: &[HomeFeedRow],
) -> Result<std::collections::HashMap<u64, bool>, sqlx::Error> {
    let domain_ids: Vec<u64> = rows.iter().filter_map(|r| r.domain_id).collect();
    crate::content::domain_reputation::preview_image_allowed_map(pool, &domain_ids).await
}

fn clamp_feed_page(raw: u32) -> u32 {
    raw.min(100_000)
}
/// Council AI shown when the day feed has no posts (matches `public-post-card` header styling).
const FEED_EMPTY_THEO_HANDLE: &str = "theo";
const FEED_POSTS_PER_PAGE: usize = 10;

fn noms_wallet_aria_label(c: &i18n::Catalog, loc: &str, balance: u16) -> String {
    c.tf(
        loc,
        "noms.wallet_aria",
        &[
            ("balance", &balance.to_string()),
            ("cap", &noms::NOMS_MAX.to_string()),
        ],
    )
}

pub(crate) fn topbar_noms_html(c: &i18n::Catalog, loc: &str, balance: u16) -> String {
    snippets::topbar_ai_tokens_html(
        balance,
        &noms_wallet_aria_label(c, loc, balance),
        &nom_icon::currency_tooltip(c, loc),
    )
}

/// Wallet chip in the secured topbar — only for fully verified users (not onboarding).
pub(crate) async fn topbar_noms_html_for_user(
    pool: &sqlx::MySqlPool,
    user_id: u64,
    c: &i18n::Catalog,
    loc: &str,
) -> Result<String, sqlx::Error> {
    if !crate::status::user_has_verified_status(pool, user_id).await?
        || !crate::status::user_meets_composite_verification(pool, user_id).await?
    {
        return Ok(String::new());
    }
    let balance = human_onboarding::ai_tokens_for_user(pool, user_id).await?;
    Ok(topbar_noms_html(c, loc, balance))
}

async fn empty_feed_day_placeholder_html(
    pool: &sqlx::MySqlPool,
    locale: &str,
    c: &i18n::Catalog,
) -> Result<String, sqlx::Error> {
    let avatar_path: Option<String> = sqlx::query_scalar::<_, Option<String>>(
        r#"
        SELECT uf.generated_artwork_path
        FROM social_entity se
        INNER JOIN ai_actor aa ON aa.social_entity_id = se.id
        LEFT JOIN user_faces uf ON uf.user_id = aa.user_id
        WHERE se.handle = ?
        LIMIT 1
        "#,
    )
    .bind(FEED_EMPTY_THEO_HANDLE)
    .fetch_optional(pool)
    .await?
    .flatten();

    let avatar_inner = match avatar_path
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        Some(p) => format!(
            r#"<img class="public-post-avatar-img" src="/{}" alt="@{}" loading="lazy" decoding="async" />"#,
            escape_html(p.trim_start_matches('/')),
            escape_html(FEED_EMPTY_THEO_HANDLE)
        ),
        None => format!(
            r#"<img class="public-post-avatar-img" src="/static/generated/council/theo/avatar-default.png" alt="@{}" loading="lazy" decoding="async" />"#,
            escape_html(FEED_EMPTY_THEO_HANDLE)
        ),
    };

    let handle_line = format!(r#"@{}"#, escape_html(FEED_EMPTY_THEO_HANDLE));
    let aria = c.tf(
        locale,
        "ui.home.feed_empty_profile_aria",
        &[("handle", FEED_EMPTY_THEO_HANDLE)],
    );
    let msg = c.t(locale, "ui.home.feed_empty_message");
    let handle_esc = escape_html(FEED_EMPTY_THEO_HANDLE);

    Ok(format!(
        r#"<section class="card app-card public-post-card public-post-card--text{accent}" data-feed-empty="1"><div class="card-accent" role="presentation"></div><div class="public-post-head"><a class="public-post-avatar-link feed-profile-link" href="/home/profile/{handle}" data-profile-handle="{handle}" aria-label="{aria}"><span class="brand-frame brand-frame--toolbar public-post-avatar">{avatar}</span></a><a class="public-post-handle feed-profile-link" href="/home/profile/{handle}" data-profile-handle="{handle}">{handle_line}</a></div><div class="public-post-body create-post-editor public-post-body--plain-fallback" role="article"><p class="sub">{msg}</p></div></section>"#,
        accent = entity_type_accent::profile_card_accent_class(ref_keys::KEY_ENTITY_TYPE_DIGITAL),
        handle = handle_esc,
        avatar = avatar_inner,
        handle_line = handle_line,
        aria = escape_html(&aria),
        msg = escape_html(&msg),
    ))
}

fn country_code_to_flag_emoji(raw: &str) -> Option<String> {
    let code = raw.trim().to_ascii_uppercase();
    if code.len() != 2 || !code.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let mut out = String::new();
    for ch in code.chars() {
        let base = 0x1F1E6u32;
        let offset = (ch as u32).saturating_sub('A' as u32);
        out.push(char::from_u32(base + offset)?);
    }
    Some(out)
}

#[derive(Debug, Deserialize)]
struct JurorVoteForm {
    vote: String,
}

async fn verify_pending_juror_invite_case(
    pool: &sqlx::MySqlPool,
    invite_id: u64,
    user_id: u64,
) -> Result<Option<u64>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT jc.id
        FROM judicial_case_juror_invite jcji
        INNER JOIN judicial_case jc ON jc.id = jcji.judicial_case_id
        INNER JOIN ref_key_values rk_inv
            ON rk_inv.id = jcji.invite_status_id
           AND rk_inv.`group` = ?
           AND rk_inv.`key` = ?
           AND rk_inv.is_active = 1
        WHERE jcji.id = ?
          AND jcji.juror_user_id = ?
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_JUROR_INVITE_STATUS)
    .bind(ref_keys::KEY_JCJI_PENDING)
    .bind(invite_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

async fn build_juror_workspace_html(
    pool: &sqlx::MySqlPool,
    user_id: u64,
    invite_id: u64,
    case_id: u64,
    c: &i18n::Catalog,
    loc: &str,
) -> Result<String, sqlx::Error> {
    let trigger_key: String = sqlx::query_scalar(
        r#"
        SELECT rk.`key`
        FROM judicial_case jc
        INNER JOIN ref_key_values rk
            ON rk.id = jc.trigger_type_id
           AND rk.`group` = ?
           AND rk.is_active = 1
        WHERE jc.id = ?
        "#,
    )
    .bind(crate::ref_keys::GROUP_JUDICIAL_CASE_TRIGGER)
    .bind(case_id)
    .fetch_one(pool)
    .await?;

    if trigger_key == crate::ref_keys::KEY_JUDICIAL_TRIGGER_CONSTITUTIONAL_AMENDMENT {
        return build_constitutional_juror_workspace_html(pool, invite_id, case_id, c, loc).await;
    }
    if trigger_key == crate::ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_PROFILE_AMENDMENT {
        return build_profile_amendment_juror_workspace_html(pool, invite_id, case_id, c, loc).await;
    }
    if trigger_key == crate::ref_keys::KEY_JUDICIAL_TRIGGER_MEMBERSHIP_REVIEW {
        return build_membership_review_juror_workspace_html(pool, invite_id, case_id, c, loc).await;
    }
    if trigger_key == crate::ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_REPARENT {
        return build_reparent_juror_workspace_html(pool, invite_id, case_id, c, loc).await;
    }

    let Some(feed_row) =
        feed::fetch_root_post_row_for_pending_juror_invite(pool, user_id, invite_id).await?
    else {
        return Ok(String::new());
    };
    let corpus = crate::ui::post_card::juror_corpus_post_card_html(&feed_row, c, loc);

    let origin: Option<(String, String)> = sqlx::query_as(
        r#"
        SELECT COALESCE(NULLIF(TRIM(jcc.body_html_sanitized), ''), ''),
               COALESCE(jcc.body_plain, '')
        FROM judicial_case_comment jcc
        INNER JOIN ref_key_values rk
            ON rk.id = jcc.type_id
           AND rk.`group` = ?
           AND rk.`key` = ?
           AND rk.is_active = 1
        WHERE jcc.judicial_case_id = ?
        ORDER BY jcc.id ASC
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_COMMENT_TYPE)
    .bind(ref_keys::KEY_JCC_ORIGINATOR)
    .bind(case_id)
    .fetch_optional(pool)
    .await?;

    let origin_inner = if let Some((html, plain)) = origin {
        let inner = juror_facing_submission_html(c, loc, &html, &plain);
        if inner.trim().is_empty() {
            format!(
                "<p class=\"sub\">{}</p>",
                escape_html(&c.t(loc, "ui.juror_workspace.origin_missing"))
            )
        } else {
            inner
        }
    } else {
        format!(
            "<p class=\"sub\">{}</p>",
            escape_html(&c.t(loc, "ui.juror_workspace.origin_missing"))
        )
    };

    let clauses: Vec<(u16, String)> = sqlx::query_as(
        r#"
        SELECT cci.point, cci.item
        FROM judicial_case_constitution_item jcci
        INNER JOIN community_constitution_item cci ON cci.id = jcci.constitution_item_id
        WHERE jcci.judicial_case_id = ?
        ORDER BY cci.point ASC, cci.id ASC
        "#,
    )
    .bind(case_id)
    .fetch_all(pool)
    .await?;

    let mut rules_html = String::new();
    for (point, item) in &clauses {
        rules_html.push_str(&format!(
            r#"<li><span class="juror-rule-point mono">#{}</span> {}</li>"#,
            point,
            escape_html(item)
        ));
    }
    if rules_html.is_empty() {
        rules_html = format!(
            "<li class=\"sub\">{}</li>",
            escape_html(&c.t(loc, "ui.juror_workspace.rules_empty"))
        );
    }

    let deliberation_rows: Vec<(String, String, String)> = sqlx::query_as(
        r#"
        SELECT rk.`key`, COALESCE(NULLIF(TRIM(jcc.body_html_sanitized), ''), ''), COALESCE(jcc.body_plain, '')
        FROM judicial_case_comment jcc
        INNER JOIN ref_key_values rk
            ON rk.id = jcc.type_id
           AND rk.`group` = ?
           AND rk.`key` IN (?, ?)
           AND rk.is_active = 1
        WHERE jcc.judicial_case_id = ?
        ORDER BY jcc.id ASC
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_COMMENT_TYPE)
    .bind(ref_keys::KEY_JCC_EVIDENCE)
    .bind(ref_keys::KEY_JCC_REBUTTAL)
    .bind(case_id)
    .fetch_all(pool)
    .await?;

    let mut delib_html = String::new();
    for (type_key, html, plain) in deliberation_rows {
        let label = if type_key == ref_keys::KEY_JCC_REBUTTAL {
            c.t(loc, "ui.juror_workspace.rebuttal_label")
        } else {
            c.t(loc, "ui.juror_workspace.evidence_label")
        };
        let body = juror_facing_submission_html(c, loc, &html, &plain);
        delib_html.push_str(&format!(
            r#"<article class="juror-delib-item"><header class="juror-delib-item-head">{}</header><div class="juror-delib-body create-post-editor">{}</div></article>"#,
            escape_html(&label),
            body
        ));
    }
    if delib_html.is_empty() {
        delib_html = format!(
            "<p class=\"sub\">{}</p>",
            escape_html(&c.t(loc, "ui.juror_workspace.deliberation_empty"))
        );
    }

    let gid = format!("inv{}", invite_id);
    let thumb_up = snippets::brand_vote_thumb_up_svg(&gid);
    let thumb_down = snippets::brand_vote_thumb_down_svg(&gid);
    let vote_violates = escape_html(&c.t(loc, "ui.juror_workspace.vote_violates"));
    let vote_clears = escape_html(&c.t(loc, "ui.juror_workspace.vote_clears"));
    let h_safety = escape_html(&c.t(loc, "ui.juror_workspace.safety_instructions_heading"));
    let safety_body = escape_html(&c.t(loc, "ui.juror_workspace.safety_instructions_body"));

    Ok(format!(
        r#"<div class="juror-workspace" data-juror-workspace="1" data-invite-id="{invite_id}">
  <section class="juror-workspace-section juror-workspace-section--corpus">{corpus}</section>
  <section class="juror-workspace-section"><h3 class="juror-workspace-h">{h_origin}</h3><div class="juror-workspace-block create-post-editor">{origin_inner}</div></section>
  <section class="juror-workspace-section juror-workspace-section--rules"><h3 class="juror-workspace-h">{h_rules}</h3><ol class="juror-rule-list">{rules_html}</ol></section>
  <section class="juror-workspace-section juror-workspace-section--safety"><h3 class="juror-workspace-h">{h_safety}</h3><p class="sub">{safety_body}</p></section>
  <section class="juror-workspace-section"><h3 class="juror-workspace-h">{h_delib}</h3><div class="juror-delib-feed">{delib_html}</div></section>
  <section class="juror-workspace-section juror-workspace-section--vote">
    <h3 class="juror-workspace-h">{h_vote}</h3>
    <p class="sub juror-vote-question">{vote_q}</p>
    <div class="juror-vote-actions" role="group" aria-label="{h_vote}">
      <form class="juror-vote-form juror-vote-form--violates" method="post" action="/home/juror-invite/{invite_id}/vote" data-juror-vote-form="1">
        <input type="hidden" name="vote" value="violates"/>
        <button type="submit" class="juror-vote-btn juror-vote-btn--split juror-vote-btn--violates" aria-label="{vote_violates}">{thumb_up}<span class="juror-vote-btn-label">{vote_violates}</span></button>
      </form>
      <form class="juror-vote-form juror-vote-form--clears" method="post" action="/home/juror-invite/{invite_id}/vote" data-juror-vote-form="1">
        <input type="hidden" name="vote" value="clears"/>
        <button type="submit" class="juror-vote-btn juror-vote-btn--split juror-vote-btn--clears" aria-label="{vote_clears}">{thumb_down}<span class="juror-vote-btn-label">{vote_clears}</span></button>
      </form>
    </div>
  </section>
</div>"#,
        invite_id = invite_id,
        corpus = corpus,
        h_origin = escape_html(&c.t(loc, "ui.juror_workspace.origin_heading")),
        origin_inner = origin_inner,
        h_rules = escape_html(&c.t(loc, "ui.juror_workspace.rules_heading")),
        rules_html = rules_html,
        h_delib = escape_html(&c.t(loc, "ui.juror_workspace.deliberation_heading")),
        delib_html = delib_html,
        h_vote = escape_html(&c.t(loc, "ui.juror_workspace.vote_heading")),
        vote_q = escape_html(&c.t(loc, "ui.juror_workspace.vote_question")),
        thumb_down = thumb_down,
        thumb_up = thumb_up,
        vote_violates = vote_violates,
        vote_clears = vote_clears,
        h_safety = h_safety,
        safety_body = safety_body,
    ))
}

async fn build_constitutional_juror_workspace_html(
    pool: &sqlx::MySqlPool,
    invite_id: u64,
    case_id: u64,
    c: &i18n::Catalog,
    loc: &str,
) -> Result<String, sqlx::Error> {
    let Some(amendment) =
        crate::social::judicial_constitutional::fetch_constitutional_amendment_for_case(pool, case_id)
            .await?
    else {
        return Ok(String::new());
    };

    let kind_label = match amendment.amendment_kind.as_str() {
        "amend_clause" => c.tf(
            loc,
            "ui.juror_workspace.constitutional_kind_amend",
            &[("point", &amendment.target_point.unwrap_or(0).to_string())],
        ),
        "remove_clause" => c.tf(
            loc,
            "ui.juror_workspace.constitutional_kind_remove",
            &[("point", &amendment.target_point.unwrap_or(0).to_string())],
        ),
        _ => c.t(loc, "ui.juror_workspace.constitutional_kind_new"),
    };

    let proposal_body = juror_facing_submission_html(
        c,
        loc,
        &amendment.proposed_body_html_sanitized,
        &amendment.proposed_body_plain,
    );

    let target_clause = amendment
        .target_item
        .as_deref()
        .filter(|t| !t.trim().is_empty())
        .map(|item| {
            format!(
                r#"<p class="sub juror-constitutional-target"><span class="mono">#{}</span> {}</p>"#,
                amendment.target_point.unwrap_or(0),
                crate::html_escape::escape_html(item.trim())
            )
        })
        .unwrap_or_default();

    let corpus = format!(
        r#"<article class="card app-card judicial-post-case-card juror-constitutional-corpus">
  <div class="card-accent" role="presentation"></div>
  <h3 class="juror-workspace-h">{}</h3>
  <p class="juror-constitutional-kind"><strong>{}</strong></p>
  {target_clause}
  <div class="juror-workspace-block create-post-editor">{proposal_body}</div>
</article>"#,
        crate::html_escape::escape_html(&c.t(loc, "ui.juror_workspace.constitutional_corpus_heading")),
        crate::html_escape::escape_html(&kind_label),
        target_clause = target_clause,
        proposal_body = proposal_body,
    );

    let origin: Option<(String, String)> = sqlx::query_as(
        r#"
        SELECT COALESCE(NULLIF(TRIM(jcc.body_html_sanitized), ''), ''),
               COALESCE(jcc.body_plain, '')
        FROM judicial_case_comment jcc
        INNER JOIN ref_key_values rk
            ON rk.id = jcc.type_id
           AND rk.`group` = ?
           AND rk.`key` = ?
           AND rk.is_active = 1
        WHERE jcc.judicial_case_id = ?
        ORDER BY jcc.id ASC
        LIMIT 1
        "#,
    )
    .bind(crate::ref_keys::GROUP_JUDICIAL_CASE_COMMENT_TYPE)
    .bind(crate::ref_keys::KEY_JCC_ORIGINATOR)
    .bind(case_id)
    .fetch_optional(pool)
    .await?;

    let origin_inner = if let Some((html, plain)) = origin {
        let inner = juror_facing_submission_html(c, loc, &html, &plain);
        if inner.trim().is_empty() {
            format!(
                "<p class=\"sub\">{}</p>",
                crate::html_escape::escape_html(
                    &c.t(loc, "ui.juror_workspace.constitutional_origin_missing"),
                )
            )
        } else {
            inner
        }
    } else {
        format!(
            "<p class=\"sub\">{}</p>",
            crate::html_escape::escape_html(&c.t(loc, "ui.juror_workspace.constitutional_origin_missing"))
        )
    };

    let gid = format!("inv{}", invite_id);
    let thumb_up = snippets::brand_vote_thumb_up_svg(&gid);
    let thumb_down = snippets::brand_vote_thumb_down_svg(&gid);
    let vote_accept = crate::html_escape::escape_html(&c.t(loc, "ui.juror_workspace.constitutional_vote_accept"));
    let vote_reject = crate::html_escape::escape_html(&c.t(loc, "ui.juror_workspace.constitutional_vote_reject"));

    Ok(format!(
        r#"<div class="juror-workspace" data-juror-workspace="1" data-invite-id="{invite_id}">
  <section class="juror-workspace-section juror-workspace-section--corpus">{corpus}</section>
  <section class="juror-workspace-section"><h3 class="juror-workspace-h">{h_origin}</h3><div class="juror-workspace-block create-post-editor">{origin_inner}</div></section>
  <section class="juror-workspace-section juror-workspace-section--vote">
    <h3 class="juror-workspace-h">{h_vote}</h3>
    <p class="sub juror-vote-question">{vote_q}</p>
    <div class="juror-vote-actions" role="group" aria-label="{h_vote}">
      <form class="juror-vote-form juror-vote-form--violates" method="post" action="/home/juror-invite/{invite_id}/vote" data-juror-vote-form="1">
        <input type="hidden" name="vote" value="violates"/>
        <button type="submit" class="juror-vote-btn juror-vote-btn--split juror-vote-btn--violates" aria-label="{vote_accept}">{thumb_up}<span class="juror-vote-btn-label">{vote_accept}</span></button>
      </form>
      <form class="juror-vote-form juror-vote-form--clears" method="post" action="/home/juror-invite/{invite_id}/vote" data-juror-vote-form="1">
        <input type="hidden" name="vote" value="clears"/>
        <button type="submit" class="juror-vote-btn juror-vote-btn--split juror-vote-btn--clears" aria-label="{vote_reject}">{thumb_down}<span class="juror-vote-btn-label">{vote_reject}</span></button>
      </form>
    </div>
  </section>
</div>"#,
        invite_id = invite_id,
        corpus = corpus,
        h_origin = crate::html_escape::escape_html(
            &c.t(loc, "ui.juror_workspace.constitutional_origin_heading"),
        ),
        origin_inner = origin_inner,
        h_vote = crate::html_escape::escape_html(&c.t(loc, "ui.juror_workspace.vote_heading")),
        vote_q = crate::html_escape::escape_html(
            &c.t(loc, "ui.juror_workspace.constitutional_vote_question"),
        ),
        vote_accept = vote_accept,
        vote_reject = vote_reject,
        thumb_up = thumb_up,
        thumb_down = thumb_down,
    ))
}

async fn build_profile_amendment_juror_workspace_html(
    pool: &sqlx::MySqlPool,
    invite_id: u64,
    case_id: u64,
    c: &i18n::Catalog,
    loc: &str,
) -> Result<String, sqlx::Error> {
    let Some(amendment) =
        crate::social::judicial_community_profile::fetch_profile_amendment_for_case(pool, case_id)
            .await?
    else {
        return Ok(String::new());
    };

    let field_label = |key: &str| -> String {
        match key {
            "description" => c.t(loc, "ui.juror_workspace.profile_field_about"),
            "intention" => c.t(loc, "ui.juror_workspace.profile_field_intention"),
            _ => c.t(loc, "ui.juror_workspace.profile_field_join_policy"),
        }
    };

    let mut corpus = String::from(
        r#"<article class="card app-card judicial-post-case-card juror-profile-amendment-corpus"><div class="card-accent" role="presentation"></div><h3 class="juror-workspace-h">"#,
    );
    corpus.push_str(&escape_html(
        &c.t(loc, "ui.juror_workspace.profile_amendment_corpus_heading"),
    ));
    corpus.push_str("</h3>");

    if amendment.change_description {
        corpus.push_str(&profile_amendment_field_diff_html(
            c,
            loc,
            &field_label("description"),
            &amendment.current.description_plain,
            &amendment.proposed.description_html_sanitized,
            &amendment.proposed.description_plain,
        ));
    }
    if amendment.change_intention {
        corpus.push_str(&profile_amendment_field_diff_html(
            c,
            loc,
            &field_label("intention"),
            &amendment.current.intention_plain,
            &amendment.proposed.intention_html_sanitized,
            &amendment.proposed.intention_plain,
        ));
    }
    if amendment.change_join_policy {
        let current = amendment
            .current
            .join_policy_key
            .as_deref()
            .map(|k| crate::ui::join_policy::community_join_policy_display(c, loc, k))
            .unwrap_or_else(|| c.t(loc, "ui.juror_workspace.profile_join_policy_unset"));
        let proposed = amendment
            .proposed
            .join_policy_key
            .as_deref()
            .map(|k| crate::ui::join_policy::community_join_policy_display(c, loc, k))
            .unwrap_or_default();
        let current_lbl = escape_html(&c.t(loc, "ui.juror_workspace.profile_diff_current"));
        let proposed_lbl = escape_html(&c.t(loc, "ui.juror_workspace.profile_diff_proposed"));
        corpus.push_str(&format!(
            r#"<section class="juror-profile-field"><h4 class="juror-workspace-h">{label}</h4><p class="sub juror-profile-diff-line"><span class="juror-profile-diff-k">{current_lbl}</span> {current}</p><p class="sub juror-profile-diff-line"><span class="juror-profile-diff-k">{proposed_lbl}</span> {proposed}</p></section>"#,
            label = escape_html(&field_label("join_policy")),
            current_lbl = current_lbl,
            current = escape_html(&current),
            proposed_lbl = proposed_lbl,
            proposed = escape_html(&proposed),
        ));
    }
    corpus.push_str("</article>");

    let origin: Option<(String, String)> = sqlx::query_as(
        r#"
        SELECT COALESCE(NULLIF(TRIM(jcc.body_html_sanitized), ''), ''),
               COALESCE(jcc.body_plain, '')
        FROM judicial_case_comment jcc
        INNER JOIN ref_key_values rk
            ON rk.id = jcc.type_id
           AND rk.`group` = ?
           AND rk.`key` = ?
           AND rk.is_active = 1
        WHERE jcc.judicial_case_id = ?
        ORDER BY jcc.id ASC
        LIMIT 1
        "#,
    )
    .bind(crate::ref_keys::GROUP_JUDICIAL_CASE_COMMENT_TYPE)
    .bind(crate::ref_keys::KEY_JCC_ORIGINATOR)
    .bind(case_id)
    .fetch_optional(pool)
    .await?;

    let origin_inner = if let Some((html, plain)) = origin {
        let inner = juror_facing_submission_html(c, loc, &html, &plain);
        if inner.trim().is_empty() {
            format!(
                "<p class=\"sub\">{}</p>",
                escape_html(&c.t(loc, "ui.juror_workspace.constitutional_origin_missing"))
            )
        } else {
            inner
        }
    } else {
        format!(
            "<p class=\"sub\">{}</p>",
            escape_html(&c.t(loc, "ui.juror_workspace.constitutional_origin_missing"))
        )
    };

    let gid = format!("inv{}", invite_id);
    let thumb_up = snippets::brand_vote_thumb_up_svg(&gid);
    let thumb_down = snippets::brand_vote_thumb_down_svg(&gid);
    let vote_accept = escape_html(&c.t(loc, "ui.juror_workspace.profile_vote_accept"));
    let vote_reject = escape_html(&c.t(loc, "ui.juror_workspace.profile_vote_reject"));

    Ok(format!(
        r#"<div class="juror-workspace" data-juror-workspace="1" data-invite-id="{invite_id}">
  <section class="juror-workspace-section juror-workspace-section--corpus">{corpus}</section>
  <section class="juror-workspace-section"><h3 class="juror-workspace-h">{h_origin}</h3><div class="juror-workspace-block create-post-editor">{origin_inner}</div></section>
  <section class="juror-workspace-section juror-workspace-section--vote">
    <h3 class="juror-workspace-h">{h_vote}</h3>
    <p class="sub juror-vote-question">{vote_q}</p>
    <div class="juror-vote-actions" role="group" aria-label="{h_vote}">
      <form class="juror-vote-form juror-vote-form--violates" method="post" action="/home/juror-invite/{invite_id}/vote" data-juror-vote-form="1">
        <input type="hidden" name="vote" value="violates"/>
        <button type="submit" class="juror-vote-btn juror-vote-btn--split juror-vote-btn--violates" aria-label="{vote_accept}">{thumb_up}<span class="juror-vote-btn-label">{vote_accept}</span></button>
      </form>
      <form class="juror-vote-form juror-vote-form--clears" method="post" action="/home/juror-invite/{invite_id}/vote" data-juror-vote-form="1">
        <input type="hidden" name="vote" value="clears"/>
        <button type="submit" class="juror-vote-btn juror-vote-btn--split juror-vote-btn--clears" aria-label="{vote_reject}">{thumb_down}<span class="juror-vote-btn-label">{vote_reject}</span></button>
      </form>
    </div>
  </section>
</div>"#,
        invite_id = invite_id,
        corpus = corpus,
        h_origin = escape_html(&c.t(loc, "ui.juror_workspace.constitutional_origin_heading")),
        origin_inner = origin_inner,
        h_vote = escape_html(&c.t(loc, "ui.juror_workspace.vote_heading")),
        vote_q = escape_html(&c.t(loc, "ui.juror_workspace.profile_vote_question")),
        vote_accept = vote_accept,
        vote_reject = vote_reject,
        thumb_up = thumb_up,
        thumb_down = thumb_down,
    ))
}

fn profile_amendment_field_diff_html(
    c: &i18n::Catalog,
    loc: &str,
    label: &str,
    current_plain: &str,
    proposed_html: &str,
    proposed_plain: &str,
) -> String {
    let current_body = if current_plain.trim().is_empty() {
        format!(
            "<p class=\"sub\">{}</p>",
            escape_html(&c.t(loc, "ui.juror_workspace.profile_field_empty"))
        )
    } else {
        juror_facing_submission_html(c, loc, &format!("<p>{}</p>", escape_html(current_plain.trim())), current_plain)
    };
    let proposed_body = juror_facing_submission_html(c, loc, proposed_html, proposed_plain);
    let current_lbl = escape_html(&c.t(loc, "ui.juror_workspace.profile_diff_current"));
    let proposed_lbl = escape_html(&c.t(loc, "ui.juror_workspace.profile_diff_proposed"));
    format!(
        r#"<section class="juror-profile-field"><h4 class="juror-workspace-h">{label}</h4><p class="sub juror-profile-diff-k">{current_lbl}</p><div class="juror-workspace-block create-post-editor">{current_body}</div><p class="sub juror-profile-diff-k">{proposed_lbl}</p><div class="juror-workspace-block create-post-editor">{proposed_body}</div></section>"#,
        label = escape_html(label),
        current_lbl = current_lbl,
        current_body = current_body,
        proposed_lbl = proposed_lbl,
        proposed_body = proposed_body,
    )
}

/// Constitution diff for jurors: clause numbers and text only (no community tier handles).
fn juror_constitution_structure_diff_html(
    diff: &community_constitution::ConstitutionStructureDiff,
    c: &i18n::Catalog,
    loc: &str,
) -> String {
    if diff.added.is_empty() && diff.removed.is_empty() {
        return format!(
            r#"<section class="profile-block"><p class="profile-value">{}</p></section>"#,
            escape_html(&c.t(loc, "ui.judicial_community_reparent.diff_unchanged"))
        );
    }
    let mut body = String::from(r#"<section class="profile-block profile-constitution-block"><h2>"#);
    body.push_str(&escape_html(&c.t(loc, "ui.judicial_community_reparent.diff_heading")));
    body.push_str("</h2>");
    if !diff.removed.is_empty() {
        body.push_str(&format!(
            r#"<h3 class="profile-brand-base-size">{}</h3><ol class="profile-constitution-list">"#,
            escape_html(&c.t(loc, "ui.judicial_community_reparent.diff_removed"))
        ));
        for entry in &diff.removed {
            body.push_str(&format!(
                r#"<li><span class="mono">#{}</span> Ã¢â‚¬â€ {}</li>"#,
                entry.point,
                escape_html(&entry.item)
            ));
        }
        body.push_str("</ol>");
    }
    if !diff.added.is_empty() {
        body.push_str(&format!(
            r#"<h3 class="profile-brand-base-size">{}</h3><ol class="profile-constitution-list">"#,
            escape_html(&c.t(loc, "ui.judicial_community_reparent.diff_added"))
        ));
        for entry in &diff.added {
            body.push_str(&format!(
                r#"<li><span class="mono">#{}</span> Ã¢â‚¬â€ {}</li>"#,
                entry.point,
                escape_html(&entry.item)
            ));
        }
        body.push_str("</ol>");
    }
    body.push_str("</section>");
    body
}

fn constitution_structure_diff_html(
    diff: &community_constitution::ConstitutionStructureDiff,
    c: &i18n::Catalog,
    loc: &str,
) -> String {
    if diff.added.is_empty() && diff.removed.is_empty() {
        return format!(
            r#"<section class="profile-block"><p class="profile-value">{}</p></section>"#,
            escape_html(&c.t(loc, "ui.judicial_community_reparent.diff_unchanged"))
        );
    }
    let mut body = String::from(r#"<section class="profile-block profile-constitution-block"><h2>"#);
    body.push_str(&escape_html(&c.t(loc, "ui.judicial_community_reparent.diff_heading")));
    body.push_str("</h2>");
    if !diff.removed.is_empty() {
        body.push_str(&format!(
            r#"<h3 class="profile-brand-base-size">{}</h3><ol class="profile-constitution-list">"#,
            escape_html(&c.t(loc, "ui.judicial_community_reparent.diff_removed"))
        ));
        for entry in &diff.removed {
            body.push_str(&format!(
                r#"<li><span class="mono">@{}</span> #{} Ã¢â‚¬â€ {}</li>"#,
                escape_html(&entry.tier_handle),
                entry.point,
                escape_html(&entry.item)
            ));
        }
        body.push_str("</ol>");
    }
    if !diff.added.is_empty() {
        body.push_str(&format!(
            r#"<h3 class="profile-brand-base-size">{}</h3><ol class="profile-constitution-list">"#,
            escape_html(&c.t(loc, "ui.judicial_community_reparent.diff_added"))
        ));
        for entry in &diff.added {
            body.push_str(&format!(
                r#"<li><span class="mono">@{}</span> #{} Ã¢â‚¬â€ {}</li>"#,
                escape_html(&entry.tier_handle),
                entry.point,
                escape_html(&entry.item)
            ));
        }
        body.push_str("</ol>");
    }
    body.push_str("</section>");
    body
}

async fn build_reparent_juror_workspace_html(
    pool: &sqlx::MySqlPool,
    invite_id: u64,
    case_id: u64,
    c: &i18n::Catalog,
    loc: &str,
) -> Result<String, sqlx::Error> {
    let Some(reparent) = community_reparent::fetch_reparent_for_case(pool, case_id).await? else {
        return Ok(String::new());
    };
    let diff_block = reparent
        .constitution_diff_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<community_constitution::ConstitutionStructureDiff>(raw).ok())
        .map(|diff| juror_constitution_structure_diff_html(&diff, c, loc))
        .unwrap_or_default();
    let corpus = format!(
        r#"<article class="card app-card judicial-post-case-card juror-reparent-corpus">
  <div class="card-accent" role="presentation"></div>
  <h3 class="juror-workspace-h">{}</h3>
  <p class="sub">{}</p>
  {diff_block}
</article>"#,
        escape_html(&c.t(loc, "ui.juror_workspace.reparent_corpus_heading")),
        escape_html(&c.t(loc, "ui.juror_workspace.reparent_parent_withheld")),
        diff_block = diff_block,
    );
    let origin: Option<(String, String)> = sqlx::query_as(
        r#"
        SELECT COALESCE(NULLIF(TRIM(jcc.body_html_sanitized), ''), ''),
               COALESCE(jcc.body_plain, '')
        FROM judicial_case_comment jcc
        INNER JOIN ref_key_values rk
            ON rk.id = jcc.type_id
           AND rk.`group` = ?
           AND rk.`key` = ?
        WHERE jcc.judicial_case_id = ?
        ORDER BY jcc.id ASC
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_COMMENT_TYPE)
    .bind(ref_keys::KEY_JCC_ORIGINATOR)
    .bind(case_id)
    .fetch_optional(pool)
    .await?;
    let origin_inner = origin
        .map(|(html, plain)| {
            let inner = juror_facing_submission_html(c, loc, &html, &plain);
            if inner.trim().is_empty() {
                return String::new();
            }
            format!(
                r#"<div class="juror-workspace-block create-post-editor">{inner}</div>"#,
                inner = inner
            )
        })
        .unwrap_or_else(|| {
            format!(
                r#"<p class="sub">{}</p>"#,
                escape_html(&c.t(loc, "ui.juror_workspace.reparent_origin_missing"))
            )
        });
    let gid = format!("inv{}", invite_id);
    let thumb_up = snippets::brand_vote_thumb_up_svg(&gid);
    let thumb_down = snippets::brand_vote_thumb_down_svg(&gid);
    let vote_upheld = escape_html(&c.t(loc, "ui.juror_workspace.reparent_vote_upheld"));
    let vote_overturn = escape_html(&c.t(loc, "ui.juror_workspace.reparent_vote_overturn"));
    Ok(format!(
        r#"<div class="juror-workspace" data-juror-workspace="1" data-invite-id="{invite_id}">
  <section class="juror-workspace-section juror-workspace-section--corpus">{corpus}</section>
  <section class="juror-workspace-section">
    <h3 class="juror-workspace-h">{h_origin}</h3>
    {origin_inner}
  </section>
  <section class="juror-workspace-section juror-workspace-section--vote">
    <h3 class="juror-workspace-h">{h_vote}</h3>
    <p class="sub juror-vote-question">{vote_q}</p>
    <div class="juror-vote-actions" role="group" aria-label="{h_vote}">
      <form class="juror-vote-form juror-vote-form--violates" method="post" action="/home/juror-invite/{invite_id}/vote" data-juror-vote-form="1">
        <input type="hidden" name="vote" value="violates"/>
        <button type="submit" class="juror-vote-btn juror-vote-btn--split juror-vote-btn--violates" aria-label="{vote_upheld}">{thumb_up}<span class="juror-vote-btn-label">{vote_upheld}</span></button>
      </form>
      <form class="juror-vote-form juror-vote-form--clears" method="post" action="/home/juror-invite/{invite_id}/vote" data-juror-vote-form="1">
        <input type="hidden" name="vote" value="clears"/>
        <button type="submit" class="juror-vote-btn juror-vote-btn--split juror-vote-btn--clears" aria-label="{vote_overturn}">{thumb_down}<span class="juror-vote-btn-label">{vote_overturn}</span></button>
      </form>
      </div>
  </section>
</div>"#,
        invite_id = invite_id,
        corpus = corpus,
        h_origin = escape_html(&c.t(loc, "ui.juror_workspace.reparent_origin_heading")),
        origin_inner = origin_inner,
        h_vote = escape_html(&c.t(loc, "ui.juror_workspace.vote_heading")),
        vote_q = escape_html(&c.t(loc, "ui.juror_workspace.reparent_vote_question")),
        vote_upheld = vote_upheld,
        vote_overturn = vote_overturn,
        thumb_up = thumb_up,
        thumb_down = thumb_down,
    ))
}

async fn build_membership_review_juror_workspace_html(
    pool: &sqlx::MySqlPool,
    invite_id: u64,
    case_id: u64,
    c: &i18n::Catalog,
    loc: &str,
) -> Result<String, sqlx::Error> {
    let Some(application) =
        judicial::membership_review::fetch_application_for_case(pool, case_id).await?
    else {
        return Ok(String::new());
    };

    let statement_body = juror_facing_submission_html(
        c,
        loc,
        &application.application_statement_html,
        &application.application_statement_plain,
    );

    let corpus = format!(
        r#"<article class="card app-card judicial-post-case-card juror-membership-review-corpus">
  <div class="card-accent" role="presentation"></div>
  <h3 class="juror-workspace-h">{}</h3>
  <div class="juror-workspace-block create-post-editor">{statement_body}</div>
</article>"#,
        crate::html_escape::escape_html(&c.t(loc, "ui.juror_workspace.membership_review_corpus_heading")),
        statement_body = statement_body,
    );

    let gid = format!("inv{}", invite_id);
    let thumb_up = snippets::brand_vote_thumb_up_svg(&gid);
    let thumb_down = snippets::brand_vote_thumb_down_svg(&gid);
    let vote_admit =
        crate::html_escape::escape_html(&c.t(loc, "ui.juror_workspace.membership_review_vote_admit"));
    let vote_deny =
        crate::html_escape::escape_html(&c.t(loc, "ui.juror_workspace.membership_review_vote_deny"));

    Ok(format!(
        r#"<div class="juror-workspace" data-juror-workspace="1" data-invite-id="{invite_id}">
  <section class="juror-workspace-section juror-workspace-section--corpus">{corpus}</section>
  <section class="juror-workspace-section juror-workspace-section--vote">
    <h3 class="juror-workspace-h">{h_vote}</h3>
    <p class="sub juror-vote-question">{vote_q}</p>
    <div class="juror-vote-actions" role="group" aria-label="{h_vote}">
      <form class="juror-vote-form juror-vote-form--violates" method="post" action="/home/juror-invite/{invite_id}/vote" data-juror-vote-form="1">
        <input type="hidden" name="vote" value="violates"/>
        <button type="submit" class="juror-vote-btn juror-vote-btn--split juror-vote-btn--violates" aria-label="{vote_admit}">{thumb_up}<span class="juror-vote-btn-label">{vote_admit}</span></button>
      </form>
      <form class="juror-vote-form juror-vote-form--clears" method="post" action="/home/juror-invite/{invite_id}/vote" data-juror-vote-form="1">
        <input type="hidden" name="vote" value="clears"/>
        <button type="submit" class="juror-vote-btn juror-vote-btn--split juror-vote-btn--clears" aria-label="{vote_deny}">{thumb_down}<span class="juror-vote-btn-label">{vote_deny}</span></button>
      </form>
    </div>
  </section>
</div>"#,
        invite_id = invite_id,
        corpus = corpus,
        h_vote = crate::html_escape::escape_html(&c.t(loc, "ui.juror_workspace.vote_heading")),
        vote_q = crate::html_escape::escape_html(
            &c.t(loc, "ui.juror_workspace.membership_review_vote_question"),
        ),
        vote_admit = vote_admit,
        vote_deny = vote_deny,
        thumb_up = thumb_up,
        thumb_down = thumb_down,
    ))
}

async fn home_outstanding_panel(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
) -> AppResult<Html<String>> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let mut html = String::new();
    let invitations = pending_invitations::list_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    for inv in &invitations {
        html.push_str(&outstanding_invitations::invitation_card_html(inv, &loc, &c));
    }
    if let Some(outcome_row) =
        judicial::membership_review::latest_applicant_outcome_for_viewer(&state.db, user_id)
            .await
            .map_err(AppError::Db)?
    {
        html.push_str(&membership_review_applicant_outcome_card_html(
            &outcome_row, &c, &loc,
        ));
    }
    let Some(inv) = judicial::find_pending_human_jury_invite_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?
    else {
        return Ok(Html(html));
    };
    let used = judicial::count_recent_abstentions_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let remaining = judicial::MAX_FREE_ABSTENTIONS_PER_YEAR.saturating_sub(used);
    let is_severe = inv.is_severe_case;
    let is_constitutional =
        inv.trigger_key == crate::ref_keys::KEY_JUDICIAL_TRIGGER_CONSTITUTIONAL_AMENDMENT;
    let is_membership_review =
        inv.trigger_key == crate::ref_keys::KEY_JUDICIAL_TRIGGER_MEMBERSHIP_REVIEW;
    let community_generic = c.t(&loc, "ui.juror_workspace.summons_community_generic");
    let intro = if is_constitutional {
        c.tf(
            &loc,
            "ui.juror_constitutional_summons.intro",
            &[
                ("community", &community_generic),
                ("cap", &judicial::MAX_FREE_ABSTENTIONS_PER_YEAR.to_string()),
                ("remaining", &remaining.to_string()),
            ],
        )
    } else if is_membership_review {
        c.tf(
            &loc,
            "ui.juror_membership_review_summons.intro",
            &[
                ("community", &community_generic),
                ("cap", &judicial::MAX_FREE_ABSTENTIONS_PER_YEAR.to_string()),
                ("remaining", &remaining.to_string()),
            ],
        )
    } else if is_severe {
        c.t(&loc, "ui.juror_summons.intro_severe")
    } else {
        c.tf(
            &loc,
            "ui.juror_summons.intro",
            &[
                ("cap", &judicial::MAX_FREE_ABSTENTIONS_PER_YEAR.to_string()),
                ("remaining", &remaining.to_string()),
            ],
        )
    };
    let severity_notice = if is_severe {
        format!(
            r#"<p class="juror-summons-severity juror-summons-severity--acute" role="note">{}</p>"#,
            escape_html(&c.t(&loc, "ui.juror_summons.severity_acute_notice"))
        )
    } else {
        String::new()
    };
    let rating_warning = if !is_severe && remaining == 0 {
        format!(
            r#"<p class="juror-summons-rating-warning" role="note">{}</p>"#,
            escape_html(&c.t(&loc, "ui.juror_summons.abstain_rating_warning"))
        )
    } else {
        String::new()
    };
    let title_key = if is_constitutional {
        "ui.juror_constitutional_summons.title"
    } else if is_membership_review {
        "ui.juror_membership_review_summons.title"
    } else {
        "ui.juror_summons.title"
    };
    let title = escape_html(&c.t(&loc, title_key));
    let respond_prompt = escape_html(&c.t(&loc, "ui.juror_summons.respond_prompt"));
    let accept_lbl = escape_html(&c.t(&loc, "ui.juror_summons.accept"));
    let abstain_lbl = escape_html(&c.t(&loc, "ui.juror_summons.abstain"));
    html.push_str(&format!(
        r#"<section class="card app-card juror-summons-card judicial-post-case-card" data-juror-summons="1" data-invite-id="{invite_id}" data-case-id="{case_id}">
  <div class="card-accent" role="presentation"></div>
  <h2 class="juror-summons-title">{title}</h2>
  {severity_notice}
  <p class="juror-summons-intro">{intro}</p>
  {rating_warning}
  <div class="juror-summons-actions">
    <section class="profile-block profile-connect-block juror-summons-connect-block">
      <div class="profile-connect-inline profile-connect-inline--solo">
        <select id="juror-summons-response-{invite_id}" class="field-input profile-connect-select profile-connect-select--solo" data-juror-summons-select="1" aria-label="{aria_respond}">
          <option value="">{respond_prompt}</option>
          <option value="accept">{accept_lbl}</option>
          <option value="abstain">{abstain_lbl}</option>
        </select>
      </div>
    </section>
  </div>
  <div class="juror-summons-workspace" data-juror-workspace-mount="1" hidden></div>
</section>"#,
        invite_id = inv.invite_id,
        case_id = inv.case_id,
        title = title,
        severity_notice = severity_notice,
        intro = escape_html(&intro),
        rating_warning = rating_warning,
        respond_prompt = respond_prompt,
        accept_lbl = accept_lbl,
        abstain_lbl = abstain_lbl,
        aria_respond = respond_prompt,
    ));
    Ok(Html(html))
}

async fn home_juror_invite_workspace(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(invite_id): Path<u64>,
) -> AppResult<Html<String>> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let Some(case_id) = verify_pending_juror_invite_case(&state.db, invite_id, user_id)
        .await
        .map_err(AppError::Db)?
    else {
        return Err(AppError::NotFound);
    };
    let inner = build_juror_workspace_html(&state.db, user_id, invite_id, case_id, &c, &loc)
        .await
        .map_err(AppError::Db)?;
    if inner.is_empty() {
        return Err(AppError::NotFound);
    }
    Ok(Html(inner))
}

async fn home_juror_invite_abstain(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(invite_id): Path<u64>,
) -> AppResult<Json<serde_json::Value>> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    match judicial::abstain_from_invite(&state.db, user_id, invite_id)
        .await
        .map_err(AppError::Db)?
    {
        judicial::AbstainOutcome::Free { .. } | judicial::AbstainOutcome::OverFreeQuota { .. } => {
            Ok(Json(serde_json::json!({ "ok": true })))
        }
        judicial::AbstainOutcome::NotInvited => Err(AppError::NotFound),
        judicial::AbstainOutcome::AlreadyResponded => Err(AppError::BadRequest(
            c.t(&loc, "ui.juror_summons.already_responded"),
        )),
    }
}

async fn home_juror_invite_decline(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(invite_id): Path<u64>,
) -> AppResult<Json<serde_json::Value>> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    match judicial::decline_from_invite(&state.db, user_id, invite_id)
        .await
        .map_err(AppError::Db)?
    {
        judicial::DeclineOutcome::Recorded => Ok(Json(serde_json::json!({ "ok": true }))),
        judicial::DeclineOutcome::NotInvited => Err(AppError::NotFound),
        judicial::DeclineOutcome::AlreadyResponded => Err(AppError::BadRequest(
            c.t(&loc, "ui.juror_summons.already_responded"),
        )),
    }
}

fn membership_review_applicant_outcome_card_html(
    row: &crate::social::judicial::membership_review::MembershipReviewApplicantOutcomeFeed,
    c: &i18n::Catalog,
    loc: &str,
) -> String {
    use crate::social::judicial::MembershipReviewApplicantOutcome;

    let community_at = format!("@{}", row.community_handle);
    let (title_key, intro_key) = match row.outcome {
        MembershipReviewApplicantOutcome::Denied => (
            "ui.membership_review_applicant_outcome.denied_title",
            "ui.membership_review_applicant_outcome.denied_intro",
        ),
        MembershipReviewApplicantOutcome::NoJury => (
            "ui.membership_review_applicant_outcome.no_jury_title",
            "ui.membership_review_applicant_outcome.no_jury_intro",
        ),
    };
    let title = crate::html_escape::escape_html(&c.t(loc, title_key));
    let intro = crate::html_escape::escape_html(&c.tf(
        loc,
        intro_key,
        &[
            ("community", community_at.as_str()),
            ("label", row.community_label.as_str()),
        ],
    ));
    format!(
        r#"<section class="card app-card juror-summons-card membership-review-outcome-card" data-membership-review-outcome="1" data-case-id="{case_id}">
  <div class="card-accent" role="presentation"></div>
  <h2 class="juror-summons-title">{title}</h2>
  <p class="juror-summons-intro">{intro}</p>
</section>"#,
        case_id = row.case_id,
        title = title,
        intro = intro,
    )
}

async fn home_juror_invite_vote(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(invite_id): Path<u64>,
    Form(body): Form<JurorVoteForm>,
) -> AppResult<Json<serde_json::Value>> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let direction = match body.vote.trim() {
        "violates" => ref_keys::KEY_JCJI_VOTE_FOR,
        "clears" => ref_keys::KEY_JCJI_VOTE_AGAINST,
        _ => {
            return Err(AppError::BadRequest(
                c.t(&loc, "ui.juror_workspace.vote_invalid"),
            ))
        }
    };
    match judicial::cast_vote(&state.db, user_id, invite_id, direction)
        .await
        .map_err(AppError::Db)?
    {
        judicial::CastVoteOutcome::Recorded => Ok(Json(serde_json::json!({ "ok": true }))),
        judicial::CastVoteOutcome::NotInvited => Err(AppError::NotFound),
        judicial::CastVoteOutcome::AlreadyResponded => Err(AppError::BadRequest(
            c.t(&loc, "ui.juror_summons.already_responded"),
        )),
    }
}

async fn home_feed_day(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Query(q): Query<FeedDayQuery>,
) -> AppResult<Response> {
    let requested_page = q.page.unwrap_or(0);
    let page = clamp_feed_page(requested_page);
    let page_size = FEED_POSTS_PER_PAGE as u64;
    let mentions_only = query_flag_is_truthy(q.mentions_only.as_deref());
    let feed_rows = feed::fetch_page_rows(
        &state.db,
        user_id,
        page,
        page_size,
        q.profile_handle.as_deref(),
        mentions_only,
    )
    .await
    .map_err(AppError::Db)?;
    let rows = feed_rows.rows;
    let has_newer = feed_rows.has_newer;
    let has_older = feed_rows.has_older;
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let range_label = c.tf(
        &loc,
        "ui.home.feed_page_label",
        &[("page", &(page + 1).to_string())],
    );

    let viewer_country = human_onboarding::country_code_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let viewer_country = viewer_country.as_deref();
    let viewer_human_entity_id =
        human_onboarding::primary_human_entity_id(&state.db, user_id)
            .await
            .map_err(AppError::Db)?;

    let mut cards = String::new();
    let preview_map = preview_image_allowed_for_feed_rows(&state.db, &rows)
        .await
        .map_err(AppError::Db)?;
    for r in &rows {
        cards.push_str(&feed_post_card_html(
            r,
            user_id,
            viewer_human_entity_id,
            true,
            true,
            viewer_country,
            crate::content::domain_reputation::preview_allowed_for_domain_id(r.domain_id, &preview_map),
            &state.config.public_base_url,
            &c,
            &loc,
        ));
    }
    if cards.is_empty() {
        cards = empty_feed_day_placeholder_html(&state.db, &loc, &c)
            .await
            .map_err(AppError::Db)?;
    }

    let noms = human_onboarding::ai_tokens_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let juror_summons_pending =
        juror_summons_pending_for_user(&state.db, user_id).await.map_err(AppError::Db)?;
    let unseen_mentions = crate::social::mentions::unseen_mention_count(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let mut res = Json(FeedDayResponse {
        page,
        range_label,
        has_newer,
        has_older,
        cards_html: cards,
        noms,
        juror_summons_pending,
        unseen_mention_count: if unseen_mentions > 0 {
            Some(unseen_mentions)
        } else {
            None
        },
    })
    .into_response();
    res.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, must-revalidate"),
    );
    Ok(res)
}

async fn home_mentions_ack(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
) -> AppResult<Json<serde_json::Value>> {
    let acked = crate::social::mentions::acknowledge_mentions(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    Ok(Json(serde_json::json!({ "acknowledged": acked })))
}

async fn fetch_constitution_clauses_for_post(
    pool: &sqlx::MySqlPool,
    post_id: u64,
) -> Result<Vec<JudicialClauseRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT
            cci.id,
            se.handle AS community_handle,
            sc.label AS community_label,
            cci.point,
            cci.item,
            cci.judicial_trigger_chk
        FROM community_constitution_item cci
        INNER JOIN social_community sc ON sc.social_entity_id = cci.community_id
        INNER JOIN social_entity se ON se.id = sc.social_entity_id
        WHERE cci.status = 'active'
          AND cci.judicial_trigger_chk = 1
          AND NOT (
              cci.point = 14
              AND cci.community_id = (
                  SELECT root_sc.social_entity_id
                  FROM social_entity root_se
                  INNER JOIN social_community root_sc ON root_sc.social_entity_id = root_se.id
                  WHERE root_se.handle = 'operating_charter_256'
                  LIMIT 1
              )
          )
          AND (
                cci.community_id = (
                SELECT root_sc.social_entity_id
                FROM social_entity root_se
                INNER JOIN social_community root_sc ON root_sc.social_entity_id = root_se.id
                WHERE root_se.handle = 'operating_charter_256'
                LIMIT 1
          )
             OR cci.community_id IN (
                SELECT sc_post.social_entity_id
                FROM post p
                INNER JOIN social_community sc_post ON sc_post.social_entity_id = p.entity_id
                WHERE p.id = ?
          )
             OR cci.community_id IN (
                SELECT pae.audience_entity_id
                FROM post_audience_entity pae
                INNER JOIN social_community sc_aud ON sc_aud.social_entity_id = pae.audience_entity_id
                WHERE pae.post_id = ?
          )
          )
        ORDER BY se.handle ASC, cci.point ASC, cci.id ASC
        "#,
    )
    .bind(post_id)
    .bind(post_id)
    .fetch_all(pool)
    .await
}

async fn judicial_case_clause_ids_apply_to_post(
    pool: &sqlx::MySqlPool,
    post_id: u64,
    clause_ids: &[u64],
) -> Result<bool, sqlx::Error> {
    if clause_ids.is_empty() {
        return Ok(false);
    }
    let mut placeholders = String::new();
    for (idx, _) in clause_ids.iter().enumerate() {
        if idx > 0 {
            placeholders.push(',');
        }
        placeholders.push('?');
    }
    let sql = format!(
        r#"
        SELECT COUNT(*)
        FROM community_constitution_item cci
        WHERE cci.id IN ({placeholders})
          AND cci.status = 'active'
          AND cci.judicial_trigger_chk = 1
          AND NOT (
              cci.point = 14
              AND cci.community_id = (
                  SELECT root_sc.social_entity_id
                  FROM social_entity root_se
                  INNER JOIN social_community root_sc ON root_sc.social_entity_id = root_se.id
                  WHERE root_se.handle = 'operating_charter_256'
                  LIMIT 1
              )
          )
          AND (
                cci.community_id = (
                    SELECT root_sc.social_entity_id
                    FROM social_entity root_se
                    INNER JOIN social_community root_sc ON root_sc.social_entity_id = root_se.id
                    WHERE root_se.handle = 'operating_charter_256'
                    LIMIT 1
                )
             OR cci.community_id IN (
                    SELECT sc_post.social_entity_id
                    FROM post p
                    INNER JOIN social_community sc_post ON sc_post.social_entity_id = p.entity_id
                    WHERE p.id = ?
                )
             OR cci.community_id IN (
                    SELECT pae.audience_entity_id
                    FROM post_audience_entity pae
                    INNER JOIN social_community sc_aud ON sc_aud.social_entity_id = pae.audience_entity_id
                    WHERE pae.post_id = ?
                )
          )
        "#,
    );

    let mut q = sqlx::query_scalar::<_, i64>(&sql);
    for id in clause_ids {
        q = q.bind(*id);
    }
    let count: i64 = q
        .bind(post_id)
        .bind(post_id)
        .fetch_one(pool)
        .await?;
    Ok(count == clause_ids.len() as i64)
}

async fn load_prior_resolved_post_dispute_case(
    pool: &sqlx::MySqlPool,
    prior_case_id: u64,
    post_id: u64,
) -> Result<Option<PriorResolvedPostCaseRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT
            jc.id,
            jc.subject_post_id,
            jc.originator_social_entity_id,
            jc.target_social_entity_id
        FROM judicial_case jc
        INNER JOIN ref_key_values rk_t
            ON rk_t.id = jc.trigger_type_id
           AND rk_t.`group` = ?
           AND rk_t.`key` = ?
           AND rk_t.is_active = 1
        INNER JOIN ref_key_values rk_s
            ON rk_s.id = jc.status_id
           AND rk_s.`group` = ?
           AND rk_s.`key` = ?
           AND rk_s.is_active = 1
        WHERE jc.id = ?
          AND jc.subject_post_id = ?
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_JUDICIAL_CASE_TRIGGER)
    .bind(ref_keys::KEY_JUDICIAL_TRIGGER_CIVIC_DISPUTE)
    .bind(ref_keys::GROUP_JUDICIAL_CASE_STATUS)
    .bind(ref_keys::KEY_JUDICIAL_CASE_RESOLVED)
    .bind(prior_case_id)
    .bind(post_id)
    .fetch_optional(pool)
    .await
}

async fn judicial_case_appeal_already_exists(
    pool: &sqlx::MySqlPool,
    prior_case_id: u64,
) -> Result<bool, sqlx::Error> {
    let v: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT id
        FROM judicial_case
        WHERE appealed_from_judicial_case_id = ?
        LIMIT 1
        "#,
    )
    .bind(prior_case_id)
    .fetch_optional(pool)
    .await?;
    Ok(v.is_some())
}

/// Returns `(originator_entity, target_entity)` for the appeal filing with `originator` as the appealing party.
async fn appeal_originator_and_target_for_user(
    pool: &sqlx::MySqlPool,
    user_id: u64,
    prior: &PriorResolvedPostCaseRow,
) -> Result<Option<(u64, u64)>, sqlx::Error> {
    let on_origin = ownership::user_has_active_role_for_entity(
        pool,
        user_id,
        prior.originator_social_entity_id,
    )
    .await?;
    let on_target = ownership::user_has_active_role_for_entity(
        pool,
        user_id,
        prior.target_social_entity_id,
    )
    .await?;
    if !on_origin && !on_target {
        return Ok(None);
    }
    let primary = ownership::primary_entity_id_for_user(pool, user_id).await?;
    let mut origin = prior.originator_social_entity_id;
    let mut target = prior.target_social_entity_id;
    if on_target && !on_origin {
        std::mem::swap(&mut origin, &mut target);
    } else if on_origin && on_target {
        if let Some(pid) = primary {
            if pid == prior.target_social_entity_id {
                std::mem::swap(&mut origin, &mut target);
            }
        }
    }
    if origin == target {
        return Ok(None);
    }
    Ok(Some((origin, target)))
}

async fn render_judicial_post_case_page(
    state: &AppState,
    user_id: u64,
    post_row: &HomeFeedRow,
    selected_clause_ids: &[u64],
    success_msg: Option<String>,
    return_to: Option<&str>,
) -> AppResult<Response> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let connections = connections::list_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let rail_html = rail::connections_nav_html(&connections, &loc, c);
    let face_path = user_faces::artwork_display_src_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let noms_balance = human_onboarding::ai_tokens_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let self_ssr = human_onboarding::ssr_rating_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let title_html = topbar_title_with_suffix_html(
        connections.len(),
        &c.t(&loc, "ui.judicial_post_case.heading"),
    );
    let topbar_html = secured_shell_topbar_html(
        &state,
        user_id,
        &loc,
        &title_html,
        true,
        &snippets::topbar_avatar_html(
            face_path.as_deref(),
            true,
            &c.t(&loc, "ui.home.logo_aria_home"),
            self_ssr,
            None,
        ),
        false,
    )
    .await?;
    let viewer_country = human_onboarding::country_code_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let viewer_human_entity_id =
        human_onboarding::primary_human_entity_id(&state.db, user_id)
            .await
            .map_err(AppError::Db)?;
    let preview_map = preview_image_allowed_for_feed_rows(&state.db, std::slice::from_ref(post_row))
        .await
        .map_err(AppError::Db)?;
    let post_card_html = feed_post_card_html(
        post_row,
        user_id,
        viewer_human_entity_id,
        false,
        false,
        viewer_country.as_deref(),
        crate::content::domain_reputation::preview_allowed_for_domain_id(post_row.domain_id, &preview_map),
        &state.config.public_base_url,
        c,
        &loc,
    );
    let clauses = fetch_constitution_clauses_for_post(&state.db, post_row.id)
        .await
        .map_err(AppError::Db)?;
    let success_html = success_msg
        .map(|m| nom_icon::flash_message_html(&m))
        .unwrap_or_default();
    let violated_clause_ids_value = selected_clause_ids
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let judicial_check_list_html = {
        let mut out = String::new();
        for clause in clauses.iter().filter(|c| c.judicial_trigger_chk == 1) {
            let sanitation_only =
                crate::social::safety::is_point_excluded_from_filing_ui(clause.point);
            let checked_attr = if !sanitation_only
                && selected_clause_ids.iter().any(|id| *id == clause.id)
            {
                " checked"
            } else {
                ""
            };
            let disabled_attr = if sanitation_only {
                r#" disabled aria-disabled="true""#
            } else {
                ""
            };
            let tier_class = if clause.community_handle == ref_keys::ROOT_OPERATING_CHARTER_HANDLE {
                "judicial-clause-root"
            } else {
                "judicial-clause-community"
            };
            let sanitation_class = if sanitation_only {
                " judicial-clause-sanitation"
            } else {
                ""
            };
            let (extra_attrs, harm_attr) =
                match crate::social::safety::harm_class_for_safety_point(clause.point) {
                    Some(crate::social::safety::SafetyHarmClass::A)
                    | Some(crate::social::safety::SafetyHarmClass::B)
                    | Some(crate::social::safety::SafetyHarmClass::LinkPolicy) => (
                        r#" data-sanitation-clause="1""#,
                        format!(
                            r#" data-harm-class="{}""#,
                            crate::social::safety::harm_class_for_safety_point(clause.point)
                                .map(|h| h.as_str())
                                .unwrap_or("")
                        ),
                    ),
                    Some(crate::social::safety::SafetyHarmClass::C) => (
                        r#" data-emergency-clause="1""#,
                        r#" data-harm-class="C""#.to_string(),
                    ),
                    Some(crate::social::safety::SafetyHarmClass::NonAcute) => (
                        "",
                        r#" data-harm-class="D""#.to_string(),
                    ),
                    None => (
                        if crate::social::judicial::is_emergency_safety_point(clause.point) {
                            r#" data-emergency-clause="1""#
                        } else {
                            ""
                        },
                        String::new(),
                    ),
                };
            let label_attrs = format!(
                r#" class="auth-check-label {tier_class}{sanitation_class}"{extra_attrs}"#,
                tier_class = tier_class,
                sanitation_class = sanitation_class,
                extra_attrs = extra_attrs,
            );
            let sanitation_note = if sanitation_only {
                format!(
                    r#"<span class="judicial-clause-sanitation-note">{}</span>"#,
                    escape_html(&c.t(&loc, "ui.judicial_post_case.sanitation_clause_footnote"))
                )
            } else {
                String::new()
            };
            out.push_str(&format!(
                r#"<label{label_attrs}{harm_attr}><input class="auth-check-input" type="checkbox" data-clause-id="{id}" data-clause-point="{point}" value="{id}"{checked}{disabled}><span class="auth-check-text"><span class="judicial-clause-ref mono">{label} #{point}</span><span class="judicial-clause-body">{item}</span>{sanitation_note}</span></label>"#,
                label_attrs = label_attrs,
                harm_attr = harm_attr,
                id = clause.id,
                point = clause.point,
                checked = checked_attr,
                disabled = disabled_attr,
                label = escape_html(&clause.community_label),
                item = escape_html(&clause.item),
                sanitation_note = sanitation_note,
            ));
        }
        out
    };
    let safety_policy_link = format!(
        r#"<p class="sub"><a href="/safety" target="_blank" rel="noopener noreferrer">{}</a></p>"#,
        escape_html(&c.t(&loc, "ui.judicial_post_case.safety_policy_link"))
    );
    let viewer_country = human_onboarding::country_code_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let crisis_country = viewer_country
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("ZZ");
    let crisis_bundle_json =
        crate::crisis_resources::filing_crisis_bundle_json(&loc, crisis_country);
    let crisis_panel_mount =
        r#"<div id="judicial-crisis-panel-root" class="judicial-crisis-panel-root" aria-live="polite"></div>"#;
    let judicial_form_card = format!(
        r#"<section class="card app-card judicial-post-case-card"><div class="card-accent" role="presentation"></div><h2>{heading}</h2>{safety_policy_link}{crisis_panel_mount}<p class="sub">{cost}</p><p class="sub">{hint}</p>{success}<form id="judicial-case-request-form" method="post" action="/home/judicial/post/{post_id}" novalidate><input type="hidden" name="return_to" value="{return_to}"><div>{judicial_checks_form}<input type="hidden" name="violated_clause_ids" id="judicial-violated-clause-ids-input" value="{violated_clause_ids_value}"></div><p class="hint">{clause_hint}</p><p id="judicial-clause-error" class="sub" role="alert" hidden>{clause_required_client}</p><label class="field-label" for="judicial-case-rationale-editor">{reason_label}</label><div class="create-post-editor-toolbar"><button type="button" class="create-post-editor-btn" data-editor-cmd="bold" aria-label="Bold"><strong>B</strong></button><button type="button" class="create-post-editor-btn" data-editor-cmd="italic" aria-label="Italic"><em>I</em></button><button type="button" class="create-post-editor-btn" data-editor-cmd="underline" aria-label="Underline"><u>U</u></button></div><div id="judicial-case-rationale-editor" class="create-post-editor" contenteditable="true" data-placeholder="{reason_placeholder}" aria-label="{reason_label}"></div><input id="judicial-case-rationale-input" type="hidden" name="rationale" value=""><p id="judicial-case-rationale-error" class="sub" role="alert" hidden>{reason_required}</p><button type="submit" class="auth-text-btn btn-cta">{submit}</button></form></section>"#,
        heading = escape_html(&c.t(&loc, "ui.judicial_post_case.heading")),
        safety_policy_link = safety_policy_link,
        crisis_panel_mount = crisis_panel_mount,
        cost = nom_icon::nom_cost_line_html(
            &c,
            &loc,
            BillableService::JudicialFiling(crate::pricing::JudicialTrigger::CivicDispute).nom_cost(),
        ),
        hint = escape_html(&c.t(&loc, "ui.judicial_post_case.hint")),
        clause_hint = escape_html(&c.t(&loc, "ui.judicial_post_case.constitution_clause_hint")),
        clause_required_client = escape_html(&c.t(&loc, "judicial_case_filing.clause_required")),
        success = success_html,
        post_id = post_row.id,
        judicial_checks_form = judicial_check_list_html,
        violated_clause_ids_value = escape_html(&violated_clause_ids_value),
        reason_label = escape_html(&c.t(&loc, "ui.judicial_post_case.reason_label")),
        reason_placeholder = escape_html(&c.t(&loc, "ui.judicial_post_case.reason_placeholder")),
        reason_required = escape_html(&c.t(&loc, "judicial_case_filing.reason_required")),
        submit = escape_html(&c.t(&loc, "ui.judicial_post_case.submit")),
        return_to = escape_html(return_to.unwrap_or("/home")),
    );
    let self_private_handle = human_onboarding::human_entity_handle_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?
        .unwrap_or_default();
    let html = JUDICIAL_POST_CASE_TEMPLATE
        .replace("__HTML_LANG__", &loc)
        .replace("__JUD_POST_CASE_TITLE__", &escape_html(&c.t(&loc, "ui.judicial_post_case.title")))
        .replace("__HOME_CONNECTION_COUNT__", &connections.len().to_string())
        .replace("__SECURED_TOPBAR__", &topbar_html)
        .replace("__HOME_APP_STAGE_CLASS__", "app-stage")
        .replace("__HOME_RAIL_HTML__", &rail_html)
        .replace("__HOME_SELF_PRIVATE_HANDLE__", &self_private_handle)
        .replace("__JUD_POST_CASE_POST_CARD__", &post_card_html)
        .replace("__JUD_POST_CASE_FORM_CARD__", &judicial_form_card)
        .replace(
            "__HOME_SHELL_I18N_JSON__",
            &escape_json_for_html_script(&home_shell_i18n_json(
                &loc,
                connections.len(),
                Some(self_private_handle.as_str()).filter(|s| !s.is_empty()),
            )),
        )
        .replace(
            "__JUDICIAL_CRISIS_BUNDLE_JSON__",
            &escape_json_for_html_script(&crisis_bundle_json),
        );

    Ok(Html(i18n::subst_markers(
        &snippets::inject_secured_public_footer_localized(&html, &c, &loc),
        &loc,
    ))
    .into_response())
}

async fn render_judicial_post_case_unavailable_page(
    state: &AppState,
    user_id: u64,
    message: Option<&str>,
) -> AppResult<Response> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let connections = connections::list_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let rail_html = rail::connections_nav_html(&connections, &loc, c);
    let face_path = user_faces::artwork_display_src_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let noms_balance = human_onboarding::ai_tokens_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let self_ssr = human_onboarding::ssr_rating_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let title_html = topbar_title_with_suffix_html(
        connections.len(),
        &c.t(&loc, "ui.judicial_post_case.heading"),
    );
    let topbar_html = secured_shell_topbar_html(
        &state,
        user_id,
        &loc,
        &title_html,
        true,
        &snippets::topbar_avatar_html(
            face_path.as_deref(),
            true,
            &c.t(&loc, "ui.home.logo_aria_home"),
            self_ssr,
            None,
        ),
        false,
    )
    .await?;
    let self_private_handle = human_onboarding::human_entity_handle_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?
        .unwrap_or_default();
    let unavailable_msg = message
        .map(|m| m.to_string())
        .unwrap_or_else(|| c.t(&loc, "ui.judicial_post_case.post_unavailable"));
    let unavailable_card = format!(
        r#"<section class="card app-card judicial-post-case-card"><div class="card-accent" role="presentation"></div><h2>{heading}</h2><p class="sub">{msg}</p><p class="sub"><a href="/home">{back}</a></p></section>"#,
        heading = escape_html(&c.t(&loc, "ui.judicial_post_case.heading")),
        msg = escape_html(&unavailable_msg),
        back = escape_html(&c.t(&loc, "ui.shared.home")),
    );
    let html = JUDICIAL_POST_CASE_TEMPLATE
        .replace("__HTML_LANG__", &loc)
        .replace(
            "__JUD_POST_CASE_TITLE__",
            &escape_html(&c.t(&loc, "ui.judicial_post_case.title")),
        )
        .replace("__HOME_CONNECTION_COUNT__", &connections.len().to_string())
        .replace("__SECURED_TOPBAR__", &topbar_html)
        .replace("__HOME_APP_STAGE_CLASS__", "app-stage")
        .replace("__HOME_RAIL_HTML__", &rail_html)
        .replace("__HOME_SELF_PRIVATE_HANDLE__", &self_private_handle)
        .replace("__JUD_POST_CASE_POST_CARD__", "")
        .replace("__JUD_POST_CASE_FORM_CARD__", &unavailable_card)
        .replace(
            "__HOME_SHELL_I18N_JSON__",
            &escape_json_for_html_script(&home_shell_i18n_json(
                &loc,
                connections.len(),
                Some(self_private_handle.as_str()).filter(|s| !s.is_empty()),
            )),
        )
        .replace("__JUDICIAL_CRISIS_BUNDLE_JSON__", "{}");
    Ok(Html(i18n::subst_markers(
        &snippets::inject_secured_public_footer_localized(&html, &c, &loc),
        &loc,
    ))
    .into_response())
}

async fn home_judicial_post_case_page(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(post_id): Path<u64>,
    Query(q): Query<JudicialPostCasePageQuery>,
) -> AppResult<Response> {
    let Some(row) = feed::fetch_visible_root_post_for_viewer(&state.db, user_id, post_id)
        .await
        .map_err(AppError::Db)? else {
        return render_judicial_post_case_unavailable_page(&state, user_id, None).await;
    };
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    if !feed::viewer_may_file_human_post_dispute(user_id, &row) {
        return render_judicial_post_case_unavailable_page(
            &state,
            user_id,
            Some(&c.t(&loc, "judicial_case_filing.cannot_file_against_self")),
        )
        .await;
    }
    if !feed::viewer_may_file_human_post_dispute_tx(&state.db, user_id, &row)
        .await
        .map_err(AppError::Db)?
    {
        return render_judicial_post_case_unavailable_page(
            &state,
            user_id,
            Some(&c.t(&loc, "judicial_case_filing.cannot_file_against_self")),
        )
        .await;
    }
    render_judicial_post_case_page(
        &state,
        user_id,
        &row,
        &[],
        None,
        q.return_to.as_deref(),
    )
    .await
}

async fn home_judicial_post_emergency_contest_page(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(post_id): Path<u64>,
    Query(q): Query<JudicialEmergencyContestPageQuery>,
) -> AppResult<Response> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let Some(row) = feed::fetch_visible_root_post_for_viewer(&state.db, user_id, post_id)
        .await
        .map_err(AppError::Db)?
    else {
        return render_judicial_post_case_unavailable_page(&state, user_id, None).await;
    };
    let err = emergency_contest_page_error(&row, user_id, &loc, &c);
    render_judicial_post_emergency_contest_page(
        &state,
        user_id,
        &row,
        err.as_deref(),
        q.return_to.as_deref(),
    )
    .await
}

fn emergency_contest_page_error(
    row: &HomeFeedRow,
    user_id: u64,
    loc: &str,
    c: &i18n::Catalog,
) -> Option<String> {
    if row.post_created_by != Some(user_id) {
        return Some(c.t(loc, "judicial_case_filing.emergency_contest_forbidden"));
    }
    if row.post_status_key.as_deref() != Some(ref_keys::KEY_POST_SUPPRESSED) {
        return Some(c.t(loc, "judicial_case_filing.emergency_contest_unavailable"));
    }
    match row.judicial_case_status_key.as_deref() {
        Some(ref_keys::KEY_JUDICIAL_CASE_CONTAINED) => None,
        Some(_) => Some(c.t(loc, "judicial_case_filing.emergency_contest_already")),
        None => Some(c.t(loc, "judicial_case_filing.emergency_contest_unavailable")),
    }
}

async fn render_judicial_post_emergency_contest_page(
    state: &AppState,
    user_id: u64,
    post_row: &HomeFeedRow,
    err: Option<&str>,
    return_to: Option<&str>,
) -> AppResult<Response> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let connections = connections::list_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let rail_html = rail::connections_nav_html(&connections, &loc, c);
    let face_path = user_faces::artwork_display_src_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let noms_balance = human_onboarding::ai_tokens_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let self_ssr = human_onboarding::ssr_rating_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let title_html = topbar_title_with_suffix_html(
        connections.len(),
        &c.t(&loc, "ui.judicial_post_emergency_contest.heading"),
    );
    let topbar_html = secured_shell_topbar_html(
        &state,
        user_id,
        &loc,
        &title_html,
        true,
        &snippets::topbar_avatar_html(
            face_path.as_deref(),
            true,
            &c.t(&loc, "ui.home.logo_aria_home"),
            self_ssr,
            None,
        ),
        false,
    )
    .await?;
    let self_private_handle = human_onboarding::human_entity_handle_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?
        .unwrap_or_default();
    let viewer_country = human_onboarding::country_code_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let viewer_human_entity_id =
        human_onboarding::primary_human_entity_id(&state.db, user_id)
            .await
            .map_err(AppError::Db)?;
    let preview_map = preview_image_allowed_for_feed_rows(&state.db, std::slice::from_ref(post_row))
        .await
        .map_err(AppError::Db)?;
    let post_card_html = feed_post_card_html(
        post_row,
        user_id,
        viewer_human_entity_id,
        false,
        false,
        viewer_country.as_deref(),
        crate::content::domain_reputation::preview_allowed_for_domain_id(post_row.domain_id, &preview_map),
        &state.config.public_base_url,
        c,
        &loc,
    );
    let rt = return_to.unwrap_or("/home");
    let return_to_esc = escape_html(rt);
    let cost_line = nom_icon::nom_cost_line_html(
        &c,
        &loc,
        BillableService::EmergencyContest.nom_cost(),
    );
    let crisis_panel = crate::crisis_resources::panel_for_viewer(
        &loc,
        viewer_country.as_deref(),
        &[],
        true,
    );
    let err_html = err
        .map(|m| nom_icon::flash_alert_html(m))
        .unwrap_or_default();
    let form_card = if err.is_some() {
        format!(
            r#"<section class="card app-card judicial-post-case-card"><div class="card-accent" role="presentation"></div><h2>{heading}</h2>{crisis_panel}{err_html}<p class="sub"><a href="/home">{back}</a></p></section>"#,
            heading = escape_html(&c.t(&loc, "ui.judicial_post_emergency_contest.heading")),
            crisis_panel = crisis_panel,
            err_html = err_html,
            back = escape_html(&c.t(&loc, "ui.shared.home")),
        )
    } else {
        format!(
            r#"<section class="card app-card judicial-post-case-card"><div class="card-accent" role="presentation"></div><h2>{heading}</h2>{crisis_panel}<p class="sub">{cost}</p><p class="sub">{hint}</p><form id="judicial-emergency-contest-form" method="post" action="/home/judicial/post/{post_id}/contest"><input type="hidden" name="return_to" value="{return_to_esc}"><button type="submit" class="auth-text-btn btn-cta">{submit}</button></form></section>"#,
            heading = escape_html(&c.t(&loc, "ui.judicial_post_emergency_contest.heading")),
            crisis_panel = crisis_panel,
            cost = cost_line,
            hint = escape_html(&c.t(&loc, "ui.judicial_post_emergency_contest.hint")),
            post_id = post_row.id,
            return_to_esc = return_to_esc,
            submit = escape_html(&c.t(&loc, "ui.judicial_post_emergency_contest.submit")),
        )
    };
    let html = JUDICIAL_POST_EMERGENCY_CONTEST_TEMPLATE
        .replace("__HTML_LANG__", &loc)
        .replace(
            "__JUD_POST_EMERGENCY_CONTEST_TITLE__",
            &escape_html(&c.t(&loc, "ui.judicial_post_emergency_contest.title")),
        )
        .replace("__HOME_CONNECTION_COUNT__", &connections.len().to_string())
        .replace("__SECURED_TOPBAR__", &topbar_html)
        .replace("__HOME_APP_STAGE_CLASS__", "app-stage")
        .replace("__HOME_RAIL_HTML__", &rail_html)
        .replace("__HOME_SELF_PRIVATE_HANDLE__", &self_private_handle)
        .replace("__JUD_POST_CASE_POST_CARD__", &post_card_html)
        .replace("__JUD_POST_EMERGENCY_CONTEST_FORM_CARD__", &form_card)
        .replace(
            "__HOME_SHELL_I18N_JSON__",
            &escape_json_for_html_script(&home_shell_i18n_json(
                &loc,
                connections.len(),
                Some(self_private_handle.as_str()).filter(|s| !s.is_empty()),
            )),
        );
    Ok(Html(i18n::subst_markers(
        &snippets::inject_secured_public_footer_localized(&html, &c, &loc),
        &loc,
    ))
    .into_response())
}

async fn home_judicial_post_emergency_contest_submit(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(post_id): Path<u64>,
    Form(body): Form<JudicialEmergencyContestForm>,
) -> AppResult<Response> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let return_to = body.return_to.as_deref();
    let Some(row) = feed::fetch_visible_root_post_for_viewer(&state.db, user_id, post_id)
        .await
        .map_err(AppError::Db)?
    else {
        return render_judicial_post_case_unavailable_page(&state, user_id, None).await;
    };
    if let Some(msg) = emergency_contest_page_error(&row, user_id, &loc, &c) {
        return render_judicial_post_emergency_contest_page(
            &state,
            user_id,
            &row,
            Some(&msg),
            return_to,
        )
        .await;
    }
    let root_post_id = row.id;

    let Some(primary_human_entity_id) =
        human_onboarding::primary_human_entity_id(&state.db, user_id)
            .await
            .map_err(AppError::Db)?
    else {
        return Err(AppError::BadRequest(
            c.t(&loc, "judicial_case_filing.no_primary_profile"),
        ));
    };

    match judicial::contest_emergency_case(
        &state.db,
        user_id,
        primary_human_entity_id,
        root_post_id,
    )
    .await
    .map_err(AppError::Db)?
    {
        judicial::ContestOutcome::Opened { case_id } => {
            if let Err(err) = crate::jobs::enqueue_job(
                &state.db,
                None,
                "judicial_jury_nomination",
                &serde_json::json!({ "limit": 32 }),
                Some(&format!("judicial-case:{case_id}")),
                30,
            )
            .await
            {
                tracing::warn!(
                    error = %err,
                    case_id,
                    "failed to enqueue judicial_jury_nomination on emergency contest"
                );
            }
            crate::email::notify_emergency_contest_opened(
                &state.db,
                &state.config,
                user_id,
                case_id,
            )
            .await;
            Ok(Redirect::to(&with_notice_query("/home", "emergency_contest_opened")).into_response())
        }
        judicial::ContestOutcome::InsufficientNoms => {
            let msg = nom_icon::insufficient_noms_html(
                &c,
                &loc,
                "noms.insufficient_for_emergency_contest",
                BillableService::EmergencyContest.nom_cost(),
                None,
            );
            render_judicial_post_emergency_contest_page(
                &state,
                user_id,
                &row,
                Some(&msg),
                return_to,
            )
            .await
        }
        judicial::ContestOutcome::Forbidden => {
            render_judicial_post_emergency_contest_page(
                &state,
                user_id,
                &row,
                Some(&c.t(&loc, "judicial_case_filing.emergency_contest_forbidden")),
                return_to,
            )
            .await
        }
        judicial::ContestOutcome::AlreadyContested => {
            render_judicial_post_emergency_contest_page(
                &state,
                user_id,
                &row,
                Some(&c.t(&loc, "judicial_case_filing.emergency_contest_already")),
                return_to,
            )
            .await
        }
        judicial::ContestOutcome::NotFound | judicial::ContestOutcome::NotContained => {
            render_judicial_post_emergency_contest_page(
                &state,
                user_id,
                &row,
                Some(&c.t(&loc, "judicial_case_filing.emergency_contest_unavailable")),
                return_to,
            )
            .await
        }
    }
}

async fn home_judicial_post_author_withdraw_submit(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(post_id): Path<u64>,
    Form(body): Form<JudicialAuthorWithdrawForm>,
) -> AppResult<Response> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    if body.withdraw_action.trim() != "withdraw_post" {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.judicial_post_case.withdraw_wrong_action"),
        ));
    }
    match judicial::author_withdraw_post_under_review(&state.db, user_id, post_id)
        .await
        .map_err(AppError::Db)?
    {
        AuthorWithdrawPostUnderReviewOutcome::ThreadDeleted => {
            Ok(Json(serde_json::json!({ "ok": true })).into_response())
        }
        AuthorWithdrawPostUnderReviewOutcome::NotFound => Err(AppError::NotFound),
        AuthorWithdrawPostUnderReviewOutcome::Forbidden => Err(AppError::BadRequest(
            c.t(&loc, "ui.judicial_post_case.withdraw_forbidden"),
        )),
        AuthorWithdrawPostUnderReviewOutcome::NotUnderReview => Err(AppError::BadRequest(
            c.t(&loc, "ui.judicial_post_case.withdraw_not_under_review"),
        )),
        AuthorWithdrawPostUnderReviewOutcome::ThreadHasOtherAuthors => Err(AppError::BadRequest(
            c.t(&loc, "ui.judicial_post_case.withdraw_thread_has_others"),
        )),
        AuthorWithdrawPostUnderReviewOutcome::CaseTerminal => Err(AppError::BadRequest(
            c.t(&loc, "ui.judicial_post_case.withdraw_case_terminal"),
        )),
        AuthorWithdrawPostUnderReviewOutcome::WrongTrigger => Err(AppError::BadRequest(
            c.t(&loc, "ui.judicial_post_case.withdraw_wrong_trigger"),
        )),
    }
}

async fn home_judicial_post_case_submit(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(post_id): Path<u64>,
    Form(body): Form<JudicialPostCaseFormBody>,
) -> AppResult<Response> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let Some(row) = feed::fetch_visible_root_post_for_viewer(&state.db, user_id, post_id)
        .await
        .map_err(AppError::Db)? else {
        return render_judicial_post_case_unavailable_page(&state, user_id, None).await;
    };
    // `post_id` from the URL may be a thread reply; case + juror workspace SQL key off the **root** row.
    let root_post_id = row.id;

    if !feed::viewer_may_file_human_post_dispute(user_id, &row) {
        let msg = c.t(&loc, "judicial_case_filing.cannot_file_against_self").to_string();
        return render_judicial_post_case_page(
            &state,
            user_id,
            &row,
            &[],
            Some(msg),
            body.return_to.as_deref(),
        )
        .await;
    }
    if !feed::viewer_may_file_human_post_dispute_tx(&state.db, user_id, &row)
        .await
        .map_err(AppError::Db)?
    {
        let msg = c.t(&loc, "judicial_case_filing.cannot_file_against_self").to_string();
        return render_judicial_post_case_page(
            &state,
            user_id,
            &row,
            &[],
            Some(msg),
            body.return_to.as_deref(),
        )
        .await;
    }

    let mut clause_ids: Vec<u64> = body
        .violated_clause_ids
        .split(',')
        .filter_map(|raw| raw.trim().parse::<u64>().ok())
        .collect();
    clause_ids.sort_unstable();
    clause_ids.dedup();
    if clause_ids.is_empty() {
        let msg = c.t(&loc, "judicial_case_filing.clause_required").to_string();
        return render_judicial_post_case_page(
            &state,
            user_id,
            &row,
            &clause_ids,
            Some(msg),
            body.return_to.as_deref(),
        )
        .await;
    }
    if !judicial_case_clause_ids_apply_to_post(&state.db, root_post_id, &clause_ids)
        .await
        .map_err(AppError::Db)?
    {
        let msg = c.t(&loc, "judicial_case_filing.clause_required").to_string();
        return render_judicial_post_case_page(
            &state,
            user_id,
            &row,
            &clause_ids,
            Some(msg),
            body.return_to.as_deref(),
        )
        .await;
    }

    let rationale_html = body.rationale.unwrap_or_default();
    let rationale_sanitized = content_sanitize::sanitize_post_markup(&rationale_html);
    let rationale_plain = strip_tags_plain(&rationale_sanitized);
    if rationale_plain.trim().is_empty() {
        let msg = c.t(&loc, "judicial_case_filing.reason_required").to_string();
        return render_judicial_post_case_page(
            &state,
            user_id,
            &row,
            &clause_ids,
            Some(msg),
            body.return_to.as_deref(),
        )
        .await;
    }

    let Some(primary_human_entity_id) =
        human_onboarding::primary_human_entity_id(&state.db, user_id)
            .await
            .map_err(AppError::Db)?
    else {
        let msg = c.t(&loc, "judicial_case_filing.no_primary_profile").to_string();
        return render_judicial_post_case_page(
            &state,
            user_id,
            &row,
            &clause_ids,
            Some(msg),
            body.return_to.as_deref(),
        )
        .await;
    };

    let type_originator_id = ref_key_values::lookup_active_id(
        &state.db,
        ref_keys::GROUP_JUDICIAL_CASE_COMMENT_TYPE,
        ref_keys::KEY_JCC_ORIGINATOR,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| AppError::BadRequest(c.t(&loc, "judicial_case_filing.refs_not_configured")))?;
    let type_evidence_id = ref_key_values::lookup_active_id(
        &state.db,
        ref_keys::GROUP_JUDICIAL_CASE_COMMENT_TYPE,
        ref_keys::KEY_JCC_EVIDENCE,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| AppError::BadRequest(c.t(&loc, "judicial_case_filing.refs_not_configured")))?;

    let url_list = judicial_link_evidence::collect_external_urls(
        &[
            row.body_html_sanitized.as_str(),
            row.body_markup.as_str(),
            rationale_sanitized.as_str(),
        ],
        &[row.link_href.as_deref()],
    );
    let evidence_client = judicial_link_evidence::evidence_http_client().map_err(|e| {
        AppError::BadRequest(format!("judicial link evidence client: {e}"))
    })?;
    let link_snapshots =
        judicial_link_evidence::capture_external_urls(&evidence_client, &url_list).await;

    let mut tx = state.db.begin().await.map_err(AppError::Db)?;
    let clause_points = match judicial::classify_clause_ids_tx(&mut tx, &clause_ids).await {
        Ok(points) => points,
        Err(judicial::EmergencyFilingError::MixedClauseKinds) => {
            let msg = c
                .t(&loc, "judicial_case_filing.mixed_emergency_civic_clauses")
                .to_string();
            return render_judicial_post_case_page(
                &state,
                user_id,
                &row,
                &clause_ids,
                Some(msg),
                body.return_to.as_deref(),
            )
            .await;
        }
        Err(_) => {
            let msg = c.t(&loc, "judicial_case_filing.clause_required").to_string();
            return render_judicial_post_case_page(
                &state,
                user_id,
                &row,
                &clause_ids,
                Some(msg),
                body.return_to.as_deref(),
            )
            .await;
        }
    };
    let is_emergency = match crate::social::safety::route_filing_for_clause_points(&clause_points) {
        Ok(crate::social::safety::FilingRoute::EmergencySafety) => true,
        Ok(crate::social::safety::FilingRoute::CivicDispute) => false,
        Ok(crate::social::safety::FilingRoute::SanitationOnly(hc)) => {
            tx.rollback().await.ok();
            crate::social::safety::log_filing_sanitation_routed(
                &state.db,
                "judicial_post_case",
                hc.as_str(),
                user_id,
                root_post_id,
                &clause_points,
            )
            .await;
            let msg_key = match hc {
                crate::social::safety::SafetyHarmClass::A => {
                    "judicial_case_filing.sanitation_class_a"
                }
                crate::social::safety::SafetyHarmClass::B => {
                    "judicial_case_filing.sanitation_class_b"
                }
                crate::social::safety::SafetyHarmClass::LinkPolicy => {
                    "judicial_case_filing.sanitation_class_link"
                }
                _ => "judicial_case_filing.sanitation_routed",
            };
            let msg = c.t(&loc, msg_key).to_string();
            return render_judicial_post_case_page(
                &state,
                user_id,
                &row,
                &clause_ids,
                Some(msg),
                body.return_to.as_deref(),
            )
            .await;
        }
        Err(crate::social::safety::RouteError::MixedHarmClasses) => {
            tx.rollback().await.ok();
            let msg = c
                .t(&loc, "judicial_case_filing.mixed_harm_class_clauses")
                .to_string();
            return render_judicial_post_case_page(
                &state,
                user_id,
                &row,
                &clause_ids,
                Some(msg),
                body.return_to.as_deref(),
            )
            .await;
        }
        Err(crate::social::safety::RouteError::Empty) => {
            tx.rollback().await.ok();
            let msg = c.t(&loc, "judicial_case_filing.clause_required").to_string();
            return render_judicial_post_case_page(
                &state,
                user_id,
                &row,
                &clause_ids,
                Some(msg),
                body.return_to.as_deref(),
            )
            .await;
        }
    };

    if is_emergency {
        if let Err(judicial::EmergencyFilingError::DailyCap) =
            judicial::check_daily_emergency_filing_cap_tx(&mut tx, user_id).await
        {
            let msg = c
                .t(&loc, "judicial_case_filing.emergency_daily_cap")
                .to_string();
            return render_judicial_post_case_page(
                &state,
                user_id,
                &row,
                &clause_ids,
                Some(msg),
                body.return_to.as_deref(),
            )
            .await;
        }
        if judicial::active_emergency_case_on_post_tx(&mut tx, root_post_id)
            .await
            .map_err(AppError::Db)?
        {
            let msg = c
                .t(&loc, "judicial_case_filing.emergency_duplicate_post")
                .to_string();
            return render_judicial_post_case_page(
                &state,
                user_id,
                &row,
                &clause_ids,
                Some(msg),
                body.return_to.as_deref(),
            )
            .await;
        }
    }

    let (trigger_key, filing_service, spend_ref_suffix, initial_status_key) = if is_emergency {
        (
            ref_keys::KEY_JUDICIAL_TRIGGER_EMERGENCY_SAFETY,
            crate::pricing::BillableService::JudicialFiling(
                crate::pricing::JudicialTrigger::EmergencySafety,
            ),
            "emergency_safety_filing",
            ref_keys::KEY_JUDICIAL_CASE_CONTAINED,
        )
    } else {
        (
            ref_keys::KEY_JUDICIAL_TRIGGER_CIVIC_DISPUTE,
            crate::pricing::BillableService::JudicialFiling(
                crate::pricing::JudicialTrigger::CivicDispute,
            ),
            "civic_dispute_filing",
            ref_keys::KEY_JUDICIAL_CASE_OPEN,
        )
    };

    let trigger_type_id = ref_key_values::lookup_active_id_conn(
        &mut *tx,
        ref_keys::GROUP_JUDICIAL_CASE_TRIGGER,
        trigger_key,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| AppError::BadRequest(c.t(&loc, "judicial_case_filing.refs_not_configured")))?;
    let case_status_id = ref_key_values::lookup_active_id_conn(
        &mut *tx,
        ref_keys::GROUP_JUDICIAL_CASE_STATUS,
        initial_status_key,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| AppError::BadRequest(c.t(&loc, "judicial_case_filing.refs_not_configured")))?;

    let spend_ref = format!("{spend_ref_suffix}:post:{root_post_id}");
    let spend_outcome = noms::spend_for_service_tx(
        &mut tx,
        primary_human_entity_id,
        filing_service,
        user_id,
        Some(&spend_ref),
    )
    .await
    .map_err(AppError::Db)?;
    if !spend_outcome.is_paid() {
        let msg = nom_icon::insufficient_noms_html(
            &c,
            &loc,
            "noms.insufficient_for_judicial_case",
            BillableService::JudicialFiling(crate::pricing::JudicialTrigger::CivicDispute).nom_cost(),
            None,
        );
        return render_judicial_post_case_page(
            &state,
            user_id,
            &row,
            &clause_ids,
            Some(msg),
            body.return_to.as_deref(),
        )
        .await;
    }

    let jury_size_frozen = if is_emergency {
        0u8
    } else {
        judicial::compute_jury_size_to_freeze_at_filing_tx(
            &mut tx,
            primary_human_entity_id,
            row.post_entity_id,
            Some(user_id),
            Some(root_post_id),
            ref_keys::KEY_JUDICIAL_TRIGGER_CIVIC_DISPUTE,
        )
        .await
        .map_err(AppError::Db)?
    };

    let originator_rating: Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(se.rating, 0) FROM social_entity se WHERE se.id = ? LIMIT 1"#,
    )
    .bind(primary_human_entity_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::Db)?;

    let rationale_markup = rationale_html.trim();
    let rationale_markup_stored = if rationale_markup.is_empty() {
        format!("<p>{}</p>", escape_html(rationale_plain.trim()))
    } else {
        rationale_markup.to_string()
    };

    let jc_res = sqlx::query(
        r#"
        INSERT INTO judicial_case (
            originator_social_entity_id,
            target_social_entity_id,
            trigger_type_id,
            status_id,
            subject_post_id,
            jury_size,
            jury_votes_for,
            jury_votes_against,
            ai_votes_for,
            ai_votes_against,
            created_by,
            modified_by
        )
        VALUES (?, ?, ?, ?, ?, ?, 0, 0, 0, 0, ?, ?)
        "#,
    )
    .bind(primary_human_entity_id)
    .bind(row.post_entity_id)
    .bind(trigger_type_id)
    .bind(case_status_id)
    .bind(root_post_id)
    .bind(jury_size_frozen)
    .bind(user_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(AppError::Db)?;
    let judicial_case_id = jc_res.last_insert_id();

    judicial_link_evidence::insert_captured_rows_tx(&mut tx, judicial_case_id, &link_snapshots, user_id)
        .await
        .map_err(AppError::Db)?;

    for clause_id in &clause_ids {
        sqlx::query(
            r#"
            INSERT INTO judicial_case_constitution_item (judicial_case_id, constitution_item_id, created_by)
            VALUES (?, ?, ?)
            ON DUPLICATE KEY UPDATE created_by = COALESCE(VALUES(created_by), created_by)
            "#,
        )
        .bind(judicial_case_id)
        .bind(*clause_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Db)?;
    }

    sqlx::query(
        r#"
        INSERT INTO judicial_case_comment (
            judicial_case_id,
            type_id,
            body_plain,
            body_markup,
            body_html_sanitized,
            author_user_id,
            author_social_entity_id,
            author_entity_rating_at_create,
            in_reply_to_comment_id,
            created_by,
            modified_by
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
        "#,
    )
    .bind(judicial_case_id)
    .bind(type_originator_id)
    .bind(rationale_plain.as_str())
    .bind(rationale_markup_stored.as_str())
    .bind(rationale_sanitized.as_str())
    .bind(user_id)
    .bind(primary_human_entity_id)
    .bind(originator_rating)
    .bind(user_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(AppError::Db)?;

    let evidence_plain = c.tf(
        &loc,
        "judicial_case_filing.evidence_comment_plain",
        &[("post_id", &root_post_id.to_string())],
    );
    let evidence_markup = format!("<p>{}</p>", escape_html(&evidence_plain));
    let evidence_sanitized = content_sanitize::sanitize_post_markup(&evidence_markup);

    sqlx::query(
        r#"
        INSERT INTO judicial_case_comment (
            judicial_case_id,
            type_id,
            body_plain,
            body_markup,
            body_html_sanitized,
            author_user_id,
            author_social_entity_id,
            author_entity_rating_at_create,
            in_reply_to_comment_id,
            linked_post_id,
            created_by,
            modified_by
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?, ?)
        "#,
    )
    .bind(judicial_case_id)
    .bind(type_evidence_id)
    .bind(evidence_plain.as_str())
    .bind(evidence_markup.as_str())
    .bind(evidence_sanitized.as_str())
    .bind(user_id)
    .bind(primary_human_entity_id)
    .bind(originator_rating)
    .bind(root_post_id)
    .bind(user_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(AppError::Db)?;

    if is_emergency {
        judicial::suppress_post_thread_tx(&mut tx, root_post_id, user_id)
            .await
            .map_err(AppError::Db)?;
    } else {
        let under_judicial_review_status_id: Option<u64> = sqlx::query_scalar(
            r#"
            SELECT id
            FROM ref_key_values
            WHERE `group` = ? AND `key` = ?
            LIMIT 1
            "#,
        )
        .bind(ref_keys::GROUP_STATUS)
        .bind(ref_keys::KEY_POST_UNDER_JUDICIAL_REVIEW)
        .fetch_optional(&mut *tx)
        .await
        .map_err(AppError::Db)?;
        if under_judicial_review_status_id.is_some() {
            judicial::set_subject_post_under_judicial_review_tx(&mut tx, root_post_id, user_id)
                .await
                .map_err(AppError::Db)?;
        }
    }

    tx.commit().await.map_err(AppError::Db)?;

    if is_emergency {
        if let Some(author_user_id) = row.post_created_by {
            crate::email::notify_emergency_filed_to_author(
                &state.db,
                &state.config,
                author_user_id,
                root_post_id,
            )
            .await;
        }
        crate::email::notify_emergency_filed_to_filer(
            &state.db,
            &state.config,
            user_id,
            root_post_id,
            judicial_case_id,
        )
        .await;
    } else {
        if let Err(err) = crate::jobs::enqueue_job(
            &state.db,
            None,
            "judicial_jury_nomination",
            &serde_json::json!({ "limit": 32 }),
            Some(&format!("judicial-case:{judicial_case_id}")),
            30,
        )
        .await
        {
            tracing::warn!(
                error = %err,
                judicial_case_id,
                "failed to enqueue judicial_jury_nomination on filing; scheduler will retry"
            );
        }
    }

    let ai_actor_id: Option<u64> = sqlx::query_scalar(
        r#"SELECT id FROM ai_actor WHERE is_active = 1 ORDER BY id ASC LIMIT 1"#,
    )
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?;
    if let Some(actor_id) = ai_actor_id {
        if let Ok(log_res) = sqlx::query(
            r#"
            INSERT INTO ai_judicial_decision_log (
                ai_actor_id,
                subject_ref,
                decision,
                rationale
            )
            VALUES (?, ?, 'user_judicial_case_filed', ?)
            "#,
        )
        .bind(actor_id)
        .bind(format!("judicial_case:{judicial_case_id}"))
        .bind(Some(rationale_sanitized.as_str()))
        .execute(&state.db)
        .await
        {
            let log_id = log_res.last_insert_id();
            for clause_id in &clause_ids {
                let _ = sqlx::query(
                    r#"
                    INSERT INTO ai_judicial_decision_clause (judicial_decision_log_id, constitution_item_id)
                    VALUES (?, ?)
                    ON DUPLICATE KEY UPDATE constitution_item_id = VALUES(constitution_item_id)
                    "#,
                )
                .bind(log_id)
                .bind(*clause_id)
                .execute(&state.db)
                .await;
            }
        }
    }

    let return_to = sanitize_return_to(body.return_to.as_deref());
    let target = with_notice_query(&return_to, "judicial_case_filed");
    Ok(Redirect::to(&target).into_response())
}

async fn render_judicial_post_appeal_page(
    state: &AppState,
    user_id: u64,
    post_row: &HomeFeedRow,
    prior_case_id_field: Option<u64>,
    err: Option<&str>,
    return_to: Option<&str>,
) -> AppResult<Response> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let connections = connections::list_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let rail_html = rail::connections_nav_html(&connections, &loc, c);
    let face_path = user_faces::artwork_display_src_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let noms_balance = human_onboarding::ai_tokens_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let self_ssr = human_onboarding::ssr_rating_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let title_html = topbar_title_with_suffix_html(
        connections.len(),
        &c.t(&loc, "ui.judicial_post_appeal.heading"),
    );
    let topbar_html = secured_shell_topbar_html(
        &state,
        user_id,
        &loc,
        &title_html,
        true,
        &snippets::topbar_avatar_html(
            face_path.as_deref(),
            true,
            &c.t(&loc, "ui.home.logo_aria_home"),
            self_ssr,
            None,
        ),
        false,
    )
    .await?;
    let self_private_handle = human_onboarding::human_entity_handle_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?
        .unwrap_or_default();
    let viewer_country = human_onboarding::country_code_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let viewer_human_entity_id =
        human_onboarding::primary_human_entity_id(&state.db, user_id)
            .await
            .map_err(AppError::Db)?;
    let preview_map = preview_image_allowed_for_feed_rows(&state.db, std::slice::from_ref(post_row))
        .await
        .map_err(AppError::Db)?;
    let post_card_html = feed_post_card_html(
        post_row,
        user_id,
        viewer_human_entity_id,
        false,
        false,
        viewer_country.as_deref(),
        crate::content::domain_reputation::preview_allowed_for_domain_id(post_row.domain_id, &preview_map),
        &state.config.public_base_url,
        c,
        &loc,
    );
    let rt = return_to.unwrap_or("/home");
    let return_to_esc = escape_html(rt);
    let cost_line = nom_icon::nom_cost_line_html(
        &c,
        &loc,
        BillableService::PostDisputeCouncilAppeal.nom_cost(),
    );
    let form_card = if let Some(prior_id) = prior_case_id_field {
        let err_html = err
            .map(|m| nom_icon::flash_alert_html(m))
            .unwrap_or_default();
        format!(
            r#"<section class="card app-card judicial-post-case-card"><div class="card-accent" role="presentation"></div><h2>{heading}</h2>{err_html}<p class="sub">{cost}</p><p class="sub">{hint}</p><form id="judicial-appeal-form" method="post" action="/home/judicial/post/{post_id}/appeal"><input type="hidden" name="prior_case_id" value="{prior_id}"><input type="hidden" name="return_to" value="{return_to_esc}"><label class="field-label" for="judicial-appeal-rationale-editor">{reason_label}</label><div class="create-post-editor-toolbar"><button type="button" class="create-post-editor-btn" data-editor-cmd="bold" aria-label="Bold"><strong>B</strong></button><button type="button" class="create-post-editor-btn" data-editor-cmd="italic" aria-label="Italic"><em>I</em></button><button type="button" class="create-post-editor-btn" data-editor-cmd="underline" aria-label="Underline"><u>U</u></button></div><div id="judicial-appeal-rationale-editor" class="create-post-editor" contenteditable="true" data-placeholder="{reason_placeholder}" aria-label="{reason_label}"></div><input id="judicial-appeal-rationale-input" type="hidden" name="rationale" value=""><p id="judicial-appeal-rationale-error" class="sub" role="alert" hidden>{reason_required}</p><button type="submit" class="auth-text-btn btn-cta">{submit}</button></form></section>"#,
            heading = escape_html(&c.t(&loc, "ui.judicial_post_appeal.heading")),
            err_html = err_html,
            cost = cost_line,
            hint = escape_html(&c.t(&loc, "ui.judicial_post_appeal.hint")),
            post_id = post_row.id,
            prior_id = prior_id,
            return_to_esc = return_to_esc,
            reason_label = escape_html(&c.t(&loc, "ui.judicial_post_appeal.reason_label")),
            reason_placeholder = escape_html(&c.t(&loc, "ui.judicial_post_appeal.reason_placeholder")),
            reason_required = escape_html(&c.t(&loc, "judicial_case_filing.reason_required")),
            submit = escape_html(&c.t(&loc, "ui.judicial_post_appeal.submit")),
        )
    } else {
        let text = err
            .map(String::from)
            .unwrap_or_else(|| c.t(&loc, "judicial_case_filing.appeal_case_required"));
        format!(
            r#"<section class="card app-card judicial-post-case-card"><div class="card-accent" role="presentation"></div><h2>{heading}</h2><p class="sub">{msg}</p><p class="sub"><a href="/home">{back}</a></p></section>"#,
            heading = escape_html(&c.t(&loc, "ui.judicial_post_appeal.heading")),
            msg = escape_html(&text),
            back = escape_html(&c.t(&loc, "ui.shared.home")),
        )
    };
    let html = JUDICIAL_POST_APPEAL_TEMPLATE
        .replace("__HTML_LANG__", &loc)
        .replace(
            "__JUD_POST_APPEAL_TITLE__",
            &escape_html(&c.t(&loc, "ui.judicial_post_appeal.title")),
        )
        .replace("__HOME_CONNECTION_COUNT__", &connections.len().to_string())
        .replace("__SECURED_TOPBAR__", &topbar_html)
        .replace("__HOME_APP_STAGE_CLASS__", "app-stage")
        .replace("__HOME_RAIL_HTML__", &rail_html)
        .replace("__HOME_SELF_PRIVATE_HANDLE__", &self_private_handle)
        .replace("__JUD_POST_CASE_POST_CARD__", &post_card_html)
        .replace("__JUD_POST_APPEAL_FORM_CARD__", &form_card)
        .replace(
            "__HOME_SHELL_I18N_JSON__",
            &escape_json_for_html_script(&home_shell_i18n_json(
                &loc,
                connections.len(),
                Some(self_private_handle.as_str()).filter(|s| !s.is_empty()),
            )),
        );
    Ok(Html(i18n::subst_markers(
        &snippets::inject_secured_public_footer_localized(&html, &c, &loc),
        &loc,
    ))
    .into_response())
}

async fn home_judicial_post_appeal_page(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(post_id): Path<u64>,
    Query(q): Query<JudicialAppealPageQuery>,
) -> AppResult<Response> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let Some(row) = feed::fetch_visible_root_post_for_viewer(&state.db, user_id, post_id)
        .await
        .map_err(AppError::Db)?
    else {
        return render_judicial_post_case_unavailable_page(&state, user_id, None).await;
    };
    let root_post_id = row.id;
    let prior_id = q
        .from_case
        .as_deref()
        .and_then(|s| s.trim().parse::<u64>().ok());
    let mut err: Option<String> = None;
    if let Some(pid) = prior_id {
        match load_prior_resolved_post_dispute_case(&state.db, pid, root_post_id)
            .await
            .map_err(AppError::Db)?
        {
            None => {
                err = Some(c.t(&loc, "judicial_case_filing.appeal_prior_invalid"));
            }
            Some(ref pr) => {
                if judicial_case_appeal_already_exists(&state.db, pid)
                    .await
                    .map_err(AppError::Db)?
                {
                    err = Some(c.t(&loc, "judicial_case_filing.appeal_already_filed"));
                } else if appeal_originator_and_target_for_user(&state.db, user_id, pr)
                    .await
                    .map_err(AppError::Db)?
                    .is_none()
                {
                    err = Some(c.t(&loc, "judicial_case_filing.appeal_not_party"));
                }
            }
        }
    }
    render_judicial_post_appeal_page(
        &state,
        user_id,
        &row,
        prior_id,
        err.as_deref(),
        q.return_to.as_deref(),
    )
    .await
}

async fn home_judicial_post_appeal_submit(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(post_id): Path<u64>,
    Form(body): Form<JudicialAppealFormBody>,
) -> AppResult<Response> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let Some(row) = feed::fetch_visible_root_post_for_viewer(&state.db, user_id, post_id)
        .await
        .map_err(AppError::Db)?
    else {
        return render_judicial_post_case_unavailable_page(&state, user_id, None).await;
    };
    let root_post_id = row.id;
    let prior_id: u64 = match body.prior_case_id.trim().parse() {
        Ok(v) if v > 0 => v,
        _ => {
            return render_judicial_post_appeal_page(
                &state,
                user_id,
                &row,
                None,
                Some(&c.t(&loc, "judicial_case_filing.appeal_prior_invalid")),
                body.return_to.as_deref(),
            )
            .await;
        }
    };
    let Some(prior) = load_prior_resolved_post_dispute_case(&state.db, prior_id, root_post_id)
        .await
        .map_err(AppError::Db)?
    else {
        return render_judicial_post_appeal_page(
            &state,
            user_id,
            &row,
            Some(prior_id),
            Some(&c.t(&loc, "judicial_case_filing.appeal_prior_invalid")),
            body.return_to.as_deref(),
        )
        .await;
    };
    if judicial_case_appeal_already_exists(&state.db, prior_id)
        .await
        .map_err(AppError::Db)?
    {
        return render_judicial_post_appeal_page(
            &state,
            user_id,
            &row,
            Some(prior_id),
            Some(&c.t(&loc, "judicial_case_filing.appeal_already_filed")),
            body.return_to.as_deref(),
        )
        .await;
    }
    let Some((appeal_origin, appeal_target)) =
        appeal_originator_and_target_for_user(&state.db, user_id, &prior)
            .await
            .map_err(AppError::Db)?
    else {
        return render_judicial_post_appeal_page(
            &state,
            user_id,
            &row,
            Some(prior_id),
            Some(&c.t(&loc, "judicial_case_filing.appeal_not_party")),
            body.return_to.as_deref(),
        )
        .await;
    };

    let rationale_html = body.rationale.unwrap_or_default();
    let rationale_sanitized = content_sanitize::sanitize_post_markup(&rationale_html);
    let rationale_plain = strip_tags_plain(&rationale_sanitized);
    if rationale_plain.trim().is_empty() {
        return render_judicial_post_appeal_page(
            &state,
            user_id,
            &row,
            Some(prior_id),
            Some(&c.t(&loc, "judicial_case_filing.reason_required")),
            body.return_to.as_deref(),
        )
        .await;
    }

    let Some(primary_human_entity_id) =
        human_onboarding::primary_human_entity_id(&state.db, user_id)
            .await
            .map_err(AppError::Db)?
    else {
        return render_judicial_post_appeal_page(
            &state,
            user_id,
            &row,
            Some(prior_id),
            Some(&c.t(&loc, "judicial_case_filing.no_primary_profile")),
            body.return_to.as_deref(),
        )
        .await;
    };

    let trigger_appeal_id = ref_key_values::lookup_active_id(
        &state.db,
        ref_keys::GROUP_JUDICIAL_CASE_TRIGGER,
        ref_keys::KEY_JUDICIAL_TRIGGER_CASE_APPEAL,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| AppError::BadRequest(c.t(&loc, "judicial_case_filing.refs_not_configured")))?;
    let status_escalated_id = ref_key_values::lookup_active_id(
        &state.db,
        ref_keys::GROUP_JUDICIAL_CASE_STATUS,
        ref_keys::KEY_JUDICIAL_CASE_ESCALATED_TO_AI,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| AppError::BadRequest(c.t(&loc, "judicial_case_filing.refs_not_configured")))?;
    let type_originator_id = ref_key_values::lookup_active_id(
        &state.db,
        ref_keys::GROUP_JUDICIAL_CASE_COMMENT_TYPE,
        ref_keys::KEY_JCC_ORIGINATOR,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| AppError::BadRequest(c.t(&loc, "judicial_case_filing.refs_not_configured")))?;
    let type_evidence_id = ref_key_values::lookup_active_id(
        &state.db,
        ref_keys::GROUP_JUDICIAL_CASE_COMMENT_TYPE,
        ref_keys::KEY_JCC_EVIDENCE,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| AppError::BadRequest(c.t(&loc, "judicial_case_filing.refs_not_configured")))?;

    let url_list = judicial_link_evidence::collect_external_urls(
        &[
            row.body_html_sanitized.as_str(),
            row.body_markup.as_str(),
            rationale_sanitized.as_str(),
        ],
        &[row.link_href.as_deref()],
    );
    let evidence_client = judicial_link_evidence::evidence_http_client().map_err(|e| {
        AppError::BadRequest(format!("judicial link evidence client: {e}"))
    })?;
    let link_snapshots =
        judicial_link_evidence::capture_external_urls(&evidence_client, &url_list).await;

    let mut tx = state.db.begin().await.map_err(AppError::Db)?;
    let appeal_service =
        crate::pricing::BillableService::PostDisputeCouncilAppeal;
    let spend_ref = format!("judicial_appeal:prior_case:{prior_id}");
    let spend_outcome = noms::spend_for_service_tx(
        &mut tx,
        primary_human_entity_id,
        appeal_service,
        user_id,
        Some(&spend_ref),
    )
    .await
    .map_err(AppError::Db)?;
    if !spend_outcome.is_paid() {
        let msg = nom_icon::insufficient_noms_html(
            &c,
            &loc,
            "noms.insufficient_for_judicial_appeal",
            BillableService::PostDisputeCouncilAppeal.nom_cost(),
            None,
        );
        return render_judicial_post_appeal_page(
            &state,
            user_id,
            &row,
            Some(prior_id),
            Some(&msg),
            body.return_to.as_deref(),
        )
        .await;
    }

    let resolution_tier_ai_id = ref_key_values::lookup_active_id(
        &state.db,
        ref_keys::GROUP_JUDICIAL_RESOLUTION_TIER,
        ref_keys::KEY_JUDICIAL_RESOLUTION_TIER_AI_COUNCIL,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| AppError::BadRequest(c.t(&loc, "judicial_case_filing.refs_not_configured")))?;

    sqlx::query(
        r#"
        UPDATE judicial_case
        SET modified_by = ?
        WHERE id = ?
        "#,
    )
    .bind(user_id)
    .bind(prior_id)
    .execute(&mut *tx)
    .await
    .map_err(AppError::Db)?;

    let originator_rating: Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(se.rating, 0) FROM social_entity se WHERE se.id = ? LIMIT 1"#,
    )
    .bind(appeal_origin)
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::Db)?;

    let rationale_markup = rationale_html.trim();
    let rationale_markup_stored = if rationale_markup.is_empty() {
        format!("<p>{}</p>", escape_html(rationale_plain.trim()))
    } else {
        rationale_markup.to_string()
    };

    let jc_res = sqlx::query(
        r#"
        INSERT INTO judicial_case (
            originator_social_entity_id,
            target_social_entity_id,
            trigger_type_id,
            status_id,
            subject_post_id,
            appealed_from_judicial_case_id,
            jury_votes_for,
            jury_votes_against,
            ai_votes_for,
            ai_votes_against,
            resolution_tier_id,
            jury_size,
            created_by,
            modified_by
        )
        VALUES (?, ?, ?, ?, ?, ?, 0, 0, 0, 0, ?, 0, ?, ?)
        "#,
    )
    .bind(appeal_origin)
    .bind(appeal_target)
    .bind(trigger_appeal_id)
    .bind(status_escalated_id)
    .bind(root_post_id)
    .bind(prior_id)
    .bind(resolution_tier_ai_id)
    .bind(user_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(AppError::Db)?;
    let judicial_case_id = jc_res.last_insert_id();

    judicial_link_evidence::insert_captured_rows_tx(&mut tx, judicial_case_id, &link_snapshots, user_id)
        .await
        .map_err(AppError::Db)?;

    sqlx::query(
        r#"
        INSERT INTO judicial_case_constitution_item (judicial_case_id, constitution_item_id, created_by)
        SELECT ?, cci.constitution_item_id, ?
        FROM judicial_case_constitution_item AS cci
        WHERE cci.judicial_case_id = ?
        "#,
    )
    .bind(judicial_case_id)
    .bind(user_id)
    .bind(prior_id)
    .execute(&mut *tx)
    .await
    .map_err(AppError::Db)?;

    sqlx::query(
        r#"
        INSERT INTO judicial_case_comment (
            judicial_case_id,
            type_id,
            body_plain,
            body_markup,
            body_html_sanitized,
            author_user_id,
            author_social_entity_id,
            author_entity_rating_at_create,
            in_reply_to_comment_id,
            created_by,
            modified_by
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
        "#,
    )
    .bind(judicial_case_id)
    .bind(type_originator_id)
    .bind(rationale_plain.as_str())
    .bind(rationale_markup_stored.as_str())
    .bind(rationale_sanitized.as_str())
    .bind(user_id)
    .bind(appeal_origin)
    .bind(originator_rating)
    .bind(user_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(AppError::Db)?;

    let evidence_plain = c.tf(
        &loc,
        "judicial_case_filing.evidence_comment_plain",
        &[("post_id", &root_post_id.to_string())],
    );
    let evidence_markup = format!("<p>{}</p>", escape_html(&evidence_plain));
    let evidence_sanitized = content_sanitize::sanitize_post_markup(&evidence_markup);

    sqlx::query(
        r#"
        INSERT INTO judicial_case_comment (
            judicial_case_id,
            type_id,
            body_plain,
            body_markup,
            body_html_sanitized,
            author_user_id,
            author_social_entity_id,
            author_entity_rating_at_create,
            in_reply_to_comment_id,
            linked_post_id,
            created_by,
            modified_by
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?, ?)
        "#,
    )
    .bind(judicial_case_id)
    .bind(type_evidence_id)
    .bind(evidence_plain.as_str())
    .bind(evidence_markup.as_str())
    .bind(evidence_sanitized.as_str())
    .bind(user_id)
    .bind(appeal_origin)
    .bind(originator_rating)
    .bind(root_post_id)
    .bind(user_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(AppError::Db)?;

    judicial::set_subject_post_under_judicial_review_tx(&mut tx, root_post_id, user_id)
        .await
        .map_err(AppError::Db)?;

    tx.commit().await.map_err(AppError::Db)?;

    let ai_actor_id: Option<u64> = sqlx::query_scalar(
        r#"SELECT id FROM ai_actor WHERE is_active = 1 ORDER BY id ASC LIMIT 1"#,
    )
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?;
    if let Some(actor_id) = ai_actor_id {
        let _ = sqlx::query(
            r#"
            INSERT INTO ai_judicial_decision_log (
                ai_actor_id,
                subject_ref,
                decision,
                rationale
            )
            VALUES (?, ?, 'user_judicial_appeal_filed', ?)
            "#,
        )
        .bind(actor_id)
        .bind(format!("judicial_case:{judicial_case_id}"))
        .bind(Some(rationale_sanitized.as_str()))
        .execute(&state.db)
        .await;
    }

    if let Err(err) = crate::jobs::enqueue_job(
        &state.db,
        Some(user_id),
        "judicial_council_appeal",
        &serde_json::json!({
            "case_limit": 1,
            "judicial_case_id": judicial_case_id
        }),
        Some(&format!("judicial-council-appeal-{judicial_case_id}")),
        45,
    )
    .await
    {
        tracing::warn!(
            error = %err,
            judicial_case_id,
            "failed to enqueue judicial_council_appeal on appeal filing"
        );
    }

    let return_to = sanitize_return_to(body.return_to.as_deref());
    let target = with_notice_query(&return_to, "judicial_appeal_filed");
    Ok(Redirect::to(&target).into_response())
}

fn rich_text_editor_toolbar_html() -> &'static str {
    r#"<button type="button" class="create-post-editor-btn" data-editor-cmd="bold" aria-label="Bold"><strong>B</strong></button><button type="button" class="create-post-editor-btn" data-editor-cmd="italic" aria-label="Italic"><em>I</em></button><button type="button" class="create-post-editor-btn" data-editor-cmd="underline" aria-label="Underline"><u>U</u></button>"#
}

async fn community_amendment_target_for_member(
    pool: &sqlx::MySqlPool,
    viewer_user_id: u64,
    handle: &str,
) -> Result<Option<CommunityAmendmentTargetRow>, sqlx::Error> {
    let Some(viewer_human_id) =
        human_onboarding::primary_human_entity_id(pool, viewer_user_id).await?
    else {
        return Ok(None);
    };
    sqlx::query_as::<_, CommunityAmendmentTargetRow>(
        r#"
        SELECT
            se.id AS entity_id,
            se.handle,
            COALESCE(NULLIF(TRIM(sc.label), ''), se.handle) AS display_label
        FROM social_entity se
        INNER JOIN social_community sc ON sc.social_entity_id = se.id
        INNER JOIN ref_key_values et
            ON et.id = se.type_id
           AND et.`group` = ?
           AND et.`key` = ?
        WHERE se.handle = ?
          AND EXISTS (
                SELECT 1
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
                  AND ser.to_social_entity_id = se.id
                  AND ser.ended_at IS NULL
          )
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(ref_keys::KEY_ENTITY_TYPE_COMMUNITY)
    .bind(handle)
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_PENDING_VERIFICATION)
    .bind(ref_keys::KEY_VERIFIED)
    .bind(ref_keys::KEY_FLAGGED)
    .bind(viewer_human_id)
    .fetch_optional(pool)
    .await
}

async fn owned_constitution_item_belongs_to_community(
    pool: &sqlx::MySqlPool,
    community_entity_id: u64,
    item_id: u64,
) -> Result<bool, sqlx::Error> {
    let n: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM community_constitution_item cci
        WHERE cci.id = ?
          AND cci.community_id = ?
          AND cci.status = 'active'
        "#,
    )
    .bind(item_id)
    .bind(community_entity_id)
    .fetch_one(pool)
    .await?;
    Ok(n > 0)
}

fn normalize_amendment_kind(raw: &str) -> Option<&'static str> {
    match raw.trim() {
        AMEND_KIND_NEW => Some(AMEND_KIND_NEW),
        AMEND_KIND_AMEND => Some(AMEND_KIND_AMEND),
        AMEND_KIND_REMOVE => Some(AMEND_KIND_REMOVE),
        _ => None,
    }
}

async fn render_judicial_community_amendment_page(
    state: &AppState,
    user_id: u64,
    community: &CommunityAmendmentTargetRow,
    form_state: &JudicialCommunityAmendmentForm,
    err: Option<&str>,
    success_msg: Option<&str>,
    return_to: Option<&str>,
) -> AppResult<Response> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let connections = connections::list_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let rail_html = rail::connections_nav_html(&connections, &loc, c);
    let face_path = user_faces::artwork_display_src_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let noms_balance = human_onboarding::ai_tokens_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let self_ssr = human_onboarding::ssr_rating_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let title_html = topbar_title_with_suffix_html(
        connections.len(),
        &c.t(&loc, "ui.judicial_community_amendment.heading"),
    );
    let topbar_html = secured_shell_topbar_html(
        &state,
        user_id,
        &loc,
        &title_html,
        true,
        &snippets::topbar_avatar_html(
            face_path.as_deref(),
            true,
            &c.t(&loc, "ui.home.logo_aria_home"),
            self_ssr,
            None,
        ),
        false,
    )
    .await?;
    let owned = community_constitution::fetch_owned_constitution_clauses(&state.db, community.entity_id)
        .await
        .map_err(AppError::Db)?;
    let kind = form_state.amendment_kind.trim();
    let selected_target = form_state
        .target_constitution_item_id
        .as_deref()
        .and_then(|s| s.trim().parse::<u64>().ok());
    let mut target_options = String::new();
    for clause in &owned {
        let checked = selected_target == Some(clause.id);
        target_options.push_str(&format!(
            r#"<label class="auth-check-label"><input class="auth-check-input" type="radio" name="target_constitution_item_id" value="{id}"{checked}><span class="auth-check-text"><span class="judicial-clause-ref mono">#{point}</span><span class="judicial-clause-body">{item}</span></span></label>"#,
            id = clause.id,
            checked = if checked { " checked" } else { "" },
            point = clause.point,
            item = escape_html(clause.item.trim()),
        ));
    }
    if target_options.is_empty() {
        target_options.push_str(&format!(
            r#"<p class="sub">{}</p>"#,
            escape_html(&c.t(&loc, "ui.judicial_community_amendment.no_owned_clauses"))
        ));
    }
    let kind_options = [
        (AMEND_KIND_NEW, c.t(&loc, "ui.judicial_community_amendment.kind_new")),
        (AMEND_KIND_AMEND, c.t(&loc, "ui.judicial_community_amendment.kind_amend")),
        (AMEND_KIND_REMOVE, c.t(&loc, "ui.judicial_community_amendment.kind_remove")),
    ];
    let mut kind_select = String::new();
    for (key, label) in kind_options {
        let sel = if kind == key { " selected" } else { "" };
        kind_select.push_str(&format!(
            r#"<option value="{key}"{sel}>{label}</option>"#,
            key = escape_html(key),
            sel = sel,
            label = escape_html(&label),
        ));
    }
    let toolbar = rich_text_editor_toolbar_html();
    let err_html = err
        .map(|m| nom_icon::flash_alert_html(m))
        .unwrap_or_default();
    let success_html = success_msg
        .map(|m| nom_icon::flash_message_html(&m))
        .unwrap_or_default();
    let context_card = format!(
        r#"<section class="card app-card profile-card profile-card--community"><div class="card-accent" role="presentation"></div><p class="public-post-handle-line mono">@{handle}</p><p class="public-post-display-name">{label}</p><p class="sub">{intro}</p></section>"#,
        handle = escape_html(&community.handle),
        label = escape_html(&community.display_label),
        intro = escape_html(&c.t(&loc, "ui.judicial_community_amendment.community_intro")),
    );
    let form_card = format!(
        r#"<section class="card app-card judicial-post-case-card"><div class="card-accent" role="presentation"></div><h2>{heading}</h2><p class="sub">{cost}</p><p class="sub">{hint}</p><p class="sub">{no_council}</p>{err}{success}<form id="judicial-community-amendment-form" method="post" action="/home/judicial/community/{handle}" novalidate><input type="hidden" name="return_to" value="{return_to}"><label class="field-label" for="judicial-amendment-kind">{kind_label}</label><div class="profile-connect-inline profile-connect-inline--solo"><select id="judicial-amendment-kind" name="amendment_kind" class="field-input profile-connect-select profile-connect-select--solo" required><option value="">{kind_default}</option>{kind_select}</select></div><div id="judicial-amendment-target-panel" hidden><p class="hint">{target_hint}</p>{target_options}<p id="judicial-amendment-target-error" class="sub" role="alert" hidden>{target_required}</p></div><div id="judicial-amendment-proposal-panel" hidden><label class="field-label" for="judicial-amendment-proposal-editor">{proposal_label}</label><div class="create-post-editor-toolbar" data-toolbar="proposal">{toolbar}</div><div id="judicial-amendment-proposal-editor" class="create-post-editor" contenteditable="true" data-placeholder="{proposal_placeholder}" aria-label="{proposal_label}"></div><input id="judicial-amendment-proposal-input" type="hidden" name="proposal" value=""><p id="judicial-amendment-proposal-error" class="sub" role="alert" hidden>{proposal_required}</p></div><div id="judicial-amendment-rationale-panel" hidden><label class="field-label" for="judicial-amendment-rationale-editor">{reason_label}</label><div class="create-post-editor-toolbar" data-toolbar="rationale">{toolbar}</div><div id="judicial-amendment-rationale-editor" class="create-post-editor" contenteditable="true" data-placeholder="{reason_placeholder}" aria-label="{reason_label}"></div><input id="judicial-amendment-rationale-input" type="hidden" name="rationale" value=""><p id="judicial-amendment-rationale-error" class="sub" role="alert" hidden>{reason_required}</p></div><button type="submit" class="auth-text-btn btn-cta">{submit}</button></form></section>"#,
        heading = escape_html(&c.t(&loc, "ui.judicial_community_amendment.form_heading")),
        cost = nom_icon::nom_cost_line_html(
            &c,
            &loc,
            BillableService::JudicialFiling(crate::pricing::JudicialTrigger::ConstitutionalAmendment)
                .nom_cost(),
        ),
        hint = escape_html(&c.t(&loc, "ui.judicial_community_amendment.hint")),
        no_council = escape_html(&c.t(&loc, "ui.judicial_community_amendment.no_council_appeal")),
        err = err_html,
        success = success_html,
        handle = escape_html(&community.handle),
        return_to = escape_html(return_to.unwrap_or("/home")),
        kind_label = escape_html(&c.t(&loc, "ui.judicial_community_amendment.kind_label")),
        kind_default = escape_html(&c.t(&loc, "ui.judicial_community_amendment.kind_default")),
        kind_select = kind_select,
        target_hint = escape_html(&c.t(&loc, "ui.judicial_community_amendment.target_hint")),
        target_options = target_options,
        target_required = escape_html(&c.t(&loc, "ui.judicial_community_amendment.target_required")),
        proposal_label = escape_html(&c.t(&loc, "ui.judicial_community_amendment.proposal_label")),
        proposal_placeholder = escape_html(&c.t(&loc, "ui.judicial_community_amendment.proposal_placeholder")),
        proposal_required = escape_html(&c.t(&loc, "ui.judicial_community_amendment.proposal_required")),
        reason_label = escape_html(&c.t(&loc, "ui.judicial_community_amendment.reason_label")),
        reason_placeholder = escape_html(&c.t(&loc, "ui.judicial_community_amendment.reason_placeholder")),
        reason_required = escape_html(&c.t(&loc, "judicial_case_filing.reason_required")),
        submit = escape_html(&c.t(&loc, "ui.judicial_community_amendment.submit")),
        toolbar = toolbar,
    );
    let self_private_handle = human_onboarding::human_entity_handle_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?
        .unwrap_or_default();
    let html = JUDICIAL_COMMUNITY_AMENDMENT_TEMPLATE
        .replace("__HTML_LANG__", &loc)
        .replace(
            "__JUD_COMMUNITY_AMEND_TITLE__",
            &escape_html(&c.t(&loc, "ui.judicial_community_amendment.title")),
        )
        .replace("__HOME_CONNECTION_COUNT__", &connections.len().to_string())
        .replace("__SECURED_TOPBAR__", &topbar_html)
        .replace("__HOME_APP_STAGE_CLASS__", "app-stage")
        .replace("__HOME_RAIL_HTML__", &rail_html)
        .replace("__HOME_SELF_PRIVATE_HANDLE__", &self_private_handle)
        .replace("__JUD_COMMUNITY_CONTEXT_CARD__", &context_card)
        .replace("__JUD_COMMUNITY_AMEND_FORM_CARD__", &form_card)
        .replace(
            "__HOME_SHELL_I18N_JSON__",
            &escape_json_for_html_script(&home_shell_i18n_json(
                &loc,
                connections.len(),
                Some(self_private_handle.as_str()).filter(|s| !s.is_empty()),
            )),
        );
    Ok(Html(i18n::subst_markers(
        &snippets::inject_secured_public_footer_localized(&html, &c, &loc),
        &loc,
    ))
    .into_response())
}

async fn home_judicial_community_amendment_page(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(handle): Path<String>,
    Query(q): Query<JudicialCommunityAmendmentPageQuery>,
) -> AppResult<Response> {
    let Some(normalized) = entity::normalize_handle_from_path_segment(&handle) else {
        return Err(AppError::NotFound);
    };
    let Some(community) =
        community_amendment_target_for_member(&state.db, user_id, &normalized).await?
    else {
        return Err(AppError::NotFound);
    };
    render_judicial_community_amendment_page(
        &state,
        user_id,
        &community,
        &JudicialCommunityAmendmentForm {
            amendment_kind: String::new(),
            target_constitution_item_id: None,
            proposal: None,
            rationale: None,
            return_to: q.return_to.clone(),
        },
        None,
        None,
        q.return_to.as_deref(),
    )
    .await
}

async fn home_judicial_community_amendment_submit(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(handle): Path<String>,
    Form(body): Form<JudicialCommunityAmendmentForm>,
) -> AppResult<Response> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let Some(normalized) = entity::normalize_handle_from_path_segment(&handle) else {
        return Err(AppError::NotFound);
    };
    let Some(community) =
        community_amendment_target_for_member(&state.db, user_id, &normalized).await?
    else {
        return Err(AppError::NotFound);
    };
    let Some(kind) = normalize_amendment_kind(&body.amendment_kind) else {
        let msg = c.t(&loc, "ui.judicial_community_amendment.kind_invalid");
        return render_judicial_community_amendment_page(
            &state,
            user_id,
            &community,
            &body,
            Some(&msg),
            None,
            body.return_to.as_deref(),
        )
        .await;
    };
    let target_id = body
        .target_constitution_item_id
        .as_deref()
        .and_then(|s| s.trim().parse::<u64>().ok());
    if matches!(kind, AMEND_KIND_AMEND | AMEND_KIND_REMOVE) {
        let Some(item_id) = target_id else {
            let msg = c.t(&loc, "ui.judicial_community_amendment.target_required");
            return render_judicial_community_amendment_page(
                &state,
                user_id,
                &community,
                &body,
                Some(&msg),
                None,
                body.return_to.as_deref(),
            )
            .await;
        };
        if !owned_constitution_item_belongs_to_community(&state.db, community.entity_id, item_id)
            .await
            .map_err(AppError::Db)?
        {
            let msg = c.t(&loc, "ui.judicial_community_amendment.target_invalid");
            return render_judicial_community_amendment_page(
                &state,
                user_id,
                &community,
                &body,
                Some(&msg),
                None,
                body.return_to.as_deref(),
            )
            .await;
        }
    }
    let proposal_html = body.proposal.as_deref().unwrap_or("").to_string();
    let proposal_sanitized = content_sanitize::sanitize_post_markup(&proposal_html);
    let proposal_plain = strip_tags_plain(&proposal_sanitized);
    if matches!(kind, AMEND_KIND_NEW | AMEND_KIND_AMEND) && proposal_plain.trim().is_empty() {
        let msg = c.t(&loc, "ui.judicial_community_amendment.proposal_required");
        return render_judicial_community_amendment_page(
            &state,
            user_id,
            &community,
            &body,
            Some(&msg),
            None,
            body.return_to.as_deref(),
        )
        .await;
    }
    let rationale_html = body.rationale.as_deref().unwrap_or("").to_string();
    let rationale_sanitized = content_sanitize::sanitize_post_markup(&rationale_html);
    let rationale_plain = strip_tags_plain(&rationale_sanitized);
    if rationale_plain.trim().is_empty() {
        let msg = c.t(&loc, "judicial_case_filing.reason_required");
        return render_judicial_community_amendment_page(
            &state,
            user_id,
            &community,
            &body,
            Some(&msg),
            None,
            body.return_to.as_deref(),
        )
        .await;
    }
    let Some(primary_human_entity_id) =
        human_onboarding::primary_human_entity_id(&state.db, user_id)
            .await
            .map_err(AppError::Db)?
    else {
        let msg = c.t(&loc, "judicial_case_filing.no_primary_profile");
        return render_judicial_community_amendment_page(
            &state,
            user_id,
            &community,
            &body,
            Some(&msg),
            None,
            body.return_to.as_deref(),
        )
        .await;
    };
    let trigger_id = ref_key_values::lookup_active_id(
        &state.db,
        ref_keys::GROUP_JUDICIAL_CASE_TRIGGER,
        ref_keys::KEY_JUDICIAL_TRIGGER_CONSTITUTIONAL_AMENDMENT,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| AppError::BadRequest(c.t(&loc, "judicial_case_filing.refs_not_configured")))?;
    let status_open_id = ref_key_values::lookup_active_id(
        &state.db,
        ref_keys::GROUP_JUDICIAL_CASE_STATUS,
        ref_keys::KEY_JUDICIAL_CASE_OPEN,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| AppError::BadRequest(c.t(&loc, "judicial_case_filing.refs_not_configured")))?;
    let type_originator_id = ref_key_values::lookup_active_id(
        &state.db,
        ref_keys::GROUP_JUDICIAL_CASE_COMMENT_TYPE,
        ref_keys::KEY_JCC_ORIGINATOR,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| AppError::BadRequest(c.t(&loc, "judicial_case_filing.refs_not_configured")))?;
    let mut tx = state.db.begin().await.map_err(AppError::Db)?;
    let filing_service = crate::pricing::BillableService::JudicialFiling(
        crate::pricing::JudicialTrigger::ConstitutionalAmendment,
    );
    let spend_ref = format!(
        "constitutional_amendment_filing:community:{}",
        community.entity_id
    );
    let spend_outcome = noms::spend_for_service_tx(
        &mut tx,
        primary_human_entity_id,
        filing_service,
        user_id,
        Some(&spend_ref),
    )
    .await
    .map_err(AppError::Db)?;
    if !spend_outcome.is_paid() {
        let msg = nom_icon::insufficient_noms_html(
            &c,
            &loc,
            "noms.insufficient_for_constitutional_amendment",
            BillableService::JudicialFiling(crate::pricing::JudicialTrigger::ConstitutionalAmendment)
                .nom_cost(),
            None,
        );
        return render_judicial_community_amendment_page(
            &state,
            user_id,
            &community,
            &body,
            Some(&msg),
            None,
            body.return_to.as_deref(),
        )
        .await;
    }
    let jury_size_frozen = judicial::compute_jury_size_to_freeze_at_filing_tx(
        &mut tx,
        primary_human_entity_id,
        community.entity_id,
        Some(user_id),
        None,
        ref_keys::KEY_JUDICIAL_TRIGGER_CONSTITUTIONAL_AMENDMENT,
    )
    .await
    .map_err(AppError::Db)?;
    let originator_rating: Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(se.rating, 0) FROM social_entity se WHERE se.id = ? LIMIT 1"#,
    )
    .bind(primary_human_entity_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::Db)?;
    let rationale_markup = rationale_html.trim();
    let rationale_markup_stored = if rationale_markup.is_empty() {
        format!("<p>{}</p>", escape_html(rationale_plain.trim()))
    } else {
        rationale_markup.to_string()
    };
    let proposal_markup = proposal_html.trim();
    let proposal_markup_stored = if proposal_markup.is_empty() {
        String::new()
    } else {
        proposal_markup.to_string()
    };
    let proposal_sanitized_stored = if proposal_markup_stored.is_empty() {
        String::new()
    } else {
        content_sanitize::sanitize_post_markup(&proposal_markup_stored)
    };
    let proposal_plain_stored = strip_tags_plain(&proposal_sanitized_stored);
    let jc_res = sqlx::query(
        r#"
        INSERT INTO judicial_case (
            originator_social_entity_id,
            target_social_entity_id,
            trigger_type_id,
            status_id,
            jury_size,
            jury_votes_for,
            jury_votes_against,
            ai_votes_for,
            ai_votes_against,
            created_by,
            modified_by
        )
        VALUES (?, ?, ?, ?, ?, 0, 0, 0, 0, ?, ?)
        "#,
    )
    .bind(primary_human_entity_id)
    .bind(community.entity_id)
    .bind(trigger_id)
    .bind(status_open_id)
    .bind(jury_size_frozen)
    .bind(user_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(AppError::Db)?;
    let judicial_case_id = jc_res.last_insert_id();
    sqlx::query(
        r#"
        INSERT INTO judicial_case_constitutional_amendment (
            judicial_case_id,
            amendment_kind,
            target_constitution_item_id,
            proposed_body_plain,
            proposed_body_markup,
            proposed_body_html_sanitized
        )
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(judicial_case_id)
    .bind(kind)
    .bind(target_id)
    .bind(proposal_plain_stored.as_str())
    .bind(proposal_markup_stored.as_str())
    .bind(proposal_sanitized_stored.as_str())
    .execute(&mut *tx)
    .await
    .map_err(AppError::Db)?;
    if let Some(item_id) = target_id {
        sqlx::query(
            r#"
            INSERT INTO judicial_case_constitution_item (judicial_case_id, constitution_item_id, created_by)
            VALUES (?, ?, ?)
            ON DUPLICATE KEY UPDATE created_by = COALESCE(VALUES(created_by), created_by)
            "#,
        )
        .bind(judicial_case_id)
        .bind(item_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Db)?;
    }
    sqlx::query(
        r#"
        INSERT INTO judicial_case_comment (
            judicial_case_id,
            type_id,
            body_plain,
            body_markup,
            body_html_sanitized,
            author_user_id,
            author_social_entity_id,
            author_entity_rating_at_create,
            in_reply_to_comment_id,
            created_by,
            modified_by
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
        "#,
    )
    .bind(judicial_case_id)
    .bind(type_originator_id)
    .bind(rationale_plain.as_str())
    .bind(rationale_markup_stored.as_str())
    .bind(rationale_sanitized.as_str())
    .bind(user_id)
    .bind(primary_human_entity_id)
    .bind(originator_rating)
    .bind(user_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(AppError::Db)?;
    tx.commit().await.map_err(AppError::Db)?;
    if let Err(err) = crate::jobs::enqueue_job(
        &state.db,
        None,
        "judicial_jury_nomination",
        &serde_json::json!({ "limit": 32 }),
        Some(&format!("judicial-case:{judicial_case_id}")),
        30,
    )
    .await
    {
        tracing::warn!(
            error = %err,
            judicial_case_id,
            "failed to enqueue judicial_jury_nomination on constitutional amendment filing"
        );
    }
    let return_to = sanitize_return_to(body.return_to.as_deref());
    let target = with_notice_query(&return_to, "constitutional_amendment_filed");
    Ok(Redirect::to(&target).into_response())
}

fn normalize_join_policy_amendment(raw: Option<&str>) -> Option<&'static str> {
    match raw.map(str::trim).filter(|s| !s.is_empty()) {
        None => None,
        Some(ref_keys::KEY_JOIN_POLICY_OPEN) => Some(ref_keys::KEY_JOIN_POLICY_OPEN),
        Some(ref_keys::KEY_JOIN_POLICY_REVIEWED) => Some(ref_keys::KEY_JOIN_POLICY_REVIEWED),
        Some(ref_keys::KEY_JOIN_POLICY_INVITE_ONLY) => Some(ref_keys::KEY_JOIN_POLICY_INVITE_ONLY),
        Some(ref_keys::KEY_JOIN_POLICY_CLOSED) => Some(ref_keys::KEY_JOIN_POLICY_CLOSED),
        Some(_) => None,
    }
}

async fn community_profile_amendment_target_for_member(
    pool: &sqlx::MySqlPool,
    viewer_user_id: u64,
    handle: &str,
) -> Result<Option<CommunityProfileAmendmentTargetRow>, sqlx::Error> {
    let Some(viewer_human_id) =
        human_onboarding::primary_human_entity_id(pool, viewer_user_id).await?
    else {
        return Ok(None);
    };
    sqlx::query_as::<_, CommunityProfileAmendmentTargetRow>(
        r#"
        SELECT
            se.id AS entity_id,
            se.handle,
            COALESCE(NULLIF(TRIM(sc.label), ''), se.handle) AS display_label,
            sc.description,
            sc.intention,
            rk_jp.`key` AS join_policy_key
        FROM social_entity se
        INNER JOIN social_community sc ON sc.social_entity_id = se.id
        INNER JOIN ref_key_values et
            ON et.id = se.type_id
           AND et.`group` = ?
           AND et.`key` = ?
        INNER JOIN ref_key_values rk_jp ON rk_jp.id = sc.join_policy_id
        WHERE se.handle = ?
          AND EXISTS (
                SELECT 1
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
                  AND ser.to_social_entity_id = se.id
                  AND ser.ended_at IS NULL
          )
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(ref_keys::KEY_ENTITY_TYPE_COMMUNITY)
    .bind(handle)
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE)
    .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_PENDING_VERIFICATION)
    .bind(ref_keys::KEY_VERIFIED)
    .bind(ref_keys::KEY_FLAGGED)
    .bind(viewer_human_id)
    .fetch_optional(pool)
    .await
}

fn join_policy_amendment_options_html(
    c: &i18n::Catalog,
    loc: &str,
    selected: Option<&str>,
) -> String {
    let policies = [
        ref_keys::KEY_JOIN_POLICY_OPEN,
        ref_keys::KEY_JOIN_POLICY_REVIEWED,
        ref_keys::KEY_JOIN_POLICY_INVITE_ONLY,
        ref_keys::KEY_JOIN_POLICY_CLOSED,
    ];
    let mut out = String::new();
    for key in policies {
        let sel = selected == Some(key);
        let label = crate::ui::join_policy::community_join_policy_display(c, loc, key);
        out.push_str(&format!(
            r#"<option value="{key}"{sel}>{label}</option>"#,
            key = escape_html(key),
            sel = if sel { " selected" } else { "" },
            label = escape_html(&label),
        ));
    }
    out
}

async fn render_judicial_community_profile_amendment_page(
    state: &AppState,
    user_id: u64,
    community: &CommunityProfileAmendmentTargetRow,
    form_state: &JudicialCommunityProfileAmendmentForm,
    err: Option<&str>,
    success_msg: Option<&str>,
    return_to: Option<&str>,
) -> AppResult<Response> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let connections = connections::list_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let rail_html = rail::connections_nav_html(&connections, &loc, c);
    let face_path = user_faces::artwork_display_src_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let noms_balance = human_onboarding::ai_tokens_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let self_ssr = human_onboarding::ssr_rating_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let title_html = topbar_title_with_suffix_html(
        connections.len(),
        &c.t(&loc, "ui.judicial_community_profile_amendment.heading"),
    );
    let topbar_html = secured_shell_topbar_html(
        &state,
        user_id,
        &loc,
        &title_html,
        true,
        &snippets::topbar_avatar_html(
            face_path.as_deref(),
            true,
            &c.t(&loc, "ui.home.logo_aria_home"),
            self_ssr,
            None,
        ),
        false,
    )
    .await?;
    let toolbar = rich_text_editor_toolbar_html();
    let err_html = err
        .map(|m| nom_icon::flash_alert_html(m))
        .unwrap_or_default();
    let success_html = success_msg
        .map(|m| nom_icon::flash_message_html(&m))
        .unwrap_or_default();
    let join_selected = form_state
        .join_policy
        .as_deref()
        .or(community.join_policy_key.as_deref());
    let join_options = join_policy_amendment_options_html(&c, &loc, join_selected);
    let location_note = escape_html(&c.t(&loc, "ui.judicial_community_profile.location_locked"));
    let context_card = format!(
        r#"<section class="card app-card profile-card profile-card--community"><div class="card-accent" role="presentation"></div><p class="public-post-handle-line mono">@{handle}</p><p class="public-post-display-name">{label}</p><p class="sub">{intro}</p><p class="sub">{location_note}</p></section>"#,
        handle = escape_html(&community.handle),
        label = escape_html(&community.display_label),
        intro = escape_html(&c.t(&loc, "ui.judicial_community_profile.community_intro")),
        location_note = location_note,
    );
    let form_card = format!(
        r#"<section class="card app-card judicial-post-case-card"><div class="card-accent" role="presentation"></div><h2>{heading}</h2><p class="sub">{cost}</p><p class="sub">{hint}</p><p class="sub">{no_council}</p>{err}{success}<form id="judicial-community-profile-amendment-form" method="post" action="/home/judicial/community/{handle}/profile" novalidate><input type="hidden" name="return_to" value="{return_to}"><label class="field-label" for="judicial-profile-description-editor">{about_label}</label><div class="create-post-editor-toolbar" data-toolbar="description">{toolbar}</div><div id="judicial-profile-description-editor" class="create-post-editor" contenteditable="true" data-placeholder="{about_placeholder}" aria-label="{about_label}"></div><input id="judicial-profile-description-input" type="hidden" name="description" value=""><label class="field-label" for="judicial-profile-intention-editor">{intention_label}</label><div class="create-post-editor-toolbar" data-toolbar="intention">{toolbar}</div><div id="judicial-profile-intention-editor" class="create-post-editor" contenteditable="true" data-placeholder="{intention_placeholder}" aria-label="{intention_label}"></div><input id="judicial-profile-intention-input" type="hidden" name="intention" value=""><label class="field-label" for="judicial-profile-join-policy">{join_label}</label><div class="profile-connect-inline profile-connect-inline--solo"><select id="judicial-profile-join-policy" name="join_policy" class="field-input profile-connect-select profile-connect-select--solo"><option value="">{join_keep}</option>{join_options}</select></div><label class="field-label" for="judicial-profile-rationale-editor">{reason_label}</label><div class="create-post-editor-toolbar" data-toolbar="rationale">{toolbar}</div><div id="judicial-profile-rationale-editor" class="create-post-editor" contenteditable="true" data-placeholder="{reason_placeholder}" aria-label="{reason_label}"></div><input id="judicial-profile-rationale-input" type="hidden" name="rationale" value=""><p id="judicial-profile-change-error" class="sub" role="alert" hidden>{change_required}</p><p id="judicial-profile-rationale-error" class="sub" role="alert" hidden>{reason_required}</p><button type="submit" class="auth-text-btn btn-cta">{submit}</button></form></section>"#,
        heading = escape_html(&c.t(&loc, "ui.judicial_community_profile.form_heading")),
        cost = nom_icon::nom_cost_line_html(
            &c,
            &loc,
            BillableService::JudicialFiling(crate::pricing::JudicialTrigger::CommunityProfileAmendment)
                .nom_cost(),
        ),
        hint = escape_html(&c.t(&loc, "ui.judicial_community_profile.hint")),
        no_council = escape_html(&c.t(&loc, "ui.judicial_community_profile.no_council_appeal")),
        err = err_html,
        success = success_html,
        handle = escape_html(&community.handle),
        return_to = escape_html(return_to.unwrap_or("/home")),
        about_label = escape_html(&c.t(&loc, "ui.judicial_community_profile.about_label")),
        about_placeholder = escape_html(&c.t(&loc, "ui.judicial_community_profile.about_placeholder")),
        intention_label = escape_html(&c.t(&loc, "ui.judicial_community_profile.intention_label")),
        intention_placeholder = escape_html(&c.t(&loc, "ui.judicial_community_profile.intention_placeholder")),
        join_label = escape_html(&c.t(&loc, "ui.judicial_community_profile.join_label")),
        join_keep = escape_html(&c.t(&loc, "ui.judicial_community_profile.join_keep_current")),
        join_options = join_options,
        reason_label = escape_html(&c.t(&loc, "ui.judicial_community_profile.reason_label")),
        reason_placeholder = escape_html(&c.t(&loc, "ui.judicial_community_profile.reason_placeholder")),
        change_required = escape_html(&c.t(&loc, "ui.judicial_community_profile.change_required")),
        reason_required = escape_html(&c.t(&loc, "judicial_case_filing.reason_required")),
        submit = escape_html(&c.t(&loc, "ui.judicial_community_profile.submit")),
        toolbar = toolbar,
    );
    let self_private_handle = human_onboarding::human_entity_handle_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?
        .unwrap_or_default();
    let html = JUDICIAL_COMMUNITY_PROFILE_AMENDMENT_TEMPLATE
        .replace("__HTML_LANG__", &loc)
        .replace(
            "__JUD_COMMUNITY_PROFILE_TITLE__",
            &escape_html(&c.t(&loc, "ui.judicial_community_profile.title")),
        )
        .replace("__HOME_CONNECTION_COUNT__", &connections.len().to_string())
        .replace("__SECURED_TOPBAR__", &topbar_html)
        .replace("__HOME_APP_STAGE_CLASS__", "app-stage")
        .replace("__HOME_RAIL_HTML__", &rail_html)
        .replace("__HOME_SELF_PRIVATE_HANDLE__", &self_private_handle)
        .replace("__JUD_COMMUNITY_CONTEXT_CARD__", &context_card)
        .replace("__JUD_COMMUNITY_PROFILE_FORM_CARD__", &form_card)
        .replace(
            "__HOME_SHELL_I18N_JSON__",
            &escape_json_for_html_script(&home_shell_i18n_json(
                &loc,
                connections.len(),
                Some(self_private_handle.as_str()).filter(|s| !s.is_empty()),
            )),
        );
    Ok(Html(i18n::subst_markers(
        &snippets::inject_secured_public_footer_localized(&html, &c, &loc),
        &loc,
    ))
    .into_response())
}

async fn home_judicial_community_profile_amendment_page(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(handle): Path<String>,
    Query(q): Query<JudicialCommunityProfileAmendmentPageQuery>,
) -> AppResult<Response> {
    let Some(normalized) = entity::normalize_handle_from_path_segment(&handle) else {
        return Err(AppError::NotFound);
    };
    let Some(community) =
        community_profile_amendment_target_for_member(&state.db, user_id, &normalized).await?
    else {
        return Err(AppError::NotFound);
    };
    render_judicial_community_profile_amendment_page(
        &state,
        user_id,
        &community,
        &JudicialCommunityProfileAmendmentForm {
            description: None,
            intention: None,
            join_policy: None,
            rationale: None,
            return_to: q.return_to.clone(),
        },
        None,
        None,
        q.return_to.as_deref(),
    )
    .await
}

async fn home_judicial_community_profile_amendment_submit(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(handle): Path<String>,
    Form(body): Form<JudicialCommunityProfileAmendmentForm>,
) -> AppResult<Response> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let Some(normalized) = entity::normalize_handle_from_path_segment(&handle) else {
        return Err(AppError::NotFound);
    };
    let Some(community) =
        community_profile_amendment_target_for_member(&state.db, user_id, &normalized).await?
    else {
        return Err(AppError::NotFound);
    };

    let current_description = community.description.as_deref().unwrap_or("").trim();
    let current_intention = community.intention.as_deref().unwrap_or("").trim();
    let current_join_key = community.join_policy_key.as_deref();

    let description_html = body.description.as_deref().unwrap_or("").to_string();
    let description_sanitized = content_sanitize::sanitize_post_markup(&description_html);
    let description_plain = strip_tags_plain(&description_sanitized);
    let change_description =
        !description_plain.trim().is_empty() && description_plain.trim() != current_description;

    let intention_html = body.intention.as_deref().unwrap_or("").to_string();
    let intention_sanitized = content_sanitize::sanitize_post_markup(&intention_html);
    let intention_plain = strip_tags_plain(&intention_sanitized);
    let change_intention =
        !intention_plain.trim().is_empty() && intention_plain.trim() != current_intention;

    let proposed_join_key = normalize_join_policy_amendment(body.join_policy.as_deref());
    if body
        .join_policy
        .as_deref()
        .is_some_and(|s| !s.trim().is_empty())
        && proposed_join_key.is_none()
    {
        let msg = c.t(&loc, "social_entity.community_join_policy_invalid");
        return render_judicial_community_profile_amendment_page(
            &state,
            user_id,
            &community,
            &body,
            Some(&msg),
            None,
            body.return_to.as_deref(),
        )
        .await;
    }
    let change_join_policy = proposed_join_key.is_some_and(|k| Some(k) != current_join_key);

    if !change_description && !change_intention && !change_join_policy {
        let msg = c.t(&loc, "ui.judicial_community_profile.change_required");
        return render_judicial_community_profile_amendment_page(
            &state,
            user_id,
            &community,
            &body,
            Some(&msg),
            None,
            body.return_to.as_deref(),
        )
        .await;
    }

    let rationale_html = body.rationale.as_deref().unwrap_or("").to_string();
    let rationale_sanitized = content_sanitize::sanitize_post_markup(&rationale_html);
    let rationale_plain = strip_tags_plain(&rationale_sanitized);
    if rationale_plain.trim().is_empty() {
        let msg = c.t(&loc, "judicial_case_filing.reason_required");
        return render_judicial_community_profile_amendment_page(
            &state,
            user_id,
            &community,
            &body,
            Some(&msg),
            None,
            body.return_to.as_deref(),
        )
        .await;
    }

    let Some(primary_human_entity_id) =
        human_onboarding::primary_human_entity_id(&state.db, user_id)
            .await
            .map_err(AppError::Db)?
    else {
        let msg = c.t(&loc, "judicial_case_filing.no_primary_profile");
        return render_judicial_community_profile_amendment_page(
            &state,
            user_id,
            &community,
            &body,
            Some(&msg),
            None,
            body.return_to.as_deref(),
        )
        .await;
    };

    let trigger_id = ref_key_values::lookup_active_id(
        &state.db,
        ref_keys::GROUP_JUDICIAL_CASE_TRIGGER,
        ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_PROFILE_AMENDMENT,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| AppError::BadRequest(c.t(&loc, "judicial_case_filing.refs_not_configured")))?;
    let status_open_id = ref_key_values::lookup_active_id(
        &state.db,
        ref_keys::GROUP_JUDICIAL_CASE_STATUS,
        ref_keys::KEY_JUDICIAL_CASE_OPEN,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| AppError::BadRequest(c.t(&loc, "judicial_case_filing.refs_not_configured")))?;
    let type_originator_id = ref_key_values::lookup_active_id(
        &state.db,
        ref_keys::GROUP_JUDICIAL_CASE_COMMENT_TYPE,
        ref_keys::KEY_JCC_ORIGINATOR,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| AppError::BadRequest(c.t(&loc, "judicial_case_filing.refs_not_configured")))?;

    let mut tx = state.db.begin().await.map_err(AppError::Db)?;
    let filing_service = crate::pricing::BillableService::JudicialFiling(
        crate::pricing::JudicialTrigger::CommunityProfileAmendment,
    );
    let spend_ref = format!(
        "community_profile_amendment_filing:community:{}",
        community.entity_id
    );
    let spend_outcome = noms::spend_for_service_tx(
        &mut tx,
        primary_human_entity_id,
        filing_service,
        user_id,
        Some(&spend_ref),
    )
    .await
    .map_err(AppError::Db)?;
    if !spend_outcome.is_paid() {
        let msg = nom_icon::insufficient_noms_html(
            &c,
            &loc,
            "noms.insufficient_for_community_profile_amendment",
            BillableService::JudicialFiling(crate::pricing::JudicialTrigger::CommunityProfileAmendment)
                .nom_cost(),
            None,
        );
        return render_judicial_community_profile_amendment_page(
            &state,
            user_id,
            &community,
            &body,
            Some(&msg),
            None,
            body.return_to.as_deref(),
        )
        .await;
    }

    let jury_size_frozen = judicial::compute_jury_size_to_freeze_at_filing_tx(
        &mut tx,
        primary_human_entity_id,
        community.entity_id,
        Some(user_id),
        None,
        ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_PROFILE_AMENDMENT,
    )
    .await
    .map_err(AppError::Db)?;

    let originator_rating: Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(se.rating, 0) FROM social_entity se WHERE se.id = ? LIMIT 1"#,
    )
    .bind(primary_human_entity_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::Db)?;

    let rationale_markup = rationale_html.trim();
    let rationale_markup_stored = if rationale_markup.is_empty() {
        format!("<p>{}</p>", escape_html(rationale_plain.trim()))
    } else {
        rationale_markup.to_string()
    };

    let proposal_desc_sanitized = if change_description {
        description_sanitized
    } else {
        String::new()
    };
    let proposal_desc_plain = if change_description {
        description_plain
    } else {
        String::new()
    };

    let proposal_int_sanitized = if change_intention {
        intention_sanitized
    } else {
        String::new()
    };
    let proposal_int_plain = if change_intention {
        intention_plain
    } else {
        String::new()
    };

    let jc_res = sqlx::query(
        r#"
        INSERT INTO judicial_case (
            originator_social_entity_id,
            target_social_entity_id,
            trigger_type_id,
            status_id,
            jury_size,
            jury_votes_for,
            jury_votes_against,
            ai_votes_for,
            ai_votes_against,
            created_by,
            modified_by
        )
        VALUES (?, ?, ?, ?, ?, 0, 0, 0, 0, ?, ?)
        "#,
    )
    .bind(primary_human_entity_id)
    .bind(community.entity_id)
    .bind(trigger_id)
    .bind(status_open_id)
    .bind(jury_size_frozen)
    .bind(user_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(AppError::Db)?;
    let judicial_case_id = jc_res.last_insert_id();

    use crate::social::judicial_community_profile::ProfileFieldSnapshot;
    use sqlx::types::Json;

    let current_snapshot = ProfileFieldSnapshot {
        description_plain: current_description.to_string(),
        description_html_sanitized: String::new(),
        intention_plain: current_intention.to_string(),
        intention_html_sanitized: String::new(),
        join_policy_key: current_join_key.map(str::to_string),
    };
    let proposed_snapshot = ProfileFieldSnapshot {
        description_plain: proposal_desc_plain.clone(),
        description_html_sanitized: proposal_desc_sanitized.clone(),
        intention_plain: proposal_int_plain.clone(),
        intention_html_sanitized: proposal_int_sanitized.clone(),
        join_policy_key: proposed_join_key.map(str::to_string),
    };

    sqlx::query(
        r#"
        INSERT INTO judicial_case_community_profile_amendment (
            judicial_case_id,
            change_description,
            change_intention,
            change_join_policy,
            current_snapshot,
            proposed_snapshot
        )
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(judicial_case_id)
    .bind(change_description)
    .bind(change_intention)
    .bind(change_join_policy)
    .bind(Json(current_snapshot))
    .bind(Json(proposed_snapshot))
    .execute(&mut *tx)
    .await
    .map_err(AppError::Db)?;

    sqlx::query(
        r#"
        INSERT INTO judicial_case_comment (
            judicial_case_id,
            type_id,
            body_plain,
            body_markup,
            body_html_sanitized,
            author_user_id,
            author_social_entity_id,
            author_entity_rating_at_create,
            in_reply_to_comment_id,
            created_by,
            modified_by
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
        "#,
    )
    .bind(judicial_case_id)
    .bind(type_originator_id)
    .bind(rationale_plain.as_str())
    .bind(rationale_markup_stored.as_str())
    .bind(rationale_sanitized.as_str())
    .bind(user_id)
    .bind(primary_human_entity_id)
    .bind(originator_rating)
    .bind(user_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(AppError::Db)?;

    tx.commit().await.map_err(AppError::Db)?;
    if let Err(err) = crate::jobs::enqueue_job(
        &state.db,
        None,
        "judicial_jury_nomination",
        &serde_json::json!({ "limit": 32 }),
        Some(&format!("judicial-case:{judicial_case_id}")),
        30,
    )
    .await
    {
        tracing::warn!(
            error = %err,
            judicial_case_id,
            "failed to enqueue judicial_jury_nomination on community profile amendment filing"
        );
    }
    let return_to = sanitize_return_to(body.return_to.as_deref());
    let target = with_notice_query(&return_to, "community_profile_amendment_filed");
    Ok(Redirect::to(&target).into_response())
}

fn sanitize_return_to(raw: Option<&str>) -> String {
    let v = raw.unwrap_or("/home").trim();
    if !v.starts_with('/') || v.starts_with("//") {
        return "/home".to_string();
    }
    if v.starts_with("/home/judicial/post/") {
        return "/home".to_string();
    }
    v.to_string()
}

fn with_notice_query(path: &str, notice: &str) -> String {
    let mut out = path.to_string();
    if out.contains('?') {
        out.push('&');
    } else {
        out.push('?');
    }
    out.push_str("notice=");
    out.push_str(notice);
    out
}

fn strip_tags_plain(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

async fn create_post_card_fragment() -> Response {
    let loc = i18n::request_locale();
    let html = i18n::subst_markers(CREATE_POST_FRAGMENT, &loc);
    let mut res = Html(html).into_response();
    res.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, must-revalidate"),
    );
    res
}

async fn search_add_card_fragment() -> Response {
    let loc = i18n::request_locale();
    let html = i18n::subst_markers(SEARCH_ADD_FRAGMENT, &loc);
    let mut res = Html(html).into_response();
    res.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, must-revalidate"),
    );
    res
}

async fn create_entity_card_fragment(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Query(q): Query<CreateEntityCardQuery>,
) -> AppResult<Response> {
    let type_key = q.entity_type.trim();
    let allowed = [
        ref_keys::KEY_ENTITY_TYPE_COMMUNITY,
        ref_keys::KEY_ENTITY_TYPE_GATHERING,
        ref_keys::KEY_ENTITY_TYPE_PROFIT,
        ref_keys::KEY_ENTITY_TYPE_NON_PROFIT,
    ];
    if !allowed.iter().any(|&k| k == type_key) {
        return Err(AppError::NotFound);
    }
    let Some(type_id) = ref_key_values::lookup_active_id(
        &state.db,
        ref_keys::GROUP_SOCIAL_ENTITY_TYPE,
        type_key,
    )
    .await
    .map_err(AppError::Db)?
    else {
        return Err(AppError::NotFound);
    };

    let location_id = entity::posting_anchor_location_id(&state.db, user_id).await?;
    let location_str = if type_key == ref_keys::KEY_ENTITY_TYPE_PROFIT {
        String::new()
    } else {
        location_id.map(|id| id.to_string()).unwrap_or_default()
    };

    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let title_key = match type_key {
        ref_keys::KEY_ENTITY_TYPE_COMMUNITY => "ui.home.create_entity_card_title_community",
        ref_keys::KEY_ENTITY_TYPE_GATHERING => "ui.home.create_entity_card_title_event",
        ref_keys::KEY_ENTITY_TYPE_PROFIT => "ui.home.create_entity_card_title_business",
        ref_keys::KEY_ENTITY_TYPE_NON_PROFIT => "ui.home.create_entity_card_title_nonprofit",
        _ => "ui.home.create_entity_card_title_community",
    };
    let title_esc = escape_html(&c.t(&loc, title_key));
    let type_key_esc = escape_html(type_key);

    let balance = human_onboarding::ai_tokens_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let nom_cost_line = BillableService::for_entity_type_key(type_key).map_or(String::new(), |service| {
        let cost = service.nom_cost();
        if cost == 0 {
            String::new()
        } else {
            c.tf(
                &loc,
                "ui.home.create_entity_nom_cost",
                &[
                    ("nom_price", &cost.to_string()),
                    ("balance", &balance.to_string()),
                ],
            )
        }
    });
    let nom_cost_esc = escape_html(&nom_cost_line);

    let html = if type_key == ref_keys::KEY_ENTITY_TYPE_PROFIT {
        CREATE_ENTITY_PROFIT_FRAGMENT
            .replace("__CREATE_ENTITY_TITLE__", &title_esc)
            .replace("__TYPE_ID__", &type_id.to_string())
            .replace("__CREATE_ENTITY_NOM_COST__", &nom_cost_esc)
    } else if type_key == ref_keys::KEY_ENTITY_TYPE_COMMUNITY {
        let country_rows = crate::ui::countries::community_country_option_rows_html(Some(
            crate::ui::countries::GLOBAL_COUNTRY_CODE,
        ));
        CREATE_ENTITY_COMMUNITY_FRAGMENT
            .replace("__CREATE_ENTITY_TITLE__", &title_esc)
            .replace("__TYPE_ID__", &type_id.to_string())
            .replace("__CREATE_ENTITY_NOM_COST__", &nom_cost_esc)
            .replace("__COMMUNITY_COUNTRY_OPTION_ROWS__", &country_rows)
            .replace("__COMMUNITY_PREFILL_REGION__", "")
            .replace("__COMMUNITY_PREFILL_LOCALITY__", "")
            .replace("__COMMUNITY_COUNTRY_SELECTED_EMPTY__", "")
    } else if type_key == ref_keys::KEY_ENTITY_TYPE_GATHERING {
        CREATE_ENTITY_GATHERING_FRAGMENT
            .replace("__CREATE_ENTITY_TITLE__", &title_esc)
            .replace("__TYPE_ID__", &type_id.to_string())
            .replace("__LOCATION_ID__", &location_str)
            .replace("__CREATE_ENTITY_NOM_COST__", &nom_cost_esc)
    } else if type_key == ref_keys::KEY_ENTITY_TYPE_NON_PROFIT {
        CREATE_ENTITY_NON_PROFIT_FRAGMENT
            .replace("__CREATE_ENTITY_TITLE__", &title_esc)
            .replace("__TYPE_ID__", &type_id.to_string())
            .replace("__LOCATION_ID__", &location_str)
            .replace("__CREATE_ENTITY_NOM_COST__", &nom_cost_esc)
    } else {
        CREATE_ENTITY_FRAGMENT
            .replace("__ENTITY_ACCENT_KEY__", type_key)
            .replace("__ENTITY_TYPE_KEY_ESC__", &type_key_esc)
            .replace("__CREATE_ENTITY_TITLE__", &title_esc)
            .replace("__TYPE_ID__", &type_id.to_string())
            .replace("__LOCATION_ID__", &location_str)
            .replace("__CREATE_ENTITY_NOM_COST__", &nom_cost_esc)
    };
    let html = i18n::subst_markers(&html, &loc);
    let mut res = Html(html).into_response();
    res.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, must-revalidate"),
    );
    Ok(res)
}

async fn private_profile_card_fragment(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(raw_handle): Path<String>,
) -> AppResult<Response> {
    let Some(normalized) = entity::normalize_handle_from_path_segment(&raw_handle) else {
        return Err(AppError::NotFound);
    };
    let has_full_access = status::user_has_verified_status(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    if !has_full_access {
        return Err(AppError::Unauthorized);
    }
    let Some(profile) = private_profile::fetch_by_handle(&state.db, user_id, &normalized)
        .await
        .map_err(AppError::Db)?
    else {
        return Err(AppError::NotFound);
    };
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let card_html = private_profile_card::entity_card_html(&profile, &loc, c, true);
    Ok(Html(card_html).into_response())
}

async fn private_profile_connect_submit(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(raw_handle): Path<String>,
    headers: HeaderMap,
    Form(body): Form<PrivateConnectForm>,
) -> AppResult<Json<serde_json::Value>> {
    profile_gate::require_profile_mutation(&state.db, &state.config, user_id, &headers).await?;
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let Some(normalized) = entity::normalize_handle_from_path_segment(&raw_handle) else {
        return Err(AppError::NotFound);
    };
    let Some(profile) = private_profile::fetch_by_handle(&state.db, user_id, &normalized)
        .await
        .map_err(AppError::Db)?
    else {
        return Err(AppError::NotFound);
    };
    if profile.type_label_key == ref_keys::KEY_ENTITY_TYPE_COMMUNITY {
        if body.connect_as.trim().to_ascii_lowercase() != "member" {
            return Err(AppError::BadRequest(
                c.t(&loc, "ui.entity_private.connection_type_invalid"),
            ));
        }
        let community_entity_id: u64 = sqlx::query_scalar(
            r#"
            SELECT se.id
            FROM social_entity se
            INNER JOIN social_community sc ON sc.social_entity_id = se.id
            WHERE se.handle = ?
            LIMIT 1
            "#,
        )
        .bind(normalized.as_str())
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::NotFound)?;
        let is_reviewed = profile.community_join_policy_key.as_deref()
            == Some(ref_keys::KEY_JOIN_POLICY_REVIEWED);
        if is_reviewed {
            if !profile.can_apply_for_membership_review {
                return Err(AppError::BadRequest(
                    c.t(&loc, "ui.entity_private.community_join_unavailable"),
                ));
            }
            let Some(viewer_human_id) =
                human_onboarding::primary_human_entity_id(&state.db, user_id)
                    .await
                    .map_err(AppError::Db)?
            else {
                return Err(AppError::BadRequest(
                    c.t(&loc, "ui.entity_private.human_profile_required"),
                ));
            };
            let raw_html = body.application_statement.unwrap_or_default();
            let sanitized = content_sanitize::sanitize_post_markup(&raw_html);
            let plain = strip_tags_plain(&sanitized);
            let case_id = judicial::membership_review::file_application(
                &state.db,
                user_id,
                viewer_human_id,
                community_entity_id,
                plain.trim(),
                raw_html.trim(),
                &sanitized,
            )
            .await?;
            if let Err(err) = crate::jobs::enqueue_job(
                &state.db,
                None,
                "judicial_jury_nomination",
                &serde_json::json!({ "limit": 32 }),
                Some(&format!("judicial-case:{case_id}")),
                30,
            )
            .await
            {
                tracing::warn!(
                    error = %err,
                    case_id,
                    "failed to enqueue judicial_jury_nomination on membership review filing"
                );
            }
            return Ok(Json(serde_json::json!({
                "ok": true,
                "message": c.t(&loc, "ui.entity_private.open_membership_review_notice"),
            })));
        }
        if !profile.can_request_community_membership {
            return Err(AppError::BadRequest(
                c.t(&loc, "ui.entity_private.community_join_unavailable"),
            ));
        }
        community_invite::request_membership(
            &state.db,
            user_id,
            community_entity_id,
            profile.community_join_policy_key.as_deref(),
        )
        .await?;
        return Ok(Json(serde_json::json!({
            "ok": true,
            "message": c.t(&loc, "ui.entity_private.community_joined_notice"),
        })));
    }

    let connect_as = body.connect_as.trim().to_ascii_lowercase();
    if connect_as == "block" {
        if !profile.can_block_human {
            return Err(AppError::BadRequest(
                c.t(&loc, "ui.entity_private.block_unavailable"),
            ));
        }
        let Some(viewer_human_id) = human_onboarding::primary_human_entity_id(&state.db, user_id)
            .await
            .map_err(AppError::Db)?
        else {
            return Err(AppError::BadRequest(
                c.t(&loc, "ui.entity_private.human_profile_required"),
            ));
        };
        let target_entity_id: u64 = sqlx::query_scalar(
            r#"
            SELECT se.id
            FROM social_entity se
            INNER JOIN social_human sh ON sh.social_entity_id = se.id
            WHERE se.handle = ?
            LIMIT 1
            "#,
        )
        .bind(normalized.as_str())
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::NotFound)?;
        blocks::block_human(&state.db, viewer_human_id, target_entity_id, user_id)
            .await
            .map_err(AppError::Db)?;
        return Ok(Json(serde_json::json!({
            "ok": true,
            "message": c.t(&loc, "ui.entity_private.block_success"),
        })));
    }

    if !profile.can_send_connection_invite {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.connection_invite_unavailable"),
        ));
    }

    let rel_key = match connect_as.as_str() {
        "friend" => ref_keys::KEY_RELATIONSHIP_FRIEND,
        "family" => ref_keys::KEY_RELATIONSHIP_FAMILY,
        "colleague" => ref_keys::KEY_RELATIONSHIP_COLLEAGUE,
        _ => {
            return Err(AppError::BadRequest(
                c.t(&loc, "ui.entity_private.connection_type_invalid"),
            ))
        }
    };
    let rel_type_id = ref_key_values::lookup_active_id(
        &state.db,
        ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE,
        rel_key,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| AppError::BadRequest(c.t(&loc, "ui.entity_private.connection_type_unconfigured")))?;
    let status_pending = ref_key_values::lookup_active_id(
        &state.db,
        ref_keys::GROUP_STATUS,
        ref_keys::KEY_PENDING_VERIFICATION,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| AppError::BadRequest(c.t(&loc, "ui.entity_private.pending_status_unconfigured")))?;
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
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?;
    let Some(from_entity_id) = viewer_human_entity_id else {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.human_profile_required"),
        ));
    };
    let to_entity_id: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT se.id
        FROM social_entity se
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = se.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` = ?
        WHERE se.handle = ?
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(ref_keys::KEY_ENTITY_TYPE_HUMAN)
    .bind(normalized.as_str())
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?;
    let Some(to_entity_id) = to_entity_id else {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.connection_human_only"),
        ));
    };

    let peer_slots = connections::distinct_peer_count_for_human_entity(&state.db, from_entity_id)
        .await
        .map_err(AppError::Db)?;
    if (peer_slots as usize) >= connections::MAX_CONNECTION_SLOTS {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.connection_slots_full"),
        ));
    }

    let target_peer_slots =
        connections::distinct_peer_count_for_human_entity(&state.db, to_entity_id)
            .await
            .map_err(AppError::Db)?;
    if (target_peer_slots as usize) >= connections::MAX_CONNECTION_SLOTS {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.connection_invite_unavailable"),
        ));
    }

    relationship::create(
        &state.db,
        relationship::CreateRequest {
            from_social_entity_id: from_entity_id.to_string(),
            to_social_entity_id: to_entity_id.to_string(),
            type_id: rel_type_id.to_string(),
            status_id: Some(status_pending.to_string()),
            started_at: None,
            ended_at: None,
        },
        user_id,
    )
    .await?;

    Ok(Json(serde_json::json!({
        "ok": true,
        "message": c.t(&loc, "ui.entity_private.connection_invitation_sent")
    })))
}

async fn private_profile_connect_action_submit(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(raw_handle): Path<String>,
    headers: HeaderMap,
    Form(body): Form<PrivateConnectActionForm>,
) -> AppResult<Response> {
    profile_gate::require_profile_mutation(&state.db, &state.config, user_id, &headers).await?;
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let Some(normalized) = entity::normalize_handle_from_path_segment(&raw_handle) else {
        return Err(AppError::NotFound);
    };
    let Some(target_profile) = private_profile::fetch_by_handle(&state.db, user_id, &normalized)
        .await
        .map_err(AppError::Db)?
    else {
        return Err(AppError::NotFound);
    };
    if !target_profile.can_respond_to_connection_invite {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.connection_invite_unavailable"),
        ));
    }
    let Some(viewer_human_id) = human_onboarding::primary_human_entity_id(&state.db, user_id)
        .await
        .map_err(AppError::Db)?
    else {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.human_profile_required"),
        ));
    };
    let target_entity_id: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT se.id
        FROM social_entity se
        INNER JOIN social_human sh ON sh.social_entity_id = se.id
        WHERE se.handle = ?
        LIMIT 1
        "#,
    )
    .bind(normalized.as_str())
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?;
    let Some(target_entity_id) = target_entity_id else {
        return Err(AppError::NotFound);
    };
    let rel_id = profile_actions::find_pending_human_invite_rel_id(
        &state.db,
        target_entity_id,
        viewer_human_id,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| {
        AppError::BadRequest(c.t(&loc, "ui.entity_private.connection_invite_unavailable"))
    })?;
    let action = body.response_action.trim().to_ascii_lowercase();
    let response = match action.as_str() {
        "accept" => profile_actions::ConnectionInviteResponse::Accept,
        "decline" => profile_actions::ConnectionInviteResponse::Decline,
        "block" => profile_actions::ConnectionInviteResponse::Block,
        _ => {
            return Err(AppError::BadRequest(
                c.t(&loc, "ui.entity_private.connection_type_invalid"),
            ));
        }
    };
    profile_actions::respond_to_connection_invite(
        &state.db,
        viewer_human_id,
        target_entity_id,
        rel_id,
        response,
        user_id,
    )
    .await?;
    if private_profile_wants_json(&headers) {
        Ok(Json(serde_json::json!({ "ok": true })).into_response())
    } else {
        Ok(Redirect::to(&format!("/home/profile/{normalized}")).into_response())
    }
}

fn private_profile_wants_json(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.contains("application/json"))
        .unwrap_or(false)
}

async fn private_profile_cancel_invite_submit(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(raw_handle): Path<String>,
    headers: HeaderMap,
) -> AppResult<Response> {
    profile_gate::require_profile_mutation(&state.db, &state.config, user_id, &headers).await?;
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let Some(normalized) = entity::normalize_handle_from_path_segment(&raw_handle) else {
        return Err(AppError::NotFound);
    };
    let Some(profile) = private_profile::fetch_by_handle(&state.db, user_id, &normalized)
        .await
        .map_err(AppError::Db)?
    else {
        return Err(AppError::NotFound);
    };
    if profile.type_label_key == ref_keys::KEY_ENTITY_TYPE_COMMUNITY {
        if profile.has_open_membership_review_application {
            let community_entity_id: u64 = sqlx::query_scalar(
                r#"
                SELECT se.id
                FROM social_entity se
                INNER JOIN social_community sc ON sc.social_entity_id = se.id
                WHERE se.handle = ?
                LIMIT 1
                "#,
            )
            .bind(normalized.as_str())
            .fetch_optional(&state.db)
            .await
            .map_err(AppError::Db)?
            .ok_or(AppError::NotFound)?;
            let Some(viewer_human_id) =
                human_onboarding::primary_human_entity_id(&state.db, user_id)
                    .await
                    .map_err(AppError::Db)?
            else {
                return Err(AppError::BadRequest(
                    c.t(&loc, "ui.entity_private.human_profile_required"),
                ));
            };
            judicial::membership_review::cancel_application(
                &state.db,
                user_id,
                viewer_human_id,
                community_entity_id,
            )
            .await?;
            let message = c.t(&loc, "ui.entity_private.membership_review_cancelled_notice");
            if private_profile_wants_json(&headers) {
                return Ok(Json(serde_json::json!({ "ok": true, "message": message })).into_response());
            }
            return Ok(Redirect::to(&format!("/home/profile/{normalized}")).into_response());
        }
        if !profile.invite_already_sent {
            return Err(AppError::BadRequest(
                c.t(&loc, "ui.entity_private.connection_invite_unavailable"),
            ));
        }
        let community_entity_id: u64 = sqlx::query_scalar(
            r#"
            SELECT se.id
            FROM social_entity se
            INNER JOIN social_community sc ON sc.social_entity_id = se.id
            WHERE se.handle = ?
            LIMIT 1
            "#,
        )
        .bind(normalized.as_str())
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Db)?
        .ok_or(AppError::NotFound)?;
        community_invite::cancel_pending_membership_request(
            &state.db,
            user_id,
            community_entity_id,
        )
        .await?;
    } else {
        let Some(viewer_human_id) = human_onboarding::primary_human_entity_id(&state.db, user_id)
            .await
            .map_err(AppError::Db)?
        else {
            return Err(AppError::BadRequest(
                c.t(&loc, "ui.entity_private.human_profile_required"),
            ));
        };
        let target_entity_id: Option<u64> = sqlx::query_scalar(
            r#"
            SELECT se.id
            FROM social_entity se
            INNER JOIN social_human sh ON sh.social_entity_id = se.id
            WHERE se.handle = ?
            LIMIT 1
            "#,
        )
        .bind(normalized.as_str())
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Db)?;
        let Some(target_entity_id) = target_entity_id else {
            return Err(AppError::NotFound);
        };
        let rel_id: Option<u64> = sqlx::query_scalar(
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
        .bind(viewer_human_id)
        .bind(target_entity_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Db)?;
        let Some(rel_id) = rel_id else {
            return Err(AppError::BadRequest(
                c.t(&loc, "ui.entity_private.connection_invite_unavailable"),
            ));
        };
        status::end_relationship(&state.db, rel_id, user_id).await?;
    }
    if private_profile_wants_json(&headers) {
        Ok(Json(serde_json::json!({ "ok": true })).into_response())
    } else {
        Ok(Redirect::to(&format!("/home/profile/{normalized}")).into_response())
    }
}

async fn private_profile_disconnect_submit(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(raw_handle): Path<String>,
    headers: HeaderMap,
) -> AppResult<Response> {
    profile_gate::require_profile_mutation(&state.db, &state.config, user_id, &headers).await?;
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let Some(normalized) = entity::normalize_handle_from_path_segment(&raw_handle) else {
        return Err(AppError::NotFound);
    };
    let Some(profile) = private_profile::fetch_by_handle(&state.db, user_id, &normalized)
        .await
        .map_err(AppError::Db)?
    else {
        return Err(AppError::NotFound);
    };
    if !profile.can_disconnect {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.connection_invite_unavailable"),
        ));
    }
    let Some(viewer_human_id) = human_onboarding::primary_human_entity_id(&state.db, user_id)
        .await
        .map_err(AppError::Db)?
    else {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.human_profile_required"),
        ));
    };
    let target_entity_id: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT se.id
        FROM social_entity se
        INNER JOIN social_human sh ON sh.social_entity_id = se.id
        WHERE se.handle = ?
        LIMIT 1
        "#,
    )
    .bind(normalized.as_str())
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?;
    let Some(target_entity_id) = target_entity_id else {
        return Err(AppError::NotFound);
    };
    let rel_id: Option<u64> = sqlx::query_scalar(
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
    .bind(target_entity_id)
    .bind(target_entity_id)
    .bind(viewer_human_id)
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?;
    let Some(rel_id) = rel_id else {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.connection_invite_unavailable"),
        ));
    };
    status::end_relationship(&state.db, rel_id, user_id).await?;
    if private_profile_wants_json(&headers) {
        Ok(Json(serde_json::json!({ "ok": true })).into_response())
    } else {
        Ok(Redirect::to(&format!("/home/profile/{normalized}")).into_response())
    }
}

async fn private_profile_block_submit(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(raw_handle): Path<String>,
    headers: HeaderMap,
) -> AppResult<Response> {
    profile_gate::require_profile_mutation(&state.db, &state.config, user_id, &headers).await?;
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let Some(normalized) = entity::normalize_handle_from_path_segment(&raw_handle) else {
        return Err(AppError::NotFound);
    };
    let Some(profile) = private_profile::fetch_by_handle(&state.db, user_id, &normalized)
        .await
        .map_err(AppError::Db)?
    else {
        return Err(AppError::NotFound);
    };
    if !profile.can_block_human {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.block_unavailable"),
        ));
    }
    let Some(viewer_human_id) = human_onboarding::primary_human_entity_id(&state.db, user_id)
        .await
        .map_err(AppError::Db)?
    else {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.human_profile_required"),
        ));
    };
    let target_entity_id: u64 = sqlx::query_scalar(
        r#"
        SELECT se.id
        FROM social_entity se
        INNER JOIN social_human sh ON sh.social_entity_id = se.id
        WHERE se.handle = ?
        LIMIT 1
        "#,
    )
    .bind(normalized.as_str())
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?
    .ok_or(AppError::NotFound)?;
    blocks::block_human(&state.db, viewer_human_id, target_entity_id, user_id)
        .await
        .map_err(AppError::Db)?;
    if private_profile_wants_json(&headers) {
        Ok(Json(serde_json::json!({
            "ok": true,
            "message": c.t(&loc, "ui.entity_private.block_success"),
        }))
        .into_response())
    } else {
        Ok(Redirect::to("/home").into_response())
    }
}

async fn private_profile_leave_community_submit(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(raw_handle): Path<String>,
    headers: HeaderMap,
) -> AppResult<Response> {
    profile_gate::require_profile_mutation(&state.db, &state.config, user_id, &headers).await?;
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let Some(normalized) = entity::normalize_handle_from_path_segment(&raw_handle) else {
        return Err(AppError::NotFound);
    };
    let Some(profile) = private_profile::fetch_by_handle(&state.db, user_id, &normalized)
        .await
        .map_err(AppError::Db)?
    else {
        return Err(AppError::NotFound);
    };
    if !profile.can_leave_community {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.leave_community_unavailable"),
        ));
    }
    let Some(viewer_human_id) = human_onboarding::primary_human_entity_id(&state.db, user_id)
        .await
        .map_err(AppError::Db)?
    else {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.human_profile_required"),
        ));
    };
    let target_entity_id: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT se.id
        FROM social_entity se
        INNER JOIN social_community sc ON sc.social_entity_id = se.id
        WHERE se.handle = ?
        LIMIT 1
        "#,
    )
    .bind(normalized.as_str())
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?;
    let Some(target_entity_id) = target_entity_id else {
        return Err(AppError::NotFound);
    };
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
    .bind(target_entity_id)
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?;
    let Some(rel_id) = rel_id else {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.leave_community_unavailable"),
        ));
    };
    status::end_relationship(&state.db, rel_id, user_id).await?;
    if private_profile_wants_json(&headers) {
        Ok(Json(serde_json::json!({ "ok": true })).into_response())
    } else {
        Ok(Redirect::to(&format!("/home/profile/{normalized}")).into_response())
    }
}

async fn apply_pending_invitation_response(
    pool: &sqlx::MySqlPool,
    user_id: u64,
    relationship_id: u64,
    action: &str,
) -> AppResult<()> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let Some(viewer_human_id) = human_onboarding::primary_human_entity_id(pool, user_id)
        .await
        .map_err(AppError::Db)?
    else {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.human_profile_required"),
        ));
    };
    let row: Option<(String, u64, u64)> = sqlx::query_as(
        r#"
        SELECT rk_type.`key`, ser.from_social_entity_id, ser.to_social_entity_id
        FROM social_entity_relationship ser
        INNER JOIN ref_key_values rk_type ON rk_type.id = ser.type_id
        INNER JOIN ref_key_values rk_status ON rk_status.id = ser.status_id
        WHERE ser.id = ?
          AND ser.ended_at IS NULL
          AND rk_status.`group` = ?
          AND rk_status.`key` = ?
          AND (
                (rk_type.`key` = ? AND ser.from_social_entity_id = ?)
                OR (
                    rk_type.`key` IN (?, ?, ?)
                    AND ser.to_social_entity_id = ?
                )
          )
        LIMIT 1
        "#,
    )
    .bind(relationship_id)
    .bind(ref_keys::GROUP_STATUS)
    .bind(ref_keys::KEY_PENDING_VERIFICATION)
    .bind(ref_keys::KEY_RELATIONSHIP_MEMBER_OF)
    .bind(viewer_human_id)
    .bind(ref_keys::KEY_RELATIONSHIP_FRIEND)
    .bind(ref_keys::KEY_RELATIONSHIP_FAMILY)
    .bind(ref_keys::KEY_RELATIONSHIP_COLLEAGUE)
    .bind(viewer_human_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Db)?;
    let Some((type_key, _from_id, _to_id)) = row else {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.connection_invite_unavailable"),
        ));
    };
    match action {
        "accept" => {
            if type_key != ref_keys::KEY_RELATIONSHIP_MEMBER_OF {
                let peer_slots =
                    connections::distinct_peer_count_for_human_entity(pool, viewer_human_id)
                        .await
                        .map_err(AppError::Db)?;
                if (peer_slots as usize) >= connections::MAX_CONNECTION_SLOTS {
                    return Err(AppError::BadRequest(
                        c.t(&loc, "ui.entity_private.connection_slots_full"),
                    ));
                }
            }
            status::transition_relationship(
                pool,
                relationship_id,
                ref_keys::KEY_VERIFIED,
                Some(user_id),
            )
            .await?;
        }
        "decline" => {
            status::end_relationship(pool, relationship_id, user_id).await?;
        }
        "block" if type_key != ref_keys::KEY_RELATIONSHIP_MEMBER_OF => {
            let inviter_entity_id = _from_id;
            blocks::block_human(pool, viewer_human_id, inviter_entity_id, user_id)
                .await
                .map_err(AppError::Db)?;
        }
        _ => {
            return Err(AppError::BadRequest(
                c.t(&loc, "ui.entity_private.connection_type_invalid"),
            ));
        }
    }
    Ok(())
}

async fn private_profile_community_invite_submit(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(raw_handle): Path<String>,
    headers: HeaderMap,
    Form(body): Form<CommunityInviteForm>,
) -> AppResult<Json<serde_json::Value>> {
    profile_gate::require_profile_mutation(&state.db, &state.config, user_id, &headers).await?;
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let Some(community_handle) = entity::normalize_handle_from_path_segment(&raw_handle) else {
        return Err(AppError::NotFound);
    };
    let Some(profile) = private_profile::fetch_by_handle(&state.db, user_id, &community_handle)
        .await
        .map_err(AppError::Db)?
    else {
        return Err(AppError::NotFound);
    };
    if !profile.can_invite_community_members {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.community_invite_unavailable"),
        ));
    }
    let invitee_raw = body.invitee_handle.trim().trim_start_matches('@');
    let Some(invitee_handle) = entity::normalize_handle_from_path_segment(invitee_raw) else {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.community_invite_handle_invalid"),
        ));
    };
    let community_entity_id: u64 = sqlx::query_scalar(
        r#"
        SELECT se.id
        FROM social_entity se
        INNER JOIN social_community sc ON sc.social_entity_id = se.id
        WHERE se.handle = ?
        LIMIT 1
        "#,
    )
    .bind(community_handle.as_str())
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?
    .ok_or(AppError::NotFound)?;
    let invitee_human_id: u64 = sqlx::query_scalar(
        r#"
        SELECT se.id
        FROM social_entity se
        INNER JOIN ref_key_values rk_type
            ON rk_type.id = se.type_id
           AND rk_type.`group` = ?
           AND rk_type.`key` = ?
        WHERE se.handle = ?
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(ref_keys::KEY_ENTITY_TYPE_HUMAN)
    .bind(invitee_handle.as_str())
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| {
        AppError::BadRequest(c.t(&loc, "ui.entity_private.community_invite_human_not_found"))
    })?;
    community_invite::send_membership_invite(
        &state.db,
        user_id,
        community_entity_id,
        invitee_human_id,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "message": c.t(&loc, "ui.entity_private.community_invite_sent")
    })))
}

async fn private_profile_community_invite_action_submit(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(raw_handle): Path<String>,
    headers: HeaderMap,
    Form(body): Form<PrivateConnectActionForm>,
) -> AppResult<Response> {
    profile_gate::require_profile_mutation(&state.db, &state.config, user_id, &headers).await?;
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let Some(normalized) = entity::normalize_handle_from_path_segment(&raw_handle) else {
        return Err(AppError::NotFound);
    };
    let Some(profile) = private_profile::fetch_by_handle(&state.db, user_id, &normalized)
        .await
        .map_err(AppError::Db)?
    else {
        return Err(AppError::NotFound);
    };
    if !profile.can_respond_to_community_invite {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.community_invite_unavailable"),
        ));
    }
    let Some(viewer_human_id) = human_onboarding::primary_human_entity_id(&state.db, user_id)
        .await
        .map_err(AppError::Db)?
    else {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.human_profile_required"),
        ));
    };
    let community_entity_id: u64 = sqlx::query_scalar(
        r#"
        SELECT se.id FROM social_entity se
        INNER JOIN social_community sc ON sc.social_entity_id = se.id
        WHERE se.handle = ?
        LIMIT 1
        "#,
    )
    .bind(normalized.as_str())
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?
    .ok_or(AppError::NotFound)?;
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
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?;
    let Some(rel_id) = rel_id else {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.community_invite_unavailable"),
        ));
    };
    let action = body.response_action.trim().to_ascii_lowercase();
    apply_pending_invitation_response(&state.db, user_id, rel_id, action.as_str()).await?;
    if private_profile_wants_json(&headers) {
        Ok(Json(serde_json::json!({ "ok": true })).into_response())
    } else {
        Ok(Redirect::to(&format!("/home/profile/{normalized}")).into_response())
    }
}

async fn pending_invitation_respond_submit(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(relationship_id): Path<u64>,
    Form(body): Form<PrivateConnectActionForm>,
) -> AppResult<Json<serde_json::Value>> {
    let action = body.response_action.trim().to_ascii_lowercase();
    apply_pending_invitation_response(&state.db, user_id, relationship_id, action.as_str()).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn private_profile_follow_submit(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(raw_handle): Path<String>,
    headers: HeaderMap,
) -> AppResult<Response> {
    profile_gate::require_profile_mutation(&state.db, &state.config, user_id, &headers).await?;
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let Some(normalized) = entity::normalize_handle_from_path_segment(&raw_handle) else {
        return Err(AppError::NotFound);
    };
    let Some(profile) = private_profile::fetch_by_handle(&state.db, user_id, &normalized)
        .await
        .map_err(AppError::Db)?
    else {
        return Err(AppError::NotFound);
    };
    if !profile.can_follow_brand {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.brand_follow_unavailable"),
        ));
    }
    let Some(viewer_human_id) = human_onboarding::primary_human_entity_id(&state.db, user_id)
        .await
        .map_err(AppError::Db)?
    else {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.human_profile_required"),
        ));
    };
    let target_entity_id: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT se.id
        FROM social_entity se
        INNER JOIN ref_key_values rk ON rk.id = se.type_id
          AND rk.`group` = ?
          AND rk.`key` IN (?, ?)
        WHERE se.handle = ?
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(ref_keys::KEY_ENTITY_TYPE_PROFIT)
    .bind(ref_keys::KEY_ENTITY_TYPE_NON_PROFIT)
    .bind(normalized.as_str())
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?;
    let Some(target_entity_id) = target_entity_id else {
        return Err(AppError::NotFound);
    };
    let peer_slots = connections::distinct_peer_count_for_human_entity(&state.db, viewer_human_id)
        .await
        .map_err(AppError::Db)?;
    if (peer_slots as usize) >= connections::MAX_CONNECTION_SLOTS {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.connection_slots_full"),
        ));
    }
    let follow_type_id = ref_key_values::lookup_active_id(
        &state.db,
        ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE,
        ref_keys::KEY_RELATIONSHIP_FOLLOW,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| AppError::BadRequest(c.t(&loc, "ui.entity_private.connection_type_unconfigured")))?;
    let verified_status_id = ref_key_values::lookup_active_id(
        &state.db,
        ref_keys::GROUP_STATUS,
        ref_keys::KEY_VERIFIED,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| AppError::BadRequest(c.t(&loc, "ui.entity_private.pending_status_unconfigured")))?;
    relationship::create(
        &state.db,
        relationship::CreateRequest {
            from_social_entity_id: viewer_human_id.to_string(),
            to_social_entity_id: target_entity_id.to_string(),
            type_id: follow_type_id.to_string(),
            status_id: Some(verified_status_id.to_string()),
            started_at: None,
            ended_at: None,
        },
        user_id,
    )
    .await?;
    if private_profile_wants_json(&headers) {
        Ok(Json(serde_json::json!({ "ok": true })).into_response())
    } else {
        Ok(Redirect::to(&format!("/home/profile/{normalized}")).into_response())
    }
}

async fn private_profile_unfollow_submit(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(raw_handle): Path<String>,
    headers: HeaderMap,
) -> AppResult<Response> {
    profile_gate::require_profile_mutation(&state.db, &state.config, user_id, &headers).await?;
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let Some(normalized) = entity::normalize_handle_from_path_segment(&raw_handle) else {
        return Err(AppError::NotFound);
    };
    let Some(profile) = private_profile::fetch_by_handle(&state.db, user_id, &normalized)
        .await
        .map_err(AppError::Db)?
    else {
        return Err(AppError::NotFound);
    };
    if !profile.is_following_brand {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.brand_follow_unavailable"),
        ));
    }
    let Some(viewer_human_id) = human_onboarding::primary_human_entity_id(&state.db, user_id)
        .await
        .map_err(AppError::Db)?
    else {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.human_profile_required"),
        ));
    };
    let target_entity_id: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT se.id
        FROM social_entity se
        INNER JOIN ref_key_values rk ON rk.id = se.type_id
          AND rk.`group` = ?
          AND rk.`key` IN (?, ?)
        WHERE se.handle = ?
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(ref_keys::KEY_ENTITY_TYPE_PROFIT)
    .bind(ref_keys::KEY_ENTITY_TYPE_NON_PROFIT)
    .bind(normalized.as_str())
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?;
    let Some(target_entity_id) = target_entity_id else {
        return Err(AppError::NotFound);
    };
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
    .bind(target_entity_id)
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?;
    let Some(rel_id) = rel_id else {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.brand_follow_unavailable"),
        ));
    };
    status::end_relationship(&state.db, rel_id, user_id).await?;
    if private_profile_wants_json(&headers) {
        Ok(Json(serde_json::json!({ "ok": true })).into_response())
    } else {
        Ok(Redirect::to(&format!("/home/profile/{normalized}")).into_response())
    }
}

fn gathering_rsvp_form_to_ref_key(raw: &str) -> Option<&'static str> {
    let s = raw.trim().to_ascii_lowercase();
    match s.as_str() {
        "going" => Some(ref_keys::KEY_RELATIONSHIP_EVENT_GOING),
        "not_going" => Some(ref_keys::KEY_RELATIONSHIP_EVENT_NOT_GOING),
        "interested" => Some(ref_keys::KEY_RELATIONSHIP_EVENT_INTERESTED),
        _ => None,
    }
}

async fn private_profile_gathering_rsvp_submit(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(raw_handle): Path<String>,
    headers: HeaderMap,
    Form(body): Form<GatheringRsvpForm>,
) -> AppResult<Response> {
    profile_gate::require_profile_mutation(&state.db, &state.config, user_id, &headers).await?;
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let Some(normalized) = entity::normalize_handle_from_path_segment(&raw_handle) else {
        return Err(AppError::NotFound);
    };
    let Some(profile) = private_profile::fetch_by_handle(&state.db, user_id, &normalized)
        .await
        .map_err(AppError::Db)?
    else {
        return Err(AppError::NotFound);
    };
    if !profile.can_use_gathering_rsvp {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.gathering_rsvp_unavailable"),
        ));
    }
    let Some(rel_type_key) = gathering_rsvp_form_to_ref_key(&body.gathering_rsvp) else {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.connection_type_invalid"),
        ));
    };
    let Some(viewer_human_id) = human_onboarding::primary_human_entity_id(&state.db, user_id)
        .await
        .map_err(AppError::Db)?
    else {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.human_profile_required"),
        ));
    };
    let target_entity_id: Option<u64> = sqlx::query_scalar(
        r#"
        SELECT se.id
        FROM social_entity se
        INNER JOIN ref_key_values rk ON rk.id = se.type_id
          AND rk.`group` = ?
          AND rk.`key` = ?
        WHERE se.handle = ?
        LIMIT 1
        "#,
    )
    .bind(ref_keys::GROUP_SOCIAL_ENTITY_TYPE)
    .bind(ref_keys::KEY_ENTITY_TYPE_GATHERING)
    .bind(normalized.as_str())
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?;
    let Some(target_entity_id) = target_entity_id else {
        return Err(AppError::NotFound);
    };
    let new_type_id = ref_key_values::lookup_active_id(
        &state.db,
        ref_keys::GROUP_SOCIAL_ENTITY_RELATIONSHIP_TYPE,
        rel_type_key,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| AppError::BadRequest(c.t(&loc, "ui.entity_private.connection_type_unconfigured")))?;
    let verified_status_id = ref_key_values::lookup_active_id(
        &state.db,
        ref_keys::GROUP_STATUS,
        ref_keys::KEY_VERIFIED,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| AppError::BadRequest(c.t(&loc, "ui.entity_private.pending_status_unconfigured")))?;
    let existing_rsvp: Option<u64> = sqlx::query_scalar(
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
    .bind(target_entity_id)
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?;
    if let Some(rid) = existing_rsvp {
        sqlx::query(
            r#"UPDATE social_entity_relationship
               SET type_id = ?, modified_by = ?
               WHERE id = ?"#,
        )
        .bind(new_type_id)
        .bind(user_id)
        .bind(rid)
        .execute(&state.db)
        .await
        .map_err(AppError::Db)?;
        scope::mark_relationship_endpoints_dirty(&state.db, viewer_human_id, target_entity_id)
            .await
            .map_err(AppError::Db)?;
    } else {
        let peer_slots = connections::distinct_peer_count_for_human_entity(&state.db, viewer_human_id)
            .await
            .map_err(AppError::Db)?;
        if (peer_slots as usize) >= connections::MAX_CONNECTION_SLOTS {
            return Err(AppError::BadRequest(
                c.t(&loc, "ui.entity_private.connection_slots_full"),
            ));
        }
        relationship::create(
            &state.db,
            relationship::CreateRequest {
                from_social_entity_id: viewer_human_id.to_string(),
                to_social_entity_id: target_entity_id.to_string(),
                type_id: new_type_id.to_string(),
                status_id: Some(verified_status_id.to_string()),
                started_at: None,
                ended_at: None,
            },
            user_id,
        )
        .await?;
    }
    if private_profile_wants_json(&headers) {
        Ok(Json(serde_json::json!({ "ok": true })).into_response())
    } else {
        Ok(Redirect::to(&format!("/home/profile/{normalized}")).into_response())
    }
}

async fn face_capture_page(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    axum::extract::Query(_q): axum::extract::Query<OnboardingQuery>,
) -> AppResult<Response> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let face_path = user_faces::artwork_display_src_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let has_full_access = status::user_has_verified_status(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    if post_auth::needs_terms_consent(&state.db, user_id)
        .await
        .map_err(AppError::Db)?
    {
        return Ok(Redirect::temporary("/home?onboarding=consent").into_response());
    }
    let has_human_profile = human_onboarding::user_has_human_profile(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let has_face_capture: bool = user_faces::user_passes_face_capture_gate(
        &state.db,
        user_id,
        state.config.face_vision_mock,
    )
    .await
    .map_err(AppError::Db)?;

    if has_full_access && has_face_capture {
        return Ok(Redirect::temporary("/home").into_response());
    }
    if has_face_capture && !has_human_profile {
        return Ok(Redirect::temporary("/home?onboarding=profile").into_response());
    }
    let show_godmother_nomination = has_face_capture && has_human_profile && !has_full_access;
    let show_face_capture = !has_face_capture;
    let godmother_only = show_godmother_nomination && !show_face_capture;
    let connections = connections::list_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let self_ssr = human_onboarding::ssr_rating_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let face_topbar_suffix = if godmother_only {
        c.t(&loc, "ui.face_capture.godmother_topbar_title")
    } else {
        c.t(&loc, "ui.face_capture.topbar_title")
    };
    let title_html = topbar_title_with_suffix_html(connections.len(), &face_topbar_suffix);
    let topbar_html = secured_shell_topbar_html(
        &state,
        user_id,
        &loc,
        &title_html,
        false,
        &snippets::topbar_avatar_html(
            face_path.as_deref(),
            false,
            &c.t(&loc, "ui.home.logo_aria_home"),
            self_ssr,
            None,
        ),
        true,
    )
    .await?;
    // Onboarding is only for users without full verification; do not show the secured-zone connection rail.
    let rail_html = String::new();
    let use_mock_vision = state.config.face_vision_mock;
    let mock_banner_attr = if use_mock_vision { "" } else { " hidden" };
    let doc_title_key = if godmother_only {
        "ui.face_capture.godmother_page_title"
    } else {
        "ui.face_capture.page_title"
    };
    let doc_title = escape_html(&c.t(&loc, doc_title_key));
    let artist_handle = state.config.council_agent_handle.as_str();
    let at_handle = format!(
        "@{}",
        artist_handle.trim().trim_start_matches('@')
    );
    let face_capture_lead = escape_html(&c.tf(
        &loc,
        "ui.face_capture.lead",
        &[("artist_handle", &at_handle)],
    ));
    let artist_embed = if show_face_capture {
        snippets::face_capture_artist_embed_html(&loc, artist_handle, "")
    } else {
        String::new()
    };
    let face_capture_vendor_scripts = if godmother_only {
        ""
    } else {
        r#"<script src="/static/vendor/face-api/face-api.min.js" defer></script>"#
    };
    let html = FACE_CAPTURE_TEMPLATE
        .replace("__HTML_LANG__", &loc)
        .replace("__FACE_CAPTURE_DOCUMENT_TITLE__", &doc_title)
        .replace(
            "__FACE_CAPTURE_BODY_CLASS__",
            if godmother_only {
                "app-shell-body--viewport-lock app-shell-body--onboarding-godmother"
            } else {
                "app-shell-body--viewport-lock app-shell-body--face-capture"
            },
        )
        .replace(
            "__FACE_CAPTURE_MAIN_CLASS__",
            if godmother_only {
                "onboarding-godmother-main"
            } else {
                "face-capture-main"
            },
        )
        .replace(
            "__ONBOARDING_STEP__",
            if godmother_only {
                "godmother"
            } else {
                "face_capture"
            },
        )
        .replace(
            "__FACE_CAPTURE_VENDOR_SCRIPTS__",
            face_capture_vendor_scripts,
        )
        .replace(
            "__FACE_CAPTURE_READY__",
            if has_face_capture { "1" } else { "0" },
        )
        .replace(
            "__PROFILE_READY__",
            if has_human_profile { "1" } else { "0" },
        )
        .replace(
            "__FACE_CAPTURE_CARD_HIDDEN_ATTR__",
            if show_face_capture { "" } else { "hidden" },
        )
        .replace(
            "__GODMOTHER_CARD_HIDDEN_ATTR__",
            if show_godmother_nomination {
                ""
            } else {
                "hidden"
            },
        )
        .replace(
            "__FACE_CAPTURE_APP_STAGE_CLASS__",
            "app-stage app-stage--face-capture",
        )
        .replace("__FACE_CAPTURE_RAIL_HTML__", &rail_html)
        .replace("__SECURED_TOPBAR__", &topbar_html)
        .replace("__FACE_CAPTURE_MOCK_BANNER_ATTR__", mock_banner_attr)
        .replace(
            "__FACE_CAPTURE_SUBMIT_LOADER__",
            &snippets::card_loader("face-capture-submit"),
        )
        .replace(
            "__FACE_CAPTURE_I18N_JSON__",
            &escape_json_for_html_script(&face_capture_i18n_json(
                &loc,
                state.config.council_agent_handle.as_str(),
            )),
        )
        .replace("__FACE_CAPTURE_LEAD__", &face_capture_lead)
        .replace("__FACE_CAPTURE_ARTIST_EMBED__", &artist_embed);
    Ok(Html(i18n::subst_markers(
        &snippets::inject_secured_public_footer_localized(&html, &c, &loc),
        &loc,
    ))
    .into_response())
}

async fn terms_consent_card_fragment() -> Html<String> {
    let loc = i18n::request_locale();
    Html(i18n::subst_markers(TERMS_CONSENT_FRAGMENT, &loc))
}

async fn profile_completion_card_fragment() -> Html<String> {
    let loc = i18n::request_locale();
    let max_birth = human_onboarding::latest_birth_date_for_min_age(
        human_onboarding::MIN_AGE_YEARS,
        Utc::now().date_naive(),
    )
    .format("%Y-%m-%d")
    .to_string();
    let raw = PROFILE_COMPLETION_FRAGMENT
        .replace(
            "__PROFILE_COUNTRY_OPTION_ROWS__",
            &crate::ui::countries::option_rows_html(),
        )
        .replace("__PROFILE_BIRTH_DATE_MAX__", &max_birth);
    Html(i18n::subst_markers(&raw, &loc))
}

#[derive(Debug, Deserialize)]
struct FaceCaptureCardQuery {
    mode: Option<String>,
}

async fn face_capture_card_fragment(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Query(q): Query<FaceCaptureCardQuery>,
) -> AppResult<Response> {
    let mode = q.mode.as_deref().unwrap_or("").trim().to_ascii_lowercase();
    if mode != "avatar_redraw" {
        return Err(AppError::NotFound);
    }
    let has_full_access = status::user_has_verified_status(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    if !has_full_access {
        return Err(AppError::Unauthorized);
    }
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let avatar_cost = BillableService::CouncilAvatarRefresh.nom_cost();
    let use_mock_vision = state.config.face_vision_mock;
    let mock_banner_attr = if use_mock_vision { "" } else { " hidden" };
    let raw = FACE_CAPTURE_CARD_FRAGMENT
        .replace(
            "__AVATAR_REDRAW_LEAD__",
            &nom_icon::locale_with_nom_price(
                &c,
                loc.as_str(),
                "ui.face_capture.avatar_redraw_lead",
                avatar_cost,
            ),
        )
        .replace(
            "__AVATAR_REDRAW_SUBMIT__",
            &nom_icon::locale_with_nom_price_paren(
                &c,
                loc.as_str(),
                "ui.face_capture.avatar_redraw_submit",
                avatar_cost,
            ),
        )
        .replace(
            "__FACE_CAPTURE_MOCK_BANNER_ATTR__",
            mock_banner_attr,
        )
        .replace(
            "__FACE_CAPTURE_SUBMIT_LOADER__",
            &snippets::card_loader("face-capture-submit"),
        )
        .replace(
            "__FACE_CAPTURE_I18N_JSON__",
            &escape_json_for_html_script(&face_capture_i18n_json(
                &loc,
                state.config.council_agent_handle.as_str(),
            )),
        );
    Ok(Html(i18n::subst_markers(&raw, &loc)).into_response())
}

async fn home_page(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    headers: HeaderMap,
    axum::extract::Query(q): axum::extract::Query<HomeQuery>,
) -> AppResult<Response> {
    let needs_terms = post_auth::needs_terms_consent(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let has_human_profile = human_onboarding::user_has_human_profile(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let onboarding_mode = q.onboarding.unwrap_or_default();
    let needs_profile = !has_human_profile;
    let has_full_access = status::user_has_verified_status(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let has_face_capture =
        user_faces::user_passes_face_capture_gate(&state.db, user_id, state.config.face_vision_mock)
            .await
            .map_err(AppError::Db)?;

    if needs_terms {
        if onboarding_mode.as_str() != "consent" {
            return Ok(Redirect::temporary("/home?onboarding=consent").into_response());
        }
    } else if onboarding_mode.as_str() == "consent" {
        return Ok(Redirect::temporary("/home").into_response());
    } else if let Some(redir) =
        post_auth::missing_face_capture_redirect(&state.db, user_id, state.config.face_vision_mock)
            .await?
    {
        return Ok(redir);
    } else if !has_full_access && onboarding_mode.trim().is_empty() {
        return Ok(Redirect::temporary("/onboarding").into_response());
    } else if onboarding_mode.as_str() == "profile" && !needs_profile {
        return Ok(Redirect::temporary("/home").into_response());
    }

    // Connection rail only after full verification, profile, and a completed face capture row.
    let show_connections_rail = has_full_access && !needs_profile && has_face_capture;

    let connections = if show_connections_rail {
        connections::list_for_user(&state.db, user_id)
            .await
            .map_err(AppError::Db)?
    } else {
        Vec::new()
    };

    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let face_path = user_faces::artwork_display_src_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;

    let tagline = c.t(&loc, "ui.site.tagline");
    let meter = connection_slot_meter_display(connections.len());
    let doc_title = c.tf(
        &loc,
        "ui.site.document_title_home",
        &[("meter", meter.as_str()), ("tagline", &tagline)],
    );
    let title_esc = escape_html(&doc_title);
    let self_human_handle = human_onboarding::human_entity_handle_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let viewer_handle_at = self_human_handle
        .as_ref()
        .map(|h| format!("@{h}"))
        .unwrap_or_else(|| tagline.clone());
    let self_ssr = human_onboarding::ssr_rating_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let title_html = topbar_title_home_feed_html(
        connections.len(),
        &tagline,
        &viewer_handle_at,
    );
    let topbar_html = secured_shell_topbar_html(
        &state,
        user_id,
        &loc,
        &title_html,
        show_connections_rail,
        &snippets::topbar_avatar_html(
            face_path.as_deref(),
            true,
            &c.t(&loc, "ui.home.logo_aria_home"),
            self_ssr,
            None,
        ),
        false,
    )
    .await?;

    let rail_html = if show_connections_rail {
        rail::connections_nav_html(&connections, &loc, c)
    } else {
        String::new()
    };

    let needs_flag = if needs_profile { "1" } else { "0" };
    let needs_consent_flag = if needs_terms { "1" } else { "0" };
    let feed_panel = if has_full_access && has_face_capture {
        HOME_FEED_PANEL.to_string()
    } else if needs_terms || needs_profile {
        String::new()
    } else {
        HOME_ONBOARDING_LOCKED_PANEL.to_string()
    };
    let csrf_meta = csrf_meta_html(&headers, &state.config);
    let html = HOME_TEMPLATE
        .replace("__HTML_LANG__", &loc)
        .replace("__CSRF_META__", &csrf_meta)
        .replace("__HOME_CONNECTION_COUNT__", &connections.len().to_string())
        .replace("__HOME_DOCUMENT_TITLE__", &title_esc)
        .replace("__SECURED_TOPBAR__", &topbar_html)
        .replace(
            "__CARD_LOADER_PROFILE__",
            &snippets::card_loader("home-profile"),
        )
        .replace("__HOME_NEEDS_PROFILE__", needs_flag)
        .replace("__HOME_NEEDS_CONSENT__", needs_consent_flag)
        .replace(
            "__HOME_FACE_CAPTURE_READY__",
            if has_face_capture { "1" } else { "0" },
        )
        .replace(
            "__HOME_SELF_PRIVATE_HANDLE__",
            if has_full_access && !needs_profile {
                self_human_handle.as_deref().unwrap_or("")
            } else {
                ""
            },
        )
        .replace(
            "__HOME_APP_STAGE_CLASS__",
            if show_connections_rail {
                "app-stage"
            } else {
                "app-stage app-stage--face-capture"
            },
        )
        .replace("__HOME_RAIL_HTML__", &rail_html)
        .replace("__HOME_FEED_PANEL__", &feed_panel)
        .replace("__HOME_FEED_DEFAULT__", HOME_FEED_DEFAULT)
        .replace(
            "__HOME_SHELL_I18N_JSON__",
            &escape_json_for_html_script(&home_shell_i18n_json(
                &loc,
                connections.len(),
                self_human_handle.as_deref(),
            )),
        );

    Ok(Html(i18n::subst_markers(
        &snippets::inject_secured_public_footer_localized(&html, &c, &loc),
        &loc,
    ))
    .into_response())
}

async fn private_profile_page(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    headers: HeaderMap,
    Path(raw_handle): Path<String>,
) -> AppResult<Response> {
    let Some(normalized) = entity::normalize_handle_from_path_segment(&raw_handle) else {
        return Err(AppError::NotFound);
    };
    let has_full_access = status::user_has_verified_status(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    if !has_full_access {
        let path = format!("/public/{normalized}");
        return Ok(Redirect::temporary(&path).into_response());
    }

    let Some(profile) = private_profile::fetch_by_handle(&state.db, user_id, &normalized)
        .await
        .map_err(AppError::Db)?
    else {
        return Err(AppError::NotFound);
    };

    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let connections = connections::list_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let rail_html = rail::connections_nav_html(&connections, &loc, c);
    let face_path = user_faces::artwork_display_src_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let viewer_human_handle = human_onboarding::human_entity_handle_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let self_private_href = viewer_human_handle
        .as_ref()
        .map(|h| format!("/home/profile/{h}"));
    let self_ssr = human_onboarding::ssr_rating_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let at = format!("@{}", profile.handle);
    let title_html = topbar_title_with_suffix_html(connections.len(), &at);
    let topbar_html = secured_shell_topbar_html(
        &state,
        user_id,
        &loc,
        &title_html,
        true,
        &snippets::topbar_avatar_html(
            face_path.as_deref(),
            true,
            &c.t(&loc, "ui.home.logo_aria_home"),
            self_ssr,
            None,
        ),
        false,
    )
    .await?;

    let doc_title = c.tf(
        &loc,
        "ui.entity_private.html_title",
        &[("handle", at.as_str())],
    );
    let card_html = private_profile_card::entity_card_html(&profile, &loc, c, true);
    let title_esc = escape_html(&doc_title);

    let html = PROFILE_PRIVATE_TEMPLATE
        .replace("__HTML_LANG__", &loc)
        .replace("__CSRF_META__", &csrf_meta_html(&headers, &state.config))
        .replace("__HOME_CONNECTION_COUNT__", &connections.len().to_string())
        .replace("__PRIVATE_DOCUMENT_TITLE__", &title_esc)
        .replace("__SECURED_TOPBAR__", &topbar_html)
        .replace(
            "__HOME_SHELL_I18N_JSON__",
            &escape_json_for_html_script(&home_shell_i18n_json(
                &loc,
                connections.len(),
                viewer_human_handle.as_deref(),
            )),
        )
        .replace(
            "__HOME_SELF_PRIVATE_HANDLE__",
            self_private_href
                .as_deref()
                .and_then(|h| h.rsplit('/').next())
                .unwrap_or(""),
        )
        .replace("__HOME_APP_STAGE_CLASS__", "app-stage")
        .replace("__HOME_RAIL_HTML__", &rail_html)
        .replace("__HOME_FEED_DEFAULT__", HOME_FEED_DEFAULT)
        .replace("__PRIVATE_PROFILE_CARD__", &card_html)
        .replace(
            "__CARD_LOADER_PROFILE__",
            &snippets::card_loader("home-profile"),
        )
        .replace(
            "__CARD_LOADER_CREATE__",
            &snippets::card_loader("home-create"),
        );

    Ok(Html(i18n::subst_markers(
        &snippets::inject_secured_public_footer_localized(&html, &c, &loc),
        &loc,
    ))
    .into_response())
}

const PROFILE_PRIVATE_TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/profile_private.html"
));

const FACE_CAPTURE_TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/face_capture.html"
));
const FACE_CAPTURE_CARD_FRAGMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/face_capture_card_fragment.html"
));
const JUDICIAL_POST_CASE_TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/judicial_post_case.html"
));
const JUDICIAL_POST_APPEAL_TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/judicial_post_appeal.html"
));
const JUDICIAL_POST_EMERGENCY_CONTEST_TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/judicial_post_emergency_contest.html"
));
const JUDICIAL_COMMUNITY_AMENDMENT_TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/judicial_community_amendment.html"
));
const JUDICIAL_COMMUNITY_PROFILE_AMENDMENT_TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/judicial_community_profile_amendment.html"
));
const JUDICIAL_COMMUNITY_REPARENT_TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/judicial_community_reparent.html"
));

const HOME_TEMPLATE: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/home.html"));
const PROFILE_COMPLETION_FRAGMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/profile_completion_card_fragment.html"
));
const TERMS_CONSENT_FRAGMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/terms_consent_card_fragment.html"
));
const CREATE_POST_FRAGMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/create_post_card_fragment.html"
));
const SEARCH_ADD_FRAGMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/search_add_card_fragment.html"
));
const CREATE_ENTITY_FRAGMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/create_entity_card_fragment.html"
));
const CREATE_ENTITY_PROFIT_FRAGMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/create_entity_card_profit_fragment.html"
));
const CREATE_ENTITY_NON_PROFIT_FRAGMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/create_entity_card_non_profit_fragment.html"
));
const CREATE_ENTITY_COMMUNITY_FRAGMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/create_entity_card_community_fragment.html"
));
#[derive(Debug, Deserialize)]
struct CommunityReparentForm {
    proposed_parent_handle: String,
    rationale: Option<String>,
    return_to: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct CommunityReparentPageQuery {
    proposed_parent_handle: Option<String>,
}

async fn render_community_reparent_page(
    state: &AppState,
    user_id: u64,
    community: &CommunityAmendmentTargetRow,
    proposed_parent_handle: &str,
    diff_html: &str,
    err: Option<&str>,
    return_to: &str,
) -> AppResult<Response> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let connections = connections::list_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let rail_html = rail::connections_nav_html(&connections, &loc, c);
    let face_path = user_faces::artwork_display_src_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let noms_balance = human_onboarding::ai_tokens_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let self_ssr = human_onboarding::ssr_rating_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?;
    let title_html = topbar_title_with_suffix_html(
        connections.len(),
        &c.t(&loc, "ui.judicial_community_reparent.heading"),
    );
    let topbar_html = secured_shell_topbar_html(
        &state,
        user_id,
        &loc,
        &title_html,
        true,
        &snippets::topbar_avatar_html(
            face_path.as_deref(),
            true,
            &c.t(&loc, "ui.home.logo_aria_home"),
            self_ssr,
            None,
        ),
        false,
    )
    .await?;
    let cost = crate::pricing::BillableService::JudicialFiling(
        crate::pricing::JudicialTrigger::CommunityReparent,
    )
    .nom_cost();
    let toolbar = rich_text_editor_toolbar_html();
    let err_html = err
        .map(|m| nom_icon::flash_alert_html(m))
        .unwrap_or_default();
    let context_card = format!(
        r#"<section class="card app-card profile-card profile-card--community"><div class="card-accent" role="presentation"></div><p class="public-post-handle-line mono">@{handle}</p><p class="public-post-display-name">{label}</p><p class="sub">{intro}</p></section>"#,
        handle = escape_html(&community.handle),
        label = escape_html(&community.display_label),
        intro = escape_html(&c.t(&loc, "ui.judicial_community_reparent.community_intro")),
    );
    let preview_action = format!("/home/community/{}/reparent", community.handle);
    let form_card = format!(
        r#"<section class="card app-card judicial-post-case-card"><div class="card-accent" role="presentation"></div><h2>{form_heading}</h2><p class="sub">{cost}</p><p class="sub">{hint}</p>{err}<form method="get" action="{preview_action}" class="judicial-reparent-preview-form"><label class="field-label" for="judicial-reparent-parent-input">{parent_label}</label><input id="judicial-reparent-parent-input" class="field-input" name="proposed_parent_handle" value="{parent_prefill}" required /><button type="submit" class="auth-text-btn">{preview_btn}</button></form>{diff_html}<form id="judicial-community-reparent-form" method="post" action="/home/community/{handle}/reparent" novalidate><input type="hidden" name="proposed_parent_handle" value="{parent_prefill}"><input type="hidden" name="return_to" value="{return_to}"><label class="field-label" for="judicial-reparent-rationale-editor">{reason_label}</label><div class="create-post-editor-toolbar" data-toolbar="rationale">{toolbar}</div><div id="judicial-reparent-rationale-editor" class="create-post-editor" contenteditable="true" data-placeholder="{reason_placeholder}" aria-label="{reason_label}"></div><input id="judicial-reparent-rationale-input" type="hidden" name="rationale" value=""><p id="judicial-reparent-rationale-error" class="sub" role="alert" hidden>{reason_required}</p><button type="submit" class="auth-text-btn btn-cta">{submit} ({cost_noms} Noms)</button></form></section>"#,
        form_heading = escape_html(&c.t(&loc, "ui.judicial_community_reparent.form_heading")),
        cost = nom_icon::locale_with_nom_price(
            &c,
            &loc,
            "ui.judicial_community_reparent.cost",
            BillableService::JudicialFiling(crate::pricing::JudicialTrigger::CommunityReparent)
                .nom_cost(),
        ),
        hint = escape_html(&c.t(&loc, "ui.judicial_community_reparent.hint")),
        err = err_html,
        preview_action = preview_action,
        parent_label = escape_html(&c.t(&loc, "ui.judicial_community_reparent.proposed_parent_label")),
        parent_prefill = escape_html(proposed_parent_handle.trim()),
        preview_btn = escape_html(&c.t(&loc, "ui.judicial_community_reparent.preview_btn")),
        diff_html = diff_html,
        handle = escape_html(&community.handle),
        return_to = escape_html(return_to),
        reason_label = escape_html(&c.t(&loc, "ui.judicial_community_reparent.reason_label")),
        reason_placeholder = escape_html(&c.t(&loc, "ui.judicial_community_reparent.reason_placeholder")),
        reason_required = escape_html(&c.t(&loc, "judicial_case_filing.reason_required")),
        submit = escape_html(&c.t(&loc, "ui.judicial_community_reparent.submit")),
        cost_noms = cost,
        toolbar = toolbar,
    );
    let self_private_handle = human_onboarding::human_entity_handle_for_user(&state.db, user_id)
        .await
        .map_err(AppError::Db)?
        .unwrap_or_default();
    let html = JUDICIAL_COMMUNITY_REPARENT_TEMPLATE
        .replace("__HTML_LANG__", &loc)
        .replace(
            "__JUD_COMMUNITY_REPARENT_TITLE__",
            &escape_html(&c.t(&loc, "ui.judicial_community_reparent.title")),
        )
        .replace("__HOME_CONNECTION_COUNT__", &connections.len().to_string())
        .replace("__SECURED_TOPBAR__", &topbar_html)
        .replace("__HOME_APP_STAGE_CLASS__", "app-stage")
        .replace("__HOME_RAIL_HTML__", &rail_html)
        .replace("__HOME_SELF_PRIVATE_HANDLE__", &self_private_handle)
        .replace("__JUD_COMMUNITY_CONTEXT_CARD__", &context_card)
        .replace("__JUD_COMMUNITY_REPARENT_FORM_CARD__", &form_card)
        .replace(
            "__HOME_SHELL_I18N_JSON__",
            &escape_json_for_html_script(&home_shell_i18n_json(
                &loc,
                connections.len(),
                Some(self_private_handle.as_str()).filter(|s| !s.is_empty()),
            )),
        );
    Ok(Html(i18n::subst_markers(
        &snippets::inject_secured_public_footer_localized(&html, &c, &loc),
        &loc,
    ))
    .into_response())
}

async fn home_community_reparent_page(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(handle): Path<String>,
    Query(q): Query<CommunityReparentPageQuery>,
) -> AppResult<Response> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let Some(normalized) = entity::normalize_handle_from_path_segment(&handle) else {
        return Err(AppError::NotFound);
    };
    let Some(community) =
        community_amendment_target_for_member(&state.db, user_id, &normalized).await?
    else {
        return Err(AppError::NotFound);
    };
    if community_graph::is_community_failing(&state.db, community.entity_id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.community_failing_notice"),
        ));
    }
    let mut diff_html = String::new();
    if let Some(parent_raw) = q.proposed_parent_handle.as_deref() {
        if let Some(parent_handle) = entity::normalize_handle_from_path_segment(parent_raw) {
            if let Ok(Some(parent_id)) = sqlx::query_scalar::<_, u64>(
                r#"
                SELECT sc.social_entity_id FROM social_entity se
                INNER JOIN social_community sc ON sc.social_entity_id = se.id
                WHERE se.handle = ? LIMIT 1
                "#,
            )
            .bind(&parent_handle)
            .fetch_optional(&state.db)
            .await
            {
                if parent_id != community.entity_id
                    && !community_graph::would_create_cycle(&state.db, community.entity_id, parent_id)
                        .await
                        .unwrap_or(true)
                {
                    let before =
                        community_constitution::fetch_effective_constitution(&state.db, community.entity_id)
                            .await
                            .unwrap_or_default();
                    let after = community_constitution::fetch_effective_constitution_after_reparent(
                        &state.db,
                        community.entity_id,
                        parent_id,
                    )
                    .await
                    .unwrap_or_default();
                    diff_html = constitution_structure_diff_html(
                        &community_constitution::diff_effective_constitutions(&before, &after),
                        c,
                        &loc,
                    );
                }
            }
        }
    }
    let parent_prefill = q.proposed_parent_handle.as_deref().unwrap_or("").trim();
    let return_to = format!("/home/profile/{}", community.handle);
    return render_community_reparent_page(
        &state,
        user_id,
        &community,
        parent_prefill,
        &diff_html,
        None,
        &return_to,
    )
    .await;

}

async fn home_community_reparent_submit(
    State(state): State<AppState>,
    Extension(VerifiedSession { user_id: user_id, password_reset_only: _ }): Extension<VerifiedSession>,
    Path(handle): Path<String>,
    Form(body): Form<CommunityReparentForm>,
) -> AppResult<Response> {
    let loc = i18n::request_locale();
    let c = i18n::catalog();
    let Some(normalized) = entity::normalize_handle_from_path_segment(&handle) else {
        return Err(AppError::NotFound);
    };
    let Some(community) =
        community_amendment_target_for_member(&state.db, user_id, &normalized).await?
    else {
        return Err(AppError::NotFound);
    };
    let Some(primary_human) = human_onboarding::primary_human_entity_id(&state.db, user_id)
        .await
        .map_err(AppError::Db)?
    else {
        return Err(AppError::BadRequest(c.t(&loc, "judicial_case_filing.no_primary_profile")));
    };
    if community_graph::is_community_failing(&state.db, community.entity_id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::BadRequest(
            c.t(&loc, "ui.entity_private.community_failing_notice"),
        ));
    }
    let proposed_handle = entity::normalize_handle_from_path_segment(&body.proposed_parent_handle)
        .ok_or_else(|| AppError::BadRequest(c.t(&loc, "ui.judicial_community_reparent.parent_invalid")))?;
    if crate::social::panoply::is_invalid_community_parent_handle(&proposed_handle) {
        return Err(AppError::BadRequest(
            c.t(&loc, "social_entity.community_parent_panoply_forbidden"),
        ));
    }
    let proposed_parent_id: u64 = sqlx::query_scalar(
        r#"
        SELECT sc.social_entity_id
        FROM social_entity se
        INNER JOIN social_community sc ON sc.social_entity_id = se.id
        WHERE se.handle = ?
        LIMIT 1
        "#,
    )
    .bind(&proposed_handle)
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| AppError::BadRequest(c.t(&loc, "ui.judicial_community_reparent.parent_invalid")))?;

    if proposed_parent_id == community.entity_id {
        return Err(AppError::BadRequest(c.t(&loc, "ui.judicial_community_reparent.parent_invalid")));
    }
    if community_graph::would_create_cycle(&state.db, community.entity_id, proposed_parent_id)
        .await
        .map_err(AppError::Db)?
    {
        return Err(AppError::BadRequest(c.t(&loc, "ui.judicial_community_reparent.cycle")));
    }
    let current_parent: Option<u64> = sqlx::query_scalar(
        r#"SELECT parent_community_id FROM social_community WHERE social_entity_id = ?"#,
    )
    .bind(community.entity_id)
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Db)?;
    if current_parent == Some(proposed_parent_id) {
        return Err(AppError::BadRequest(c.t(&loc, "ui.judicial_community_reparent.no_op")));
    }
    if community_reparent::open_reparent_case_for_community(&state.db, community.entity_id)
        .await
        .map_err(AppError::Db)?
        .is_some()
    {
        return Err(AppError::BadRequest(c.t(&loc, "ui.judicial_community_reparent.already_open")));
    }

    let rationale_html = body.rationale.as_deref().unwrap_or("").to_string();
    let rationale_sanitized = content_sanitize::sanitize_post_markup(&rationale_html);
    let rationale_plain = strip_tags_plain(&rationale_sanitized);
    if rationale_plain.trim().is_empty() {
        return Err(AppError::BadRequest(
            c.t(&loc, "judicial_case_filing.reason_required"),
        ));
    }
    let rationale_markup_stored = if rationale_html.trim().is_empty() {
        format!("<p>{}</p>", escape_html(rationale_plain.trim()))
    } else {
        rationale_html
    };

    let before =
        community_constitution::fetch_effective_constitution(&state.db, community.entity_id)
            .await
            .map_err(AppError::Db)?;
    let after = community_constitution::fetch_effective_constitution_after_reparent(
        &state.db,
        community.entity_id,
        proposed_parent_id,
    )
    .await
    .map_err(AppError::Db)?;
    let diff = community_constitution::diff_effective_constitutions(&before, &after);
    let diff_json = serde_json::to_string(&diff).map_err(|e| AppError::BadRequest(e.to_string()))?;

    let originator_rating: rust_decimal::Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(se.rating, 0) FROM social_entity se WHERE se.id = ? LIMIT 1"#,
    )
    .bind(primary_human)
    .fetch_one(&state.db)
    .await
    .map_err(AppError::Db)?;

    let mut tx = state.db.begin().await.map_err(AppError::Db)?;
    let filing_service = crate::pricing::BillableService::JudicialFiling(
        crate::pricing::JudicialTrigger::CommunityReparent,
    );
    let spend_ref = format!("community_reparent_filing:community:{}", community.entity_id);
    if !noms::spend_for_service_tx(
        &mut tx,
        primary_human,
        filing_service,
        user_id,
        Some(&spend_ref),
    )
    .await
    .map_err(AppError::Db)?
    .is_paid()
    {
        return Err(AppError::BadRequest(nom_icon::insufficient_noms_plain(
            &c,
            &loc,
            "noms.insufficient_for_community_reparent",
            BillableService::JudicialFiling(crate::pricing::JudicialTrigger::CommunityReparent)
                .nom_cost(),
            None,
        )));
    }

    let jury_size = judicial::compute_jury_size_to_freeze_at_filing_tx(
        &mut tx,
        community.entity_id,
        proposed_parent_id,
        Some(user_id),
        None,
        ref_keys::KEY_JUDICIAL_TRIGGER_COMMUNITY_REPARENT,
    )
    .await
    .map_err(AppError::Db)?;

    let case_id = community_reparent::file_reparent_case_tx(
        &mut tx,
        community.entity_id,
        proposed_parent_id,
        user_id,
        jury_size,
        Some(diff_json.as_str()),
    )
    .await
    .map_err(AppError::Db)?;

    let type_originator_id = ref_key_values::lookup_active_id(
        &state.db,
        ref_keys::GROUP_JUDICIAL_CASE_COMMENT_TYPE,
        ref_keys::KEY_JCC_ORIGINATOR,
    )
    .await
    .map_err(AppError::Db)?
    .ok_or_else(|| AppError::BadRequest(c.t(&loc, "judicial_case_filing.refs_not_configured")))?;
    sqlx::query(
        r#"
        INSERT INTO judicial_case_comment (
            judicial_case_id,
            type_id,
            body_plain,
            body_markup,
            body_html_sanitized,
            author_user_id,
            author_social_entity_id,
            author_entity_rating_at_create,
            in_reply_to_comment_id,
            created_by,
            modified_by
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
        "#,
    )
    .bind(case_id)
    .bind(type_originator_id)
    .bind(rationale_plain.as_str())
    .bind(rationale_markup_stored.as_str())
    .bind(rationale_sanitized.as_str())
    .bind(user_id)
    .bind(primary_human)
    .bind(originator_rating)
    .bind(user_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(AppError::Db)?;

    tx.commit().await.map_err(AppError::Db)?;

    let _ = crate::jobs::enqueue_job(
        &state.db,
        None,
        "judicial_jury_nomination",
        &serde_json::json!({ "limit": 32 }),
        Some(&format!("judicial-case:{case_id}")),
        30,
    )
    .await;

    let return_to = sanitize_return_to(body.return_to.as_deref());
    Ok(Redirect::to(&return_to).into_response())
}

const CREATE_ENTITY_GATHERING_FRAGMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/create_entity_card_gathering_fragment.html"
));

const HOME_FEED_PANEL: &str = r#"
        <div id="app-home-feed-panel" class="app-cards">
        </div>
"#;

const HOME_FEED_DEFAULT: &str = "";

const HOME_ONBOARDING_LOCKED_PANEL: &str = r#"
        <div id="app-home-feed-panel" class="app-cards">
          <section class="card app-card app-card--placeholder">
            <div class="card-accent" role="presentation"></div>
            <h1>@@ui.home.onboarding_locked_title@@</h1>
            <p class="sub">@@ui.home.onboarding_locked_body@@</p>
            <p class="sub"><a href="/onboarding">@@ui.home.onboarding_locked_cta@@</a></p>
          </section>
        </div>
"#;
