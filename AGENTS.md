# Agent / contributor notes

Mirrored for Cursor as **always-on** rules: `.cursor/rules/social-graph-connection-limits.mdc`, `.cursor/rules/mobile-first-ui.mdc`, `.cursor/rules/image-processing-standard.mdc`, `.cursor/rules/ui-color-palette-usage.mdc`, `.cursor/rules/ai-token-monetization.mdc`.

**Launch planning:** [`docs/LAUNCH_READINESS.md`](docs/LAUNCH_READINESS.md) — product assessment, checklists, and domain scorecard (2026-05-24). **Beta CI/CD (confirmed 2026-08-12):** [`docs/DEPLOY_BETA.md`](docs/DEPLOY_BETA.md). **Product & brand:** [`docs/PRODUCT_GUIDE.md`](docs/PRODUCT_GUIDE.md). **Doc map:** [`docs/README.md`](docs/README.md).

**Governance theory:** [`docs/SYSTEM_THEORY.md`](docs/SYSTEM_THEORY.md) (backlog); scholarly reviews in `docs/EVOLUTIONARY_BIOLOGY_REVIEW.md`, `docs/POLITICAL_SCIENCE_REVIEW.md`, `docs/SOCIAL_PSYCHOLOGY_REVIEW.md`, `docs/CLINICAL_PSYCHOLOGY_REVIEW.md`. **Root charter (shipped 2026-05-27):** [`docs/ROOT_CONSTITUTION_PROPOSAL.md`](docs/ROOT_CONSTITUTION_PROPOSAL.md). **Safety:** [`docs/TRUST_AND_SAFETY.md`](docs/TRUST_AND_SAFETY.md). **Crisis resources design:** [`docs/CRISIS_RESOURCES.md`](docs/CRISIS_RESOURCES.md). **Owner reminder:** [`docs/DESIGN_BIAS_RECOVERY.md`](docs/DESIGN_BIAS_RECOVERY.md).

## Content model (hard rule)

**Link-and-text-only** social product: posts are **text + attached content links** (OG preview); users follow links **off-site** to view source material and discuss on-platform. **Do not persist user-uploaded files** on posts or profiles. **Posts are immutable after publish** — no author edit or routine delete; authors **apologize via new reply**, not revision ([`docs/PRODUCT_GUIDE.md`](docs/PRODUCT_GUIDE.md) § Post immutability).

**Exception:** **generated avatars only** — face-capture JPEGs are transient (vision → portraiture → discard); durable path is `user_faces.generated_artwork_path`. Media hash / vendor Class A/B gates apply to **avatar ingress** (`read_face_capture_upload`), not to posts. Canon: [`docs/PRODUCT_GUIDE.md`](docs/PRODUCT_GUIDE.md) § Content model.

**Branded post authorship:** create-post **Post as** (`GET /me/posting-entities`) allows human, profit, non-profit, and gathering entities only — **not communities**. Branded publishes debit Noms (profit **4**, non-profit & gathering **2**; human free) via `BillableService::BrandedPost*` on `create_post`. Unaffordable brands stay in the picker with red cost and `can_afford_post: false`. See [`docs/PRODUCT_GUIDE.md`](docs/PRODUCT_GUIDE.md) § Branded posts.

## UI: mobile-first

Ship CSS and layouts for **narrow viewports first**; use **`min-width`** breakpoints to enhance for tablet/desktop (not the reverse). Prefer **safe-area** padding, **≥44px** touch targets for primary controls, **`font-size` ≥16px** on inputs to avoid iOS zoom-on-focus, and keep **binary-bg** particle density lower under **~480px** width for battery/CPU.

## Social graph lifecycle (communities + humans)

Scheduler job **`community_graph_lifecycle`** (`src/social/community_graph/`) processes both entity types each tick.

