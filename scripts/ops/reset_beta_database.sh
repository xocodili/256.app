#!/usr/bin/env bash
# Drop and recreate the app MySQL database (migration seeds re-apply on next app start).
#
# Manual (on beta VM):
#   cd ~/256.app
#   DB_RESET_ALLOW=1 bash scripts/ops/reset_beta_database.sh --restart
#
# Deploy automation passes --deploy-ok (sets DB_RESET_ALLOW) and uses binaries in target/release/.
set -euo pipefail

APP_ROOT="${APP_ROOT:-$HOME/256.app}"
ENV_FILE="${ENV_FILE:-$APP_ROOT/.env}"
DB_RESET_BIN="${DB_RESET_BIN:-$APP_ROOT/target/release/db_reset}"
RESTART_SERVICES=0
DEPLOY_OK=0

usage() {
  cat <<'EOF'
Usage: reset_beta_database.sh [--restart] [--deploy-ok]

  --restart    Restart ai-gateway + twofivesix after reset (migrations run on app boot)
  --deploy-ok  Allow reset without DB_RESET_ALLOW=1 (GitHub Actions deploy only)

Requires target/release/db_reset and DATABASE_URL in .env.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --restart) RESTART_SERVICES=1; shift ;;
    --deploy-ok) DEPLOY_OK=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

if [[ ! -f "$ENV_FILE" ]]; then
  echo "FAIL: missing $ENV_FILE" >&2
  exit 1
fi

# shellcheck disable=SC1090
set -a && source "$ENV_FILE" && set +a

if [[ -z "${DATABASE_URL:-}" ]]; then
  echo "FAIL: DATABASE_URL unset in $ENV_FILE" >&2
  exit 1
fi

if [[ "$DEPLOY_OK" != "1" ]]; then
  if [[ "${DB_RESET_ALLOW:-}" != "1" && "${DB_RESET_ALLOW:-}" != "true" ]]; then
    echo "FAIL: set DB_RESET_ALLOW=1 to confirm destructive database reset" >&2
    exit 1
  fi
fi

if [[ ! -x "$DB_RESET_BIN" ]]; then
  echo "FAIL: db_reset binary not found or not executable: $DB_RESET_BIN" >&2
  echo "Build with: cargo build --release --bin db_reset" >&2
  exit 1
fi

db_name="${DATABASE_URL%%\?*}"
db_name="${db_name##*/}"

echo "== 256.app database reset =="
echo "APP_ROOT=$APP_ROOT"
echo "Database: $db_name"
echo "WARNING: all users, posts, sessions, and purchases will be destroyed."
echo "Migration seeds (council, operator @sphinx, content domains) will be recreated on app start."

export DB_RESET_ALLOW=1

echo "Stopping twofivesix + ai-gateway..."
sudo systemctl stop twofivesix ai-gateway || true

echo "Running db_reset..."
"$DB_RESET_BIN"

if [[ "$RESTART_SERVICES" == "1" ]]; then
  echo "Starting ai-gateway + twofivesix (migrations apply on boot)..."
  sudo systemctl start ai-gateway
  sudo systemctl start twofivesix

  for unit in ai-gateway twofivesix; do
    for i in $(seq 1 30); do
      state="$(systemctl is-active "$unit" 2>/dev/null || true)"
      if [[ "$state" == "active" ]]; then
        echo "OK: $unit is active"
        break
      fi
      if [[ "$i" -eq 30 ]]; then
        echo "FAIL: $unit did not become active" >&2
        journalctl -u "$unit" -n 40 --no-pager || true
        exit 1
      fi
      sleep 3
    done
  done

  curl -sf "http://127.0.0.1:8080/health/status" | head -c 500 || true
  echo
fi

echo "Database reset complete."
