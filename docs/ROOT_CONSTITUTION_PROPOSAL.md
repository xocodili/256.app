# Root constitution — 256 Operating Charter

| Field | Value |
|-------|--------|
| **Canonical spec** | This file — full Root text, trigger table, routing model, implementation checklist |
| **v1 (14 points)** | **Shipped** 2026-05-27 — migration `20260527120000_root_operating_charter_unified_moderation.sql` (+ follow-ups) |
| **v2 (16 points)** | **Shipped** 2026-05-29 — migrations `20260529120000_*`, `20260529121000_*`; consent `v2-2026-05-29` |
| **Recorded** | 2026-05-24 (v1 draft); v2 from product safety review |

Replaces draft direction in [`CONSTITUTIONAL_SAFETY_FRAMEWORK.md`](CONSTITUTIONAL_SAFETY_FRAMEWORK.md). Supersedes held migration [`migrations/20260524120000_constitutional_safety_framework.sql.held`](../migrations/20260524120000_constitutional_safety_framework.sql.held) (**held — do not apply**).

**Related:** [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md), [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md), [`CONSTITUTIONAL_SAFETY_FRAMEWORK.md`](CONSTITUTIONAL_SAFETY_FRAMEWORK.md).

**Out of scope for Root:** locale addenda, topic relevance, vendor names (PhotoDNA, CyberTipline), SLA numbers — those belong to community clauses or ops/legal annexes. Root sets floors, precedence, and response **classes**.

---

## Why v2 (summary)

v1 unified all safety concerns into **one post-dispute path** and treated **citing item 4** as **emergency jury moderation** (7-day serious tier). That conflates:

1. **What is forbidden** (Universal Safety Floor)
2. **Acute harm sanitation** (machine-first; no human ever views CSAM/NCII raw media)
3. **Civic moderation** (human juries for constitutional disagreement)

v2 **splits prohibition from routing**: item 4 defines prohibitions only; **points 15–16** govern acute sanitation and mandatory external reporting; **point 13** governs jury-path civic moderation only.

---

## v1 → v2 mapping

| v1 (shipped) | v2 (shipped) |
|--------------|---------------|
| 4 — floor + “citing invokes emergency moderation” | **4** — floor (prohibitions only) |
| 13 — unified moderation (emergency vs standard by filer citing 4) | **13** — civic moderation (jury path only) |
| 11 — legal process | **11** — strengthened preservation / operator duty |
| 6 — council appeals on post disputes | **6** — narrowed: civic appeals only; never Class A/B sanitation |
| *(none)* | **15** — acute harm sanitation (machine-first) |
| *(none)* | **16** — mandatory reporting and external duty |
| 9 — precedence | **9** — precedence updated (15, 16 before civic path) |

