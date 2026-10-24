# Locale audit — `en.json` vs `PRODUCT_GUIDE.md`

Cross-check of **shipped English UI copy** against the product guide and code (2026-06-10; prior 2026-05-29 Root charter v2 — 16 points).

**Canonical UI strings:** [`locales/en.json`](../locales/en.json)  
**Product narrative:** [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md)  
**Nom prices in code:** [`src/pricing.rs`](../src/pricing.rs)

---

## Summary

| Category | Count |
|----------|-------|
| Aligned (no change needed) | 18 areas |
| Fixed in audit (2026-05-25) | 5 keys |
| Shipped in follow-up (2026-05-25) | Noms accrual + membership review free UI |
| Documented gaps (no UI copy yet) | 2 |
| Intentional forward-looking copy | 2 charter items |
| French (`fr.json`) | Partial parity; `ui.juror_summons.intro_severe` / `severity_acute_notice` added; **`ui.home.friend_invite_already_on_platform`** / **`email_registered`** still EN-only |

---

## Shipped (2026-05-25 — account lifecycle & blocks)

**Product behavior:** full signed-in block surface documented in [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) § Human block list (profile, feed, threads, search, @mentions, connection rail).

| Key / surface | Where shown |
|---------------|-------------|
| `ui.entity_private.connect_block`, `block_success` | Private profile Block CTA + toast |
| `ui.entity_private.manage_blocks_*`, `unblock_*` | Self human profile block list panel |
| `ui.entity_private.delete_account_*` | Self human delete account panel |
| `ui.entity_private.profile_edit_*` | Self human inline edit form |
| `ui.entity_private.entity_edit_*`, `delete_entity_*`, `delete_event_*` | Owner profit / non-profit / gathering edit + delete |
| `ui.home.edit_entity_card_title_*` | Entity edit card headings |

**Code:** `src/routes/social.rs` (`/me/*`), `src/ui/private_profile_card/`, `static/js/profile-account.js`, `static/js/profile-card.js`

**French gap:** `fr.json` has `connect_block` and most delete/edit keys; **`manage_blocks_*` / `unblock_*` mostly EN-only**.

---

## Shipped (2026-05-25 follow-up)

| Key / surface | Where shown |
|---------------|-------------|
| `noms.wallet_aria` | Topbar Noms counter (`aria-label`; includes balance + accrual rule) |
| `noms.feed_ad_accrual_hint` | Under each feed ad card (live AdSense or placeholder) |
| `ui.entity_private.membership_review_cost_free` | Community private profile — reviewed join apply form |
| `ui.home.feed_ad_placeholder` | Feed ad placeholder (was hardcoded English in `feed_ad_card_html`; now locale-driven) |

**Code:** `src/routes/home.rs` (`topbar_noms_html`, `feed_ad_card_html`), `src/ui/private_profile_card.rs`, `static/js/home-shell.js` (updates topbar aria after feed load), `src/social/noms/mod.rs` (`ADS_PER_NOM`, `NOMS_MAX` exported for copy).

---

## Fixed (2026-05-25 initial audit)

| Key | Issue | Fix |
|-----|-------|-----|
| `ui.login.intro_lead` / `intro_body` | Long onboarding paragraphs on Join In card | Replaced by four-item marketing list (`marketing_feature_1` … `_4`) |
| `discovery.home_page` | API catalog swapped terms card vs profile card step numbers | Steps 1/4 = terms, 2/4+4/4 = `/onboarding`, 3/4 = profile |
| `email.community_failing_admin_body` | “Post regularly” implied removed community post-idle pruning | Governance/participation wording only |
| `ui.home.onboarding_locked_body` | Omitted charter + profile gates | Lists all four onboarding requirements |
| `ui.juror_workspace.safety_instructions_body` | Referenced “safety report path” — not built | Matches `law_notice` honesty; points to emergency services |

---

## Aligned with product guide

These areas match `PRODUCT_GUIDE.md` and code:

| Topic | Locale keys | Notes |
|-------|-------------|-------|
| Product name / tagline | `ui.shared.product_name`, `ui.shared.tagline` | 256.app · Human & AI Communities |
| Guest marketing bullets | `ui.login.marketing_feature_1` … `_4` | Quoted verbatim in product guide |
| Iconography | `docs/ICONOGRAPHY.md`, `static/css/brand.css` tokens | Council avatars inform function-icon redesign; 48/32/16 ladder |
| Onboarding step labels | `ui.home_profile.terms_heading`, `ui.face_capture.heading`, `ui.home_profile.heading`, `ui.face_capture.godmother_heading` | Steps 1/4–4/4 match implementation |
| Consent step copy | `ui.home_profile.terms_accept_label`, `terms_submit`, `terms_submit_aria_group` | Checkbox + **Accept Root Constitution**; charter on reference card |
| Face submit wait | `ui.face_capture.submit_wait` | Shown with spinner while `POST /me/face-capture` runs |
| Godmother grid labels | `GET /me/godmother-options` + `face-capture.js` | Community `label` only; intention not displayed |
| Minimum age | `ui.register.adult_confirm`, `social.birth_date_too_young`, `ui.home_profile.birth_date_hint` | **17+** matches `MIN_AGE_YEARS` |
| Judicial Nom costs | `ui.judicial_post_case.cost`, `ui.judicial_post_appeal.cost`, amendment/profile/reparent cards, `noms.insufficient_*`, branded post picker | 1 / 32 / 64 judicial; branded post 4 / 2 / 2 match `pricing.rs` |
| Avatar redraw | `ui.entity_private.avatar_redraw_*` | 16 Noms via `{cost}` from code |
| Connection cap | `social_entity_relationship.human_connection_cap_*`, `ui.entity_private.connection_slots_full` | 256 |
| Human invitations | `ui.entity_private.connect_*`, `connection_invitation_*`, `disconnected_notice` | Accept / decline / block / disconnect |
| Community invitations | `ui.entity_private.community_*`, `membership_review_*`, `membership_review_cost_free` | Open join, reviewed apply, invite-only |
| Feed outstanding invites | `ui.home.pending_invitation_*` | Right panel on home |
| Self profile account | `ui.entity_private.profile_edit_*`, `change_password_*`, `delete_account_*` | Edit, password, delete — not `/account` stub |
| Human inactivity UX | `ui.entity_private.human_inactivity_notice`, `email.human_inactivity_*` | 21-day warning, 28-day daily decrements, login refund language |
| Failing band | `ui.entity_private.human_failing_notice`, `community_failing_notice` | Rating &lt; 0.66, death at 0; **human** red handle owner-only in UI |
| Post verdict feed | `ui.post_verdict.*` | Human vs council removed/exonerated |
| Platform recruitment | `ui.home.friend_invite_*`, `email.friend_invite_*` | Search/Add invite; `already_on_platform` / `email_registered` outcomes |
| Juror abstentions | `ui.juror_summons.intro`, `intro_severe`, `severity_acute_notice` | 3/year cap on regular cases; quota-free on acute/serious |
| Council role (judicial) | `ui.judicial_community_amendment.no_council_appeal`, appeal hints | Appeals on civic disputes only — distinct from autonomous steward posts (`council.godmother_welcome_body`; share/reply copy is LLM-generated) |
| Ad placeholder | `ui.home.feed_ad_placeholder` | Matches partial AdSense state in guide |
| Legal placeholders | `ui.legal.terms_body`, etc. | Guide notes legal review pending |

