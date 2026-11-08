//! lsl.exe: command-line interface. A separate, tiny process.

use std::ptr;

use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, HWND, SYSTEMTIME};
use windows_sys::Win32::System::DataExchange::COPYDATASTRUCT;
use windows_sys::Win32::System::Memory::{
    MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, FILE_MAP_READ,
};
use windows_sys::Win32::System::Time::{FileTimeToSystemTime, SystemTimeToTzSpecificLocalTime};
use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, SendMessageW, WM_COPYDATA};

use lightswitch_lite::ipc::*;
use lightswitch_lite::{wide, APP_VERSION};

const EXIT_OK: i32 = 0;
const EXIT_FAILED: i32 = 1;
const EXIT_USAGE: i32 = 2;
const EXIT_NO_SERVICE: i32 = 3;

fn send_command(window: HWND, command: u32, argument: u32) -> bool {
    let request = LslIpc {
        magic: IPC_MAGIC,
        cmd: command,
        arg: argument,
    };
    let data = COPYDATASTRUCT {
        dwData: 0,
        cbData: std::mem::size_of::<LslIpc>() as u32,
        lpData: &request as *const LslIpc as *mut core::ffi::c_void,
    };
    unsafe {
        SendMessageW(
            window,
            WM_COPYDATA,
            0,
            &data as *const COPYDATASTRUCT as isize,
        ) == 1
    }
}

fn read_status() -> Option<LslStatus> {
    let name = wide(MAPPING);
    let mapping = unsafe { OpenFileMappingW(FILE_MAP_READ, 0, name.as_ptr()) };
    if mapping.is_null() {
        return None;
    }

    let view = unsafe {
        MapViewOfFile(
            mapping,
            FILE_MAP_READ,
            0,
            0,
            std::mem::size_of::<LslStatus>(),
        )
    };

    let mut result = None;
    if !view.Value.is_null() {
        let status = unsafe { ptr::read(view.Value as *const LslStatus) };
        if status.magic == STATUS_MAGIC && status.size == std::mem::size_of::<LslStatus>() as u32 {
            result = Some(status);
        }
        unsafe { UnmapViewOfFile(view) };
    }

    unsafe { CloseHandle(mapping) };
    result
}

fn theme_name(value: u32) -> &'static str {
    match value {
        0 => "dark",
        1 => "light",
        _ => "unknown",
    }
}

fn mode_name(mode: u32) -> &'static str {
    match mode {
        1 => "FixedHours",
        2 => "SunsetToSunrise",
        3 => "FollowNightLight",
        _ => "Off",
    }
}

fn parse_mode(value: &str) -> Option<u32> {
    let value = value.to_ascii_lowercase();
    match value.as_str() {
        "off" | "disabled" => Some(0),
        "fixed" | "fixed-hours" | "fixedhours" => Some(1),
        "solar" | "sunrise" | "sunset-to-sunrise" | "sunsettosunrise" => Some(2),
        "nightlight" | "follow-night-light" | "night-light" => Some(3),
        _ => None,
    }
}

fn print_time(epoch: u32) {
    if epoch == 0 {
        print!("none scheduled");
        return;
    }

    let value = epoch as u64 * 10_000_000 + 116_444_736_000_000_000;
    let utc_file = FILETIME {
        dwLowDateTime: (value & 0xFFFF_FFFF) as u32,
        dwHighDateTime: (value >> 32) as u32,
    };

    let mut utc: SYSTEMTIME = unsafe { std::mem::zeroed() };
    let mut local: SYSTEMTIME = unsafe { std::mem::zeroed() };
    if unsafe { FileTimeToSystemTime(&utc_file, &mut utc) } != 0
        && unsafe { SystemTimeToTzSpecificLocalTime(ptr::null(), &utc, &mut local) } != 0
    {
        print!("{:02}:{:02}", local.wHour, local.wMinute);
    } else {
        print!("unknown");
    }
}

