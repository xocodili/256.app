# Political science review — 256.app as a designed polity

Comparative-governance analysis of 256.app’s institutional architecture, written as if for peer-reviewed publication in platform governance or democratic theory. Use alongside [`EVOLUTIONARY_BIOLOGY_REVIEW.md`](EVOLUTIONARY_BIOLOGY_REVIEW.md) (ecological/evolutionary framing), [`SYSTEM_THEORY.md`](SYSTEM_THEORY.md) (backlog & triage), [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) (product canon), and [`AGENTS.md`](../AGENTS.md) (judiciary rules).

**Last reviewed:** 2026-05-24 (rating-economics pass — δ=0.035, symmetric spillover, phased inactivity + login refund).

---

## Abstract

256.app encodes governance as a **constitutional order**, not as opaque moderation: federated communities with inheritable charters, sortition-adjacent juries sized by √n, reputation-weighted supermajorities, and an automated fitness regime that can terminate communities and delete accounts at rating zero. This document classifies that order and states its normative limits.

**Main finding:** 256.app is a **digital federacy with reputational sortition courts**—among the most institutionally ambitious governance experiments in contemporary social media—but it is **not liberal democracy**. Subsidiarity is real for member communities; **keystone immunity** and an acknowledged operator sovereign (“invisible 13th council member”) concentrate residual power outside amendable constitutional politics. **Governance fitness** (ratings, juries, death) and **platform safety** (illegal content, acute harm) are explicitly **orthogonal layers**; the latter remains deferred.

---

## Classification

| Label | Fits? | Notes |
|-------|-------|-------|
| Liberal representative democracy | No | No elections; rating-weighted jury votes |
| Sortition / demarchy | Partial | Random jury draw; votes weighted by reputation |
| Polycentric / Ostromian governance | Partial | Overlapping communities, local rule-making; keystone asymmetry |
| Federal constitutionalism | Partial | Parent-tree inheritance; no dual sovereignty or supremacy clause |
| Evolutionary constitutionalism | Yes | Fitness ratings, inactivity pruning, death at zero |
| Platform feudalism (ToS + mods) | No | Codified judiciary, transparent procedure |

**Working label:** *digital federacy with reputational sortition courts and ecological selection*.

---

## 1. Constitutional structure

### 1.1 Federated layering

Communities form a **single-parent tree** (`social_community.parent_community_id`). Each community **owns** clauses on `community_constitution_item`; **effective law** is assembled by walking ancestors **leaf-first**, merging active sections (`src/social/community_constitution.rs`, `fetch_effective_constitution`).

- Inheritance is **additive sectioning by tier**, not clause-level preemption.
- **Re-parent** (`community_reparent`, 64 Noms) changes inherited law; constitution diffs are stored on cases (`constitution_diff_json`).

**Comparative analogue:** federal layering without a supremacy clause—legal pluralism within a tree, not American dual sovereignty.

### 1.2 Amendment politics

Constitutional change requires:

- Filer with verified `member_of` edge rating **≥ 3.00** (`src/social/private_profile.rs`)
- **32 Noms** (`src/pricing.rs`)
- Human jury from **amending community only** (not descendant subtree)
- Jury size **⌈√n⌉, uncapped** (`AGENTS.md`, judiciary section)

**AI cannot amend constitutions.** Council of 12 stewards seeded clauses for its domains; changing them is human-only (`constitutional_amendment`). This is a deliberate **separation of powers**: AI as narrow appellate interpreter, not constitutional legislator.

**Comparative analogue:** participatory constitutional convention scaled with community size, not a fixed senate.

---

## 2. Judiciary: subsidiarity, sortition, weighted suffrage

### 2.1 Subsidiarity in jury pools

Default rule (marketing canon): **“Communities handle their own dramas.”**

| Trigger | Jury pool | Size |
|---------|-----------|------|
| Post dispute | Post **audience** (friends/family or community members) | ⌈√n⌉, cap 12 |
| Membership review | Community members (>3.00, fallback ≥2.50) | ⌈√n⌉, cap 12 |
| Constitutional / profile amendment | Amending community members | ⌈√n⌉, uncapped |
| Re-parent phase 1 | Moving community **M** only | ⌈√n⌉, cap 12 |
| Re-parent phase 2 | Distinct humans in **M ∪ descendant subtree** | ⌈√N⌉, uncapped |

