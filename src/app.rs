//! Tray icon, scheduling, registry notifications and the message loop.

use std::mem::size_of;
use std::ptr;

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, ERROR_SUCCESS, FILETIME, HANDLE, HINSTANCE,
    HWND, INVALID_HANDLE_VALUE, LPARAM, LRESULT, SYSTEMTIME, WAIT_OBJECT_0, WPARAM,
};
use windows_sys::Win32::System::DataExchange::COPYDATASTRUCT;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Memory::{
    CreateFileMappingW, MapViewOfFile, SetProcessWorkingSetSizeEx, UnmapViewOfFile, FILE_MAP_WRITE,
    PAGE_READWRITE,
};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegNotifyChangeKeyValue, HKEY, KEY_NOTIFY, KEY_QUERY_VALUE,
    REG_NOTIFY_CHANGE_LAST_SET,
};
use windows_sys::Win32::System::SystemInformation::GetSystemTimeAsFileTime;
use windows_sys::Win32::System::Threading::{
    CreateEventW, CreateMutexW, GetCurrentProcess, GetCurrentProcessId, ReleaseMutex, ResetEvent,
    INFINITE,
};
use windows_sys::Win32::System::Time::{FileTimeToSystemTime, SystemTimeToFileTime};
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyMenu,
    DestroyWindow, DispatchMessageW, FindWindowW, GetCursorPos, GetSystemMetrics, KillTimer,
    LoadCursorW, LoadImageW, MsgWaitForMultipleObjectsEx, PeekMessageW, PostMessageW,
    PostQuitMessage, RegisterClassExW, RegisterWindowMessageW, SendMessageW, SetForegroundWindow,
    SetTimer, TrackPopupMenu, TranslateMessage, HICON, IDC_ARROW, IMAGE_ICON, LR_DEFAULTSIZE,
    MF_CHECKED, MF_POPUP, MF_SEPARATOR, MF_STRING, MSG, MWMO_INPUTAVAILABLE,
    PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND, PM_REMOVE, QS_ALLINPUT, SM_CXSMICON, SM_CYSMICON,
    TPM_BOTTOMALIGN, TPM_RIGHTBUTTON, WM_CLOSE, WM_COMMAND, WM_CONTEXTMENU, WM_COPYDATA,
    WM_DESTROY, WM_ENDSESSION, WM_LBUTTONDBLCLK, WM_NULL, WM_POWERBROADCAST, WM_QUERYENDSESSION,
    WM_QUIT, WM_RBUTTONUP, WM_TIMECHANGE, WM_TIMER, WNDCLASSEXW,
};

use crate::ipc::*;
use crate::schedule::{
    crossed_boundary, eff_times, is_light_at, local_now, ms_to_next_change, stamp_of,
};
use crate::settings::{autostart_get, autostart_set, cfg_load, cfg_save};
use crate::theme::{
    night_light_read, reg_open_night_light, reg_open_personalize, theme_apply, theme_read,
};
use crate::ui;
use crate::{copy_wide, wide, wide_static, Config, Global, Mode, APP_NAME};

const TRAY_MESSAGE: u32 = 0x8000 + 10; // WM_APP + 10
const SHOW_MESSAGE: u32 = 0x8000 + 11; // WM_APP + 11
const TRAY_ID: u32 = 1;
const TIMER_ID: usize = 1;

#[derive(Clone, Copy)]
pub struct Launch {
    pub background: bool,
    pub command: u32,
    pub mode: Option<Mode>,
}

impl Launch {
    pub fn new() -> Launch {
        Launch {
            background: false,
            command: CMD_SHOW,
            mode: None,
        }
    }
}

impl Default for Launch {
    fn default() -> Self {
        Self::new()
    }
}

