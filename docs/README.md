# 256.app documentation map

One place to find product, implementation, launch, and research docs. **Start here** when onboarding a contributor, writing marketing copy, or reconciling “what does the site actually do?”

**Last reviewed:** 2026-08-12 (multi-lens review refresh — Jul–Aug 2026 batch). Prior: 2026-08-12 (Epic portrait deploy to beta; CI disk hardening). Prior: 2026-07-23 (civic standing literacy).

---

## Tier 1 — Product & brand (read first)

| Document | Purpose |
|----------|---------|
| **[`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md)** | **Canonical product bible** — positioning, features (live vs planned), brand, voice, judiciary & Noms summary, lifecycle rules, where user copy lives |
| [`USER_STORIES.md`](USER_STORIES.md) | **User stories + acceptance criteria** — traced to Playwright/Rust tests; narrative journeys for QA, demos, and marketing |
| [`ONBOARDING.md`](ONBOARDING.md) | Signup → activation funnel (server gates, consent column, implementation paths) |
| [`LOCALE_AUDIT.md`](LOCALE_AUDIT.md) | `en.json` vs product guide drift log |
| [`../SITE_FEATURES_REFERENCE.md`](../SITE_FEATURES_REFERENCE.md) | **Redirect stub** — kept for old links; content lives in `PRODUCT_GUIDE.md` |

**User-facing copy (source of truth for shipped UI text):**

| Location | Role |
|----------|------|
| [`locales/en.json`](../locales/en.json) | English UI strings (marketing bullets, Root Charter, feed, judiciary) |
| [`locales/fr.json`](../locales/fr.json) | French (partial — ~47% of en keys) |
| [`templates/guest_*`](../templates/) | Guest login shell fragments (marketing, constitution, council avatars) |

**Brand & visual system:**

| Location | Role |
|----------|------|
| [`static/css/brand.css`](../static/css/brand.css) | Palette tokens, typography, card accent semantics, mobile-first layout |
| [`docs/ICONOGRAPHY.md`](ICONOGRAPHY.md) | Icon sizes (48 / 32 / 16), council-informed art direction, system-icon yellow hover |
| [`static/icons/`](../static/icons/) | Action SVG sprites (`post-actions.svg`, globe) |
| [`.cursor/rules/ui-color-palette-usage.mdc`](../.cursor/rules/ui-color-palette-usage.mdc) | Agent mirror of palette usage (keep in sync with `brand.css`) |
| [`.cursor/rules/ui-iconography.mdc`](../.cursor/rules/ui-iconography.mdc) | Agent mirror of iconography rules |

---

## Tier 2 — Implementation rules (code + agents)

| Document | Purpose |
|----------|---------|
| [`../AGENTS.md`](../AGENTS.md) | Load-bearing product rules for agents: graph lifecycle, judiciary math, Noms, scope, image standard |
| [`../src/pricing.rs`](../src/pricing.rs) | **Nom price schedule** (code-only; mirror changes in `PRODUCT_GUIDE.md`) |
| [`COMMUNITY_GRAPH_MANAGEMENT.md`](COMMUNITY_GRAPH_MANAGEMENT.md) | Graph lifecycle implementation spec (death, inactivity, failing UI) |
| [`.cursor/rules/`](../.cursor/rules/) | Cursor rule mirrors of AGENTS sections |

---

## Tier 2b — Testing & CI

Two-layer pyramid — domain logic in Rust, guest UX in Playwright. **Both layers run in GitHub Actions** before merge/deploy to beta:

| Layer | Location | Local | CI |
|-------|----------|-------|-----|
| **Integration (API contract)** | [`tests/`](../tests/) + [`tests/common/`](../tests/common/) | `cargo test` | `rust-test` job in [`.github/workflows/test.yml`](../.github/workflows/test.yml) |
| **Domain preview reputation (Tier 2b)** | [`tests/domain_reputation.rs`](../tests/domain_reputation.rs) + [`src/content/domain_reputation.rs`](../src/content/domain_reputation.rs) unit tests | `cargo test --test domain_reputation` | Same `rust-test` job |
| **Avatar redraw contract** | [`tests/m2_avatar_redraw.rs`](../tests/m2_avatar_redraw.rs) | `cargo test --test m2_avatar_redraw` | Same `rust-test` job |
| **Portrait unit tests** | `src/ai/portraiture.rs`, `portrait_*.rs`, `agent_specs.rs` | `cargo test --lib portrait epic_portrait` | Same `rust-test` job |
| **Browser smoke (UI/UX)** | [`e2e/`](../e2e/) | `npm run test:public` | `playwright-public` job (MySQL 8.4 service, `e2e_seed`, app boot) |
| **User stories (test ↔ product)** | [`USER_STORIES.md`](USER_STORIES.md) | — | Story IDs map to specs; see narrative journeys |
| **Persona seed** | [`src/bin/e2e_seed.rs`](../src/bin/e2e_seed.rs) | `cargo run --bin e2e_seed -- --reset --manifest e2e/fixtures/manifest.json` | Same command in CI before Playwright |

**Deploy gate:** [`.github/workflows/deploy.yml`](../.github/workflows/deploy.yml) on push to `main`: `test` → `build` → `deploy`. **PRs:** [`.github/workflows/ci.yml`](../.github/workflows/ci.yml) runs the same `test.yml` without deploy. Runner disk: [`scripts/ci/free_runner_disk.sh`](../scripts/ci/free_runner_disk.sh). Manual pre-deploy — [`DEPLOY_BETA.md`](DEPLOY_BETA.md).

---

## Tier 3 — Launch, safety & backlog

| Document | Purpose |
|----------|---------|
| [`LAUNCH_READINESS.md`](LAUNCH_READINESS.md) | Launch tiers, domain scorecard, checklists, doc drift log |
| [`DEPLOY_BETA.md`](DEPLOY_BETA.md) | **Beta CI/CD (confirmed 2026-08-12)** — test-gated GitHub Actions → GCP VM |
| [`BETA_AI_SETUP.md`](BETA_AI_SETUP.md) | Vertex / `ai_gateway` credentials on the beta VM |
| [`BETA_AI_SETUP.md`](BETA_AI_SETUP.md) | **Hosted beta:** Vertex / `ai_gateway` credentials on the VM (face vision + Imagen avatars) |
| [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) | Tier 1–2 safety roadmap; content model; emergency flow; **§ Outbound link domain policy** (live Tier 2b) |
| [`CRISIS_RESOURCES.md`](CRISIS_RESOURCES.md) | Charter-keyed crisis contacts — **design** (`config/` vs `locales/`) |
| [`SYSTEM_THEORY.md`](SYSTEM_THEORY.md) | Ecosystem backlog + **current priorities** (June 2026): verdict UX, recovery/standing literacy, beta ops, public-launch T&S gates |
| [`ROOT_CONSTITUTION_PROPOSAL.md`](ROOT_CONSTITUTION_PROPOSAL.md) | Root Operating Charter — **v1** 2026-05-27; **v2 shipped** 2026-05-29 (16 points) |
| [`CONSTITUTIONAL_SAFETY_FRAMEWORK.md`](CONSTITUTIONAL_SAFETY_FRAMEWORK.md) | Safety floor + charter alignment notes |

---

## Tier 4 — Strategic reviews (research, not product canon)

Use for tuning and diligence — **do not** treat as user-facing copy without editing through `PRODUCT_GUIDE.md` / locales.

| Document | Lens |
|----------|------|
| [`MULTI_LENS_REVIEW_2026-08-12.md`](MULTI_LENS_REVIEW_2026-08-12.md) | **Five-lens snapshot** (Product Architect, enthusiast, clinical, adversarial, investor) — Aug 2026 batch |
| [`POLITICAL_SCIENCE_REVIEW.md`](POLITICAL_SCIENCE_REVIEW.md) | Governance legitimacy |
| [`EVOLUTIONARY_BIOLOGY_REVIEW.md`](EVOLUTIONARY_BIOLOGY_REVIEW.md) | Fitness, selection, extinction |
| [`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md) | Fresh-eyes social media take (power-user lens) |
| [`CLINICAL_PSYCHOLOGY_REVIEW.md`](CLINICAL_PSYCHOLOGY_REVIEW.md) | Harm assessment, publication ethics, vulnerable populations |
| [`DESIGN_BIAS_RECOVERY.md`](DESIGN_BIAS_RECOVERY.md) | Punitive vs recovery bias reminder |

