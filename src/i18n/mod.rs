//! Locale string catalogs: `locales/<locale>.json` with nested objects; leaves are strings.
//! Keys are flattened to dot paths (e.g. `error.unauthorized`). Use `{name}` placeholders in
//! values and [`tf`] to substitute.
//!
//! Per-request locale is set by the `negotiate_locale` HTTP middleware using `Accept-Language`
//! and read via [`request_locale`] (Tokio task-local). Outside that scope, [`request_locale`]
//! returns [`DEFAULT_LOCALE`].

use std::collections::HashMap;
use std::future::Future;
use std::path::Path;
use std::sync::OnceLock;

use serde_json::Value;

tokio::task_local! {
    static REQUEST_LOCALE: String;
}

pub const DEFAULT_LOCALE: &str = "en";

static CATALOG: OnceLock<Catalog> = OnceLock::new();

pub struct Catalog {
    default_locale: String,
    /// locale id → flat key → message
    locales: HashMap<String, HashMap<String, String>>,
}

impl Catalog {
    pub fn load_from_dir(dir: &Path) -> Result<Self, String> {
        let mut locales = HashMap::new();
        for entry in std::fs::read_dir(dir).map_err(|e| format!("read locales dir: {e}"))? {
            let entry = entry.map_err(|e| format!("locales entry: {e}"))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let locale = path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| format!("bad locale filename: {}", path.display()))?
                .to_string();
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("read {}: {e}", path.display()))?;
            let v: Value = serde_json::from_str(&text)
                .map_err(|e| format!("parse {}: {e}", path.display()))?;
            let mut map = HashMap::new();
            flatten_value("", &v, &mut map);
            locales.insert(locale, map);
        }
        if !locales.contains_key(DEFAULT_LOCALE) {
            return Err(format!(
                "missing required locale file {DEFAULT_LOCALE}.json under {}",
                dir.display()
            ));
        }
        Ok(Self {
            default_locale: DEFAULT_LOCALE.to_string(),
            locales,
        })
    }

    pub fn t(&self, locale: &str, key: &str) -> String {
        self.locales
            .get(locale)
            .and_then(|m| m.get(key).cloned())
            .or_else(|| {
                self.locales
                    .get(&self.default_locale)
                    .and_then(|m| m.get(key).cloned())
            })
            .unwrap_or_else(|| key.to_string())
    }

    pub fn tf(&self, locale: &str, key: &str, vars: &[(&str, &str)]) -> String {
        let mut s = self.t(locale, key);
        for (k, v) in vars {
            s = s.replace(&format!("{{{k}}}"), v);
        }
        s
    }

    /// Best match from `Accept-Language` against loaded locale files, else [`DEFAULT_LOCALE`].
    #[must_use]
    pub fn pick_locale(&self, accept_language: Option<&str>) -> String {
        let Some(raw) = accept_language.map(str::trim).filter(|s| !s.is_empty()) else {
            return self.default_locale.clone();
        };
        let prefs = parse_accept_language(raw);
        for (_q, tag) in prefs {
            if self.locales.contains_key(&tag) {
                return tag;
            }
            if let Some(primary) = tag.split('-').next().filter(|p| !p.is_empty()) {
                if self.locales.contains_key(primary) {
                    // Preserve region/script subtags for locale-sensitive formatting elsewhere.
                    // Translation lookup already falls back to DEFAULT_LOCALE when exact tag file is absent.
                    return tag;
                }
            }
        }
        self.default_locale.clone()
    }
}

/// RFC 7231-style `Accept-Language`: comma-separated ranges with optional `;q=` (0–1).
/// Entries with `q <= 0` are skipped. Sorted by descending quality.
fn parse_accept_language(value: &str) -> Vec<(f32, String)> {
    let mut out = Vec::new();
    for part in value.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (tag_raw, q) = if let Some((t, qpart)) = part.split_once(';') {
            let q = qpart
                .trim()
                .strip_prefix("q=")
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(1.0);
            (t.trim(), q)
        } else {
            (part, 1.0f32)
        };
        if tag_raw.is_empty() || tag_raw == "*" {
            continue;
        }
        if q <= 0.0 {
            continue;
        }
        out.push((q, tag_raw.to_ascii_lowercase()));
    }
    out.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    out
}