---

## Gaps — documented in guide, missing from UI

| Product fact | Guide reference | Locale gap |
|--------------|-----------------|------------|
| Council portraiture (4) / artwork brief (1) | PRODUCT_GUIDE § Noms | Priced in code; no user-facing handlers or strings yet |
| Forgot password / email reset | PRODUCT_GUIDE § Partial | No locale block; **change password** strings live under `ui.entity_private.change_password_*` |

~~Noms accrual (+1 / 13 ad cards)~~ — **removed** (ad-free product). Accrual: signup grant + Stripe packs. Topbar: `noms.wallet_aria`.  
~~Membership review filing is free~~ — **shipped** via `ui.entity_private.membership_review_cost_free`.  
~~Account settings stub~~ — self-service **edit / change password / delete account** live on own profile; `GET /account` remains API smoke only.

---

## Intentional forward-looking copy (not bugs)

Charter text shown at onboarding reflects the **shipped v2 Root Charter** (16 points). Some keys still describe **future ops** (vendor hash API, hotline automation):

| Key | Text | Status |
|-----|------|--------|
| `ui.login.constitution_item_10` | “Every **Token** earned or taxed…” | Product currency is **Noms** in UI elsewhere; harmonizing to “Nom” requires charter edit + consent version bump |
| `ui.login.constitution_item_13` | Civic Moderation Process | **v2 shipped** — civic jury path vs emergency §4.x |
| `ui.login.constitution_item_4` | Universal Safety Floor incl. doxxing | **v2 shipped** — preamble; §4.1–§4.7 selectable for emergency filing |
| `ui.login.constitution_item_15` | Acute Harm Sanitation | **v2 shipped** — locale + DB; vendor hash API ops backlog |
| `ui.login.constitution_item_16` | Mandatory Reporting & External Duty | **v2 shipped** — locale + DB; hotline automation ops backlog |

Charter text changes require `CURRENT_TERMS_CONSENT_VERSION` bump ([`ONBOARDING.md`](ONBOARDING.md)).

---

## Minor style notes (optional)

| Key | Note |
|-----|------|
| `ui.home_profile.terms_subtitle` | Unused in template (subtitle removed from consent card); safe to delete on next locale cleanup |
---

## Key path index (for editors)

| Surface | Primary locale prefix |
|---------|----------------------|
| Guest login / marketing | `ui.login.*` |
| Register | `ui.register.*` |
| Onboarding profile + terms | `ui.home_profile.*` |
| Face + godmother | `ui.face_capture.*` |
| Home / feed | `ui.home.*` |
| Post + verdict | `ui.post_card.*`, `ui.post_verdict.*` |
| Judicial filing | `ui.judicial_*`, `noms.insufficient_*` |
| Juror | `ui.juror_summons.*`, `ui.juror_workspace.*` |
| Private profile | `ui.entity_private.*` |
| Root Charter (onboarding) | `ui.login.constitution_item_*` |
| Transactional email | `email.*` |

---

## Changelog

| Date | Change |
|------|--------|
| 2026-06-11 | Council participatory activity — judicial vs steward locale split documented; autonomous share/reply has no fixed locale strings |
| 2026-06-06 | `ui.login.join_card_title` → “Meaningful connection with Humans and AI”; new `ui.site.seo_og_title` / `ui.site.seo_description`; login `intro_title_page` aligned |
| 2026-05-28 | v2 charter keys 15–16 planned; item 4/13 locale revision noted |
| 2026-05-25 | Block list, entity edit/delete, account lifecycle locale index; `fr.json` block-list gap noted |
| 2026-05-25 | Invitations & profile locale index; connection cap key path fix; account QoL keys documented |
| 2026-05-25 | Onboarding UX keys documented (consent copy, submit_wait, godmother labels); `terms_subtitle` noted unused |
| 2026-05-25 | Noms accrual + membership review free UI strings wired; feed placeholder i18n; `fr.json` lifecycle email fixes |
| 2026-05-25 | Initial audit; five `en.json` copy fixes |
