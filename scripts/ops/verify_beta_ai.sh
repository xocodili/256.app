#!/usr/bin/env bash
# Quick checks that beta AI env is present (run on the VM as the deploy user).
set -euo pipefail

APP_ROOT="${APP_ROOT:-$HOME/256.app}"
ENV_FILE="${ENV_FILE:-$APP_ROOT/.env}"

echo "== 256.app beta AI verification =="
echo "APP_ROOT=$APP_ROOT"

fail=0
warn() { echo "WARN: $*"; }
ok() { echo "OK:   $*"; }
die() { echo "FAIL: $*"; fail=1; }

if [[ -f "$ENV_FILE" ]]; then
  # shellcheck disable=SC1090
  set -a && source "$ENV_FILE" && set +a
  ok "sourced $ENV_FILE"
else
  die "missing $ENV_FILE"
fi

if [[ "${FACE_VISION_MOCK:-0}" == "1" ]] || [[ "${FACE_VISION_MOCK:-}" == "true" ]]; then
  die "FACE_VISION_MOCK is enabled — real vision is disabled"
else
  ok "FACE_VISION_MOCK off"
fi

cred="${GOOGLE_APPLICATION_CREDENTIALS:-}"
if [[ -z "$cred" ]]; then
  die "GOOGLE_APPLICATION_CREDENTIALS unset"
elif [[ ! -r "$cred" ]]; then
  die "credentials not readable: $cred"
else
  ok "credentials file readable ($cred)"
fi

proj="${GOOGLE_CLOUD_PROJECT:-${VERTEX_PROJECT_ID:-${GOOGLE_PROJECT_ID:-}}}"
if [[ -z "$proj" ]]; then
  die "GOOGLE_CLOUD_PROJECT (or VERTEX_PROJECT_ID) unset — required for Imagen raster on twofivesix"
else
  ok "project id set ($proj)"
fi

portrait_model="${PORTRAIT_RASTER_MODEL:-${VERTEX_IMAGE_MODEL:-imagen-3.0-capability-001}}"
if [[ "$portrait_model" == *capability* ]]; then
  ok "PORTRAIT_RASTER_MODEL=$portrait_model"
else
  die "PORTRAIT_RASTER_MODEL must be an Imagen *capability* model (subject refs); got: $portrait_model — set imagen-3.0-capability-001 in ~/256.app/.env and restart twofivesix"
fi

endpoint="${AI_GRPC_ENDPOINT:-${GOOGLE_GRPC_ENDPOINT:-http://127.0.0.1:50051}}"
ok "AI gRPC endpoint: $endpoint"

if systemctl is-active --quiet ai-gateway 2>/dev/null; then
  ok "ai-gateway service active"
  if journalctl -u ai-gateway -n 30 --no-pager 2>/dev/null | grep -q "failed to load GOOGLE_APPLICATION_CREDENTIALS"; then
    warn "ai-gateway journal still shows failed credential load — restart after fixing .env"
  fi
else
  warn "ai-gateway not active (systemctl)"
fi

if systemctl is-active --quiet twofivesix 2>/dev/null; then
  ok "twofivesix service active"
else
  warn "twofivesix not active (systemctl)"
fi

if curl -sf --max-time 3 http://127.0.0.1:8080/health/status >/dev/null; then
  ok "app /health/status"
else
  warn "app /health/status not reachable"
fi

if curl -sf --max-time 5 http://127.0.0.1:8080/health/ai-gateway >/dev/null 2>&1; then
  ok "app /health/ai-gateway"
else
  warn "/health/ai-gateway failed or requires auth — check manually"
fi

art_dir="${GENERATED_FACE_ART_DIR:-static/generated/user-faces}"
if [[ -d "$APP_ROOT/$art_dir" ]]; then
  ok "artwork dir exists ($APP_ROOT/$art_dir)"
else
  warn "artwork dir missing — mkdir -p $APP_ROOT/$art_dir (app may create on first success)"
fi

style_plate="$APP_ROOT/agents/council/rag/epic/style-plate-bust-v6.png"
if [[ -r "$style_plate" ]]; then
  ok "Epic style plate v6 present ($(stat -c%s "$style_plate" 2>/dev/null || stat -f%z "$style_plate" 2>/dev/null || echo '?') bytes)"
else
  die "missing style plate: $style_plate (git deploy must ship agents/council/rag/epic/)"
fi

if [[ -r "$APP_ROOT/agents/council/epic.json" ]]; then
  ok "agents/council/epic.json present"
else
  die "missing agents/council/epic.json"
fi

echo "Disk:"
df -h / | tail -1 || true

if [[ "$fail" -ne 0 ]]; then
  echo ""
  echo "See docs/BETA_AI_SETUP.md"
  exit 1
fi

echo ""
echo "Env looks ready. Test onboarding face capture on beta in the browser."
