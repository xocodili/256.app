# Constitutional safety framework — root charter review

**Superseded for Root text by:** [`ROOT_CONSTITUTION_PROPOSAL.md`](ROOT_CONSTITUTION_PROPOSAL.md) — **v1 shipped** 2026-05-27; **v2 shipped** 2026-05-29 (16 points).

Review of the **256 Operating Charter** (`operating_charter_256`) with the goal of a **seriously safe** social network. Aligns [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) (operator tiers), [`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md) (wellbeing), [`CLINICAL_PSYCHOLOGY_REVIEW.md`](CLINICAL_PSYCHOLOGY_REVIEW.md) (clinical harm assessment), and jurisdiction split (local law vs universal harm).

**Last reviewed:** 2026-05-29 (Root charter v2 shipped).

---

## Executive summary

**v2 (shipped 2026-05-29):** Root charter is **16 points** — splits **prohibition (4)**, **acute sanitation (15)**, **mandatory reporting (16)**, and **civic moderation (13)**. See [`ROOT_CONSTITUTION_PROPOSAL.md`](ROOT_CONSTITUTION_PROPOSAL.md).

**v1 (shipped 2026-05-27, superseded):** Twelve-point charter review below remains **historical analysis**; live product ran **14-point v1** until v2 migrated.

The current twelve root principles are a strong **governance constitution** but a weak **safety constitution**. Two clauses create the biggest risk:

| Item | Title | Problem |
|------|-------|---------|
| **4** | Prohibition of Malice | “Illegal material” collapses universal harm, foreign criminal law, and moral disagreement |
| **11** | External Jurisdictional Compliance | Reads like content law; **`judicial_trigger_chk = 1`** lets users file post disputes on “illegal in my location” |

**Item 7** (Cognitive Liberty) and **item 9** (Preservation of the Tree) already protect identity and block overrides — but they are **not enforced in dispute UI** and lose to item 4/11 in practice.

**Target architecture:** four layers with explicit precedence:

```text
Layer 0 — Operator safety (minutes)     TRUST_AND_SAFETY.md Tier 1–2 — NOT jury
Layer 1 — Universal safety floor        Root item 4 — prohibitions (v2: routing via 15/13/16)
Layer 1b — Acute sanitation (v2)        Root item 15 — Class A/B/C; zero-human media
Layer 1c — Outbound link denylist (live) Root item 4 / 15 — content_domain Tier 2b; see TRUST_AND_SAFETY.md
Layer 1d — Mandatory reporting (v2)   Root item 16 — hotline-class programs
Layer 2 — Root rights                 Items 1, 2, 7, 9 — defenses; not dispute weapons
Layer 3 — Community locale            Country-scoped addenda + point-100 relevance — jury
Layer 4 — Legal process               Item 11 — operator lawful requests; NOT user disputes
```

---

## How constitution maps to product today

| Mechanism | Role in safety |
|-----------|----------------|
| `community_constitution_item` | Owned clauses per community; inherited leaf-first |
| `judicial_trigger_chk` | Only clauses with `= 1` can be selected when filing **post disputes** |
| Post dispute jury pool | Post **audience** (scoped visibility stakeholders) |
| `post.country_code` | Metadata at publish — not yet used for geo-withhold |
| Country-locked communities | Locale boundary for disputes and norms |
| AI Council appeal | Post-dispute **constitution** review only — not acute safety |

**Current triggers (root):** points **4** and **11** (`migrations/20260425154500_*`).  
**Council communities:** point **100** relevance (`migrations/20260517140000_*`).

**Fix applied (2026-05-24 migration):** remove `judicial_trigger_chk` from item **11**; revise text for items **4**, **7**, **9**, **11**; add items **13–14** (reporting channel + dignity in process).

---

## Clause-by-clause review (original twelve)

### 1. Voluntary Association — **Keep**

**Safety role:** Opt-in reduces coercion; essential for closed/intentional communities.

**Gap:** None material. Pair with item 2 (exit) in onboarding copy.

---

### 2. Right of Exit — **Keep**

**Safety role:** Abuse victims must leave without penalty. Do not add “except when under dispute” exceptions.

---

### 3. Integrity of the Vote — **Keep**

**Safety role:** Anti-manipulation. Protects jury legitimacy.

**Note:** Sock-puppet cost (Noms, connection cap) is engineering; this clause is the constitutional anchor.

---

### 4. Prohibition of Malice → **Universal Safety Floor — Revise (critical)**

**Original problem:** “Illegal material” invites lowest-common-denominator global censorship (e.g. LGBTQ expression lawful in AU, criminalized elsewhere).

**Revised role (Layer 1):** Enumerate **universal** prohibitions independent of locale:

- Child sexual abuse material and exploitation
- Credible threats of physical violence
- Non-consensual intimate imagery
- Fraud, scams, impersonation for harm
- Coordinated harassment and bullying **as conduct** (not identity)
- Malware / phishing distribution

**Explicitly exclude from item 4:** “Illegal in country X” unless also on universal floor; identity/belief expression protected by item 7.

**`judicial_trigger_chk`:** **Yes** — primary post-dispute safety clause.

**Operator alignment:** Tier 1–2 takedowns should cite the same categories — jury and operator speak one language.

---

### 5. Privacy of Weight — **Keep**

**Safety role:** Reduces status-based pile-on and reputational warfare.

**Wellbeing link:** [`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md) — comparison anxiety.

