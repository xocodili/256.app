//! Charter-keyed crisis contact registry (`config/crisis_resources.{locale}.json`).
//!
//! Structured phone numbers live here; human-readable labels live in `locales/*.json`
//! (`crisis.action.*`, `crisis.panel.*`). See [`docs/CRISIS_RESOURCES.md`](../docs/CRISIS_RESOURCES.md).

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::html_escape::escape_html;
use crate::i18n::{self, Catalog};

/// Charter points that may appear on judicial filing checkboxes (`data-clause-point`).
pub const FILING_CHARTER_POINTS: [u16; 7] = [401, 402, 403, 404, 405, 406, 407];

static REGISTRY: OnceLock<CrisisRegistry> = OnceLock::new();

#[derive(Debug, Clone, Deserialize)]
struct CrisisResourcesFile {
    version: u32,
    #[serde(default)]
    default_locale: String,
    points: HashMap<String, CrisisPointDef>,
}

#[derive(Debug, Clone, Deserialize)]
struct CrisisPointDef {
    charter_point: Option<u16>,
    #[serde(default)]
    severity: String,
    #[serde(default)]
    actions: Vec<CrisisActionDef>,
    #[serde(default)]
    notice_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CrisisActionDef {
    id: String,
    #[serde(default)]
    r#type: String,
    digits: String,
    #[serde(default)]
    regions: Vec<String>,
    label_key: String,
}

#[derive(Debug)]
pub struct CrisisRegistry {
    points: HashMap<String, CrisisPointDef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCrisisAction {
    pub id: String,
    pub label: String,
    pub tel_href: String,
    pub digits_display: String,
}

impl CrisisRegistry {
    pub fn load_from_path(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("read {}: {e}", path.display()))?;
        let file: CrisisResourcesFile =
            serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))?;
        if file.version != 1 {
            return Err(format!(
                "unsupported crisis_resources version {} in {}",
                file.version,
                path.display()
            ));
        }
        Ok(Self {
            points: file.points,
        })
    }

    fn point_key(charter_point: u16) -> String {
        charter_point.to_string()
    }

    fn actions_for_point_def(
        &self,
        def: &CrisisPointDef,
        country_code: Option<&str>,
        catalog: &Catalog,
        locale: &str,
    ) -> Vec<ResolvedCrisisAction> {
        let mut out = Vec::new();
        for action in &def.actions {
            if !action_matches_region(&action.regions, country_code) {
                continue;
            }
            if let Some(resolved) = resolve_action(action, catalog, locale) {
                out.push(resolved);
            }
        }
        out
    }

    /// Actions for one charter point (e.g. 406) plus always-merged `platform` actions (deduped by id).
    pub fn resolved_actions_for_charter_points(
        &self,
        charter_points: &[u16],
        country_code: Option<&str>,
        locale: &str,
        catalog: &Catalog,
    ) -> Vec<ResolvedCrisisAction> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();

        let mut push_point = |key: &str| {
            let Some(def) = self.points.get(key) else {
                return;
            };
            for action in self.actions_for_point_def(def, country_code, catalog, locale) {
                if seen.insert(action.id.clone()) {
                    out.push(action);
                }
            }
        };

        for &point in charter_points {
            push_point(&Self::point_key(point));
        }
        push_point("platform");

        out
    }

    /// Default safety surface: `platform` actions only.
    pub fn resolved_platform_actions(
        &self,
        country_code: Option<&str>,
        locale: &str,
        catalog: &Catalog,
    ) -> Vec<ResolvedCrisisAction> {
        self.resolved_actions_for_charter_points(&[], country_code, locale, catalog)
    }

    /// All `platform` dial actions (ignore region) — for guests and the global crisis-help page.
    pub fn resolved_platform_actions_unfiltered(
        &self,
        locale: &str,
        catalog: &Catalog,
    ) -> Vec<ResolvedCrisisAction> {
        let Some(def) = self.points.get("platform") else {
            return Vec::new();
        };
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for action in &def.actions {
            if let Some(resolved) = resolve_action(action, catalog, locale) {
                if seen.insert(resolved.id.clone()) {
                    out.push(resolved);
                }
            }
        }
        out
    }

    pub fn notice_for_point(
        &self,
        point_key: &str,
        locale: &str,
        catalog: &Catalog,
    ) -> Option<String> {
        let def = self.points.get(point_key)?;
        let key = def.notice_key.as_ref()?;
        let text = catalog.t(locale, key);
        if text == *key {
            return None;
        }
        Some(text)
    }
}