fn print_json(status: &LslStatus) {
    println!(
        "{{\"version\":1,\"success\":true,\"state\":{{\"systemTheme\":\"{}\",\"appsTheme\":\"{}\",\
         \"changeSystem\":{},\"changeApps\":{},\"scheduleMode\":\"{}\",\"manualOverride\":{}}}}}",
        theme_name(status.system_light),
        theme_name(status.apps_light),
        status.change_system != 0,
        status.change_apps != 0,
        mode_name(status.mode),
        status.manual_override != 0
    );
}

fn print_human(status: &LslStatus) {
    println!("Mode: {}", mode_name(status.mode));
    println!("System theme: {}", theme_name(status.system_light));
    println!("Apps theme: {}", theme_name(status.apps_light));

    print!("Targets: ");
    if status.change_system != 0 && status.change_apps != 0 {
        println!("system and apps");
    } else if status.change_system != 0 {
        println!("system");
    } else if status.change_apps != 0 {
        println!("apps");
    } else {
        println!("none");
    }

    print!("Next change: ");
    print_time(status.next_change);
    println!();
    println!(
        "Manual override: {}",
        if status.manual_override != 0 {
            "yes"
        } else {
            "no"
        }
    );
}

fn usage() -> i32 {
    println!("LightSwitch Lite - control the Windows theme from a terminal\n");
    println!("  lsl status [--json]        current state");
    println!("  lsl light                  apply the light theme");
    println!("  lsl dark                   apply the dark theme");
    println!("  lsl toggle                 toggle the theme");
    println!("  lsl mode <mode>            off | fixed | solar | nightlight");
    println!("  lsl --version              print the version");
    println!("  lsl --help                 this help\n");
    println!("LightSwitchLite.exe must be running for this command to work.");
    EXIT_USAGE
}

fn main() {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();

    if args.is_empty() {
        std::process::exit(usage());
    }

    let command = &args[0];

    if command.eq_ignore_ascii_case("--help")
        || command.eq_ignore_ascii_case("-h")
        || command.eq_ignore_ascii_case("help")
    {
        usage();
        std::process::exit(EXIT_OK);
    }

    if command.eq_ignore_ascii_case("--version") || command.eq_ignore_ascii_case("-v") {
        println!("{}", APP_VERSION);
        std::process::exit(EXIT_OK);
    }

    let json = args
        .iter()
        .skip(1)
        .any(|arg| arg.eq_ignore_ascii_case("--json"));

    let mut action = 0u32;
    let mut argument = 0u32;
    let mut status_only = false;

    if command.eq_ignore_ascii_case("status") {
        status_only = true;
    } else if command.eq_ignore_ascii_case("light") {
        action = CMD_LIGHT;
    } else if command.eq_ignore_ascii_case("dark") {
        action = CMD_DARK;
    } else if command.eq_ignore_ascii_case("toggle") {
        action = CMD_TOGGLE;
    } else if command.eq_ignore_ascii_case("mode") {
        match args.get(1).and_then(|value| parse_mode(value)) {
            Some(mode) => argument = mode,
            None => {
                eprintln!("Invalid mode. Use off, fixed, solar or nightlight.");
                std::process::exit(EXIT_USAGE);
            }
        }
        action = CMD_MODE;
    } else {
        eprintln!("Unknown command: {}", command);
        std::process::exit(usage());
    }

    let class = wide(WNDCLASS);
    let window = unsafe { FindWindowW(class.as_ptr(), ptr::null()) };
    if window.is_null() {
        eprintln!("LightSwitch Lite is not running.");
        std::process::exit(EXIT_NO_SERVICE);
    }

    if status_only {
        send_command(window, CMD_REFRESH, 0);
        let Some(status) = read_status() else {
            eprintln!("Could not read the service state.");
            std::process::exit(EXIT_FAILED);
        };
        if json {
            print_json(&status);
        } else {
            print_human(&status);
        }
        std::process::exit(EXIT_OK);
    }

    if !send_command(window, action, argument) {
        eprintln!("The operation failed.");
        std::process::exit(EXIT_FAILED);
    }

    if json {
        println!("{{\"version\":1,\"success\":true}}");
    } else {
        println!("Applied.");
    }
    std::process::exit(EXIT_OK);
}
