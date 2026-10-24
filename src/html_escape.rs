//! HTML and inline-script escaping for dynamic values embedded in templates.

/// Escape text for HTML text nodes and double-quoted attributes.
#[must_use]
pub fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Prevent a closing `</script>` sequence inside JSON embedded in HTML.
#[must_use]
pub fn escape_json_for_html_script(s: &str) -> String {
    s.replace("</script", "<\\/script")
}
