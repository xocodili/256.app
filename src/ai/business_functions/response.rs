//! JSON response parsers for council business functions (with prose fallback for moderation).

use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModerationVote {
    Uphold,
    Overturn,
}

#[derive(Debug, Clone)]
pub struct ModerationReviewDecision {
    pub vote: ModerationVote,
    pub ruling: Option<String>,
    pub reasoning: Option<String>,
    pub advisory: Option<String>,
    pub raw: String,
}

#[derive(Debug, Clone)]
pub enum ThreadReplyDecision {
    Reply { parent_post_id: u64, body_markup: String },
    Decline { reason: Option<String> },
}

#[derive(Debug, Clone)]
pub enum ContentShortlistDecision {
    Shortlist { content_link_ids: Vec<u64> },
    Decline { reason: Option<String> },
}

#[derive(Debug, Clone)]
pub enum ContentShareDecision {
    Share {
        content_link_id: u64,
        body_markup: String,
    },
    Decline { reason: Option<String> },
}

#[derive(Debug, Clone)]
pub enum PortfolioReplaceDecision {
    Replace {
        proof_url: String,
        body_markup: String,
    },
    Decline { reason: Option<String> },
}

#[derive(Debug, Clone)]
pub enum InviteDecision {
    Invite {
        intended_email: String,
        intended_label: Option<String>,
        personal_note: Option<String>,
    },
    Decline { reason: Option<String> },
}

#[derive(Debug, Clone)]
pub enum InviteArticleShortlistDecision {
    Shortlist { post_ids: Vec<u64> },
    Decline { reason: Option<String> },
}