---

## Tier 5 — AI Council agent specs (internal)

| Location | Purpose |
|----------|---------|
| [`agents/council/`](../agents/council/) | Council member agent specs, RAG corpora, AVC pattern |
| [`static/generated/council/`](../static/generated/council/) | Generated council avatar assets |

---

## Canonical source reconciliation

When docs disagree, use this order:

| Topic | Canonical |
|-------|-----------|
| What the product **is** (features, voice, live vs planned) | [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) |
| Nom **prices** | [`src/pricing.rs`](../src/pricing.rs) → mirrored in `PRODUCT_GUIDE.md` |
| Shipped **UI strings** | [`locales/en.json`](../locales/en.json) |
| Graph **lifecycle math** | [`AGENTS.md`](../AGENTS.md) → detail in [`COMMUNITY_GRAPH_MANAGEMENT.md`](COMMUNITY_GRAPH_MANAGEMENT.md) |
| **Onboarding gate order** | [`ONBOARDING.md`](ONBOARDING.md) |
| **Launch blockers & checklists** | [`LAUNCH_READINESS.md`](LAUNCH_READINESS.md) |
| **Browser + integration testing** | [`e2e/README.md`](../e2e/README.md) + [`tests/common/api.rs`](../tests/common/api.rs) |
| **Safety build list** | [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) |
| **Incomplete product work** | [`SYSTEM_THEORY.md`](SYSTEM_THEORY.md) |

**Resolved drift (2026-06-09):** Post-auth consent (not signup); Google OIDC + reCAPTCHA shipped. Age minimum is **16+**. Onboarding order is **auth → consent → face → profile → godmother**. See [`ONBOARDING.md`](ONBOARDING.md).

---

## Changelog

| Date | Change |
|------|--------|
| 2026-06-10 | **Product UX batch** — Search/Add loader + shared recommendation rows; no Post as community; human failing band owner-only; friend-invite email precheck; jury summons severity + free acute abstention — [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) changelog |
| 2026-06-09 | **Rating economics** — symmetric Loved/Loathed conversation settlement; failing band **0.66**; [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) § Conversation settlement |
| 2026-05-25 | Initial documentation map; `PRODUCT_GUIDE.md` becomes product canon |
| 2026-05-25 | Added [`LOCALE_AUDIT.md`](LOCALE_AUDIT.md) |
| 2026-05-29 | Root charter v2 shipped — [`ROOT_CONSTITUTION_PROPOSAL.md`](ROOT_CONSTITUTION_PROPOSAL.md) |
| 2026-05-28 | Outbound link domain policy documented in TRUST_AND_SAFETY + PRODUCT_GUIDE |
| 2026-06-03 | Scholarly reviews refreshed; [`CRISIS_RESOURCES.md`](CRISIS_RESOURCES.md); content model + emergency flow in PRODUCT_GUIDE / TRUST_AND_SAFETY |
