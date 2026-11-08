//! Settings in HKCU\Software\LightSwitchLite and the Run key.

use std::ptr;

use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows_sys::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_DWORD, REG_OPTION_NON_VOLATILE,
    REG_SZ,
};

use crate::{wide, Config, Mode};

const CFG_KEY: &str = "Software\\LightSwitchLite";
const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN_VALUE: &str = "LightSwitchLite";

fn open_key(path: &str, access: u32) -> Option<HKEY> {
    let path = wide(path);
    let mut key: HKEY = ptr::null_mut();
    let status = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, path.as_ptr(), 0, access, &mut key) };
    if status == ERROR_SUCCESS {
        Some(key)
    } else {
        None
    }
}

fn create_key(path: &str, access: u32) -> Option<HKEY> {
    let path = wide(path);
    let mut key: HKEY = ptr::null_mut();
    let status = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            path.as_ptr(),
            0,
            ptr::null(),
            REG_OPTION_NON_VOLATILE,
            access,
            ptr::null(),
            &mut key,
            ptr::null_mut(),
        )
    };
    if status == ERROR_SUCCESS {
        Some(key)
    } else {
        None
    }
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn read_dword(key: HKEY, name: &str) -> Option<u32> {
    let name = wide(name);
    let mut value: u32 = 0;
    let mut bytes: u32 = std::mem::size_of::<u32>() as u32;
    let mut kind: u32 = 0;
    let status = unsafe {
        RegQueryValueExW(
            key,
            name.as_ptr(),
            ptr::null(),
            &mut kind,
            &mut value as *mut u32 as *mut u8,
            &mut bytes,
        )
    };
    if status == ERROR_SUCCESS && kind == REG_DWORD && bytes == std::mem::size_of::<u32>() as u32 {
        Some(value)
    } else {
        None
    }
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn write_dword(key: HKEY, name: &str, value: u32) -> bool {
    let name = wide(name);
    let bytes = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegSetValueExW(
            key,
            name.as_ptr(),
            0,
            REG_DWORD,
            &value as *const u32 as *const u8,
            bytes,
        )
    };
    status == ERROR_SUCCESS
}

fn write_string(key: HKEY, name: &str, value: &str) -> bool {
    let name = wide(name);
    let value = wide(value);
    let bytes = (value.len() * std::mem::size_of::<u16>()) as u32;
    let status = unsafe {
        RegSetValueExW(
            key,
            name.as_ptr(),
            0,
            REG_SZ,
            value.as_ptr() as *const u8,
            bytes,
        )
    };
    status == ERROR_SUCCESS
}

fn read_string(key: HKEY, name: &str) -> Option<String> {
    let name = wide(name);
    let mut buf = [0u16; 64];
    let mut bytes = ((buf.len() - 1) * std::mem::size_of::<u16>()) as u32;
    let mut kind: u32 = 0;
    let status = unsafe {
        RegQueryValueExW(
            key,
            name.as_ptr(),
            ptr::null(),
            &mut kind,
            buf.as_mut_ptr() as *mut u8,
            &mut bytes,
        )
    };
    if status != ERROR_SUCCESS || kind != REG_SZ {
        return None;
    }
    Some(crate::from_wide(&buf))
}

fn clamp_minute(value: i32) -> i32 {
    value.rem_euclid(1440)
}

fn read_coord(key: HKEY, name: &str, limit: f64) -> Option<f64> {
    let text = read_string(key, name)?;
    let cleaned = text.trim().replace(',', ".");
    let value: f64 = cleaned.parse().ok()?;
    if !value.is_finite() || value < -limit || value > limit {
        return None;
    }
    Some(value)
}

pub fn cfg_load(cfg: &mut Config) -> bool {
    *cfg = Config::defaults();

    let Some(key) = open_key(CFG_KEY, KEY_QUERY_VALUE) else {
        return false;
    };

    if let Some(mode) = read_dword(key, "Mode") {
        cfg.mode = Mode::from_u32(mode);
    }
    if let Some(value) = read_dword(key, "LightMinutes") {
        cfg.light_min = clamp_minute(value as i32);
    }
    if let Some(value) = read_dword(key, "DarkMinutes") {
        cfg.dark_min = clamp_minute(value as i32);
    }
    cfg.target_system = read_dword(key, "TargetSystem").unwrap_or(1) != 0;
    cfg.target_apps = read_dword(key, "TargetApps").unwrap_or(1) != 0;

    if let Some(value) = read_coord(key, "Latitude", 90.0) {
        cfg.lat = value;
    }
    if let Some(value) = read_coord(key, "Longitude", 180.0) {
        cfg.lon = value;
    }

    unsafe { RegCloseKey(key) };
    true
}

pub fn cfg_save(cfg: &Config) -> bool {
    let Some(key) = create_key(CFG_KEY, KEY_SET_VALUE) else {
        return false;
    };

    let mut ok = true;
    ok &= write_dword(key, "Mode", cfg.mode.as_u32());
    ok &= write_dword(key, "LightMinutes", clamp_minute(cfg.light_min) as u32);
    ok &= write_dword(key, "DarkMinutes", clamp_minute(cfg.dark_min) as u32);
    ok &= write_dword(key, "TargetSystem", u32::from(cfg.target_system));
    ok &= write_dword(key, "TargetApps", u32::from(cfg.target_apps));
    ok &= write_string(key, "Latitude", &format!("{:.6}", cfg.lat));
    ok &= write_string(key, "Longitude", &format!("{:.6}", cfg.lon));

    unsafe { RegCloseKey(key) };
    ok
}

pub fn autostart_get() -> bool {
    let Some(key) = open_key(RUN_KEY, KEY_QUERY_VALUE) else {
        return false;
    };
    let name = wide(RUN_VALUE);
    let status = unsafe {
        RegQueryValueExW(
            key,
            name.as_ptr(),
            ptr::null(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
        )
    };
    unsafe { RegCloseKey(key) };
    status == ERROR_SUCCESS
}

pub fn autostart_set(on: bool) -> bool {
    let Some(key) = create_key(RUN_KEY, KEY_SET_VALUE) else {
        return false;
    };

    let ok = if on {
        let mut path = [0u16; 260];
        let len =
            unsafe { GetModuleFileNameW(ptr::null_mut(), path.as_mut_ptr(), path.len() as u32) };
        if len == 0 {
            unsafe { RegCloseKey(key) };
            return false;
        }
        let exe = crate::from_wide(&path);
        write_string(key, RUN_VALUE, &format!("\"{}\" --background", exe))
    } else {
        let name = wide(RUN_VALUE);
        let status = unsafe { RegDeleteValueW(key, name.as_ptr()) };
        status == ERROR_SUCCESS || status == ERROR_FILE_NOT_FOUND
    };

    unsafe { RegCloseKey(key) };
    ok
}
