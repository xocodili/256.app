#!/usr/bin/env bash
# GitHub Actions + local helper: validate deploy secrets and write ~/.ssh/gha_deploy.
set -euo pipefail

KEY_PATH="${HOME}/.ssh/gha_deploy"

validate_secrets() {
  local missing=0
  if [[ -z "${VM_HOST:-}" ]]; then
    echo "::error::VM_HOST secret is empty"
    missing=1
  fi
  if [[ -z "${VM_USER:-}" ]]; then
    echo "::error::VM_USER secret is empty"
    missing=1
  fi
  if [[ -z "${VM_SSH_KEY:-}" && -z "${VM_SSH_KEY_B64:-}" ]]; then
    echo "::error::Set VM_SSH_KEY (preferred) or VM_SSH_KEY_B64 in GitHub Actions secrets"
    missing=1
  fi
  if [[ "${VM_HOST:-}" =~ ^172\.67\. ]]; then
    echo "::error::VM_HOST looks like Cloudflare (172.67.x.x). Use the GCP VM external IP, not beta.twofivesix.app."
    missing=1
  fi
  if [[ $missing -ne 0 ]]; then
    exit 1
  fi
  echo "Deploy secrets present (host/user/key)."
}

install_key() {
  install -m 700 -d "${HOME}/.ssh"
  if [[ -n "${VM_SSH_KEY_B64:-}" ]]; then
    printf '%s' "$VM_SSH_KEY_B64" | tr -d '\r\n ' | base64 -d >"$KEY_PATH"
  else
    printf '%s\n' "$VM_SSH_KEY" | tr -d '\r' >"$KEY_PATH"
  fi
  chmod 600 "$KEY_PATH"
  if ! ssh-keygen -y -f "$KEY_PATH" >/dev/null 2>&1; then
    echo "::error::Deploy private key is invalid. Re-paste VM_SSH_KEY (full OPENSSH PEM) or use VM_SSH_KEY_B64."
    exit 1
  fi
  echo "Deploy key installed at $KEY_PATH"
}

print_fingerprint() {
  if [[ ! -f "$KEY_PATH" ]]; then
    install_key
  fi
  echo "Deploy key fingerprint (must match public key in GCP instance SSH metadata):"
  ssh-keygen -lf "$KEY_PATH"
  echo "Public key line for GCP:"
  ssh-keygen -y -f "$KEY_PATH"
}

# Write private key to GITHUB_OUTPUT for appleboy/* actions (legacy; prefer native ssh in deploy.yml).
emit_key_output() {
  install_key
  if [[ -z "${GITHUB_OUTPUT:-}" ]]; then
    echo "::error::emit-key-output requires GITHUB_OUTPUT (GitHub Actions only)" >&2
    exit 1
  fi
  {
    echo "private_key<<EOF"
    cat "$KEY_PATH"
    echo "EOF"
  } >>"$GITHUB_OUTPUT"
  echo "Deploy key written to step output for appleboy/ssh-action"
}

case "${1:-}" in
  validate-secrets) validate_secrets ;;
  install-key) install_key ;;
  print-fingerprint) print_fingerprint ;;
  emit-key-output) emit_key_output ;;
  *)
    echo "Usage: $0 {validate-secrets|install-key|print-fingerprint|emit-key-output}" >&2
    exit 2
    ;;
esac
