# Playwright front-end tests (256.app)

Browser smoke tests for user-visible flows. Domain logic stays in Rust `cargo test`; data is seeded via the `e2e_seed` binary.

**User stories:** [`docs/USER_STORIES.md`](../docs/USER_STORIES.md) — acceptance criteria, `@journey-*` / `@us-*` tags, narrative journeys.

## Test pyramid (this repo)

| Tier | Tool | Scope |
|------|------|--------|
| **API contract** | `cargo test` (`tests/`, `TestApp`) | Auth JSON, onboarding redirects, judicial math, graph lifecycle — no browser |
| **Browser smoke** | Playwright (`e2e/`) | Guest login shell, legal cards, public profiles, authenticated home shell (dev bypass) |
| **Regression** | Playwright `@regression` | Real form login (`form-login.spec.ts`) |

**CI / deploy:** On every PR and push to `main`, GitHub Actions runs **`cargo test`** + **`npm run test:public`**. Push to `main` also triggers **deploy to beta** after tests pass ([`.github/workflows/deploy.yml`](../.github/workflows/deploy.yml)). Full `test:smoke` (authenticated flows) is local-only until stabilized — see [`docs/LAUNCH_READINESS.md`](../docs/LAUNCH_READINESS.md) § Authenticated Playwright backlog.

## Prerequisites

- MySQL with migrations applied (`DATABASE_URL` in `.env`)
- Node.js 20+
- App running on `http://127.0.0.1:8080`

## One-time setup

```powershell
cd e2e
npm install
npx playwright install chromium
```

## Seed E2E personas

From the repo root (uses your `.env` `DATABASE_URL`):

```powershell
cargo run --bin e2e_seed -- --reset --manifest e2e/fixtures/manifest.json
```

For non-local databases, set `E2E_SEED_ALLOW=1`.

Personas created:

| Key | Email | UI gate |
|-----|-------|---------|
| `full` | `e2e.full@256.test` | `/home` feed shell |
| `consent` | `e2e.consent@256.test` | Terms consent step |
| `partial` | `e2e.partial@256.test` | `/onboarding` |

Shared password: `e2e-password-12`

Output: `e2e/fixtures/manifest.json` (gitignored; falls back to `manifest.example.json`).

## Run tests

Terminal 1 — start the app with dev auth bypass for smoke login:

```powershell
$env:DEV_AUTH_BYPASS="1"
$env:FACE_VISION_MOCK="1"
cargo run
```

Terminal 2:

```powershell
cd e2e
npm run test:smoke
```

**Public guest/login sweep only** (14 scenarios × desktop + mobile — recommended pre-deploy UI gate):

```powershell
npm run test:public
```

**Narrative journeys** (tagged `@journey-a` … `@journey-e` — see [`docs/USER_STORIES.md`](../docs/USER_STORIES.md)):

```powershell
npm run test:journey-a   # Guest discovers platform
npm run test:journey-b   # Public profile → join
npm run test:journey-c   # Returning member login → home
npm run test:journey-d   # Onboarding gates
npm run test:journey-e   # Protected route boundary
npm run test:journeys    # All journey-tagged specs
```

All tests (including `@regression` form login):

```powershell
npm test
```

Interactive UI mode:

```powershell
npm run test:ui
```

### Spec inventory

| File | Tags | Coverage |
|------|------|----------|
| `tests/smoke/public-pages.spec.ts` | `@m1 @smoke` | `/`, `/login`, register/login/forgot/reactivate cards, legal redirects, footer links, `/public/{handle}`, static CSS, health |
| `tests/smoke/home-full-user.spec.ts` | `@m2 @m1 @smoke` | Dev-bypass `/home`, private profile, public entity |
| `tests/smoke/onboarding-gates.spec.ts` | `@m2 @smoke` | Consent + partial onboarding redirects |
| `tests/regression/form-login.spec.ts` | `@m1 @regression` | Real `POST /auth/login` → `/home` |

Playwright runs with **`workers: 1`** — the local dev server is single-process; parallel workers cause 429 rate limits and flaky timeouts.

## Environment variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `BASE_URL` | `http://127.0.0.1:8080` | Target app |
| `DEV_AUTH_BYPASS` | — | Must be `1` on the **server** for `login-as` smoke helpers; also raises UI-card rate-limit burst via `Config` (local E2E only) |
| `E2E_SEED_ALLOW` | — | Required to seed non-local DBs |