**Incomplete ecosystem / viability backlog:** [`docs/SYSTEM_THEORY.md`](docs/SYSTEM_THEORY.md) (triage — post verdict UX, recovery, metrics, T&S, wellbeing). **Launch readiness:** [`docs/LAUNCH_READINESS.md`](docs/LAUNCH_READINESS.md). **Scholarly reviews:** [`docs/EVOLUTIONARY_BIOLOGY_REVIEW.md`](docs/EVOLUTIONARY_BIOLOGY_REVIEW.md), [`docs/POLITICAL_SCIENCE_REVIEW.md`](docs/POLITICAL_SCIENCE_REVIEW.md), [`docs/SOCIAL_PSYCHOLOGY_REVIEW.md`](docs/SOCIAL_PSYCHOLOGY_REVIEW.md), [`docs/CLINICAL_PSYCHOLOGY_REVIEW.md`](docs/CLINICAL_PSYCHOLOGY_REVIEW.md). **Safety roadmap:** [`docs/TRUST_AND_SAFETY.md`](docs/TRUST_AND_SAFETY.md).

**Communities**

- New communities get **`social_entity.rating = 2.50`** at create (`src/social/entity.rs`). Migration backfills communities at `0.00` → `2.50`.
- New **profit**, **non-profit**, and **gathering** entities get **`social_entity.rating = 2.50`** at create (same neutral floor). Migration `20260527190000` backfills alive brand entities still at `0.00`.
- **Failing** (`rating < 0.66`, not dead): red `@handle`, profile notice; blocks amendment/reparent filing. First crossing below 0.66 emails all verified **owners** (`rating_failing_notified_at` on `social_entity`).
- **Post inactivity**: **removed** — communities no longer lose rating from post idle (`inactivity_batch.rs`; `community_graph_lifecycle` runs human login-idle only). `last_post_activity_at` still updated on posts via `touch_post_activity_for_entities_tx`.
- **Human login inactivity** (phased; **`jury_civic_rating_delta()` = 0.035**): **21** days without sign-in → warning email (no rating change); **28** days → daily global decrements — **21** days at **−1×δ**, then **21** days at **−3×δ**, then **−6×δ**/day (`users.inactivity_penalties_applied`, `inactivity_rating_debited`). **`record_user_login`** refunds debited standing and resets counters. First decrement phase emails owner with **`estimated_deletion_at_from_last_login`** (phased math, UTC date). Job payload: `human_inactivity_batch`.
- **Death** (`rating == 0.00`): `community_graph::community_death_tx` (memberships ended, constitution retired, children re-parented, `social_entity_status.dead`).
- **Death immune**: root (`parent_community_id IS NULL`), `256app`, `operating_charter_256`, `panoply` (operator clearance community), council domain handles (`community_graph::COUNCIL_COMMUNITY_HANDLES`; immune list in `community_graph::immune` — `IMMUNE_PRODUCT_HANDLES`).
- **Judicial re-parent**: `community_reparent` trigger, **64 Noms**, two-phase jury (`src/social/judicial/community_reparent.rs`).

**Humans**

- Default human rating **2.50** at create (existing).
- **Community edge → global spillover** (symmetric): community-edge rating deltas apply **`delta / 6`** to global `social_entity.rating` in either direction (`judicial::community_member_delta_to_global_spillover` via `bump_community_member_rating_tx`).
- **Failing** (`rating < 0.66`, not dead): **Humans** — arterial-red `@handle` and profile warning (`ui.entity_private.human_failing_notice`) **for the owner only** (feed, private profile to others, and invitations show a healthy handle — same privacy model as rating stars). First crossing below 0.66 emails the owner (global) or the member (`member_of` edge + `social_entity_relationship.rating_failing_notified_at`). **Communities** — red `@handle` and profile notice remain public; blocks amendment/reparent filing; emails verified owners once (`rating_failing_notified_at` on `social_entity`).
- **Away-user engagement digest**: job `user_engagement_digest` emails verified humans every **3 idle days** (days 3–18) with activity since `last_login_at` (`src/social/engagement_digest.rs`); **stops at 21 days** when inactivity warning mail begins. Dedupe via `users.engagement_digest_sent_at`.
- **Login inactivity**: phased decrements as above; sign-in **refunds** `inactivity_rating_debited` and resets penalty cycle (`src/auth/session.rs::record_user_login`). Separate from the **&lt; 0.66** failing email (`rating_failing_notified_at`).
- **Death** (`rating == 0.00`): `community_graph::human_death_tx` — all relationships ended, open judicial cases voided, linked `users` → `to_be_deleted`, sessions cleared, `social_entity_status.dead`, strike-through red handle (non-clickable) on historical posts. **`require_verified_session`** rejects any surviving session for `to_be_deleted` / `blocked` users and deletes their session rows again.
- **Death immune**: platform handle `256`, `users.user_type = ai`, and **active verified `member_of @panoply`** operators (fitness-death only — eject still applies). See `community_graph::immune::is_human_death_immune`.
- **Public discovery**: dead entities and their posts are hidden from `/public/*`, `/api/public/*`, and `/social-entities/search` (`public_profile::sql_alive_entity_predicate`). Failing humans are **not** blocked from posting or jury participation beyond existing gates.

