# Evolutionary biology review — 256.app as a digital ecosystem

Ecological and evolutionary analysis of 256.app’s social graph, treating the platform as an **engineered ecosystem** with fitness signals, selective pressure, keystone immunities, carrying capacity, and automated extinction. Use alongside [`SYSTEM_THEORY.md`](SYSTEM_THEORY.md) (backlog & triage), [`POLITICAL_SCIENCE_REVIEW.md`](POLITICAL_SCIENCE_REVIEW.md) (institutional legitimacy), [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) (product canon), and [`AGENTS.md`](../AGENTS.md) (lifecycle rules).

**Last reviewed:** 2026-05-24 (rating-economics pass — δ=0.035, symmetric spillover, phased inactivity + login refund).

---

## Abstract

256.app implements a **scalar fitness model** (`social_entity.rating`, community `member_of` edge ratings) coupled to **automated selection**: human login-idle pruning (phased), jury-mediated civic feedback, conversation-quality settlement, and **terminal extinction** at rating zero (community death with subtree re-parenting; human account removal). Humans face a **Dunbar-scale carrying capacity** (256 connections); keystones (charter roots, council domains, system actors) are **death-immune**. The judiciary functions as a **distributed immune system**—local juries as tissue-level response, AI council appeals as systemic escalation, post-verdict labels as visible inflammation markers.

**Main finding:** 256.app **explicitly encodes evolutionary-ecological logic in production code**, not merely in metaphor. A **2026-05-24 refactor** softened basal selection (δ **0.075 → 0.035**), made community-edge → global spillover **symmetric (±δ/6)**, removed **community post-idle** decay, and added **login refund** of inactivity debits — moving the ecosystem from **extinction-heavy** toward **selective with credible torpor recovery**. **Conversation settlement asymmetry** (Loathed −6×δ edge vs Loved +3×δ) remains the dominant downward bias. The system is still **ecologically immature** for visible recolonization UX, anti-parasitism (gaming), and pathogen response (trust & safety outside fitness).

---

## Classification

| Ecological concept | 256.app instantiation | Evidence |
|--------------------|------------------------|----------|
| **Fitness trait** | Scalar rating 0.00–5.00 (default 2.50) | `social_entity.rating`, edge ratings |
| **Phenotypic bands** | Healthy ≥1.0; failing <1.0; dead = 0.0 | `AGENTS.md`, UI handles |
| **Selection pressure** | Inactivity, jury deltas, conversation settlement, no-jury penalty | `community_graph/`, `judicial/`, `conversation_rating.rs` |
| **Extinction event** | Community death ritual; human `to_be_deleted` | `death_community.rs`, `death_human.rs` |
| **Keystone immunity** | Roots, charter, council domains, AI/system handles | `immune.rs` |
| **Carrying capacity** | 256 human connections | `connections.rs` `MAX_CONNECTION_SLOTS` |
| **Trophic spillover** | Community edge → global rating (**±δ/6**, symmetric) | `judicial/rating.rs` |
| **Immune response** | Post disputes, jury nomination, council appeal | `src/social/judicial/` |
| **Pathogen layer** | Baseline illegal/harmful content | **Deferred** — `SYSTEM_THEORY.md` §9 |

**Working label:** *engineered digital ecosystem with Dunbar-limited agents, keystone-protected niches, and reputation-mediated immune response*.

---

## 1. Fitness architecture

### 1.1 The rating as phenotype

Every human and community carries a **continuous fitness proxy** on `social_entity.rating` (0.00–5.00). New entities default to **2.50**—neutral standing, mapping to **unit jury vote weight** (rating / 2.5 = 1.0 vote at invite).

Community membership adds a **second fitness surface**: `member_of` edge ratings on `social_entity_relationship`. A single organism thus has **global** and **local (community) phenotypes**—a nested fitness landscape without full separation of niches.