#[derive(Debug, Clone)]
pub struct InvitePersonCandidateParsed {
    pub post_id: u64,
    pub intended_label: String,
    pub profile_url: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub enum InvitePersonDiscoverDecision {
    Candidates { people: Vec<InvitePersonCandidateParsed> },
    Decline { reason: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GodmotherWelcomeDecision {
    Welcome { body_markup: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GodmotherOutreachDecision {
    Outreach { body_markup: String },
}

#[derive(Debug, Deserialize)]
struct ModerationJson {
    vote: Option<String>,
    ruling: Option<String>,
    reasoning: Option<String>,
    advisory: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ActionJson {
    action: Option<String>,
    parent_post_id: Option<u64>,
    content_link_id: Option<u64>,
    content_link_ids: Option<Vec<u64>>,
    proof_url: Option<String>,
    body_markup: Option<String>,
    intended_email: Option<String>,
    intended_label: Option<String>,
    personal_note: Option<String>,
    reason: Option<String>,
    post_ids: Option<Vec<u64>>,
    people: Option<Vec<PersonCandidateJson>>,
}

#[derive(Debug, Deserialize)]
struct PersonCandidateJson {
    post_id: Option<u64>,
    intended_label: Option<String>,
    profile_url: Option<String>,
    reason: Option<String>,
}

fn extract_json_object(text: &str) -> Option<Value> {
    let trimmed = text.trim();
    if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
        return Some(v);
    }
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str(&trimmed[start..=end]).ok()
}

/// LLMs often emit numeric ids as strings; accept both.
fn parse_json_u64(v: &Value) -> Option<u64> {
    match v {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn parse_json_u64_list(v: &Value) -> Option<Vec<u64>> {
    let arr = v.as_array()?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        out.push(parse_json_u64(item)?);
    }
    Some(out)
}

fn json_field<'a>(v: &'a Value, snake: &str, camel: &str) -> Option<&'a Value> {
    v.get(snake).or_else(|| v.get(camel))
}

/// Why [`parse_thread_reply_response`] returned `None` (for ops logging).
#[must_use]
pub fn thread_reply_parse_diagnosis(text: &str, allowed_parent_ids: &[u64]) -> &'static str {
    let Some(v) = extract_json_object(text) else {
        return "no_json_object";
    };
    let Some(action_raw) = json_field(&v, "action", "action")
        .and_then(|a| a.as_str())
        .map(str::trim)
    else {
        return "missing_action";
    };
    let action = action_raw.to_ascii_lowercase();
    match action.as_str() {
        "decline" => "ok_decline",
        "reply" => {
            let Some(parent_raw) = json_field(&v, "parent_post_id", "parentPostId") else {
                return "missing_parent_post_id";
            };
            let Some(parent) = parse_json_u64(parent_raw) else {
                return "invalid_parent_post_id";
            };
            if !allowed_parent_ids.contains(&parent) {
                return "stale_parent_treated_as_decline";
            }
            let body = json_field(&v, "body_markup", "bodyMarkup")
                .and_then(|b| b.as_str())
                .unwrap_or("")
                .trim();
            if body.is_empty() {
                let reason = json_field(&v, "reason", "reason")
                    .and_then(|r| r.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty());
                if reason.is_some() {
                    return "reply_used_reason_not_body_markup";
                }
                return "empty_body_markup";
            }
            "ok_reply"
        }
        _ => "unknown_action",
    }
}

fn parse_vote_token(raw: &str) -> Option<ModerationVote> {
    let upper = raw.trim().to_ascii_uppercase();
    if upper.contains("OVERTURN") {
        Some(ModerationVote::Overturn)
    } else if upper.contains("UPHOLD") {
        Some(ModerationVote::Uphold)
    } else {
        None
    }
}

pub fn parse_moderation_review_response(text: &str) -> ModerationReviewDecision {
    if let Some(v) = extract_json_object(text) {
        if let Ok(parsed) = serde_json::from_value::<ModerationJson>(v) {
            if let Some(vote_raw) = parsed.vote.as_deref() {
                if let Some(vote) = parse_vote_token(vote_raw) {
                    return ModerationReviewDecision {
                        vote,
                        ruling: parsed.ruling,
                        reasoning: parsed.reasoning,
                        advisory: parsed.advisory,
                        raw: text.to_string(),
                    };
                }
            }
        }
    }
    let upper = text.to_ascii_uppercase();
    let vote = if upper.contains("OVERTURN") {
        ModerationVote::Overturn
    } else {
        ModerationVote::Uphold
    };
    ModerationReviewDecision {
        vote,
        ruling: None,
        reasoning: None,
        advisory: None,
        raw: text.to_string(),
    }
}

fn thread_reply_body_markup(v: &Value) -> Option<String> {
    let body = json_field(v, "body_markup", "bodyMarkup")
        .and_then(|b| b.as_str())
        .unwrap_or("")
        .trim();
    if !body.is_empty() {
        return Some(body.to_string());
    }
    let reason = json_field(v, "reason", "reason")
        .and_then(|r| r.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())?;
    if reason.contains('<') {
        Some(reason.to_string())
    } else {
        Some(format!("<p>{}</p>", reason))
    }
}

pub fn parse_thread_reply_response(
    text: &str,
    allowed_parent_ids: &[u64],
) -> Option<ThreadReplyDecision> {
    let v = extract_json_object(text)?;
    let action = json_field(&v, "action", "action")
        .and_then(|a| a.as_str())
        .map(str::trim)
        .map(str::to_ascii_lowercase)?;
    match action.as_str() {
        "reply" => {
            let parent = parse_json_u64(json_field(&v, "parent_post_id", "parentPostId")?)?;
            if !allowed_parent_ids.contains(&parent) {
                let reason = json_field(&v, "reason", "reason")
                    .and_then(|r| r.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .or_else(|| Some(format!("stale parent_post_id {parent}")));
                return Some(ThreadReplyDecision::Decline { reason });
            }
            let body = thread_reply_body_markup(&v)?;
            Some(ThreadReplyDecision::Reply {
                parent_post_id: parent,
                body_markup: body,
            })
        }
        "decline" => {
            let reason = json_field(&v, "reason", "reason")
                .and_then(|r| r.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            Some(ThreadReplyDecision::Decline { reason })
        }
        _ => None,
    }
}

pub fn parse_portfolio_replace_response(text: &str) -> Option<PortfolioReplaceDecision> {
    let v = extract_json_object(text)?;
    let parsed: ActionJson = serde_json::from_value(v).ok()?;
    match parsed
        .action
        .as_deref()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("replace") => {
            let proof_url = parsed.proof_url?.trim().to_string();
            if proof_url.is_empty() || !proof_url.starts_with("http") {
                return None;
            }
            let body = parsed.body_markup?.trim().to_string();
            if body.is_empty() {
                return None;
            }
            Some(PortfolioReplaceDecision::Replace {
                proof_url,
                body_markup: body,
            })
        }
        Some("decline") => Some(PortfolioReplaceDecision::Decline {
            reason: parsed.reason,
        }),
        _ => None,
    }
}

pub fn parse_content_shortlist_response(
    text: &str,
    allowed_link_ids: &[u64],
) -> Option<ContentShortlistDecision> {
    let v = extract_json_object(text)?;
    let action = json_field(&v, "action", "action")
        .and_then(|a| a.as_str())
        .map(str::trim)
        .map(str::to_ascii_lowercase)?;
    match action.as_str() {
        "shortlist" => {
            let ids = json_field(&v, "content_link_ids", "contentLinkIds")
                .and_then(parse_json_u64_list)?;
            if ids.is_empty() {
                return None;
            }
            let mut out = Vec::new();
            for id in ids {
                if allowed_link_ids.contains(&id) && !out.contains(&id) {
                    out.push(id);
                }
            }
            if out.is_empty() {
                return None;
            }
            out.truncate(crate::content::portfolio::COUNCIL_SHORTLIST_TARGET);
            Some(ContentShortlistDecision::Shortlist {
                content_link_ids: out,
            })
        }
        "decline" => {
            let reason = json_field(&v, "reason", "reason")
                .and_then(|r| r.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            Some(ContentShortlistDecision::Decline { reason })
        }
        _ => None,
    }
}

fn share_body_markup(v: &Value) -> Option<String> {
    let body = json_field(v, "body_markup", "bodyMarkup")
        .and_then(|b| b.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())?;
    Some(body.to_string())
}

pub fn parse_content_share_response(
    text: &str,
    allowed_link_ids: &[u64],
) -> Option<ContentShareDecision> {
    let v = extract_json_object(text)?;
    let action = json_field(&v, "action", "action")
        .and_then(|a| a.as_str())
        .map(str::trim)
        .map(str::to_ascii_lowercase)?;
    match action.as_str() {
        "share" => {
            let link_id =
                parse_json_u64(json_field(&v, "content_link_id", "contentLinkId")?)?;
            if !allowed_link_ids.contains(&link_id) {
                return None;
            }
            let body = share_body_markup(&v)?;
            Some(ContentShareDecision::Share {
                content_link_id: link_id,
                body_markup: body,
            })
        }
        "decline" => {
            let reason = json_field(&v, "reason", "reason")
                .and_then(|r| r.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            Some(ContentShareDecision::Decline { reason })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moderation_json_uphold() {
        let d = parse_moderation_review_response(
            r#"{"vote":"uphold","ruling":"Clear","reasoning":"ok","advisory":"none"}"#,
        );
        assert_eq!(d.vote, ModerationVote::Uphold);
    }

    #[test]
    fn thread_reply_validates_parent_id() {
        let d = parse_thread_reply_response(
            r#"{"action":"reply","parent_post_id":42,"body_markup":"<p>hi</p>"}"#,
            &[42],
        );
        assert!(matches!(d, Some(ThreadReplyDecision::Reply { .. })));
        assert!(matches!(
            parse_thread_reply_response(
                r#"{"action":"reply","parent_post_id":99,"body_markup":"<p>hi</p>"}"#,
                &[42],
            ),
            Some(ThreadReplyDecision::Decline { .. })
        ));
    }

    #[test]
    fn thread_reply_accepts_reason_as_body_fallback() {
        let d = parse_thread_reply_response(
            r#"{"action":"reply","parent_post_id":42,"reason":"Hello there"}"#,
            &[42],
        );
        assert!(matches!(
            d,
            Some(ThreadReplyDecision::Reply {
                body_markup,
                ..
            }) if body_markup == "<p>Hello there</p>"
        ));
    }

    #[test]
    fn thread_reply_accepts_string_parent_id() {
        let d = parse_thread_reply_response(
            r#"{"action":"reply","parent_post_id":"42","body_markup":"<p>hi</p>"}"#,
            &[42],
        );
        assert!(matches!(d, Some(ThreadReplyDecision::Reply { .. })));
    }

    #[test]
    fn thread_reply_diagnosis_parent_not_in_list() {
        assert_eq!(
            thread_reply_parse_diagnosis(
                r#"{"action":"reply","parent_post_id":99,"body_markup":"<p>hi</p>"}"#,
                &[42],
            ),
            "stale_parent_treated_as_decline"
        );
    }

    #[test]
    fn content_shortlist_validates_ids() {
        let d = parse_content_shortlist_response(
            r#"{"action":"shortlist","content_link_ids":[1,2,3]}"#,
            &[1, 2, 3, 4],
        );
        assert!(matches!(
            d,
            Some(ContentShortlistDecision::Shortlist { .. })
        ));
        assert!(parse_content_shortlist_response(
            r#"{"action":"shortlist","content_link_ids":[99]}"#,
            &[1, 2],
        )
        .is_none());
    }

    #[test]
    fn content_share_requires_body_markup_not_reason() {
        assert!(parse_content_share_response(
            r#"{"action":"share","content_link_id":7,"reason":"The summaries are too thin to comment honestly."}"#,
            &[7],
        )
        .is_none());
        let d = parse_content_share_response(
            r#"{"action":"share","content_link_id":7,"body_markup":"<p>Worth a read on civic crowdfunding.</p>"}"#,
            &[7],
        );
        assert!(matches!(d, Some(ContentShareDecision::Share { .. })));
    }

    #[test]
    fn content_share_decline_does_not_publish() {
        let d = parse_content_share_response(
            r#"{"action":"decline","reason":"Summaries too thin to comment honestly"}"#,
            &[7],
        );
        assert!(matches!(d, Some(ContentShareDecision::Decline { .. })));
    }

    #[test]
    fn content_share_validates_link_id() {
        let d = parse_content_share_response(
            r#"{"action":"share","content_link_id":7,"body_markup":"<p>read this</p>"}"#,
            &[7],
        );
        assert!(matches!(d, Some(ContentShareDecision::Share { .. })));
        assert!(parse_content_share_response(
            r#"{"action":"share","content_link_id":99,"body_markup":"<p>x</p>"}"#,
            &[7],
        )
        .is_none());
    }

    #[test]
    fn portfolio_replace_requires_proof_url() {
        let d = parse_portfolio_replace_response(
            r#"{"action":"replace","proof_url":"https://example.com/a","body_markup":"<p>ok</p>"}"#,
        );
        assert!(matches!(d, Some(PortfolioReplaceDecision::Replace { .. })));
        assert!(parse_portfolio_replace_response(
            r#"{"action":"replace","body_markup":"<p>missing url</p>"}"#,
        )
        .is_none());
    }

    #[test]
    fn godmother_welcome_requires_body() {
        let d = parse_godmother_welcome_response(
            r#"{"action":"welcome","body_markup":"<p>Welcome @newbie</p>"}"#,
        );
        assert!(matches!(d, Some(GodmotherWelcomeDecision::Welcome { .. })));
        assert!(parse_godmother_welcome_response(r#"{"action":"welcome"}"#).is_none());
    }

    #[test]
    fn invite_requires_email() {
        let d = parse_invite_response(
            r#"{"action":"invite","intended_email":"friend@example.com","personal_note":"join us"}"#,
        );
        assert!(matches!(d, Some(InviteDecision::Invite { .. })));
        assert!(parse_invite_response(r#"{"action":"invite","intended_email":"not-an-email"}"#).is_none());
    }

    #[test]
    fn invite_email_mismatch_rejected_when_verified() {
        assert!(parse_invite_response_with_verified_email(
            r#"{"action":"invite","intended_email":"wrong@example.com"}"#,
            Some("verified@example.org"),
        )
        .is_none());
        assert!(parse_invite_response_with_verified_email(
            r#"{"action":"invite","intended_email":"verified@example.org"}"#,
            Some("verified@example.org"),
        )
        .is_some());
    }

    #[test]
    fn invite_article_shortlist_respects_allowlist() {
        let d = parse_invite_article_shortlist_response(
            r#"{"action":"shortlist","post_ids":[1,2,3,4,5,6,7,8,9]}"#,
            &[1, 2, 3, 4, 5, 6, 7, 8],
        );
        match d {
            Some(InviteArticleShortlistDecision::Shortlist { post_ids }) => {
                assert_eq!(post_ids.len(), 8);
            }
            _ => panic!("expected shortlist"),
        }
    }

    #[test]
    fn invite_person_discover_requires_profile_url() {
        let d = parse_invite_person_discover_response(
            r#"{"action":"candidates","people":[{"post_id":1,"intended_label":"Jane","profile_url":"https://jane.test/about"}]}"#,
            &[1],
        );
        assert!(matches!(d, Some(InvitePersonDiscoverDecision::Candidates { .. })));
    }
}

pub fn parse_godmother_outreach_response(text: &str) -> Option<GodmotherOutreachDecision> {
    let v = extract_json_object(text)?;
    let parsed: ActionJson = serde_json::from_value(v).ok()?;
    match parsed
        .action
        .as_deref()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("welcome") | Some("outreach") => {
            let body = parsed.body_markup?.trim().to_string();
            if body.is_empty() {
                return None;
            }
            Some(GodmotherOutreachDecision::Outreach { body_markup: body })
        }
        _ => None,
    }
}

pub fn parse_godmother_welcome_response(text: &str) -> Option<GodmotherWelcomeDecision> {
    parse_godmother_outreach_response(text).map(|d| match d {
        GodmotherOutreachDecision::Outreach { body_markup } => {
            GodmotherWelcomeDecision::Welcome { body_markup }
        }
    })
}

pub fn parse_invite_response(text: &str) -> Option<InviteDecision> {
    parse_invite_response_with_verified_email(text, None)
}

/// When `verified_email` is set, reject invites whose `intended_email` does not match.
pub fn parse_invite_response_with_verified_email(
    text: &str,
    verified_email: Option<&str>,
) -> Option<InviteDecision> {
    let v = extract_json_object(text)?;
    let parsed: ActionJson = serde_json::from_value(v).ok()?;
    match parsed.action.as_deref().map(str::to_ascii_lowercase).as_deref() {
        Some("invite") => {
            let email = parsed.intended_email?.trim().to_string();
            if email.is_empty() || !email.contains('@') {
                return None;
            }
            if let Some(expected) = verified_email {
                if !email.eq_ignore_ascii_case(expected.trim()) {
                    return None;
                }
            }
            Some(InviteDecision::Invite {
                intended_email: email,
                intended_label: parsed
                    .intended_label
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty()),
                personal_note: parsed
                    .personal_note
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty()),
            })
        }
        Some("decline") => Some(InviteDecision::Decline {
            reason: parsed.reason,
        }),
        _ => None,
    }
}

pub fn parse_invite_article_shortlist_response(
    text: &str,
    allowed_post_ids: &[u64],
) -> Option<InviteArticleShortlistDecision> {
    let v = extract_json_object(text)?;
    let parsed: ActionJson = serde_json::from_value(v).ok()?;
    match parsed
        .action
        .as_deref()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("shortlist") => {
            let ids = parsed.post_ids?;
            if ids.is_empty() {
                return None;
            }
            let mut out = Vec::new();
            for id in ids {
                if allowed_post_ids.contains(&id) && !out.contains(&id) {
                    out.push(id);
                }
            }
            if out.is_empty() {
                return None;
            }
            out.truncate(8);
            Some(InviteArticleShortlistDecision::Shortlist { post_ids: out })
        }
        Some("decline") => Some(InviteArticleShortlistDecision::Decline {
            reason: parsed.reason,
        }),
        _ => None,
    }
}

pub fn parse_invite_person_discover_response(
    text: &str,
    allowed_post_ids: &[u64],
) -> Option<InvitePersonDiscoverDecision> {
    let v = extract_json_object(text)?;
    let parsed: ActionJson = serde_json::from_value(v).ok()?;
    match parsed
        .action
        .as_deref()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("candidates") => {
            let rows = parsed.people?;
            let mut out = Vec::new();
            for row in rows {
                let post_id = row.post_id?;
                if !allowed_post_ids.contains(&post_id) {
                    continue;
                }
                let label = row.intended_label?.trim().to_string();
                let profile_url = row.profile_url?.trim().to_string();
                if label.is_empty()
                    || profile_url.is_empty()
                    || !profile_url.starts_with("http")
                {
                    continue;
                }
                out.push(InvitePersonCandidateParsed {
                    post_id,
                    intended_label: label,
                    profile_url,
                    reason: row
                        .reason
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty()),
                });
            }
            if out.is_empty() {
                return None;
            }
            out.truncate(3);
            Some(InvitePersonDiscoverDecision::Candidates { people: out })
        }
        Some("decline") => Some(InvitePersonDiscoverDecision::Decline {
            reason: parsed.reason,
        }),
        _ => None,
    }
}
