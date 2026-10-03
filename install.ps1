# Builds in release and installs to %LOCALAPPDATA%\Programs\compx-tray.
# On start the tray refreshes the "Start with Windows" path.
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

cargo build --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$dest = Join-Path $env:LOCALAPPDATA 'Programs\compx-tray'
New-Item -ItemType Directory -Force $dest | Out-Null

Get-Process compx-tray, compx-tray-ui -ErrorAction SilentlyContinue | ForEach-Object { $_.Kill(); $_.WaitForExit(3000) | Out-Null }

$files = 'target\release\compx-tray.exe', 'target\release\compx-tray-ui.exe'
# With the GNU toolchain the panel loads WebView2Loader.dll at runtime.
if (Test-Path 'target\release\WebView2Loader.dll') { $files += 'target\release\WebView2Loader.dll' }
Copy-Item $files $dest -Force

Start-Process (Join-Path $dest 'compx-tray.exe')
Write-Host "COMPX Tray installed to $dest"