| Band | Ecological reading | System behavior |
|------|-------------------|-----------------|
| **≥ 1.0** | Viable / healthy | Normal presentation |
| **< 1.0** | Stressed / failing | Red handle, warnings; structural mutations blocked |
| **= 0.0** | Extinct | Death batch: graph excision or community dissolution |

Evidence: [`docs/COMMUNITY_GRAPH_MANAGEMENT.md`](COMMUNITY_GRAPH_MANAGEMENT.md) product summary; `AGENTS.md`.

### 1.2 Trophic spillover (symmetric coupling)

When a human’s community-edge rating changes, global rating updates at **±δ/6** — same divisor for gains and losses:

```text
edge delta  →  global delta / 6
```

Evidence: `src/social/judicial/rating.rs` (`COMMUNITY_MEMBER_TO_GLOBAL_RATING_SPILLOVER_DIVISOR`, `community_member_delta_to_global_spillover`); test `community_member_global_spillover_is_one_sixth`.

**Ecological interpretation (2026-05-24):** The pre-refactor **2× faster loss propagation** (+δ/6 vs −δ/3) was removed. Local harm no longer **double-integrates** into global fitness relative to local good. **Residual asymmetry** now lives mainly in **conversation settlement** (Loathed applies −6×δ on edge vs Loved +3×δ) and **stacked global direct penalties** on Loathed (−3×δ direct plus spill).

---

## 2. Selection pressures

### 2.1 Basal metabolic unit: `jury_civic_rating_delta`

Most civic increments/decrements scale from **δ = 0.035** (`jury_civic_rating_delta()` in `src/social/judicial/types.rs`), via `civic_rating_delta_times(multiplier)`. Shared multipliers include **`RATING_MULT_JUROR_PARTICIPATION` = 3**, **`RATING_MULT_ORIGINATOR_CLEAR` = 6**.

At default 2.50, roughly **72 basal-unit penalties** (ignoring phased inactivity acceleration and bounds) would drive an entity from neutral to extinction — roughly **2× more tolerance** than at the old δ=0.075 (~34 steps).

### 2.2 Inactivity pruning (environmental stress)

**Communities:** **no longer** penalized for post idle (`inactivity_batch.rs` module comment; `community_graph_lifecycle` runs human login-idle only). `last_post_activity_at` still tracked for product/analytics touch paths.

**Humans:** phased **login-idle** clock on `users.last_login_at`:

| Milestone | Idle days | Penalty |
|-----------|-----------|---------|
| Warning email | **21+** | None |
| Daily decrements begin | **28+** | Phased daily global −δ |
| Decrement phase 1 | Days 1–21 | **−1×δ**/day |
| Decrement phase 2 | Days 22–42 | **−3×δ**/day |
| Decrement phase 3 | Day 43+ | **−6×δ**/day |

From rating 2.50, **~38 decrement-days** reach zero if the user never returns (`decrement_days_until_rating_zero`).

**Login refund:** `record_user_login` (`src/auth/session.rs`) credits back **`inactivity_rating_debited`** to global rating and resets counters — **seasonal torpor with metabolic reversal**, not irreversible senescence from absence alone.

Evidence: `src/social/community_graph/inactivity.rs`, `inactivity_batch.rs`; `AGENTS.md`.

**Interpretation:** Activity-dependent maintenance cost applies to **humans who stay away**, with staged acceleration — but **return restores debited fitness**. First crossing below 1.0 still triggers **stress signaling** (email, UI). At 0.0, **extinction** unchanged.

### 2.3 Judicial civic feedback (social immune response)

**Jurors who vote:** **+3×δ** on relevant rating surface (community edge or global).

**Excess abstainers** (>3 per rolling year): **−3×δ** — penalty for **non-participation in immune response**.

**Post-dispute originator (clear verdict, not hung):** **±6×δ** on community edge or global.

**Community timing feedback** (post-dispute / membership cases): communities gain **+3×δ** if resolved ≤3 days, **+1×δ** if ≤7 days, **0** if ≤14 days; **no bonus** beyond 14 days.

