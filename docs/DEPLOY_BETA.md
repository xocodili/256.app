# Beta deploy pipeline (`beta.twofivesix.app`)

Operational runbook for the **confirmed** GitHub Actions → GCP VM deploy path. Product context: [`LAUNCH_READINESS.md`](LAUNCH_READINESS.md) · AI credentials: [`BETA_AI_SETUP.md`](BETA_AI_SETUP.md).

**Milestone:** **2026-08-12** — **Epic portrait consolidation + avatar redraw** deployed to beta after CI runner disk hardening and VM staging-order fix. **2026-06-05** — **confirmed** end-to-end pipeline on push to `main`: **`test`** (`cargo test` + Playwright `test:public`) → **`build`** → **`deploy`** (SCP + systemd restart), all green. Prior: **2026-06-02** — deploy path verified without automated test gate.

---

## Pipeline overview

| Job | Runner | Output |
|-----|--------|--------|
| `test` | `ubuntu-22.04` | Reusable [`.github/workflows/test.yml`](../.github/workflows/test.yml): `rust-test` + `playwright-public` (parallel) |
| `build` | `ubuntu-22.04` | Release binaries `app`, `ai_gateway`, `db_reset` (runs only after `test` passes) |
| `deploy` | `ubuntu-22.04` | Reclaim VM disk → SCP → git pull → `install` → restart |

Workflows:

| File | Trigger | Role |
|------|---------|------|
| [`.github/workflows/ci.yml`](../.github/workflows/ci.yml) | PR to `main`, `workflow_dispatch` | Quality gate only (tests — **not** on push to `main`) |
| [`.github/workflows/deploy.yml`](../.github/workflows/deploy.yml) | Push to `main`, `workflow_dispatch` | Tests → build → beta deploy |

