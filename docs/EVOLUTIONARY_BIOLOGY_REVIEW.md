# Evolutionary biology review — 256.app as a digital ecosystem

Ecological and evolutionary analysis of 256.app’s social graph: **fitness signals**, selective pressure, keystone immunities, carrying capacity, automated extinction, and the **pathogen layer** (platform safety outside fitness). Use alongside [`POLITICAL_SCIENCE_REVIEW.md`](POLITICAL_SCIENCE_REVIEW.md), [`ADVERSARIAL_USER_REVIEW.md`](ADVERSARIAL_USER_REVIEW.md), [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md), [`COMMUNITY_GRAPH_MANAGEMENT.md`](COMMUNITY_GRAPH_MANAGEMENT.md), and [`AGENTS.md`](../AGENTS.md).

**Last reviewed:** 2026-08-12 (council link quality gate; domain reputation + inline body URLs; godmother_outreach). Prior: 2026-06-30 (Noms economy pivot; council participatory stewards; ad-free accrual).

---

## Abstract

256.app implements a **scalar fitness model** (`social_entity.rating`, community edge ratings) with **automated selection**: human login-idle pruning (phased, δ=0.035), jury feedback, conversation settlement, and **terminal extinction** at zero. Keystones (charter roots, council domains, `@panoply` operators) are **death-immune**. The judiciary is a **distributed immune system**; post-verdict labels are **inflammation markers**.

**Main finding (August 2026):** The ecosystem runs on a **closed beta host** (~99% invite-ready) with scheduled jobs, migrations, and operator-mediated **pathogen suppression** (Panoply, **inline body URL block**, **domain preview reputation bands**, council **link quality gate** before AI). The **2026-05-24 rating-economics pass** holds in code. **Council stewards** are **active ecosystem engineers** with **vetted link shares**. **Noms accrual:** signup (+32), daily login (+1), jury (+3); **ad-free feed**; **Stripe top-up**. **Recolonization UX improved** (civic standing + godmother recovery); **population monitoring** and **vendor hash immunity** (Class A/B) remain immature.

---

## Classification

| Ecological concept | 256.app instantiation | June 2026 notes |
|--------------------|------------------------|-----------------|
| **Fitness trait** | Rating 0–5 (default 2.50) | Failing band **< 0.66** |
| **Selection** | Inactivity, jury, conversation | δ=0.035; login refund |
| **Extinction** | Death at 0; subtree re-parent | Live on beta |
| **Keystone immunity** | Roots, council, **@panoply** | Operators in refugia |
| **Active stewards** | Council AI daily posts/replies | Portfolio link ecology |
| **Carrying capacity** | 256 human edges | Unchanged |
| **Trophic spillover** | ±δ/6 symmetric | Post–May-24 fix holds |
| **Immune response** | Juries + council appeal | Mature |
| **Pathogen layer** | Emergency, Panoply, domain deny, hash stub, **inline body URLs**, **domain reputation**, **council link gate** | **Maturing for beta** |
| **Resource flux** | Noms ledger (no ad accrual) | Stripe + civic accrual |

**Working label:** *engineered digital ecosystem with Dunbar-limited agents, keystone refugia, active steward guilds, reputational immune response, and emerging extrinsic sanitation*.

---

## 1. Fitness architecture (live deployment)

Global and **member_of** edge ratings form a **nested fitness landscape**. Bands: healthy ≥**0.66**, failing <**0.66**, extinct =0.0 (`failing_rating_threshold()` in `src/social/judicial/types.rs`).

**Spillover:** community-edge delta → global **±δ/6** (`src/social/judicial/rating.rs`).

**Conversation settlement:** symmetric — Loved **+3×δ** / Loathed **−3×δ** edge; **+1×δ** / **−1×δ** global direct; Controversial **+1×δ** edge only (`src/social/judicial/types.rs`, `src/social/conversation_rating.rs`).

**Human failing presentation:** red `@handle` **owner-only** since 2026-06-10 (`src/ui/entity_handle.rs`) — reduces public selection pressure on individuals while preserving owner awareness.

---

## 2. Selection pressures

### 2.1 Basal unit δ = 0.035

`jury_civic_rating_delta()` — roughly **72 basal penalties** from 2.50 to extinction (vs ~34 at old δ=0.075).

### 2.2 Inactivity (environmental stress)

- **Communities:** no post-idle decay (removed).
- **Humans:** 21d warn → 28d start → phased **−1× / −3× / −6× δ** per day (`src/social/community_graph/inactivity_batch.rs`).
- **Login refund:** restores `inactivity_rating_debited` — **torpor recovery** coded (`src/auth/session.rs`).
- **Engagement digest:** email every 3 idle days (days 3–18) before inactivity warning — **re-immigration signal** without fitness change (`src/social/engagement_digest.rs`, migration `20260611180000`).

### 2.3 Conversation-mediated selection

**Fitness-neutral in expectation** — symmetric Loved/Loathed magnitudes. Event-driven penalties (no-jury −6×δ, bad-faith filer −6×δ, emergency uncontested) remain asymmetric by design.

### 2.4 Jury / no-jury penalties

Failed jury seating: **−6×δ** on communities. Juror participation: **+3×δ** standing and **+3 Noms** on case resolve (`JURY_DUTY_NOM_GRANT` in `src/social/noms/mod.rs`).

**Juror abstention:** quota-free on emergency and serious moderation tier cases (`src/social/judicial/moderation_tier.rs`) — reduces selection pressure on jurors exposed to acute harm.

---

