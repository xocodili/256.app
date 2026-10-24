# System Theory — ecosystem viability & incomplete features

Living **backlog and triage** notes from an evolutionary-biology / systems-theory review of 256.app’s social graph, judiciary, and lifecycle jobs. For the full peer-review-style ecological analysis, see [`EVOLUTIONARY_BIOLOGY_REVIEW.md`](EVOLUTIONARY_BIOLOGY_REVIEW.md). Use this file when planning feed work, post moderation UX, metrics, trust & safety, and wellbeing guardrails.

**Related implementation docs:** [`LAUNCH_READINESS.md`](LAUNCH_READINESS.md) (product launch assessment & checklists), [`EVOLUTIONARY_BIOLOGY_REVIEW.md`](EVOLUTIONARY_BIOLOGY_REVIEW.md) (ecological/evolutionary analysis), [`POLITICAL_SCIENCE_REVIEW.md`](POLITICAL_SCIENCE_REVIEW.md) (comparative governance & legitimacy), [`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md) (wellbeing & civic-burden analysis), [`CLINICAL_PSYCHOLOGY_REVIEW.md`](CLINICAL_PSYCHOLOGY_REVIEW.md) (clinical harm assessment & publication ethics), [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) (safety roadmap), [`COMMUNITY_GRAPH_MANAGEMENT.md`](COMMUNITY_GRAPH_MANAGEMENT.md), [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md), [`AGENTS.md`](../AGENTS.md) (graph lifecycle, judiciary, Noms).

**Last triaged:** 2026-06-30 (closed beta live; priority snapshot refreshed). **Launch review:** [`LAUNCH_READINESS.md`](LAUNCH_READINESS.md) (2026-06-11). **Scholarly reviews:** codebase pass 2026-06-30 — [`EVOLUTIONARY_BIOLOGY_REVIEW.md`](EVOLUTIONARY_BIOLOGY_REVIEW.md), [`POLITICAL_SCIENCE_REVIEW.md`](POLITICAL_SCIENCE_REVIEW.md) (full rewrite), [`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md), [`CLINICAL_PSYCHOLOGY_REVIEW.md`](CLINICAL_PSYCHOLOGY_REVIEW.md), [`ADVERSARIAL_USER_REVIEW.md`](ADVERSARIAL_USER_REVIEW.md).

