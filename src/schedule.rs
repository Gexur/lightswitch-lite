//! Schedule, solar math and next-change computation.

use windows_sys::Win32::Foundation::{FILETIME, SYSTEMTIME};
use windows_sys::Win32::System::Time::{
    FileTimeToSystemTime, GetTimeZoneInformation, GetTimeZoneInformationForYear,
    SystemTimeToFileTime, SystemTimeToTzSpecificLocalTime, TIME_ZONE_ID_INVALID,
};

use crate::{Config, Mode};

const PI: f64 = std::f64::consts::PI;

fn to_rad(deg: f64) -> f64 {
    deg * PI / 180.0
}

fn to_deg(rad: f64) -> f64 {
    rad * 180.0 / PI
}

fn wrap360(mut value: f64) -> f64 {
    value %= 360.0;
    if value < 0.0 {
        value += 360.0;
    }
    value
}

pub fn wrap_min(value: i32) -> i32 {
    value.rem_euclid(1440)
}

fn julian_day(y: i32, m: i32, d: i32) -> f64 {
    let (mut y, mut m) = (y, m);
    if m <= 2 {
        y -= 1;
        m += 12;
    }
    let a = y / 100;
    let b = 2 - a + a / 4;
    (365.25 * (y + 4716) as f64).floor() + (30.6001 * (m + 1) as f64).floor() + d as f64 + b as f64
        - 1524.5
        + 0.5
}

fn filetime_to_u64(ft: &FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64
}

fn u64_to_filetime(value: u64) -> FILETIME {
    FILETIME {
        dwLowDateTime: (value & 0xFFFF_FFFF) as u32,
        dwHighDateTime: (value >> 32) as u32,
    }
}

/// Converts a UTC instant (minutes since midnight of day d) into local minutes,
/// honouring the daylight-saving rule in force on that specific date.
fn utc_minutes_to_local(y: i32, m: i32, d: i32, utc_min: f64) -> Option<i32> {
    let day_shift = (utc_min / 1440.0).floor() as i32;
    let rest = utc_min - day_shift as f64 * 1440.0;

    let mut hour = (rest / 60.0) as i32;
    let mut minute = (rest - hour as f64 * 60.0).round() as i32;
    if minute == 60 {
        minute = 0;
        hour += 1;
    }

    let mut extra_day = 0;
    if hour >= 24 {
        hour -= 24;
        extra_day = 1;
    }

    let utc = SYSTEMTIME {
        wYear: y as u16,
        wMonth: m as u16,
        wDay: d as u16,
        wDayOfWeek: 0,
        wHour: hour as u16,
        wMinute: minute as u16,
        wSecond: 0,
        wMilliseconds: 0,
    };

    let mut ft = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    if unsafe { SystemTimeToFileTime(&utc, &mut ft) } == 0 {
        return None;
    }

    let shifted_value =
        filetime_to_u64(&ft) + (day_shift as i64 + extra_day as i64) as u64 * 864_000_000_000;
    let shifted_ft = u64_to_filetime(shifted_value);

    let mut shifted = unsafe { std::mem::zeroed() };
    if unsafe { FileTimeToSystemTime(&shifted_ft, &mut shifted) } == 0 {
        return None;
    }

    let mut tz = unsafe { std::mem::zeroed() };
    let got_year =
        unsafe { GetTimeZoneInformationForYear(shifted.wYear, std::ptr::null(), &mut tz) } != 0;
    if !got_year {
        let result = unsafe { GetTimeZoneInformation(&mut tz) };
        if result == TIME_ZONE_ID_INVALID {
            return None;
        }
    }

    let mut local = unsafe { std::mem::zeroed() };
    if unsafe { SystemTimeToTzSpecificLocalTime(&tz, &shifted, &mut local) } == 0 {
        return None;
    }

    Some(local.wHour as i32 * 60 + local.wMinute as i32)
}

pub fn is_light_at(now_min: i32, light_min: i32, dark_min: i32) -> bool {
    let light = wrap_min(light_min);
    let dark = wrap_min(dark_min);
    let now = wrap_min(now_min);

    if light < dark {
        now >= light && now < dark
    } else {
        now >= light || now < dark
    }
}

