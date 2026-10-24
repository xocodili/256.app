//! **Templates** — locale keys + variables → rendered subject/body (plain + branded HTML).
//!
//! Category **functions** choose a [`MailTemplate`] variant; [`render`] materializes copy.
//! HTML uses [`super::layout`] (256.app frame, logo link, CTA button, legal footer).

use crate::email::layout::{self, EmailFrameInput};
use crate::i18n::Catalog;

/// Stable template identifiers grouped by product category.
#[derive(Debug, Clone)]
pub enum MailTemplate {
    // --- Recruitment ---
    PlatformInvite {
        inviter_line: String,
        join_url: String,
        personal_note: Option<String>,
    },
    /// Council AI invite routed to operator review inbox (intended recipient in body).
    CouncilInviteReview {
        inviter_line: String,
        intended_recipient_email: String,
        intended_recipient_label: Option<String>,
        join_url: String,
        personal_note: Option<String>,
    },

    // --- Auth (system initiator) ---
    VerifyEmail { verify_url: String },
    PasswordReset { reset_url: String },

    // --- Judiciary ---
    JurorSummons { home_url: String, invite_id: String },
    EmergencyFiledAuthor { post_id: String, contest_url: String },
    EmergencyFiledFiler { post_id: String, case_id: String },
    EmergencyContestOpened { case_id: String, home_url: String },
    EmergencyStandingApplied { post_id: String },
    EmergencyContestAuthorWon { case_id: String, post_id: String, home_url: String },
    EmergencyContestAuthorLost { case_id: String, post_id: String },
    EmergencyContestFilerWon { case_id: String, post_id: String, home_url: String },
    EmergencyContestFilerLost { case_id: String, post_id: String, home_url: String },
    AiCouncilDecision { case_id: String, home_url: String },

    // --- Engagement (standing / lifecycle) ---
    CommunityFailingAdmin {
        community_label: String,
        community_handle: String,
        profile_url: String,
    },
    HumanGlobalFailing { label: String, profile_url: String },
    HumanCommunityEdgeFailing {
        community_label: String,
        profile_url: String,
    },
    HumanInactivityWarning { label: String, home_url: String },
    HumanInactivityDecrementStarted { label: String, home_url: String },
    HumanInactivityAcceleration3x { label: String, home_url: String },
    HumanInactivityAcceleration6x { label: String, home_url: String },
    HumanInactivityPruning {
        label: String,
        home_url: String,
        estimated_deletion_date: String,
    },
    /// Away-user digest — sections omitted when counts are zero.
    UserEngagementDigest {
        label: String,
        home_url: String,
        pending_connections: Option<u32>,
        audience_mentions: Option<u32>,
        replies_to_author: Option<u32>,
        missed_feed_posts: Option<u32>,
        community_posts: Option<u32>,
    },
    /// Panoply operator daily platform rollup.
    PanoplyDailyStatus {
        report_date: String,
        ops_url: String,
        metrics: crate::social::panoply::PanoplyDailyMetrics,
    },

    /// Escape hatch while migrating legacy notify helpers.
    LocalizedKeys {
        subject_key: String,
        body_key: String,
        pairs: Vec<(String, String)>,
    },
}

#[derive(Debug, Clone)]
pub struct RenderedMail {
    pub subject: String,
    pub body_plain: String,
    pub body_html: String,
}

#[must_use]
pub fn render(
    template: &MailTemplate,
    loc: &str,
    c: &Catalog,
    public_base_url: &str,
) -> RenderedMail {
    let (subject, body_plain) = render_subject_and_plain(template, loc, c);
    let frame = frame_for_template(template, loc, c, public_base_url, &body_plain);
    let body_html = layout::render_html(&frame, public_base_url, loc, c);
    RenderedMail {
        subject,
        body_plain,
        body_html,
    }
}