**No-jury timeout (21 days open):** community penalized **−6×δ** for post-moderation contexts — **−0.21** at current δ (was −0.45).

Evidence: `src/social/judicial/rating.rs`, `deadlines.rs`; `AGENTS.md` judiciary section.

### 2.4 Conversation settlement (quality-mediated selection)

When a post/reply conversation closes after **3 days** inactivity **and** **≥3 non-author votes**, `conversation_rating_settlement` applies **engagement-derived** deltas:

| Feel (from engagement stars) | Community edge mult | Global direct mult | Global spill (from edge) |
|------------------------------|---------------------|--------------------|---------------------------|
| Loved | +3×δ | +1×δ | +δ/6 of edge delta |
| Controversial | +1×δ | 0 | +δ/6 of edge delta |
| Loathed | −6×δ | −3×δ | −δ/6 of edge delta |

Evidence: `src/social/conversation_rating.rs` (`CONVERSATION_INACTIVITY_DAYS = 3`, `CONVERSATION_MIN_VOTES = 3`).

**Interpretation:** Partial **recovery ecology** — quality participation can restore fitness. Loathed content still applies **strong negative selection** (2× edge magnitude vs Loved; 3× global direct vs Loved). **Matched engagement** (3 Loved + 3 Loathed/year, community context) lands ~**2.19 edge / 2.24 global** from 2.50 — viable vs pre-refactor collapse. Backend path exists; **user-facing rehabilitation UX remains thin** (`SYSTEM_THEORY.md` §2).

### 2.5 Noms (metabolic currency)

Noms gate expensive governance acts (1–64 per filing). Accrual via ad exposure (+1 / 13 cards, cap 256). **Membership review is free**—admission is not wealth-gated.

**Ecological reading:** **energy budget** for costly collective behaviors (dispute, amendment, re-parent). Risk: **oligotrophic governance** if accrual is slow relative to structural mutation costs (`SYSTEM_THEORY.md` §7).

---

## 3. Extinction and population dynamics

### 3.1 Community death (`community_death_tx`)

At rating 0.0, non-immune communities undergo:

1. Status → `dead`
2. All `member_of` edges ended
3. Owned constitution retired
4. Open judicial cases voided
5. **Child communities re-parented** to dead node’s parent
6. Scope invalidation (visibility graph rebuilt)

Evidence: `src/social/community_graph/death_community.rs`; [`COMMUNITY_GRAPH_MANAGEMENT.md`](COMMUNITY_GRAPH_MANAGEMENT.md).

**Interpretation:** **local extinction with lineage rescue**—descendants are not culled, but **re-nested** under surviving ancestor. Analogous to **local population crash** with **metapopulation persistence** via parent habitat.

### 3.2 Human death (`human_death_tx`)

At rating 0.0, non-immune humans: all relationships ended, open cases voided, user → `to_be_deleted`, sessions cleared, public discovery hidden, historical posts show strike-through dead handles.

Evidence: `src/social/community_graph/death_human.rs`; `AGENTS.md`.

**Interpretation:** **individual removal from gene pool**—no offspring re-parenting analogue; total excision from the social graph.

### 3.3 Death batch scheduler

`community_graph_lifecycle` job processes death candidates + inactivity batches each tick—**automated trophic cascade** without human operator per entity.

Evidence: `src/social/community_graph/mod.rs` (`run_death_batch`).

---

## 4. Keystone species and refugia

### 4.1 Death immunity

**Communities immune from auto-extinction:**

- Root communities (`parent_community_id IS NULL`)
- Product handles: `256app`, `operating_charter_256`
- Twelve **council domain** communities (creativity, fitness, intellectual life, etc.)

**Humans immune:**

- Platform handle `256`
- `users.user_type = ai`

Evidence: `src/social/community_graph/immune.rs` (`COUNCIL_COMMUNITY_HANDLES`, etc.).

