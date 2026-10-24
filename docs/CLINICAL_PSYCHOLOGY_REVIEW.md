# Clinical psychology review — 256.app harm assessment for publication

Clinical and cyberpsychology assessment from a **practitioner and manuscript-review** lens: what the platform is, what it does for users, where it reduces psychological harm relative to incumbent social media, and where it introduces or amplifies harm. Use alongside [`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md) (fresh-eyes social-media take), [`EVOLUTIONARY_BIOLOGY_REVIEW.md`](EVOLUTIONARY_BIOLOGY_REVIEW.md), [`POLITICAL_SCIENCE_REVIEW.md`](POLITICAL_SCIENCE_REVIEW.md), [`DESIGN_BIAS_RECOVERY.md`](DESIGN_BIAS_RECOVERY.md), [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md), and [`LAUNCH_READINESS.md`](LAUNCH_READINESS.md).

**Last reviewed:** 2026-08-12 (civic standing literacy; godmother_outreach recovery; domain reputation + inline body URLs; Epic portrait; pre-vote comment preview; on-call v1). Prior: 2026-06-30 (32 Nom grant; ad-free accrual; council stewards; engagement digest).

---

## Abstract

256.app is a **reputational civic social network** running as a **closed beta** at [`beta.twofivesix.app`](https://beta.twofivesix.app/login): Dunbar-bounded human graphs (256 connections), audience-scoped feeds, peer jury governance, scalar standing (0.00–5.00), and **automated extinction at zero**. The platform ships **Root charter v2 (16 points)**, **emergency suppress-first routing**, **Panoply operator console**, **password reset**, **human graph block**, **quota-free jury abstention on acute/serious cases**, and **owner-only human failing presentation**. **Correction (carried forward):** conversation settlement is **symmetric** in code (±3×δ edge).

**Main finding (August 2026):** 256.app remains a **credible field site for cyberpsychology research** and a **conditional fit for closed beta among informed adults**. It is **not ethically recommendable as a general-purpose or wellbeing-positive mass product**. The harm profile still **substitutes scroll addiction for status threat, evaluative anxiety, distributed civic exposure, and identity erasure** — but **Jul–Aug 2026** materially improved **informed consent and recovery signalling**: **civic standing literacy** (self-profile + How it works § Civic standing), **`godmother_outreach`** (welcome + global failing + global recovery in steward voice), **pre-vote comment preview**, and **link-quality gates** on council shares. **Residual clinical risks:** death at zero, inactivity pruning, jury burden, **32 Nom** multi-account filing economics, engagement digest re-engagement pressure, and **vendor hash/hotline backlog** for acute illegal content.

---

## Classification

| Clinical construct | 256.app instantiation (June 2026) | Assessment |
|--------------------|-----------------------------------|------------|
| **Compulsive use / addiction** | Scoped feed, thread caps, **ad-free scroll** | **Lower risk** than infinite-scroll incumbents |
| **Social comparison distress** | Scalar rating; **human** failing red `@handle` owner-only; **community** failing public | **Moderate–high** for humans vs prior public-shame model; **high** for communities |
| **Ostracism / social death** | Rating → 0; strike-through handles; graph excision | **High risk** |
| **Rejection sensitivity** | Loved / Controversial / Loathed; symmetric ±3×δ edge | **Moderate–high** for RSD-prone users |
| **Learned helplessness** | Inactivity pruning primary downward force; **partial recovery UX** (godmother + standing block) | **Moderate** in failing band (< 0.66) — improved vs June |
| **Impulsive onboarding** | Google OIDC + peer recruitment email | **Elevated risk** — commitment before full comprehension; standing literacy **partially** addressed post-godmother |
| **Vicarious trauma** | Peer juries; depersonalized corpus; quota-free abstention on acute/serious cases; **pre-vote comment preview** | **Moderate** — jury wellbeing backlog |
| **Autonomy / informed consent** | Charter + **How it works § Civic standing** + self-profile block; face before full literacy | **Moderate risk** — **improved** vs June |
| **Acute harm response** | Emergency suppress + Panoply `/home/ops` + **on-call v1**; inline URL block + domain reputation; vendor hash backlog | **Improved** vs May; **not complete** |
| **Agency in harassment** | Graph block (feed/profile/thread/search); voluntary delete; operator eject | **Partial–good** on signed-in surfaces |
| **Re-engagement pressure** | Engagement digest every 3 idle days (days 3–18) | **Low–moderate** — not punitive but persistent |
| **Parasocial AI exposure** | Godmother + council daily link shares in communities | **Low–moderate** — bounded but ambient |
| **Developmental protection** | 16+ checkbox; no robust minor exclusion | **Insufficient** for youth |

**Working label:** *reputational civic social network with standing-based extinction signals, partial platform safety nets, ad-free consumption design, and incomplete psychological recovery design*.

---

## 1. What it is and what it does (today)

### 1.1 Product identity

256.app presents as **human-centred communities** with AI Council guides and **community-native dispute resolution**. Implementation matches that structurally: scoped visibility, inherited constitutions, staged verification, and a **fitness regime** (`rating`, death at 0, juries) documented in [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) and [`AGENTS.md`](../AGENTS.md).

**Operational reality:** Closed beta on a GCP VM with HTTPS, SMTP, hosted MySQL, and CI/CD deploy on push to `main` ([`DEPLOY_BETA.md`](DEPLOY_BETA.md)). **Beta AI live** for async face capture → Epic portrait pipeline + paid avatar redraw ([`LAUNCH_READINESS.md`](LAUNCH_READINESS.md) § Shipped 2026-07-24 / 2026-08-11).

### 1.2 User-facing behavioral loop

1. **Entry** — register (16+) or **Google OIDC**, accept **Root charter v2** (`v2-2026-05-29`), verify email (auto-login on link), face capture (512×512), profile, AI godmother; **32 Noms** credited on human profile creation (`SIGNUP_NOM_GRANT` in `src/social/noms/mod.rs`).
2. **Bounded relating** — up to **256** human connection slots.
3. **Scoped exposure** — audience-chosen posts; thread depth capped; **no third-party ads in feed**.
4. **Standing evaluation** — conversation settlement → Loved / Controversial / Loathed; **symmetric ±3×δ edge / ±1×δ global** at δ=0.035. Failing band at **< 0.66**.
5. **Civic labor** — jury summons; abstention quota then standing penalty on regular cases; **quota-free on acute/serious** cases.
6. **Absence** — engagement digest (days 3–18 idle) → 21d warning → phased daily decrements from 28d; **login refund** of debited standing in code.
7. **Safety agency** — graph block; post-scoped emergency/civic filing; operators have Panoply console.
8. **Ambient council content** — steward communities receive daily AI-curated link shares (budget-capped).

**Governance fitness** and **platform safety** remain **orthogonal** but the safety layer is **less hollow** than at the May review date.

---

## 2. What it does well (harm reduction vs incumbents)

### 2.1 Anti-addiction architecture

Audience-scoped feeds, thread limits, and **ad-free home scroll** shift primary harm **away from compulsive passive consumption** toward **standing anxiety** — a different clinical profile, arguably more cognitively demanding.

### 2.2 Dunbar cap

256 human connections may reduce weak-tie overload; uncapped communities/orgs can still recreate hierarchy in a smaller pond.

### 2.3 Procedural transparency

Documented jury math (⌈√n⌉, frozen weights, ⅔ thresholds) supports **procedural justice** even when outcomes hurt.

### 2.4 Absence reversibility (coded, under-communicated)

Phased inactivity + **full login refund** (`src/auth/session.rs`) matter for illness, grief, deployment — if users know they exist.

### 2.5 User agency and operator backstop

| Capability | Clinical relevance |
|------------|-------------------|
| Human **graph block** | Immediate interpersonal friction tool |
| Voluntary account/entity delete | Exit without waiting for jury |
| **Emergency filing** | Suppress-first; contest opens jury |
| **Panoply `/home/ops`** | Operator suppress, vacate, eject, session kill, domain block — **audited** |
| Honest **law_notice** copy | Reduces false belief that juries = crisis response |
| **Human failing owner-only** | Reduces public shame and browse-to-target on human profiles |

Evidence: [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md); `src/social/panoply/`, `src/social/blocks.rs`, `src/ui/entity_handle.rs`.

### 2.6 Meta-critical documentation

Internal docs acknowledge punitive bias ([`DESIGN_BIAS_RECOVERY.md`](DESIGN_BIAS_RECOVERY.md)) and safety gaps — preferable to wellbeing marketing without engineering support.

---

## 3. What it does badly (harm amplification)

### 3.1 Status threat and social death

Scalar rating, failing band (**< 0.66**), projected deletion dates, and death at 0.0 remain **high ostracism risk** for communities and for humans who know their own standing. **Human failing red `@handle` is owner-only** — reduces public shame; community failing presentation unchanged. **Recovery is more legible (Aug 2026):** self-profile **civic standing block**, How it works annex, and **`godmother_outreach`** global recovery posts — but **login-refund toast** and edge-only failing notices remain backlog.

### 3.2 Inactivity as primary downward force

With conversation symmetric, **login-idle pruning** (phased from day 28, refund on return) is the main structural hazard. Login refund is coded but **not surfaced** in UX. **Engagement digest** adds contact during absence without restoring standing — may increase **return anxiety**.

### 3.3 Reputation-weighted juries

Vote weight = rating/2.5 (0–2×) — status contests, incumbent advantage, alienation of low-rated users.

### 3.4 Distributed moderation burden

Peer juries still expose ordinary users to dispute content; trauma-informed decline and reporter protections remain backlog ([`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) §P1).