fn render_subject_and_plain(
    template: &MailTemplate,
    loc: &str,
    c: &Catalog,
) -> (String, String) {
    match template {
        MailTemplate::PlatformInvite {
            inviter_line,
            personal_note,
            ..
        } => {
            let note_block = personal_note
                .as_deref()
                .filter(|n| !n.trim().is_empty())
                .map(|n| format!("\n\n\"{n}\""))
                .unwrap_or_default();
            let body = format!(
                "{inviter_line} would like to invite you to 256.app:{note_block}\n\nIf you did not expect this invitation, you can ignore this message.\n"
            );
            let subject = c.tf(loc, "email.friend_invite_subject", &[("inviter_handle", inviter_line)]);
            (subject, body)
        }
        MailTemplate::CouncilInviteReview {
            inviter_line,
            intended_recipient_email,
            intended_recipient_label,
            join_url,
            personal_note,
        } => {
            let label_line = intended_recipient_label
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .map(|l| format!("  Label: {l}\n"))
                .unwrap_or_default();
            let note_block = personal_note
                .as_deref()
                .filter(|n| !n.trim().is_empty())
                .map(|n| format!("\n{n}\n"))
                .unwrap_or_else(|| "\n(no personal note drafted)\n".to_string());
            let body = c.tf(
                loc,
                "email.council_invite_review_body",
                &[
                    ("intended_email", intended_recipient_email),
                    ("intended_label_line", &label_line),
                    ("inviter_line", inviter_line),
                    ("note_block", &note_block),
                    ("join_url", join_url),
                ],
            );
            let subject = c.tf(
                loc,
                "email.council_invite_review_subject",
                &[
                    ("intended_email", intended_recipient_email),
                    ("inviter_line", inviter_line),
                ],
            );
            (subject, body)
        }
        MailTemplate::VerifyEmail { verify_url } => (
            c.t(loc, "email.verify_subject"),
            c.tf(loc, "email.verify_body", &[("verify_url", verify_url)]),
        ),
        MailTemplate::PasswordReset { reset_url } => (
            c.t(loc, "email.reset_subject"),
            c.tf(loc, "email.reset_body", &[("reset_url", reset_url)]),
        ),
        MailTemplate::JurorSummons { home_url, invite_id } => (
            c.t(loc, "email.juror_summons_subject"),
            c.tf(
                loc,
                "email.juror_summons_body",
                &[("home_url", home_url), ("invite_id", invite_id)],
            ),
        ),
        MailTemplate::EmergencyFiledAuthor {
            post_id,
            contest_url,
        } => (
            c.t(loc, "email.emergency_filed_author_subject"),
            c.tf(
                loc,
                "email.emergency_filed_author_body",
                &[("post_id", post_id), ("contest_url", contest_url)],
            ),
        ),
        MailTemplate::EmergencyFiledFiler { post_id, case_id } => (
            c.t(loc, "email.emergency_filed_filer_subject"),
            c.tf(
                loc,
                "email.emergency_filed_filer_body",
                &[("post_id", post_id), ("case_id", case_id)],
            ),
        ),
        MailTemplate::EmergencyContestOpened { case_id, home_url } => (
            c.t(loc, "email.emergency_contest_opened_subject"),
            c.tf(
                loc,
                "email.emergency_contest_opened_body",
                &[("case_id", case_id), ("home_url", home_url)],
            ),
        ),
        MailTemplate::EmergencyStandingApplied { post_id } => (
            c.t(loc, "email.emergency_standing_applied_subject"),
            c.tf(
                loc,
                "email.emergency_standing_applied_body",
                &[("post_id", post_id)],
            ),
        ),
        MailTemplate::EmergencyContestAuthorWon {
            case_id,
            post_id,
            home_url,
        } => (
            c.t(loc, "email.emergency_contest_author_won_subject"),
            c.tf(
                loc,
                "email.emergency_contest_author_won_body",
                &[("case_id", case_id), ("post_id", post_id), ("home_url", home_url)],
            ),
        ),
        MailTemplate::EmergencyContestAuthorLost { case_id, post_id } => (
            c.t(loc, "email.emergency_contest_author_lost_subject"),
            c.tf(
                loc,
                "email.emergency_contest_author_lost_body",
                &[("case_id", case_id), ("post_id", post_id)],
            ),
        ),
        MailTemplate::EmergencyContestFilerWon {
            case_id,
            post_id,
            home_url,
        } => (
            c.t(loc, "email.emergency_contest_filer_won_subject"),
            c.tf(
                loc,
                "email.emergency_contest_filer_won_body",
                &[("case_id", case_id), ("post_id", post_id), ("home_url", home_url)],
            ),
        ),
        MailTemplate::EmergencyContestFilerLost {
            case_id,
            post_id,
            home_url,
        } => (
            c.t(loc, "email.emergency_contest_filer_lost_subject"),
            c.tf(
                loc,
                "email.emergency_contest_filer_lost_body",
                &[("case_id", case_id), ("post_id", post_id), ("home_url", home_url)],
            ),
        ),
        MailTemplate::AiCouncilDecision { case_id, home_url } => (
            c.tf(
                loc,
                "email.ai_council_decision_subject",
                &[("case_id", case_id)],
            ),
            c.tf(
                loc,
                "email.ai_council_decision_body",
                &[("case_id", case_id), ("home_url", home_url)],
            ),
        ),
        MailTemplate::CommunityFailingAdmin {
            community_label,
            community_handle,
            profile_url,
        } => (
            c.tf(
                loc,
                "email.community_failing_admin_subject",
                &[("community_label", community_label)],
            ),
            c.tf(
                loc,
                "email.community_failing_admin_body",
                &[
                    ("community_label", community_label),
                    ("community_handle", community_handle),
                    ("profile_url", profile_url),
                ],
            ),
        ),
        MailTemplate::HumanGlobalFailing { label, profile_url } => (
            c.t(loc, "email.human_global_failing_subject"),
            c.tf(
                loc,
                "email.human_global_failing_body",
                &[("label", label), ("profile_url", profile_url)],
            ),
        ),
        MailTemplate::HumanCommunityEdgeFailing {
            community_label,
            profile_url,
        } => (
            c.tf(
                loc,
                "email.human_community_edge_failing_subject",
                &[("community_label", community_label)],
            ),
            c.tf(
                loc,
                "email.human_community_edge_failing_body",
                &[
                    ("community_label", community_label),
                    ("profile_url", profile_url),
                ],
            ),
        ),
        MailTemplate::HumanInactivityWarning { label, home_url } => (
            c.tf(
                loc,
                "email.human_inactivity_warning_subject",
                &[("label", label)],
            ),
            c.tf(
                loc,
                "email.human_inactivity_warning_body",
                &[("label", label), ("home_url", home_url)],
            ),
        ),
        MailTemplate::HumanInactivityDecrementStarted { label, home_url } => (
            c.tf(
                loc,
                "email.human_inactivity_decrement_started_subject",
                &[("label", label)],
            ),
            c.tf(
                loc,
                "email.human_inactivity_decrement_started_body",
                &[("label", label), ("home_url", home_url)],
            ),
        ),
        MailTemplate::HumanInactivityAcceleration3x { label, home_url } => (
            c.tf(
                loc,
                "email.human_inactivity_acceleration_3x_subject",
                &[("label", label)],
            ),
            c.tf(
                loc,
                "email.human_inactivity_acceleration_3x_body",
                &[("label", label), ("home_url", home_url)],
            ),
        ),
        MailTemplate::HumanInactivityAcceleration6x { label, home_url } => (
            c.tf(
                loc,
                "email.human_inactivity_acceleration_6x_subject",
                &[("label", label)],
            ),
            c.tf(
                loc,
                "email.human_inactivity_acceleration_6x_body",
                &[("label", label), ("home_url", home_url)],
            ),
        ),
        MailTemplate::HumanInactivityPruning {
            label,
            home_url,
            estimated_deletion_date,
        } => (
            c.tf(
                loc,
                "email.human_inactivity_pruning_subject",
                &[("label", label)],
            ),
            c.tf(
                loc,
                "email.human_inactivity_pruning_body",
                &[
                    ("label", label),
                    ("home_url", home_url),
                    ("estimated_deletion_date", estimated_deletion_date),
                ],
            ),
        ),
        MailTemplate::UserEngagementDigest {
            label,
            home_url,
            pending_connections,
            audience_mentions,
            replies_to_author,
            missed_feed_posts,
            community_posts,
        } => (
            c.tf(
                loc,
                "email.user_engagement_digest_subject",
                &[("label", label)],
            ),
            engagement_digest_body_plain(
                c,
                loc,
                label,
                home_url,
                *pending_connections,
                *audience_mentions,
                *replies_to_author,
                *missed_feed_posts,
                *community_posts,
            ),
        ),
        MailTemplate::PanoplyDailyStatus {
            report_date,
            ops_url,
            metrics,
        } => (
            c.tf(
                loc,
                "email.panoply_daily_status_subject",
                &[("report_date", report_date)],
            ),
            panoply_daily_body_plain(c, loc, report_date, ops_url, metrics),
        ),
        MailTemplate::LocalizedKeys {
            subject_key,
            body_key,
            pairs,
        } => {
            let refs: Vec<(&str, &str)> =
                pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
            (
                c.tf(loc, subject_key, &refs),
                c.tf(loc, body_key, &refs),
            )
        }
    }
}

