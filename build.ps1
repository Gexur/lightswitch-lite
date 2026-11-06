# Builds LightSwitch Lite with MSVC (no Visual Studio IDE, no CMake).
# Usage:  powershell -ExecutionPolicy Bypass -File build.ps1
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$obj = Join-Path $root 'obj'
$dist = Join-Path $root 'dist'
New-Item -ItemType Directory -Force -Path $obj, $dist | Out-Null

$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (-not (Test-Path $vswhere)) { throw 'vswhere.exe not found (install Visual Studio Build Tools).' }

$vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vs) { throw 'No C++ tools found (install the "Desktop development with C++" workload).' }

$vcvars = Join-Path $vs 'VC\Auxiliary\Build\vcvars64.bat'
if (-not (Test-Path $vcvars)) { throw "Falta $vcvars" }

$flags = '/nologo /c /O2 /GL /MT /W4 /WX /permissive- /std:c++20 /utf-8 /DUNICODE /D_UNICODE /D_WIN32_WINNT=0x0A00 /DWINVER=0x0A00'

$script = @"
@echo off
call "$vcvars" >nul || exit /b 1
cd /d "$obj" || exit /b 1
rc /nologo /fo app.res "$root\res\app.rc" || exit /b 1
cl $flags /I "$root\src" "$root\src\settings.cpp" "$root\src\theme.cpp" "$root\src\schedule.cpp" "$root\src\main.cpp" "$root\src\ui.cpp" || exit /b 1
link /nologo /LTCG /OPT:REF /OPT:ICF /SUBSYSTEM:WINDOWS /OUT:"$dist\LightSwitchLite.exe" settings.obj theme.obj schedule.obj main.obj ui.obj app.res user32.lib gdi32.lib shell32.lib advapi32.lib dwmapi.lib uxtheme.lib || exit /b 1
cl $flags /I "$root\src" "$root\src\cli.cpp" || exit /b 1
link /nologo /LTCG /OPT:REF /OPT:ICF /SUBSYSTEM:CONSOLE /OUT:"$dist\lsl.exe" cli.obj user32.lib || exit /b 1
"@

$scriptPath = Join-Path $obj 'build.cmd'
Set-Content -Path $scriptPath -Value $script -Encoding ASCII

& cmd.exe /c "`"$scriptPath`""
if ($LASTEXITCODE -ne 0) { throw "Build failed with exit code $LASTEXITCODE" }

Get-ChildItem $dist -File | ForEach-Object { '{0,-22} {1,8:N0} bytes' -f $_.Name, $_.Length }