fn flatten_value(prefix: &str, v: &Value, out: &mut HashMap<String, String>) {
    match v {
        Value::Object(map) => {
            for (k, val) in map {
                let p = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                match val {
                    Value::String(s) => {
                        out.insert(p, s.clone());
                    }
                    Value::Object(_) => flatten_value(&p, val, out),
                    Value::Number(n) => {
                        out.insert(p, n.to_string());
                    }
                    Value::Bool(b) => {
                        out.insert(p, b.to_string());
                    }
                    Value::Null | Value::Array(_) => {}
                }
            }
        }
        Value::String(s) => {
            out.insert(prefix.to_string(), s.clone());
        }
        _ => {}
    }
}

/// First call loads all `locales/*.json` (requires `en.json`). Call once from `main` so bad files fail at startup.
#[must_use]
pub fn catalog() -> &'static Catalog {
    CATALOG.get_or_init(|| {
        let dir = crate::app_root::runtime_app_root().join("locales");
        Catalog::load_from_dir(&dir).unwrap_or_else(|e| panic!("i18n load failed: {e}"))
    })
}

/// Locale for the current request ([`crate::middleware::locale::negotiate_locale`]), else [`DEFAULT_LOCALE`].
#[must_use]
pub fn request_locale() -> String {
    REQUEST_LOCALE
        .try_with(|s| s.clone())
        .unwrap_or_else(|_| DEFAULT_LOCALE.to_string())
}

/// Run `future` with [`request_locale`] set to `locale` (typically per HTTP request).
pub async fn with_request_locale<F>(locale: String, future: F) -> F::Output
where
    F: Future,
{
    REQUEST_LOCALE.scope(locale, future).await
}

#[must_use]
pub fn t(key: &str) -> String {
    let loc = request_locale();
    catalog().t(&loc, key)
}

#[must_use]
pub fn tf(key: &str, vars: &[(&str, &str)]) -> String {
    let loc = request_locale();
    catalog().tf(&loc, key, vars)
}

/// Replace `@@dot.key@@` segments with [`Catalog::t`] for `locale` (e.g. HTML templates).
#[must_use]
pub fn subst_markers(html: &str, locale: &str) -> String {
    let c = catalog();
    let mut out = String::with_capacity(html.len() + 64);
    let mut i = 0usize;
    while i < html.len() {
        let tail = &html[i..];
        let Some(rel_open) = tail.find("@@") else {
            out.push_str(tail);
            break;
        };
        let open = i + rel_open;
        out.push_str(&html[i..open]);
        let after_open = open + 2;
        let tail2 = &html[after_open..];
        let Some(rel_close) = tail2.find("@@") else {
            out.push_str(&html[open..]);
            break;
        };
        let key = &html[after_open..after_open + rel_close];
        out.push_str(&c.t(locale, key));
        i = after_open + rel_close + 2;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_keys_and_placeholders() {
        let _ = catalog();
        assert_eq!(
            t("error.unauthorized"),
            "Login unsuccessful. Please check caps-lock and try again."
        );
        let s = tf("auth.password_too_short", &[("min_len", "12")]);
        assert!(s.contains("12"));
    }

    #[test]
    fn subst_markers_replaces_keys() {
        let _ = catalog();
        let s = subst_markers("a @@ui.shared.home@@ b", DEFAULT_LOCALE);
        assert_eq!(s, "a Home b");
    }

    #[test]
    fn pick_locale_from_accept_language() {
        let _ = catalog();
        let c = catalog();
        assert_eq!(c.pick_locale(None), DEFAULT_LOCALE.to_string());
        assert_eq!(c.pick_locale(Some("")), DEFAULT_LOCALE.to_string());
        assert_eq!(c.pick_locale(Some("en")), "en");
        assert_eq!(c.pick_locale(Some("en-US")), "en-us");
        assert_eq!(c.pick_locale(Some("fr")), "fr");
        assert_eq!(c.pick_locale(Some("fr-CA")), "fr-ca");
        assert_eq!(c.pick_locale(Some("de, en;q=0.9")), "en");
        assert_eq!(c.pick_locale(Some("fr;q=0, en")), "en");
    }

    #[test]
    fn parse_accept_language_orders_by_quality() {
        let v = parse_accept_language("en;q=0.5, fr;q=0.9, de");
        assert_eq!(v.len(), 3);
        assert_eq!(v[0].1, "de");
        assert_eq!(v[1].1, "fr");
        assert_eq!(v[2].1, "en");
    }
}