fn frame_for_template(
    template: &MailTemplate,
    loc: &str,
    c: &Catalog,
    public_base_url: &str,
    body_plain: &str,
) -> EmailFrameInput {
    let base = public_base_url.trim_end_matches('/');
    let home = format!("{base}/home");
    let login = format!("{base}/login");

    match template {
        MailTemplate::PlatformInvite {
            inviter_line,
            join_url,
            personal_note,
        } => {
            let base = base.trim_end_matches('/');
            let site_link = format!(
                r#"<a href="{base}" style="color:{YELLOW};text-decoration:none;">256.app</a>"#,
                YELLOW = "#f4c430"
            );
            let mut html_paras: Vec<String> = vec![format!(
                "{} would like to invite you to {}:",
                crate::html_escape::escape_html(inviter_line),
                site_link
            )];
            if let Some(note) = personal_note
                .as_deref()
                .filter(|n| !n.trim().is_empty())
            {
                html_paras.push(format!(
                    r#"<em style="color:#c4c4d4;">&ldquo;{}&rdquo;</em>"#,
                    crate::html_escape::escape_html(note.trim())
                ));
            }
            html_paras.push("If you did not expect this invitation, you can ignore this message.".to_string());
            EmailFrameInput {
                type_heading: c.tf(loc, "email.type.invite_from", &[("inviter", inviter_line)]),
                paragraphs: layout::paragraphs_from_body(body_plain, join_url),
                html_paragraphs: Some(html_paras),
                note: None,
                cta_label: c.t(loc, "email.cta.join_in"),
                cta_url: join_url.clone(),
            }
        }
        MailTemplate::CouncilInviteReview {
            inviter_line,
            intended_recipient_email,
            intended_recipient_label,
            join_url,
            personal_note,
        } => {
            let base = base.trim_end_matches('/');
            let site_link = format!(
                r#"<a href="{base}" style="color:{YELLOW};text-decoration:none;">256.app</a>"#,
                YELLOW = "#f4c430"
            );
            let label_html = intended_recipient_label
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .map(|l| {
                    format!(
                        r#"<div style="margin-top:4px;">Label: <strong>{}</strong></div>"#,
                        crate::html_escape::escape_html(l.trim())
                    )
                })
                .unwrap_or_default();
            let review_banner = format!(
                r#"<div style="margin:0 0 16px;padding:12px 14px;border-left:4px solid #f4c430;background:#1a1a24;color:#e8e8f0;">
<strong>Review copy</strong> — this invite was <em>not</em> sent to the intended recipient.
<div style="margin-top:10px;font-size:14px;">
<div>Intended recipient email: <strong>{}</strong></div>
{}
</div>
</div>"#,
                crate::html_escape::escape_html(intended_recipient_email),
                label_html
            );
            let mut html_paras: Vec<String> = vec![review_banner, format!(
                "{} would like to invite them to {}:",
                crate::html_escape::escape_html(inviter_line),
                site_link
            )];
            if let Some(note) = personal_note
                .as_deref()
                .filter(|n| !n.trim().is_empty())
            {
                html_paras.push(format!(
                    r#"<em style="color:#c4c4d4;">&ldquo;{}&rdquo;</em>"#,
                    crate::html_escape::escape_html(note.trim())
                ));
            }
            EmailFrameInput {
                type_heading: c.t(loc, "email.type.council_invite_review"),
                paragraphs: layout::paragraphs_from_body(body_plain, join_url),
                html_paragraphs: Some(html_paras),
                note: None,
                cta_label: c.t(loc, "email.cta.join_in"),
                cta_url: join_url.clone(),
            }
        }
        MailTemplate::VerifyEmail { verify_url } => EmailFrameInput {
            type_heading: c.t(loc, "email.type.verify"),
            paragraphs: layout::paragraphs_from_body(body_plain, verify_url),
            note: None,
            cta_label: c.t(loc, "email.cta.verify_now"),
            html_paragraphs: None,
            cta_url: verify_url.clone(),
        },
        MailTemplate::PasswordReset { reset_url } => EmailFrameInput {
            type_heading: c.t(loc, "email.type.reset"),
            paragraphs: layout::paragraphs_from_body(body_plain, reset_url),
            note: None,
            cta_label: c.t(loc, "email.cta.reset_password"),
            html_paragraphs: None,
            cta_url: reset_url.clone(),
        },
        MailTemplate::JurorSummons { home_url, .. } => EmailFrameInput {
            type_heading: c.t(loc, "email.type.jury"),
            paragraphs: layout::paragraphs_from_body(body_plain, home_url),
            note: None,
            cta_label: c.t(loc, "email.cta.sign_in"),
            html_paragraphs: None,
            cta_url: home_url.clone(),
        },
        MailTemplate::EmergencyFiledAuthor { contest_url, .. } => EmailFrameInput {
            type_heading: c.t(loc, "email.type.emergency"),
            paragraphs: layout::paragraphs_from_body(body_plain, contest_url),
            note: None,
            cta_label: c.t(loc, "email.cta.contest"),
            html_paragraphs: None,
            cta_url: contest_url.clone(),
        },
        MailTemplate::EmergencyFiledFiler { .. } => EmailFrameInput {
            type_heading: c.t(loc, "email.type.emergency"),
            paragraphs: layout::paragraphs_from_body(body_plain, &home),
            note: None,
            cta_label: c.t(loc, "email.cta.sign_in"),
            html_paragraphs: None,
            cta_url: home.clone(),
        },
        MailTemplate::EmergencyContestOpened { home_url, .. }
        | MailTemplate::EmergencyContestAuthorWon { home_url, .. }
        | MailTemplate::EmergencyContestFilerWon { home_url, .. }
        | MailTemplate::EmergencyContestFilerLost { home_url, .. }
        | MailTemplate::AiCouncilDecision { home_url, .. } => EmailFrameInput {
            type_heading: email_type_for_template(template, loc, c),
            paragraphs: layout::paragraphs_from_body(body_plain, home_url),
            note: None,
            cta_label: c.t(loc, "email.cta.sign_in"),
            html_paragraphs: None,
            cta_url: home_url.clone(),
        },
        MailTemplate::EmergencyStandingApplied { .. }
        | MailTemplate::EmergencyContestAuthorLost { .. } => EmailFrameInput {
            type_heading: c.t(loc, "email.type.emergency"),
            paragraphs: layout::paragraphs_from_body(body_plain, &login),
            note: None,
            cta_label: c.t(loc, "email.cta.sign_in"),
            html_paragraphs: None,
            cta_url: login.clone(),
        },
        MailTemplate::CommunityFailingAdmin { profile_url, .. }
        | MailTemplate::HumanGlobalFailing { profile_url, .. }
        | MailTemplate::HumanCommunityEdgeFailing { profile_url, .. } => EmailFrameInput {
            type_heading: c.t(loc, "email.type.standing"),
            paragraphs: layout::paragraphs_from_body(body_plain, profile_url),
            note: None,
            cta_label: c.t(loc, "email.cta.view_profile"),
            html_paragraphs: None,
            cta_url: profile_url.clone(),
        },
        MailTemplate::HumanInactivityWarning { home_url, .. }
        | MailTemplate::HumanInactivityDecrementStarted { home_url, .. }
        | MailTemplate::HumanInactivityAcceleration3x { home_url, .. }
        | MailTemplate::HumanInactivityAcceleration6x { home_url, .. }
        | MailTemplate::HumanInactivityPruning { home_url, .. } => EmailFrameInput {
            type_heading: c.t(loc, "email.type.inactivity"),
            paragraphs: layout::paragraphs_from_body(body_plain, home_url),
            note: None,
            cta_label: c.t(loc, "email.cta.sign_in"),
            html_paragraphs: None,
            cta_url: home_url.clone(),
        },
        MailTemplate::UserEngagementDigest { home_url, .. } => EmailFrameInput {
            type_heading: c.t(loc, "email.type.engagement"),
            paragraphs: layout::paragraphs_from_body(body_plain, home_url),
            note: None,
            cta_label: c.t(loc, "email.cta.sign_in"),
            html_paragraphs: None,
            cta_url: home_url.clone(),
        },
        MailTemplate::PanoplyDailyStatus { ops_url, .. } => EmailFrameInput {
            type_heading: c.t(loc, "email.type.panoply_ops"),
            paragraphs: layout::paragraphs_from_body(body_plain, ops_url),
            note: None,
            cta_label: c.t(loc, "email.cta.open_ops"),
            html_paragraphs: None,
            cta_url: ops_url.clone(),
        },
        MailTemplate::LocalizedKeys { body_key, pairs, .. } => {
            let action_url = pairs
                .iter()
                .find_map(|(k, v)| {
                    matches!(
                        k.as_str(),
                        "verify_url"
                            | "reset_url"
                            | "join_url"
                            | "home_url"
                            | "profile_url"
                            | "contest_url"
                    )
                    .then(|| v.clone())
                })
                .unwrap_or_else(|| home.clone());
            EmailFrameInput {
                type_heading: localized_type_heading(body_key, loc, c),
                paragraphs: layout::paragraphs_from_body(body_plain, &action_url),
                note: None,
                cta_label: localized_cta_label(body_key, loc, c),
            html_paragraphs: None,
                cta_url: action_url,
            }
        }
    }
}

