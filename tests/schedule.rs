use lightswitch_lite::schedule::{
    crossed_boundary, is_light_at, ms_to_next_change, stamp_of, sun_times,
};
use lightswitch_lite::{Config, Mode};
use windows_sys::Win32::Foundation::SYSTEMTIME;

fn system_time(y: u16, mo: u16, d: u16, h: u16, mi: u16) -> SYSTEMTIME {
    SYSTEMTIME {
        wYear: y,
        wMonth: mo,
        wDayOfWeek: 0,
        wDay: d,
        wHour: h,
        wMinute: mi,
        wSecond: 0,
        wMilliseconds: 0,
    }
}

#[test]
fn fixed_ranges() {
    assert!(is_light_at(8 * 60, 8 * 60, 20 * 60));
    assert!(is_light_at(12 * 60, 8 * 60, 20 * 60));
    assert!(!is_light_at(20 * 60, 8 * 60, 20 * 60));
    assert!(!is_light_at(7 * 60 + 59, 8 * 60, 20 * 60));

    assert!(is_light_at(22 * 60, 20 * 60, 8 * 60));
    assert!(is_light_at(2 * 60, 20 * 60, 8 * 60));
    assert!(!is_light_at(9 * 60, 20 * 60, 8 * 60));
}

#[test]
fn buenos_aires_solar_times() {
    let (rise, set) = sun_times(-34.6037, -58.3816, 2026, 11, 5).expect("solar times");
    assert!(
        (rise - (5 * 60 + 46)).abs() <= 5,
        "sunrise was {} ({:02}:{:02})",
        rise,
        rise / 60,
        rise % 60
    );
    assert!(
        (set - (19 * 60 + 31)).abs() <= 5,
        "sunset was {} ({:02}:{:02})",
        set,
        set / 60,
        set % 60
    );
}

#[test]
fn next_change_is_light_at_0800() {
    let now = system_time(2026, 11, 5, 21, 5);
    let cfg = Config {
        mode: Mode::Fixed,
        light_min: 8 * 60,
        dark_min: 20 * 60,
        ..Config::defaults()
    };

    let (ms, to_light) = ms_to_next_change(&cfg, &now);
    assert!(to_light);
    assert_eq!(ms, 10 * 3600 * 1000 + 55 * 60 * 1000);
}

#[test]
fn boundary_crossing() {
    let before = system_time(2026, 11, 5, 19, 55);
    let after = system_time(2026, 11, 5, 20, 5);
    let cfg = Config {
        mode: Mode::Fixed,
        light_min: 8 * 60,
        dark_min: 20 * 60,
        ..Config::defaults()
    };

    assert!(crossed_boundary(&cfg, stamp_of(&before), &after));
    assert!(!crossed_boundary(&cfg, stamp_of(&after), &after));
}