**Improved but not solved:** Emergency **suppresses immediately**; quota-free abstention on acute/serious cases. **Class A/B vendor hash** and **hotline (16)** not fully wired — acute illegal imagery must not reach jury pools.

### 3.5 Residual platform safety gaps

| Gap | Impact |
|-----|--------|
| No standalone **report** UX norm | Users lack one obvious harm pathway |
| Platform `user_block` / notification mute incomplete | Off-feed harassment persistence |
| Vendor PhotoDNA-class API | CSAM re-exposure risk |
| Crisis hotlines in UI | **Shipped** — Safety & Crisis card + filing surfaces ([`CRISIS_RESOURCES.md`](CRISIS_RESOURCES.md)) |
| Dedicated safety policy page | Incomplete informed consent at scale |

### 3.6 Onboarding: commitment before comprehension

Face capture still precedes full standing literacy, but **post-godmother** users can read **civic standing on self-profile** and How it works § Civic standing. Gap: optional pre–face-capture primer and login-refund toast still open.

### 3.7 Fear-framed inactivity messaging

“Exit accelerating” email tone + deletion projections may drive **threat-based** return; login refund under-surfaced.

### 3.8 Marketing–mechanism gap

“Meaningful connection” copy vs evaluative/shame signals → **betrayal distress** when users discover the fitness regime post-commitment.