fn email_type_for_template(template: &MailTemplate, loc: &str, c: &Catalog) -> String {
    match template {
        MailTemplate::AiCouncilDecision { .. } => c.t(loc, "email.type.council"),
        _ => c.t(loc, "email.type.emergency"),
    }
}

fn localized_type_heading(body_key: &str, loc: &str, c: &Catalog) -> String {
    if body_key.contains("juror") {
        c.t(loc, "email.type.jury")
    } else if body_key.contains("emergency") || body_key.contains("ai_council") {
        if body_key.contains("ai_council") {
            c.t(loc, "email.type.council")
        } else {
            c.t(loc, "email.type.emergency")
        }
    } else if body_key.contains("inactivity") {
        c.t(loc, "email.type.inactivity")
    } else if body_key.contains("failing") {
        c.t(loc, "email.type.standing")
    } else if body_key.contains("council_invite_review") {
        c.t(loc, "email.type.council_invite_review")
    } else {
        c.t(loc, "email.type.notification")
    }
}

fn localized_cta_label(body_key: &str, loc: &str, c: &Catalog) -> String {
    if body_key.contains("profile_url") {
        c.t(loc, "email.cta.view_profile")
    } else if body_key.contains("contest") {
        c.t(loc, "email.cta.contest")
    } else if body_key.contains("join_url") {
        c.t(loc, "email.cta.join_in")
    } else if body_key.contains("verify_url") {
        c.t(loc, "email.cta.verify_now")
    } else if body_key.contains("reset_url") {
        c.t(loc, "email.cta.reset_password")
    } else {
        c.t(loc, "email.cta.sign_in")
    }
}

