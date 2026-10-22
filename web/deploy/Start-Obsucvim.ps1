param(
  [switch]$LocalOnly,
  [switch]$OpenFirewall,
  [string]$Port = ""
)

# 这个脚本只保留两件批处理做不了或不好做的事：防火墙规则、以及把参数
# 转交给 Start-Obsucvim.bat。真正的启动逻辑只有一处，避免两份实现跑偏。

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
$Bat = Join-Path $Root "Start-Obsucvim.bat"
if (!(Test-Path $Bat)) { throw "Launcher missing: $Bat" }

$EffectivePort = if ($Port) { $Port } elseif ($env:PORT) { $env:PORT } else { "8787" }

if ($OpenFirewall) {
  $isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
  if (!$isAdmin) { throw "-OpenFirewall requires an elevated PowerShell window." }
  $ruleName = "Obsucvim Web TCP $EffectivePort"
  if (!(Get-NetFirewallRule -DisplayName $ruleName -ErrorAction SilentlyContinue)) {
    New-NetFirewallRule -DisplayName $ruleName -Direction Inbound -Protocol TCP -LocalPort $EffectivePort -Action Allow -Profile Domain,Private | Out-Null
    Write-Host "Created Windows Firewall rule '$ruleName' (Domain/Private profiles only)." -ForegroundColor Green
  } else {
    Write-Host "Firewall rule '$ruleName' already exists." -ForegroundColor DarkGray
  }
}

$forward = @(if ($LocalOnly) { "/local" } else { "/public" })
if ($Port) { $forward += $Port }

& $Bat @forward
exit $LASTEXITCODE
