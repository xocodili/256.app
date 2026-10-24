# Design bias — downward spiral vs recovery

**Purpose:** A standing reminder for the product owner and future contributors. Read this before tuning ratings, inactivity, death, or jury penalties.

**Recorded:** 2026-05-24, after independent reviews from [`POLITICAL_SCIENCE_REVIEW.md`](POLITICAL_SCIENCE_REVIEW.md) and [`EVOLUTIONARY_BIOLOGY_REVIEW.md`](EVOLUTIONARY_BIOLOGY_REVIEW.md).

**Updated:** 2026-08-14 — **login-refund toast shipped**; civic standing literacy + godmother recovery shipped; residual bias is **UX-only** (edge failing notices). Prior: 2026-08-12 — civic standing literacy + godmother recovery **shipped**; residual bias was **UX-only** (login-refund toast, edge failing notices). Prior: 2026-06-11 — conversation settlement confirmed **symmetric** in code. Prior: 2026-06-03 closed-beta scholarly refresh; 2026-05-24 rating-economics pass.

---

## The consensus (all lenses — updated)

256.app is **stronger at removal than renewal**, but **less skewed than at the initial 2026-05-24 review**:

| Lens | Prior verdict (May 2026) | After rating-economics + June triage |
|------|--------------------------|--------------------------------------|
| **Evolutionary biology** | Extinction-heavy; losses **2× faster** on spillover | Softer δ; **symmetric spillover**; **symmetric conversation**; community lurker decay **removed**; **login refund** — **inactivity** is primary environmental stress |
| **Political science** | Downward pressure without rehabilitation | **Rehabilitation-by-return** coded; legitimacy gap is **visible recovery UX** + operator sovereignty |
| **Social psychology** | Hypervigilance, learned helplessness on absence | **Absence partially addressed**; human failing **owner-only**; **civic standing + godmother recovery + login-refund toast** improve literacy — edge-only failing notices still open |

This is not a bug report from users yet—it is a **structural observation** about how the rules compose. **Coded fairness is ahead of felt fairness** — refund and symmetric settlement exist; **comeback path is clearer (Aug 2026)** but not fully celebrated.

---

## What changed (2026-05-24 implementation)

| Mechanism | Before (initial review) | Now (code) |
|-----------|-------------------------|------------|
| Basal **δ** | 0.075 | **0.035** (`jury_civic_rating_delta`) |
| Edge → global spillover | +δ/6 gains, **−δ/3** losses | **±δ/6** symmetric |
| Community post idle | −δ weekly after 21d | **Removed** |
| Human login idle | −δ weekly after 21d | **21d warn → 28d start**; phased daily **−1× / −3× / −6× δ** |
| Sign-in | Reset counters only | **Full refund** of `inactivity_rating_debited` + reset |
| Conversation close | 21d max open or 3d idle | **3d idle + ≥3 non-author votes** |
| Juror vote reward | +δ (docs) | **+3×δ** (`RATING_MULT_JUROR_PARTICIPATION`) |

**Unchanged (still load-bearing bias):**

- **Death at zero** — terminal, not timeout
- **Inactivity pruning** — phased daily decrements from day 28 idle (softened by login refund, not removed)
- **No-jury penalty** — **−6×δ** on communities (now −0.21 at current δ)
- **Hung post-dispute juries default to upheld** — enforcement bias on speech
- **Thin failing → healthy UX** — red handles and warnings for **communities** (public) and **humans** (owner-only); **godmother_outreach** + civic standing block (2026-07–08); **login-refund toast** (2026-08-14); edge-only failing notices backlog
- **Standing literacy** — **partially shipped** (self-profile + How it works § Civic standing); optional pre–face-capture primer backlog

---

## Likely design bias (explicit)

The architecture may encode a **design preference** for **consequences that stick** over legible recovery. That showed up as asymmetric spillover and harsh idle clocks — **corrected** in the 2026-05-24 pass (symmetric spillover and conversation). What remains:

- Bias toward *“idle humans should fade”* — softened by refund, not removed
- Bias toward *“consequences should be visible”* (failing band, death) without equal energy for *“recovery should be legible”*
- Less bias toward *“communities die from silence”* — **removed**

**Hypothesis to revisit:** the product may still **feel** punitive because removal signals (red handles, suppressed posts, death) are louder than comeback signals (Loved settlement, login refund, juror +3×δ) — a **UX composition** problem, not a settlement multiplier problem.

---

## How the math stacks (current code)

Basal increment: **`jury_civic_rating_delta()` = 0.035** (`src/social/judicial/types.rs`).

Rough intuition from default **2.50**:

| Event | Edge (community) | Global |
|-------|------------------|--------|
| One **Loathed** conversation | **−0.105** (−3×δ) | **−0.0525** (−1×δ direct + spill) |
| One **Loved** conversation | **+0.105** (+3×δ) | **+0.0525** (+1×δ + spill) |
| One **Controversial** | **+0.035** (+1×δ edge) | **+0.0058** (spill only) |
| Juror vote (community edge) | **+0.105** (+3×δ) | **+0.0175** (spill) |
| **3 Loved + 3 Loathed** (year) | **~2.50** (neutral drift) | **~2.50** (neutral drift) |
| Inactivity to zero (never login) | — | **~38 decrement-days** from 2.50 after day-28 start |
| **Login after inactivity debits** | — | **Full refund** of debited amount |

