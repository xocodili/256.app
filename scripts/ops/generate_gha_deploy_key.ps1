# Generate ed25519 deploy key pair for GitHub Actions → GCP beta VM.
# Run from repo root or anywhere; keys land in $env:USERPROFILE\.ssh\
param(
  [string]$KeyPath = "$env:USERPROFILE\.ssh\gha_deploy"
)

$sshDir = Split-Path -Parent $KeyPath
New-Item -ItemType Directory -Force -Path $sshDir | Out-Null

if (Test-Path $KeyPath) {
  Write-Host "Key already exists: $KeyPath"
  Write-Host "Delete it first if you want a new pair."
} else {
  ssh-keygen -t ed25519 -f $KeyPath -C "gha-deploy-beta" -N '""'
}

Write-Host ""
Write-Host "=== Public key (register in GCP VM → Edit → SSH Keys) ==="
Get-Content "$KeyPath.pub"
Write-Host ""
Write-Host "=== Or run (with gcloud): ==="
Write-Host "  bash scripts/ops/register_gha_deploy_key_gcp.sh INSTANCE ZONE deploy $KeyPath.pub"
Write-Host ""
Write-Host "=== Private key → GitHub secret VM_SSH_KEY ==="
Write-Host "  Repo → Settings → Secrets → Actions → VM_SSH_KEY"
Write-Host "  Paste entire file including BEGIN/END lines."
Write-Host ""
Write-Host "=== Optional base64 secret (if newlines break in GitHub UI) ==="
$bytes = [System.IO.File]::ReadAllBytes($KeyPath)
$b64 = [Convert]::ToBase64String($bytes)
Write-Host "  Add secret VM_SSH_KEY_B64 (single line, no spaces):"
Write-Host $b64
