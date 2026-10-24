# Social psychology review — 256.app as a civic social environment

Cyberpsychology and social-media research analysis of 256.app’s standing signals, engagement design, jury burden, and wellbeing implications. Use alongside [`CLINICAL_PSYCHOLOGY_REVIEW.md`](CLINICAL_PSYCHOLOGY_REVIEW.md) (clinical harm assessment & publication ethics), [`EVOLUTIONARY_BIOLOGY_REVIEW.md`](EVOLUTIONARY_BIOLOGY_REVIEW.md) (ecological selection), [`POLITICAL_SCIENCE_REVIEW.md`](POLITICAL_SCIENCE_REVIEW.md) (institutional legitimacy), [`DESIGN_BIAS_RECOVERY.md`](DESIGN_BIAS_RECOVERY.md) (punitive asymmetry), [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) (safety roadmap), [`SYSTEM_THEORY.md`](SYSTEM_THEORY.md) (backlog), and [`AGENTS.md`](../AGENTS.md).

**Last reviewed:** 2026-05-24 (rating-economics pass — δ=0.035, symmetric spillover, phased inactivity + login refund).

---

## Abstract

256.app is **not** architected like a classic attention-extraction network. It caps human social graph size, scopes visibility, stages onboarding, and replaces opaque moderation with peer juries. From a social-psychology lens, however, it substitutes **scroll addiction** with **status threat**, **comparison anxiety**, and **distributed civic labor**. Scalar fitness ratings, failing-band shame markers, and account death at zero still create a high-stakes social environment — but a **2026-05-24 reward/punishment refactor** materially softened absence penalties (phased inactivity, **full login refund** of debited standing) and removed asymmetric global spillover. **Conversation settlement asymmetry** (Loathed −6×δ edge vs Loved +3×δ) remains the primary downward bias; recovery paths are **partially coded** but still **under-specified for hope** in UX — see [`DESIGN_BIAS_RECOVERY.md`](DESIGN_BIAS_RECOVERY.md).

**Main finding:** 256.app would interest researchers as a **natural experiment in reputational civic social media**. It is **less psychologically punishing on offline absence** than the pre-refactor design, but **not yet recommendable as a general-purpose network for psychological wellbeing** — especially for rejection-sensitive users, minors, or people who fear public evaluative labels tied to standing. It remains closer to a **consent-heavy intentional community laboratory** than to a mass-market “healthy alternative” to incumbent platforms.

---

## Classification

| Psychological construct | 256.app instantiation | Research analogue |
|-------------------------|------------------------|-------------------|
| **Social comparison** | Public scalar rating (0–5); red failing `@handle` | Festinger; upward/downward comparison on fixed status axis |
| **Ostracism / social death** | Rating → 0; human graph excision; strike-through dead handles | Williams; Need Threat Model (belonging, self-esteem, control) |
| **Procedural justice** | Transparent jury rules, frozen vote weights, evidence snapshots | Tyler; voice + neutrality + trust |
| **Status hierarchy** | Reputation-weighted jury votes (0–2× weight) | Power asymmetry in peer judgment |
| **Cognitive load** | Dual ratings (global + edge), constitution inheritance, jury math | Extraneous cognitive load; decision fatigue |
| **Civic / moral distress** | Compulsory-adjacent jury summons; 3 free abstentions/year | Bystander burden; vicarious trauma exposure |
| **Evaluative feedback** | Loved / Controversial / Loathed from weighted votes | Rejection sensitivity; public evaluation anxiety |
| **Parasocial attachment** | AI Council “godmother” at onboarding | Parasocial relationships with non-human guides |
| **Bounded relatedness** | 256 human connection cap | Dunbar-scale cognitive limit (protective) |
| **Compliance vs autonomy** | Phased inactivity decay; login **refund** of debited standing; projected deletion dates | Threat reduced on return; still fear-driven if users do not know refund exists |
| **Baseline harm response** | Deferred — not jury-scale | Acute trauma; harassment; illegal content |

**Working label:** *reputational civic social network with Dunbar-bounded humans and extinction-oriented standing signals*.

---

## 1. What differs from “awful” incumbents

### 1.1 Anti-addiction architecture (partial)

256.app does **not** optimize primarily for infinite passive consumption:

- **Audience-scoped posts** rather than broadcast-by-default feeds
- **Thread depth and reply caps** (`src/social/engagement.rs`)
- **Staged onboarding** before full participation (`docs/PRODUCT_GUIDE.md`)
- Marketing canon: *“meaningful connection instead of endless feed noise”*

