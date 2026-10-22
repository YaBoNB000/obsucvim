<#
  远程升级已部署的 Obsucvim（保留 data\：账户、卡密、发布产物、额度都不动）。

  最简用法（在安装目录里）：
      .\Update-Obsucvim.ps1

  指定版本：
      .\Update-Obsucvim.ps1 -Sha256 "1eac29..."          # 校验哈希后再覆盖
      .\Update-Obsucvim.ps1 -Url "https://.../x.zip"      # 换下载源（自建镜像）
      .\Update-Obsucvim.ps1 -Zip "D:\x.zip"               # 用已经下好的离线包
      .\Update-Obsucvim.ps1 -DryRun                       # 只下载校验，不覆盖

  这个脚本只负责下载几 KB 的更新器；真正的下载、校验、备份、替换由
  self-update.mjs 完成（纯 Node，无第三方依赖）。
#>
param(
  [string]$InstallDir = (Split-Path -Parent $MyInvocation.MyCommand.Path),
  [string]$Url = "",
  [string]$Sha256 = "",
  [string]$Zip = "",
  [switch]$DryRun
)
$ErrorActionPreference = "Stop"

$rawBase = if ($env:OBSUCVIM_RAW_BASE) { $env:OBSUCVIM_RAW_BASE } else {
  "https://raw.githubusercontent.com/YaBoNB000/obsucvim/arena/01a038eb-luraph"
}
$officialZip = "$rawBase/release-assets/obsucvim-web-win64/Obsucvim-Web-Windows-x64.zip"

if (!(Test-Path $InstallDir)) {
  Write-Host "Install dir not found: $InstallDir" -ForegroundColor Red
  exit 1
}
$Root = (Resolve-Path $InstallDir).Path
$Node = Join-Path $Root "runtime\node.exe"
if (!(Test-Path $Node)) { $Node = "node" }

$UpdaterTmp = Join-Path $env:TEMP "obsucvim-self-update.mjs"
Write-Host "Fetching updater..." -ForegroundColor Cyan
Invoke-WebRequest -UseBasicParsing -Uri "$rawBase/web/deploy/self-update.mjs?v=$([DateTime]::Now.Ticks)" -OutFile $UpdaterTmp

$arguments = @("--dir", $Root)
if ($Zip) {
  $arguments += @("--zip", $Zip)
} else {
  if (!$Url) { $Url = $officialZip }
  $arguments += @("--url", $Url)
}
if ($Sha256) { $arguments += @("--sha256", $Sha256) }
if ($DryRun) { Write-Host "Dry run: only download and verify, nothing is replaced." -ForegroundColor Yellow }

& $Node $UpdaterTmp @arguments
$code = $LASTEXITCODE

if ($code -eq 0) {
  Write-Host ""
  Write-Host "Done. Restart Obsucvim with Start-Obsucvim.bat" -ForegroundColor Green
} else {
  Write-Host ""
  Write-Host "Update failed (exit $code). Nothing was half-written on purpose; see the message above." -ForegroundColor Red
}
exit $code
