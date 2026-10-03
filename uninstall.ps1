# Removes COMPX Tray: processes, install folder, preferences, autostart entry and backups.
# Settings already written to the mouse stay on the mouse. Pass -KeepBackups to keep the JSON backups.
param([switch]$KeepBackups)
$ErrorActionPreference = 'Stop'

Get-Process compx-tray, compx-tray-ui -ErrorAction SilentlyContinue |
    ForEach-Object { $_.Kill(); $_.WaitForExit(3000) | Out-Null }

Remove-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name 'COMPX Tray' -ErrorAction SilentlyContinue
Remove-Item 'HKCU:\Software\compx-tray' -Recurse -ErrorAction SilentlyContinue
Remove-Item (Join-Path $env:LOCALAPPDATA 'Programs\compx-tray') -Recurse -Force -ErrorAction SilentlyContinue

$backups = Join-Path $env:LOCALAPPDATA 'compx-tray'
if ((Test-Path $backups) -and -not $KeepBackups) { Remove-Item $backups -Recurse -Force }
Write-Host 'COMPX Tray removed'
