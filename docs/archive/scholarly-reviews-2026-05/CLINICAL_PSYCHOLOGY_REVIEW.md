# Clinical psychology review — 256.app harm assessment for publication

Clinical and cyberpsychology assessment of 256.app from a **practitioner and manuscript-review** lens: what the platform is, what it does for users, where it reduces psychological harm relative to incumbent social media, and where it introduces or amplifies harm. Use alongside [`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md) (cyberpsychology research framing), [`EVOLUTIONARY_BIOLOGY_REVIEW.md`](EVOLUTIONARY_BIOLOGY_REVIEW.md) (ecological selection), [`POLITICAL_SCIENCE_REVIEW.md`](POLITICAL_SCIENCE_REVIEW.md) (institutional legitimacy), [`DESIGN_BIAS_RECOVERY.md`](DESIGN_BIAS_RECOVERY.md) (punitive asymmetry), [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) (safety roadmap), [`SYSTEM_THEORY.md`](SYSTEM_THEORY.md) (backlog), and [`AGENTS.md`](../AGENTS.md).

**Last reviewed:** 2026-05-27.

---

## Abstract

256.app is a **reputational civic social network**: Dunbar-bounded human graphs (256 connections), audience-scoped feeds, peer jury governance, and scalar standing signals (0.00–5.00) that drive **failing-band shame**, **inactivity pruning**, and **social death at zero**. From a clinical harm perspective, the platform **does not optimize primarily for compulsive passive use** — a meaningful departure from attention-extraction incumbents — but **substitutes scroll addiction with status threat, evaluative anxiety, distributed civic trauma exposure, and identity erasure**.

**Main finding:** 256.app is a **credible field site for cyberpsychology research** and a **conditional fit for closed beta among informed adults**; it is **not yet ethically recommendable as a general-purpose or wellbeing-positive mass product**. Strengths include bounded graphs, procedural transparency, partial absence reversibility (login refund in code), and honest in-product limits on emergency response. Weaknesses dominate for vulnerable users: asymmetric punishment (Loathed vs Loved), public failing markers, thin recovery UX, reputation-weighted juries, fear-framed inactivity messaging, onboarding before full informed consent, and a **deferred platform safety layer** (no in-product report flow, incomplete block scope, no crisis resources).

---

## Classification

| Clinical construct | 256.app instantiation | Assessment |
|--------------------|------------------------|------------|
| **Compulsive use / addiction** | Audience-scoped feed, thread caps, staged onboarding | **Lower risk** than infinite-scroll incumbents |
| **Social comparison distress** | Public scalar rating; failing red `@handle` | **High risk** — continuous evaluative axis |
| **Ostracism / social death** | Rating → 0; strike-through handles; graph excision | **High risk** — Williams need-threat model |
| **Rejection sensitivity** | Loved / Controversial / Loathed with fitness consequences | **High risk** for RSD-prone users |
| **Learned helplessness** | Asymmetric Loathed penalties; thin recovery path | **Moderate–high risk** in failing band |
| **Vicarious trauma** | Peer jury exposure to dispute content | **Moderate risk** — safeguards deferred |
| **Autonomy / informed consent** | Charter at step 1/4; face + godmother before full standing literacy | **Moderate risk** — commitment before comprehension |
| **Acute harm response** | Tier 1–2 T&S deferred; jury timescales for governance | **Poor** — wrong mechanism for crisis |
| **Agency in harassment** | Graph block (feed/profile/thread); voluntary delete | **Partial** — public discovery, notifications gap |
| **Developmental protection** | 17+ checkbox; no robust minor exclusion | **Insufficient** for youth |

**Working label:** *reputational civic social network with standing-based extinction signals and incomplete psychological safety nets*.

---

## 1. What it is and what it does

### 1.1 Product identity

256.app positions itself as a **human-centred community platform** where people build intentional relationships with humans and AI guides inside governed communities. Implementation matches that framing structurally (scoped visibility, constitution-aware disputes, staged verification) while coupling participation to a **fitness regime** documented in [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) and [`AGENTS.md`](../AGENTS.md).

### 1.2 User-facing behavioral loop

From a clinician’s functional analysis:

1. **Staged entry** — verify email → accept Root Operating Charter → **512×512 face capture** → profile → AI godmother → posting rights.
2. **Bounded relating** — up to **256** human connection slots; communities/orgs uncapped.
3. **Scoped exposure** — posts to chosen audiences; threads depth-capped.
4. **Standing evaluation** — engagement settles to Loved / Controversial / Loathed with **asymmetric rating deltas** (Loathed hurts ~2× Loved on edge; ~2.7× globally).
5. **Civic labor** — jury summons; three free abstentions/year, then standing penalty.
6. **Absence consequences** — 21d warning → 28d phased daily decrements → projected deletion date; **login refunds debited standing** in code.
7. **Partial mute** — human block on profile + feed + threads; not platform-wide.
8. **Monetization** — Noms via feed ad exposure; judicial and AI debits.

**Governance fitness** (juries, ratings, death) and **platform safety** (illegal content, acute harm) remain **orthogonal** — see [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md), [`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md) §3.

