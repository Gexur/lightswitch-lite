# Installs LightSwitch Lite for the current user (no administrator needed).
#   .\install.ps1                     install and launch
#   .\install.ps1 -StartWithWindows   also enable start with Windows
#   .\install.ps1 -Uninstall          remove everything
param(
    [switch]$StartWithWindows,
    [switch]$Uninstall
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$dist = Join-Path $root 'dist'
if (-not (Test-Path (Join-Path $dist 'LightSwitchLite.exe'))) { $dist = $root }
$target = Join-Path $env:LOCALAPPDATA 'Programs\LightSwitchLite'
$startMenu = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs'
$link = Join-Path $startMenu 'LightSwitch Lite.lnk'
$runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$exe = Join-Path $target 'LightSwitchLite.exe'

if ($Uninstall) {
    Get-Process LightSwitchLite -ErrorAction SilentlyContinue | Stop-Process -Force
    Remove-ItemProperty -Path $runKey -Name 'LightSwitchLite' -ErrorAction SilentlyContinue
    if (Test-Path $link) { Remove-Item $link -Force }
    if (Test-Path $target) { Remove-Item $target -Recurse -Force }
    Remove-Item 'HKCU:\Software\LightSwitchLite' -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host 'LightSwitch Lite uninstalled.'
    return
}

foreach ($file in 'LightSwitchLite.exe', 'lsl.exe') {
    $source = Join-Path $dist $file
    if (-not (Test-Path $source)) { throw "Falta $source. Ejecuta build.ps1 primero." }
}

New-Item -ItemType Directory -Force -Path $target | Out-Null
Copy-Item (Join-Path $dist 'LightSwitchLite.exe') $target -Force
Copy-Item (Join-Path $dist 'lsl.exe') $target -Force

$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut($link)
$shortcut.TargetPath = $exe
$shortcut.WorkingDirectory = $target
$shortcut.Description = 'Automatic light/dark theme switching'
$shortcut.Save()

if ($StartWithWindows) {
    Set-ItemProperty -Path $runKey -Name 'LightSwitchLite' -Value ('"{0}" --background' -f $exe)
}

Get-Process LightSwitchLite -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 300
Start-Process -FilePath $exe

Write-Host "Installed to $target"
Write-Host "Shortcut: $link"
if ($StartWithWindows) { Write-Host 'Start with Windows enabled.' }