pub fn parse_launch() -> Launch {
    let mut launch = Launch::new();
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if arg.eq_ignore_ascii_case("--background") || arg.eq_ignore_ascii_case("--minimized") {
            launch.background = true;
        } else if arg.eq_ignore_ascii_case("--light") {
            launch.command = CMD_LIGHT;
        } else if arg.eq_ignore_ascii_case("--dark") {
            launch.command = CMD_DARK;
        } else if arg.eq_ignore_ascii_case("--toggle") {
            launch.command = CMD_TOGGLE;
        } else if arg.eq_ignore_ascii_case("--mode") && i + 1 < args.len() {
            i += 1;
            let name = &args[i];
            if name.eq_ignore_ascii_case("off") {
                launch.mode = Some(Mode::Off);
            } else if name.eq_ignore_ascii_case("fixed") {
                launch.mode = Some(Mode::Fixed);
            } else if name.eq_ignore_ascii_case("solar") || name.eq_ignore_ascii_case("sunrise") {
                launch.mode = Some(Mode::Solar);
            } else if name.eq_ignore_ascii_case("nightlight") {
                launch.mode = Some(Mode::NightLight);
            }
        }
        i += 1;
    }

    launch
}

pub struct AppState {
    pub instance: HINSTANCE,
    pub main: HWND,
    pub single_mutex: HANDLE,
    pub mapping: HANDLE,
    pub status: *mut LslStatus,
    pub cfg: Config,
    pub override_: bool,
    pub has_night: bool,
    pub night_on: bool,
    pub has_eval: bool,
    pub eval_stamp: u64,
    pub has_obs_sys: bool,
    pub obs_sys: bool,
    pub has_obs_apps: bool,
    pub obs_apps: bool,
    pub pers_key: HKEY,
    pub pers_event: HANDLE,
    pub pers_armed: bool,
    pub night_key: HKEY,
    pub night_event: HANDLE,
    pub night_armed: bool,
    pub taskbar_created: u32,
    pub tray_added: bool,
    pub icon_small: HICON,
    pub icon_large: HICON,
}

impl AppState {
    const fn new() -> AppState {
        AppState {
            instance: ptr::null_mut(),
            main: ptr::null_mut(),
            single_mutex: ptr::null_mut(),
            mapping: ptr::null_mut(),
            status: ptr::null_mut(),
            cfg: Config::defaults(),
            override_: false,
            has_night: false,
            night_on: false,
            has_eval: false,
            eval_stamp: 0,
            has_obs_sys: false,
            obs_sys: false,
            has_obs_apps: false,
            obs_apps: false,
            pers_key: ptr::null_mut(),
            pers_event: ptr::null_mut(),
            pers_armed: false,
            night_key: ptr::null_mut(),
            night_event: ptr::null_mut(),
            night_armed: false,
            taskbar_created: 0,
            tray_added: false,
            icon_small: ptr::null_mut(),
            icon_large: ptr::null_mut(),
        }
    }
}

pub static APP: Global<AppState> = Global::new(AppState::new());

pub fn app() -> &'static mut AppState {
    APP.get()
}

fn zero<T>() -> T {
    unsafe { std::mem::zeroed() }
}

fn filetime_to_u64(ft: &FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64
}

fn unix_now_plus(ms: u32) -> u32 {
    let mut ft: FILETIME = zero();
    unsafe { GetSystemTimeAsFileTime(&mut ft) };
    let value = filetime_to_u64(&ft);

    const EPOCH: u64 = 116_444_736_000_000_000;
    if value < EPOCH {
        return 0;
    }
    let seconds = (value - EPOCH) / 10_000_000 + (ms as u64).div_ceil(1000);
    seconds as u32
}

fn publish() {
    let state = app();
    if state.status.is_null() {
        return;
    }

    unsafe {
        let status = &mut *state.status;
        status.magic = STATUS_MAGIC;
        status.size = size_of::<LslStatus>() as u32;
        status.version = 1;
        status.pid = GetCurrentProcessId();
        status.mode = state.cfg.mode.as_u32();
        status.change_system = u32::from(state.cfg.target_system);
        status.change_apps = u32::from(state.cfg.target_apps);
        status.manual_override = u32::from(state.override_);
        status.night_light = if state.has_night {
            u32::from(state.night_on)
        } else {
            2
        };

        status.system_light = match theme_read(true) {
            Some(light) => u32::from(light),
            None => 2,
        };
        status.apps_light = match theme_read(false) {
            Some(light) => u32::from(light),
            None => 2,
        };

        let now = local_now();
        let (ms, _) = ms_to_next_change(&state.cfg, &now);
        status.next_change = if ms != 0 { unix_now_plus(ms) } else { 0 };
    }
}