fn action_matches_region(regions: &[String], country_code: Option<&str>) -> bool {
    if regions.is_empty() {
        return true;
    }
    let Some(cc) = country_code.map(str::trim).filter(|s| !s.is_empty()) else {
        return false;
    };
    let cc = cc.to_ascii_uppercase();
    regions
        .iter()
        .any(|r| r.trim().eq_ignore_ascii_case(&cc))
}

fn resolve_action(
    action: &CrisisActionDef,
    catalog: &Catalog,
    locale: &str,
) -> Option<ResolvedCrisisAction> {
    let digits_clean: String = action
        .digits
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '+')
        .collect();
    if digits_clean.is_empty() {
        return None;
    }
    let label = catalog.t(locale, &action.label_key);
    let label = if label == action.label_key {
        action.label_key.clone()
    } else {
        label
    };
    Some(ResolvedCrisisAction {
        id: action.id.clone(),
        label,
        tel_href: format!("tel:{digits_clean}"),
        digits_display: action.digits.clone(),
    })
}

/// Load `config/crisis_resources.{locale}.json`, falling back to `.en.json`.
#[must_use]
pub fn registry() -> &'static CrisisRegistry {
    REGISTRY.get_or_init(|| {
        let root = crate::app_root::runtime_app_root();
        let config_dir = root.join("config");
        let locale = i18n::request_locale();
        let path = config_dir.join(format!("crisis_resources.{locale}.json"));
        let path = if path.is_file() {
            path
        } else {
            config_dir.join("crisis_resources.en.json")
        };
        CrisisRegistry::load_from_path(&path).unwrap_or_else(|e| {
            panic!("crisis_resources load failed ({}): {e}", path.display())
        })
    })
}

/// Eager load at process start (fail fast). Safe to call before request locale is set — uses `en`.
pub fn init_at_startup() {
    let root = crate::app_root::runtime_app_root();
    let path = root.join("config").join("crisis_resources.en.json");
    let reg = CrisisRegistry::load_from_path(&path).unwrap_or_else(|e| {
        panic!("crisis_resources startup load failed ({}): {e}", path.display())
    });
    let _ = REGISTRY.set(reg);
}

fn normalize_country_code(country_code: &str) -> String {
    country_code.trim().to_ascii_uppercase()
}

fn resolve_actions(
    reg: &CrisisRegistry,
    locale: &str,
    catalog: &Catalog,
    country_code: Option<&str>,
    charter_points: &[u16],
) -> Vec<ResolvedCrisisAction> {
    if charter_points.is_empty() {
        reg.resolved_platform_actions(country_code, locale, catalog)
    } else {
        reg.resolved_actions_for_charter_points(charter_points, country_code, locale, catalog)
    }
}

/// Render HTML panel for a signed-in viewer with a profile [`country_code`] (required at onboarding).
#[must_use]
pub fn panel_for_country(
    locale: &str,
    country_code: &str,
    charter_points: &[u16],
    include_point_16_notice: bool,
) -> String {
    let cc = normalize_country_code(country_code);
    render_panel_html(
        locale,
        Some(cc.as_str()),
        charter_points,
        include_point_16_notice,
    )
}

