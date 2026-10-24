//! Branded HTML shell for outbound mail — table layout, inline styles, site palette.
//!
//! Plain-text bodies remain the accessibility fallback; [`super::transport`] sends
//! multipart alternative (plain + HTML).

use crate::html_escape::escape_html;
use crate::i18n::Catalog;

/// Brand tokens mirrored from `static/css/brand.css` (email clients ignore external CSS).
const BG: &str = "#000000";
const SURFACE: &str = "#12121a";
const TEXT: &str = "#f6f6fa";
const MUTED: &str = "#c4c4d4";
const YELLOW: &str = "#f4c430";
const FONT: &str = "Lexend, Atkinson Hyperlegible, Verdana, Geneva, Arial, sans-serif";
/// PNG mark for `<img>` in HTML mail — SVG is blocked by most clients (Gmail, Outlook).
pub const EMAIL_LOGO_PATH: &str = "/static/branding/email-logo.png";
/// Outer canvas — mirrors `body` + `.app-shell--guest` radial stacks in brand.css.
const BG_GRADIENT: &str = "radial-gradient(ellipse 90% 55% at 50% -5%, rgba(65,105,225,0.22), transparent 68%), radial-gradient(ellipse 55% 45% at 12% 88%, rgba(79,195,247,0.14), transparent 72%), radial-gradient(ellipse 50% 40% at 88% 78%, rgba(123,30,61,0.12), transparent 70%), radial-gradient(ellipse 100% 80% at 50% -30%, #1e1432 0%, #000000 45%), #000000";

/// Content placed inside the branded frame (heading, body copy, optional note, CTA).
#[derive(Debug, Clone, Default)]
pub struct EmailFrameInput {
    pub type_heading: String,
    /// Plain-text paragraphs — escaped before insertion.
    pub paragraphs: Vec<String>,
    /// Pre-rendered HTML paragraphs — used instead of `paragraphs` when set.
    /// Each string must already be safe HTML (no further escaping applied).
    pub html_paragraphs: Option<Vec<String>>,
    pub note: Option<String>,
    pub cta_label: String,
    pub cta_url: String,
}