**Conversation settlement** (post feel when thread closes — 3d idle, ≥3 non-author votes): **Loved** +3×δ community edge / +1×δ global direct; **Loathed** −3×δ edge / −1×δ global (symmetric to Loved); **Controversial** +1×δ edge only. Global spillover ±δ/6 on edge deltas. Uncontested emergency standing uses the same Loathed-equivalent multipliers (`conversation_rating.rs`, `emergency.rs`; constants `RATING_MULT_CONVERSATION_*` in `judicial/types.rs`).

`@panoply` is the **invite-only** operator clearance community (`src/social/panoply/`, migrations `20260528150100_panoply_ops.sql`, `20260529130000_panoply_constitution.sql`, `…140000/150000/160000`). Active verified `member_of @panoply` grants:

- **Ops dashboard** `GET|POST /home/ops` (`src/routes/panoply.rs`) — operator-only (404 otherwise). Actions, all writing a `safety_operator_action` audit row with a **mandatory note**: emergency filing (`emergency_safety`, post `suppressed`), civic filing (`civic_dispute`), vacate uncontested `contained` emergency (restore thread), eject (`platform_safety_ban_human_tx`), session kill, domain block (`content_domain` → `blocked`/`unsafe`). Operator filings **waive Noms and the 3/day emergency cap**.
- **Fitness-death immunity** (rating-zero + login-idle only) via `community_graph::immune::is_human_death_immune` → `panoply_member_immunity_conn`. **Not** immune to eject or legal process.
- **Concealment**: `@panoply` + roster hidden from non-operators in search/public/private profiles (`panoply::conceal`); `@panoply` is an invalid community parent.

Operator clearance check: `community_graph::is_panoply_operator_for_user`. Seed operator: `operator@example.com` / `@sphinx`. Operator charter (10 points) is **descriptive** (`judicial_trigger_chk = 0`) — not a filing trigger in v1. Product/safety docs: [`docs/TRUST_AND_SAFETY.md`](docs/TRUST_AND_SAFETY.md) § Panoply operator console, [`docs/PRODUCT_GUIDE.md`](docs/PRODUCT_GUIDE.md) § Panoply operator console.

## Social graph — connection limits (humans only)

**Human** `social_entity` profiles are capped at **256** distinct active connections (any relationship type, pending/verified/flagged, `ended_at IS NULL`). Enforced in `src/social/connections.rs` (`MAX_CONNECTION_SLOTS`) on insert via `relationship::create` and `entity::ensure_creator_member_connection_tx`. **Digital** actors, communities, orgs, and gatherings are **not** capped—only humans with meat brains need the Dunbar-scale limit.

## Image processing (avatar pipeline only)

The product does **not** store user post media. The only image path is **face capture / avatar redraw** → transient JPEGs (async job staging) → vision validate → **canonical appearance** (transient prose) → optional **512×512** generated PNG stored on `user_faces`. **Vertex / Imagen credentials live on `ai_gateway` only** (web app uses `AI_GRPC_ENDPOINT`). Default onboarding artist: `@epic` with Imagen subject refs + style plate (2026-07-24).

