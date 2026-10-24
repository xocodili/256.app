//! Shared council **business function** pipeline: budget → prompt → gRPC → JSON → side effects.
//!
//! Individual job handlers (`social::council_jobs`) build context documents and call into
//! [`runner`] + [`response`] parsers.

pub mod budget;
pub mod population;
pub mod response;
pub mod runner;

pub use budget::{
    accrue_reply_credit, daily_budget_b, daily_budget_b_live, daily_reply_budget_b,
    daily_reply_budget_b_live, ensure_budget_row, REPLY_BUDGET_FLOOR,
    reply_credit_available, spend_invite, spend_post, spend_reply,
};
pub use population::count_active_verified_humans;
pub use response::{
    parse_content_share_response, parse_content_shortlist_response, parse_godmother_outreach_response,
    parse_godmother_welcome_response, parse_invite_response, parse_invite_article_shortlist_response,
    parse_invite_person_discover_response, parse_invite_response_with_verified_email,
    parse_moderation_review_response, parse_portfolio_replace_response, parse_thread_reply_response,
    thread_reply_parse_diagnosis,
    ContentShareDecision, ContentShortlistDecision, GodmotherOutreachDecision, GodmotherWelcomeDecision,
    InviteArticleShortlistDecision, InviteDecision, InvitePersonDiscoverDecision,
    InvitePersonCandidateParsed, ModerationReviewDecision,
    PortfolioReplaceDecision, ThreadReplyDecision,
};
pub use runner::{run_council_text_bf, CouncilBfRunResult};
