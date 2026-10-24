# 256.app — user stories

**Canonical user-story catalog** for shipped and tested product behavior. Each story maps to automated tests so QA, engineering, and marketing share one vocabulary.

**Navigation:** [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) (what we build) · [`e2e/README.md`](../e2e/README.md) (how to run browser tests) · [`tests/m1_public_api.rs`](../tests/m1_public_api.rs) (public API contract)

**Last reviewed:** 2026-06-06

---

## How to use this document

| Audience | Use |
|----------|-----|
| **Product / marketing** | Pull story titles and “so that” lines for feature explainers, demo scripts, and beta onboarding copy. |
| **QA / engineering** | Trace acceptance criteria → spec files; compose **narrative journeys** (below) into chained Playwright runs. |
| **Contributors** | Add a story **before** or **with** a new test; keep IDs stable once referenced in CI or release notes. |

### Story format

```
ID · Title
As a [persona]
I want [goal]
So that [benefit]

Acceptance criteria
- Given … When … Then …

Tests · [automated trace]
Status · CI | local | planned
```

### Personas

| Persona | Meaning |
|---------|---------|
| **Guest** | Not signed in; browser visitor |
| **Prospective member** | Guest exploring Join In / register |
| **Returning member** | Has an account; may or may not be verified or onboarded |
| **Verified member** | Email verified; may still be mid-onboarding |
| **Fully onboarded member** | Passed consent, face capture, profile, godmother — reaches `/home` feed |
| **Platform** | System rules enforced via API (no browser) |
| **Operator / monitoring** | Health checks, deploy gates |

### E2E seed personas (local / CI browser tests)

| Key | Email | Password | Gate after login |
|-----|-------|----------|------------------|
| `full` | `e2e.full@256.test` | `e2e-password-12` | `/home` feed shell |
| `consent` | `e2e.consent@256.test` | same | Terms consent (`?onboarding=consent`) |
| `partial` | `e2e.partial@256.test` | same | Face onboarding (`/onboarding`) |

Public profile fixture handle: **`256`** (`@256` — from `e2e/fixtures/manifest.example.json`).

---

## Epic 1 — Guest discovery & login shell

*First impression: join, sign in, legal transparency, public discovery.*

### US-PUB-01 · Land on login from home

**As a** guest visiting the site  
**I want** the home URL to take me to the login experience  
**So that** I always start from sign-in or join  

**Acceptance criteria**
- Given I open `/`
- When the page loads
- Then I am on `/login`
- And the guest auth layout is visible (`#guest-split-auth`)

**Tests:** `e2e/tests/smoke/public-pages.spec.ts` → *root redirects to login*  
**Status:** CI (`npm run test:public`)

---

### US-PUB-02 · See the full login landing page

**As a** guest  
**I want** the login page to show join, sign-in, constitution, and footer  
**So that** I understand how to enter and what governance I am joining  

**Acceptance criteria**
- Given I open `/login`
- Then I see the join panel, Join In and Sign in buttons, the Root constitution panel, and the public footer

**Tests:** `public-pages.spec.ts` → *login page loads guest shell with join card and constitution*  
**Status:** CI

---

### US-PUB-03 · Start registration from Join In

**As a** prospective member  
**I want** to open the registration form from the Join In card  
**So that** I can create an account  

**Acceptance criteria**
- Given I am on `/login`
- When I click Join In
- Then the register panel and form appear with a visible submit control

**Tests:** `public-pages.spec.ts` → *join card opens register form*  
**Status:** CI

---

### US-PUB-04 · Start sign-in from the login choice

**As a** returning member  
**I want** to open the email/password form from Sign in  
**So that** I can log back in  

**Acceptance criteria**
- Given I am on `/login`
- When I click Sign in
- Then the login panel and form appear with email and password fields

**Tests:** `public-pages.spec.ts` → *login choice opens sign-in form*  
**Status:** CI

---

### US-PUB-05 · Deep-link straight to sign-in

**As a** returning member following a “log in” link  
**I want** `?login=1` to show the sign-in panel immediately  
**So that** I skip the join chooser  

**Acceptance criteria**
- Given I open `/login?login=1`
- Then the login panel is visible and the join panel is hidden

**Tests:** `public-pages.spec.ts` → *login query param shows sign-in panel*  
**Status:** CI

---

### US-PUB-06 · Register via `/register`

