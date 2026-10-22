param(
  [string]$InstallDir = (Split-Path -Parent $MyInvocation.MyCommand.Path),
  [string]$Url = "",
  [string]$Sha256 = "",
  [string]$Zip = "",
  [switch]$DryRun
)
$ErrorActionPreference = "Stop"

# 只下载更新器本身（几 KB），真正的下载/校验/替换由 self-update.mjs 完成。
$rawBase = if ($env:OBSUCVIM_RAW_BASE) { $env:OBSUCVIM_RAW_BASE } else {
  "https://raw.githubusercontent.com/YaBoNB000/obsucvim/arena/01a038eb-luraph"
}

$Root = (Resolve-Path $InstallDir).Path
$Node = Join-Path $Root "runtime\node.exe"
if (!(Test-Path $Node)) { $Node = "node" }

$UpdaterTmp = Join-Path $env:TEMP "obsucvim-self-update.mjs"
Write-Host "Fetching updater..." -ForegroundColor Cyan
Invoke-WebRequest -UseBasicParsing -Uri "$rawBase/web/deploy/self-update.mjs" -OutFile $UpdaterTmp

$arguments = @("--dir", $Root)
if ($Zip) { $arguments += @("--zip", $Zip) }
elseif ($Url) { $arguments += @("--url", $Url) }
else {
  Write-Host "Provide -Url <zip url> or -Zip <local zip>." -ForegroundColor Yellow
  Write-Host "Example:"
  Write-Host "  .\Update-Obsucvim.ps1 -Url $rawBase/release-assets/obsucvim-web-win64/Obsucvim-Web-Windows-x64.zip -Sha256 <hash>"
  exit 1
}
if ($Sha256) { $arguments += @("--sha256", $Sha256) }
if ($DryRun) { Write-Host "Dry run: the updater will still verify but you should inspect the output." -ForegroundColor Yellow }

& $Node $UpdaterTmp @arguments
exit $LASTEXITCODE