fn panoply_entity_line(
    c: &Catalog,
    loc: &str,
    type_label: &str,
    counts: &crate::social::panoply::EntityTypeDayCounts,
) -> String {
    c.tf(
        loc,
        "email.panoply_daily_status_entity_line",
        &[
            ("type_label", type_label),
            ("new_count", &counts.new_today.to_string()),
            ("retired_count", &counts.retired_today.to_string()),
            ("active_count", &counts.active_verified.to_string()),
        ],
    )
}

fn panoply_daily_body_plain(
    c: &Catalog,
    loc: &str,
    report_date: &str,
    ops_url: &str,
    metrics: &crate::social::panoply::PanoplyDailyMetrics,
) -> String {
    let mut lines = vec![c.tf(
        loc,
        "email.panoply_daily_status_intro",
        &[("report_date", report_date)],
    )];
    lines.push(c.t(loc, "email.panoply_daily_status_entities_heading"));
    lines.push(panoply_entity_line(c, loc, "Humans", &metrics.human));
    lines.push(panoply_entity_line(
        c,
        loc,
        "Communities",
        &metrics.community,
    ));
    lines.push(panoply_entity_line(c, loc, "Profits", &metrics.profit));
    lines.push(panoply_entity_line(
        c,
        loc,
        "Non-profits",
        &metrics.non_profit,
    ));
    lines.push(panoply_entity_line(c, loc, "Gatherings", &metrics.gathering));
    lines.push(panoply_entity_line(c, loc, "Digital", &metrics.digital));
    lines.push(c.tf(
        loc,
        "email.panoply_daily_status_humans_total",
        &[(
            "count",
            &metrics.active_verified_humans_total.to_string(),
        )],
    ));
    lines.push(String::new());
    lines.push(c.t(loc, "email.panoply_daily_status_judiciary_heading"));
    lines.push(c.tf(
        loc,
        "email.panoply_daily_status_judiciary_line",
        &[
            ("opened", &metrics.judiciary_opened.to_string()),
            ("resolved", &metrics.judiciary_resolved.to_string()),
            ("unresolved", &metrics.judiciary_unresolved.to_string()),
        ],
    ));
    lines.push(String::new());
    lines.push(c.t(loc, "email.panoply_daily_status_posts_heading"));
    lines.push(c.tf(
        loc,
        "email.panoply_daily_status_posts_line",
        &[
            ("new_posts", &metrics.posts_new.to_string()),
            ("resolved_posts", &metrics.posts_resolved.to_string()),
            ("active_posts", &metrics.posts_active_disputed.to_string()),
        ],
    ));
    lines.push(String::new());
    lines.push(c.t(loc, "email.panoply_daily_status_ops_heading"));
    lines.push(c.tf(
        loc,
        "email.panoply_daily_status_ops_line",
        &[
            ("operator_actions", &metrics.operator_actions.to_string()),
            ("contained", &metrics.contained_emergencies.to_string()),
            ("sanitation", &metrics.sanitation_events.to_string()),
            ("dau", &metrics.dau.to_string()),
        ],
    ));
    lines.push(c.tf(
        loc,
        "email.panoply_daily_status_outro",
        &[("ops_url", ops_url)],
    ));
    lines.join("\n")
}