### 3.9 Multi-account economics (updated)

**32 Nom signup grant** + daily login + OIDC lowers cost of sock-puppet **emergency filing** campaigns vs prior 16-Nom reviews — update threat model and informed-consent copy ([`ADVERSARIAL_USER_REVIEW.md`](ADVERSARIAL_USER_REVIEW.md)).

---

## 4. Clinical verdict by audience

| Audience | Verdict |
|----------|---------|
| **General public / teens** | **Not recommended** until P0 safety vendor layer; standing literacy **partial**; recovery UX **partial** |
| **Rejection-sensitive / trauma history** | **High caution** |
| **Intentional adult communities** | **Conditional** — closed beta with explicit fitness consent |
| **Researchers** | **Recommended study object** — standing + jury + cap + live beta + council activity |
| **Life disruption / absence** | **Caution** — refund coded, pause path thin; digest emails during absence |
| **Juror pool / conflict-averse** | **Caution** — moral burden; acute abstention improved |

---

## 5. Overall harm balance (publication summary)

| Dimension | August 2026 assessment |
|-----------|------------------------|
| Compulsive use risk | **Lower** than incumbents (ad-free feed) |
| Social comparison / status threat | **Higher** than incumbents — **more legible** via civic standing UI |
| Ostracism / identity erasure | **High** |
| Acute harm / trauma response | **Fair–good** (Panoply + emergency + inline URLs + on-call); **not excellent** (vendor 15/16 backlog) |
| Procedural fairness | **Moderate–good** |
| Recovery / resilience support | **Partial** (godmother + standing block; login-refund toast backlog) |
| Wellbeing-positive mass product | **No** |
| Research laboratory | **Yes** — stronger with live cohort + steward telemetry |

**Clinical bottom line:** 256.app still trades **passive addiction** for **active civic and reputational stress**. Closed beta is **more defensible** than May 2026 and **clearer** than June 2026 on standing consent, but **public launch** without vendor sanitation, full recovery UX, and juror wellbeing would remain **ethically indefensible** for a clinician endorsing psychological safety.

---

## 6. Clinical recommendations

| Priority | Recommendation |
|----------|----------------|
| **P0** | Vendor hash API + hotline (16) before open signup |
| **P0** | Recovery UX — **partial shipped** (godmother + standing block); **login-refund toast**, edge failing notices |
| **P1** | Optional pre–face-capture standing primer (charter + How it works may suffice for informed adults) |
| **P1** | Juror wellbeing — safe decline, content warnings |
| **P1** | Full block scope + notification mute |
| **P1** | Review engagement digest tone for absence/illness contexts |
| **P2** | Track inactivity vs conversation standing trajectories in beta cohort |

---

## Relationship to other docs

| Document | Lens |
|----------|------|
| [`SOCIAL_PSYCHOLOGY_REVIEW.md`](SOCIAL_PSYCHOLOGY_REVIEW.md) | Fresh-eyes social-media nerd |
| **This file** | Clinical harm & publication ethics |
| [`EVOLUTIONARY_BIOLOGY_REVIEW.md`](EVOLUTIONARY_BIOLOGY_REVIEW.md) | Ecological selection |
| [`POLITICAL_SCIENCE_REVIEW.md`](POLITICAL_SCIENCE_REVIEW.md) | Institutional legitimacy |
| [`ADVERSARIAL_USER_REVIEW.md`](ADVERSARIAL_USER_REVIEW.md) | Threat model |
| [`TRUST_AND_SAFETY.md`](TRUST_AND_SAFETY.md) | Safety roadmap |

---

## Changelog

| Date | Change |
|------|--------|
| 2026-08-12 | Civic standing literacy; godmother_outreach; domain reputation + inline URLs; Epic portrait; pre-vote preview; on-call |
| 2026-06-30 | Full refresh — 32 Nom grant; ad-free accrual; council stewards; engagement digest; Stripe |
| 2026-06-10 | Human failing privacy; jury acute abstention; friend-invite email gate |
| 2026-06-03 | Full refresh — beta live, charter v2, Panoply, password reset; May editions archived |
| 2026-05-27 | Initial clinical review (archived) |