**As a** guest with a register bookmark or outbound link  
**I want** `/register` to land me on signup  
**So that** acquisition URLs work reliably  

**Acceptance criteria**
- Given I open `/register`
- Then I am redirected to `/login` with the register card and form visible

**Tests:** `public-pages.spec.ts` → *register redirect loads signup card*  
**Status:** CI

---

### US-PUB-07 · Register via `?join=1`

**As a** guest from a Join In CTA (e.g. public profile)  
**I want** `?join=1` to open signup directly  
**So that** I can join right after discovering someone  

**Acceptance criteria**
- Given I open `/login?join=1`
- Then the register card and form are visible

**Tests:** `public-pages.spec.ts` → *join query param loads signup card*  
**Status:** CI

---

### US-PUB-08 · Recover a forgotten password

**As a** member who forgot my password  
**I want** to reach the forgot-password card from sign-in  
**So that** I can request a reset  

**Acceptance criteria**
- Given I am on `/login?login=1`
- When I activate forgot password from the login panel
- Then the forgot-password panel and card are visible

**Tests:** `public-pages.spec.ts` → *forgot password card loads from login panel*  
**Status:** CI

---

### US-PUB-09 · Reactivate a dormant account

**As a** member whose account was deactivated  
**I want** to reach the reactivate card from sign-in  
**So that** I can attempt recovery  

**Acceptance criteria**
- Given I am on `/login?login=1`
- When I activate reactivate from the login panel
- Then the reactivate panel and card are visible

**Tests:** `public-pages.spec.ts` → *reactivate account card loads from login panel*  
**Status:** CI

---

### US-PUB-10 · Read legal documents as a guest

**As a** guest researching the platform  
**I want** terms, privacy, conditions, and safety URLs to open the legal card  
**So that** I can read policies without creating an account  

**Acceptance criteria**
- Given I open `/terms`, `/privacy`, `/conditions`, or `/safety`
- Then I am redirected to `/login` with the legal card and constitution panel visible

**Tests:** `public-pages.spec.ts` → *legal pages render via redirects*; `tests/m1_public_api.rs` → *public_pages_render_for_guests* (redirect targets)  
**Status:** CI

---

### US-PUB-11 · Open legal docs from the footer

**As a** guest on the login page  
**I want** footer links for terms, privacy, and safety to open the legal card  
**So that** I can read policies in context  

**Acceptance criteria**
- Given I am on `/login`
- When I click each footer legal link (terms, privacy, safety)
- Then the legal card opens

**Tests:** `public-pages.spec.ts` → *footer legal links open legal cards*  
**Status:** CI

---

### US-PUB-12 · Discover a public profile without logging in

**As a** guest who followed a profile link  
**I want** to view a public entity page with clear join and sign-in paths  
**So that** I can decide whether to join after seeing who they are  

**Acceptance criteria**
- Given I open `/public/256` (platform handle; seeded in E2E)
- Then I see the public guest layout and entity profile card
- And I see Join In (`/login?join=1`) and Sign in (`/login?login=1`) CTAs

**Tests:** `public-pages.spec.ts` → *public entity page renders join CTA and profile card*; `home-full-user.spec.ts` → *public entity page renders*  
**Status:** CI (public-pages); local smoke (home-full-user duplicate)

**Marketing angle:** Public profiles are discovery surfaces — link-out content model, shareable handles, acquisition CTAs on every guest view.

---

### US-PUB-13 · Site styling loads reliably

**As a** visitor  
**I want** the brand stylesheet to load  
**So that** the UI renders with correct typography, palette, and layout  

**Acceptance criteria**
- Given a request to `/static/css/brand.css`
- Then the response is OK with `text/css` content type

**Tests:** `public-pages.spec.ts` → *brand stylesheet loads*  
**Status:** CI

---

### US-PUB-14 · Know the service is up

**As an** operator or monitoring tool  
**I want** a health endpoint to respond OK  
**So that** deploy and uptime checks can succeed  

**Acceptance criteria**
- Given a request to `/health/status`
- Then the response is OK

**Tests:** `public-pages.spec.ts` → *health status responds*; `m1_public_api.rs` → *public_pages_render_for_guests*  
**Status:** CI

---

## Epic 2 — Authenticated home & profiles

*Signed-in shell after onboarding — feed, private profile, public discovery.*

### US-HOME-01 · Fully onboarded member sees the feed

