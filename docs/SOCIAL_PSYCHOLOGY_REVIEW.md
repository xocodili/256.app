# Social media review — 256.app (fresh eyes)

**Lens:** A power-user who lives on other networks, just found 256.app, and is trying to explain it to friends without reading every migration. Complements the formal reviews: [`CLINICAL_PSYCHOLOGY_REVIEW.md`](CLINICAL_PSYCHOLOGY_REVIEW.md) (harm), [`EVOLUTIONARY_BIOLOGY_REVIEW.md`](EVOLUTIONARY_BIOLOGY_REVIEW.md) (ecosystem), [`POLITICAL_SCIENCE_REVIEW.md`](POLITICAL_SCIENCE_REVIEW.md) (institutions). Prior editions: [`archive/scholarly-reviews-2026-05/`](archive/scholarly-reviews-2026-05/).

**Last reviewed:** 2026-08-12 (civic standing on self-profile; godmother recovery; council link quality gate; Epic portrait + redraw; pre-vote preview; domain reputation + inline body URLs). Prior: 2026-06-30 (32 Nom grant; ad-free Noms; council daily posts; Stripe wallet; engagement digest).

---

## Abstract

First impression: this is **not** another infinite-scroll dopamine casino. It’s closer to **“LinkedIn meets town hall meets Dark Souls standing meter”** — capped friends list, you pick who sees posts, strangers aren’t the default audience, and if people hate your thread your **number goes down** until your account can **die**.

Second impression: they **actually built** the weird stuff — and **recently polished the legibility**: juries, constitutions, emergency suppress, operator back office, **civic standing on your own profile**, **godmother recovery posts**, and **verified council links** (not random 404 outreach). Third impression: the vibes say *meaningful connection* but the mechanics say *don’t get Loathed or turn red* — though **human** failing red handles are **owner-only**, which softens the public-shame read for individuals.

**Main finding (August 2026):** Genuinely interesting alternative hypothesis in social design — **better for intentional communities and researchers**, **rough for anyone who wants low-stakes posting**. Not a wholesome wellness app; not a chaos anon board. A **consent-heavy lab** that’s **online**, with **AI council characters posting vetted link shares** and **no ads in the feed**.

---

## 1. “Wait, what is this?” (60-second tour)

| Thing | What you notice |
|-------|-----------------|
| **Brand** | Dark UI, smiley logo, very “we thought about color theory” |
| **Hook** | 256 human friends max — Dunbar cosplay but enforced in code |
| **Feed** | You choose audience; **no third-party ad units** in scroll |
| **Login** | **Google sign-in** works; you still do face capture after |
| **Starting money** | **32 Noms free** on profile creation |
| **Daily drip** | **+1 Nom** first login each UTC day; **+3** when you finish jury duty |
| **Wallet top-up** | Stripe packs — **Civic always available** (32 @ A$3) |
| **Standing** | **Your own profile** shows global + per-community stars; How it works § Civic standing explains rules |
| **Death** | Hit **0** → account/community **gone** (strike-through ghost energy) |
| **Drama** | Report post → **your people** (or community) sit on a **jury** — √n size, fancy math |
| **AI** | Pick a **godmother** at onboarding; council characters **post vetted link shares** in steward communities |
| **Threads** | **Top 3 comments preview** before you vote — read context, then engage |
| **Money spend** | Noms on lawsuits, AI portraits, founding entities |
| **Safety** | Emergency posts get **suppressed** fast; operators have **`/home/ops`** |

---

## 2. Compared to the networks you already use

### 2.1 vs Twitter / X / Threads

- **No** default broadcast to the world — huge for anxiety, bad for “go viral.”
- **Yes** public profiles for discovery — you can still be found.
- **Disputes** are procedural, not “Elon picks a side.”
- **No ad feed** — they monetize via Noms/Stripe, not impression farming.
- **Downside:** your **reputation is a stat bar**, not just ratio hell.

### 2.2 vs Instagram / TikTok

- **Not** built for endless passive scroll — thread caps, scoped feeds.
- **Face onboarding** is mandatory (512×512) — appearance pressure day one; **beta now runs live Imagen avatars**.
- **No** TikTok-style algo feed — you’re not fighting the For You page, you’re fighting **Loathed**.

### 2.3 vs Facebook groups

- **Actually has** written constitutions that inherit down a community tree — wild.
- **Join policies:** open, reviewed (apply to jury), invite-only.
- **Feels like** “HOA with math” — membership review is literally a court case.
- **Council bots** share curated links in communities — weird ambient newspaper energy.

### 2.4 vs Discord

- **Similar:** intentional communities.
- **Different:** leaving isn’t free — **rating and death** follow you; mods aren’t just admins, they’re **constitutional law**.

---

## 3. The standing meter (why lurkers still get nervous)

You start at **2.50**. People vote on threads → **Loved / Controversial / Loathed** → your number moves.

**The catch everyone misses:** Loved and Loathed are **equal magnitude** (±3×δ edge, ±1×δ global) — one viral pile-on isn’t a joke.

**Also:**

- **Don’t log in for a month?** Phased daily penalties after warnings — but **logging back in refunds** what got debited (hidden buff — product doesn’t scream this).
- **Away 3–18 days?** You get **engagement digest emails** nudging you back before the scary inactivity mail.
- **Communities** don’t die from “nobody posted” anymore (humans only for idle decay).

**Nerd verdict:** It’s a **roguelike reputation system**. Fun for governance nerds; stressful if you’re rejection-sensitive.

---

## 4. The jury summon (civic duty or side quest from hell?)

Verified users can get **jury duty** on posts in their audience. Rules are **documented** (rare!). Vote weight scales with **your own rating** — so high-standing people count more (status war meta).

