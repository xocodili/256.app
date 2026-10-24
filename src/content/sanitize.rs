//! HTML sanitization policy for user-authored rich text.
//!
//! This sanitizer keeps the formatting primitives used by the create-post composer
//! while stripping active content and unsafe attributes.

/// Sanitize post markup for safe public rendering.
#[must_use]
pub fn sanitize_post_markup(raw: &str) -> String {
    let mut builder = ammonia::Builder::default();
    // Keep composer formatting output.
    builder.add_tags(["u", "span", "font"]);
    // Preserve link behavior and legacy execCommand color output.
    builder.add_tag_attributes("a", ["target"]);
    builder.add_tag_attributes("font", ["color"]);
    // Normalize link rel for outbound safety when links are present.
    builder.link_rel(Some("noopener noreferrer nofollow"));
    builder.clean(raw).to_string()
}

/// Same tag/attribute policy as [`sanitize_post_markup`], but `<a>` tags are stripped while
/// keeping their text content so nothing in the fragment navigates externally (used for posts
/// under judicial review).
#[must_use]
pub fn sanitize_post_markup_judicial_quarantine(raw: &str) -> String {
    let mut builder = ammonia::Builder::default();
    builder.add_tags(["u", "span", "font"]);
    builder.add_tag_attributes("font", ["color"]);
    builder.rm_tags(["a"]);
    builder.clean(raw).to_string()
}