fn record_observations() {
    let state = app();
    match theme_read(true) {
        Some(value) => {
            state.has_obs_sys = true;
            state.obs_sys = value;
        }
        None => state.has_obs_sys = false,
    }
    match theme_read(false) {
        Some(value) => {
            state.has_obs_apps = true;
            state.obs_apps = value;
        }
        None => state.has_obs_apps = false,
    }
}

fn update_tray_tip() {
    let state = app();
    if !state.tray_added {
        return;
    }

    let tip = match theme_read(true) {
        Some(light) => format!(
            "{} - now {}",
            APP_NAME,
            if light { "light" } else { "dark" }
        ),
        None => format!("{} - theme unknown", APP_NAME),
    };

    let mut data: NOTIFYICONDATAW = zero();
    data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = state.main;
    data.uID = TRAY_ID;
    data.uFlags = NIF_TIP;
    copy_wide(&mut data.szTip, &tip);
    unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) };
}

fn add_tray_icon() {
    let state = app();

    let mut data: NOTIFYICONDATAW = zero();
    data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = state.main;
    data.uID = TRAY_ID;
    data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    data.uCallbackMessage = TRAY_MESSAGE;
    data.hIcon = state.icon_small;
    copy_wide(&mut data.szTip, APP_NAME);

    unsafe {
        Shell_NotifyIconW(
            if state.tray_added {
                NIM_MODIFY
            } else {
                NIM_ADD
            },
            &data,
        )
    };
    state.tray_added = true;
    update_tray_tip();
}

fn remove_tray_icon() {
    let state = app();
    if !state.tray_added {
        return;
    }

    let mut data: NOTIFYICONDATAW = zero();
    data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = state.main;
    data.uID = TRAY_ID;
    unsafe { Shell_NotifyIconW(NIM_DELETE, &data) };
    state.tray_added = false;
}

fn target_wants_light(now: &SYSTEMTIME) -> bool {
    let state = app();
    if state.cfg.mode == Mode::NightLight {
        return if state.has_night {
            !state.night_on
        } else {
            true
        };
    }

    let (light_min, dark_min) = eff_times(&state.cfg, now);
    is_light_at(
        now.wHour as i32 * 60 + now.wMinute as i32,
        light_min,
        dark_min,
    )
}

fn apply_theme_now(light: bool) -> bool {
    let state = app();
    if !state.cfg.target_system && !state.cfg.target_apps {
        return true;
    }

    let need_system =
        state.cfg.target_system && theme_read(true).is_none_or(|current| current != light);
    let need_apps =
        state.cfg.target_apps && theme_read(false).is_none_or(|current| current != light);

    if !need_system && !need_apps {
        record_observations();
        return true;
    }

    let (system_ok, apps_ok) = theme_apply(light, need_system, need_apps);
    record_observations();
    system_ok && apps_ok
}

fn reset_timer() {
    let state = app();
    unsafe { KillTimer(state.main, TIMER_ID) };

    if state.cfg.mode == Mode::Off {
        return;
    }

    if state.cfg.mode == Mode::NightLight {
        // Without a reliable registry notification, poll slowly so state is not lost.
        if !state.night_armed {
            unsafe { SetTimer(state.main, TIMER_ID, 60_000, None) };
        }
        return;
    }

    let now = local_now();
    let (mut ms, _) = ms_to_next_change(&state.cfg, &now);
    if ms == 0 {
        return;
    }

    if state.cfg.mode == Mode::Solar {
        let seconds_left =
            86400 - (now.wHour as i64 * 3600 + now.wMinute as i64 * 60 + now.wSecond as i64);
        let to_midnight = (seconds_left * 1000 - now.wMilliseconds as i64) as u32;
        if to_midnight < ms {
            ms = to_midnight; // sun times change with the date
        }
    }

    if ms < 200 {
        ms = 200;
    }
    unsafe { SetTimer(state.main, TIMER_ID, ms + 200, None) };
}

