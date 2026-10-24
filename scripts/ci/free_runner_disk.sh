#!/usr/bin/env bash
# Reclaim space on GitHub-hosted ubuntu runners before Rust/Playwright jobs.
# Runners ship with large preinstalled SDKs we do not use (~10+ GB).
set -euo pipefail

echo "=== disk before cleanup ==="
df -h /

sudo rm -rf \
  /usr/share/dotnet \
  /usr/local/lib/android \
  /opt/ghc \
  /opt/hostedtoolcache/CodeQL \
  /usr/local/share/boost \
  /usr/share/swift \
  2>/dev/null || true

if command -v docker >/dev/null 2>&1; then
  sudo docker image prune --all --force 2>/dev/null || true
fi

echo "=== disk after cleanup ==="
df -h /