**As a** fully onboarded member  
**I want** to land on the home feed shell after login  
**So that** I can read my audience-scoped feed and participate  

**Acceptance criteria**
- Given I am signed in as the `full` E2E persona
- When I reach `/home`
- Then I see the app shell with profile complete (`data-needs-profile="0"`)
- And the main content area is visible

**Tests:** `e2e/tests/smoke/home-full-user.spec.ts` → *fully onboarded user sees feed shell*  
**Status:** Local (`npm run test:smoke`; requires `DEV_AUTH_BYPASS=1` on server)

**Prerequisites:** `cargo run --bin e2e_seed -- --reset --manifest e2e/fixtures/manifest.json`

---

### US-HOME-02 · View my private profile

**As a** signed-in member  
**I want** to open my private profile page  
**So that** I can review my entity, connections, and settings context  

**Acceptance criteria**
- Given I am signed in as the `full` persona
- When I open `/home/profile/{my-handle}`
- Then the app shell and main content area load

**Tests:** `home-full-user.spec.ts` → *private profile page loads for self*  
**Status:** Local smoke

---

### US-HOME-04 · Private message a connected human

**As a** signed-in member with an active friend/family/colleague connection  
**I want** a Private message control on their private profile  
**So that** I can open create-post with the correct `@private` audience preset  

**Acceptance criteria**
- Given I view a connected human’s private profile (not self)
- When the profile card loads
- Then I see a **Private message** header icon left of **Copy public link**
- When I tap Private message
- Then the create-post composer opens with `For: @private, @{handle}` and **Post as** my human entity

**Tests:** Manual QA — [`e2e/README.md`](../e2e/README.md) § Manual QA (2026-07-22); Rust HTML via connected profile fixture *(automated Playwright backlog)*  
**Status:** Shipped (2026-07-22)

---

### US-HOME-05 · Back and cancel scroll feed to top

**As a** signed-in member navigating the home shell  
**I want** back/cancel actions to return me to the top of the feed  
**So that** I am not left mid-scroll after closing a panel  

**Acceptance criteria**
- Given I have scrolled the feed, then opened and closed any of: create-post, search/add, create-entity (back to search), or a private profile overlay
- When I return to the home feed
- Then the feed panel scroll position is at the top
- And browser back/forward to `/home` also restores top-of-feed scroll

**Tests:** Manual QA — [`e2e/README.md`](../e2e/README.md) § Manual QA (2026-07-23)  
**Status:** Shipped (2026-07-22)

---

### US-FEED-03 · Pre-vote comment preview on feed cards

**As a** signed-in member who has not yet voted on a post  
**I want** to see the top comments before I vote  
**So that** I can gauge the conversation before committing my feel vote  

**Acceptance criteria**
- Given a post in my feed with L1 comments and I have **not** cast a feel vote
- When the post card loads
- Then up to **3** top L1 comments appear read-only (no vote/comment/reply on those rows)
- And “Vote to comment & reply” is shown in the thread slot
- When I cast my feel vote
- Then the full interactive thread appears (compose, pagination, nested replies)
- And `GET /posts/{id}/thread` returns **404** for roots outside my feed scope

**Tests:** `cargo test --test engagement_thread` → `thread_preview_top_three_before_vote`  
**Status:** Shipped (2026-07-23) · CI (`cargo test`)

---

### US-HOME-03 · Public profile renders for any visitor

**As a** visitor (guest or member)  
**I want** a public entity page to show the profile card  
**So that** discovery works regardless of auth state  

**Acceptance criteria**
- Given I open `/public/{handle}` without signing in
- Then the entity public profile card is visible

**Tests:** `home-full-user.spec.ts` → *public entity page renders* (overlaps US-PUB-12)  
**Status:** Local smoke

---

## Epic 3 — Onboarding gates

*Staged identity: charter consent before face capture before feed.*

### US-ONB-01 · Accept terms before using the app

**As a** verified member who has not yet consented to the Root charter  
**I want** `/home` to route me to the consent step  
**So that** I cannot skip governance agreement  

**Acceptance criteria**
- Given I am signed in as the `consent` E2E persona
- When I navigate to `/home`
- Then I am redirected to a URL containing `onboarding=consent`

**Tests:** `e2e/tests/smoke/onboarding-gates.spec.ts` → *consent-only user is routed to terms step*  
**Status:** Local smoke