**Ecological interpretation:** **keystone refugia**—foundational niches that cannot be selected out by the fitness regime. Necessary for **ecosystem stability** (always-on intention domains, onboarding anchors) but create **persistent structural inequality** in the fitness landscape. Politically analyzed in [`POLITICAL_SCIENCE_REVIEW.md`](POLITICAL_SCIENCE_REVIEW.md) §4.

### 4.2 The operator as extrinsic selector

Documented **“invisible 13th council member”** (`SYSTEM_THEORY.md` §5): platform operator as **external anthropogenic force** outside coded selection—equivalent to **park ranger** or **god-model** intervention not subject to fitness rules.

---

## 5. Carrying capacity and niche asymmetry

### 5.1 Dunbar limit (256)

Humans: max **256** distinct active connections (`MAX_CONNECTION_SLOTS`). Digital actors, communities, orgs, gatherings: **uncapped**.

Evidence: `src/social/connections.rs`; `AGENTS.md`.

**Interpretation:** Explicit **cognitive carrying capacity** for Homo sapiens nodes only—grounded in Dunbar’s number literature. Institutional entities may form **dense hub networks** humans cannot replicate—a **trophic level asymmetry** where non-human nodes occupy **super-connective niches**.

### 5.2 Population caps (open questions)

Profit/non-profit death at rating zero: **not defined** (`SYSTEM_THEORY.md` §4). Digital actors uncapped—** invasive species risk** if bad actors spawn unlimited org nodes.

---

## 6. Immune system metaphor (judiciary)

### 6.1 Local response

**Post disputes:** jury from post **audience**—**tissue-level** response where antigen (post) is presented.

**Membership / amendments / re-parent:** community or subtree pools—**organ-level** or **metapopulation-level** coordination.

Jury size ⌈√n⌉: **sublinear sampling** of effector cells—statistical immune surveillance without mobilizing entire population.

### 6.2 Memory and weighting

Vote weight from **frozen rating at invite**: high-fitness jurors exert stronger effector response—** affinity maturation** by prior civic standing, not by antigen exposure alone.

### 6.3 Systemic escalation

AI Council of 12: **post-dispute appeals only**—hypothalamic escalation when local inflammation (human jury) is contested. Uses simple majority, ties favor moderation—**pro-inflammatory default** on appeal ambiguity.

### 6.4 Visible inflammation (partial)

Feed renders removed/exonerated post labels (`src/ui/post_card/feed.rs`)—**macroscopic signs of immune activity**. Full `anathema` consequence cascade deferred (`docs/PRODUCT_GUIDE.md`).

---

## 7. Anti-parasitism and signal integrity

| Threat | Defense | Status |
|--------|---------|--------|
| **Heartbeat posting** to reset activity clocks | Planned Nom costs (1 post, 3 moderate) | **Not wired** |
| **Sock-puppetry** | Nom costs + connection cap + jury exclusions | Partial |
| **Login-only survival** | Login **refunds** inactivity debits; quality path still separate | Partially addressed — refund reduces pure clock-gaming |
| **Low-effort recolonization** | Death removes entity; re-registration possible | Open |

Product stance (`SYSTEM_THEORY.md` §3): non-viral shape raises parasite cost; **watch production** before dedicated anti-gaming sprint.

**Ecological gap:** **no dedicated immune surveillance for cheaters**—selection assumes honest activity signals.

---

## 8. Pathogens outside the fitness model

Graph/judiciary select for **governance fitness** (constitutional compliance, civic participation)—not **baseline harm** (illegal content, acute harassment, minors).

**Interpretation:** The ecosystem has a **mature intraspecific competition regime** but **no native pathogen response**. Relying on 21-day jury clocks for acute harm is like **using evolutionary selection to treat cholera**—wrong timescale, wrong mechanism.