---

### 6. Finality of Challenge — **Keep; clarify scope**

**Safety role:** Council appeal is **post-dispute constitution** review — not emergency safety.

**Copy note:** “Acute harm → safety report; constitutional disagreement → dispute / appeal.”

---

### 7. Cognitive Liberty → **Cognitive Liberty & Protected Identity — Expand (critical)**

**Original:** Internal thought vs external conduct; topic relevance OK.

**Add explicitly:**

- Sexual orientation, gender identity, disability, ethnicity, religion, and non-belief are **protected identity and belief**
- Communities may set **topic relevance**; they may **not** prohibit identity or lawful expression of belief
- “Illegal in some jurisdiction” is **not** grounds to violate this item (see item 11)

**`judicial_trigger_chk`:** **No** — this is a **defense**, cited in jury instructions, not a weapon to remove others’ posts.

---

### 8. Duty of Participation — **Keep**

**Safety role:** Indirect — idle decay is fitness, not safety.

**Wellbeing caution:** Do not tie safety reporting to participation rating ([`DESIGN_BIAS_RECOVERY.md`](DESIGN_BIAS_RECOVERY.md)).

---

### 9. Preservation of the Tree — **Expand with precedence rule**

**Add:**

> Precedence: **Universal Safety Floor (4) → Root Rights (1, 2, 7) → Governance machinery → Locale addenda → Legal process (11).** No descendant clause may negate a root principle.

**`judicial_trigger_chk`:** **No**.

---

### 10. Financial Accountability — **Keep**

**Safety role:** Traceability for scams/Noms abuse. Operational, not jury-primary.

---

### 11. External Jurisdictional Compliance → **Legal Process & Cross-Border Visibility — Revise (critical)**

**Original problem:** Reads as “all local law is network law”; enabled as post-dispute trigger.

**Revised role (Layer 4 — procedural only):**

- Operator responds to **valid legal process** where platform has nexus/duty
- **Geo-restriction** of visibility preferred over global deletion when content is lawful under root rights in its community context
- Participants **must not** use item 11 as a post-dispute shortcut for “illegal where I sit”

**`judicial_trigger_chk`:** **No** — removed in safety migration.

---

### 12. Sunset Clause — **Keep**

**Safety role:** None direct. Ecosystem fitness.

---

## Recommended new root items (13–14)

Add via migration; optional point 15 later.

### 13. Safety Reporting & Acute Harm — **New**

> Content that may imminently harm a person, or that appears to violate the Universal Safety Floor, must be reported through the **platform safety channel** — not only a post dispute. Post disputes are for constitutional disagreement within normal timelines; they are **not** an emergency service.

**`judicial_trigger_chk`:** **No** (directs to Tier 1–2 product path).

### 14. Dignity in Civic Process — **New**

> Filing judicial cases, jury service, and appeals must not be used to harass, brigade, retaliate, or re-traumatize participants. Bad-faith dispute filing is a violation of this principle and may result in operator or fitness consequences.

**`judicial_trigger_chk`:** **No** — process rule, not a post-content clause. Bad-faith filing is penalized automatically when a jury clears the post (−6× civic δ on the filer at resolution).

### 15. Minors (recommended Phase 2) — **Not in initial migration**

> The Network is not directed at persons under 16. Exploitation or grooming of minors violates the Universal Safety Floor and warrants immediate operator action.