**Marketing angle:** Every member signs the Root Operating Charter — disputes resolved by community juries, not opaque moderation.

---

### US-ONB-02 · Complete face onboarding before the feed

**As a** member who consented but has not finished onboarding  
**I want** `/home` to route me to onboarding  
**So that** I finish identity setup (face capture, profile, godmother) before participating  

**Acceptance criteria**
- Given I am signed in as the `partial` E2E persona
- When I navigate to `/home`
- Then I am redirected to `/onboarding`

**Tests:** `onboarding-gates.spec.ts` → *partial user is routed to face capture onboarding*  
**Status:** Local smoke

---

## Epic 4 — Real sign-in (regression)

*Production auth path without dev bypass.*

### US-AUTH-01 · Sign in with email and password

**As a** returning verified member  
**I want** to sign in through the login form  
**So that** I reach my home feed like a real user  

**Acceptance criteria**
- Given I am on `/login?login=1`
- When I enter the `full` persona email and password and submit
- Then I land on `/home` with profile complete

**Tests:** `e2e/tests/regression/form-login.spec.ts` → *form login reaches home for full persona*  
**Status:** Local regression (`npm test`; not CI gate today)

---

## Epic 5 — Auth & public API contracts

*System stories backing the guest shell — JSON clients, session rules, HTML redirects.*

### US-API-01 · Register successfully

**As a** prospective member submitting valid registration  
**I want** the server to create my account  
**So that** I can verify email and continue onboarding  

**Acceptance criteria**
- Valid email and password → `201 Created` with `user_id` in JSON body

**Tests:** `m1_public_api.rs` → *register_succeeds_and_returns_user_id*  
**Status:** CI (`cargo test`)

---

### US-API-02 · One account per canonical email

**As the** platform  
**I want** Gmail dot, plus-tag, and alias variants to collapse to one account  
**So that** duplicate registrations are blocked  

**Acceptance criteria**
- Second register with canonical duplicate of an existing address → `409 Conflict`

**Tests:** `m1_public_api.rs` → *register_rejects_duplicate_canonical_dup_vector*  
**Status:** CI

---

### US-API-03 · Block disposable email domains

**As the** platform  
**I want** throwaway email domains rejected at registration  
**So that** abuse and throwaway accounts are harder  

**Acceptance criteria**
- Register with a known disposable domain → `400 Bad Request` with disposable-domain message

**Tests:** `m1_public_api.rs` → *register_rejects_disposable_domain*  
**Status:** CI

---

### US-API-04 · Require charter and age acknowledgements

**As the** platform  
**I want** both adult and constitution acknowledgements on register  
**So that** consent is explicit before account creation  

**Acceptance criteria**
- Register without `constitution_ack` or without `adult_ack` → `400 Bad Request`

**Tests:** `m1_public_api.rs` → *register_requires_both_acknowledgements*  
**Status:** CI

---

### US-API-05 · Reject bots and invalid email

**As the** platform  
**I want** honeypot fields and malformed emails rejected  
**So that** automated signups fail  

**Acceptance criteria**
- Honeypot field populated → `400 Bad Request`
- Invalid email format → `400 Bad Request`

**Tests:** `m1_public_api.rs` → *register_rejects_honeypot_and_invalid_email*  
**Status:** CI

---

### US-API-06 · Reject wrong password

**As a** returning member  
**I want** wrong credentials to fail without issuing a session  
**So that** my account stays protected  

**Acceptance criteria**
- Wrong password for a verified account → `401 Unauthorized`, no session cookie

**Tests:** `m1_public_api.rs` → *login_rejects_wrong_password*  
**Status:** CI

---

### US-API-07 · Block login until email verified

**As a** new registrant  
**I want** login blocked until I verify my email  
**So that** unverified accounts cannot access the app  

**Acceptance criteria**
- Login with unverified account → `403 Forbidden`

**Tests:** `m1_public_api.rs` → *login_rejects_unverified_user*  
**Status:** CI

---

### US-API-08 · Successful login opens protected API

**As a** verified member  
**I want** login to issue a session that works on protected routes  
**So that** I stay signed in across requests  

**Acceptance criteria**
- Verified login → `200 OK` and session cookie set
- Authenticated GET `/account` → `200 OK` with `ok: true`

**Tests:** `m1_public_api.rs` → *login_issues_session_and_unlocks_protected_route*  
**Status:** CI

---

### US-API-09 · Protected API requires valid session

