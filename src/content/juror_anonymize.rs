//! Strip identifying party references from HTML shown to human jurors.

use crate::content::sanitize;

/// Sanitized markup safe for jurors: no links, images, or embedded media.
#[must_use]
pub fn sanitize_juror_facing_markup(raw: &str) -> String {
    let mut builder = ammonia::Builder::default();
    builder.add_tags([
        "u", "span", "font", "p", "br", "div", "ul", "ol", "li", "strong", "b", "em", "i",
        "blockquote", "h1", "h2", "h3",
    ]);
    builder.add_tag_attributes("font", ["color"]);
    builder.rm_tags(["a", "img"]);
    builder.clean(raw).to_string()
}

/// Body HTML for juror workspace: quarantine outbound navigation, redact @handles and profile paths.
#[must_use]
pub fn juror_facing_body(html: &str, plain: &str, party_label: &str, link_redacted: &str) -> String {
    if !html.trim().is_empty() {
        let cleaned = sanitize_juror_facing_markup(html);
        redact_party_references(&cleaned, party_label)
    } else if !plain.trim().is_empty() {
        let text = redact_party_plain(plain.trim(), party_label, link_redacted);
        format!("<p>{}</p>", crate::html_escape::escape_html(&text))
    } else {
        String::new()
    }
}

#[must_use]
pub fn redact_party_plain(plain: &str, party_label: &str, link_redacted: &str) -> String {
    let urls = redact_http_urls_in_plain(plain, link_redacted);
    redact_at_handles(&urls, party_label)
}

#[must_use]
pub fn redact_party_references(s: &str, party_label: &str) -> String {
    redact_profile_path_mentions(&redact_at_handles(s, party_label), party_label)
}

/// Replace `@handle` tokens (composer / prose mentions).
#[must_use]
pub fn redact_at_handles(s: &str, party_label: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while !rest.is_empty() {
        let Some(pos) = rest.find('@') else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..pos]);
        let after_at = &rest[pos + 1..];
        let handle_len = after_at
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .count();
        if handle_len > 0 {
            out.push_str(party_label);
            let skip = 1 + after_at
                .char_indices()
                .nth(handle_len)
                .map(|(i, _)| i)
                .unwrap_or(after_at.len());
            rest = &rest[pos + skip..];
        } else {
            out.push('@');
            rest = &rest[pos + 1..];
        }
    }
    out
}

/// Remove `/home/profile/{handle}` path segments from text/HTML fragments.
#[must_use]
pub fn redact_profile_path_mentions(s: &str, party_label: &str) -> String {
    const PREFIX: &str = "/home/profile/";
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(pos) = rest.find(PREFIX) {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + PREFIX.len()..];
        let handle_len = after
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .count();
        if handle_len > 0 {
            out.push_str(party_label);
            let skip = PREFIX.len()
                + after
                    .char_indices()
                    .nth(handle_len)
                    .map(|(i, _)| i)
                    .unwrap_or(after.len());
            rest = &rest[pos + skip..];
        } else {
            out.push_str(PREFIX);
            rest = &rest[pos + PREFIX.len()..];
        }
    }
    out.push_str(rest);
    out
}

#[must_use]
pub fn redact_http_urls_in_plain(text: &str, link_redacted: &str) -> String {
    let mut redacted = String::with_capacity(text.len());
    let mut rest = text;
    while !rest.is_empty() {
        let pos_https = rest.find("https://");
        let pos_http = rest.find("http://");
        let url_start = match (pos_https, pos_http) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        match url_start {
            None => {
                redacted.push_str(rest);
                break;
            }
            Some(pos) => {
                redacted.push_str(&rest[..pos]);
                redacted.push_str(link_redacted);
                let after = &rest[pos..];
                let skip_len = if after.starts_with("https://") {
                    "https://".len()
                } else {
                    "http://".len()
                };
                let mut tail = &after[skip_len..];
                let end = tail
                    .find(|c: char| c.is_whitespace() || matches!(c, '<' | '"' | '\'' | ')' | ']'))
                    .unwrap_or(tail.len());
                tail = &tail[end..];
                rest = tail;
            }
        }
    }
    redacted
}

/// Juror-facing post body from stored fields (mirrors feed judicial quarantine + party redaction).
#[must_use]
pub fn juror_facing_post_body(
    body_html_sanitized: &str,
    body_markup: &str,
    body_plain: &str,
    party_label: &str,
    link_redacted: &str,
) -> String {
    if !body_html_sanitized.trim().is_empty() {
        juror_facing_body(
            &sanitize::sanitize_post_markup_judicial_quarantine(body_html_sanitized),
            "",
            party_label,
            link_redacted,
        )
    } else if !body_markup.trim().is_empty() {
        juror_facing_body(
            &sanitize::sanitize_post_markup_judicial_quarantine(body_markup),
            "",
            party_label,
            link_redacted,
        )
    } else {
        juror_facing_body("", body_plain, party_label, link_redacted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_at_handles_replaces_mentions() {
        let out = redact_at_handles("Hello @alice and @bob_x", "[party]");
        assert_eq!(out, "Hello [party] and [party]");
    }

    #[test]
    fn redact_profile_paths() {
        let out = redact_profile_path_mentions("see /home/profile/alice today", "[party]");
        assert_eq!(out, "see [party] today");
    }
}