**Trigger (deploy):** push to `main`, or **`workflow_dispatch`** (optional **Reset database** checkbox — see [Database reset](#database-reset-beta-fresh-seed)).

**GitHub retention:** repo **Actions → General → Artifact and log retention** — **28 days** (2026-08-12).

**On the VM after deploy:**

1. **Reclaim disk** (debug `target/`, old journals) — **before** SCP.
2. Binaries land in `~/deploy-staging/`, then `install` to `~/256.app/target/release/{app,ai_gateway,db_reset}`.
3. `git fetch` + `git reset --hard origin/main` refreshes templates, `locales/`, `static/`, `agents/`, migrations (not embedded in the binary).
4. Staging copies removed; `systemctl restart ai-gateway` then `twofivesix`.
5. Deploy waits up to ~90s per unit; on failure prints `journalctl -n 80`.

---

## Database reset (beta fresh seed)

Use when you want **migration seed state** again — all human accounts, posts, sessions, Nom purchases, and council activity wiped; council / operator / domain seeds recreated on next app boot.

| What survives | What is destroyed |
|---------------|-------------------|
| `~/256.app/.env`, `.secrets/`, binaries | Every row in the app MySQL database |
| Stripe keys (Dashboard) | All beta user accounts (register again) |
| GCP SSH metadata | Posts, threads, judicial cases, sessions |

**After reset:** migrations re-run on `twofivesix` start (same as first deploy). Operator seed `operator@example.com` / `@sphinx` / `@panoply` returns from migrations; **your personal beta account does not** — sign up fresh for onboarding retest.

### Option A — deploy with reset (recommended)

1. Merge/push today's code to `main` (includes `db_reset` binary + `scripts/ops/reset_beta_database.sh`).
2. GitHub → **Actions** → **Deploy beta** → **Run workflow**.
3. Check **Reset database** → Run.
4. Pipeline runs tests → build → deploy → drop/create DB → restart → migrations apply.

Plain push to `main` **never** resets the database (only `workflow_dispatch` with the checkbox).

### Option B — manual reset on the VM (SSH)

After at least one deploy that shipped `target/release/db_reset`:

```bash
cd ~/256.app
git pull   # or wait for deploy git reset
DB_RESET_ALLOW=1 bash scripts/ops/reset_beta_database.sh --restart
```

**Binary:** `cargo build --release --bin db_reset` (or use CI artifact). **Safety:** non-local `DATABASE_URL` requires `DB_RESET_ALLOW=1` ([`src/bin/db_reset.rs`](../src/bin/db_reset.rs)).

**Verify:** `curl -s http://127.0.0.1:8080/health/status` → register a new account on beta; confirm verification email (SMTP live).

**Not the same as:** `e2e_seed --reset` (Playwright personas only — does not drop the database).

---

## GitHub Actions secrets

| Secret | Value |
|--------|--------|
| `VM_HOST` | GCP VM **external IP** (e.g. `203.0.113.10`) — **not** a Cloudflare-proxied hostname (`172.67.x.x` will timeout on port 22) |
| `VM_USER` | Linux account (e.g. `deploy`) |
| `VM_SSH_KEY` | Full **private** deploy key (the whole OpenSSH key file, including its BEGIN/END lines) |
| `VM_SSH_KEY_B64` | *(Optional)* Base64 of the same private key — use if pasting PEM into `VM_SSH_KEY` breaks newlines in GitHub UI |

**Helper scripts (run on your laptop):**

| Script | Purpose |
|--------|---------|
| [`scripts/ops/generate_gha_deploy_key.ps1`](../scripts/ops/generate_gha_deploy_key.ps1) | Windows: create `~/.ssh/gha_deploy` + print GCP/GitHub steps |
| [`scripts/ops/register_gha_deploy_key_gcp.sh`](../scripts/ops/register_gha_deploy_key_gcp.sh) | Append `gha_deploy.pub` to **instance SSH metadata** (safe; keeps existing keys) |
| [`.github/workflows/deploy-ssh-check.yml`](../.github/workflows/deploy-ssh-check.yml) | Actions → **Deploy SSH check** — test secrets without a full build |

The deploy workflow ([`deploy.yml`](../.github/workflows/deploy.yml)) validates secrets, writes the key to a temp file (`ssh-keygen -y` check), runs an SSH **preflight**, then SCP + restart.

**Optional (not in workflow today):** `GCP_SERVICE_ACCOUNT_JSON` — keep on VM in `~/256.app/.secrets/` per [`BETA_AI_SETUP.md`](BETA_AI_SETUP.md); do not commit to git.

### SSH key setup (GCP — do not edit `authorized_keys` by hand)

Deploy uses **public-key auth** with a dedicated **`gha_deploy`** key pair. The private half lives in GitHub (`VM_SSH_KEY`); the public half must be registered in a place **GCP will not overwrite**.

**Why keys “disappear” after every deploy**

The beta deploy workflow **does not** touch `~/.ssh` — SCP fails *before* any `git reset` or `systemctl restart` runs. What actually happens on most GCP VMs:

1. **`google-guest-agent`** syncs `~/.ssh/authorized_keys` for metadata-managed users from **instance/project SSH metadata**.
2. Manually `cat … >> authorized_keys` works briefly, then the agent **replaces** the file to match metadata (often within minutes or on the next agent tick).
3. GitHub Actions then fails with `unable to authenticate, attempted methods [none publickey]`.

This looks like “deploy broke SSH,” but it is **metadata sync**, not the pipeline.

**What deploy does *not* wipe**

| Path | Survives `git reset --hard`? |
|------|------------------------------|
| `~/256.app/.env` | Yes (gitignored) |
| `~/256.app/.secrets/` | Yes (gitignored) |
| `~/.ssh/authorized_keys` | **No** — managed by GCP guest agent for metadata users |

#### One-time setup (recommended)

**1. Generate a deploy key** (on your laptop, not on the VM):

```bash
ssh-keygen -t ed25519 -f ~/.ssh/gha_deploy -C "gha-deploy-beta" -N ""
```

**2. Register the *public* key in GCP instance metadata** (Console → Compute Engine → VM → **Edit** → **SSH Keys** → **Add item**, or `gcloud` below). Use the same Linux username as `VM_USER` (e.g. `deploy`).

```bash
# Download existing keys first — blind overwrite erases other keys.
gcloud compute instances describe INSTANCE_NAME --zone=ZONE \
  --format='value(metadata.items.ssh-keys)' > /tmp/ssh-keys.txt
echo "deploy:$(cat ~/.ssh/gha_deploy.pub)" >> /tmp/ssh-keys.txt
gcloud compute instances add-metadata INSTANCE_NAME --zone=ZONE \
  --metadata-from-file ssh-keys=/tmp/ssh-keys.txt
```

In the Console UI (**Edit** → **SSH Keys** → **Add item**), paste the **public** key only; GCP prefixes your username.

**3. Store the *private* key in GitHub** → Settings → Secrets → `VM_SSH_KEY` (the whole key file, including its BEGIN/END lines, with real newlines).

**4. Reserve a static external IP** for the VM and set `VM_HOST` to that IP (not a Cloudflare hostname). Update the secret if the IP ever changes.

**5. Smoke test from your laptop:**

```bash
ssh -i ~/.ssh/gha_deploy -o IdentitiesOnly=yes deploy@<VM_HOST> "echo ok"
```

**6. GitHub Actions → Deploy SSH check → Run workflow** (validates secrets + SSH without building Rust). Green = deploy auth is fixed.

**Windows keygen shortcut:**

```powershell
.\scripts\ops\generate_gha_deploy_key.ps1
```

#### Alternative: dedicated deploy user (not metadata-managed)

If you prefer not to use metadata SSH keys, create a **separate** Linux user that guest-agent does not manage (e.g. `gha`), and authorize via `/etc/ssh/authorized_keys.d/` once as root. Set `VM_USER=gha` in GitHub secrets. Only pursue this if metadata keys are unsuitable.

#### OS Login

If `enable-oslogin=TRUE` on the VM or project, GCP may **delete** traditional `authorized_keys` files. Either disable OS Login for this VM or follow [GCP OS Login docs](https://cloud.google.com/compute/docs/oslogin) and use a local user with a distinct username/UID.

---

## Stripe Nom purchases (beta VM)

**Status (2026-08-14):** End-to-end **test-card checkout on `beta.twofivesix.app` confirmed** — Payment Element → webhook (or sync fallback) → Noms credited to wallet. Use test mode keys until production launch.

**Production release:** Stripe must be **explicitly activated** for public launch — live `pk_live_` / `sk_live_` keys, production webhook endpoint on the production domain, billing/compliance review, and ops runbook sign-off. Do not flip to live keys on beta without a deliberate go-live decision.

Fiat checkout is **optional** — enabled only when both publishable and secret keys are set in `~/256.app/.env`. Webhook secret is required for server-side fulfillment without relying on the browser `/sync` fallback.

| Variable | Where | Notes |
|----------|--------|--------|
| `STRIPE_PUBLISHABLE_KEY` | `~/256.app/.env` | `pk_test_…` or `pk_live_…` |
| `STRIPE_SECRET_KEY` | `~/256.app/.env` | `sk_test_…` or `sk_live_…` |
| `STRIPE_WEBHOOK_SECRET` | `~/256.app/.env` | `whsec_…` from Stripe Dashboard → Webhooks |

**After deploy** (migration `20260610120000_nom_purchase.sql` must be applied — runs on app boot if your migrate path is active):

1. [Stripe Dashboard](https://dashboard.stripe.com/) → **Developers → Webhooks** → **Add endpoint**
2. URL: `https://beta.twofivesix.app/webhooks/stripe`
3. Events: `payment_intent.succeeded`, `payment_intent.payment_failed`
4. Copy signing secret → `STRIPE_WEBHOOK_SECRET` on VM → `sudo systemctl restart twofivesix`
5. **Payment methods:** Stripe → Settings → Payment methods — enable card (and Link if desired). No separate Google Pay domain setup required for v1.

**Smoke test (test mode):**

```bash
curl -s http://127.0.0.1:8080/health/status
# Log in on beta → tap topbar Noms → modal shows three packs (Founder / Standard / Steward)
# Civic: 32 Noms @ A$3.00 (always available; wallet cap 256)
# Standard: 64 Noms @ A$5.00 (balance must be < 64)
# Steward: 128 Noms @ A$8.00 (balance must be < 128)
# Close modal without paying → reopen → checkout must still work (pending row cancelled)
```

**Refund runbook:** If payment succeeds but wallet rules block credit (per-pack balance gate or 256 cap), the app marks `nom_purchase.status = failed` and logs `manual Stripe refund required`. Refund in Stripe Dashboard against the PaymentIntent; do not re-run fulfill.

Catalog and rules: [`src/nom_packs.rs`](../src/nom_packs.rs) · Product copy: [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) § Noms.

---

## SMTP (live on beta)

Outbound mail is **configured and working** on `beta.twofivesix.app` (verification, password reset, friend invites, engagement digest, Panoply daily status, council invite review). Same variables in `~/256.app/.env` as local dev.

| Variable | Notes |
|----------|--------|
| `SMTP_HOST` | Relay hostname |
| `SMTP_PORT` | Usually `587` |
| `SMTP_TLS_MODE` | `starttls` (typical) |
| `SMTP_USERNAME` / `SMTP_PASSWORD` | Relay credentials |
| `SMTP_FROM` | e.g. `256.app <noreply@…>` |

After editing: `sudo systemctl restart twofivesix`.

**Verify:** `curl -s http://127.0.0.1:8080/health/status` → `"smtp_configured": true`.

---

## Job scheduler (in-process)

Recurring jobs are **not** cron and **not** triggered by page views. The `twofivesix` process runs a background loop every `SCHEDULER_TICK_SECS` (default **1**).

| Variable | Notes |
|----------|--------|
| `SCHEDULER_TICK_SECS` | `1` on beta (set `0` to disable auto-schedule) |
| `SCHEDULER_BATCH_SIZE` | Default **16** (beta: keep ≥16 so round-robin reaches council + panoply jobs) |
| `JOB_CONDUCTOR_PARALLELISM` | Default **4** — max concurrent handlers per conductor tick (beta: `4`–`8`) |
| `JOB_CONDUCTOR_LLM_PARALLELISM` | Default **2** — sub-cap for gateway/LLM-heavy job types |
| `FEED_POLL_WALL_BUDGET_SECS` | Default **120** — RSS/crawl wall clock per `council_portfolio_feed_poll` run (handler timeout ≈ budget + 30s) |
| `JOB_QUEUE_LEASE_SECS` | Default **1800** (30 min). Council reply/share ticks run sequential LLM calls — if jobs log `lease expired without completion`, raise this on the VM. |
| `VERTEX_GATEWAY_MAX_RPM` | Default `20` on `ai-gateway` — shared Vertex budget |
| `VERTEX_GATEWAY_BURST` | Default `1` |
| `COUNCIL_INVITE_REVIEW_EMAIL` | Review inbox for recruitment (default `owner@example.com`) |
| `PLATFORM_RECRUITMENT_LIVE` | Omit or `0` — review-only invites |
| `OPS_JOB_QUEUE_TOKEN` | Optional manual `POST /ops/jobs/run` |
| `AI_GRPC_ENDPOINT` | **`http://127.0.0.1:50051`** on the VM (not the public hostname) |
| `JOURNAL_RETENTION_DAYS` | Default **3** — systemd journal kept by daily `system_journal_retention` job (`journalctl --vacuum-time`) |

**Journal retention (one-time VM setup):** logs live in the **systemd journal**, not a flat file. Run once as the service user (installs a journald drop-in `MaxRetentionSec=3day` — the primary mechanism — plus a NOPASSWD sudoers rule for the daily in-app vacuum job):

```bash
sudo bash ~/256.app/scripts/ops/install_journal_retention_sudoers.sh
```

Verify: `cat /etc/systemd/journald.conf.d/256app-retention.conf`, then `sudo journalctl --disk-usage` after a day. If the `twofivesix` unit sets `NoNewPrivileges`, the in-app vacuum cannot use sudo — the journald drop-in still enforces retention on its own.

**Verify:** `journalctl -u twofivesix | grep "job scheduler starting"` after restart; `/health/status` shows `scheduler_enabled: true`, `job_conductor_mode: "parallel_capped"`, `job_conductor_parallelism`, `journal_retention_days: 3`, `scheduler_tick_count` increasing, `job_queue_pending_due` near zero when healthy; `/health/jobs` shows `sample_last_error` empty and council types in `pending_by_type` not growing without bound. After UTC midnight (or manual `POST /ops/jobs/run`), `journalctl -u twofivesix --since "1 day ago" | grep system_journal_retention completed`. If Panoply mail is absent while SMTP is configured, confirm `ops_digest_live: true` on `/health/status` (set `OPS_DIGEST_LIVE=1` or unset in `~/256.app/.env` — local dev uses `0` to avoid duplicate operator digests).

**Migration checksum mismatch (`VersionMismatch`):** if a migration file was edited after apply, run on the VM from `~/256.app`:

```bash
cargo run --bin repair_sqlx_checksums
sudo systemctl restart twofivesix
```

This realigns `_sqlx_migrations.checksum` with on-disk files and applies any pending migrations.

**Council reply fan-out backlog (2026-08-17):** migration `20260817100000_purge_council_reply_fanout_backlog.sql` runs automatically on deploy — purges duplicate `council_reply_member` / coordinator rows and resets `ops_daily_digest_sent` claims for council fan-out jobs. No manual SQL needed after this ships.

**Council daily share/invite coordinators (2026-08-18):** migration `20260818100000_reset_council_daily_coordinator_claims.sql` clears burned `council_post_tick` / `council_invite_tick` period claims (and any stuck share/invite queue rows) so the post-deploy app can fan out once the same day. Pairs with the handler fix that claims only after `load_active_council_members` returns rows. After deploy, expect one `council_post_tick coordinator completed members=12 enqueued=12` in `journalctl` (not repeated `members=0` spam).

**Feed poll starvation + council pipeline (2026-08-19):** migration `20260819100000_council_queue_starvation_recovery.sql` drops stuck feed-poll rows, resets daily coordinator / panoply / feed-poll period claims, and bumps pending share jobs to priority 76. Code fix: skip re-enqueue when `ops_daily_digest_sent` already holds the period; feed poll claims its 6h bucket; default `SCHEDULER_BATCH_SIZE=16`. After deploy expect `council_share_shortlist` dispatch within minutes and `panoply_daily_status completed sent=1`.

**Queue backlog (council share jobs):** If `/health/jobs` shows thousands of `council_share_shortlist` rows and `sample_last_error: lease expired without completion`, the conductor was starved (often by a single `council_portfolio_feed_poll` hitting the 300s handler timeout while running sequentially). Deploy builds with **`JOB_CONDUCTOR_PARALLELISM`** (default 4) and **`FEED_POLL_WALL_BUDGET_SECS`** (default 120) so feed poll cannot monopolize the tick. On the VM (MySQL), purge stale pending rows then drain:

```sql
-- Preview counts by job type
SELECT rk.`key`, COUNT(*) FROM job_queue jq
INNER JOIN ref_key_values rk ON rk.id = jq.job_type_id AND rk.`group` = 'ops_job_type'
WHERE jq.status IN ('pending','leased')
GROUP BY rk.`key`;

-- Drop stale daily council jobs (utc_date before today UTC)
DELETE jq FROM job_queue jq
INNER JOIN ref_key_values rk ON rk.id = jq.job_type_id AND rk.`group` = 'ops_job_type'
WHERE jq.status = 'pending'
  AND rk.`key` IN (
    'council_share_shortlist','council_share_read','council_share_publish',
    'council_invite_article_pick','council_invite_person_discover',
    'council_invite_contact_resolve','council_invite_compose_send'
  )
  AND JSON_UNQUOTE(JSON_EXTRACT(jq.payload, '$.utc_date')) < DATE_FORMAT(UTC_TIMESTAMP(), '%Y-%m-%d');
```

Then bump drain cadence temporarily: `SCHEDULER_BATCH_SIZE=32`, `JOB_QUEUE_LEASE_SECS=1800` in `~/256.app/.env`, restart `twofivesix`, and/or:

```bash
curl -s -X POST http://127.0.0.1:8080/ops/jobs/run \
  -H "Authorization: Bearer $OPS_JOB_QUEUE_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"batch_size": 64}'
```

Ship the dedupe fix (coordinator payloads must not include random `correlation_id`) so daily enqueues collapse to one row per councillor per day. **As of 2026-06-30:** the conductor also **auto-purges** stale daily / 15-min bucket rows each tick (`purge_stale_period_jobs` in `src/jobs/queue.rs`) — manual SQL is only needed for one-time backlog recovery after a bad deploy.

---

## VM layout

| Path | Role |
|------|------|
| `~/256.app/` | Git checkout — **must** be `WorkingDirectory` for `twofivesix` |
| `~/256.app/target/release/app` | Main web binary (systemd `twofivesix`) |
| `~/256.app/target/release/ai_gateway` | gRPC gateway (`ai-gateway`) |
| `~/256.app/locales/` | Runtime i18n (required at boot — see below) |
| `~/256.app/static/`, `agents/` | Served / council specs at runtime |
| `~/256.app/.env` | Production env (`EnvironmentFile` in systemd) |
| `~/deploy-staging/` | SCP drop zone from CI |

### Runtime app root (CI-built binaries)

Binaries built on GitHub embed compile-time paths from the runner. At runtime the app resolves the repo root via [`src/app_root.rs`](../src/app_root.rs):

1. `APP_ROOT` env (optional)
2. Process **working directory** (`WorkingDirectory=~/256.app`)
3. Walk up from `target/release/app`
4. Dev fallback: `CARGO_MANIFEST_DIR`

**Symptom if misconfigured:** panic `i18n load failed: read locales dir: No such file or directory`.

---

## systemd units

Both services should load `~/256.app/.env` and use:

```ini
WorkingDirectory=/home/deploy/256.app
```

| Unit | Binary | Port |
|------|--------|------|
| `twofivesix` | `target/release/app` | 8080 (behind HTTPS terminator) |
| `ai-gateway` | `target/release/ai_gateway` | 50051 (localhost) |

Post-deploy checks:

```bash
sudo systemctl is-active twofivesix ai-gateway
curl -s http://127.0.0.1:8080/health/status
bash ~/256.app/scripts/ops/verify_beta_ai.sh   # after AI creds configured
```

---

## What the pipeline does

On every **pull request** and **push to `main`** ([`.github/workflows/ci.yml`](../.github/workflows/ci.yml)):

1. **`cargo test`** — integration suite against ephemeral MySQL 8.4 ([`.github/workflows/test.yml`](../.github/workflows/test.yml))
2. **`npm run test:public`** — Playwright guest/login sweep (app + `e2e_seed` on same MySQL)

**Deploy to beta** ([`.github/workflows/deploy.yml`](../.github/workflows/deploy.yml)) runs the same test workflow first, then build → SCP → restart.

## What the pipeline does **not** do yet

- Run Playwright `test:smoke` (authenticated home/onboarding — stabilize locally first).
- Ship Docker images or blue/green rollback.
- Write GCP credentials from GitHub (manual / future secret step).
- Monitoring or alerting hooks.

**Rollback plan — safety vendor tier:** Manual rollback today is `git reset` + prior binary on the VM (see deploy workflow). **Paid trust-and-safety vendors** (hash API, hotline automation, moderation SaaS) are **out of scope until the product has users and revenue**; until then rely on Panoply operators + in-app crisis numbers (**Safety & Crisis** legal card, judicial/emergency surfaces; optional `/crisis-help`). When traction warrants spend, budget vendor contracts from real operating income.

Track these under **Deploy / ops** in [`LAUNCH_READINESS.md`](LAUNCH_READINESS.md).

---

## Troubleshooting (quick)

| Error | Fix |
|-------|-----|
| `can't connect without a private SSH key` | `VM_SSH_KEY` empty or misnamed |
| `dial tcp 172.67.x.x:22: i/o timeout` | `VM_HOST` is Cloudflare — use GCP external IP |
| `getKeyFile error: open ~/.ssh/gha_deploy` | Fixed in workflow: appleboy actions run in Docker and cannot read runner `key_path`. Ensure latest `deploy.yml` uses `key:` from the prepare step. |
| `unable to authenticate` | Confirm `VM_SSH_KEY` is the matching **private** key (or set `VM_SSH_KEY_B64`); register `gha_deploy.pub` in **GCP instance SSH metadata**; run **Deploy SSH check** workflow. |
| `Deploy private key is invalid` | Re-paste full PEM into `VM_SSH_KEY` or use `VM_SSH_KEY_B64` from `generate_gha_deploy_key.ps1` |
| SSH works once, fails on next deploy | Guest agent reverted `authorized_keys` — public key missing from **instance metadata** |
| `i18n load failed: read locales dir` | `WorkingDirectory` + `app_root` fix; ensure `~/256.app/locales/en.json` exists after `git reset` |
| `activating` / exit 3 on `is-active` | See `journalctl -u twofivesix`; often crash loop before app_root fix |
| Nom modal "not configured" | `STRIPE_PUBLISHABLE_KEY` + `STRIPE_SECRET_KEY` missing from `~/256.app/.env`; restart `twofivesix` after edit |
| Payment succeeds, balance unchanged | Webhook not registered or `STRIPE_WEBHOOK_SECRET` wrong — check Stripe → Webhooks → delivery log |
| `manual Stripe refund required` in logs | Paid after wallet became ineligible — refund in Stripe Dashboard |
| `No space left on device` on deploy | **Beta VM:** SSH cleanup in [Disk cleanup](#disk-cleanup-beta-vm). **GitHub Actions:** runner disk — see [CI runner disk](#ci-runner-disk-not-the-beta-vm) |

---

## CI runner disk (not the beta VM)

If deploy fails during **`cargo test`** or **`Swatinem/rust-cache`** with paths under `/home/runner/`, the **GitHub-hosted runner** is out of space — not the beta VM.

Common cause: `rust-cache` was saving the full `target/debug` tree after integration tests (~30 test binaries). Fix shipped in workflow:

- Test jobs: **restore only** (`save-if: false`) + `cargo clean` after tests
- Build job: cache **release** artifacts only
- `scripts/ci/free_runner_disk.sh` removes unused SDKs before each job
- `ci.yml` no longer runs on push to `main` (deploy already runs the same test suite)

If it still fails, re-run the workflow once (cache key `256-v2` drops old blobs) or delete the repo **Actions → Caches** entry manually.

---

## Disk cleanup (beta VM)

Deploy needs free space for `git fetch`, SCP binaries, and journal writes. The VM only needs **release binaries** from CI — not a local `target/debug` tree.

**One-shot (SSH from laptop):**

```bash
ssh -i ~/.ssh/gha_deploy -o IdentitiesOnly=yes deploy@<VM_HOST> 'bash ~/256.app/scripts/ops/cleanup_beta_disk.sh'
```

If the script is not on the VM yet (deploy cannot pull), run manually:

```bash
df -h
du -sh ~/* ~/256.app/* 2>/dev/null | sort -hr | head -15
rm -rf ~/256.app/target/debug ~/256.app/target/diag
find ~/256.app/target/release -mindepth 1 -maxdepth 1 ! -name app ! -name ai_gateway ! -name db_reset -exec rm -rf {} +
rm -rf ~/deploy-staging/*
sudo journalctl --vacuum-time=3d
cd ~/256.app && git gc --prune=now
df -h
```

**Safe to delete:** `target/debug`, `target/diag`, extra files under `target/release/`, `~/deploy-staging/*`, `~/.cargo/*` (if present), apt cache, old journal logs.

**Keep:** `~/256.app/.env`, `.secrets/`, `target/release/{app,ai_gateway,db_reset}`, `static/generated/user-faces/` (user avatars), MySQL data.

**Do not wipe** unless intentional: database reset workflow ([Database reset](#database-reset-beta-fresh-seed)).

After cleanup, re-run deploy. The deploy workflow runs disk reclaim **before** SCP (never wipes `~/deploy-staging` after binaries are copied).

**Deploy order:** reclaim disk → SCP to `~/deploy-staging/` → `install` to `target/release/` → remove staging copies → restart.
