#!/usr/bin/env bash
# Reclaim disk on the beta VM before deploy (or when df reports 100%).
# Run **before** SCP copies new binaries into ~/deploy-staging — not after.
set -euo pipefail

APP_ROOT="${APP_ROOT:-$HOME/256.app}"
STAGING="${DEPLOY_STAGING:-$HOME/deploy-staging}"

echo "=== disk before ==="
df -h / /home 2>/dev/null || df -h /

echo "=== largest paths (home + app) ==="
du -sh "$HOME"/* "$APP_ROOT"/* 2>/dev/null | sort -hr | head -20 || true

freed=0

remove_if_exists() {
  local path="$1"
  if [[ -e "$path" ]]; then
    local before
    before="$(du -sk "$path" 2>/dev/null | awk '{print $1}')"
    rm -rf "$path"
    freed=$((freed + before))
    echo "removed: $path (~$((before / 1024)) MB)"
  fi
}

# Dev/debug Rust artifacts — never needed on beta (CI ships release binaries).
remove_if_exists "$APP_ROOT/target/debug"
remove_if_exists "$APP_ROOT/target/diag"
remove_if_exists "$APP_ROOT/target/tmp"
remove_if_exists "$APP_ROOT/target/flycheck0"

# Incremental release build junk if someone ran cargo on the VM; keep the three binaries.
if [[ -d "$APP_ROOT/target/release" ]]; then
  find "$APP_ROOT/target/release" -mindepth 1 -maxdepth 1 \
    ! -name app ! -name ai_gateway ! -name db_reset ! -name '*.d' \
    -exec rm -rf {} + 2>/dev/null || true
  echo "trimmed target/release/ (kept app, ai_gateway, db_reset)"
fi

# Staging drop zone from prior deploys.
remove_if_exists "$STAGING"/*
mkdir -p "$STAGING"

# Local cargo cache only if cargo was ever run on the VM.
remove_if_exists "$HOME/.cargo/registry"
remove_if_exists "$HOME/.cargo/git"

# Apt package cache (Ubuntu VM).
if command -v apt-get >/dev/null 2>&1; then
  sudo apt-get clean -y 2>/dev/null || true
fi

# systemd journals — keep 3 days (daily job also vacuums; deploy uses same window).
if command -v journalctl >/dev/null 2>&1; then
  sudo journalctl --vacuum-time=3d 2>/dev/null || true
fi

# Shrink git object store so fetch has headroom.
if [[ -d "$APP_ROOT/.git" ]]; then
  (cd "$APP_ROOT" && git gc --prune=now --quiet) || true
fi

echo "=== disk after (~$((freed / 1024)) MB reclaimed from listed trees) ==="
df -h / /home 2>/dev/null || df -h /