Public launch requires extrinsic **sanitation layer** (reporting, operator takedown)—see [`LAUNCH_READINESS.md`](LAUNCH_READINESS.md), [`POLITICAL_SCIENCE_REVIEW.md`](POLITICAL_SCIENCE_REVIEW.md) §5.

---

## 9. Comparative ecology table

| Biological analogue | 256.app mechanism | Divergence |
|---------------------|-------------------|------------|
| **Fitness landscape** | Scalar rating + edge ratings | Single axis; bounded 0–5 |
| **Natural selection** | Inactivity, jury, conversation deltas | **Automated**, not emergent |
| **Extinction** | Rating = 0 | Deterministic, not stochastic |
| **Keystone species** | Death-immune council/charter | **Engineered**, not emergent |
| **Carrying capacity (K)** | 256 human edges | Species-specific K |
| **Metapopulation dynamics** | Community tree, death re-parent | Children survive local extinction |
| **Immune system** | Juries + council appeal | Codified, √n-sampled |
| **Symbiosis** | Human–AI godmother links | Onboarding mutualism |
| **Parasitism** | Sock-puppets, heartbeat gaming | Incomplete defenses |
| **Invasive species** | Uncapped digital/org nodes | Open design question |
| **Recolonization / recovery** | Conversation settlement, jury service (+3×δ), login refund | Downward bias reduced; conversation asymmetry remains |
| **Allee effect** | Failing band <1.0 | Stress zone before extinction |
| **Ecosystem monitoring** | Ops metrics deferred | **Blind to no_jury, death throughput** |

---

## 10. Observability and homeostasis

Desired but **deferred** (`SYSTEM_THEORY.md` §6):

- `no_jury` rate (failed immune response)
- Death-batch throughput (extinction rate)
- Failing clusters (epidemiology)
- Scope recalc lag (materialized niche boundaries)

Without these, managers cannot assess **population viability**—equivalent to managing a reserve without census data.

**Noms homeostasis** (§7): balance ad accrual vs governance spend—**resource flux** stability.

---

## 11. Conclusion (journal-ready summary)

256.app treats social participation as **ecological viability** rather than infinite growth. Its **metabolic layer**—scalar fitness, quantized δ (**0.035**), phased human inactivity with login refund, jury feedback, conversation settlement, extinction at zero—is **implemented and load-bearing**. The platform exhibits rare design coherence: Dunbar capacity limits, keystone immunities, **symmetric spillover**, subtree-preserving community death, and judiciary-as-immune-system are not marketing metaphors; they are **scheduled jobs and SQL transactions**.

From an evolutionary-ecology standpoint, the **2026-05-24 refactor** rebalanced selection:

1. **Downward selection softened** — lower δ, no community lurker decay, refundable human absence; no-jury and Loathed penalties smaller in absolute terms.
2. **Upward selection unchanged in structure** — jury participation (+3×δ), loved conversation (+3×δ edge), fast community dispute resolution (+3×δ community).
3. **Primary residual bias** — **conversation Loathed asymmetry** and stacked global direct penalties on bad posts.
4. **Keystone refugia** stabilize the graph but prevent **pure bottom-up ecological succession**.
5. **Anti-parasitism and pathogen layers are immature** — fitness signals are gameable; harm outside governance fitness is unaddressed.
6. **Monitoring is absent** — no population ecology dashboard for a system that **automates extinction**.

**Final assessment:** 256.app remains a **remarkable engineered ecosystem**—closer to **managed rewilding with codified trophic rules and seasonal torpor** than to a standard social network. It would interest evolutionary biologists studying **digital selection**, **carrying capacity in social graphs**, and **immune metaphors for distributed governance**. It is **less extinction-heavy** than at the initial 2026-05-24 review but **not yet a stable ecology** for large-scale colonization without visible recolonization UX, cheater resistance, and pathogen control.

**Owner follow-up:** Product owner acknowledges this imbalance may reflect **personal bias toward consequences over comeback** — [`DESIGN_BIAS_RECOVERY.md`](DESIGN_BIAS_RECOVERY.md).