**Quint framing:** [`EVOLUTIONARY_BIOLOGY_REVIEW.md`](EVOLUTIONARY_BIOLOGY_REVIEW.md) = ecological/evolutionary analysis; [`POLITICAL_SCIENCE_REVIEW.md`](POLITICAL_SCIENCE_REVIEW.md) = comparative politics & legitimacy; [`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md) = cyberpsychology / collective wellbeing research; [`CLINICAL_PSYCHOLOGY_REVIEW.md`](CLINICAL_PSYCHOLOGY_REVIEW.md) = clinical harm assessment & publication ethics; **this file** = incomplete features & build priorities. [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) consolidates Tier 1–2 safety work. Product canon: [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md).

**Owner reminder — punitive bias:** Conversation settlement is **symmetric** in code (±3×δ edge / ±1×δ global). The residual skew is **removal > renewal in UX** — thin recovery explainability, inactivity-as-unfitness, death at zero — not Loathed math. Read [`DESIGN_BIAS_RECOVERY.md`](DESIGN_BIAS_RECOVERY.md) before further tuning.

---

## What is already load-bearing

Do not rebuild these; they are the metabolic layer the items below plug into:

- Scalar **fitness** (`social_entity.rating`, community `member_of` edge ratings); failing band **`rating < 0.66`** (`failing_rating_threshold()`).
- **Failing presentation** — **community** red `@handle` public; **human** failing styling **owner-only** (2026-06-10).
- **Conversation settlement** — symmetric Loved/Loathed **±3×δ** edge / **±1×δ** global direct (`src/social/judicial/types.rs`).
- **Inactivity pruning** — **human login-idle only** (phased daily decrements from day 28 idle; warning at 21; **login refund** of debited standing in `record_user_login`). Community post-idle penalties **removed**.
- **Death at rating zero** (community reparent + human `to_be_deleted`; scope invalidation).
- **Death-immune keystones** (council domains, charter roots, `@panoply` operators, system/AI handles).
- **Judiciary** — emergency suppress-first + civic dispute; multi-trigger; √n juries; weighted votes; refill; council appeal on civic disputes; **quota-free abstention** on acute/serious cases (2026-06-10).
- **Root charter v2** — 16 points; consent `v2-2026-05-29`.
- **Panoply operator console** — `/home/ops` + `safety_operator_action` audit (2026-05-29).
- **Auth & onboarding** — email + **Google OIDC**, reCAPTCHA v3, password reset, **32 Nom** profile-creation grant (+1 daily login, +3 jury duty), face capture, godmother.
- **Human Dunbar cap** — 256 distinct active connections (`connections::MAX_CONNECTION_SLOTS`); enforced on relationship create.
- **Human graph block** — signed-in feed/profile/thread/search filter (`src/social/blocks.rs`).
- **Scoped read** — `entity_read_scope` + live view fallback.
- **Council participatory activity** — godmother welcome, reply tick, portfolio link shares, invite tick (2026-06).
- **Closed beta host** — `beta.twofivesix.app`; test-gated CI/CD (2026-06-05).

---

## Incomplete features (backlog)

Status legend: **planned** (direction set), **deferred** (acknowledged, no schedule), **open** (needs design).

### 1. Closed enforcement loop — post verdict outcomes on the feed

**Status:** **shipped (2026-06-30)** — durable `@256` verdict notices on the home feed for human jury, Council appeal, `no_jury`, and emergency author outcomes.

**Gap (closed):** Judiciary resolves cases; users now see durable outcomes on posts—not only `under_judicial_review` while open.

**Implemented:** `src/ui/post_card/feed.rs` (`PostVerdictFeedMode`) — removed stub cards use the same `@256` structured notice as exonerated banners; modes distinguish human jury, Council of 12 (appeal), `no_jury`, and emergency standing/contest (author-only). Copy: `ui.post_verdict.*` in `locales/en.json`.

**Direction (remaining polish only):** manual QA on beta with seeded verdict states (~375px width); jury-weight primer at summons remains a separate task.

**Out of scope for this note:** Full `anathema` product beyond moderation status (see [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) — repercussions still intentionally thin at launch).

---

### 2. Recovery ecology — earning rating back

**Status:** **partially implemented** — `conversation_rating_settlement` (3d idle, ≥3 votes; Loved/Loathed **symmetric ±3×δ** edge / **±1×δ** global); juror participation **+3×δ**; **login refund** of inactivity debits coded. User-facing “how do I recover?” UX remains **planned** (P0 for closed beta quality).

**Owner note:** Independent reviews converge on **removal > renewal in UX**, not conversation math. Spillover and absence penalties were **rebalanced 2026-05-24** — see [`DESIGN_BIAS_RECOVERY.md`](DESIGN_BIAS_RECOVERY.md).

**Gap:** **Inactivity** is the dominant coded downward force while away; Loathed threads can still hurt when posting. No clear user-facing recovery loop; login refund not surfaced.

**Implemented (2026-05-24 review; updated same day):** `src/social/conversation_rating.rs`, `src/auth/session.rs::record_user_login`, `src/social/judicial/rating.rs` (symmetric spillover, +3×δ juror reward). See [`EVOLUTIONARY_BIOLOGY_REVIEW.md`](EVOLUTIONARY_BIOLOGY_REVIEW.md) §2.

**Direction (remaining):**

- Next **rating change** tied to **quality of post content** (not just activity timestamps).
- Connect to feed/post build: rehabilitation should feel like **participation**, not grinding logins.
- UX still needed later: failing → healthy checklist, copy for global vs edge rating (see §8).

---

### 3. Signal integrity — anti-gaming of activity clocks

**Status:** **partially addressed by economics**; no separate build scheduled.

**Gap:** Cheap heartbeat posts or login-only resets could game inactivity clocks.

**Product stance (2026-05-19):**

- **Branded post Nom costs** (profit 4 / non-profit & gathering 2) are **live** on `create_post`; human posts remain free. Unaffordable branded options appear in the Post as picker with red cost and cannot be selected.
- **3 Noms to moderate** (pricing schedule) remains **not wired** — see [`LAUNCH_READINESS.md`](LAUNCH_READINESS.md).
- **Sock-puppetry** remains possible; **Google OIDC + 32 Nom startup grant** lowers multi-account cost — top adversarial path is **emergency filing** ([`ADVERSARIAL_USER_REVIEW.md`](ADVERSARIAL_USER_REVIEW.md)).
- **P1 for closed beta:** emergency-filing pattern monitoring (ops); no multi-account detection in code yet.
- Revisit Nom post/moderation costs if heartbeat abuse appears; no dedicated anti-gaming sprint until then.

---

### 4. Population caps & org lifecycle — digital vs human

**Status:** **intentional asymmetry**; profit/NPO death **open**.

**Current design:**

- **Humans:** 256 connection cap enforced.
- **Digital** (and non-human org types): **not** connection-capped — intentional.
- **Profit / non-profit** entity death at rating zero: **not yet considered**; add when org graph behavior is defined.

---

### 5. Keystone / charter governance — theory vs lived reality

**Status:** **acknowledged** (governance gap, not a near-term code task). Classified politically in [`POLITICAL_SCIENCE_REVIEW.md`](POLITICAL_SCIENCE_REVIEW.md) §4 as **asymmetric federalism** — subsidiarity for member communities, immunity for keystones.

**Gap:** Immune council roots and charter communities are ecologically necessary but concentrate power.

**Product stance (2026-05-19):**

- Solo builder = de facto **“invisible 13th council member”** / operator of last resort; known and accepted for now.
- Worth documenting for honesty with users long-term; no automated “charter amendment” path required immediately.
- **Legitimacy note (2026-05-24):** residual sovereignty outside amendable constitutional politics is the main normative limit on “communities handle their own dramas” marketing — see [`POLITICAL_SCIENCE_REVIEW.md`](POLITICAL_SCIENCE_REVIEW.md) conclusion.

---

### 6. Judicial & graph operations metrics

**Status:** **deferred** — desired, not today.

**Gap:** Need observability for ecosystem health: `no_jury` rate, hung juries, appeal backlog, failing clusters, death-batch throughput, scope lag.

**Direction:** Dream up metrics when ready; hook into ops/job dashboards (scheduler + `POST /ops/jobs/run` culture).

---

### 7. Noms economic homeostasis

**Status:** **open** — not discussed in 2026-05-19 triage; keep on list.

**Gap:** Balance accrual (signup +32, daily +1, jury +3; **ad-free feed**) vs judicial/AI spend and Stripe packs so governance stays participatory, not elite-only.

**Note:** Posting/moderation Nom costs (§3) affect this balance; revisit with analytics.

---

### 8. Cognitive load — rating & constitution explainability

**Status:** **partial — shipped 2026-07-23** — self-profile civic standing block + How it works § Civic standing (`#civic-standing`). Optional pre–face-capture primer backlog.

**Gap (residual):** Face capture still precedes full literacy for some users; edge-only failing notices and login-refund toast still open.

**Direction:** Charter consent + published How it works annex + profile standing panel (not a separate consent ritual). Post-card civic shortcut still backlog.

---

### 11. Root Operating Charter revision

**v1 status:** **superseded** (2026-05-27) — 14-point charter; replaced by v2 (2026-05-29).

**v1 delivered (2026-05-27):** Universal Safety Floor charter text (incl. doxxing); protected identity (point 7); `moderation_tier_id` on constitution items; item 11 not a dispute trigger; item 14 outcome penalties.

**Greenfield split (2026-05-28):** `emergency_safety` + `civic_dispute` triggers; post **`suppressed`** / **`contained`** emergency path; civic **`under_judicial_review`** + serious/regular `no_jury` (7d / 21d) → **`flagged`**. See [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) § Judiciary.

**v2 status:** **shipped** (2026-05-29) — **16 points** in DB/UI; split **prohibition (4)** / **acute sanitation (15)** / **mandatory reporting (16)** / **civic moderation (13)**. Class A/B (CSAM, matched NCII) **must never** enter jury or council corpora (charter + ops target; vendor hash API still backlog).

**v2 ops backlog:** vendor hash API; hotline reporting (16); Class C operator queue; harm-class router beyond emergency/civic split; post-card civic shortcut; safety policy page; integration tests (Class A/B never seat jurors).

---

### 9. Platform trust & safety (outside graph/judiciary)

**Status:** **partially implemented** — **acceptable for closed beta** with operator on-call; **hard blocker for public launch** per [`LAUNCH_READINESS.md`](LAUNCH_READINESS.md). **Full roadmap:** [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md).

**Gap:** Graph death and community jury handle **governance fitness** (Tier 3), not all **baseline safety** (Tier 1–2: vendor Class A/B, hotline automation, notification mute, minors policy). Fitness ≠ safety ≠ wellbeing — see scholarly reviews.

**Live (2026-08):** emergency + civic filing; **Panoply `/home/ops`** + audit; human graph block; **`content_domain` denylist** (Tier 2b); **domain preview reputation bands**; **inline body URL block** at post create; **council share link quality gate**; **`safety_sanitation_event`** + face-capture hash stub; Safety & Crisis legal card + crisis panels; Google OIDC + reCAPTCHA; **on-call v1** ([`OPS_ON_CALL.md`](OPS_ON_CALL.md)).

**Ops backlog (P0 public launch; partially P1 beta):** operator **roster expansion** (v1 on-call shipped 2026-08-12); vendor hash API; hotline submission (16); dedicated safety policy page; platform notification mute.

**Deferral note:** Paid vendor integrations deferred until revenue/traffic justify cost — [`LAUNCH_READINESS.md`](LAUNCH_READINESS.md) rollback note.

---

### 10. Wellbeing guardrails (psychology-informed)

**Status:** **open** — cross-cuts §2 recovery, §8 explainability, §9 safety.

**Gap:** Reviews flag standing threat (failing band), inactivity-as-unfitness, jury moral burden, and thin recovery UX as **psychological risk vectors** distinct from classic engagement addiction. **Shipped (2026-06-10):** human failing owner-only; quota-free abstention on acute/serious jury cases.

**Direction (P1 unless noted):**

- **Standing literacy before commitment** — cross-cuts §8 (P0 beta)
- **Recovery checklist** — failing → healthy with named actions (§2, P0 beta)
- **Inactivity pause** — optional freeze on decay during documented offline periods ([`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) §P1)
- **Juror wellbeing** — sensitive-case content warnings beyond summons severity notice ([`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) §P1)
- **Metrics** — failing → healthy transitions; juror abstention burden (§6)

---

## Current priorities (August 2026 — closed beta)

Core platform is **built and hosted** (~99% invite-ready). Leverage is **verdict UX polish, login-refund toast, abuse monitoring, and public-launch T&S** — not feature breadth.

| Tier | Goal | Next actions |
|------|------|--------------|
| **1 — Finish closed beta** | Stable invite-only cohort | VM boot disk resize; **§1 verdict feed polish**; cohort metrics |
| **2 — Beta quality** | Users understand and trust standing | ~~§8 standing literacy~~ **shipped**; ~~§2 godmother recovery~~ **partial shipped** — **login-refund toast**, edge failing notices; post-card civic shortcut |
| **3 — Abuse resistance** | Close top adversarial path | ~~inline URLs + domain reputation~~ **shipped**; emergency-filing pattern monitoring; council invite corpus unification |
| **4 — Public launch** | Open signup | Vendor hash API + hotline (16); dedicated safety policy; legal/minors; monitoring; `test:smoke` in CI |
| **Watch** | No sprint until signal | §3 Nom post costs; §6 metrics; §7 Noms balance; §4 profit/NPO death |

---

## Priority snapshot (backlog items)

| Priority | Item | Status |
|----------|------|--------|
| P0 β | §1 Post verdict labels on feed | Partial — polish/QA |
| P0 β | §2 Recovery ecology UX | **Partial shipped** — godmother_outreach + civic standing; login-refund toast backlog |
| P0 β | §8 Standing explainability | **Partial shipped** (2026-07-23) — self-profile + How it works; pre-face primer optional |
| P0 β | §9 Operator on-call roster | **Partial shipped** — v1 [`OPS_ON_CALL.md`](OPS_ON_CALL.md) (2026-08-12); roster expansion backlog |
| P0 launch | §9 Vendor hash + hotline + safety policy + mute | Partial — inline URLs + domain reputation **shipped**; vendor hash/hotline backlog |
| — | §11 Root charter v2 | **Shipped** (2026-05-29) |
| P1 | §3 Emergency-filing abuse monitoring | Open — OIDC + 32 Nom grant |
| P1 | §10 Wellbeing guardrails | Partial — acute abstention shipped; inactivity pause, juror warnings open |
| P1 | §6 Ops metrics | Deferred |
| P2 | §4 Profit/NPO death | Open |
| P2 | §5 Keystone governance | Acknowledged |
| P2 | §7 Noms balance | Open |

**Legend:** P0 β = closed-beta minimum; P0 launch = public signup gate.

---

## Changelog

| Date | Change |
|------|--------|
| 2026-08-12 | §8 standing literacy shipped; §9 inline URLs + domain reputation + on-call v1; priorities refresh; ad-free Noms note |
| 2026-06-11 | Full triage refresh — failing 0.66, symmetric conversation, shipped beta stack; **Current priorities** table; §8 elevated to P0 β; §9 status corrected |
| 2026-05-29 | §11 Root charter **v2 shipped** (16 points); §9 sanitation audit + hash stub live |
| 2026-05-28 | §9 — outbound link denylist (Tier 2b) documented as live |
| 2026-05-27 | §11 Root charter — **shipped**; §9 T&S partial; priority snapshot updated |
| 2026-05-27 | Add [`CLINICAL_PSYCHOLOGY_REVIEW.md`](CLINICAL_PSYCHOLOGY_REVIEW.md); quint framing |
| 2026-05-24 | §11 Root charter — ready for implementation; link [`ROOT_CONSTITUTION_PROPOSAL.md`](ROOT_CONSTITUTION_PROPOSAL.md) |
| 2026-05-24 | Add [`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md), [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md); §10 wellbeing guardrails; quad framing |
| 2026-05-24 | Link [`DESIGN_BIAS_RECOVERY.md`](DESIGN_BIAS_RECOVERY.md) — owner punitive-bias reminder |
| 2026-05-24 | Cross-ref [`EVOLUTIONARY_BIOLOGY_REVIEW.md`](EVOLUTIONARY_BIOLOGY_REVIEW.md); §2 partial backend recovery noted |
| 2026-05-24 | Cross-ref [`POLITICAL_SCIENCE_REVIEW.md`](POLITICAL_SCIENCE_REVIEW.md); §5 legitimacy note |
| 2026-05-24 | Launch review cross-ref; §1 partial implementation noted; §9 elevated for public launch; §3 posting costs not wired |
| 2026-05-19 | Initial System Theory backlog from viability review + product triage |