**Live product today** runs **Root charter v2 (16 points)** in DB/UI with consent **`v2-2026-05-29`**. Moderation routing from the **2026-05-28 greenfield split** remains: point **4** preamble + selectable **§4.1–§4.7** (401–407) → **`emergency_safety`** (suppress-first, contest to open jury); other dispute clauses → **`civic_dispute`** (jury from filing). Legacy **`post_moderation`** rows remain readable. **Shipped (v2 foundation):** points **15–16** charter text; **`safety_sanitation_event`** audit; SHA-256 media hash gate on face capture (`MEDIA_HASH_BLOCKLIST`); domain-deny audit logging. **Ops backlog:** vendor hash API (PhotoDNA-class); hotline automation (16); Class C operator queue UI; inline body URL checks — see [Implementation checklist (v2)](#implementation-checklist-v2).

---

## v2 summary of changes

| Topic | v1 (shipped) | v2 (shipped) |
|-------|--------------|---------------|
| Item 4 | Prohibitions + filer citation ⇒ emergency tier | **Prohibitions only** — does not determine response path |
| Item 13 | Unified path; item 4 ⇒ emergency jury tier | **Civic moderation** — juries for non-acute disputes only |
| Acute harm | Operator runbook (deferred); jury serious tier | **Point 15** — machine-first; Class A/B **never** in jury corpus |
| CSAM / NCII (avatar ingress) | N/A on posts (link/text product) | **Zero-human media path** — hash/vendor before portraiture; no stored user JPEGs |
| Reporting | LE “not wired” stub | **Point 16** — operator reports via hotline-class programs |
| Tier selection | Filer clause selection (item 4 ⇒ serious) | **Harm class routing** (15) + cited clauses (13) |
| Council AI | Post-dispute appeals after human jury | **Excluded** from Class A/B; civic appeals only (6, 13) |

---

## Product alignment (v2 — execution notes, not Root text)

### Response routing (target vs live)

| Harm class | Root anchor | Primary path | Human views raw media? | Jury? |
|----------|-------------|--------------|------------------------|-------|
| **Class A** — CSAM / exploitation imagery | 4, 15, 16 | Pre-persist hash/vendor match → block → hotline report | **Never** | **No** |
| **Class B** — NCII (matched / victim-hash) | 4, 15, 16 | Same as A where hash workflows apply | **Never** | **No** |
| **Class C** — Imminent violence, active doxxing + safety nexus | 4, 15 | Auto-hide → operator on-call → optional civic follow-up on **redacted** record | Redacted text only | Only after contain; not for raw media |
| **Malware / phishing URLs** (outbound links) | 4, 15 | **`content_domain` denylist** — block at ingest / OG / attach ([`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) § Outbound link domain policy) | N/A (URL only) | **No** |
| **Non-acute item 4** — disputed threats, scams, harassment patterns | 4, 13 | Post dispute → civic jury | Depersonalized text | **Yes** |
| **Community norms** — relevance, locale addenda | 13, 100+ | Post dispute → civic jury | Per 15 rules | **Yes** |

**Filing surface:** keep `GET|POST /home/judicial/post/{post_id}` for **civic moderation (13)**. Class A/B must **not** open `judicial_case` with human jurors — audit via `safety_sanitation_event` (or equivalent) instead.

**Moderation tier (`regular` \| `serious`):** derive from **harm class + cited clauses**, not “item 4 cited ⇒ serious” alone. Class A/B bypass civic clocks entirely.

**Schema (existing + proposed):**

- `community_constitution_item.moderation_tier_id` — civic path only
- **`content_domain`** — outbound link denylist (**live**); extend with inline URL checks + audit log
- New: sanitation audit log (point 15 due diligence)
- New: pre-persist media hash gate on upload pipeline

---

## Point mapping (v2 — DB `community_constitution_item.point`)

| Point | Title | `judicial_trigger_chk` | Notes |
|-------|--------|------------------------|--------|
| 1 | Voluntary Association | 0 | |
| 2 | Right of Exit | 0 | |
| 3 | Integrity of the Vote | 0 | |
| 4 | Universal Safety Floor | **1** | **Non-acute** disputes only; Class A/B auto-routed to 15 |
| 5 | Privacy of Weight | 0 | |
| 6 | Finality of Challenge | 0 | |
| 7 | Cognitive Liberty & Protected Identity | 0 | Defense only |
| 8 | Duty of Participation | 0 | |
| 9 | Preservation of the Tree | 0 | Precedence rule |
| 10 | Financial Accountability | 0 | |
| 11 | Legal Process & Cross-Border Visibility | 0 | Operator |
| 12 | Sunset and Renewal | 0 | |
| 13 | Civic Moderation Process | 0 | Process rule; filing via post dispute |
| 14 | Dignity in Civic Process | 0 | Outcome enforcement |
| 15 | Acute Harm Sanitation | 0 | System routing |
| 16 | Mandatory Reporting & External Duty | 0 | Operator duty |
| 100+ | Topic relevance, locale addenda | 1 when community-owned | |

---

## Root charter text (v2 — 16 points, shipped)

### Preamble

The Network exists for **voluntary, intentional association** among humans and their communities, organized as a **tree of communities**. Each community may own constitutional clauses and inherits those of its ancestors. Descendant clauses are **additive** and may not override a Root principle.

Governance rests on **transparent procedure** and **subsidiarity**. **Prohibited harm** is defined at Root. **Acute harm** is contained and escalated through **machine-first sanitation and operator duty**. **Constitutional disagreement** within normal timelines is resolved through the **civic moderation** process among the post audience.

---

### Point 1 — Voluntary Association

Every community is an **opt-in** collective. No human shall be bound to a community without explicit consent, except the Root itself, which constitutes the shared reality of the Network.

---

### Point 2 — Right of Exit

The path of departure shall remain **as frictionless as the path of entry**. Any human may leave a community; any community may migrate or dissolve, subject to its governance quorums. Exit shall not be penalized by retroactive standing loss for past lawful participation.

---

### Point 3 — Integrity of the Vote

The ballot is sacrosanct. Spoofing, botting, or bypassing sortition mechanisms strikes at the Root. The AI Council exists to **protect the vote**, not the voter.

---

### Point 4 — Universal Safety Floor

The Network prohibits content and conduct that **universally endangers people**, regardless of locale:

- Child sexual abuse material and exploitation of minors
- Credible threats of physical violence
- Non-consensual intimate imagery
- Doxxing and non-consensual publication of personally identifying information
- Fraud, scams, and impersonation intended to harm
- Coordinated harassment and bullying **as conduct** (not as identity)
- Distribution of malware or phishing intended to harm

This floor is **independent of any single country's criminal code**. Lawful protected identity and belief (point 7) are not violations here solely because they are illegal or stigmatized elsewhere.

**This point defines what is forbidden.** It does **not** by itself determine whether a matter is handled under **Acute Harm Sanitation (15)**, **Legal Process (11)**, or **Civic Moderation (13)**.

---

### Point 5 — Privacy of Weight

Every human has the right to know **their own** social standing and the variables that affect it. No other participant may see another human's standing or vote weight.

---

### Point 6 — Finality of Challenge

No constitutional verdict is beyond review until it reaches the **AI Council** on permitted **civic moderation appeals** under point 13. Council jurisdiction is **appellate and interpretive** on constitution questions — not legislative, not constitutional amendment, and **not acute harm sanitation** under point 15.

---

### Point 7 — Cognitive Liberty and Protected Identity

The Network shall not mandate **internal thought**, only **external conduct**. Communities may define **topic relevance** in **their own** constitutional clauses; they may not prohibit **protected identity or belief**.

Protected identity and belief include, without limitation: sexual orientation, gender identity, disability, ethnicity, religion, and non-belief. Locale law or custom is not grounds to remove lawful identity expression that does not otherwise violate the Universal Safety Floor (point 4).

---

### Point 8 — Duty of Participation

Citizenship is a function of **participation**. Prolonged absence may reduce standing under published rules, distinguished from persistent unfitness under point 12.

---

### Point 9 — Preservation of the Tree

No community rule may override a Root principle.

**Precedence:**

Universal Safety Floor (4) →  
Acute Harm Sanitation (15) and Mandatory Reporting (16) →  
Voluntary Association, Right of Exit, Protected Identity (1, 2, 7) →  
Legal Process (11) →  
Civic Moderation and Dignity in Process (13, 14) →  
Standing and governance machinery (5, 6, 10) →  
**community-owned clauses** (locale, relevance, local norms) →  
cross-border visibility rules within point 11.

Foundational seed communities may carry **structural immunity** from fitness extinction, but **not** immunity from points 4, 11, 15, or 16.

---

### Point 10 — Financial Accountability

Every token earned or spent in governance shall be traceable to its agency or community of origin.

---

### Point 11 — Legal Process and Cross-Border Visibility

The Network cooperates with **valid legal process** where the operator has legal duty or nexus. Lawful authority requests are handled by the **operator**, not by civic juries.

The operator shall maintain **preservation and production** procedures for lawful requests without requiring participants or jurors to review material routed under **Acute Harm Sanitation (15)**.

When content is lawful under Root principles in its community context but restricted elsewhere, the Network may **limit visibility by viewer geography** rather than require global removal.

This principle is **not** selectable as a civic moderation violation clause.

---

### Point 12 — Sunset and Renewal

Stagnation is unfitness. Communities that fail to govern or evolve may be returned to the parent node. Local extinction shall preserve descendant communities where the tree allows. Fitness consequences shall be **proportionate** and **reversible** where rehabilitation is possible; extinction is not for a single error or protected identity expression.

---

### Point 13 — Civic Moderation Process

Post moderation for **constitutional disagreement** is initiated through the **post dispute** process among the post audience. The filer identifies violated clauses from the **effective constitution** for the post.

**In scope for civic moderation:** community relevance, local norms, disputed application of the Safety Floor where **acute harm sanitation does not apply**, and other non-acute disputes.

**Out of scope for civic moderation:**

- Child sexual abuse material and confirmed exploitation imagery (point 15)
- Content matched or classified under **machine-first sanitation** before any human review
- Matters reserved to **Legal Process (11)** and **Mandatory Reporting (16)**

Civic moderation follows the ordinary jury process: sortition from the post audience, weighted vote, published timelines, and depersonalized review **only where human review is permitted under point 15**.

Civic moderation is **not** an emergency service and **not** a substitute for local emergency response.

---

### Point 14 — Dignity in Civic Process

Filing cases, jury service, and appeals must not be used to **harass, brigade, retaliate, or re-traumatize** participants. Bad-faith dispute filing — including citing the Safety Floor to bypass civic rules or to target protected identity (point 7) — violates this principle. When a jury **clears** the post, the filer receives an automatic fitness penalty (−6× civic δ) at resolution.

**Not a civic moderation trigger:** jurors decide cited clauses; item 14 is enforced at outcome.

---

### Point 15 — Acute Harm Sanitation

Certain harms require **immediate containment** and **machine-first handling**. No participant, juror, council member, or operator shall be required to **view** child sexual abuse material or non-consensual intimate imagery in raw form as part of Network review.

**Sanitation classes (non-exhaustive):**

- **Class A — Zero-human media:** CSAM and confirmed exploitation imagery. Detected by **hash or vendor match before persistence**; blocked from publication; **never** entered into civic moderation or council review corpora.
- **Class B — Zero-human or victim-hash media:** Non-consensual intimate imagery where **victim-side or vendor hash** workflows apply; same publication ban and review restrictions as Class A where matched.
- **Class C — Imminent physical danger:** Credible, imminent threats and active doxxing with credible safety nexus. **Immediately hidden** from circulation; operator on-call acts under published playbooks; civic moderation may follow only on **redacted** records if dispute remains.
- **Outbound link denial (live today):** Known malware, phishing, and operator-listed toxic **registrable domains** are blocked at link ingest, OG preview, and attached post publish via **`content_domain`** (`status = blocked` or `safety_state = unsafe`). This is **automated Tier 2b prevention** — not jury adjudication. Does not cover plain-text URLs in post body without link attach; does not replace upload hash gates (Class A/B). See [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) § Outbound link domain policy.

**Rules:**

1. **Contain first** — visibility ends before any civic timeline runs.
2. **No jury exposure** to Class A or Class B media.
3. **Routing is systemic** — filer clause selection alone does not place Class A/B material into jury review.
4. **Audit** — every sanitation action produces a tamper-evident operator/system record (timestamp, action class, case reference); not public vote detail.
5. Sanitation **does not** replace **Mandatory Reporting (16)** or **Legal Process (11)**.

---

### Point 16 — Mandatory Reporting and External Duty

Where the operator has **legal duty** or participates in **recognized global reporting programs**, the Network shall **report, refer, or preserve** through designated channels — not through ad hoc contact with individual law-enforcement agencies by participants or jurors.

**Minimum duties:**

- **CSAM and exploitation:** report through applicable **national or regional hotlines** (CyberTipline-class programs and INHOPE-network hotlines), within statutory deadlines where they apply.
- **Imminent threat to life:** preserve evidence; operator follows counsel-approved escalation; users in immediate danger are directed to **local emergency services**.
- **Self-harm crisis:** the Network surfaces **crisis resources**; it does not present jury service as crisis intervention.

Participants and jurors shall **not** be tasked with making law-enforcement reports on behalf of the platform. The operator is the **reporting entity** where law requires.

Due diligence is measured by **containment, preservation, and certified external referral** — not by opening a civic case alone.

---

## What stays local (not Root)

| Content | Owner | Example |
|---------|-------|---------|
| Topic relevance | Each intentional community | Point **100** on council seeds |
| Locale conduct addendum | Country-scoped communities | Amendment at create or later |
| Jury interpretive nuance | Product + community context | Depersonalized post card; redaction rules per 15 |
| Hash vendors, hotline APIs, SLAs | Ops / legal annex | Not Root text |
| Outbound link denylist (`content_domain`) | Ops / product | Live enforcement; see [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) |
| Crisis hotline numbers | Product locales | Referenced by 16 |

Root **precedence** (point 9) ensures local relevance clauses cannot override points 4, 7, 15, or 16.

---

## Emergency actor matrix (ops annex — supports points 15–16)

Due diligence requires **named actors**, not flags alone. See [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) § Emergency actor matrix.

| Emergency | Primary actor | External actor |
|-----------|---------------|----------------|
| CSAM (Class A) | Automated hash gate + platform | National CSAM hotline → LE |
| NCII spread (Class B) | Hash / victim-hash gate + platform | Jurisdiction-specific programs; counsel |
| Imminent violence (Class C) | Platform on-call (contain) | Local emergency services / LE per counsel |
| Active doxxing + nexus (Class C) | Platform on-call (remove PII) | LE only if bundled with imminent threat |
| Self-harm crisis | Platform (resources) | Crisis lines (988-class), not jury |
| **Malware / phishing URL** (listed domain) | **`content_domain` denylist** (automated) | None — Tier 2b prevention |

**Not in this table:** outbound links on **unlisted** scam domains, plain-text body URLs (gap), non-acute harassment, norm disputes → **Tier 2 queue or Tier 3 civic jury**. Full policy: [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) § Outbound link domain policy.

## Shipped charter text + moderation routing (in production)

**Charter text (v2, 16 points):** migration `20260529120000_root_operating_charter_v2.sql` (+ `20260528150000_root_charter_point13_emergency_civic_text.sql` interim text superseded by v2).

**Safety foundation (v2, 2026-05-29):** `20260529121000_safety_sanitation_event.sql`; `src/social/safety/` (hash gate + domain audit).

**Post-scoped moderation (2026-05-28):** migrations `20260528140000_emergency_civic_moderation.sql`.

**Historical (v1, 14 points):** `20260527120000_root_operating_charter_unified_moderation.sql` (+ `20260527140000_moderation_tier_on_constitution_item.sql`).

Live behavior today:

- Point **4** — non-selectable preamble; selectable **§4.1–§4.7** (401–407) → **`emergency_safety`**: post **`suppressed`**, case **`contained`**, no jury until author contests (1 Nom)
- Point **13** — **Civic Moderation Process** text (DB + locales); describes civic jury path vs emergency §4.x path
- Other dispute clauses → **`civic_dispute`**: post **`under_judicial_review`**, jury from filing; serious/regular `no_jury` clocks (7d / 21d) on civic cases only
- Legacy **`post_moderation`** trigger rows remain readable; new filings use the two triggers above
- Item 11 not a dispute trigger; item 14 outcome penalties live
- **v2 charter text (16 points)** in DB + UI; **`safety_sanitation_event`** audit; media hash gate on face capture
- **Ops backlog:** vendor hash API; hotline automation; Class C operator queue UI

Full v1 charter text is preserved in git history at this file (2026-05-27 revision). Do not delete migration records.

---

## Implementation checklist (v2)

### Constitution & consent

- [x] Owner sign-off on v2 Root text (this document)
- [x] SQL migration: `operating_charter_256` points 1–16; revise points 4, 6, 9, 11, 13; add 15, 16
- [x] Bump `human_onboarding::CURRENT_TERMS_CONSENT_VERSION`; re-consent flow for beta users
- [x] `locales/en.json` — `ui.login.constitution_item_*` through `_16`; revise `_4`, `_6`, `_9`, `_11`, `_13`
- [x] `templates/root_constitution_content_fragment.html` — 16 points
- [x] `docs/PRODUCT_GUIDE.md`, `TRUST_AND_SAFETY.md`, `LAUNCH_READINESS.md`, `AGENTS.md`

### Acute harm sanitation (15)

- [x] Pre-persist hash gate on face-capture JPEG ingress (SHA-256 blocklist via `MEDIA_HASH_BLOCKLIST`)
- [ ] Class A/B: vendor hash API integration (PhotoDNA-class)
- [x] `safety_sanitation_event` audit log
- [x] Domain deny audit logging (link ingest, create_post, entity brand URL)
- [ ] Class C: immediate hide + operator on-call queue (redacted review)
- [ ] **Outbound links:** extend inline body URL checks; vendor feed → `content_domain`

### Mandatory reporting (16)

- [ ] Counsel-approved hotline integration (CyberTipline-class / regional)
- [ ] Ops runbook: preservation metadata, submission receipts, escalation chain
- [ ] Crisis resource copy in UI (self-harm); not jury-as-crisis-response

### Civic moderation (13)

- [ ] Post dispute UI: distinguish civic path vs “handled automatically” for Class A/B
- [ ] Moderation tier: derive from harm class + clauses, not item 4 alone
- [ ] Post-card shortcut for **non-acute** safety disputes (optional)
- [ ] Integration tests: Class A/B never seat jurors; Class C contain-before-jury

---

## Changelog

| Date | Change |
|------|--------|
| 2026-05-29 | **v2 shipped** — 16-point charter in DB/UI; consent `v2-2026-05-29`; `safety_sanitation_event`; media hash gate |
| 2026-05-28 | **Greenfield emergency + civic split shipped** — interim toward v2; updated “in production” section |
| 2026-05-28 | Outbound link domain policy — point 15 Tier 2b; harm routing + emergency matrix + checklist |
| 2026-05-28 | **v2 proposed** — 16 points; split prohibition (4) / sanitation (15) / reporting (16) / civic moderation (13); harm-class routing; emergency actor matrix |
| 2026-05-27 | **v1 shipped** — 14 points; unified moderation; item 4 ⇒ serious tier |
| 2026-05-27 | Point 4 — doxxing on Universal Safety Floor |
| 2026-05-24 | Initial v1 proposal — unified moderation, no Root annexes |