## Auth behavior (browser vs API)

Protected routes use `src/middleware/auth_gate.rs`:

- Browser **GET** without session → redirect `/login?login=1`
- Browser **GET** with unverified email session → redirect `/login?resend=1`
- JSON / fetch clients (no `Accept: text/html`) → `401` / `403` JSON as before

Contract tests: `tests/m1_public_api.rs` (`protected_html_routes_*`).

## Reports

After a failed run:

```powershell
npm run report
```

Artifacts: `e2e/playwright-report/`, `e2e/test-results/`

## Manual QA (2026-08-11)

Run after local changes to home shell, feed engagement, council shares, private profiles, judicial filing pages, Nom purchase UI, or **Epic portrait pipeline** (`agents/council/`, `src/ai/portrait_*`, `src/bin/ai_gateway/portrait.rs`). Requires signed-in dev session (`DEV_AUTH_BYPASS=1` or real login) and seed data with at least one **connected human** peer and posts with comments. Portrait QA also needs **`ai_gateway` running** with Vertex credentials (or mock flags documented in [`docs/BETA_AI_SETUP.md`](../docs/BETA_AI_SETUP.md)).

| # | Area | Steps | Expected |
|---|------|-------|----------|
| 1 | **Private message** | Open a connected human’s private profile from the rail | Speech-bubble icon appears left of copy-link; tap opens create-post with `For: @private, @{handle}` and human **Post as** selected |
| 2 | **Sprite cache** | Hard refresh once after deploy | Private message icon visible (not empty 44px hit target) — confirms `post-actions.svg?v=…` cache bust |
| 3 | **Scroll on cancel** | Scroll feed down → open create-post → back | Feed returns to top |
| 4 | **Scroll on dismiss** | Scroll feed → open connected profile → dismiss/back to feed | Feed at top |
| 5 | **Scroll on search back** | Open search/add → back | Feed at top |
| 6 | **Judicial preview** | From feed, open **Raise an issue** on a post you have already voted on | Filing page shows post body **without** comment input or feel heart |
| 7 | **Nom purchase** | Topbar Noms → select Civic pack → Pay with Stripe test card | Processing state hides form; success closes modal, toasts, topbar balance updates. Cancel path closes without charge |
| 8 | **Civic pack gate** | With balance &lt; 256 | Civic pack selectable regardless of balance (wallet cap only) |
| 9 | **Comment preview** | Find a post with comments you have **not** voted on | Top 3 comments visible read-only; “Vote to comment & reply” shown; after vote, compose input appears |
| 10 | **Comment clamp** | Expand a long comment via see-more | No partial fourth line visible above ellipsis |
| 11 | **Avatar redraw** | Profile → avatar redraw; lean in, glasses on; wait **30–90s** on painting step | Editorial bust on `#000000`; likeness to capture; no mouth void or coloured disk; gateway log `portrait likeness pick`; **Noms debited only on success** |

**Epic portrait deploy (2026-08-11 code; 2026-08-12 beta):** style plate v6 + colour card + likeness pick live on beta. Hard refresh `/home` after deploy (`home-shell.js?v=20260811-redraw-models2`). Manual QA row 11 — avatar redraw end-to-end.

Traceability: [`docs/USER_STORIES.md`](../docs/USER_STORIES.md) — US-HOME-04, US-HOME-05, US-JUD-03, US-NOMS-01, US-FEED-03.

## Pre-deploy command checklist

From repo root (matches [`.github/workflows/deploy.yml`](../.github/workflows/deploy.yml)):

```powershell
cargo test -- --test-threads=2
cargo test --test m2_avatar_redraw
cargo test --lib portrait_timeout portrait_raster epic_portrait
cargo build --bin app --bin ai_gateway --bin e2e_seed
cargo run --bin e2e_seed -- --reset --manifest e2e/fixtures/manifest.json
```

Terminal 1 — app with dev bypass (Playwright):

```powershell
$env:DEV_AUTH_BYPASS="1"
$env:FACE_VISION_MOCK="1"
$env:SCHEDULER_TICK_SECS="0"
cargo run
```

Terminal 2 — Playwright public gate (14 scenarios × desktop + mobile):

```powershell
cd e2e
npm run test:public
```

Optional local smoke (authenticated flows): `npm run test:smoke` with seed personas above.
