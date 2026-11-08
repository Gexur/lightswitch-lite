//! IPC contract between LightSwitchLite.exe and lsl.exe.
//!
//! Kept byte-compatible with the original C++ implementation.

pub const WNDCLASS: &str = "LSL.MainWindow";
pub const SETTINGS_WNDCLASS: &str = "LSL.SettingsWindow";
pub const MAPPING: &str = "Local\\LSL.Status";
pub const MUTEX: &str = "Local\\LSL.SingleInstance";
pub const IPC_MAGIC: u32 = 0x4C53_4C31; // "LSL1"
pub const STATUS_MAGIC: u32 = 0x4C53_5431; // "LST1"

pub const CMD_PING: u32 = 0;
pub const CMD_SHOW: u32 = 1;
pub const CMD_LIGHT: u32 = 2;
pub const CMD_DARK: u32 = 3;
pub const CMD_TOGGLE: u32 = 4;
pub const CMD_MODE: u32 = 5;
pub const CMD_REFRESH: u32 = 6;
pub const CMD_QUIT: u32 = 7;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LslIpc {
    pub magic: u32,
    pub cmd: u32,
    pub arg: u32,
}

/// Published by the app, read by the CLI.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LslStatus {
    pub magic: u32,
    pub size: u32,
    pub version: u32,
    pub pid: u32,
    pub mode: u32,
    /// 0 dark, 1 light, 2 unknown.
    pub system_light: u32,
    pub apps_light: u32,
    pub manual_override: u32,
    pub change_system: u32,
    pub change_apps: u32,
    /// 0 off, 1 on, 2 unknown.
    pub night_light: u32,
    /// Seconds since the Unix epoch (UTC), 0 = none.
    pub next_change: u32,
}
