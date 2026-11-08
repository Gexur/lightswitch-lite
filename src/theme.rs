//! System theme read/write and the shell notifications.
//!
//! The refresh sequence mirrors PowerToys Light Switch:
//!   * write each target independently,
//!   * after every write broadcast `WM_SETTINGCHANGE` + `WM_THEMECHANGED`,
//!   * and when switching the system theme to light also reset `ColorPrevalence`
//!     and broadcast `WM_DWMCOLORIZATIONCOLORCHANGED`.
//!
//! Doing the broadcast per target (instead of once at the end) is what makes
//! Windows repaint the taskbar on secondary monitors.

use std::ptr;

use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE,
    KEY_SET_VALUE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_DWMCOLORIZATIONCOLORCHANGED,
    WM_SETTINGCHANGE, WM_THEMECHANGED,
};

use crate::settings::{read_dword, write_dword};
use crate::wide;

const PERSONALIZE: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize";
const NIGHT_LIGHT: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\CloudStore\\Store\\DefaultAccount\\Current\\default$windows.data.bluelightreduction.bluelightreductionstate\\windows.data.bluelightreduction.bluelightreductionstate";

fn broadcast(message: u32, lparam: isize) {
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            message,
            0,
            lparam,
            SMTO_ABORTIFHUNG,
            5000,
            ptr::null_mut(),
        );
    }
}

fn notify_theme_change() {
    let section = wide("ImmersiveColorSet");
    broadcast(WM_SETTINGCHANGE, section.as_ptr() as isize);
    broadcast(WM_THEMECHANGED, 0);
}

fn notify_colorization() {
    broadcast(WM_DWMCOLORIZATIONCOLORCHANGED, 0);
}

pub fn reg_open_personalize(access: u32) -> Option<HKEY> {
    let path = wide(PERSONALIZE);
    let mut key: HKEY = ptr::null_mut();
    let status = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, path.as_ptr(), 0, access, &mut key) };
    if status == ERROR_SUCCESS {
        Some(key)
    } else {
        None
    }
}

pub fn reg_open_night_light(access: u32) -> Option<HKEY> {
    let path = wide(NIGHT_LIGHT);
    let mut key: HKEY = ptr::null_mut();
    let status = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, path.as_ptr(), 0, access, &mut key) };
    if status == ERROR_SUCCESS {
        Some(key)
    } else {
        None
    }
}

pub fn theme_read(system: bool) -> Option<bool> {
    let key = reg_open_personalize(KEY_QUERY_VALUE)?;
    let name = if system {
        "SystemUsesLightTheme"
    } else {
        "AppsUseLightTheme"
    };
    let value = read_dword(key, name);
    unsafe { RegCloseKey(key) };
    value.map(|v| v != 0)
}

/// Writes one target and broadcasts. Returns false only when the registry write
/// failed. `system` selects `SystemUsesLightTheme` instead of `AppsUseLightTheme`.
fn set_one(light: bool, system: bool) -> bool {
    let Some(key) = reg_open_personalize(KEY_SET_VALUE) else {
        return false;
    };

    let name = if system {
        "SystemUsesLightTheme"
    } else {
        "AppsUseLightTheme"
    };
    let ok = write_dword(key, name, u32::from(light));

    if ok && system && light {
        // Windows paints the accent color on the title bar again if this stays on.
        let _ = write_dword(key, "ColorPrevalence", 0);
    }

    unsafe { RegCloseKey(key) };

    if ok {
        notify_theme_change();
        if system && light {
            notify_colorization();
        }
    }
    ok
}

/// Applies the requested theme. `system_ok` / `apps_ok` are true when the
/// corresponding target was not requested.
pub fn theme_apply(light: bool, system: bool, apps: bool) -> (bool, bool) {
    let mut system_ok = true;
    let mut apps_ok = true;

    if system {
        system_ok = set_one(light, true);
    }
    if apps {
        apps_ok = set_one(light, false);
    }

    (system_ok, apps_ok)
}

pub fn night_light_read() -> Option<bool> {
    let key = reg_open_night_light(KEY_QUERY_VALUE)?;

    let mut data = [0u8; 1024];
    let mut bytes = data.len() as u32;
    let mut kind: u32 = 0;
    let name = wide("Data");
    let status = unsafe {
        RegQueryValueExW(
            key,
            name.as_ptr(),
            ptr::null(),
            &mut kind,
            data.as_mut_ptr(),
            &mut bytes,
        )
    };
    unsafe { RegCloseKey(key) };

    const REG_BINARY: u32 = 3;
    if status != ERROR_SUCCESS || kind != REG_BINARY || bytes < 25 {
        return None;
    }

    Some(data[23] == 0x10 && data[24] == 0x00)
}