**As the** platform  
**I want** protected JSON routes to reject missing or invalid sessions  
**So that** private data stays private  

**Acceptance criteria**
- GET `/account` without cookie → `401` with `auth_required`
- GET with bogus session token → `401`

**Tests:** `m1_public_api.rs` → *protected_route_requires_session*  
**Status:** CI

---

### US-API-10 · Browser GET to protected pages redirects to login

**As a** guest using a normal browser  
**I want** protected HTML routes to redirect to login  
**So that** I am prompted to sign in instead of seeing raw JSON errors  

**Acceptance criteria**
- HTML GET `/home` or `/account` without session → redirect to `/login?login=1`
- JSON GET `/home` without session → `401` with `auth_required`

**Tests:** `m1_public_api.rs` → *protected_html_routes_redirect_to_login*  
**Status:** CI

---

### US-API-11 · Unverified session redirects to resend verification

**As a** registrant who has not verified email but has a session  
**I want** browser access to `/home` to send me to resend verification  
**So that** I know to check my inbox  

**Acceptance criteria**
- HTML GET `/home` with unverified session → redirect to `/login?resend=1`
- JSON GET `/home` with unverified session → `403` with `email_not_verified`

**Tests:** `m1_public_api.rs` → *protected_html_routes_redirect_unverified_session_to_resend*  
**Status:** CI

---

### US-API-12 · Guest login shell matches legal redirect contract

**As a** guest  
**I want** `/login` and canonical legal URLs to behave consistently  
**So that** public surfaces match what browser tests assert  

**Acceptance criteria**
- GET `/login` → `200` with guest auth shell marker in HTML
- GET `/terms`, `/privacy`, `/conditions`, `/safety` → redirect to `/login?legal={doc}`
- GET `/health/status` → `200 OK`

**Tests:** `m1_public_api.rs` → *public_pages_render_for_guests*  
**Status:** CI

---

## Narrative journeys

Each journey has a Playwright **`@journey-*` tag** on the specs that cover its steps. Run from `e2e/`:

| Journey | Tag | Command | Stories |
|---------|-----|---------|---------|
| A · Guest discovers platform | `@journey-a` | `npm run test:journey-a` | US-PUB-01 … 14 (subset) |
| B · Discover `@256` and join | `@journey-b` | `npm run test:journey-b` | US-PUB-12, 07, 03 |
| C · Returning member signs in | `@journey-c` | `npm run test:journey-c` | US-PUB-05, US-AUTH-01, US-HOME-* |
| D · Onboarding funnel | `@journey-d` | `npm run test:journey-d` | US-ONB-*, US-HOME-01 |
| E · Protected app boundary | `@journey-e` | `npm run test:journey-e` | US-PUB-01, 05 + API gate |
| All journeys | `@journey` | `npm run test:journeys` | Union of the above |

Specs also carry **`@us-*` tags** matching story IDs in this document (e.g. `@us-pub-01`).

Compose these chains for demo scripts or manual QA. Run stories in order; later steps assume earlier outcomes.

### Journey A · Guest discovers platform and joins *(CI-covered)*

| Step | Story | User-visible outcome |
|------|-------|----------------------|
| 1 | US-PUB-01 | Arrive at login from `/` |
| 2 | US-PUB-02 | See join, sign-in, constitution, footer |
| 3 | US-PUB-10 or US-PUB-11 | Read terms or safety before committing |
| 4 | US-PUB-03 or US-PUB-07 | Open registration (chooser or `?join=1`) |
| 5 | US-API-01 *(API)* | Account created *(not yet in browser test)* |

**Playwright today:** steps 1–4 via `public-pages.spec.ts` (`@journey-a`; discrete tests, not one chained spec).

---

### Journey B · Guest discovers `@256` and joins *(CI + marketing demo)*

| Step | Story | User-visible outcome |
|------|-------|----------------------|
| 1 | US-PUB-12 | View `/public/256` — profile card + CTAs |
| 2 | US-PUB-07 | Click Join In → register card |
| 3 | US-PUB-03 | Complete register form UI *(submit not automated)* |

**Playwright today:** steps 1–2 partially covered; step 3 stops at form visibility.

---

### Journey C · Returning member signs in *(local regression)*