A psychologist would **not** classify this as a dopamine-slot-machine design in the TikTok sense. Harm vectors shift from **compulsive use** toward **standing anxiety** and **governance fatigue**.

### 1.2 Dunbar cap as protective design

Humans are capped at **256** distinct active connections (`src/social/connections.rs`). This aligns with Dunbar-scale social cognition literature: bounded networks may reduce weak-tie overload and comparison surface area.

**Caveat:** Digital actors, communities, and orgs are **uncapped** — humans remain cognitively bounded while institutional nodes can form super-connective hubs. That asymmetry may recreate hierarchy in a smaller pond.

### 1.3 Procedural transparency

Judiciary rules are codified: √n jury sizing, frozen rating at invite, weighted ⅔ thresholds, automatic seat refill. Users can **in principle** understand *who decides* and *how* — rare among consumer social products.

**Caveat:** Transparency of procedure does not guarantee **perceived fairness** if outcomes feel status-weighted or irreversible.

---

## 2. Primary psychological risk vectors

### 2.1 Standing threat and the failing band

| Signal | User experience | Risk |
|--------|-----------------|------|
| Scalar rating 0–5 | Continuous social comparison on one axis | Chronic evaluative anxiety |
| `rating < 1.0` failing | Arterial-red `@handle`, profile warnings | Public shame; anticipatory anxiety |
| Projected deletion date | Email + profile from inactivity math | Return driven by fear, not desire |
| Death at 0.0 | Account removal; historical strike-through | Identity erasure; social death |

**Key asymmetry (still load-bearing — see [`DESIGN_BIAS_RECOVERY.md`](DESIGN_BIAS_RECOVERY.md)):**

Basal increment **δ = 0.035** (`jury_civic_rating_delta()`). Global spillover from community-edge changes is now **symmetric ±δ/6** (no longer 2× faster losses).

| Settlement | Community edge | Global (direct + spill) |
|------------|----------------|-------------------------|
| **Loved** | +3×δ (+0.105) | +1×δ direct + δ/6 spill (+0.0525) |
| **Controversial** | +1×δ (+0.035) | 0 |
| **Loathed** | −6×δ (−0.21) | −3×δ direct + spill (−0.14) |

One bad thread still costs **~2× Loved reward** on edge and **~2.7×** on global — enough for **hypervigilance** in the failing band, especially without a legible recovery checklist (see [`SYSTEM_THEORY.md`](SYSTEM_THEORY.md) §2, §8).

**Improved (same section):** matched engagement (e.g. 3 Loved + 3 Loathed per year) now lands near **2.19 edge / 2.24 global** from 2.50 — survivable vs the pre-refactor collapse toward ~1.7 global at δ=0.075 with asymmetric spillover.

### 2.2 Inactivity pruning vs normal human life

**Communities** no longer lose standing from post idle alone (`inactivity_batch.rs` — human login-idle only).

**Humans** (`src/social/community_graph/inactivity.rs`, `inactivity_batch.rs`):

| Phase | Idle days | Effect |
|-------|-----------|--------|
| Warning | **21+** | Email only — **no rating change** |
| Decrement start | **28+** | Daily global penalties begin (7-day buffer after warning) |
| Phase 1 | Decrement days 1–21 | **−1×δ** per day |
| Phase 2 | Decrement days 22–42 | **−3×δ** per day |
| Phase 3 | Decrement day 43+ | **−6×δ** per day |

**Login refund:** `record_user_login` (`src/auth/session.rs`) restores **`inactivity_rating_debited`** to global rating and resets the penalty cycle — a genuine **reversibility mechanism** for life events (illness, grief, deployment).

**Residual risk:** Users who never learn about the refund may still **log in from fear**; projected deletion dates remain stressors. UX does not yet surface “standing restored on return” (Self-Determination Theory: autonomy improves in code, not yet in product copy).

### 2.3 Reputation-weighted juries as status contests

Vote weight = `juror_rating_at_invite / 2.5` (0–2×). High-rated members count more toward verdicts.

Predicted effects:

- **Incumbent advantage** — well-rated users’ judgments dominate
- **Participation inequality** — low-rated users may disengage from civic roles
- **Disputes as status warfare** — post disputes become contests between standing tiers, not neutral fact-finding

See [`POLITICAL_SCIENCE_REVIEW.md`](POLITICAL_SCIENCE_REVIEW.md) §2.2, §5.

### 2.4 Distributed moderation as moral burden