Standardize that pipeline on **512×512** square pixels (client crop/resize, server normalization). Do not add post attachments, profile galleries, or other upload surfaces without updating **Content model** above, [`docs/PRODUCT_GUIDE.md`](docs/PRODUCT_GUIDE.md), and [`docs/TRUST_AND_SAFETY.md`](docs/TRUST_AND_SAFETY.md). Do not introduce ad hoc sizes (e.g. 768) without updating this note and code together.

Mirrored in `.cursor/rules/image-processing-standard.mdc`.

## UI palette usage

Use the site-wide accent palette:

- `--c-red`: arterial red
- `--c-green`: forest green
- `--c-blue`: royal blue
- `--c-cyan`: sky blue
- `--c-magenta`: burgundy
- `--c-yellow`: sun-burst yellow

Semantic guidance:

- Use **blues** for posts/content surfaces.
- Use **yellows and reds** for call-to-actions.
- Use **green** for action-oriented elements where the user is being prompted to do something.
- Use **burgundy** for self-serving/promotional surfaces (e.g. marketing cards, promo modules).

Card top-border guidance:

- Keep card top borders consolidated through shared card accent tokens/variables.
- Match top-border styling to card purpose:
  - action cards: green
  - primary acquisition card (`Join In`): arterial red (highest visual urgency)
  - promotional/marketing cards: burgundy
  - content/profile cards: blues

Mirrored in `.cursor/rules/ui-color-palette-usage.mdc`.

## Noms (monetization anchor)

Treat public-facing **Noms** (stored in `social_human.ai_tokens`) as the **primary monetization mechanism** for the product.

- AI services (for example council-assisted image/art generation) should consume these tokens.
- The primary accrual paths are the **signup grant** (`SIGNUP_NOM_GRANT`, 32 Noms), **daily login** (+1 Nom on first sign-in each UTC day), and **jury duty** (+3 Noms when a juror votes and the case resolves), plus optional **fiat top-up** via Stripe (three packs in `src/nom_packs.rs`: Civic 32/A$3, Standard 64/A$5, Steward 128/A$8) when `STRIPE_PUBLISHABLE_KEY` + `STRIPE_SECRET_KEY` are set — topbar wallet button, webhook fulfillment (`credit_purchase` ledger); per-pack balance gates in `NomPack::can_purchase_at_balance` (Civic always available subject to wallet cap 256; Standard/Steward tier gates). Entity founding debits via `spend_for_service_tx` + `BillableService::EntityCreate*` in `src/pricing.rs`.
- The home feed is **ad-free by design** — no third-party ad units in scroll.
- All debit prices live in `src/pricing.rs` — see the "Pricing schedule" section below.
- Keep internal implementation explicit (credits/debits, accounting, limits), while user-facing copy can remain simple.

Mirrored in `.cursor/rules/ai-token-monetization.mdc`.

## Judiciary — jury sizing and verdict threshold

The judiciary subsystem has fixed rules that should not be changed without an explicit product decision. Mirror this in any worker, validator, or UI copy.

**Jury sizing** uses the square-root rule, rounded up (`ceil(sqrt(pool_size))`), against the **eligible** population for the case. The jury size is frozen on `judicial_case.jury_size` at filing time: post-dispute filing calls `judicial::compute_jury_size_to_freeze_at_filing_tx` on the **same DB transaction** as the insert so the pool snapshot matches uncommitted filing state. Seating uses `min(frozen, current_pool.len())` so a shrinking pool cannot deadlock jury fill.