#[must_use]
pub fn render_html(
    input: &EmailFrameInput,
    public_base_url: &str,
    loc: &str,
    c: &Catalog,
) -> String {
    let base = public_base_url.trim_end_matches('/');
    let site_url = escape_html(base);
    let logo_url = escape_html(&format!("{base}{EMAIL_LOGO_PATH}"));
    let tagline = escape_html(&c.t(loc, "email.layout.tagline"));
    let heading = escape_html(&input.type_heading);
    let cta_label = escape_html(&input.cta_label);
    let cta_url = escape_html(&input.cta_url);

    let body_paragraphs = if let Some(html_paras) = &input.html_paragraphs {
        html_paras
            .iter()
            .map(|p| {
                format!(r#"<p style="margin:0 0 1rem;font-size:15px;line-height:1.55;color:{TEXT};">{p}</p>"#)
            })
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        input
            .paragraphs
            .iter()
            .map(|p| {
                format!(
                    r#"<p style="margin:0 0 1rem;font-size:15px;line-height:1.55;color:{TEXT};">{html}</p>"#,
                    html = escape_html(p)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let note_block = input
        .note
        .as_ref()
        .filter(|n| !n.trim().is_empty())
        .map(|n| {
            format!(
                r#"<p style="margin:0 0 1.25rem;font-size:14px;line-height:1.5;color:{MUTED};font-style:italic;">{html}</p>"#,
                html = escape_html(n.trim())
            )
        })
        .unwrap_or_default();

    let terms_url = format!("{base}/login?legal=terms");
    let privacy_url = format!("{base}/login?legal=privacy");
    let safety_url = format!("{base}/login?legal=safety");
    let how_url = format!("{base}/login?legal=how");
    let footer_terms = escape_html(&c.t(loc, "email.layout.footer_terms"));
    let footer_privacy = escape_html(&c.t(loc, "email.layout.footer_privacy"));
    let footer_safety = escape_html(&c.t(loc, "email.layout.footer_safety"));
    let footer_how = escape_html(&c.t(loc, "email.layout.footer_how"));

    format!(
        r#"<!DOCTYPE html>
<html lang="{lang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="color-scheme" content="dark">
<title>{heading}</title>
</head>
<body style="margin:0;padding:0;background-color:{BG};background-image:{BG_GRADIENT};font-family:{FONT};">
<table role="presentation" width="100%" cellspacing="0" cellpadding="0" bgcolor="{BG}" style="background-color:{BG};background-image:{BG_GRADIENT};">
<tr><td align="center" bgcolor="{BG}" style="padding:28px 16px;background-color:{BG};background-image:{BG_GRADIENT};">
<table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="max-width:560px;border:2px solid {YELLOW};background:{SURFACE};border-radius:10px;">
<tr><td style="padding:28px 28px 12px;text-align:center;">
<a href="{site_url}" style="text-decoration:none;display:inline-block;">
<img src="{logo_url}" alt="256.app" width="64" height="64" style="display:block;margin:0 auto 10px;border-radius:50%;border:2px solid {YELLOW};background:{SURFACE};" />
<span style="font-family:{FONT};font-size:32px;font-weight:700;letter-spacing:-0.02em;color:{YELLOW};display:block;">256.app</span>
<span style="font-family:{FONT};font-size:13px;font-weight:400;color:{MUTED};display:block;margin-top:4px;">{tagline}</span>
</a>
</td></tr>
<tr><td style="padding:0 24px 24px;">
<table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="background:{BG};border-radius:8px;">
<tr><td style="padding:28px 24px;">
<h1 style="margin:0 0 1.25rem;font-family:{FONT};font-size:20px;font-weight:600;line-height:1.35;color:{YELLOW};">{heading}</h1>
{body_paragraphs}
{note_block}
<table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="margin:1.5rem 0 0;">
<tr><td style="border-radius:14px;border:1px solid {YELLOW};text-align:center;box-shadow:0 3px 12px rgba(0,0,0,0.42);">
<a href="{cta_url}" style="display:block;width:100%;box-sizing:border-box;padding:14px 32px;font-family:{FONT};font-size:16px;font-weight:700;letter-spacing:0.04em;color:{YELLOW};text-decoration:none;background:{BG};border-radius:14px;">{cta_label}</a>
</td></tr>
</table>
</td></tr>
</table>
</td></tr>
<tr><td style="padding:0 24px 28px;text-align:center;font-size:12px;line-height:1.6;color:{MUTED};">
<a href="{terms_url}" style="color:{MUTED};text-decoration:underline;">{footer_terms}</a>
&nbsp;&middot;&nbsp;
<a href="{privacy_url}" style="color:{MUTED};text-decoration:underline;">{footer_privacy}</a>
&nbsp;&middot;&nbsp;
<a href="{safety_url}" style="color:{MUTED};text-decoration:underline;">{footer_safety}</a>
&nbsp;&middot;&nbsp;
<a href="{how_url}" style="color:{MUTED};text-decoration:underline;">{footer_how}</a>
</td></tr>
</table>
</td></tr>
</table>
</body>
</html>"#,
        lang = escape_html(loc),
        heading = heading,
        site_url = site_url,
        logo_url = logo_url,
        tagline = tagline,
        body_paragraphs = body_paragraphs,
        note_block = note_block,
        cta_label = cta_label,
        cta_url = cta_url,
        terms_url = escape_html(&terms_url),
        privacy_url = escape_html(&privacy_url),
        safety_url = escape_html(&safety_url),
        how_url = escape_html(&how_url),
        footer_terms = footer_terms,
        footer_privacy = footer_privacy,
        footer_safety = footer_safety,
        footer_how = footer_how,
        BG = BG,
        BG_GRADIENT = BG_GRADIENT,
        SURFACE = SURFACE,
        MUTED = MUTED,
        YELLOW = YELLOW,
        FONT = FONT,
    )
}

/// Split plain body into paragraphs, dropping lines that only repeat the action URL.
#[must_use]
pub fn paragraphs_from_body(body: &str, action_url: &str) -> Vec<String> {
    let url = action_url.trim();
    body.split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .filter(|p| *p != url)
        .filter(|p| !is_url_only_paragraph(p, url))
        .map(|p| p.replace(url, "").trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

fn is_url_only_paragraph(paragraph: &str, url: &str) -> bool {
    if paragraph == url {
        return true;
    }
    let lower = paragraph.to_ascii_lowercase();
    (lower.starts_with("http://") || lower.starts_with("https://"))
        && paragraph.trim() == url
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_bare_url_paragraphs() {
        let body = "Welcome.\n\nhttps://example.com/verify\n\nFooter line.";
        let paras = paragraphs_from_body(body, "https://example.com/verify");
        assert_eq!(paras, vec!["Welcome.", "Footer line."]);
    }

    #[test]
    fn outer_shell_uses_guest_page_gradient() {
        use crate::i18n;
        let frame = EmailFrameInput {
            type_heading: "Test".into(),
            paragraphs: vec!["Hello.".into()],
            html_paragraphs: None,
            note: None,
            cta_label: "Go".into(),
            cta_url: "https://256.app/login".into(),
        };
        let html = render_html(
            &frame,
            "https://256.app",
            "en",
            i18n::catalog(),
        );
        assert!(html.contains(BG_GRADIENT));
        assert!(html.contains("radial-gradient(ellipse 90% 55%"));
        assert!(html.contains(EMAIL_LOGO_PATH));
        assert!(!html.contains("favicon.svg"));
    }
}