fn arm_personalize() {
    let state = app();
    if state.pers_key.is_null() {
        state.pers_key =
            reg_open_personalize(KEY_NOTIFY | KEY_QUERY_VALUE).unwrap_or(ptr::null_mut());
    }
    if state.pers_key.is_null() {
        return;
    }
    if state.pers_event.is_null() {
        state.pers_event = unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) };
    }
    if state.pers_event.is_null() {
        return;
    }

    unsafe {
        ResetEvent(state.pers_event);
        state.pers_armed = RegNotifyChangeKeyValue(
            state.pers_key,
            0,
            REG_NOTIFY_CHANGE_LAST_SET,
            state.pers_event,
            1,
        ) == ERROR_SUCCESS;
    }
}

fn arm_night_light() {
    let state = app();
    if state.night_key.is_null() {
        state.night_key =
            reg_open_night_light(KEY_NOTIFY | KEY_QUERY_VALUE).unwrap_or(ptr::null_mut());
    }
    if state.night_key.is_null() {
        return;
    }
    if state.night_event.is_null() {
        state.night_event = unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) };
    }
    if state.night_event.is_null() {
        return;
    }

    unsafe {
        ResetEvent(state.night_event);
        state.night_armed = RegNotifyChangeKeyValue(
            state.night_key,
            0,
            REG_NOTIFY_CHANGE_LAST_SET,
            state.night_event,
            1,
        ) == ERROR_SUCCESS;
    }
}

fn configure_night_light() {
    let state = app();
    if state.cfg.mode == Mode::NightLight {
        arm_night_light();
        return;
    }

    state.night_armed = false;
    if !state.night_key.is_null() {
        unsafe { RegCloseKey(state.night_key) };
        state.night_key = ptr::null_mut();
    }
}

fn evaluate() {
    let state = app();
    let now = local_now();

    if state.cfg.mode == Mode::Off {
        state.has_eval = true;
        state.eval_stamp = stamp_of(&now);
        publish();
        return;
    }

    if state.cfg.mode == Mode::NightLight {
        if let Some(on) = night_light_read() {
            if !state.has_night || state.night_on != on {
                state.has_night = true;
                state.night_on = on;
                state.override_ = false;
            }
            if !state.override_ {
                apply_theme_now(!on);
            }
        }
        state.has_eval = true;
        state.eval_stamp = stamp_of(&now);
        publish();
        return;
    }

    if state.override_ {
        if state.has_eval && crossed_boundary(&state.cfg, state.eval_stamp, &now) {
            state.override_ = false;
        } else {
            state.eval_stamp = stamp_of(&now);
            state.has_eval = true;
            publish();
            return;
        }
    }

    apply_theme_now(target_wants_light(&now));
    state.has_eval = true;
    state.eval_stamp = stamp_of(&now);
    publish();
}

fn set_theme_manual(light: bool) -> bool {
    let state = app();
    if !state.cfg.target_system && !state.cfg.target_apps {
        return false;
    }

    let now = local_now();
    let ok = apply_theme_now(light);
    if state.cfg.mode != Mode::Off {
        state.override_ = light != target_wants_light(&now);
    }
    state.eval_stamp = stamp_of(&now);
    state.has_eval = true;

    reset_timer();
    publish();
    update_tray_tip();
    ui::ui_theme_changed();
    ok
}

fn toggle_theme() -> bool {
    let state = app();
    let value = if state.cfg.target_system {
        theme_read(true)
    } else if state.cfg.target_apps {
        theme_read(false)
    } else {
        None
    };

    match value {
        Some(light) => set_theme_manual(!light),
        None => false,
    }
}

fn set_mode(mode: Mode) -> bool {
    let state = app();
    state.cfg.mode = mode;
    cfg_save(&state.cfg);
    state.override_ = false;
    configure_night_light();
    evaluate();
    reset_timer();
    update_tray_tip();
    ui::ui_refresh();
    true
}