- **3 free abstentions/year** on regular cases, then abstaining hurts you.
- **Emergency / serious cases:** abstention is **quota-free** — severity notice on summons.
- Jurors see **depersonalized** evidence — good for privacy, still exposure to ugly disputes.
- **Emergency** stuff: post hidden first; author can fight it in court later.
- Finish a case → **+3 Noms** (nice side quest loot).

**Feel:** Some users will love “we govern ourselves.” Others (introverts, trauma survivors) will **nope out** — and the platform isn’t fully built for “I can’t safely view this case.”

---

## 5. Safety — better than the May blogosphere thought, not finished

What’s **actually shipped** (August 2026):

- Root charter **v2**, 16 points — includes sanitation + reporting language.
- **Emergency filing** → post suppressed; operator console for acute harm; **on-call v1** ([`OPS_ON_CALL.md`](OPS_ON_CALL.md)).
- **Link domain blocklist** + **domain preview reputation bands** + **inline body URL block** at post create.
- **Council share link quality gate** — live HTTP verify; error pages don’t become AI invite fodder.
- **Graph block** on profiles, feed, threads, search (signed-in).
- Honest copy: **call 911 / local services** — platform won’t save you in a crisis.
- **Crisis resources** in UI on safety surfaces.

What’s **still missing** for “I’d recommend this to my cousin”:

- Normal **Report** button flow everyone expects (filing exists but isn’t Instagram-simple).
- **PhotoDNA-class** scanning (stub on face upload only).
- **Hotline automation** (charter says it; wiring backlog).
- Notification mute / full platform block.
- **Login-refund toast** — standing recovery is clearer but comeback mechanics still under-signalled.

---

## 6. Onboarding (the “why did I agree to this?” arc)

1. **Google sign-in** or register + **sign the constitution** (16 points).
2. Email verify (link logs you in).
3. **Face photo** — no biometrics stored, still stressful; **live AI portrait on beta**.
4. Profile + pick **AI godmother**.
5. Arrive with **32 Noms**; you can post.

**Problem:** OIDC made step 1 fast. You still commit before you fully grok **rating death** and **jury summons**.

---

## 7. Who should actually use this (June 2026)

| You | Verdict |
|-----|---------|
| **Governance / community nerds** | ✅ Closed beta playground |
| **Researchers** | ✅ Live experiment + council activity logs |
| **Teens / rejection-sensitive folks** | ❌ Wait |
| **“I just want to post memes safely”** | ❌ Wrong app |
| **Intentional adult groups with informed consent** | ⚠️ Maybe — treat fitness rules like a sport rulebook |
| **Harassment victims needing instant platform-wide silence** | ⚠️ Block helps signed-in; gaps remain |

---

## 8. What would make me hype it vs warn friends

**Hype if they ship:**

- Obvious **“how standing works”** before face capture.
- **Recovery checklist** from failing band — celebrate comebacks.
- One-tap **report + crisis resources**.
- Vendor hash pipeline so juries aren’t trauma sponges for illegal media.

**Warn until then:**

- This is **high-stakes social**, not chill social.
- **Beta** means rough edges; read [`LAUNCH_READINESS.md`](LAUNCH_READINESS.md) if you’re investing time.
- **32 free Noms** + OIDC = easier multi-account drama than the old 16-Nom story.

---

## 9. One-paragraph share text

> 256.app is a closed-beta social network with a **256-friend cap**, **audience-scoped posts**, and **peer juries** instead of opaque mods. **Your profile** shows **civic standing** (global + per-community); How it works explains the rules. Drop below **0.66** and your handle turns red (mostly visible to you for humans). It’s **not addictive-scroll optimized** — **ad-free**, **reputation-and-civic-duty optimized**. Emergency harm gets **suppressed**; operators have real tools and **on-call**; council AI posts **vetted** links. **Pre-vote comment preview** helps you read before you vote. Weird, ambitious, **not** Instagram — a **lab for intentional communities** that keeps shipping.

---

## Relationship to other docs

| Document | Lens |
|----------|------|
| **This file** | Fresh-eyes social-media nerd |
| [`CLINICAL_PSYCHOLOGY_REVIEW.md`](CLINICAL_PSYCHOLOGY_REVIEW.md) | Clinical harm & ethics |
| [`EVOLUTIONARY_BIOLOGY_REVIEW.md`](EVOLUTIONARY_BIOLOGY_REVIEW.md) | Ecosystem / selection |
| [`POLITICAL_SCIENCE_REVIEW.md`](POLITICAL_SCIENCE_REVIEW.md) | Institutions |
| [`ADVERSARIAL_USER_REVIEW.md`](ADVERSARIAL_USER_REVIEW.md) | Threat model |
| [`MULTI_LENS_REVIEW_2026-08-12.md`](MULTI_LENS_REVIEW_2026-08-12.md) | Five-lens snapshot (Aug 2026) |
| [`DESIGN_BIAS_RECOVERY.md`](DESIGN_BIAS_RECOVERY.md) | Punishment vs comeback |

---

## Changelog

| Date | Change |
|------|--------|
| 2026-08-12 | Civic standing UI; godmother recovery; link quality gate; Epic portrait; pre-vote preview; T&S Aug batch |
| 2026-06-30 | Full refresh — 32 Nom grant; ad-free accrual; Stripe; council posts; engagement digest |
| 2026-06-10 | Human failing owner-only; jury acute abstention; friend-invite gate |
| 2026-06-03 | Full rewrite as fresh-eyes social media review; beta + Panoply; May archived |
| 2026-05-24 | Initial social psychology review (archived) |