Evidence: [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) § Judiciary; `src/social/judicial/pool.rs`; `src/social/judicial/community_reparent.rs`.

Post disputes are adjudicated by **visibility stakeholders**, not platform staff—a subsidiarity innovation sortition literature has barely addressed.

### 2.2 Sortition with reputational inequality

Jurors are **randomly shuffled** from eligible pools (`src/social/judicial/nomination.rs`). Excluded: originator, target, originator-controlled entities, prior abstainers/decliners for the same case.

Votes are **not** one-person-one-vote. Weight at invite:

```text
weight = juror_rating_at_invite / 2.5   →  scale 0.0–2.0  (2.50 rating = 1.0 vote)
```

Frozen on `juror_rating_at_invite` (`AGENTS.md`). Verdict carries at weighted **⅔** of total cast weight (`src/social/judicial/verdict_eval.rs`).

**Political significance:** sortition-adjacent selection paired with **meritocratic/reputational suffrage**—a combination liberal democracies historically rejected.

Council agent prompts reference **“demarchy”** (`src/agent_specs.rs` collective context)—engineering-native theory, not cited academic canon (Rawls, Ostrom, etc.).

### 2.3 Asymmetric hung-jury defaults

| Case type | Hung band (below weighted ⅔) |
|-----------|------------------------------|
| Post dispute / appeal | **Upheld** (violation presumed) |
| Amendment, membership, re-parent | **Overturned** (status quo preserved) |

Evidence: `src/social/judicial/verdict_eval.rs`.

**Interpretation:** conservative bias on **structural change**, enforcement bias on **speech moderation**.

### 2.4 AI Council as appellate chamber

- **Humans:** all primary adjudication (posts, membership, amendments, re-parent).
- **Council of 12:** **post-dispute appeals only** (32 Noms after resolved post dispute).

Human juries: weighted supermajority. Council: **simple majority** on binary violation votes; ties favor moderation (`src/social/judicial_council.rs`).

**Interpretation:** deferential appellate standard, not countermajoritarian constitutional court.

---

## 3. Citizenship, fitness, and fiscal participation

### 3.1 Rating as civic standing

| Band | Meaning |
|------|---------|
| Default **2.50** | Neutral jury weight |
| **< 1.0** failing | Red handle, notices; blocks amendment/reparent filing |
| **0.0** death | Community: retire constitution, end memberships, re-parent children. Human: `to_be_deleted`, graph excision |

Inactivity (**humans only** — communities no longer decay from post idle):

| Phase | Idle days (no sign-in) | Effect |
|-------|------------------------|--------|
| Warning | **21+** | Email; no rating change |
| Decrements | **28+** | Daily global penalties: 21d at **−1×δ**, then 21d at **−3×δ**, then **−6×δ**/day |
| Return | Sign-in | **Full refund** of `inactivity_rating_debited`; counters reset (`src/auth/session.rs`) |

Projected deletion dates use phased math (`estimated_deletion_at_from_last_login`). See `src/social/community_graph/inactivity_batch.rs`, `AGENTS.md`.

**Comparative analogues:** ostracism, social credit, ecological carrying capacity—not criminal law. The login refund introduces a **rehabilitation-by-return** mechanic rare in platform ToS regimes.

### 3.2 Trophic spillover (symmetric)

Community edge rating changes propagate to global human rating at **±δ/6** (`judicial::community_member_delta_to_global_spillover`, `AGENTS.md`).

The pre-refactor **2× faster loss rate** (−δ/3 on losses) was removed. **Risk-averse polity** pressure now concentrates in **conversation settlement** (Loathed −6×δ edge, −3×δ global direct) rather than spillover alone.

### 3.3 Dunbar cap

Humans: **256** distinct active connections (`src/social/connections.rs`). Digital/org/community entities: **uncapped**.

**Asymmetry:** cognitively bounded citizens; uncapped institutional actors.