Requires age attestation product work ([`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) P2).

---

## Community-level clauses (descendants)

### Point 100 — Topic relevance (council seeds)

**Keep** as Layer 3 trigger. Safe framing:

> Posts should be relevant to the core intention of this group.

**Must not** be amended to ban protected identity (item 7 blocks).

### Locale addenda (country-locked communities)

Template for communities with `country_code` (not global):

> **Locale conduct (example AU):** Within this community, participants must comply with applicable Australian law **insofar as it does not require removal of content protected by Cognitive Liberty & Protected Identity (root item 7).** Disputes under this addendum are judged for this community’s audience and constitution, not foreign criminal codes.

**Rules:**

- Locale addenda cannot override root items 4, 7, 9
- **`judicial_trigger_chk = 1`** only when community explicitly adopts via amendment
- Dispute scope = post audience + governing community chain

---

## Jury instructions (required UX copy)

When seating post-dispute jurors, display:

1. Apply the **effective constitution** for this post’s community and audience.
2. **Universal Safety Floor (item 4)** — only enumerated universal harms; not “illegal somewhere.”
3. **Protected Identity (item 7)** — identity and lawful belief expression are not violations.
4. **Item 11 does not apply** to your vote — legal process is operator-handled.
5. **Acute danger** — stop and use safety report; do not treat this case as an emergency substitute.
6. **Bad-faith filing (item 14)** — do not file to harass or retaliate. You do not vote on item 14; vote on the cited clauses. Clearing the post applies an automatic filer penalty at resolution.

Store keys under `ui.juror_workspace.safety_instructions_*` in `locales/en.json`.

---

## `judicial_trigger_chk` policy (canonical)

| Point | Clause | Trigger? |
|-------|--------|----------|
| 1–3 | Association, exit, vote | No |
| **4** | Universal Safety Floor | **Yes** |
| 5–6 | Weight privacy, challenge | No |
| **7** | Protected identity | **No** (defense only) |
| 8 | Participation | No |
| 9 | Tree / precedence | No |
| 10 | Financial | No |
| **11** | Legal process | **No** |
| 12 | Sunset | No |
| **13** | Safety reporting | No |
| **14** | Dignity in process | **No** |
| 100+ | Relevance / locale addenda | Yes when community-owned |

---

## Relationship to operator safety (Layer 0)

Constitution **does not replace** [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md):

| Harm type | Channel |
|-----------|---------|
| CSAM, threats, doxxing, imminent self-harm | Operator Tier 1 — immediate |
| Harassment campaigns, scams, NCII | Operator Tier 2 — report queue |
| Constitution / relevance / community norms | Jury Tier 3 — post dispute |
| Court order / mandatory reporting | Operator — item 11 |

---

## Implementation checklist

| Step | Status |
|------|--------|
| Revise root clause text (4, 7, 9, 11) | Migration `20260524120000_constitutional_safety_framework.sql` |
| Add items 13–14 | Same migration |
| Remove trigger from item 11 | Same migration |
| Update `locales/en.json` marketing copy | Same PR |
| Juror safety instruction copy | **TODO** — UI strings |
| Safety report flow (item 13 meaningful) | **TODO** — [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) P0 |
| Geo-withhold by viewer country | **TODO** — product |
| Locale addendum template in community create | **TODO** — UX |

---

## Canonical revised clause text

See migration and `locales/en.json` keys `ui.login.constitution_item_*` plus `_13`, `_14`.

---

## Related docs

| Doc | Role |
|-----|------|
| [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) | Operator tiers, report/takedown build list |
| [`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md) | Wellbeing guardrails |
| [`CLINICAL_PSYCHOLOGY_REVIEW.md`](CLINICAL_PSYCHOLOGY_REVIEW.md) | Clinical harm assessment, launch ethics |
| [`DESIGN_BIAS_RECOVERY.md`](DESIGN_BIAS_RECOVERY.md) | Punitive asymmetry — do not stack safety on fitness without recovery |
| [`POLITICAL_SCIENCE_REVIEW.md`](POLITICAL_SCIENCE_REVIEW.md) | Legitimacy without security functions |
| [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) | Product canon — sync judiciary section |

---

## Changelog

| Date | Change |
|------|--------|
| 2026-05-28 | Layer 1c outbound link denylist (live); TRUST_AND_SAFETY cross-ref |
| 2026-05-28 | v2 charter proposed — layers 1b/1c/1d (points 15–16); [`ROOT_CONSTITUTION_PROPOSAL.md`](ROOT_CONSTITUTION_PROPOSAL.md) |
| 2026-05-24 | Initial framework; clause review; migration for items 4/7/9/11 + 13/14 |