- **Emergency safety** (Root §4.1–§4.7, trigger `emergency_safety`) → post **`suppressed`** on file; **no jury** until author **contests** (1 Nom, `POST /home/judicial/post/{id}/contest`); case status **`contained`** until contest. Uncontested: T+3 Loathed-equivalent standing via job `emergency_standing_settlement`; post stays hidden. Contest loss → platform **`blocked`** ban (`human_safety_ban_tx`). **3 filings/filer/UTC day**; **one emergency/post**. Author emails (SMTP live on beta): filed, contest opened, standing applied (`src/email/notify/moderation.rs`).
- **Civic dispute** (other `judicial_trigger_chk` clauses, trigger `civic_dispute`) → post **`under_judicial_review`**; jury from filing. Legacy `post_moderation` rows remain readable.
- Civic / contested emergency → pool is the post's audience (direct entities, audience-targeted community members, or friends/family relations of the author). No minimum rating.
- Membership review → pool is verified community members with **community relationship rating > 3.00**, backfilled from members rated **≥ 2.50** when the primary pool is too small. Jury size follows post-moderation rules (sqrt, cap 12). No AI appeal; applicant statement is the case corpus. Rejected applicants cannot reapply for one year.
- Constitutional amendment → pool is verified **`member_of` the amending community only** (not the descendant subtree); edge rating tiers **> 2.50** primary / **≥ 2.50** fallback; jury size uncapped √n.
- Community re-parent → phase 1: moving community members only (√n, cap 12); phase 2: **distinct humans** with `member_of` to any community in **M ∪ descendant subtree** (uncapped √N).

Cap is **12** for post disputes and membership reviews. **Uncapped** for constitutional amendments.

**Exclusions** from juror selection: case originator, case target, and any human entity owned/controlled by the originator (covers sockpuppets via `social_entity.created_by` and `user_social_entity_role`). Jurors who have already **abstained** or **declined** from a case are permanently excluded from re-nomination for that same case.

**Vote weight normalization**: juror ratings (0.00–5.00) are normalized to a **0–2 vote weight** scale using `weight = rating / 2.5` (constant `judicial::RATING_NORMALIZATION_DIVISOR`). A rating of 2.50 yields weight 1.0 (one full vote); 5.00 yields 2.0 (double vote); 0.00 yields 0.0 (no weight).

**Verdict threshold** is two-thirds of total normalized voting weight: `normalized_for >= normalized_total * 2.0 / 3.0`, where `normalized_for = SUM(rating / 2.5)` for jurors who voted "for" and `normalized_total = SUM(rating / 2.5)` for all jurors who voted (for + against). Do **not** pre-round the threshold to an integer in either the worker or the UI; round only when presenting a human-readable target (and label it clearly as a rounded display). Use `judicial::check_and_resolve_verdict` to evaluate and resolve.

**Feed-scoped community rating (post moderation)** — when `judicial::community_context_for_case` resolves a community for the subject post, `social_entity.rating` on that community is adjusted on verdict close from wall-clock `closed_at - opened_at`: **≤3 days → +3×** `judicial::jury_civic_rating_delta()`, **≤7 days → +1×**, **≤14 days → no change**, **\>14 days with a jury verdict → no timing bonus** (jury was slow but still decided). If the case is still `open` or `jury_active` past the post-dispute `no_jury` window, `judicial_jury_nomination` closes it as `judicial_case_status.no_jury`, voids pending invites, and applies **−6×** the same delta to that community (floored at 0). **Post-dispute `no_jury` deadline** (`src/social/judicial/moderation_tier.rs`, `deadlines.rs`): **7 days** when any cited clause has `moderation_tier_id` → `judicial_moderation_tier.serious` (includes §4.1–§4.7 emergency sub-clauses); **21 days** otherwise for **civic dispute** only. On civic `no_jury`, the subject thread stays **`flagged`** (hidden from feed) via `settle_post_moderation_subject_after_no_jury_tx` — not restored to `verified`. Emergency uncontested cases do not use the jury clock; they use `contained` + standing settlement.

**Root Operating Charter v2** (`operating_charter_256`, **16 points**, shipped 2026-05-29) — point **4** is a **preamble** (not selectable); selectable safety sub-clauses are **401–407** (§4.1–§4.7). Points **15** (Acute Harm Sanitation) and **16** (Mandatory Reporting) are in DB/UI. **`safety_sanitation_event`** logs domain denies and hash-gate blocks; face capture runs pre-persist SHA-256 gate (`MEDIA_HASH_BLOCKLIST`). Migrations `20260528140000_emergency_civic_moderation.sql`, `20260529120000_root_operating_charter_v2.sql`, `20260529121000_safety_sanitation_event.sql`. Item **11** (legal process) is not a dispute trigger; item **14** (dignity) is not selectable — bad-faith civic/emergency filing is penalized when the jury clears the post / author wins contest (originator **−6×δ**). Spec: [`docs/ROOT_CONSTITUTION_PROPOSAL.md`](docs/ROOT_CONSTITUTION_PROPOSAL.md). Consent: **`v2-2026-05-29`**.