**Testable hypotheses for field study:**

1. Does **conversation asymmetry** (Loathed −6×δ vs Loved +3×δ) produce **rating collapse cascades** in simulated or live cohorts?
2. Does √n jury sampling maintain **effective immune coverage** as community size increases?
3. Does Dunbar capping humans while leaving orgs uncapped create **hub-parasite dynamics**?
4. Does **login refund** enable **recovery ecology** after absence, or do conversation/jury losses dominate irreversibility for failing entities?
5. Did **symmetric spillover** (post-refactor) measurably reduce global rating collapse vs the old ±δ/6 / −δ/3 rule?

---

## Relationship to other docs

| Document | Lens |
|----------|------|
| [`SYSTEM_THEORY.md`](SYSTEM_THEORY.md) | **Backlog & triage** — incomplete features, P0/P1 priorities |
| [`DESIGN_BIAS_RECOVERY.md`](DESIGN_BIAS_RECOVERY.md) | Owner reminder — punitive bias, future-self checklist |
| [`POLITICAL_SCIENCE_REVIEW.md`](POLITICAL_SCIENCE_REVIEW.md) | Legitimacy, federalism, keystone sovereignty |
| [`COMMUNITY_GRAPH_MANAGEMENT.md`](COMMUNITY_GRAPH_MANAGEMENT.md) | Death/re-parent implementation spec |
| [`LAUNCH_READINESS.md`](LAUNCH_READINESS.md) | Product gates; T&S as launch blocker |
| [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) | Consolidated safety roadmap (Tier 1–2) |
| [`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md) | Wellbeing lens; fitness ≠ safety |
| [`CLINICAL_PSYCHOLOGY_REVIEW.md`](CLINICAL_PSYCHOLOGY_REVIEW.md) | Clinical harm assessment; publication ethics |
| [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) | User-facing product canon |

**Quint framing:** This doc = **ecological/evolutionary** analysis; [`POLITICAL_SCIENCE_REVIEW.md`](POLITICAL_SCIENCE_REVIEW.md) = **institutional** analysis; [`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md) = **cyberpsychology research**; [`CLINICAL_PSYCHOLOGY_REVIEW.md`](CLINICAL_PSYCHOLOGY_REVIEW.md) = **clinical harm assessment**; [`SYSTEM_THEORY.md`](SYSTEM_THEORY.md) tracks **what to build next**; scholarly reviews explain **what the ecosystem is**.

---

## Evidence index

| Path | Role |
|------|------|
| `src/social/community_graph/` | Death, inactivity, immunity, death batch |
| `src/social/community_graph/immune.rs` | Keystone handles |
| `src/social/connections.rs` | Dunbar cap (256) |
| `src/social/judicial/rating.rs` | Spillover, civic settlement, timing bonuses |
| `src/social/judicial/types.rs` | `jury_civic_rating_delta` = **0.035**; multiplier constants |
| `src/auth/session.rs` | Login refund of inactivity debits |
| `src/social/judicial/deadlines.rs` | No-jury −6×δ penalties |
| `src/social/conversation_rating.rs` | Quality-mediated selection |
| `src/social/judicial/` | Immune response machinery |
| `docs/SYSTEM_THEORY.md` | Backlog §1–9 |
| `AGENTS.md` | Load-bearing lifecycle rules |

---

## Suggested external publication venues

Depending on emphasis: *Trends in Ecology & Evolution* (commentary); *BioScience* (digital ecosystems); *Nature Human Behaviour* (social graphs); *Artificial Life* (engineered selection); *Proceedings of the Royal Society B* (if empirical cohort study follows).

---

## Changelog

| Date | Change |
|------|--------|
| 2026-05-24 | **Rating-economics pass** — δ=0.035, symmetric spillover, phased inactivity + login refund, community post-idle removed |
| 2026-05-24 | Initial evolutionary biology review from architecture analysis |
