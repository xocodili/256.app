# Trust & safety — roadmap and minimums

Consolidated trust, safety, and wellbeing guardrails for 256.app. **Governance fitness** (ratings, juries, death) and **platform safety** (illegal content, acute harm, minors) are **orthogonal** — see [`POLITICAL_SCIENCE_REVIEW.md`](POLITICAL_SCIENCE_REVIEW.md) §5, [`EVOLUTIONARY_BIOLOGY_REVIEW.md`](EVOLUTIONARY_BIOLOGY_REVIEW.md) §8, [`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md) §3, [`CLINICAL_PSYCHOLOGY_REVIEW.md`](CLINICAL_PSYCHOLOGY_REVIEW.md) §3.5.

**Status:** **Tier 1 + Tier 2 in-product** — Root charter **v2 shipped (2026-05-29)** — 16 points in DB/UI; **`safety_sanitation_event`** audit; media hash gate on face capture. **Panoply operator dashboard shipped (2026-05-29)** — `/home/ops` gives cleared operators an in-product acute-harm console (emergency/civic filing without Noms, vacate, eject/ban, session kill, domain block) with an audit log (`safety_operator_action`). **Ops backlog:** vendor hash API, hotline **submission** automation (16), notification mute. **Shipped:** Safety & Crisis legal card + integrated crisis panels ([`CRISIS_RESOURCES.md`](CRISIS_RESOURCES.md)). The SQL-only emergency runbook is now largely superseded by the operator console for on-network harm.

**Last reviewed:** 2026-06-09 (Google OIDC, reCAPTCHA v3, post-auth consent). Prior: 2026-06-03 (content model scoping; emergency flow aligned to code; crisis resources design).

---

## Product content model (hard rule)

Canonical product statement: [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) § Content model.

| Rule | Safety consequence |
|------|-------------------|
| **Link-and-text-only posts** | No user image/video attachments on posts; discussion happens in **text** + link cards (OG preview). Users are expected to **follow links off-site** to view source material. |
| **No persisted user uploads** except **generated avatars** | Class A/B **vendor hash** scope is **avatar ingress only** (`POST /me/face-capture`, paid redraw) — not “every upload path.” |
| **Transient face JPEGs** | Request-only; SHA-256 gate + optional vendor API **before** portraiture; raw bytes are not stored in `user_faces`. |

Posts do **not** host CSAM/NCII binaries. Emergency §4.1–§4.7 is about **on-platform text/links** (threats, doxxing, self-harm references, etc.). Class A/B hash work protects the **face pipeline**, not the feed.

---

## Emergency safety flow (canonical — matches code)

Root **§4.1–§4.7** (points **401–407**) → trigger **`emergency_safety`**. Implementation: `src/social/judicial/emergency.rs`, feed SQL `src/social/feed.rs` (`home_feed_audience_visibility_sql`), feed UI `src/ui/post_card/feed.rs`.

```text
File emergency (§4.1–§4.7)
  → entire thread post status = suppressed
  → judicial_case status = contained (no jury)
  → non-authors: root post DROPped from home/profile aggregate feeds
  → author: post still listed (pinned sort) with hold notice; body quarantined; contest CTA

Author does nothing for 3 UTC days (contested_at IS NULL)
  → job emergency_standing_settlement
  → Loathed-equivalent standing on author (−3×δ edge, −1×δ global)
  → post STAYS suppressed (not restored)

Author contests (POST …/contest, 1 Nom)
  → case contained → open; jury seats
  → post remains suppressed during jury
  → jury Overturned → thread restored (restore_post_thread_tx)
  → jury Upheld → platform_safety_ban_human_tx (blocked + graph death ritual)
```

**“Sudden death runoff”** in product terms = **suppress-first + optional contest + uncontested T+3 standing without jury** — not instant account deletion. Deletion/ban requires **contest + upheld jury** (or operator **eject** via Panoply).

**Civic dispute** (other clauses) differs: post **`under_judicial_review`**, jury **from filing**, same feed rule (**only author** sees quarantined post in aggregate feeds until resolved). Serious/regular **`no_jury`** clocks apply to **civic** cases only (7d / 21d).

Cross-ref: [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) § Emergency safety vs civic dispute.

---

## What exists today

| Capability | Status | Evidence |
|------------|--------|----------|
| Root Operating Charter | ✅ v2 shipped | 16 points — `migrations/20260529120000_*`; consent `v2-2026-05-29` |
| Universal Safety Floor (item 4) + §4.1–§4.7 | ✅ Live | Points **401–407** emergency filing; point **4** preamble; routing via 15/11/13 |
| Acute harm sanitation (item 15) | ✅ Foundation | `safety_sanitation_event`; **avatar-ingress** hash stub; domain deny audit — vendor API backlog (avatar only) |
| Mandatory reporting (item 16) | ✅ Text / 📋 ops | Charter in DB/UI; hotline API integration backlog |
| Emergency safety filing | ✅ Live | Trigger `emergency_safety`; post **`suppressed`**; case **`contained`**; contest opens jury (1 Nom) |
| Civic dispute filing | ✅ Live | Trigger `civic_dispute`; post **`under_judicial_review`**; jury from filing |
| Civic `no_jury` — serious tier | ✅ Live | **7-day** window on **civic dispute** when serious clause cited; post **`flagged`** |
| Civic `no_jury` — regular tier | ✅ Live | **21-day** window; post **`flagged`** |
| Emergency uncontested T+3 standing | ✅ Live | Job `emergency_standing_settlement`; post stays **`suppressed`** |
| Item 11 (legal process) | ✅ Not a dispute trigger | Operator-only per charter |
| Item 14 (dignity) | ✅ Outcome enforcement | Not selectable; filer **−6×δ** when jury clears post / author wins emergency contest |
| Juror depersonalized corpus | ✅ Live | `src/content/juror_anonymize.rs`, juror workspace |
| Post hidden while case open | ✅ Live | **`suppressed`** (emergency); **`under_judicial_review`** / **`flagged`** (civic) |
| Emergency author notification emails | ✅ Live (SMTP on beta + dev) | `src/email/notify/moderation.rs` |
| Domain blocklist for outbound links | ✅ Live | `content_domain` / `content_link`; § Outbound link domain policy below |
| User disconnect / end relationship | ✅ Live | `status::end_relationship`, private profile |
| **Human graph block** (signed-in profile, feed, search, mentions, connection rail) | ✅ Live | `src/social/blocks.rs` — blockee→blocker edge; **not** platform `user_block` |
| Block list manage / unblock | ✅ Live | `GET /me/block-list`, `POST /me/blocks/unblock`, self profile panel |
| Voluntary account / entity deletion | ✅ Live | `POST /me/account/delete`, `POST /me/entities/{handle}/delete`; voids open judicial cases |
| Human death at rating 0 | ✅ Live | `death_human.rs` — fitness extinction, not a safety tool |
| Honest “law enforcement not wired” copy | ✅ Filing card + juror workspace | `ui.juror_workspace.law_notice`; crisis callout on filing card |
| **Google OIDC sign-in** | ✅ Live | `user_oidc_identity`; PKCE + state cookie; password-gated link for existing email accounts |
| **reCAPTCHA v3** | ✅ Live | Register, login, OIDC start/link; `RECAPTCHA_MIN_SCORE` (default 0.5); bypass local loopback only |
| **Post-auth charter consent** | ✅ Live | Prevents OIDC bypass; 16+ attestation + Root charter on `/home?onboarding=consent` |
| Post-scoped filing surface | ✅ Live | `GET|POST /home/judicial/post/{post_id}`; feed cards expose a judicial CTA when eligible |
| Post-card judicial filing CTA (feed) | ✅ Partial | `src/ui/post_card/feed.rs` links eligible feed cards to `/home/judicial/post/{id}`; not on public cards; no pre-selected clause yet |
| **Panoply operator console** (`/home/ops`) | ✅ **Live** | `src/routes/panoply.rs`, `src/social/panoply/` — emergency/civic filing (Noms + daily-cap waived), vacate contained emergency, eject (platform safety ban), session kill, domain block; operator-only, audited |
| **Operator audit log** | ✅ Live | `safety_operator_action` table; every console action requires a written ops note (`src/social/panoply/audit.rs`) |
| **Operator clearance model** | ✅ Live | Invite-only `@panoply` community; cleared humans get ops dashboard + fitness-death immunity; `@panoply` concealed from non-operators (`src/social/panoply/conceal.rs`) |
| Operator emergency response (in-product) | ✅ Live | Console supersedes most of the SQL-only runbook for on-network harm; SQL runbook still relevant for pre-persist / legal preservation |
| Platform suspend / ban (operator) | ✅ Live | Eject = `platform_safety_ban_human_tx` (graph death ritual + `to_be_deleted`); session kill clears all sessions for the handle |
| Platform `user_block` (notifications, logged-out `/public/*`) | ❌ Partial | Graph block covers signed-in discovery; operator **eject** now covers hard suspend — see [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) § Human block list |
| CSAM / NCII hash matching (avatar ingress) | ⚠️ Partial | Face-capture + redraw JPEG SHA-256 stub (`MEDIA_HASH_BLOCKLIST`); vendor PhotoDNA-class API backlog — **no post uploads** |
| Minors / age gating | ⚠️ Partial | 16+ attestation at consent step + profile DOB; no standalone T&S policy page |
| Published safety policy (standalone) | ❌ Deferred | Consent step surfaces charter; no dedicated safety page |

**Do not rely on jury clocks alone for acute harm** (credible threats, doxxing, harassment, self-harm text). **Emergency** cases **suppress immediately** with **no jury until author contests**; uncontested cases get **T+3 standing** without seating jurors. **Civic** serious-tier cases use a **7-day** `no_jury` window. **Class A/B media** (avatar ingress only) must **never** reach jurors — hash gate before portraiture; emergency **text** contest may still seat a jury on depersonalized **text** (not applicable to blocked avatar bytes). **Tier 1 operator action** plus **operator reporting workflow (16)** remain required for public launch. User-facing crisis numbers: **shipped** — [`CRISIS_RESOURCES.md`](CRISIS_RESOURCES.md) (`config/crisis_resources.en.json`, Safety & Crisis card, filing/emergency surfaces).

---

## Authentication & signup abuse (shipped 2026-06-09)

Canonical detail: [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) § Authentication & abuse prevention.

| Control | Purpose |
|---------|---------|
| **reCAPTCHA v3** | Bot/abuse friction on register, login, OIDC start, OIDC link |
| **Rate limits** | Shared auth throttles on login, register resend, forgot-password, OIDC routes |
| **Post-auth consent** | Charter + age attestation after session — closes OIDC bypass |
| **Password-gated OIDC link** | Prevents account takeover when Google email matches existing password account |
| **PKCE + signed state cookie** | CSRF / code-interception hardening on Google callback |
| **`TRUSTED_PROXY=1`** | Use `X-Forwarded-For` for reCAPTCHA `remoteip` only behind a trusted reverse proxy |
| **Startup guards** | `DEV_AUTH_BYPASS` and `RECAPTCHA_BYPASS` refuse non-loopback `HOST` |

**Backlog:** reCAPTCHA on resend-verification and forgot-password; Apple OIDC.

---

## Safety model — three response tiers

```text
Tier 1 — Acute harm (minutes–hours)
  Operator emergency action + preserve evidence + notify if legally required
  Examples: CSAM, credible violence threats, doxxing, imminent self-harm

Tier 2 — Platform policy harm (hours–days)
  Report queue → operator or trained reviewer → warn / restrict / remove content / suspend
  Examples: harassment campaigns, hate speech, scams, non-consensual imagery

Tier 2b — Automated outbound-link policy (immediate at ingest)  ← content_domain denylist
  Known-bad registrable domains blocked before link attach / OG fetch
  Examples: malware/phishing hosts, operator-maintained toxic platforms

Tier 3 — Governance fitness (days–weeks)
  Existing judiciary — civic disputes, contested emergency juries, membership review, amendments
  Examples: constitution violations, community norm disputes, civic standing
```

Tier 3 is **mature** (civic dispute + contested emergency juries). Tier 1 now has an **in-product operator console** (Panoply `/home/ops`) plus the audit log — **closed-beta on-call shipped** ([`OPS_ON_CALL.md`](OPS_ON_CALL.md)); **vendor hash on avatar ingress** and **operator CyberTipline workflow (16)** remain gaps. User-facing crisis registry: **live** ([`CRISIS_RESOURCES.md`](CRISIS_RESOURCES.md)). Tier 2 is **substantially live** — **remaining gaps:** platform notification mute.

---

## Panoply operator console (shipped 2026-05-29)

**Panoply** is the platform safety operator subsystem — the in-product realization of Tier 1/Tier 2 operator response that earlier drafts of this doc tracked as "SQL runbook only" and "operator dashboard missing".

**Clearance model**

- Invite-only `@panoply` community (`migrations/20260528150100_panoply_ops.sql`). Active verified `member_of @panoply` grants **ops dashboard access** + **fitness-death immunity** (rating-zero and login-idle only — not immunity from eject or legal process).
- `@panoply` and its roster are **concealed** from non-operators in search, public profiles, and private profile views (`src/social/panoply/conceal.rs`); operators present as ordinary citizens elsewhere. `@panoply` is also rejected as a community parent.
- Seed operator: `operator@example.com` / human `@sphinx` (structural founder, fully verified, skips onboarding funnel — migrations `20260529150000`, `20260529160000`).
- 10-point **Panoply charter** (`migrations/20260529130000_panoply_constitution.sql`) demarcates operator mandate, two-plane conduct, audit obligation, proportionality, and legal-process cooperation. Descriptive only — **not** judicial triggers in v1; Panoply-on-Panoply enforcement is deferred.

**Console (`GET|POST /home/ops`, `src/routes/panoply.rs`)** — operator-only (404 to everyone else):

| Action | Effect | Code |
|--------|--------|------|
| **Emergency file** | File `emergency_safety` on a post citing §4.1–§4.7; post `suppressed`; case `contained`; **Noms + 3/day cap waived** | `panoply::file_emergency_safety` |
| **Civic file** | File `civic_dispute` citing other clauses; post `under_judicial_review`; jury enqueued; Noms waived | `panoply::file_civic_dispute` |
| **Vacate emergency** | Void an uncontested `contained` emergency case and restore the thread | `panoply::vacate_emergency_case` |
| **Eject** | Platform safety ban a human by handle (graph death ritual + `to_be_deleted`); respects death immunity | `panoply::eject_human_by_handle` |
| **Session kill** | Delete all sessions for a handle (force re-auth) | `panoply::kill_sessions_for_handle` |
| **Domain block** | Set a registrable domain `blocked` + `unsafe` in `content_domain` | `panoply::block_domain` |

Plus read-only queues: **contained emergencies awaiting contest** and **recent operator actions**. Every action **requires a written ops note** and writes a `safety_operator_action` row (actor, action type, post/user/domain/case targets, note, timestamp).

**Topbar:** cleared operators see a `/home/ops` button (`panoply::topbar_ops_button_html`).

**Tests:** `tests/panoply_ops.rs` — clearance, death immunity on eject, Noms-waived emergency filing, duplicate rejection, concealment from non-operators, charter seeding, parent-search exclusion.

**Still backlog after Panoply:** multi-operator roster + operator revocation/discipline (v1 trusts the roster); vendor hash API on **avatar ingress** (`read_face_capture_upload`); operator CyberTipline workflow (16); notification-mute primitive.

**Deferred until revenue / meaningful users (rollback plan):** paid vendor hash (PhotoDNA-class), automated CyberTipline submission, and commercial moderation SaaS — **not required for zero-user beta**. In-app crisis hotlines (Safety & Crisis card, judicial/emergency surfaces, optional `/crisis-help`) and manual Panoply ops ship now; revisit vendor spend when operating income supports it ([`LAUNCH_READINESS.md`](LAUNCH_READINESS.md) rollback note, [`DEPLOY_BETA.md`](DEPLOY_BETA.md)).

---

## Emergency actor matrix (due diligence — not flags alone)

Aligns with Root **v2** points **15** and **16**. Ops annex — vendor names and SLAs live outside Root text.

| Emergency | Harm class (v2) | Primary actor (must act) | External actor | Human at 256.app views raw media? |
|-----------|-----------------|--------------------------|----------------|-----------------------------------|
| **CSAM / exploitation imagery** | A | Automated hash gate + platform automation | National CSAM hotline (CyberTipline-class / INHOPE) → LE | **Never** |
| **Active NCII spread** | B | Hash / victim-hash gate + platform | Jurisdiction-specific programs; counsel | **Never** (hash path) |
| **Imminent credible violence** | C | Platform on-call (contain) + preserve logs | Local emergency services / LE per counsel | Redacted text only |
| **Active doxxing + safety nexus** | C | Platform on-call (remove PII) | LE only if bundled with imminent threat | Redacted PII review OK |
| **Self-harm crisis** | Tier 1 (not item 4) | Platform (crisis resources) | Crisis lines (988-class); not jury | Text triage only |

**Not emergency response** (Tier 2 queue or Tier 3 civic jury): scams, malware, non-acute harassment, disputed non-imminent threats, community relevance.

**Scope note (link-and-text product):** Emergency **contest** may seat jurors on **depersonalized post text** — that is correct for §4.x **text** disputes. **Class A/B** applies to **avatar JPEG ingress** only: matched bytes must not be portraited or stored; they never enter jury corpora. There is **no** post-image upload path. Gap: vendor hash API beyond env blocklist; optional scan of **generated PNG** before `write_generated_image`.

---

## Outbound link domain policy

**Role in the safety model:** **Tier 2b automated prevention** — not acute harm response (Tier 1), not civic moderation (Tier 3). Supports Root **item 4** prohibitions on **malware, phishing, and fraud/scam destinations** and v2 **point 15** containment for known-toxic **outbound URLs**. It does **not** replace hash gates (Class A/B media), operator on-call (Class C text), or post disputes.

### Data model

Registry table **`content_domain`** (registrable eTLD+1, e.g. `4chan.org`) plus canonical rows in **`content_link`**. Migration: [`migrations/20260416100000_content_links_foundation.sql`](../migrations/20260416100000_content_links_foundation.sql).

| Column | Purpose |
|--------|---------|
| `status_id` | Trust tier (`status` ref group): `pending_verification` (default), `flagged`, `blocked`, `verified` |
| `safety_state` | `pending` \| `safe` \| `suspicious` \| `unsafe` |
| `rating` | Internal 0.00–5.00 (ecosystem signal; not jury-driven today) |
| `safety_note` | Operator annotation (e.g. seed reason) |

**Enforcement rule** ([`src/content/links.rs`](../src/content/links.rs) `registrable_domain_is_blocked`): block when **`status = blocked`** OR **`safety_state = unsafe`**.  
`suspicious`, `pending`, `safe`, and `flagged` status **do not** block in code today.

### Where enforcement applies (live)

| Surface | Behavior |
|---------|----------|
| Link ingest API | Reject — `social.domain_blocked_for_post` |
| OG preview (`preview_content_link_og`) | `domain_blocked: true`; **no outbound HTTP fetch** |
| Post create with attached `content_link_id` | Re-check domain at publish |
| Entity profile URLs (website, logo, event) | Block at canonicalize via `optional_brand_url_to_content_link_id` |
| Plain-text URL in post body (no attach) | **Not checked** — gap |
| User-uploaded post images | **Not applicable** — link-and-text-only product |
| Avatar face JPEG ingress (CSAM path) | **`media_hash_gate`** at face capture / redraw — domain table irrelevant |

### Seeds & operator maintenance

| Seed | Migration | Effect |
|------|-----------|--------|
| **Denylist example** | `20260417123000_seed_blocked_domain_4chan.sql` | `4chan.org` → `blocked` + `unsafe` |
| **Trusted domains** | `20260416104500_seed_trusted_content_domains.sql` | ~256 domains at `verified` — positive signal for council-content plans; **not** a post gate |

**Today:** operator updates via SQL/migration (`UPDATE content_domain SET …`). **No ops UI**, **no vendor feed sync**, **no sanitation audit log row** on block (v2 backlog).

### Mapping to Root charter v2

| Item 4 harm | Domain denylist |
|-------------|-----------------|
| Malware / phishing | **Primary fit** — block known bad hosts at link boundary |
| Fraud / scams | Partial — only listed domains |
| CSAM / NCII | Wrong tool — **avatar** hash gate (Class A/B); not post attachments |
| Threats / doxxing in text | Not URL-based |
| Harassment (on-platform) | Not domain-based |
| Coordinated brigading via links | Partial — if coordination host is listed |

Aligns with **point 15** as **automated outbound-link containment** (subset of Class C for malware/phishing URLs), **orthogonal** to civic moderation (**point 13**) and hotline reporting (**point 16**).

### Gaps (backlog)

- [x] Block **inline body URLs** at post create / sanitize — shipped 2026-08-12 (`src/content/url_extract.rs`, `domain_reputation::ensure_inline_body_urls_allowed`)
- [ ] **`suspicious` → `unsafe`** operator workflow (schema exists; no product queue)
- [ ] Vendor phishing/malware feed → auto-`unsafe`
- [x] Log domain blocks in **`safety_sanitation_event`** (shipped 2026-05-29)
- [ ] Ops UI or documented SQL runbook for domain tier changes

Spec cross-ref: [`ROOT_CONSTITUTION_PROPOSAL.md`](ROOT_CONSTITUTION_PROPOSAL.md) § Point 15 (outbound links), [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) § Outbound links & domain policy.

### P0 — Must ship before open signup

#### 1. In-product report / civic post dispute

**Shipped (2026-05-28 — greenfield emergency + civic):** see **Live today** block below. Supersedes unified `post_moderation` filing for new cases.

**Shipped (2026-05-27 — charter v1 baseline):** Root 14-point charter in DB (superseded by v2 text 2026-05-29); `moderation_tier_id` on constitution items; civic serious/regular clocks.

**Shipped (2026-05-29 — charter v2 foundation):** 16-point Root text in DB/UI; consent **`v2-2026-05-29`**; **`safety_sanitation_event`** audit; face-capture SHA-256 hash gate stub (`MEDIA_HASH_BLOCKLIST`); domain-deny audit logging. See [`ROOT_CONSTITUTION_PROPOSAL.md`](ROOT_CONSTITUTION_PROPOSAL.md).

**Target routing (v2 — ops backlog):**

- **Class A/B (15):** no post dispute / no jury — hash gate + sanitation audit + **16** reporting
- **Class C (15):** hide first; operator on-call; optional civic follow-up on redacted record
- **Non-acute item 4 (13):** post dispute unchanged for disputed threats, scams, harassment patterns

**Live today (greenfield emergency + civic):**

- **Emergency safety** (`emergency_safety` trigger): §4.1–§4.7 clauses; post **`suppressed`** on file; no jury until author **contests** (1 Nom); uncontested T+3 standing job; contest loss → platform **`blocked`** ban
- **Civic dispute** (`civic_dispute` trigger): other charter clauses; **`under_judicial_review`** + jury from filing
- **3 emergency filings / filer / UTC day**; **one emergency per post**
- Filing card crisis callout + **Report to law enforcement** (`templates/judicial_post_case.html`)
- Job `emergency_standing_settlement` on scheduler tick

**Still to ship:**

- Vendor hash API (PhotoDNA-class) on **avatar ingress** (face capture + redraw; optional generated PNG scan)
- Operator CyberTipline / INHOPE reporting workflow (16) — not user-dialed
- User crisis registry — [`CRISIS_RESOURCES.md`](CRISIS_RESOURCES.md)
- Post card overflow for **non-acute** civic disputes (not Class A/B)
- Class C operator on-call queue UI
- Inline body URL denylist at post create
- Integration tests: Class A/B never seat jurors; Class C contain-before-jury

**Alternative deferred:** standalone `POST /api/report` + `safety_report` table — rejected; unified path is canonical.

#### 2. Operator emergency response (Panoply console + SQL)

**Shipped (2026-05-29 — Panoply):** `/home/ops` lets a cleared operator open emergency/civic cases as a system actor on the existing `judicial_case` tables, vacate uncontested emergencies, eject (platform ban), kill sessions, and block domains — all audited in `safety_operator_action`. This is the canonical operator path for **on-network** harm and supersedes the SQL-only runbook for those cases. See § Panoply operator console.

**Still needed:**

- **SQL runbook** for legal-hold preservation and bulk operations the console does not expose — documented for ops; not a normative product route (no post media uploads)
- Auto-enqueue scope recalc when console visibility changes (verify on eject/suppress paths)
- Operator **roster expansion + on-call rotation** (v1 ships a single structural operator)

**Do not ship:** `POST /ops/safety/takedown` as a parallel moderation constitution — the console reuses the judicial tables and Root clauses by design.

#### 3. Published safety policy

**Scope:** Safety & Crisis legal card (footer link), judicial filing / emergency surfaces, optional `/crisis-help` (see [`ONBOARDING.md`](ONBOARDING.md) §Trust & safety).

**Must cover:**

- What Tier 1 vs Tier 2 vs Tier 3 mean (jury ≠ emergency response)
- Prohibited content (illegal, threats, harassment, CSAM, doxxing)
- How to report; expected response times (targets, not guarantees)
- Operator override authority (honest “13th council member” framing from [`SYSTEM_THEORY.md`](SYSTEM_THEORY.md) §5)
- Face data and AI processing pointer to privacy policy
- **Minors:** 16+ attestation at consent step; profile DOB validation (`MIN_AGE_YEARS = 16`); stronger verification if regulations require

#### 4. Block / mute primitives

**Scope:** User-controlled harm reduction without requiring jury.

**Live today (signed-in):**

- **Human graph block** — `src/social/blocks.rs`: `status.blocked` on human connection edge (blockee → blocker)
- Private profile + profile feed hidden both ways; home feed excludes blocked authors; thread comments from blocked authors hidden; connection rail omits blocked peers
- Entity search (`/social-entities/search`) and community invite search omit blocked humans
- @mention typeahead and persisted mentions omit blocked peers
- Manage list: `GET /me/block-list`, `POST /me/blocks/unblock`, profile Block CTA

**Still outside graph block:**

- Logged-out **`/public/{handle}`** (no session-scoped filter)
- **Notification** suppression (no platform `user_block` yet)

**Still needed for public launch:**

- Optional **`user_block`** table if product wants mute independent of graph edges
- Notification suppression on read/delivery paths
- Distinct from **disconnect** (ends relationship) and **fitness death** (graph extinction at rating 0)

Psychology rationale: victims of harassment need **immediate agency**, not a 21-day civic process ([`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md) §3). Graph block now covers the core signed-in discovery paths; platform notification mute remains a gap.

#### 5. Closed-beta operator on-call — **shipped (2026-08-12)**

Primary on-call: human **@sphinx** (seed Panoply operator). Console: `/home/ops`. Public contact: `PLATFORM_SAFETY_CONTACT_EMAIL` on Safety legal card. **24h best-effort** response aim for closed beta (not a legal SLA). Full roster: [`OPS_ON_CALL.md`](OPS_ON_CALL.md). Multi-operator rotation remains backlog.

---

## P1 — Ship within first public quarter

#### 6. Report triage dashboard (ops)

- Queue sorted by severity heuristic (reason enum + keyword flags)
- One-click: hide post, suspend user, escalate to law-enforcement template
- Link report → existing judicial case when issue is Tier 3 (optional file post dispute on behalf of reporter)

#### 7. Automated CSAM / NCII signals (avatar ingress only)

- **Hash matching** (PhotoDNA or vendor API) on **face-capture / redraw JPEGs** before portraiture — **Class A/B (v2 point 15)**; no human review of matched media; no post upload surface
- Victim-hash workflows for NCII (StopNCII-class) where applicable
- **Outbound link denylist** — extend `content_domain` via operator SQL, vendor feeds, or ops UI; already live for attach/OG paths — see § Outbound link domain policy
- URL blocklist expansion beyond current domain table
- Keyword/ML pre-flag for posts — **human review before action** on redacted text only; do not auto-delete fitness ratings
- **Do not** route Class A/B to AI Council voting or human juries

#### 8. Juror wellbeing guardrails

- Case content warnings when dispute involves slurs, violence, or sexual content
- Expand **decline** copy — hard conflict vs. “I cannot review this safely”
- Consider **exempting** jurors who filed a Tier 2 report on the same subject from nomination
- Do not penalize abstention when operator marks case as `sensitive`

#### 9. Rate limits and anti-abuse on standing system

- Report false-positive tracking; malicious mass-reporting → operator review, not auto-rating penalty
- Sock-puppet detection hooks (already partial via connection cap + jury exclusions)
- Watch heartbeat gaming ([`SYSTEM_THEORY.md`](SYSTEM_THEORY.md) §3)

#### 10. Recovery and absence grace (wellbeing safety)

Not traditional T&S, but reduces **psychological harm** from standing system:

- **Inactivity pause** — one 90-day pause per year (account setting) freezes decay; requires explicit opt-in before idle threshold
- **Failing-band recovery checklist** in profile — mirror `estimated_deletion_at_from_last_login` for path back to ≥1.0
- Email copy review — reduce shame framing; add actionable steps

See [`DESIGN_BIAS_RECOVERY.md`](DESIGN_BIAS_RECOVERY.md), [`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md) §2.

---

## P2 — Mature platform safety

#### 11. Age verification / minors policy

- **16+ attestation** at post-auth consent (`ui.register.adult_confirm`) plus profile DOB gate (`MIN_AGE_YEARS = 16`); stronger verification if regulations require
- Restrict jury service for new accounts (< N days verified)
- No ad personalization for minors if youth access ever allowed

#### 12. Transparency reporting

- Periodic summary: reports received, actioned, median response time, appeals
- Aggregate jury outcomes — not individual cases

#### 13. Law-enforcement request process

- Replace `law_notice` placeholder with documented legal process page
- Preserve-evidence workflow tied to `safety_operator_action` audit log

#### 14. External trust & safety vendor evaluation

- For scale: contract CSAM scanning, crisis escalation partners (e.g. suicide hotline resources in UI)

---

## What not to do

| Anti-pattern | Why |
|--------------|-----|
| Route CSAM/threat reports into jury queue only | Re-traumatizes jurors; wrong SLA |
| Auto-apply Loathed / rating penalties on report | Guilt before due process; gaming vector |
| Use fitness death as moderation ban | Irreversible; conflates Tier 2 with Tier 3 |
| Promise “community self-polices” in marketing without Tier 1–2 | Misleading; see [`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md) |
| Add posting Nom costs before recovery UX widens | Punitive stack without upward path ([`DESIGN_BIAS_RECOVERY.md`](DESIGN_BIAS_RECOVERY.md)) |

---

## Launch checklists (safety slice)

### Closed beta

- [x] Operator on-call contact published to beta users — Safety legal card + `PLATFORM_SAFETY_CONTACT_EMAIL` (2026-08-12); see [`OPS_ON_CALL.md`](OPS_ON_CALL.md)
- [ ] SQL + judicial emergency runbook tested (hide pre-publish; operator case filing)
- [ ] Beta consent copy: jury timelines not for emergencies; use local emergency services; graph block explained
- [ ] `law_notice` reviewed — accurate for beta

### Public launch (add to [`LAUNCH_READINESS.md`](LAUNCH_READINESS.md))

- [x] Emergency + civic post-scoped filing — shipped 2026-05-28 (`emergency_safety`, `civic_dispute`)
- [x] Root charter v2 text (16 points) + re-consent — shipped 2026-05-29
- [x] Sanitation audit log (`safety_sanitation_event`) — shipped 2026-05-29
- [x] Pre-persist hash gate stub (face capture; env blocklist) — shipped 2026-05-29
- [x] Operator console for on-network acute harm (eject, session kill, suppress, domain block) — shipped 2026-05-29 (Panoply `/home/ops`)
- [x] Operator action audit log (`safety_operator_action`) — shipped 2026-05-29
- [ ] Vendor hash API on avatar ingress; Class A/B bytes never portraited/stored
- [ ] Operator CyberTipline workflow (16)
- [x] User crisis registry ([`CRISIS_RESOURCES.md`](CRISIS_RESOURCES.md)) — Safety & Crisis card + filing/emergency panels
- [ ] Operator **roster + on-call rotation** (v1 = single structural operator) and pre-persist SQL runbook
- [ ] Safety policy page published (tiers 15/13/11 explained) — `/safety` redirect + legal-card copy live; dedicated page still pending
- [ ] Platform notification mute beyond graph edge — **partial:** graph block (signed-in discovery) + operator eject (hard ban) live; notification suppression pending
- [ ] Legal review: face data, reports retention, CSAM duty of care
- [ ] Response-time targets documented
- [ ] Minors statement (exclude or gate)

---

## Metrics (ops dashboard + daily operator mail)

The Panoply console exposes live action and contained-emergency queues. A **daily status email** (`panoply_daily_status` job) goes to all verified `@panoply` operators — see [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) § Panoply daily status report (entity/judiciary/post rollups + operator actions, contained emergencies, sanitation, DAU).

| Metric | Purpose |
|--------|---------|
| Entity new / retired / active (by type) | Graph growth & extinction |
| Judicial opened / resolved / unresolved | Civic backlog |
| Post new / resolved / active disputed | Content + moderation load |
| `safety_operator_action` / day | Operator console utilization |
| `safety_sanitation_event` / day | Tier-2 prevention volume |
| Contained emergency queue depth | Acute harm SLA |
| DAU / sign-ins | Incident rate denominator |
| Juror abstention / decline rate | Civic burden monitor |
| Failing → healthy transitions | Wellbeing recovery ([`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md)) |

---

## Related docs

| Doc | Relevance |
|-----|-----------|
| [`LAUNCH_READINESS.md`](LAUNCH_READINESS.md) | Launch gates; T&S hard blocker |
| [`SYSTEM_THEORY.md`](SYSTEM_THEORY.md) | §9 backlog pointer |
| [`CONSTITUTIONAL_SAFETY_FRAMEWORK.md`](CONSTITUTIONAL_SAFETY_FRAMEWORK.md) | Root charter safety layers, clause triggers |
| [`POLITICAL_SCIENCE_REVIEW.md`](POLITICAL_SCIENCE_REVIEW.md) | Legitimacy without security functions |

---

## Changelog

| Date | Change |
|------|--------|
| 2026-06-11 | Panoply daily status report spec added to PRODUCT_GUIDE; metrics table aligned to planned operator rollup mail |
| 2026-06-03 | Content model (link/text + avatar-only storage); emergency flow section; hash scope narrowed; crisis resources design cross-ref |
| 2026-05-30 | **Panoply operator console** reconciled into doc — `/home/ops` (emergency/civic filing, vacate, eject, session kill, domain block), `safety_operator_action` audit, clearance/concealment model, immunity; status raised to Tier 1+2 in-product; checklist + P0 §2/§4 updated; forgot-password noted |
| 2026-05-29 | Root charter v2 shipped; sanitation audit + hash gate stub live; launch checklist updated |
| 2026-05-28 | Greenfield emergency + civic moderation shipped; “what exists” table, P0 §1, launch checklist aligned |
| 2026-05-28 | § Outbound link domain policy — Tier 2b; enforcement surfaces, gaps, v2 mapping |
| 2026-05-28 | v2 charter proposed (15 sanitation, 16 reporting); emergency actor matrix; Class A/B jury exclusion |
| 2026-05-27 | Root charter v1 shipped; unified post dispute + serious/regular tiers; “what exists” and P0 §1 updated |
| 2026-05-24 | Initial consolidated T&S roadmap — psychology-informed safety tiers and prioritized build list |