**Rating snapshot**: for community-scoped cases the juror's **community relationship rating** (`social_entity_relationship.rating`) is frozen onto `judicial_case_juror_invite.juror_rating_at_invite` at invite time. For post-audience jurors not connected through a community, the global `social_entity.rating` is used. That snapshot is the weight used in any subsequent vote tally for that case.

**Voting**: jurors cast votes via `judicial::cast_vote(pool, user_id, invite_id, "for"|"against")` which transitions the invite from `pending` to `accepted` with `vote_direction` set, and increments the parent case's `jury_votes_for` or `jury_votes_against` tally. Returns `CastVoteOutcome::{Recorded, NotInvited, AlreadyResponded}`.

**Abstention and refill**: jurors may abstain on up to `judicial::MAX_FREE_ABSTENTIONS_PER_YEAR` (3) invitations in a rolling 365-day window without penalty on **regular** cases. **Emergency safety** and **serious moderation tier** cases are **quota-free**: abstention does not consume the annual allowance and does not incur the −3×δ standing penalty at settlement (`case_is_severe_for_juror_abstention`). The summons shows a severity notice before the case workspace on those paths. Use `judicial::abstain_from_invite` to record an abstention — it returns `AbstainOutcome::{Free, OverFreeQuota, NotInvited, AlreadyResponded}`. The seat is permanently vacated for that case (the juror will never be re-invited to the same case); the recurring `judicial_jury_nomination` job fills it from the remaining pool on the next tick. `judicial_jury_nomination` examines both `open` and `jury_active` cases so refill happens automatically for already-seated juries that lose members.

**Filing cost** is **trigger-dependent** and lives in the product schedule at `src/pricing.rs` (`BillableService::JudicialFiling(JudicialTrigger::*)`). All Nom-debiting flows must read from that schedule — never inline a `SET ai_tokens = ai_tokens - N` query in a handler. Go through `social::noms::spend_for_service_tx` (or `spend_for_service` when no wider transaction is involved). Treat all of these as acts-of-conscience costs, not revenue levers; keep them visible in UX.

Current judicial schedule (mirrored in `docs/PRODUCT_GUIDE.md`):

| Trigger | Cost |
| --- | --- |
| Emergency safety filing | 1 Nom |
| Civic dispute filing | 1 Nom |
| Emergency contest (author) | 1 Nom |
| Membership review | free |
| Constitutional amendment | 32 Noms |
| Community profile amendment | 32 Noms |
| Community re-parent | 64 Noms |
| Civic dispute appeal | 32 Noms (AI council escalation) |

Membership review must remain free under all circumstances — joining a community must never be gated by token balance. **Constitutional amendments have no council appeal** — only a human community jury (`constitutional_amendment` trigger). The AI council arbitrates **civic disputes** against the constitution (including after a resolved civic dispute is appealed via `case_appeal` → `escalated_to_ai`). **Emergency safety cases have no council appeal.** Council members receive seed constitutional clauses for their communities; changing those clauses is a human-only process. Immutable extrinsic evidence for cases with URLs is stored in `judicial_case_evidence` (HTML/text/screenshot metadata); scrape-at-filing and council prompts should use that snapshot, not live links.

## Pricing schedule (code-only)

All public Nom prices live in `src/pricing.rs` (`BillableService` + `nom_cost`). This includes:

- Judicial filings and appeals (per-trigger pricing above).
- Council-assisted AI services (portraiture, avatar refresh, artwork brief), with starting prices pending product calibration.

Rules:

