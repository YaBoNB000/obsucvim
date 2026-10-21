param(
  [switch]$LocalOnly,
  [switch]$OpenFirewall
)
$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
$Node = Join-Path $Root "runtime\node.exe"
$Server = Join-Path $Root "tools\engine-server.mjs"
$Wasm = Join-Path $Root "tools\obsucvim.wasm"
$Dist = Join-Path $Root "dist"
$Data = Join-Path $Root "data"
$Port = if ($env:PORT) { $env:PORT } else { "8787" }

if (!(Test-Path $Node)) { throw "Bundled Node.js runtime missing: $Node" }
if (!(Test-Path $Server)) { throw "Web engine server missing: $Server" }
if (!(Test-Path $Wasm)) { throw "Obsucvim WASI engine missing: $Wasm" }
if (!(Test-Path (Join-Path $Dist "index.html"))) { throw "Built web UI missing: $Dist\index.html" }
New-Item -ItemType Directory -Force -Path $Data | Out-Null

# Portable Node + Rust/WASI engine: no Rust compiler, MSVC, or Python required.
$env:OBSUCVIM_BIN = Join-Path $Root "tools\obsucvim.exe" # absent by design; forces WASI fallback
$env:OBSUCVIM_WASM = $Wasm
$env:OBSUCVIM_WEB_ROOT = $Dist
$env:OBSUCVIM_STATE_FILE = Join-Path $Data "state.json"
$env:HOST = if ($LocalOnly) { "127.0.0.1" } else { "0.0.0.0" }
$env:PORT = $Port
if (!$env:SMTP_USER) { $env:SMTP_USER = "obsucvim@163.com" }
if (!$env:SMTP_FROM) { $env:SMTP_FROM = $env:SMTP_USER }
if (!$env:SMTP_HOST) { $env:SMTP_HOST = "smtp.163.com" }
if (!$env:SMTP_PORT) { $env:SMTP_PORT = "465" }

Write-Host ""
Write-Host "Obsucvim Web + Rust/WASI is starting..." -ForegroundColor Cyan
Write-Host "URL:  http://localhost:$Port" -ForegroundColor Green
if (!$LocalOnly) {
  Write-Host "Network URL: http://<this-server-ip>:$Port" -ForegroundColor Green
  Write-Host "IMPORTANT: For public Internet, put this behind HTTPS (IIS/reverse proxy); do not expose plain HTTP login directly." -ForegroundColor Yellow
}
if (!$env:SMTP_PASS) {
  Write-Host "Mail: DEMO mode (verification code is shown in the UI). Set SMTP_PASS privately to enable 163 SMTP." -ForegroundColor Yellow
} else {
  Write-Host "Mail: SMTP configured for $env:SMTP_USER" -ForegroundColor Green
}
Write-Host "State: $env:OBSUCVIM_STATE_FILE" -ForegroundColor DarkGray
Write-Host "Stop with Ctrl+C." -ForegroundColor DarkGray
Write-Host ""

if ($OpenFirewall) {
  $isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
  if (!$isAdmin) { throw "-OpenFirewall requires an elevated PowerShell window." }
  $ruleName = "Obsucvim Web TCP $Port"
  if (!(Get-NetFirewallRule -DisplayName $ruleName -ErrorAction SilentlyContinue)) {
    New-NetFirewallRule -DisplayName $ruleName -Direction Inbound -Protocol TCP -LocalPort $Port -Action Allow -Profile Domain,Private | Out-Null
    Write-Host "Created Windows Firewall rule '$ruleName' (Domain/Private profiles only)." -ForegroundColor Green
  }
}

& $Node $Server
exit $LASTEXITCODE
