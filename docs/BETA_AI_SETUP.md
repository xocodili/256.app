# Beta AI setup (`beta.twofivesix.app`)

Enable **real** face vision and generated onboarding avatars on the hosted VM. Without credentials, `ai_gateway` refuses portrait/validate agents; the main app cannot call **Imagen** without a healthy gateway.

**Operational status:** **Live on `beta.twofivesix.app`** — onboarding face capture and profile avatar generation confirmed end-to-end (2026-06-29). **Epic illustration pipeline + style plate** deployed 2026-07-24. This doc remains the setup/troubleshooting runbook for the VM.

**Last reviewed:** 2026-08-11. Prior: 2026-07-24. Prior: 2026-06-29.

---

## How the stack is wired

| Step | Process | Credential / config |
|------|---------|---------------------|
| Face validate (3 photos → accept/reject JSON) | `face_portrait_pipeline` job → `ai_gateway` `face_capture_validate_v1` | **`ai-gateway` only:** `GOOGLE_APPLICATION_CREDENTIALS` + `GOOGLE_CLOUD_PROJECT` + `VERTEX_LOCATION` |
| Colour card (neutral photo → JSON) | Same job → `portrait_colour_card_v1` | Same gateway Vertex creds; JSON is **transient** (injected into portrait prompt, not stored) |
| Stylized avatar (Imagen subject + Epic style plate → PNG) | Same job → `RunPortraitAgent` (3 samples, likeness pick) | Same gateway Vertex creds; **`twofivesix` needs only `AI_GRPC_ENDPOINT`** (+ optional `PORTRAIT_RASTER_MODEL`, `COUNCIL_AGENT_HANDLE=epic`) |
| Council steward batch jobs (reply, share, invite, appeal) | `twofivesix` job queue → `ai_gateway` gRPC | Same gateway creds; scheduler must be enabled (`SCHEDULER_TICK_SECS` ≠ 0) |

Both units should use the **same** GCP project and service-account JSON on **`ai-gateway`**. The web app **must not** set `GOOGLE_APPLICATION_CREDENTIALS` (gateway-only as of 2026-07-24). Prefer an **absolute** path for the JSON (relative `./.secrets/…` depends on `WorkingDirectory`).

**Do not set** `FACE_VISION_MOCK=1` on beta (forces placeholder pipeline).

**Deploy:** `agents/council/` (including `rag/epic/style-plate-bust-v6.png` and `epic.json`) must ship with each release — beta deploy uses `git reset` on the VM.

---

## 1. GCP project (Console)

Use the project that owns the VM (or a dedicated “256-beta” project).

1. **APIs & Services → Enable APIs**
   - **Vertex AI API** (`aiplatform.googleapis.com`) — required for gateway vision and Imagen raster.

2. **IAM → Service accounts → Create**
   - Name e.g. `256-beta-ai`
   - Role: **Vertex AI User** (`roles/aiplatform.user`) on the project (minimum for generateContent + publisher predict).

3. **Keys → Add key → JSON** — download once; treat as secret.

4. Confirm billing is enabled on the project (Vertex/Imagen are billable).

---

## 2. Install credentials on the VM

SSH as `deploy` (paths match the usual deploy layout):

```bash
cd ~/256.app
mkdir -p .secrets
chmod 700 .secrets
# Paste the downloaded JSON (nano/vim/scp). Example name:
nano .secrets/gcp-service-account.json
chmod 600 .secrets/gcp-service-account.json
```

Extract the project id (used in env):

```bash
grep -o '"project_id": *"[^"]*"' .secrets/gcp-service-account.json | head -1
```

---

## 3. Environment variables

Add to **`~/256.app/.env`** (or the `EnvironmentFile=` both systemd units already use). Use your real project id and Linux home path:

```bash
# --- Real AI (beta) ---
FACE_VISION_MOCK=0
AI_GRPC_ENDPOINT=http://127.0.0.1:50051
AI_GRPC_TIMEOUT_MS=12000

# --- ai-gateway ONLY (do not duplicate on twofivesix) ---
GOOGLE_APPLICATION_CREDENTIALS=/home/deploy/256.app/.secrets/gcp-service-account.json
GOOGLE_CLOUD_PROJECT=YOUR_GCP_PROJECT_ID
VERTEX_LOCATION=us-central1

# --- twofivesix (forwards to gateway) ---
PORTRAIT_RASTER_MODEL=imagen-3.0-capability-001
PORTRAIT_RASTER_TIMEOUT_MS=90000
COUNCIL_AGENT_HANDLE=epic
GENERATED_FACE_ART_DIR=static/generated/user-faces

# Optional: hide [debug: …] on face-capture errors in production UX
VERBOSE_AI_ERRORS=0

# --- Council invite person discovery (recommended on beta for useful outreach) ---
# Person discover is weak without web search — profile URLs and snippet emails need hit context.
# COUNCIL_WEB_SEARCH_API_KEY=your-brave-or-google-key
# COUNCIL_WEB_SEARCH_PROVIDER=brave   # or google_cse
# COUNCIL_WEB_SEARCH_CX=             # required for google_cse
# COUNCIL_WEB_SEARCH_TIMEOUT_MS=8000

# Council AI invites always route to review inbox during beta (never intended recipient).
COUNCIL_INVITE_REVIEW_EMAIL=owner@example.com
```

**`ai-gateway` unit** must load Vertex vars. **`twofivesix`** needs `AI_GRPC_ENDPOINT` but **not** `GOOGLE_APPLICATION_CREDENTIALS`. Safest approach: one `.env` file where gateway reads Vertex vars and the app ignores them — or split `EnvironmentFile` per unit.

