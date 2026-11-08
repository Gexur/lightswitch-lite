//! LightSwitch Lite - shared core.
//!
//! Direct Rust port of the original Win32 implementation. No runtime, no
//! service, no JSON crate: plain Win32 calls and a hand-written CLI.

pub mod app;
pub mod ipc;
pub mod schedule;
pub mod settings;
pub mod theme;
pub mod ui;

use std::cell::UnsafeCell;
use std::ptr;

pub const APP_NAME: &str = "LightSwitch Lite";
pub const APP_VERSION: &str = "1.0.0";

/// Single-threaded global. The message loop and every window procedure run on
/// the same thread, so an `UnsafeCell` avoids the borrow checker noise while
/// keeping the same ownership model as the original C++ globals.
pub struct Global<T>(UnsafeCell<T>);

unsafe impl<T> Sync for Global<T> {}

impl<T> Global<T> {
    pub const fn new(value: T) -> Self {
        Self(UnsafeCell::new(value))
    }

    #[allow(clippy::mut_from_ref)]
    pub fn get(&self) -> &mut T {
        unsafe { &mut *self.0.get() }
    }
}

/// Schedule mode. Values are shared with lsl.exe: do not renumber.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Off = 0,
    Fixed = 1,
    Solar = 2,
    NightLight = 3,
}

impl Mode {
    pub fn from_u32(value: u32) -> Mode {
        match value {
            1 => Mode::Fixed,
            2 => Mode::Solar,
            3 => Mode::NightLight,
            _ => Mode::Off,
        }
    }

    pub fn as_u32(self) -> u32 {
        self as u32
    }

    pub fn name(self) -> &'static str {
        match self {
            Mode::Fixed => "FixedHours",
            Mode::Solar => "SunsetToSunrise",
            Mode::NightLight => "FollowNightLight",
            Mode::Off => "Off",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub mode: Mode,
    /// Minute of day when the light theme starts.
    pub light_min: i32,
    /// Minute of day when the dark theme starts.
    pub dark_min: i32,
    pub lat: f64,
    pub lon: f64,
    pub target_system: bool,
    pub target_apps: bool,
}

impl Config {
    pub const fn defaults() -> Config {
        Config {
            mode: Mode::Fixed,
            light_min: 8 * 60,
            dark_min: 20 * 60,
            lat: 0.0,
            lon: 0.0,
            target_system: true,
            target_apps: true,
        }
    }

    pub fn same_as(&self, other: &Config) -> bool {
        self.mode == other.mode
            && self.light_min == other.light_min
            && self.dark_min == other.dark_min
            && self.lat == other.lat
            && self.lon == other.lon
            && self.target_system == other.target_system
            && self.target_apps == other.target_apps
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::defaults()
    }
}

/// NUL-terminated UTF-16 buffer.
pub fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Leaks a UTF-16 string once. Used for window class names and other values
/// that Win32 keeps for the lifetime of the process.
pub fn wide_static(value: &str) -> *const u16 {
    Box::leak(wide(value).into_boxed_slice()).as_ptr()
}

pub fn null_handle<T>() -> *mut T {
    ptr::null_mut()
}

/// `MAKEINTRESOURCEW(id)`: the integer lives in the pointer value itself, so it
/// is not a real (dangling) allocation.
#[allow(clippy::manual_dangling_ptr)]
pub fn int_resource(id: u16) -> *const u16 {
    id as usize as *const u16
}

/// Copies a Rust string into a fixed-size Win32 character array, truncating if
/// needed and always leaving room for the terminator.
pub fn copy_wide(dst: &mut [u16], src: &str) {
    if dst.is_empty() {
        return;
    }
    let mut used = 0;
    for unit in src.encode_utf16() {
        if used + 1 >= dst.len() {
            break;
        }
        dst[used] = unit;
        used += 1;
    }
    dst[used] = 0;
    for slot in &mut dst[used + 1..] {
        *slot = 0;
    }
}

/// Reads a NUL-terminated UTF-16 buffer into a String.
pub fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}