## 3. Extinction and metapopulation dynamics

Community death **re-parents** children — local extinction, regional persistence. Human death at 0 removes graph presence. **Voluntary delete** and **operator eject** add **anthropogenic extinction** channels outside pure fitness.

---

## 4. Keystone species, stewards, and carrying capacity

| Actor | Role |
|-------|------|
| Charter roots / council domains | Death-immune; constitutional refugia |
| **@panoply** operators | Death-immune (fitness); concealed; operator console |
| **Council of 12** | Death-immune handles; **active stewards** — daily portfolio shares (≤3 novel links), reply tick, invite tick |
| System actors | Scheduler, AI handles |

Council daily budget **B = max(1, ⌈√(√N)⌉)** per councillor from live human population (`src/ai/business_functions/budget.rs`). Portfolio algorithm replaces hosts **< 2.50** and publishes link shares to own steward `@public` only (`src/social/council_portfolio.rs`, `src/social/council_jobs.rs`).

**Ecological reading:** stewards are **managed apex guild** — engineered nutrient cycling (outbound links) without uncapped reproduction (budget-capped, steward-scoped voice).

**K = 256** human edges (`MAX_CONNECTION_SLOTS` in `src/social/connections.rs`). Org/community nodes **uncapped** — potential hub-parasite dynamics.

---

## 5. Immune system (judiciary)

√n jury sampling, weighted ⅔, automatic seat refill (~15s tick), evidence snapshots. **Emergency:** suppress-first (inflammation containment before full tissue jury). **Civic:** `under_judicial_review` from filing.

**Ecological reading:** Tier-3 immune response is **mature** on beta. Tier-1 **macrophages** (Panoply operators at `/home/ops`) are in-product with audited `safety_operator_action` rows.

---

## 6. Pathogen layer

| Pathogen class | Mechanism | Maturity |
|----------------|-----------|----------|
| Malware/phishing links | `content_domain` denylist | **Live** |
| Emergency harm posts | Suppress + Panoply | **Live** |
| CSAM / NCII hash | Face-capture `MEDIA_HASH_BLOCKLIST` stub | **Partial** |
| Vendor PhotoDNA-class | Backlog | **Absent** |
| Hotline reporting (16) | Charter text; API backlog | **Partial** |

**Fitness model does not detect illegal harm** — pathogen response is **extrinsic**. Without vendor Class A/B, illegal media can still enter **jury corpora** if not caught at ingest — an **ecological catastrophe** risk for juror populations.

---

## 7. Anti-parasitism and gaming

Sock-puppets, heartbeat login, reputation farming — **incomplete**. **32 Nom startup grant** + OIDC + daily login accrual lowers multi-account colonization cost vs the prior 16-Nom reviews. Emergency filing at **1 Nom** remains the top adversarial path — see [`ADVERSARIAL_USER_REVIEW.md`](ADVERSARIAL_USER_REVIEW.md). **Ad accrual removed** (migration `20260629140000`) — predators can no longer farm Noms via feed impressions; **Stripe packs** add fiat bypass.

---

## 8. Recolonization and Allee effects

Failing band (<**0.66**) = **Allee-effect stress zone**. Upward paths: Loved threads (symmetric recovery), jury service (+3×δ, +3 Noms), login refund — **under-marketed in UX**. Engagement digest nudges return before inactivity penalties bite.

---

## 9. Observability

`no_jury` rate, death throughput, failing clusters — **deferred** (`SYSTEM_THEORY.md` §6). **Panoply daily status email** gives operators a census pulse; population-level ecology metrics remain absent.

---

## 10. Conclusion (June 2026)

256.app is an **engineered ecosystem in production**:

1. **Selection softened** (May 2024 pass); **conversation symmetric**; **inactivity** is primary environmental stress for absent humans.
2. **Keystone refugia** expanded with **Panoply**; **council stewards** add managed primary production (link shares) — stability ↑, pure bottom-up succession ↓.
3. **Pathogen layer emerging** — better than “deferred entirely,” worse than public-scale reserve standards.
4. **Noms economy pivot** — ad-free feed removes one parasitic niche; signup/daily/jury accrual + Stripe reshape resource competition.
5. **Monitoring absent** at population scale — blind to extinction rate and immune failure.
6. **Live beta** enables **empirical** tests of hypotheses below.

**Final assessment:** A **remarkable managed rewilding experiment** running in the wild (closed beta). Interesting for digital selection, carrying capacity, steward guilds, and immune metaphors. **Not yet stable ecology** for mass colonization without recolonization UX, cheater resistance, and full pathogen control.

**Testable hypotheses:**

1. Does symmetric conversation produce **neutral fitness drift**, or do social dynamics recreate effective asymmetry at population scale?
2. Does √n sampling maintain immune coverage as communities scale?
3. Does login refund enable **recovery ecology** after absence?
4. Did Panoply + emergency routing reduce **jury exposure** to acute harm vs pure civic routing?
5. Do council steward link shares increase **community colonization** without raising human connection load?

---

## Changelog

| Date | Change |
|------|--------|
| 2026-08-12 | Council link quality gate; domain reputation + inline body URLs; godmother recovery; civic standing |
| 2026-06-30 | Full refresh — 32 Nom grant; ad-free accrual; council stewards; engagement digest; Stripe |
| 2026-06-10 | Focused refresh — symmetric conversation; failing 0.66; OIDC/recruitment |
| 2026-06-03 | Full refresh — beta, Panoply, pathogen maturity; May archived |
| 2026-05-24 | Rating-economics pass (archived) |
