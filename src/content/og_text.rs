//! Normalize Open Graph text fields before persistence (entity decode, tag strip).

use super::og_fetch::text_looks_like_error_page;

/// Decode HTML entities, strip tags, collapse whitespace, trim.
#[must_use]
pub fn normalize_og_field(s: &str) -> String {
    let decoded = decode_html_entities(s);
    let stripped = strip_html_tags(&decoded);
    let ascii = normalize_typographic_punctuation(&stripped);
    collapse_whitespace(&ascii)
}

/// Normalize optional OG field; returns None if empty or error-page-like after normalize.
#[must_use]
pub fn normalize_og_field_optional(raw: Option<&str>) -> Option<String> {
    let s = raw?.trim();
    if s.is_empty() {
        return None;
    }
    let normalized = normalize_og_field(s);
    if normalized.is_empty() || text_looks_like_error_page(&normalized) {
        None
    } else {
        Some(normalized)
    }
}

fn collapse_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_space && !out.is_empty() {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

fn strip_html_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

fn decode_html_entities(s: &str) -> String {
    let mut current = s.to_string();
    for _ in 0..4 {
        let next = decode_html_entities_once(&current);
        if next == current {
            break;
        }
        current = next;
    }
    current
}

fn decode_html_entities_once(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '&' {
            let mut entity = String::new();
            let mut found_semi = false;
            while let Some(&next) = chars.peek() {
                if next == ';' {
                    chars.next();
                    found_semi = true;
                    break;
                }
                if next == '&' || entity.len() > 16 || next.is_whitespace() {
                    break;
                }
                entity.push(chars.next().unwrap());
            }
            if found_semi {
                if let Some(decoded) = decode_entity(&entity) {
                    out.push_str(&decoded);
                    continue;
                }
            }
            out.push('&');
            for c in entity.chars() {
                out.push(c);
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn normalize_typographic_punctuation(s: &str) -> String {
    s.replace('\u{2019}', "'")
        .replace('\u{2018}', "'")
        .replace('\u{201C}', "\"")
        .replace('\u{201D}', "\"")
}

fn decode_entity(entity: &str) -> Option<String> {
    let key = entity.trim_end_matches(';');
    match key {
        "amp" | "#38" | "#x26" => Some("&".into()),
        "lt" | "#60" | "#x3c" => Some("<".into()),
        "gt" | "#62" | "#x3e" => Some(">".into()),
        "quot" | "#34" | "#x22" => Some("\"".into()),
        "apos" | "#39" | "#x27" => Some("'".into()),
        "nbsp" | "#160" | "#xA0" | "#xa0" => Some("\u{00A0}".into()),
        _ if key.starts_with('#') => decode_numeric_entity(key),
        _ => NAMED_ENTITIES
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, ch)| ch.to_string()),
    }
}

fn decode_numeric_entity(key: &str) -> Option<String> {
    let digits = key.trim_start_matches('#');
    let (base, num_str) = if digits.starts_with('x') || digits.starts_with('X') {
        (16, &digits[1..])
    } else {
        (10, digits)
    };
    let code = u32::from_str_radix(num_str, base).ok()?;
    char::from_u32(code).map(|c| c.to_string())
}

const NAMED_ENTITIES: &[(&str, char)] = &[
    ("rsquo", '\u{2019}'),
    ("lsquo", '\u{2018}'),
    ("rdquo", '\u{201D}'),
    ("ldquo", '\u{201C}'),
    ("mdash", '\u{2014}'),
    ("ndash", '\u{2013}'),
    ("hellip", '\u{2026}'),
    ("copy", '\u{00A9}'),
    ("reg", '\u{00AE}'),
    ("trade", '\u{2122}'),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_amp_entity() {
        assert_eq!(normalize_og_field("Tom &amp; Jerry"), "Tom & Jerry");
    }

    #[test]
    fn decodes_numeric_apostrophe() {
        assert_eq!(normalize_og_field("It&#8217;s fine"), "It's fine");
    }

    #[test]
    fn strips_html_tags() {
        assert_eq!(normalize_og_field("<p>Hi</p>"), "Hi");
    }

    #[test]
    fn collapses_whitespace() {
        assert_eq!(normalize_og_field("  Hello   world  "), "Hello world");
    }

    #[test]
    fn double_encoded_amp() {
        assert_eq!(normalize_og_field("A &amp;amp; B"), "A & B");
    }

    #[test]
    fn rejects_error_page_after_normalize() {
        assert!(normalize_og_field_optional(Some("404 Not Found")).is_none());
    }
}
