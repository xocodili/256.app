#!/usr/bin/env bash
# One-time beta VM setup for 3-day systemd journal retention.
#
#   1. journald drop-in (MaxRetentionSec) — PRIMARY mechanism. journald enforces
#      the window continuously with no sudo needed from the app.
#   2. sudoers NOPASSWD rule so the twofivesix service user's daily
#      system_journal_retention job can also run explicit vacuums (belt and braces).
#
# Usage (as the service user):  sudo bash scripts/ops/install_journal_retention_sudoers.sh
# Or explicitly:                sudo APP_USER=deploy bash scripts/ops/install_journal_retention_sudoers.sh
set -euo pipefail

if [[ "$(id -u)" -ne 0 ]]; then
  echo "Run with sudo: sudo bash $0"
  exit 1
fi

# Under `sudo bash`, whoami is root — the service user is in SUDO_USER.
APP_USER="${APP_USER:-${SUDO_USER:-}}"
if [[ -z "$APP_USER" || "$APP_USER" == "root" ]]; then
  echo "Cannot determine the service user. Run: sudo APP_USER=<user> bash $0"
  exit 1
fi

RETENTION_DAYS="${JOURNAL_RETENTION_DAYS:-3}"

# 1. journald native retention (survives app restarts; no runtime sudo).
mkdir -p /etc/systemd/journald.conf.d
cat >/etc/systemd/journald.conf.d/256app-retention.conf <<EOF
[Journal]
MaxRetentionSec=${RETENTION_DAYS}day
SystemMaxUse=200M
EOF
systemctl restart systemd-journald
echo "journald drop-in installed: MaxRetentionSec=${RETENTION_DAYS}day, SystemMaxUse=200M"

# 2. NOPASSWD vacuum for the app-side daily job.
JOURNALCTL="$(command -v journalctl)"
if [[ -z "$JOURNALCTL" ]]; then
  echo "journalctl not found — skipping sudoers install"
  exit 0
fi

SUDOERS_FILE="/etc/sudoers.d/256app-journal-retention"
cat >"$SUDOERS_FILE" <<EOF
# 256.app daily system_journal_retention job — vacuum journals older than retention window.
${APP_USER} ALL=(root) NOPASSWD: ${JOURNALCTL} --vacuum-time *
EOF
chmod 0440 "$SUDOERS_FILE"
visudo -cf "$SUDOERS_FILE"
echo "Installed ${SUDOERS_FILE} for user ${APP_USER}"

# 3. Warn if the service unit blocks sudo entirely (journald drop-in still applies).
if systemctl cat twofivesix 2>/dev/null | grep -qiE '^\s*NoNewPrivileges\s*=\s*(yes|true|1)'; then
  echo "WARNING: twofivesix unit sets NoNewPrivileges — the in-app vacuum job cannot use sudo."
  echo "         The journald drop-in above still enforces retention on its own."
fi