fn engagement_digest_body_plain(
    c: &Catalog,
    loc: &str,
    label: &str,
    home_url: &str,
    pending_connections: Option<u32>,
    audience_mentions: Option<u32>,
    replies_to_author: Option<u32>,
    missed_feed_posts: Option<u32>,
    community_posts: Option<u32>,
) -> String {
    let mut lines = vec![c.tf(
        loc,
        "email.user_engagement_digest_intro",
        &[("label", label)],
    )];
    if let Some(n) = pending_connections {
        lines.push(c.tf(
            loc,
            "email.user_engagement_digest_pending_connections",
            &[("count", &n.to_string())],
        ));
    }
    if let Some(n) = audience_mentions {
        lines.push(c.tf(
            loc,
            "email.user_engagement_digest_audience_mentions",
            &[("count", &n.to_string())],
        ));
    }
    if let Some(n) = replies_to_author {
        lines.push(c.tf(
            loc,
            "email.user_engagement_digest_replies",
            &[("count", &n.to_string())],
        ));
    }
    if let Some(n) = missed_feed_posts {
        lines.push(c.tf(
            loc,
            "email.user_engagement_digest_missed_posts",
            &[("count", &n.to_string())],
        ));
    }
    if let Some(n) = community_posts {
        lines.push(c.tf(
            loc,
            "email.user_engagement_digest_community_posts",
            &[("count", &n.to_string())],
        ));
    }
    lines.push(c.tf(
        loc,
        "email.user_engagement_digest_outro",
        &[("home_url", home_url)],
    ));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_email_html_has_branded_cta() {
        let c = crate::i18n::catalog();
        let mail = render(
            &MailTemplate::VerifyEmail {
                verify_url: "https://256.app/verify-email?token=abc".to_string(),
            },
            "en",
            &c,
            "https://256.app",
        );
        assert!(mail.body_html.contains("Verify Now"));
        assert!(mail.body_html.contains("https://256.app/verify-email?token=abc"));
        assert!(mail.body_html.contains("256.app"));
        assert!(mail.body_html.contains("href=\"https://256.app\""));
        assert!(mail.body_html.contains(crate::email::layout::EMAIL_LOGO_PATH));
        assert!(!mail.body_html.contains("/static/branding/favicon.svg"));
    }
}