/// All platform hotlines (no region filter) — guests and anyone without a profile country.
#[must_use]
pub fn panel_universal(
    locale: &str,
    include_point_16_notice: bool,
) -> String {
    render_panel_html_with_actions(
        locale,
        &registry().resolved_platform_actions_unfiltered(locale, i18n::catalog()),
        include_point_16_notice,
    )
}

/// Generic panel for guests / unsecured users (no profile country).
#[must_use]
pub fn panel_for_guest(
    locale: &str,
    charter_points: &[u16],
    include_point_16_notice: bool,
) -> String {
    if charter_points.is_empty() {
        panel_universal(locale, include_point_16_notice)
    } else {
        render_panel_html(locale, None, charter_points, include_point_16_notice)
    }
}

/// Global crisis-help surface: profile country when known, else universal list.
#[must_use]
pub fn panel_for_everyone(
    locale: &str,
    profile_country_code: Option<&str>,
    include_point_16_notice: bool,
) -> String {
    match profile_country_code.map(str::trim).filter(|s| !s.is_empty()) {
        Some(cc) => panel_for_country(locale, cc, &[], include_point_16_notice),
        None => panel_universal(locale, include_point_16_notice),
    }
}

/// Signed-in surfaces: use profile country when present; otherwise guest fallback.
#[must_use]
pub fn panel_for_viewer(
    locale: &str,
    profile_country_code: Option<&str>,
    charter_points: &[u16],
    include_point_16_notice: bool,
) -> String {
    match profile_country_code.map(str::trim).filter(|s| !s.is_empty()) {
        Some(cc) => panel_for_country(locale, cc, charter_points, include_point_16_notice),
        None => {
            tracing::warn!("crisis panel: signed-in viewer missing country_code; using guest panel");
            panel_for_guest(locale, charter_points, include_point_16_notice)
        }
    }
}

/// Back-compat alias: `Some(country)` uses profile country only; `None` is guest/generic.
#[must_use]
pub fn panel_html(
    locale: &str,
    country_code: Option<&str>,
    charter_points: &[u16],
    include_point_16_notice: bool,
) -> String {
    match country_code.map(str::trim).filter(|s| !s.is_empty()) {
        Some(cc) => panel_for_country(locale, cc, charter_points, include_point_16_notice),
        None => panel_for_guest(locale, charter_points, include_point_16_notice),
    }
}

fn render_panel_html(
    locale: &str,
    country_code: Option<&str>,
    charter_points: &[u16],
    include_point_16_notice: bool,
) -> String {
    let catalog = i18n::catalog();
    let reg = registry();
    let actions = resolve_actions(reg, locale, catalog, country_code, charter_points);
    render_panel_html_with_actions(locale, &actions, include_point_16_notice)
}