Verified users may be summoned to juries; communities face penalties when juries fail to seat. Jurors gain **+3×δ** for voting; excess abstainers (>3/year) lose **−3×δ** (`RATING_MULT_JUROR_PARTICIPATION`).

Some users will experience this as **meaningful citizenship**. Others — conflict-averse users, trauma survivors, neurodivergent users — may experience **chronic moral distress** and exposure to others’ interpersonal harm without professional support.

**Research question:** Does peer jury duty increase perceived justice or spread vicarious stress across the community?

### 2.5 Engagement labels with fitness consequences

`PostFeel` (Loved / Controversial / Loathed) derives from weighted yes/no votes (`src/social/engagement.rs`, `src/social/conversation_rating.rs`). Softer language than “likes,” but **Loathed** content triggers strong negative civic selection.

For users high in **rejection sensitivity**, public evaluative labels tied to standing loss may be more harmful than anonymous downvotes.

### 2.6 Onboarding: identity anchoring before exploration

Flow: email → profile → **512×512 face capture** → **AI Council selection** → verified posting.

Psychological dimensions:

- **Appearance pressure** at entry (face pipeline)
- **Parasocial attachment** to AI godmother before human ties form
- **Commitment before informed consent** — users choose civic context before fully understanding standing mechanics

---

## 3. Orthogonal layers: fitness ≠ safety ≠ wellbeing

Three layers must stay separate in product thinking:

| Layer | Question | 256.app status |
|-------|----------|----------------|
| **Governance fitness** | Did this post violate community constitution? | Mature — judiciary, juries, council appeal |
| **Platform safety** | Is this illegal, threatening, or acutely harmful? | **Deferred** — see [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) |
| **Psychological wellbeing** | Does participation support belonging, autonomy, and recovery? | **Partial** — Dunbar cap + login refund help; conversation asymmetry and thin recovery UX hurt |

**Critical distinction:** Ecological fitness (rating, death, jury) is **not** psychological wellbeing. A system can be ecologically coherent and psychologically harsh. [`EVOLUTIONARY_BIOLOGY_REVIEW.md`](EVOLUTIONARY_BIOLOGY_REVIEW.md) §8 makes the same point for “pathogens outside the fitness model.”

---

## 4. Recommendations by audience (researcher verdict)

| Audience | Verdict |
|----------|---------|
| **General public / teens** | **Not recommended** until safety layer, recovery UX, and explainability ship |
| **Highly intentional adult communities** | **Conditional** — explicit opt-in to fitness regime; closed beta with informed consent |
| **Researchers** | **Recommended study object** — novel standing + jury + cap design |
| **Users in vulnerable life periods** | **Caution** — inactivity decay + failing shame + no acute harm path |

**Not “awful” in the shallow sense** (designed to addict). **Demanding in ways most users are unprepared for** — punishment mechanics better specified than recovery mechanics.

---

## 5. Product directions implied by this review

Prioritized alignment with existing backlog ([`SYSTEM_THEORY.md`](SYSTEM_THEORY.md), [`LAUNCH_READINESS.md`](LAUNCH_READINESS.md)):

| Priority | Direction | Backlog ref |
|----------|-----------|-------------|
| **P0** | Platform safety MVP (report, operator takedown, policy) | §9, [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) |
| **P0** | Recovery ecology UX — failing → healthy checklist, named actions, dates | §2, [`DESIGN_BIAS_RECOVERY.md`](DESIGN_BIAS_RECOVERY.md) |
| **P0** | Rating & constitution explainability at onboarding/profile | §8 |
| **P1** | Surface **login refund** and inactivity phases in profile/email; optional **pause** for documented life events | §2, §9 |
| **P1** | Juror wellbeing — content warnings, decline without penalty expansion, trauma-aware copy | New — see [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) §4 |
| **P1** | Metrics: failing → healthy transitions, jury burden surveys, abstention rates | §6 |
| **P2** | Rebalance **conversation** asymmetry (Loathed multiplier, ≥3-vote gate) after cohort data — spillover already symmetric | [`DESIGN_BIAS_RECOVERY.md`](DESIGN_BIAS_RECOVERY.md) |
| **P2** | Age gating / minors policy before public launch | [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) |

---

## 6. Testable hypotheses (field study)