| Step | Story | User-visible outcome |
|------|-------|----------------------|
| 1 | US-PUB-05 | Land on `/login?login=1` |
| 2 | US-AUTH-01 | Form login → `/home` feed |
| 3 | US-HOME-01 | Feed shell with profile complete |
| 4 | US-HOME-02 | Open private profile |
| 5 | US-HOME-04 | Private message shortcut *(manual QA)* |
| 6 | US-HOME-05 | Back/cancel scrolls feed to top *(manual QA)* |

**Playwright today:** US-AUTH-01 + US-HOME-01/02 via `form-login.spec.ts` + `home-full-user.spec.ts` (separate specs; requires seed + `DEV_AUTH_BYPASS` for steps 3–4 dev path). Steps 5–6 manual until Playwright backlog clears.

---

### Journey D · New member onboarding funnel *(local smoke)*

| Step | Story | User-visible outcome |
|------|-------|----------------------|
| 1 | US-API-01 | Register |
| 2 | US-API-07 | Cannot login until verified |
| 3 | US-ONB-01 | After verify, consent gate before feed |
| 4 | US-ONB-02 | Face onboarding before feed |
| 5 | US-HOME-01 | Full persona reaches feed |

**Playwright today:** US-ONB-01/02 and US-HOME-01 via seeded personas; register/verify not browser-automated.

---

### Journey E · Protected app boundary *(CI API + partial browser)*

| Step | Story | User-visible outcome |
|------|-------|----------------------|
| 1 | US-API-10 | Guest hitting `/home` → login redirect |
| 2 | US-PUB-05 | Sign-in panel from redirect |
| 3 | US-API-08 | Valid login unlocks session |
| 4 | US-HOME-01 | Feed shell |

**Playwright today:** steps 1–2 browser; steps 3–4 split across API and local smoke.

---

## Traceability matrix

| ID | CI | Test location |
|----|----|---------------|
| US-PUB-01 … 14 | Playwright public | `e2e/tests/smoke/public-pages.spec.ts` |
| US-HOME-01 … 05 | Local smoke / manual | `e2e/tests/smoke/home-full-user.spec.ts` (01–03); manual QA (04–05) |
| US-ONB-01 … 02 | Local smoke | `e2e/tests/smoke/onboarding-gates.spec.ts` |
| US-AUTH-01 | Local regression | `e2e/tests/regression/form-login.spec.ts` |
| US-API-01 … 12 | `cargo test` | `tests/m1_public_api.rs` |
| US-FEED-03 | `cargo test` | `tests/engagement_thread.rs` |

**CI gate (2026-06-06):** `cargo test` + `npm run test:public` → US-PUB-* + US-API-* + US-FEED-03 (Rust integration).

---

## Backlog — stories without automated UI tests yet

Use these as templates when extending Playwright or writing marketing copy for features documented in [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) but not yet in `e2e/`.

| Planned ID | Epic | Summary |
|------------|------|---------|
| US-FEED-01 | Feed | Compose a text + link post; see OG preview card |
| US-FEED-02 | Feed | Audience-scoped visibility (private vs public profile posts) |
| US-JUD-01 | Judiciary | File civic dispute from post; see under-review state |
| US-JUD-02 | Judiciary | Juror invite → vote → verdict threshold display |
| US-JUD-03 | Judiciary | Filing page shows read-only post preview (no comment/vote bar) — **shipped 2026-07-22**; promote to automated test |
| US-NOMS-01 | Noms | Stripe Civic/Standard/Steward pack purchase; processing spinner; success toast + balance refresh; wallet cap; cancel checkout — **shipped 2026-07-23** UI; **beta test-card E2E confirmed 2026-08-14**; production live keys + webhook TBD at launch |
| US-BLOCK-01 | Safety | Block human → hidden from feed and search |
| US-OPS-01 | Panoply | Operator emergency filing *(dev/staging only)* |
| US-HOME-04 | Profiles | Private message shortcut — **shipped 2026-07-22**; Playwright backlog |
| US-HOME-05 | Home shell | Back/cancel scroll-to-top — **shipped 2026-07-22**; Playwright backlog |

When a backlog story ships with a test, move it into a numbered epic above and link the spec file.

---

## Related

- [`LAUNCH_READINESS.md`](LAUNCH_READINESS.md) — QA scorecard; authenticated Playwright promotion backlog
- [`ONBOARDING.md`](ONBOARDING.md) — Server gates behind US-ONB-* stories
- [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) § User-facing surfaces — Guest login shell, public profiles