### 3.4 Noms as fiscal citizenship

| Action | Noms |
|--------|------|
| Post dispute | 1 |
| Membership review | **0** (constitutional product rule) |
| Constitutional / profile amendment | 32 |
| Community re-parent | 64 |
| Post dispute council appeal | 32 |

Accrual: +1 Nom per 13 feed ad cards (cap 256). Prices: `src/pricing.rs`.

Architects frame costs as **acts of conscience**, not revenue—but high-cost structural politics may stratify by ad exposure unless homeostasis is tuned (`SYSTEM_THEORY.md` §7).

---

## 4. Keystone immunity and residual sovereignty

### 4.1 Death-immune keystones

Cannot be selected out by fitness regime:

- Root communities (`parent_community_id IS NULL`)
- `256app`, `operating_charter_256`
- Twelve council domain handles
- Platform handle `256`, AI user types

Evidence: `src/social/community_graph/immune.rs`, `AGENTS.md`.

### 4.2 “Invisible 13th council member”

Documented product stance (`SYSTEM_THEORY.md` §5): solo operator as **de facto sovereign of last resort** alongside Council of 12; **no automated charter-amendment path** for keystones.

**Political reading:** **asymmetric federalism** favoring entrenched, unamendable centers. Subsidiarity for members; immunity for foundations.

---

## 5. Trust, safety, and legitimacy gaps

Graph/judiciary govern **governance fitness**, not **baseline safety** (`SYSTEM_THEORY.md` §9, `LAUNCH_READINESS.md`):

- No in-product reporting (deferred)
- No operator emergency takedown outside jury clock (deferred)
- **21-day** jury timelines inadequate for acute harm (CSAM, threats, harassment at scale)

**Legitimacy gaps** (governance lens):

| Gap | Risk |
|-----|------|
| Rating-weighted juries | Reputational oligarchy |
| Keystone immunity | Unaccountable constitutional core |
| Thin recovery ecology | Extinction without **visible** rehabilitation UX; login refund coded but not surfaced (`SYSTEM_THEORY.md` §2) |
| Cognitive opacity | Dual ratings, inheritance, jury weights (`SYSTEM_THEORY.md` §8) |
| Thin anathema enforcement | Procedure without visible consequence (`PRODUCT_GUIDE.md`) |
| Noms imbalance | Elite-only constitutional politics (`SYSTEM_THEORY.md` §7) |

---

## 6. Comparative politics table

| Real-world analogue | 256.app instantiation | Divergence |
|---------------------|-------------------------|------------|
| **Federalism** | Parent-tree constitution inheritance | Additive sections; no supremacy clause |
| **Subsidiarity** | Audience/member juries | Operator + AI council retain appellate/override roles |
| **Judicial review** | Post disputes vs effective constitution + evidence | No standing court; ad hoc cases |
| **Sortition** | Random draw, √n sizing | Rating-weighted votes |
| **Supermajoritarian constitutionalism** | Weighted ⅔; hung → status quo on amendments | Amendment juries scale with n (uncapped) |
| **Bicameral consent** | Re-parent phase 1 + phase 2 | Second “chamber” = stakeholder-expanded, not territorial |
| **Charter entrenchment** | Death-immune roots, council domains | No internal reform path |
| **Appellate review** | Council of 12, post disputes only | Simple majority; ties favor moderation |
| **Naturalization** | Membership review (free); deny → 1-year reapply | Pool skewed to high-rated members |
| **Administrative state** | Scheduler: human inactivity, death, scope jobs | Automated bureaucracy; login refund as restoring force |

---

## 7. Conclusion (journal-ready summary)

256.app demonstrates that social platforms **can be designed as federated constitutional orders** rather than feudal fiefdoms of Terms-of-Service. It implements transparent dispute procedure, human supremacy over constitutional politics, narrow AI appellate jurisdiction, and procedural jury protections (exclusions, frozen weights, automatic refill).

It is **not** a model for liberal-democratic governance online:

- **Reputational jury weighting** risks oligarchy of the well-rated.
- **Keystone immunity** without democratic amendment path concentrates sovereign power.
- **Fitness extinction** lacks robust **visible** due process of rehabilitation — login refund and softer δ improve the coded baseline; see [`DESIGN_BIAS_RECOVERY.md`](DESIGN_BIAS_RECOVERY.md) (owner acknowledgment: residual bias in conversation asymmetry).
- **Safety** is explicitly deferred— a polity without basic security functions.
- **Noms** may stratify access to high politics.

For scholarship, the platform offers three **testable propositions**:

1. Can sortition scale sublinearly (⌈√n⌉) while preserving perceived legitimacy?
2. Does reputation-weighted jury voting improve outcomes or entrench incumbents?
3. Can ecological “death” substitute for moderation, or does it relocate coercion into automated bureaucracy?

**Final assessment:** A provocative **laboratory for institutional innovation**, honest about evolutionary selection and keystone sovereignty, **not yet** a legitimate model for democratic governance at public scale until safety, explainability, recovery, and economic access to governance are addressed. **Owner follow-up:** [`DESIGN_BIAS_RECOVERY.md`](DESIGN_BIAS_RECOVERY.md).

---

## Relationship to other docs

| Document | Lens |
|----------|------|
| [`SYSTEM_THEORY.md`](SYSTEM_THEORY.md) | Backlog & triage — incomplete features |
| [`DESIGN_BIAS_RECOVERY.md`](DESIGN_BIAS_RECOVERY.md) | Owner reminder — downward spiral vs recovery bias |
| [`EVOLUTIONARY_BIOLOGY_REVIEW.md`](EVOLUTIONARY_BIOLOGY_REVIEW.md) | Ecological selection, carrying capacity, immune metaphor |
| [`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md) | Wellbeing, standing threat, jury burden |
| [`CLINICAL_PSYCHOLOGY_REVIEW.md`](CLINICAL_PSYCHOLOGY_REVIEW.md) | Clinical harm assessment, publication ethics |
| [`LAUNCH_READINESS.md`](LAUNCH_READINESS.md) | Product/ops gates; T&S as launch blocker |
| [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) | Tier 1–2 safety roadmap |
| [`COMMUNITY_GRAPH_MANAGEMENT.md`](COMMUNITY_GRAPH_MANAGEMENT.md) | Graph death, re-parent implementation |
| [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) | User-facing product canon |

**Quint framing:** This doc = **institutional** analysis; [`EVOLUTIONARY_BIOLOGY_REVIEW.md`](EVOLUTIONARY_BIOLOGY_REVIEW.md) = **ecological** analysis; [`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md) = **cyberpsychology research**; [`CLINICAL_PSYCHOLOGY_REVIEW.md`](CLINICAL_PSYCHOLOGY_REVIEW.md) = **clinical harm assessment**; [`SYSTEM_THEORY.md`](SYSTEM_THEORY.md) = **what to build next**.

---

## Evidence index

| Path | Role |
|------|------|
| `src/social/community_constitution.rs` | Inheritance, owned clauses, reparent diffs |
| `src/social/judicial/` | Triggers, pools, nomination, verdict, reparent |
| `src/social/judicial_council.rs` | AI council appeals |
| `src/social/judicial/verdict_eval.rs` | Weighted ⅔, hung defaults |
| `src/social/judicial/rating.rs` | Vote weight, spillover |
| `src/social/community_graph/immune.rs` | Death immunity |
| `src/social/connections.rs` | Dunbar cap |
| `src/pricing.rs` | Nom costs |
| `src/agent_specs.rs` | Council demarchy context in prompts |
| `docs/SYSTEM_THEORY.md` §5 | Invisible 13th council member |

---

## Suggested external publication venues

Depending on emphasis: *Perspectives on Politics*, *Journal of Democracy* (legitimacy); *Governance* (polycentric/Ostrom); *Information, Communication & Society* (platform governance empirics).

---

## Changelog

| Date | Change |
|------|--------|
| 2026-05-24 | **Rating-economics pass** — δ=0.035, symmetric spillover, phased human inactivity + login refund, community post-idle removed |
| 2026-05-24 | Initial political science review from product architecture analysis |