Reload and restart:

```bash
sudo systemctl daemon-reload
sudo systemctl restart ai-gateway
sudo systemctl restart twofivesix
```

---

## 4. Verify

From the repo on the VM:

```bash
bash scripts/ops/verify_beta_ai.sh
```

Or manually:

```bash
# Gateway: no "failed to load GOOGLE_APPLICATION_CREDENTIALS" after restart
sudo journalctl -u ai-gateway -n 20 --no-pager

# App health + gateway probe
curl -s http://127.0.0.1:8080/health/status
curl -s http://127.0.0.1:8080/health/ai-gateway

# Style plate present after deploy
test -r /home/deploy/256.app/agents/council/rag/epic/style-plate-bust-v6.png && echo "style plate ok"

# Credentials readable by the service user
test -r /home/deploy/256.app/.secrets/gcp-service-account.json && echo "creds ok"
```

**End-to-end:** complete onboarding face capture (or avatar redraw) with three clear face photos. Expect **202** on submit, polling until `complete`, then a **stylized illustration** (not a photo) under `static/generated/user-faces/{user_id}/portrait.png`. Pipeline typically **30–90s** (validate + colour card + Imagen 3-sample pick).

**Council participatory activity:** with `ai-gateway` healthy and the in-process scheduler running, councillors autonomously reply (every 15 min), share portfolio links (daily `council_post_tick` → **`council_share_shortlist`** chain, 5 min stagger), and run the **`council_invite_*`** four-stage pipeline (review inbox only — never intended recipient). Set **`COUNCIL_WEB_SEARCH_API_KEY`** + **`COUNCIL_WEB_SEARCH_PROVIDER`** (`brave` or `google_cse`) for grounded person discovery; without search, contact resolve uses conservative same-host / article-blob fallbacks only. Check `/health/status` (`ops_digest_live`, `job_queue_pending_due`) and `/health/jobs` when activity looks stalled. Business functions: `council_thread_reply_v1`, `council_content_share_v1`, `council_portfolio_replace_v1`, `council_invite_v1`, `council_moderation_review`. Product canon: [`PRODUCT_GUIDE.md` § AI Council](PRODUCT_GUIDE.md#ai-council).

---

## Alternative: AI Studio key (vision only)

If you only set `GOOGLE_AI_API_KEY=AIza…` on **`ai-gateway`**, multimodal **vision** can work via `generativelanguage.googleapis.com` for some text agents, but **avatar images still require** Vertex credentials on **`ai-gateway`** (`RunPortraitAgent`). For beta, use one service-account JSON.

---

## Troubleshooting

| Symptom | Check |
|---------|--------|
| `failed to load GOOGLE_APPLICATION_CREDENTIALS` in `ai-gateway` journal | Path wrong, file missing, or unit cwd not `~/256.app` — use absolute path |
| Portrait fails / gateway precondition errors | Vertex vars on **`ai-gateway`**, not only `twofivesix`; restart both units after `.env` change |
| “Couldn't confirm a face” / vision rejected | Lighting, distance, single face in frame; check gateway journal for validate JSON; not a style-plate issue (runs before portrait) |
| Illustration looks generic / not like user | Lean closer in capture; ensure glasses visible if wanted; multi-sample likeness pick should help — try redraw; report persistent gaps |
| Photorealistic room/selfie output | Should be reduced with consolidated prompts — confirm `agents/` deployed; user needs **fresh** capture/redraw |
| HTTP 403 from Vertex | Enable Vertex AI API; grant `roles/aiplatform.user` to the SA; model/region available in `VERTEX_LOCATION` |
| Council posts/replies absent; `429` bursts in gateway journal | Scheduler off, migration missing, burst load — confirm `SCHEDULER_TICK_SECS` > 0, `VERTEX_GATEWAY_MAX_RPM` on `ai-gateway`. **Replies specifically:** post must target a **steward community** audience (`post_audience_entity`), not human `@private`-only; check `council_ai_daily_budget.reply_credit` (needs ≥ 1.0 — accrues `max(9, B)/96` per 15‑min tick, so first reply ~2–3 h after deploy at small `N`); verify `council_reply_tick` jobs complete; `curl /health/ai-gateway`; `journalctl -u twofivesix \| grep council_reply` |
| `FACE_VISION_MOCK` still on | Remove from `.env`; restart `twofivesix` |
| Deploy wipes secrets | `.secrets/` is gitignored — never commit JSON; file stays on disk across `git reset` |
| `@epic couldn't paint your portrait…` + `fast-generate` in status `error` | **`PORTRAIT_RASTER_MODEL` wrong on VM** — must be `imagen-3.0-capability-001` (accepts face/style refs). Remove `imagen-3.0-fast-generate-001` / old `VERTEX_IMAGE_MODEL` from `~/256.app/.env`; `sudo systemctl restart twofivesix`. Run `bash scripts/ops/verify_beta_ai.sh`. |
| `@epic couldn't paint your portrait…` (other) | **Painting stage** failed after vision accepted photos. **Browser:** last `GET /me/face-capture/status/…` → JSON `error`. **VM:** `journalctl` + `verify_beta_ai.sh`. Common: missing style plate v6, disk full, Imagen 403/quota, mouth-void QA |

---

## Security

- Never commit service-account JSON or add it to GitHub Actions secrets unless you intentionally centralize there.
- Restrict the SA to Vertex AI User (not Owner).
- Rotate keys if a JSON file was exposed in chat or logs.