1. Does the **failing band** (<1.0) increase withdrawal or frantic compliance?
2. Does **visible recovery pathway** (including login refund surfacing) reduce distress and improve re-engagement vs punishment-only UX?
3. Does the **256 cap** reduce loneliness or recreate comparison in a bounded network?
4. Does **reputation-weighted jury voting** improve outcome quality or entrench incumbents?
5. Does **symmetric spillover** (post-refactor) reduce guilt-by-association stress vs the old 2× loss rate?
6. Does **login refund** increase return-after-absence and reduce distress vs punishment-only inactivity?
7. Does **conversation asymmetry** (Loathed −6×δ vs Loved +3×δ) produce rejection-sensitivity harm disproportionate to governance benefit?
8. Does **AI godmother onboarding** accelerate belonging or delay human tie formation?

---

## 7. Conclusion (journal-ready summary)

256.app represents a **genuine alternative hypothesis** in social media design: bounded graphs, local peer governance, and explicit standing signals instead of opaque engagement optimization. Social psychologists would find it **more interesting than incumbent platforms** and **more ethically fraught than its marketing suggests**.

The platform is **strong on intentional structure** (caps, scopes, procedure) and **moderately improved on absence reversibility** (phased inactivity, login refund in code). It remains **weak on psychological safety nets** (recovery legibility, conversation asymmetry, acute harm response, juror burden). The owner’s docs acknowledge **residual punitive bias** in conversation settlement — honest framing that does not substitute for UX and further tuning.

**Final assessment:** Valuable **research laboratory**; **not yet a wellbeing-positive mass product**. Ship public scale only after [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) minimums and recovery/explainability UX address the gaps this review and [`DESIGN_BIAS_RECOVERY.md`](DESIGN_BIAS_RECOVERY.md) identify.

---

## Relationship to other docs

| Document | Lens |
|----------|------|
| [`EVOLUTIONARY_BIOLOGY_REVIEW.md`](EVOLUTIONARY_BIOLOGY_REVIEW.md) | Ecological selection, carrying capacity, immune metaphor |
| [`POLITICAL_SCIENCE_REVIEW.md`](POLITICAL_SCIENCE_REVIEW.md) | Legitimacy, federalism, reputational suffrage |
| [`CLINICAL_PSYCHOLOGY_REVIEW.md`](CLINICAL_PSYCHOLOGY_REVIEW.md) | Clinical harm assessment, publication ethics, vulnerable populations |
| [`DESIGN_BIAS_RECOVERY.md`](DESIGN_BIAS_RECOVERY.md) | Owner reminder — downward spiral vs recovery |
| [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) | Safety roadmap — illegal harm, reporting, wellbeing guardrails |
| [`SYSTEM_THEORY.md`](SYSTEM_THEORY.md) | Backlog & triage |
| [`LAUNCH_READINESS.md`](LAUNCH_READINESS.md) | Launch gates |

**Quint framing:** Evolutionary biology = **ecological**; political science = **institutional**; social psychology = **cyberpsychology / collective wellbeing research**; [`CLINICAL_PSYCHOLOGY_REVIEW.md`](CLINICAL_PSYCHOLOGY_REVIEW.md) = **clinical harm assessment & publication ethics**; **this file** = construct mapping and research analogues. [`SYSTEM_THEORY.md`](SYSTEM_THEORY.md) tracks what to build; scholarly reviews explain what the system *is*.

---

## Evidence index

| Path | Role |
|------|------|
| `src/social/connections.rs` | Dunbar cap (256) |
| `src/social/engagement.rs` | PostFeel, thread caps, weighted votes |
| `src/social/conversation_rating.rs` | Loved/Loathed fitness deltas |
| `src/social/judicial/` | Jury burden, vote weighting |
| `src/social/community_graph/inactivity.rs` | Phased human login-idle penalties |
| `src/social/community_graph/inactivity_batch.rs` | Warning/decrement constants, phased multipliers |
| `src/auth/session.rs` | Login refund of `inactivity_rating_debited` |
| `src/ui/entity_handle.rs` | Failing/dead handle presentation |
| `locales/en.json` | `ui.juror_workspace.law_notice` (T&S honesty) |
| `docs/DESIGN_BIAS_RECOVERY.md` | Punitive asymmetry math |

---

## Changelog

| Date | Change |
|------|--------|
| 2026-05-24 | **Rating-economics pass** — δ=0.035, symmetric spillover, phased inactivity + login refund, community post-idle removed; conversation asymmetry remains primary bias |
| 2026-05-24 | Initial social psychology review — consolidated from product architecture analysis |
| 2026-05-27 | Cross-ref [`CLINICAL_PSYCHOLOGY_REVIEW.md`](CLINICAL_PSYCHOLOGY_REVIEW.md); quint framing |