---

## 2. What it does well (psychological harm reduction)

These are **relative strengths vs attention-extraction social media**, not claims of net wellbeing benefit.

### 2.1 Anti-addiction architecture

Audience-scoped feeds, reply/thread limits, and onboarding gates shift primary harm **away from compulsive passive consumption** toward standing anxiety — a different clinical profile. Users prone to **problematic scrolling** may fare better structurally; users prone to **rejection sensitivity** may fare worse.

Evidence: [`docs/SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md) §1.1; `src/social/engagement.rs`.

### 2.2 Dunbar cap as cognitive protection

The **256** human connection limit aligns with Dunbar-scale social cognition — potentially reducing weak-tie overload and comparison surface area. **Caveat:** uncapped digital/community nodes may recreate hierarchy in a smaller pond.

Evidence: `src/social/connections.rs`; [`EVOLUTIONARY_BIOLOGY_REVIEW.md`](EVOLUTIONARY_BIOLOGY_REVIEW.md) §1.

### 2.3 Procedural transparency (procedural justice)

Documented jury sizing, frozen vote weights, ⅔ normalized thresholds, and evidence snapshots support **legibility of process** — associated in Tyler’s procedural-justice work with perceived fairness even when outcomes hurt.

Evidence: `src/social/judicial/`; [`POLITICAL_SCIENCE_REVIEW.md`](POLITICAL_SCIENCE_REVIEW.md).

### 2.4 Absence reversibility (coded, under-communicated)

Phased inactivity (warning → slow → accelerated decay) and **full login refund** of `inactivity_rating_debited` (`src/auth/session.rs`) are clinically meaningful for illness, grief, or deployment — if users know they exist. Community post-idle decay was **removed** (humane for lurkers).

Evidence: [`DESIGN_BIAS_RECOVERY.md`](DESIGN_BIAS_RECOVERY.md); `src/social/community_graph/inactivity.rs`.

### 2.5 User agency primitives (partial)

Human **block**, disconnect, voluntary account/entity deletion give interpersonal-friction victims **immediate tools** on core surfaces without waiting for jury resolution — consistent with harassment literature on agency reducing helplessness.

Evidence: `src/social/blocks.rs`; [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) § Human block list.

### 2.6 Honest emergency limits in-product

Juror workspace copy (`ui.juror_workspace.law_notice`): law-enforcement reporting is **not wired**; use local emergency services. Reduces **false security** — a common harm when platforms imply moderation equals crisis response.

### 2.7 Meta-critical documentation

Internal reviews ([`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md), [`DESIGN_BIAS_RECOVERY.md`](DESIGN_BIAS_RECOVERY.md), [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md)) acknowledge punitive bias and safety gaps — ethically preferable to wellbeing marketing without engineering support.

---

## 3. What it does badly (psychological harm amplification)

### 3.1 Status threat and social death replace scroll addiction

| Signal | Clinical concern |
|--------|------------------|
| Scalar rating 0–5 | Chronic social comparison on one axis |
| Failing band (<1.0) | Public shame marker (arterial-red `@handle`) |
| Projected deletion date | Fear-driven return; anticipatory anxiety |
| Death at 0.0 | Identity erasure; ostracism (belonging, self-esteem, control) |

Recovery exists in code but is **“better specified than hope”** in UX — no failing→healthy checklist, login refund under-surfaced. See [`DESIGN_BIAS_RECOVERY.md`](DESIGN_BIAS_RECOVERY.md).

### 3.2 Punishment asymmetry and hypervigilance

At δ = 0.035, one **Loathed** conversation costs roughly **2×** one **Loved** on edge and **~2.7×** globally. Predicted effects: hypervigilance, speech avoidance, learned helplessness in failing band, compliance from fear (autonomy undermined).

| Settlement | Edge | Global (direct + spill) |
|------------|------|-------------------------|
| **Loathed** | −6×δ (−0.21) | −3×δ + spill (−0.14) |
| **Loved** | +3×δ (+0.105) | +1×δ + spill (+0.0525) |