fn on_personalize_changed() {
    let state = app();
    let has_system = theme_read(true);
    let has_apps = theme_read(false);
    if has_system.is_none() && has_apps.is_none() {
        return;
    }

    if state.cfg.mode != Mode::Off && !state.override_ {
        if state.cfg.mode == Mode::NightLight && !state.has_night {
            record_observations();
            publish();
            return;
        }

        let now = local_now();
        let want = target_wants_light(&now);

        let mismatch = (state.cfg.target_system && has_system.is_some_and(|v| v != want))
            || (state.cfg.target_apps && has_apps.is_some_and(|v| v != want));
        if mismatch {
            // The user changed the theme by hand: honour it until the next boundary.
            state.override_ = true;
            state.has_eval = true;
            state.eval_stamp = stamp_of(&now);
        }
    }

    record_observations();
    publish();
    update_tray_tip();
    ui::ui_theme_changed();
}

fn on_night_light_changed() {
    let state = app();
    let Some(on) = night_light_read() else {
        publish();
        return;
    };

    let changed = !state.has_night || state.night_on != on;
    state.has_night = true;
    state.night_on = on;

    if state.cfg.mode == Mode::NightLight && changed {
        state.override_ = false;
        apply_theme_now(!on);
        let now = local_now();
        state.has_eval = true;
        state.eval_stamp = stamp_of(&now);
        reset_timer();
    }

    publish();
    update_tray_tip();
    ui::ui_theme_changed();
}

fn show_tray_menu(owner: HWND) {
    unsafe {
        let menu = CreatePopupMenu();
        let modes = CreatePopupMenu();
        if menu.is_null() || modes.is_null() {
            if !modes.is_null() {
                DestroyMenu(modes);
            }
            if !menu.is_null() {
                DestroyMenu(menu);
            }
            return;
        }

        let entries = [
            (Mode::Off, "Off"),
            (Mode::Fixed, "Fixed hours"),
            (Mode::Solar, "Sunrise and sunset"),
            (Mode::NightLight, "Night light"),
        ];
        for (index, (mode, label)) in entries.iter().enumerate() {
            let mut flags = MF_STRING;
            if app().cfg.mode == *mode {
                flags |= MF_CHECKED;
            }
            let text = wide(label);
            AppendMenuW(modes, flags, 100 + index, text.as_ptr());
        }

        let schedule = wide("Schedule");
        AppendMenuW(menu, MF_POPUP, modes as usize, schedule.as_ptr());
        AppendMenuW(menu, MF_SEPARATOR, 0, ptr::null());
        let light = wide("Light theme");
        AppendMenuW(menu, MF_STRING, 110, light.as_ptr());
        let dark = wide("Dark theme");
        AppendMenuW(menu, MF_STRING, 111, dark.as_ptr());
        let toggle = wide("Toggle theme");
        AppendMenuW(menu, MF_STRING, 112, toggle.as_ptr());
        AppendMenuW(menu, MF_SEPARATOR, 0, ptr::null());
        let settings = wide("Settings...");
        AppendMenuW(menu, MF_STRING, 120, settings.as_ptr());
        let autostart = wide("Start with Windows");
        let mut flags = MF_STRING;
        if autostart_get() {
            flags |= MF_CHECKED;
        }
        AppendMenuW(menu, flags, 121, autostart.as_ptr());
        AppendMenuW(menu, MF_SEPARATOR, 0, ptr::null());
        let exit = wide("Exit");
        AppendMenuW(menu, MF_STRING, 122, exit.as_ptr());

        let mut cursor = zero();
        GetCursorPos(&mut cursor);
        SetForegroundWindow(owner);
        TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
            cursor.x,
            cursor.y,
            0,
            owner,
            ptr::null(),
        );
        PostMessageW(owner, WM_NULL, 0, 0);
        DestroyMenu(menu);
    }
}