- The schedule is **not** mirrored in the database. Do not introduce a `service_price` table or any DB-driven pricing without an explicit product decision and a corresponding update to `src/pricing.rs`.
- New Nom-debiting flows MUST: (1) add a variant to `BillableService`, (2) set its cost, (3) spend through `noms::spend_for_service[_tx]`, and (4) mirror the price in `docs/PRODUCT_GUIDE.md` for product transparency.

## In-process job scheduler

**SMTP is live** on `beta.twofivesix.app` and in local dev when `SMTP_HOST` is set in `.env` (verification, password reset, recruitment review, `user_engagement_digest`, `panoply_daily_status`, council invite review). **`/health/status`** exposes `smtp_configured` and `scheduler_tick_secs` for ops checks.

`src/jobs/scheduler.rs` spawns a single background tokio task at startup (`jobs::spawn_scheduler` from `main.rs`) — **not** triggered by HTTP traffic — every `SCHEDULER_TICK_SECS` (default 1s):

1. Enqueues recurring system jobs — `judicial_jury_nomination`, `judicial_council_appeal`, `scope_recalc_drain`, `community_graph_lifecycle`, `conversation_rating_settlement`, `emergency_standing_settlement`, `council_reply_tick` (15‑min `period_key`), `council_post_tick` / `council_invite_tick` / `council_portfolio_feed_poll` / `user_engagement_digest` / `panoply_daily_status` (daily or 6h `period_key`), and `content_domain_lifecycle` — with a fixed correlation id so the conductor's `(job_type_id, payload_hash)` dedup collapses concurrent enqueues to a single pending row.
2. Calls `jobs::run_conductor_tick` with `SCHEDULER_BATCH_SIZE` (default 2) to drain whatever is due.

**Vertex smoothing:** `ai_gateway` caps outbound `generateContent` calls via `VERTEX_GATEWAY_MAX_RPM` (default 20, burst 1) — council jobs and face vision share one wait-for-slot budget.

Rules:

- Handlers that need a job triggered immediately (e.g. `home_judicial_post_case_submit` enqueuing `judicial_jury_nomination` on case filing) should call `jobs::enqueue_job` directly. The scheduler is a safety net, not the primary trigger path.
- Set `SCHEDULER_TICK_SECS=0` to disable the in-process scheduler entirely. In that case the queue is only advanced via `POST /ops/jobs/run` or the dev "Run Jobs" button.
- Any new recurring system job (e.g. abstention-window summarizer, jury vote tally, escalation timeout) should be added to `scheduler::enqueue_recurring` so it participates in the same dedup pattern.
- **Council AI activity** — `godmother_outreach` (welcome on `select_godmother`; global failing/recovery from graph lifecycle — not the recurring scheduler; requires active `godmother` edge). Reply/post/invite ticks use live population `N` and daily budget `B = max(1, ceil(√(√N)))` per councillor (`council_ai_daily_budget`); **replies floor at `max(9, B)`**; shares/invites use `B`. Caps debit on successful publish/invite only. Each councillor **stewards one community** (`moderator` edge); autonomous share/reply prompts inject `@{steward_handle}`, label, and intention — not platform-wide voice (`agents/council/shared/default_council_*_prompt_template.txt`, `format_avc_block` in `src/agent_specs.rs`). **Content shares** use per-councillor outbound-host portfolios (`ai_actor_content_domain`, 32 hosts; seed `20260611140000_ai_portfolio_and_entity_profile_link.sql`). Shares publish to **own** steward `@public` only. **Council cross-talk:** each councillor is `member_of` every peer steward community (`20260611160000_council_cross_community_membership.sql`); reply tick scans posts in all `member_of` + `moderator` audience communities (voice stays own-steward). **Link shares:** catalogue links may lack OG crawl — UI falls back to hostname preview; council ingest sets `og_title`/`og_site_name` from domain.