Recovery **exists in code** (`conversation_rating_settlement` symmetric, jury **+3×δ**, login refund) and is **more visible** after login-refund toast (2026-08-14), though still quieter than failing-band UI, phased inactivity **while away**, and death at zero. See [`SYSTEM_THEORY.md`](SYSTEM_THEORY.md) §2.

---

## What this is not

- **Not a call to remove death or failing bands** — selective pressure is load-bearing (`AGENTS.md`, graph lifecycle).
- **Not claiming the system is “wrong”** — it may match intentional community standards.
- **Not user research** — no cohort data yet on whether failing entities actually recover.
- **Not “problem solved”** — settlement math is **symmetric**; recovery UX and standing literacy are **not shipped**.

It **is** a flag that **felt fairness lags coded fairness** until recovery and explainability are deliberately designed.

---

## Future-self checklist

Before shipping any change that **increases downward pressure**, ask:

1. **What is the symmetric or faster upward path?** (Not just “they can post better”—is it coded, legible, and tested?)
2. **Does this punish absence?** (Illness, grief, offline life — login refund helps; is it **visible**?)
3. **Does spillover or edge rating trap people in communities they did not choose?** (Spillover and conversation settlement are symmetric; inactivity is the main global downward force.)
4. **Can a failing user see a path back above 0.66 with named actions and dates?** (Mirror `estimated_deletion_at_from_last_login` for **recovery**, not just deletion.)
5. **Are we biasing enforcement because it is easier to specify than grace?**

Before **softening** the system, ask:

1. Does recovery create **cheap heartbeat** or **sock-puppet** loopholes? (`SYSTEM_THEORY.md` §3)
2. Does weakening death undermine **“communities earn their place”**? (Marketing anchor in `docs/PRODUCT_GUIDE.md`.)
3. Does full login refund encourage **hit-and-run** accounts? (Track `inactivity_rating_debited` refund frequency.)

---

## Direction (product, not yet scheduled)

Prioritized in [`SYSTEM_THEORY.md`](SYSTEM_THEORY.md) §2 (P0) and [`LAUNCH_READINESS.md`](LAUNCH_READINESS.md):

- **Recovery ecology UX (P0 β)** — **partial shipped:** global failing email + **`godmother_outreach`** recovery post; civic standing on self-profile; **login-refund toast** (2026-08-14); **backlog:** edge-only failing notices, full checklist
- **Standing literacy (P0 β)** — **shipped (2026-07-23):** How it works § Civic standing + self-profile block; optional pre–face-capture primer backlog
- **Post-quality upward path** — surface `conversation_rating_settlement` outcomes in profile; ensure Loved gains are **legible**
- **Conversation math** — **symmetric ±3×δ edge / ±1×δ global** in code since 2026-05-24; do not re-tune without user research
- **Optional Nom costs on post** (1 Nom) — anti-gaming for inactivity clocks; **do not add** without widening recovery (`SYSTEM_THEORY.md` §3)

Track metrics when available: **failing → healthy transitions**, time-to-recover, death rate by cause (inactivity vs jury vs conversation), **refund events per returning user**.

---

## Related docs

| Doc | Relevant section |
|-----|------------------|
| [`EVOLUTIONARY_BIOLOGY_REVIEW.md`](EVOLUTIONARY_BIOLOGY_REVIEW.md) | §2 selection, §11 conclusion |
| [`POLITICAL_SCIENCE_REVIEW.md`](POLITICAL_SCIENCE_REVIEW.md) | §3.1 rating, §5 legitimacy gaps, conclusion |
| [`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md) | §2 risk vectors, §5 product directions |
| [`CLINICAL_PSYCHOLOGY_REVIEW.md`](CLINICAL_PSYCHOLOGY_REVIEW.md) | §3 harm amplification, §4 audience verdict, §6 recommendations |
| [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) | P1 wellbeing guardrails (inactivity pause, recovery checklist) |
| [`SYSTEM_THEORY.md`](SYSTEM_THEORY.md) | §2 recovery ecology, §3 anti-gaming |
| [`LAUNCH_READINESS.md`](LAUNCH_READINESS.md) | Soft blockers — recovery ecology |
| [`MULTI_LENS_REVIEW_2026-08-12.md`](MULTI_LENS_REVIEW_2026-08-12.md) | Five-lens snapshot (Aug 2026) |

---

## Changelog

| Date | Change |
|------|--------|
| 2026-08-14 | **Login-refund toast shipped** — sign-in surfaces inactivity debit reversal on home shell |
| 2026-08-12 | Civic standing literacy + godmother recovery shipped — residual bias UX-only; edge failing notices backlog |
| 2026-06-11 | Correct conversation math to **symmetric**; shift residual bias framing to **UX-only**; P0 β priorities aligned with [`SYSTEM_THEORY.md`](SYSTEM_THEORY.md) |
| 2026-06-03 | Scholarly reviews refreshed for closed beta, charter v2, Panoply |
| 2026-05-24 | **Rating-economics pass** — document δ=0.035, symmetric spillover, phased inactivity, login refund, community post-idle removal |
| 2026-05-24 | Cross-ref [`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md), [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) |
| 2026-05-24 | Initial note — owner acknowledgment of punitive bias; cross-review consensus |
