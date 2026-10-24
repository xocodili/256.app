# Crisis resources — design (charter-keyed contacts + locales)

How to surface **user-facing** emergency numbers and **operator-facing** reporting duties without fighting the i18n system. Product context: **link-and-text-only** site — crisis copy attaches to **charter clauses** and filing surfaces, not to media uploads. See [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) § Content model, [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md).

**Status:** **Shipped** — `config/crisis_resources.en.json`, `src/crisis_resources.rs`, clause-aware judicial filing panel (JS), emergency contest page, emergency hold feed cards, **Safety & Crisis** legal card (`GET /login/legal-card?doc=safety`), optional `GET /crisis-help`. Guest/secured footers: `src/ui/snippets.rs` (`__PUBLIC_FOOTER__` / `inject_public_footer`). Legacy `?legal=crisis` aliases `safety`.

**Last reviewed:** 2026-06-03 (integrated in Safety & Crisis card and safety-adjacent surfaces; footer no longer lists a separate Crisis help link).

---

## Why this doc exists (the locale blocker)

You considered a JSON file keyed by charter / constitutional items with lists of phone numbers. That is the right **data** shape, but it collides with how **`locales/`** works today unless you split responsibilities deliberately.

### What `locales/*.json` is today

[`src/i18n/mod.rs`](../src/i18n/mod.rs) loads **every** `locales/*.json` file as a **translation catalog**:

1. Each file stem becomes a locale id (`en`, `fr`, …).
2. The JSON tree is **flattened** to dot keys (`ui.login.title` → string).
3. **Leaves must be strings** — there is no first-class support for arrays of objects (phone list entries) in the catalog API. `Catalog::t` / `tf` return a single string per key.

So if you put this in `locales/en.json`:

```json
"crisis": {
  "401": {
    "numbers": [{ "digits": "911", "regions": ["US"] }]
  }
}
```

…it **will not work** as structured data — flattening would produce unusable keys or fail type expectations. Putting **only** human-readable sentences under `crisis.*` **does** work, but then **every region’s numbers are duplicated per locale file** and you cannot reuse one canonical contact list across languages without copy-paste.

**That is the thought-process blocker:** `locales/` feels like the natural place for “all user-visible JSON,” but it is optimized for **translatable prose**, not **structured crisis registries**.

---

## Recommended split (data vs copy)

| Layer | Location | Contains | Loaded by |
|-------|----------|----------|-----------|
| **Contact registry** | `config/crisis_resources.json` (or `config/crisis_resources.en.json` per locale) | Charter point → list of `{ type, digits, regions, url? }` | New `crisis_resources` module at startup (like catalog, separate `OnceLock`) |
| **UI copy** | `locales/en.json` (and `fr.json` when translated) | Labels, disclaimers, button text, “call local emergency services” | Existing `i18n::catalog()` |
| **Operator reporting** | `.env` + runbook — **not** this JSON | CyberTipline / INHOPE API credentials, counsel workflow | Ops only |

Users see: **registry picks numbers** + **locale strings explain what to do**. Operators see: charter point 16 obligations in [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) — automated hotline **submission** is not the same as showing **988** in the UI.

---

## Do **not** put the registry in `locales/`

| Approach | Problem |
|----------|---------|
| `locales/crisis_en.json` | Treated as a **locale** named `crisis_en` — breaks `Accept-Language` and `pick_locale` |
| Nested arrays in `en.json` | Catalog flattener only stores **string leaves** |
| DB table for numbers | Overkill for beta; harder to review in git; you said you prefer file-based |

**Do** use a dedicated directory:

```text
config/
  crisis_resources.schema.json   # optional JSON Schema for editors
  crisis_resources.en.json     # default registry (region tags inside entries)
  crisis_resources.fr.json     # future: same keys, French labels via locales still
```

Or a single `config/crisis_resources.json` with **locale-agnostic** `regions` and let `en.json` supply translated **labels** via keys like `crisis.action.call_emergency`.

---

## Proposed registry shape (charter-keyed)

Keys match **Root charter points** used in filing UI (401–407, plus meta keys for platform-wide crisis).

```json
{
  "version": 1,
  "default_locale": "en",
  "points": {
    "401": {
      "charter_point": 401,
      "severity": "emergency",
      "actions": [
        {
          "id": "local_emergency",
          "type": "voice",
          "digits": "911",
          "regions": ["US"],
          "label_key": "crisis.action.local_emergency"
        }
      ]
    },
    "406": {
      "charter_point": 406,
      "severity": "crisis",
      "actions": [
        {
          "id": "suicide_lifeline_us",
          "type": "voice",
          "digits": "988",
          "regions": ["US"],
          "label_key": "crisis.action.suicide_lifeline"
        }
      ]
    },
    "16": {
      "charter_point": 16,
      "severity": "informational",
      "actions": [],
      "notice_key": "crisis.notice.operator_reports_not_user"
    },
    "platform": {
      "charter_point": null,
      "severity": "crisis",
      "actions": [
        {
          "id": "suicide_lifeline_us",
          "type": "voice",
          "digits": "988",
          "regions": ["US"],
          "label_key": "crisis.action.suicide_lifeline"
        }
      ]
    }
  }
}
```

