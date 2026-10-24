//! Normalize stored avatar values for HTML `img src` (relative paths vs absolute URLs).

#[must_use]
pub fn img_src_from_stored_avatar(stored: &str) -> String {
    let t = stored.trim();
    if t.starts_with("https://") || t.starts_with("http://") {
        return t.to_string();
    }
    if t.starts_with('/') {
        return t.to_string();
    }
    format!("/{}", t.trim_start_matches('/'))
}