Evidence: `src/social/conversation_rating.rs`; [`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md) §2.1.

### 3.3 Reputation-weighted juries entrench hierarchy

Vote weight = `juror_rating_at_invite / 2.5` (0–2×). High-standing jurors dominate; disputes risk **status contests**; low-rated users may withdraw from civic roles — worsening alienation.

Evidence: [`POLITICAL_SCIENCE_REVIEW.md`](POLITICAL_SCIENCE_REVIEW.md) §2.2; [`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md) §2.3.

### 3.4 Distributed moderation as moral and vicarious trauma

Ordinary users may review slurs, violence, sexual content, and interpersonal harm. Only **three free abstentions/year**; beyond that, abstention **diminishes standing**. Content warnings, trauma-informed decline, and reporter exemption are **planned, not shipped** ([`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) §P1).

Routing **acute harm** (CSAM, credible threats, doxxing) into **multi-day jury clocks** would **re-traumatize jurors** and fail victims — clinically indefensible if marketed as self-policing community.

### 3.5 Platform safety layer deferred

| Gap | Psychological impact |
|-----|------------------------|
| No in-product **report** flow | No normalized harm pathway |
| Block incomplete (public, search, notifications) | Harassment persists off-feed |
| No CSAM/threat hash matching | Exposure and re-victimization risk |
| No published standalone **safety policy** | Incomplete informed consent |
| No crisis resources (hotlines) in UI | Missed harm-reduction touchpoint |
| Operator emergency path = SQL runbook only | Acute harm on wrong timescale |

Evidence: [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) — **hard blocker for public launch**.

### 3.6 Onboarding: commitment before comprehension

Face capture (appearance/biometric-adjacent pressure) and **parasocial AI godmother** attachment precede full literacy of standing mechanics, death at zero, and jury summons. Problematic for **autonomy** and for users who would opt out if the fitness regime were legible upfront.

Evidence: [`ONBOARDING.md`](ONBOARDING.md); [`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md) §2.6.

### 3.7 Fear-framed inactivity messaging

Email subjects such as “exit from 256 is accelerating” and profile **estimated deletion dates** may drive return through **threat**, not reconnection — especially when login refund is not prominent. Optional **90-day life-event pause** remains backlog only.

Evidence: `locales/en.json` (`human_inactivity_*`); `ui.entity_private.human_inactivity_notice`.

### 3.8 Developmental and vulnerable-population posture

17+ checkbox without robust minor exclusion, new-account jury restrictions, or youth safety policy. **Not recommended** for teens, rejection-sensitive users, trauma survivors in active recovery, or people in unstable life periods — consistent with [`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md) §4.

### 3.9 Marketing–mechanism gap (betrayal distress)

Copy emphasizes “meaningful connection” and “trust”; dominant affective signals are **evaluation, shame, and extinction**. Increases **betrayal distress** when users discover the fitness regime post-commitment.

Evidence: [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) marketing anchors vs failing notices in `locales/en.json`.

---

## 4. Clinical verdict by audience

| Audience | Verdict |
|----------|---------|
| **General public / teens** | **Not recommended** until P0 safety, recovery UX, standing explainability |
| **Rejection-sensitive / trauma history** | **High caution** — public evaluative labels + Loathed asymmetry |
| **Highly intentional adult communities** | **Conditional** — closed beta with explicit informed consent to fitness regime |
| **Researchers (cyberpsychology)** | **Recommended study object** — novel standing + jury + cap design |
| **Users during life disruption** | **Caution** — inactivity decay + thin pause path; refund exists but hidden |
| **Juror pool / conflict-averse users** | **Caution** — moral burden + abstention quota |

---

## 5. Overall harm balance (publication summary)

| Dimension | Assessment |
|-----------|------------|
| Compulsive use risk | **Lower** than incumbents |
| Social comparison / status threat | **Higher** than incumbents |
| Ostracism / identity erasure | **High** |
| Acute harm / trauma response | **Poor** (deferred T&S) |
| Procedural fairness | **Moderate–good** (transparent rules; status-weighted outcomes weaken equity) |
| Recovery / resilience support | **Poor** (asymmetric penalties; thin hope UX) |
| Suitability as wellbeing-positive mass product | **No** at current maturity |
| Suitability as research laboratory | **Yes** |

**Clinical bottom line:** 256.app trades **passive addiction** for **active civic and reputational stress**. That trade may benefit some users (bounded graphs, intentional communities) while **net harming** others — especially those with rejection sensitivity, trauma history, or need for immediate safety response. Public launch without [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) P0 minimums and recovery/explainability UX would be **ethically indefensible** for a clinician endorsing the product as psychologically safe.

For journal publication, the strongest framing is **256.app as a field site for reputational civic social media** — not as evidence that peer juries produce healthier online life than professional moderation (untested; undermined by acute-harm gap).

---

## 6. Clinical recommendations (aligned with product backlog)

| Priority | Recommendation | Backlog ref |
|----------|----------------|-------------|
| **P0** | Ship Tier 1–2 safety before open signup | [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) |
| **P0** | Recovery as legible as punishment — checklist, surface login refund, reduce shame copy | [`DESIGN_BIAS_RECOVERY.md`](DESIGN_BIAS_RECOVERY.md), [`SYSTEM_THEORY.md`](SYSTEM_THEORY.md) §2 |
| **P0** | Standing/jury informed consent **before** face capture and godmother | [`ONBOARDING.md`](ONBOARDING.md) |
| **P1** | Juror wellbeing — content warnings, penalty-free “cannot review safely” decline | [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) §P1 |
| **P1** | Optional inactivity pause for documented life events | [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) §P1 |
| **P1** | Full block scope + notification suppression | [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) §P0 |
| **P2** | Rebalance Loathed asymmetry after cohort data | [`DESIGN_BIAS_RECOVERY.md`](DESIGN_BIAS_RECOVERY.md) |
| **P2** | Do not market as general “healthy alternative” without distress/recovery metrics | [`LAUNCH_READINESS.md`](LAUNCH_READINESS.md) |

---

## 7. Testable clinical hypotheses

1. Does the **failing band** increase withdrawal, somatic stress symptoms, or frantic compliance vs healthy users?
2. Does **surfacing login refund** reduce distress and improve return-after-absence vs fear-only messaging?
3. Does **visible recovery pathway** improve re-engagement among users who cross below 1.0?
4. Does **conversation asymmetry** produce disproportionate harm in users high in rejection sensitivity?
5. Does **jury service** increase perceived justice or spread vicarious stress / moral injury?
6. Does **AI godmother onboarding** accelerate belonging or delay human tie formation and increase parasocial dependence?
7. Does **partial block** leave harassment victims with persistent off-feed distress measurable in self-report?

---

## Relationship to other docs

| Document | Lens |
|----------|------|
| [`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md) | Cyberpsychology research — standing, civic burden, wellbeing constructs |
| **This file** | **Clinical harm assessment** — publication ethics, vulnerable populations, practitioner verdict |
| [`EVOLUTIONARY_BIOLOGY_REVIEW.md`](EVOLUTIONARY_BIOLOGY_REVIEW.md) | Ecological selection, extinction, immune metaphor |
| [`POLITICAL_SCIENCE_REVIEW.md`](POLITICAL_SCIENCE_REVIEW.md) | Institutional legitimacy, keystone sovereignty |
| [`DESIGN_BIAS_RECOVERY.md`](DESIGN_BIAS_RECOVERY.md) | Owner reminder — punitive vs recovery bias |
| [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) | Tier 1–2 safety roadmap |
| [`SYSTEM_THEORY.md`](SYSTEM_THEORY.md) | Backlog & triage |
| [`LAUNCH_READINESS.md`](LAUNCH_READINESS.md) | Launch gates; psychology note for public scale |

**Quint framing:** Evolutionary biology = **ecological**; political science = **institutional**; social psychology = **cyberpsychology / collective wellbeing research**; **clinical psychology (this file)** = **harm assessment & publication ethics**; [`SYSTEM_THEORY.md`](SYSTEM_THEORY.md) = **what to build next**.

[`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md) and this document are **complementary**: the former maps constructs and research analogues; the latter states **clinical recommendability** and **ethical launch conditions**.

---

## Evidence index

| Path | Role |
|------|------|
| `src/social/connections.rs` | Dunbar cap (256) |
| `src/social/conversation_rating.rs` | Loved/Loathed fitness deltas |
| `src/social/engagement.rs` | PostFeel, thread caps |
| `src/social/blocks.rs` | Partial human block |
| `src/social/judicial/` | Jury burden, vote weighting |
| `src/social/community_graph/inactivity.rs` | Phased login-idle penalties |
| `src/auth/session.rs` | Login refund of inactivity debits |
| `src/ui/entity_handle.rs` | Failing/dead handle presentation |
| `locales/en.json` | Failing/inactivity copy; `law_notice` |
| `docs/TRUST_AND_SAFETY.md` | Safety gaps and P0 list |
| `docs/DESIGN_BIAS_RECOVERY.md` | Punitive asymmetry math |

---

## Suggested external publication venues

Depending on emphasis: *Clinical Psychological Science* (commentary); *Cyberpsychology, Behavior, and Social Networking*; *JMIR Mental Health*; *New Media & Society* (harm reduction); *Ethics and Information Technology* (informed consent, launch ethics).

---

## Changelog

| Date | Change |
|------|--------|
| 2026-05-27 | Initial clinical psychology review — harm assessment for publication; complements [`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md) |