**Lookup rules (implementation sketch):**

1. User files emergency citing clause **406** → show `points["406"].actions` filtered by **profile `country_code`** (required at onboarding for signed-in users). Judicial filing updates the panel client-side when §4.x checkboxes change (`filing_crisis_bundle_json`).
2. Always append `platform` actions on emergency filing card and hold notice (self-harm is not only one sub-clause).
3. Point **16** → show `notice_key` only (no user dial list for NCMEC — that is operator duty).

---

## Locale keys to add in `en.json` (copy only)

Keep **words** in locales; keep **digits** in config (digits are often locale-invariant; EU uses 112, etc.).

| Key | Example EN value |
|-----|------------------|
| `crisis.action.local_emergency` | Call local emergency services |
| `crisis.action.suicide_lifeline` | Suicide & Crisis Lifeline (US) |
| `crisis.notice.operator_reports_not_user` | 256.app does not file police reports for you. In immediate danger, call local emergency services. |
| `crisis.panel.title` | If you are in immediate danger |
| `crisis.emergency_hold.hint` | This post is hidden while the emergency case is uncontested. You may contest within the window shown in your email. |

Templates call something like:

```rust
crisis_resources::actions_for_charter_point(locale, point, user_region)
// → Vec<{ label: String, tel_href: String }>
```

Render `<a href="tel:…">` from `digits` + localized `label_key`.

---

## Where to render (product surfaces)

| Surface | Registry keys | Locale keys |
|---------|---------------|-------------|
| `templates/judicial_post_case.html` | Selected §4.x point + `platform` (clause-aware JS) | Panel title, disclaimers |
| Emergency **contest** page | `platform` | Same as hold |
| Emergency **hold notice** on feed card | `platform` | `ui.post_emergency.*` + crisis panel |
| `/login?legal=safety` card (**Safety & Crisis**) | Profile country if session; else universal hotlines | `ui.legal.safety_body` + `panel_for_everyone` |
| Emergency contest page | `platform` | Same panel as hold |
| Feed emergency-hold cards | `platform` | `panel_for_viewer` |
| `GET /crisis-help` (optional bookmark) | Profile country if signed in; else universal | Dedicated page; same registry |
| Secured + guest footers | — | **Terms & Conditions**, **Privacy**, **Safety & Crisis** (no separate Crisis help link — hotlines live on Safety & Crisis) |
| Juror workspace | `platform` on safety-adjacent copy | `ui.juror_workspace.law_notice` (already honest) |

**Not** on every page — only **safety-adjacent** surfaces to avoid noise.

---

## Operator hotline (point 16) — separate from this JSON

| Concern | Mechanism |
|---------|-----------|
| User sees crisis numbers | `config/crisis_resources.*.json` + `locales/*.json` |
| Operator reports CSAM to NCMEC | Env + Panoply playbook + `safety_sanitation_event` when hash gate fires — **no user-facing phone list** |
| Legal preservation | Existing audit tables + SQL runbook in [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) |

Charter point **16** in the registry should be **`actions: []`** plus an informational `notice_key` so you do not imply users file CyberTipline reports.

---

## Region selection

1. **Signed-in:** filter by human profile **`country_code`** (required at sign-up/onboarding) on Safety & Crisis surfaces, judicial filing panels, and emergency hold cards. No `Accept-Language` inference.
2. **Guests / unsigned:** universal platform hotlines via `panel_universal` / `panel_for_guest` on the Safety & Crisis legal card and `/crisis-help`.
3. **Footer:** **Safety & Crisis** opens `/login?legal=safety` (crisis panel embedded in the card). `/crisis-help` remains for direct links and bookmarks. **`?legal=crisis`** is accepted as an alias for `safety` (no separate crisis-only legal card).
4. If a signed-in profile lacks `country_code`, log a warning and fall back to the universal list until data is repaired.

Do not block ship on geo-IP.

---

## French / partial locales

[`locales/fr.json`](../locales/fr.json) is ~47% complete. Crisis **labels** can fall back to English via `Catalog::t` while **digits** still come from `crisis_resources.en.json` until `crisis_resources.fr.json` exists — or one global registry with all regions and only labels translated.

---

## Implementation checklist

- [x] `config/crisis_resources.en.json`
- [x] `src/crisis_resources.rs` — load at startup (`init_at_startup` in `run()`)
- [x] Unit test: point 406 returns 988 for US region
- [x] Judicial filing form + emergency hold feed card + safety legal card
- [ ] JSON Schema for editors (optional)
- [ ] `crisis_resources.fr.json` when French ships
- [ ] Document env vars for operator NCMEC separately in [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md)
- [ ] [`LOCALE_AUDIT.md`](LOCALE_AUDIT.md) entry for new `crisis.*` keys

---

## Changelog

| Date | Change |
|------|--------|
| 2026-06-03 | Initial design — charter-keyed registry vs `locales/` split |