**`council_post_tick` / share funnel (PRODUCT_GUIDE):** daily coordinator (`council_post_tick`) enqueues 12 `council_share_shortlist` jobs (5‑min stagger). Each chain runs portfolio hygiene then **32→8→read→publish** (`social::council_share_pipeline`): shortlist LLM on blurbs only, transient article read with paywall filter, share LLM on open candidates only (no republished body). Pool maintenance: `council_portfolio_feed_poll` (RSS + bounded crawl). Legacy `council_post_member` wraps shortlist. Non-AI `profile_content_link_id` shipped; UI deferred. Councillor `social_entity.rating` unchanged by autonomous replies. Handlers: `src/social/council_jobs.rs`, `src/social/council_share_pipeline.rs`, `src/content/feed_poll.rs`.

**`council_invite_tick` / invite funnel (PRODUCT_GUIDE):** daily coordinator enqueues 12 staggered `council_invite_article_pick` jobs. Four-stage chain (`social::council_invite_pipeline`): own-steward link shares **32→8** → optional web search + person discover → contact resolve (snippet/profile/OG email verify; profile URL grounded in search hits) → compose + **`deliver_platform_invite_for_review` only** (beta; `COUNCIL_INVITE_REVIEW_EMAIL`); **re-chains** while `invites_spent < invite_budget`. Migration `20260814140000_council_invite_pipeline.sql`.

## Scoped read security

Non-human / graph-driven visibility is enforced with **`vw_entity_read_scope_live`** (relationship-derived source of truth) plus a **materialized fast path** **`entity_read_scope`**, kept in sync by the job queue.

**Materialization worker** — `scope_recalc_drain` (`src/jobs/queue.rs`, handler `run_scope_recalc_drain_job`): batches subjects from **`entity_scope_recalc_queue`** (ordered by `requested_at`), replaces that subject’s rows in **`entity_read_scope`** with a snapshot from **`vw_entity_read_scope_live`**, then removes the queue row. The in-process scheduler enqueues this job every tick alongside `judicial_jury_nomination` (`src/jobs/scheduler.rs`).

**Invalidation** — `src/social/scope.rs` (`mark_entity_dirty`, `mark_relationship_endpoints_dirty`, and `_in_tx` variants) upserts the queue and increments **`social_entity.scope_version`** for affected entities. Relationship create / status transition / end paths call into this (e.g. `social::relationship::create`, `status::end_relationship` / `transition_relationship_status`, `entity::ensure_creator_member_connection_tx`, selected routes in `routes/social.rs`).

**Read paths** — Feed and audience helpers use **`entity_read_scope` first** and fall back to **`vw_entity_read_scope_live`** where a materialized row may lag (`src/social/feed.rs`, `src/routes/social.rs`), so correctness does not depend on the worker having run.

**Full rebuild (ops)** — `scope_rematerialize_all` truncates **`entity_read_scope`**, repopulates from the live view for all subjects, and clears the queue. It is **not** on the recurring scheduler; trigger via ops job controls / `POST /ops/jobs/run` when reconciling drift or after bulk imports.

## Testing

Two layers — see [`docs/README.md`](docs/README.md) § Tier 2b and [`e2e/README.md`](e2e/README.md):

1. **Rust integration** — `cargo test`; in-process `TestApp` (`tests/common/api.rs`) against disposable MySQL; ~26 module binaries (`m1_public_api`, judicial, graph death, panoply, etc.).
2. **Playwright browser smoke** — `e2e/`; seed personas via `cargo run --bin e2e_seed`; **`npm run test:public`** for the guest/login sweep (14 scenarios × desktop + mobile).

**Pre-deploy (automatic on push to `main`):** [`deploy.yml`](.github/workflows/deploy.yml) runs **`cargo test`** + Playwright **`test:public`**, then build + SCP to beta — confirmed 2026-08-12 ([`docs/DEPLOY_BETA.md`](docs/DEPLOY_BETA.md)). PRs run the same tests via [`ci.yml`](.github/workflows/ci.yml) (no deploy). **Local optional:** same commands — [`e2e/README.md`](e2e/README.md).

**HTML auth gate:** browser GETs to protected routes without a valid session redirect to `/login?login=1`; unverified email → `/login?resend=1` (`src/middleware/auth_gate.rs`). JSON/API clients unchanged.
