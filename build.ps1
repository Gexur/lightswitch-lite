# Builds LightSwitch Lite and copies the two executables to dist\.
# Usage:  powershell -ExecutionPolicy Bypass -File .\build.ps1
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$dist = Join-Path $root 'dist'

Push-Location $root
try {
    cargo build --release
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed with exit code $LASTEXITCODE" }

    New-Item -ItemType Directory -Force -Path $dist | Out-Null
    Copy-Item (Join-Path $root 'target\release\LightSwitchLite.exe') $dist -Force
    Copy-Item (Join-Path $root 'target\release\lsl.exe') $dist -Force

    Get-ChildItem $dist -File | ForEach-Object { '{0,-22} {1,8:N0} bytes' -f $_.Name, $_.Length }
}
finally {
    Pop-Location
}
