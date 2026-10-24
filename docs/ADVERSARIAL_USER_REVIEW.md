# Adversarial user review — 256.app threat model

**Lens:** A calculating, instrumentally rational actor evaluating 256.app as a platform to exploit. Internal red-team artifact — identifies gaps that benefit everyone else. Use alongside [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md), [`CLINICAL_PSYCHOLOGY_REVIEW.md`](CLINICAL_PSYCHOLOGY_REVIEW.md), and [`LAUNCH_READINESS.md`](LAUNCH_READINESS.md).

**Last reviewed:** 2026-08-12 (domain reputation + inline body URL block; council share link quality gate; 32 Nom grant; on-call v1). Prior: 2026-06-10 (human failing owner-only; friend-invite email gate; jury acute abstention).

---

## Abstract

256.app runs on peer reputation, public scalar ratings, decentralized juries, and automated account death. From an adversarial lens this is a **system with attackable mechanics**, not just a social network.

**Summary verdict (August 2026):** Not a soft target, but not hardened either. The most powerful role (`@panoply` operator) remains structurally inaccessible. Conversation standing is **symmetric** (±3×δ edge). The sharpest harassment path is still **multi-account emergency filing** via OIDC + the **32 Nom** signup grant — **worse** than the prior 16-Nom model. **Community** failing bands (< 0.66, red `@handle`) remain a **public target list**; **human** failing is **owner-only**. **New mitigations (Jul–Aug 2026):** council **link quality gate** (live HTTP verify + error-page heuristics before share/invite LLM), **inline body URL block** at post create, **domain preview reputation bands**, and **v1 on-call** — reduce phishing/spam and embarrassing AI outreach, not emergency brigading.

---

## 1. Reputation leverage

**Conversation settlement is symmetric** (`src/social/judicial/types.rs`: `LOVED_EDGE = +3`, `LOATHED_EDGE = -3`). Pushing someone to the failing band via Loathed threads alone requires ~18 settled threads with no offset — a sustained campaign, not a drive-by.

**Emergency filing** (1 Nom, post suppressed, standing at T+3 if uncontested) is the sharper weapon. Caps: 3 filings/filer/UTC day; one emergency/post. Bad-faith filers who lose take **−6×δ**. Fresh OIDC accounts arrive with **32 Noms** — enough for **32** filings before discard (or civic/emergency mix).

**Failing band targeting:** rating < **0.66** turns **community** `@handle` red on public profiles. **Humans** see failing red styling on their own profile/feed only. Near-extinction **communities** remain legible via `/api/public/entity-profile/{handle}`.

---

## 2. Jury manipulation

Jury vote weight = rating/2.5 (0–2×). A faction of high-rated allies in a controlled community can skew outcomes — but jury exclusions catch sock-puppets linked via `created_by` / `user_social_entity_role`.

**Civic exhaustion:** flooding a community with filings drains jurors' 3 free abstentions/year on **regular** cases — **emergency safety** and **serious moderation tier** cases are quota-free (2026-06-10). Costs 1 Nom per civic filing; startup grants make throwaway accounts cheaper.

---

## 3. Noms economy

- **Startup grant arbitrage:** **32 Noms** × n OIDC accounts = n×32 judicial firepower. No multi-account detection in code.
- **Ad accrual farming:** **Removed** — ad-free feed; accrual is signup + daily login + jury duty only.
- **Stripe top-up:** Fiat packs live; does not reduce throwaway cost (payment identity separate from OIDC).

---

## 4. Link / content abuse

**Reduced (2026-07–08):**

- **Council share link quality gate** — live HTTP verify, 2xx HTML only, error-page title/description heuristics; catalogue `fetch_status = ok` filter; publish re-check (`src/social/council_portfolio.rs`, `src/content/og_fetch.rs`).
- **Inline body URL block** at post create — denylisted domains in post body text blocked at publish (`src/content/domain_reputation.rs`).
- **Domain preview reputation bands** on OG ingest — low-trust hosts surfaced/blocked earlier in the pipeline.

**Remaining:** invite tick still reads link posts without the full share gate until product unifies all AI link corpora; no multi-account detection; malicious but **live** article pages still valid for civic disputes.

---

## 5. Power acquisition

**@panoply** is the only role worth having: Noms-waived filing, eject, session kill, domain block, death immunity, concealment. Invite-only from existing operators; single structural operator (`@sphinx`) seeded; **on-call v1** documented ([`OPS_ON_CALL.md`](OPS_ON_CALL.md)). Audit log (`safety_operator_action`) records every action.

**Legitimate slow game:** build a high-rating invite-only community, seed allies, control local constitutional machinery. Architecturally legal.

---

## 6. What actually stops me

| Defense | Effectiveness |
|---------|---------------|
| Emergency caps (3/day, 1/post) | Slows single-account harassment |
| Bad-faith originator penalty (−6×δ) | Deterrent for persistent single accounts |
| Jury sock-puppet exclusions | Catches linked accounts |
| Panoply audit log + on-call v1 | Deterrent; ops response path exists |
| Symmetric conversation | No cheap discourse-collapse attack |
| 256 connection cap | Limits per-account reach |
| Council link quality gate | Blocks error-page / dead-link AI outreach |
| Inline body URL + domain reputation | Blocks obvious phishing/spam in posts |
| Platform recruitment dedupe | One invite email globally |

**Honest gaps:** no multi-account detection; **32 Nom** grants arm fresh accounts; no platform notification mute; no automated filing-pattern monitoring; single-operator roster depth; vendor hash API still backlog.

---

## 7. Verdict

Low ROI for most exploitation patterns. Realistic damage: **targeted harassment via multi-account emergency filing**, or **finishing off entities already in the failing band**. Architecture has real antiparasite logic; **Jul–Aug 2026** closed the **embarrassing AI outreach / bad-link** path; **operations layer** improved but remains thin for coordinated abuse.

---

## Relationship to other docs

| Document | Lens |
|----------|------|
| **This file** | Threat model / exploitation surfaces |
| [`MULTI_LENS_REVIEW_2026-08-12.md`](MULTI_LENS_REVIEW_2026-08-12.md) | Five-lens snapshot (Aug 2026) |
| [`POLITICAL_SCIENCE_REVIEW.md`](POLITICAL_SCIENCE_REVIEW.md) | Institutional legitimacy |
| [`CLINICAL_PSYCHOLOGY_REVIEW.md`](CLINICAL_PSYCHOLOGY_REVIEW.md) | Clinical harm |
| [`EVOLUTIONARY_BIOLOGY_REVIEW.md`](EVOLUTIONARY_BIOLOGY_REVIEW.md) | Ecosystem mechanics |
| [`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md) | Fresh-eyes social media |
| [`ADVERSARIAL_USER_REVIEW.md`](ADVERSARIAL_USER_REVIEW.md) | Threat model |
| [`MULTI_LENS_REVIEW_2026-08-12.md`](MULTI_LENS_REVIEW_2026-08-12.md) | Five-lens snapshot (Aug 2026) |
| [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) | Safety roadmap |

---

## Changelog

| Date | Change |
|------|--------|
| 2026-08-12 | Council link quality gate; inline body URLs; domain reputation; 32 Nom; on-call v1 — §3–§6 |
| 2026-06-10 | Initial adversarial user review |
| 2026-06-10 | Human failing band owner-only; acute-case abstention quota-free — §1–§2 |