fn render_panel_html_with_actions(
    locale: &str,
    actions: &[ResolvedCrisisAction],
    include_point_16_notice: bool,
) -> String {
    let catalog = i18n::catalog();
    let reg = registry();

    let mut parts = Vec::new();
    parts.push(format!(
        r#"<div class="crisis-resources-panel" role="complementary" aria-label="{}">"#,
        escape_html(&catalog.t(locale, "crisis.panel.title"))
    ));
    parts.push(format!(
        r#"<p class="crisis-resources-panel-title">{}</p>"#,
        escape_html(&catalog.t(locale, "crisis.panel.title"))
    ));

    if include_point_16_notice {
        if let Some(notice) = reg.notice_for_point("16", locale, catalog) {
            parts.push(format!(
                r#"<p class="crisis-resources-panel-notice">{}</p>"#,
                escape_html(&notice)
            ));
        }
    }

    if actions.is_empty() {
        parts.push(format!(
            r#"<p class="crisis-resources-panel-fallback">{}</p>"#,
            escape_html(&catalog.t(locale, "crisis.panel.fallback_no_numbers"))
        ));
    } else {
        parts.push(r#"<ul class="crisis-resources-panel-list">"#.to_string());
        for action in actions {
            parts.push(format!(
                r#"<li><a href="{href}" rel="noopener noreferrer">{label} <span class="mono">({digits})</span></a></li>"#,
                href = escape_html(&action.tel_href),
                label = escape_html(&action.label),
                digits = escape_html(&action.digits_display),
            ));
        }
        parts.push("</ul>".to_string());
    }

    parts.push("</div>".to_string());
    parts.join("")
}

#[derive(Debug, Clone, Serialize)]
struct ClientCrisisAction {
    id: String,
    label: String,
    tel_href: String,
    digits_display: String,
}

impl From<ResolvedCrisisAction> for ClientCrisisAction {
    fn from(a: ResolvedCrisisAction) -> Self {
        Self {
            id: a.id,
            label: a.label,
            tel_href: a.tel_href,
            digits_display: a.digits_display,
        }
    }
}

#[derive(Debug, Serialize)]
struct FilingCrisisBundle {
    title: String,
    notice16: Option<String>,
    fallback: String,
    by_point: HashMap<String, Vec<ClientCrisisAction>>,
    platform: Vec<ClientCrisisAction>,
}

/// JSON for judicial filing UI: clause checkboxes update the crisis panel client-side.
#[must_use]
pub fn filing_crisis_bundle_json(locale: &str, country_code: &str) -> String {
    let catalog = i18n::catalog();
    let reg = registry();
    let cc = normalize_country_code(country_code);
    let cc_ref = Some(cc.as_str());

    let mut by_point = HashMap::new();
    for &point in &FILING_CHARTER_POINTS {
        let actions = reg.resolved_actions_for_charter_points(&[point], cc_ref, locale, catalog);
        by_point.insert(
            point.to_string(),
            actions.into_iter().map(ClientCrisisAction::from).collect(),
        );
    }
    let platform = reg
        .resolved_platform_actions(cc_ref, locale, catalog)
        .into_iter()
        .map(ClientCrisisAction::from)
        .collect();

    let bundle = FilingCrisisBundle {
        title: catalog.t(locale, "crisis.panel.title"),
        notice16: reg.notice_for_point("16", locale, catalog),
        fallback: catalog.t(locale, "crisis.panel.fallback_no_numbers"),
        by_point,
        platform,
    };
    serde_json::to_string(&bundle).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_registry() -> CrisisRegistry {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("config/crisis_resources.en.json");
        CrisisRegistry::load_from_path(&path).expect("test crisis_resources.en.json")
    }

    #[test]
    fn point_406_returns_988_for_us() {
        let reg = test_registry();
        let catalog = i18n::catalog();
        let actions = reg.resolved_actions_for_charter_points(&[406], Some("US"), "en", catalog);
        assert!(actions.iter().any(|a| a.digits_display == "988"));
    }

    #[test]
    fn platform_dedupes_with_point() {
        let reg = test_registry();
        let catalog = i18n::catalog();
        let actions = reg.resolved_actions_for_charter_points(&[406], Some("US"), "en", catalog);
        let count_988 = actions.iter().filter(|a| a.id == "suicide_lifeline_us").count();
        assert_eq!(count_988, 1);
    }

    #[test]
    fn unknown_country_gets_fallback_only_when_no_global_actions() {
        let reg = test_registry();
        let catalog = i18n::catalog();
        let actions = reg.resolved_platform_actions(Some("ZZ"), "en", catalog);
        assert!(actions.is_empty());
    }

    #[test]
    fn universal_panel_lists_platform_hotlines() {
        let html = panel_universal("en", false);
        assert!(html.contains("tel:988") || html.contains("tel:911") || html.contains("tel:112"));
    }

    #[test]
    fn guest_panel_uses_universal_hotlines() {
        let html = panel_for_guest("en", &[], true);
        assert!(!html.contains("crisis-resources-panel-fallback"));
    }

    #[test]
    fn country_panel_uses_profile_not_locale() {
        let html = panel_for_country("en-GB", "US", &[406], false);
        assert!(html.contains("988"));
    }
}
