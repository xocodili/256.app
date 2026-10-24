#!/usr/bin/env bash
# Council pipeline snapshot for beta (no MySQL). Safe when greps match nothing.
# Usage: bash ~/256.app/scripts/ops/beta_council_diagnostic.sh 2>&1 | tee ~/council-diag.txt

hr() { printf '\n======== %s ========\n' "$1"; }

hr "script started"
date -u '+%Y-%m-%d %H:%M:%S UTC'

hr "host"
hostname

hr "deploy"
git -C "${HOME}/256.app" log -1 --oneline 2>/dev/null || echo "(git unavailable)"
systemctl show twofivesix -p ActiveEnterTimestamp --value 2>/dev/null || true

hr "services"
systemctl is-active twofivesix ai-gateway 2>/dev/null || true

hr "health/status"
curl -sf http://127.0.0.1:8080/health/status | jq . 2>/dev/null || curl -sf http://127.0.0.1:8080/health/status

hr "health/jobs"
curl -sf http://127.0.0.1:8080/health/jobs | jq . 2>/dev/null || curl -sf http://127.0.0.1:8080/health/jobs

hr "health/ai-gateway"
curl -sf http://127.0.0.1:8080/health/ai-gateway | jq . 2>/dev/null || curl -sf http://127.0.0.1:8080/health/ai-gateway

hr "feed poll 6h"
sudo journalctl -u twofivesix --since "6 hours ago" --no-pager 2>/dev/null \
  | grep council_portfolio_feed_poll | tail -20 || echo "(none)"

hr "shares 6h"
sudo journalctl -u twofivesix --since "6 hours ago" --no-pager 2>/dev/null \
  | grep -E "council_share|content share published|share shortlist|share read|share publish" \
  | tail -40 || echo "(none)"

hr "replies 24h"
sudo journalctl -u twofivesix --since "24 hours ago" --no-pager 2>/dev/null \
  | grep -E "council reply published|credit below" | tail -25 || echo "(none)"

hr "coordinators today"
sudo journalctl -u twofivesix --since today --no-pager 2>/dev/null \
  | grep -E "council_post_tick|council_invite_tick|panoply_daily_status" \
  | tail -25 || echo "(none)"

hr "dispatch 6h"
sudo journalctl -u twofivesix --since "6 hours ago" --no-pager 2>/dev/null \
  | grep -E "job_queue dispatch|handler timed out|handler failed" \
  | tail -40 || echo "(none)"

hr "done"
date -u '+%Y-%m-%d %H:%M:%S UTC'