fn handle_command(request: &LslIpc) -> LRESULT {
    match request.cmd {
        CMD_PING | CMD_REFRESH => {
            publish();
            1
        }
        CMD_SHOW => {
            unsafe { PostMessageW(app().main, SHOW_MESSAGE, 0, 0) };
            1
        }
        CMD_LIGHT => {
            if set_theme_manual(true) {
                1
            } else {
                0
            }
        }
        CMD_DARK => {
            if set_theme_manual(false) {
                1
            } else {
                0
            }
        }
        CMD_TOGGLE => {
            if toggle_theme() {
                1
            } else {
                0
            }
        }
        CMD_MODE => {
            if request.arg > 3 {
                0
            } else {
                if set_mode(Mode::from_u32(request.arg)) {
                    1
                } else {
                    0
                }
            }
        }
        CMD_QUIT => {
            unsafe { PostQuitMessage(0) };
            1
        }
        _ => 0,
    }
}

unsafe extern "system" fn main_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let state = app();
    if msg == state.taskbar_created && state.taskbar_created != 0 {
        state.tray_added = false;
        add_tray_icon();
        return 0;
    }

    match msg {
        TRAY_MESSAGE => {
            match (lparam as u32) & 0xFFFF {
                WM_RBUTTONUP | WM_CONTEXTMENU => show_tray_menu(hwnd),
                WM_LBUTTONDBLCLK => ui::ui_show(hwnd),
                _ => {}
            }
            0
        }

        SHOW_MESSAGE => {
            ui::ui_show(hwnd);
            0
        }

        WM_TIMER => {
            if wparam == TIMER_ID {
                evaluate();
                reset_timer();
            }
            0
        }

        WM_POWERBROADCAST => {
            if wparam as u32 == PBT_APMRESUMEAUTOMATIC || wparam as u32 == PBT_APMRESUMESUSPEND {
                arm_personalize();
                if state.cfg.mode == Mode::NightLight {
                    arm_night_light();
                }
                evaluate();
                reset_timer();
            }
            1
        }

        WM_TIMECHANGE => {
            evaluate();
            reset_timer();
            0
        }

        WM_COPYDATA => {
            let copy = lparam as *const COPYDATASTRUCT;
            if !copy.is_null()
                && (*copy).cbData as usize == size_of::<LslIpc>()
                && !(*copy).lpData.is_null()
            {
                let request = &*((*copy).lpData as *const LslIpc);
                if request.magic == IPC_MAGIC {
                    return handle_command(request);
                }
            }
            0
        }

        WM_COMMAND => {
            let id = (wparam & 0xFFFF) as u32;
            match id {
                100..=103 => {
                    set_mode(Mode::from_u32(id - 100));
                }
                110 => {
                    set_theme_manual(true);
                }
                111 => {
                    set_theme_manual(false);
                }
                112 => {
                    toggle_theme();
                }
                120 => ui::ui_show(hwnd),
                121 => {
                    autostart_set(!autostart_get());
                    ui::ui_refresh();
                }
                122 => unsafe { PostQuitMessage(0) },
                _ => {}
            }
            0
        }

        WM_QUERYENDSESSION => 1,

        WM_ENDSESSION => {
            if wparam != 0 {
                unsafe { PostQuitMessage(0) };
            }
            0
        }

        WM_CLOSE => 0,

        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            0
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

pub fn describe_status() -> String {
    let state = app();
    let light = theme_read(true);
    let now = local_now();
    let (ms, to_light) = ms_to_next_change(&state.cfg, &now);

    let when = if ms != 0 {
        let mut ft: FILETIME = zero();
        unsafe { SystemTimeToFileTime(&now, &mut ft) };
        let value = filetime_to_u64(&ft) + ms as u64 * 10_000;
        let ft = FILETIME {
            dwLowDateTime: (value & 0xFFFF_FFFF) as u32,
            dwHighDateTime: (value >> 32) as u32,
        };
        let mut target = zero();
        unsafe { FileTimeToSystemTime(&ft, &mut target) };
        format!("{:02}:{:02}", target.wHour, target.wMinute)
    } else {
        String::new()
    };

    let now_text = match light {
        Some(true) => "light",
        Some(false) => "dark",
        None => "unknown",
    };

    if state.cfg.mode == Mode::Off {
        format!("Now {} \u{00b7} schedule off", now_text)
    } else if state.cfg.mode == Mode::NightLight {
        let nl = if state.has_night {
            if state.night_on {
                "on"
            } else {
                "off"
            }
        } else {
            "unknown"
        };
        format!(
            "Now {} \u{00b7} night light {}{}",
            now_text,
            nl,
            if state.override_ {
                " \u{00b7} manual"
            } else {
                ""
            }
        )
    } else if ms != 0 {
        format!(
            "Now {} \u{00b7} {} at {}{}",
            now_text,
            if to_light { "light" } else { "dark" },
            when,
            if state.override_ {
                " \u{00b7} manual"
            } else {
                ""
            }
        )
    } else {
        format!("Now {} \u{00b7} no scheduled changes", now_text)
    }
}

pub fn app_describe() -> String {
    describe_status()
}

/// Drops the process out of resident memory while it waits.
pub fn app_trim_memory() {
    unsafe {
        SetProcessWorkingSetSizeEx(GetCurrentProcess(), usize::MAX, usize::MAX, 0);
    }
}

pub fn app_show_settings() {
    let state = app();
    if !state.main.is_null() {
        unsafe { PostMessageW(state.main, SHOW_MESSAGE, 0, 0) };
    }
}

pub fn app_config_changed() {
    let state = app();
    cfg_load(&mut state.cfg);
    state.override_ = false;
    configure_night_light();
    evaluate();
    reset_timer();
    update_tray_tip();
    publish();
}

fn forward_to_running(launch: &Launch) {
    let class = wide_static(WNDCLASS);
    let other = unsafe { FindWindowW(class, ptr::null()) };
    if other.is_null() {
        return;
    }

    let request = LslIpc {
        magic: IPC_MAGIC,
        cmd: if launch.mode.is_some() {
            CMD_MODE
        } else {
            launch.command
        },
        arg: launch.mode.map_or(0, Mode::as_u32),
    };

    let data = COPYDATASTRUCT {
        dwData: 0,
        cbData: size_of::<LslIpc>() as u32,
        lpData: &request as *const LslIpc as *mut core::ffi::c_void,
    };
    unsafe {
        SendMessageW(
            other,
            WM_COPYDATA,
            0,
            &data as *const COPYDATASTRUCT as isize,
        )
    };
}

pub fn run(launch: Launch) -> i32 {
    let state = app();
    state.instance = unsafe { GetModuleHandleW(ptr::null()) };

    let mutex_name = wide(MUTEX);
    state.single_mutex = unsafe { CreateMutexW(ptr::null(), 1, mutex_name.as_ptr()) };
    let already_running =
        !state.single_mutex.is_null() && unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;

    if already_running {
        // A second silent start (for example a duplicated autostart entry) must
        // not pop the window open.
        if launch.background && launch.mode.is_none() && launch.command == CMD_SHOW {
            return 0;
        }
        forward_to_running(&launch);
        return 0;
    }

    cfg_load(&mut state.cfg);

    let class_name = wide_static(WNDCLASS);
    let mut wc: WNDCLASSEXW = zero();
    wc.cbSize = size_of::<WNDCLASSEXW>() as u32;
    wc.lpfnWndProc = Some(main_window_proc);
    wc.hInstance = state.instance;
    wc.lpszClassName = class_name;
    wc.hCursor = unsafe { LoadCursorW(ptr::null_mut(), IDC_ARROW) };
    if unsafe { RegisterClassExW(&wc) } == 0 {
        return 1;
    }

    let title = wide(APP_NAME);
    state.main = unsafe {
        CreateWindowExW(
            0,
            class_name,
            title.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            ptr::null_mut(),
            ptr::null_mut(),
            state.instance,
            ptr::null(),
        )
    };
    if state.main.is_null() {
        return 1;
    }

    let taskbar_name = wide("TaskbarCreated");
    state.taskbar_created = unsafe { RegisterWindowMessageW(taskbar_name.as_ptr()) };

    let icon_id = crate::int_resource(1);
    state.icon_large =
        unsafe { LoadImageW(state.instance, icon_id, IMAGE_ICON, 0, 0, LR_DEFAULTSIZE) } as HICON;
    let small = unsafe { GetSystemMetrics(SM_CXSMICON) };
    let small_y = unsafe { GetSystemMetrics(SM_CYSMICON) };
    state.icon_small =
        unsafe { LoadImageW(state.instance, icon_id, IMAGE_ICON, small, small_y, 0) } as HICON;

    let mapping_name = wide(MAPPING);
    state.mapping = unsafe {
        CreateFileMappingW(
            INVALID_HANDLE_VALUE,
            ptr::null(),
            PAGE_READWRITE,
            0,
            size_of::<LslStatus>() as u32,
            mapping_name.as_ptr(),
        )
    };
    if !state.mapping.is_null() {
        let view =
            unsafe { MapViewOfFile(state.mapping, FILE_MAP_WRITE, 0, 0, size_of::<LslStatus>()) };
        state.status = view.Value as *mut LslStatus;
    }
    if !state.status.is_null() {
        unsafe { ptr::write_bytes(state.status, 0, 1) };
    }

    add_tray_icon();
    arm_personalize();
    reset_timer();
    evaluate();
    configure_night_light();
    publish();

    if let Some(mode) = launch.mode {
        set_mode(mode);
    } else if launch.command == CMD_LIGHT {
        set_theme_manual(true);
    } else if launch.command == CMD_DARK {
        set_theme_manual(false);
    } else if launch.command == CMD_TOGGLE {
        toggle_theme();
    } else if !launch.background {
        ui::ui_show(state.main);
    } else {
        app_trim_memory();
    }

    let mut msg: MSG = zero();
    let mut quit = false;
    while !quit {
        let mut events: [HANDLE; 2] = [ptr::null_mut(); 2];
        let mut count = 0u32;
        if state.pers_armed {
            events[count as usize] = state.pers_event;
            count += 1;
        }
        if state.night_armed {
            events[count as usize] = state.night_event;
            count += 1;
        }

        let result = unsafe {
            MsgWaitForMultipleObjectsEx(
                count,
                events.as_ptr(),
                INFINITE,
                QS_ALLINPUT,
                MWMO_INPUTAVAILABLE,
            )
        };

        if result == WAIT_OBJECT_0 + count {
            while unsafe { PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, PM_REMOVE) } != 0 {
                if msg.message == WM_QUIT {
                    // WM_QUIT
                    quit = true;
                    break;
                }
                unsafe {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        } else if result < WAIT_OBJECT_0 + count {
            let signaled = events[(result - WAIT_OBJECT_0) as usize];
            if signaled == state.pers_event {
                state.pers_armed = false;
                on_personalize_changed();
                arm_personalize();
            } else if signaled == state.night_event {
                state.night_armed = false;
                on_night_light_changed();
                if state.cfg.mode == Mode::NightLight {
                    arm_night_light();
                }
            }
        } else {
            break;
        }
    }

    remove_tray_icon();
    ui::ui_destroy();

    if !state.status.is_null() {
        unsafe {
            UnmapViewOfFile(
                windows_sys::Win32::System::Memory::MEMORY_MAPPED_VIEW_ADDRESS {
                    Value: state.status as *mut core::ffi::c_void,
                },
            )
        };
    }
    if !state.mapping.is_null() {
        unsafe { CloseHandle(state.mapping) };
    }
    if !state.pers_event.is_null() {
        unsafe { CloseHandle(state.pers_event) };
    }
    if !state.night_event.is_null() {
        unsafe { CloseHandle(state.night_event) };
    }
    if !state.pers_key.is_null() {
        unsafe { RegCloseKey(state.pers_key) };
    }
    if !state.night_key.is_null() {
        unsafe { RegCloseKey(state.night_key) };
    }
    if !state.icon_large.is_null() {
        unsafe { DestroyIcon(state.icon_large) };
    }
    if !state.icon_small.is_null() {
        unsafe { DestroyIcon(state.icon_small) };
    }
    if !state.single_mutex.is_null() {
        unsafe {
            ReleaseMutex(state.single_mutex);
            CloseHandle(state.single_mutex);
        };
    }
    if !state.main.is_null() {
        unsafe { DestroyWindow(state.main) };
    }

    0
}
