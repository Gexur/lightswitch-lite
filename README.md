# LightSwitch Lite

Automatic light/dark theme switching for Windows. A single 240 KB executable
written in Rust, with no installer, no .NET and no dependencies.

![LightSwitch Lite settings](docs/settings-light.png)

## Features

- Fixed hours, or sunrise and sunset calculated locally from your coordinates.
- Or follow the Windows Night light state.
- Change the Windows UI and apps independently.
- Taskbars on every monitor stay in sync.
- Manual changes stay until the next scheduled switch.
- Tray icon, optional start with Windows, and a CLI.

## Install

Download `LightSwitchLite.exe` from Releases and run it. No administrator rights
needed.

Optional Start menu shortcut:

```powershell
powershell -ExecutionPolicy Bypass -File .\install.ps1
```

Settings live in `HKCU\Software\LightSwitchLite`.

## Build

Requires Rust with the MSVC target and the Windows SDK.

```powershell
cargo build --release
```

Or run `.\build.ps1`, which copies both executables to `dist\`.

## Command line

```powershell
lsl status [--json]
lsl light
lsl dark
lsl toggle
lsl mode off|fixed|solar|nightlight
```

Exit codes: `0` ok, `1` failure, `2` invalid arguments, `3` not running.

## Notes

- Changes the current user only.
- Solar mode uses the Windows time zone and the coordinates you enter.
- The app must be running for automatic changes.

MIT license. See [LICENSE](LICENSE).
