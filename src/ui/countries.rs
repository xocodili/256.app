//! ISO 3166-1 alpha-2 list for HTML `<select>` (data from `data/iso3166_alpha2_en.tsv`).

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::html_escape::escape_html;

const TSV: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/data/iso3166_alpha2_en.tsv"
));

static NAME_BY_CODE: OnceLock<HashMap<String, String>> = OnceLock::new();

fn name_map() -> &'static HashMap<String, String> {
    NAME_BY_CODE.get_or_init(|| {
        let mut m = HashMap::new();
        for line in TSV.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let mut parts = line.splitn(2, '\t');
            let code = parts.next().unwrap_or("").trim();
            let name = parts.next().unwrap_or("").trim();
            if code.len() == 2 {
                m.insert(code.to_ascii_uppercase(), name.to_string());
            }
        }
        m
    })
}

/// Reserved code for the platform global location (see seed `locations` row `ZZ` / display `Global`).
pub const GLOBAL_COUNTRY_CODE: &str = "ZZ";

/// English place-name label for a code (e.g. `AU` → `Australia`).
#[must_use]
pub fn display_name_for_code(code: &str) -> Option<String> {
    let u = code.trim().to_ascii_uppercase();
    if u == GLOBAL_COUNTRY_CODE {
        return Some("Global".to_string());
    }
    if u.len() != 2 {
        return None;
    }
    name_map().get(&u).cloned()
}

/// Country `<select>` rows for community create: **Global** first, then ISO countries.
#[must_use]
pub fn community_country_option_rows_html(selected_code: Option<&str>) -> String {
    let selected = selected_code
        .map(|c| c.trim().to_ascii_uppercase())
        .filter(|c| !c.is_empty());
    let global_selected = selected.as_deref() == Some(GLOBAL_COUNTRY_CODE);
    let mut out = String::with_capacity(4096);
    push_option_row(&mut out, GLOBAL_COUNTRY_CODE, "Global", global_selected.then_some(true));
    out.push_str(&option_rows_html_with_selected(
        if global_selected {
            None
        } else {
            selected_code
        },
    ));
    out
}

/// `<option value="CODE">…</option>` rows, sorted by English place name, names HTML-escaped.
#[must_use]
pub fn option_rows_html() -> String {
    let mut pairs: Vec<(&str, &str)> = Vec::new();
    for line in TSV.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(2, '\t');
        let code = parts.next().unwrap_or("").trim();
        let name = parts.next().unwrap_or("").trim();
        if code.len() == 2 {
            pairs.push((code, name));
        }
    }
    pairs.sort_by(|a, b| a.1.cmp(b.1));

    let mut out = String::with_capacity(pairs.len() * 48);
    for (code, name) in pairs {
        push_option_row(&mut out, code, name, None);
    }
    out
}

/// Like [`option_rows_html`], with one ISO code marked `selected` when it matches.
#[must_use]
pub fn option_rows_html_with_selected(selected_code: Option<&str>) -> String {
    let selected = selected_code.map(|c| c.trim().to_ascii_uppercase());
    let mut pairs: Vec<(&str, &str)> = Vec::new();
    for line in TSV.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(2, '\t');
        let code = parts.next().unwrap_or("").trim();
        let name = parts.next().unwrap_or("").trim();
        if code.len() == 2 {
            pairs.push((code, name));
        }
    }
    pairs.sort_by(|a, b| a.1.cmp(b.1));

    let mut out = String::with_capacity(pairs.len() * 56);
    for (code, name) in pairs {
        let sel = selected
            .as_deref()
            .is_some_and(|s| s == code.to_ascii_uppercase().as_str());
        push_option_row(&mut out, code, name, sel.then_some(true));
    }
    out
}

fn push_option_row(out: &mut String, code: &str, name: &str, selected: Option<bool>) {
    out.push_str(r#"<option value=""#);
    out.push_str(code);
    out.push('"');
    if selected.is_some() {
        out.push_str(" selected");
    }
    out.push('>');
    out.push_str(&escape_html(name));
    out.push_str("</option>");
}