/// Returns the local sunrise and sunset minutes, or `None` during midnight sun
/// or polar night.
pub fn sun_times(lat: f64, lon: f64, y: i32, m: i32, d: i32) -> Option<(i32, i32)> {
    let lat = lat.clamp(-89.5, 89.5);

    let n = julian_day(y, m, d) - 2451545.0;
    let mean_lon = wrap360(280.460 + 0.9856474 * n);
    let anomaly = to_rad(wrap360(357.528 + 0.9856003 * n));
    let ecliptic = to_rad(mean_lon + 1.915 * anomaly.sin() + 0.020 * (2.0 * anomaly).sin());
    let tilt = to_rad(23.439 - 0.0000004 * n);

    let right_asc = (tilt.cos() * ecliptic.sin()).atan2(ecliptic.cos());
    let decl = (tilt.sin() * ecliptic.sin()).asin();

    // Equation of time in minutes, clamped to the physical range (+-20 min).
    let mut equation = 4.0 * (mean_lon - to_deg(right_asc));
    while equation > 20.0 {
        equation -= 1440.0;
    }
    while equation < -20.0 {
        equation += 1440.0;
    }

    let lat_rad = to_rad(lat);
    let cos_hour =
        (to_rad(90.833).cos() - lat_rad.sin() * decl.sin()) / (lat_rad.cos() * decl.cos());
    if cos_hour > 1.0 || cos_hour < -1.0 {
        return None;
    }

    let hour_angle = to_deg(cos_hour.acos());
    let noon_utc = 720.0 - 4.0 * lon - equation;
    let rise_utc = noon_utc - 4.0 * hour_angle;
    let set_utc = noon_utc + 4.0 * hour_angle;

    let rise = utc_minutes_to_local(y, m, d, rise_utc)?;
    let set = utc_minutes_to_local(y, m, d, set_utc)?;

    Some((wrap_min(rise), wrap_min(set)))
}

pub fn eff_times(cfg: &Config, now: &SYSTEMTIME) -> (i32, i32) {
    if cfg.mode == Mode::Solar {
        if let Some((rise, set)) = sun_times(
            cfg.lat,
            cfg.lon,
            now.wYear as i32,
            now.wMonth as i32,
            now.wDay as i32,
        ) {
            return (wrap_min(rise), wrap_min(set));
        }
        // No sunrise/sunset today: fall back to a default schedule.
        return (8 * 60, 20 * 60);
    }

    (wrap_min(cfg.light_min), wrap_min(cfg.dark_min))
}

pub fn stamp_of(t: &SYSTEMTIME) -> u64 {
    let mut ft = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    if unsafe { SystemTimeToFileTime(t, &mut ft) } == 0 {
        return 0;
    }
    filetime_to_u64(&ft) / 600_000_000 // minutes
}

pub fn crossed_boundary(cfg: &Config, from_stamp: u64, now: &SYSTEMTIME) -> bool {
    let current = stamp_of(now);
    if from_stamp == 0 || current == 0 || current <= from_stamp {
        return false;
    }
    if current - from_stamp >= 1440 {
        return true;
    }

    let file_time = from_stamp * 600_000_000;
    let ft = u64_to_filetime(file_time);

    let mut previous = unsafe { std::mem::zeroed() };
    if unsafe { FileTimeToSystemTime(&ft, &mut previous) } == 0 {
        return true;
    }

    let (light_a, dark_a) = eff_times(cfg, &previous);
    let (light_b, dark_b) = eff_times(cfg, now);

    let was_light = is_light_at(
        previous.wHour as i32 * 60 + previous.wMinute as i32,
        light_a,
        dark_a,
    );
    let is_light = is_light_at(now.wHour as i32 * 60 + now.wMinute as i32, light_b, dark_b);
    was_light != is_light
}

/// Milliseconds until the next theme change. The boolean says whether the next
/// change goes to light.
pub fn ms_to_next_change(cfg: &Config, now: &SYSTEMTIME) -> (u32, bool) {
    if cfg.mode == Mode::Off || cfg.mode == Mode::NightLight {
        return (0, false);
    }

    let (light_min, dark_min) = eff_times(cfg, now);
    if light_min == dark_min {
        return (0, false); // degenerate schedule: nothing to program
    }

    let now_sec = now.wHour as i64 * 3600 + now.wMinute as i64 * 60 + now.wSecond as i64;

    let until = |target_min: i32| -> i64 {
        let target = target_min as i64 * 60;
        let mut diff = (target - now_sec) % 86400;
        if diff <= 0 {
            diff += 86400;
        }
        let mut ms = diff * 1000 - now.wMilliseconds as i64;
        if ms <= 0 {
            ms += 86_400_000;
        }
        ms
    };

    let to_light_ms = until(light_min);
    let to_dark_ms = until(dark_min);

    if to_light_ms <= to_dark_ms {
        (to_light_ms as u32, true)
    } else {
        (to_dark_ms as u32, false)
    }
}

pub fn local_now() -> SYSTEMTIME {
    let mut now = unsafe { std::mem::zeroed() };
    unsafe {
        windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut now);
    }
    now
}
