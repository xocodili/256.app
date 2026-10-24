//! `POST /jobs/enqueue` must not expose system job types to verified users.

mod common;

use app::jobs::is_user_http_enqueue_allowed;

#[test]
fn system_job_types_are_not_user_http_enqueueable() {
    for key in [
        "godmother_outreach",
        "godmother_welcome",
        "council_reply_tick",
        "council_post_tick",
        "council_post_member",
        "council_share_shortlist",
        "council_share_read",
        "council_share_publish",
        "council_portfolio_feed_poll",
        "council_invite_tick",
        "council_invite_article_pick",
        "council_invite_person_discover",
        "council_invite_contact_resolve",
        "council_invite_compose_send",
        "content_domain_lifecycle",
        "judicial_jury_nomination",
        "council_reply_member",
        "judicial_council_vote_member",
        "judicial_council_appeal",
        "emergency_standing_settlement",
        "conversation_rating_settlement",
        "community_graph_lifecycle",
        "scope_recalc_drain",
        "scope_rematerialize_all",
        "user_engagement_digest",
        "panoply_daily_status",
        "system_journal_retention",
    ] {
        assert!(
            !is_user_http_enqueue_allowed(key),
            "expected {key} to be blocked from user HTTP enqueue"
        );
    }
}
