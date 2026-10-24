#!/usr/bin/env bash
# Register gha_deploy.pub in GCP instance SSH metadata (persistent — guest-agent safe).
#
# Usage:
#   ./register_gha_deploy_key_gcp.sh INSTANCE_NAME ZONE LINUX_USER [PUB_KEY_FILE]
#
# Example:
#   ssh-keygen -t ed25519 -f ~/.ssh/gha_deploy -C gha-deploy-beta -N ""
#   ./register_gha_deploy_key_gcp.sh beta-vm australia-southeast1-a deploy ~/.ssh/gha_deploy.pub
#
# Then paste ~/.ssh/gha_deploy (private) into GitHub secret VM_SSH_KEY.
set -euo pipefail

INSTANCE="${1:?INSTANCE_NAME required}"
ZONE="${2:?ZONE required}"
USER="${3:?LINUX_USER required (must match VM_USER secret)}"
PUB="${4:-${HOME}/.ssh/gha_deploy.pub}"

if [[ ! -f "$PUB" ]]; then
  echo "Public key not found: $PUB" >&2
  echo "Generate: ssh-keygen -t ed25519 -f ~/.ssh/gha_deploy -C gha-deploy-beta -N \"\"" >&2
  exit 1
fi

TMP="$(mktemp)"
trap 'rm -f "$TMP"' EXIT

gcloud compute instances describe "$INSTANCE" --zone="$ZONE" \
  --format='value(metadata.items.ssh-keys)' >"$TMP" 2>/dev/null || true

MARKER="gha-deploy-beta"
if grep -qF "$MARKER" "$TMP" 2>/dev/null; then
  echo "Removing previous gha_deploy line from metadata..."
  grep -vF "$MARKER" "$TMP" >"${TMP}.new" || true
  mv "${TMP}.new" "$TMP"
fi

echo "${USER}:$(cat "$PUB")" >>"$TMP"

gcloud compute instances add-metadata "$INSTANCE" --zone="$ZONE" \
  --metadata-from-file "ssh-keys=$TMP"

echo "OK — registered $(basename "$PUB") for Linux user $USER on $INSTANCE ($ZONE)"
echo "Next:"
echo "  1. GitHub repo → Settings → Secrets → VM_HOST (GCP external IP), VM_USER=$USER"
echo "  2. VM_SSH_KEY = private key matching $PUB"
echo "  3. Test: ssh -i ~/.ssh/gha_deploy -o IdentitiesOnly=yes ${USER}@<VM_IP> echo ok"
echo "  4. Actions → Deploy SSH check → Run workflow"
