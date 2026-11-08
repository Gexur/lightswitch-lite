//! Settings window: custom-drawn Win32 controls, ported from ui.cpp.

use std::mem::size_of;
use std::ptr;

use windows_sys::Win32::Foundation::{
    GetLastError, COLORREF, ERROR_CLASS_ALREADY_EXISTS, HINSTANCE, HWND, LPARAM, LRESULT, POINT,
    RECT, WPARAM,
};
use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreatePen, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint,
    FillRect, GetMonitorInfoW, InvalidateRect, LineTo, MonitorFromPoint, MoveToEx, RoundRect,
    SelectObject, SetBkColor, SetBkMode, SetTextColor, UpdateWindow, CLEARTYPE_QUALITY,
    CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_PITCH, DT_CENTER, DT_END_ELLIPSIS, DT_LEFT,
    DT_RIGHT, DT_SINGLELINE, DT_TOP, DT_VCENTER, DT_WORDBREAK, FF_DONTCARE, FW_NORMAL, FW_SEMIBOLD,
    HBRUSH, HDC, HFONT, MONITORINFO, MONITOR_DEFAULTTONEAREST, OUT_DEFAULT_PRECIS, PAINTSTRUCT,
    PS_SOLID, TRANSPARENT,
};
use windows_sys::Win32::UI::Controls::{SetWindowTheme, DRAWITEMSTRUCT, ODS_SELECTED};
use windows_sys::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForSystem, GetDpiForWindow};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, IsWindowEnabled, VK_ESCAPE};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyWindow, DrawIconEx, GetClientRect,
    GetCursorPos, GetDlgCtrlID, GetSystemMetrics, GetWindowLongPtrW, GetWindowTextW,
    IsWindowVisible, LoadCursorW, LoadImageW, RegisterClassExW, SendMessageW, SetForegroundWindow,
    SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow, BN_CLICKED, BS_OWNERDRAW,
    CBN_SELCHANGE, CBS_DROPDOWNLIST, CB_ADDSTRING, CB_GETCURSEL, CB_SETCURSEL, CW_USEDEFAULT,
    DI_NORMAL, EN_CHANGE, EN_KILLFOCUS, ES_AUTOHSCROLL, GWLP_HINSTANCE, GWLP_USERDATA, HICON,
    IDC_ARROW, IMAGE_ICON, SM_CXICON, SM_CXSMICON, SM_CYICON, SM_CYSMICON, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOZORDER, SW_SHOWNORMAL, WM_CLOSE, WM_COMMAND, WM_CTLCOLORBTN, WM_CTLCOLOREDIT,
    WM_CTLCOLORLISTBOX, WM_CTLCOLORSTATIC, WM_DESTROY, WM_DPICHANGED, WM_DRAWITEM, WM_ERASEBKGND,
    WM_KEYDOWN, WM_PAINT, WM_SETFONT, WNDCLASSEXW, WS_CAPTION, WS_CHILD, WS_EX_APPWINDOW,
    WS_EX_CLIENTEDGE, WS_OVERLAPPED, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};

use crate::app::{app_config_changed, app_describe, app_trim_memory};
use crate::settings::{autostart_get, autostart_set, cfg_load, cfg_save};
use crate::theme::theme_read;
use crate::{from_wide, wide, wide_static, Config, Global, Mode, APP_NAME, APP_VERSION};

const ID_MODE_OFF: i32 = 1000;
const ID_MODE_FIXED: i32 = 1001;
const ID_MODE_SOLAR: i32 = 1002;
const ID_MODE_NL: i32 = 1003;
const ID_LIGHT_H: i32 = 1010;
const ID_LIGHT_M: i32 = 1011;
const ID_DARK_H: i32 = 1012;
const ID_DARK_M: i32 = 1013;
const ID_LAT: i32 = 1020;
const ID_LON: i32 = 1021;
const ID_SYS: i32 = 1030;
const ID_APPS: i32 = 1031;
const ID_AUTO: i32 = 1032;
const ID_CLOSE: i32 = 1040;
const ID_RESET: i32 = 1041;

const WIDTH: i32 = 460;
const HEIGHT: i32 = 578;
const HEADER: i32 = 64;

struct Ui {
    hwnd: HWND,
    cfg: Config,
    auto_start: bool,
    dark: bool,
    loading: bool,
    bad_input: bool,
    dpi: i32,

    font_body: HFONT,
    font_bold: HFONT,
    font_title: HFONT,
    font_small: HFONT,

    brush_bg: HBRUSH,
    brush_card: HBRUSH,
    brush_card2: HBRUSH,
    brush_input: HBRUSH,
    brush_accent: HBRUSH,

    col_bg: COLORREF,
    col_card: COLORREF,
    col_card2: COLORREF,
    col_input: COLORREF,
    col_text: COLORREF,
    col_sub: COLORREF,
    col_border: COLORREF,
    col_accent: COLORREF,
    col_on_accent: COLORREF,

    h_mode: [HWND; 4],
    h_light_h: HWND,
    h_light_m: HWND,
    h_dark_h: HWND,
    h_dark_m: HWND,
    h_lat: HWND,
    h_lon: HWND,
    h_sys: HWND,
    h_apps: HWND,
    h_auto: HWND,
    h_close: HWND,
    h_reset: HWND,
    h_icon: HICON,
}

impl Ui {
    fn new() -> Box<Ui> {
        Box::new(Ui {
            hwnd: ptr::null_mut(),
            cfg: Config::defaults(),
            auto_start: false,
            dark: false,
            loading: false,
            bad_input: false,
            dpi: 96,
            font_body: ptr::null_mut(),
            font_bold: ptr::null_mut(),
            font_title: ptr::null_mut(),
            font_small: ptr::null_mut(),
            brush_bg: ptr::null_mut(),
            brush_card: ptr::null_mut(),
            brush_card2: ptr::null_mut(),
            brush_input: ptr::null_mut(),
            brush_accent: ptr::null_mut(),
            col_bg: 0,
            col_card: 0,
            col_card2: 0,
            col_input: 0,
            col_text: 0,
            col_sub: 0,
            col_border: 0,
            col_accent: 0,
            col_on_accent: 0,
            h_mode: [ptr::null_mut(); 4],
            h_light_h: ptr::null_mut(),
            h_light_m: ptr::null_mut(),
            h_dark_h: ptr::null_mut(),
            h_dark_m: ptr::null_mut(),
            h_lat: ptr::null_mut(),
            h_lon: ptr::null_mut(),
            h_sys: ptr::null_mut(),
            h_apps: ptr::null_mut(),
            h_auto: ptr::null_mut(),
            h_close: ptr::null_mut(),
            h_reset: ptr::null_mut(),
            h_icon: ptr::null_mut(),
        })
    }
}

static UI: Global<Option<Box<Ui>>> = Global::new(None);
static CLASS_READY: Global<bool> = Global::new(false);

fn ui_slot() -> &'static mut Option<Box<Ui>> {
    UI.get()
}

fn state_of(hwnd: HWND) -> *mut Ui {
    unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Ui }
}

fn px(u: &Ui, value: i32) -> i32 {
    (value * u.dpi + 48) / 96
}

fn text(dc: HDC, value: &str, mut box_: RECT, color: COLORREF, font: HFONT, flags: u32) {
    let value = wide(value);
    unsafe {
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, color);
        let previous = SelectObject(dc, font);
        DrawTextW(dc, value.as_ptr(), -1, &mut box_, flags);
        SelectObject(dc, previous);
    }
}

fn round_fill(dc: HDC, box_: RECT, radius: i32, fill: COLORREF, border: COLORREF) {
    unsafe {
        let brush = CreateSolidBrush(fill);
        let pen = CreatePen(PS_SOLID, 1, border);
        let old_brush = SelectObject(dc, brush);
        let old_pen = SelectObject(dc, pen);
        RoundRect(
            dc,
            box_.left,
            box_.top,
            box_.right,
            box_.bottom,
            radius,
            radius,
        );
        SelectObject(dc, old_brush);
        SelectObject(dc, old_pen);
        DeleteObject(brush);
        DeleteObject(pen);
    }
}

fn check_mark(dc: HDC, x: i32, y: i32, size: i32, width: i32, color: COLORREF) {
    unsafe {
        let pen = CreatePen(PS_SOLID, width, color);
        let old_pen = SelectObject(dc, pen);
        MoveToEx(
            dc,
            x + size * 22 / 100,
            y + size * 52 / 100,
            ptr::null_mut(),
        );
        LineTo(dc, x + size * 42 / 100, y + size * 72 / 100);
        LineTo(dc, x + size * 78 / 100, y + size * 28 / 100);
        SelectObject(dc, old_pen);
        DeleteObject(pen);
    }
}

fn make_font(points: i32, dpi: i32, weight: i32) -> HFONT {
    let height = -((points * dpi + 36) / 72);
    let face = wide("Segoe UI");
    unsafe {
        CreateFontW(
            height,
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            (DEFAULT_PITCH | FF_DONTCARE) as u32,
            face.as_ptr(),
        )
    }
}

fn drop_fonts(u: &mut Ui) {
    for font in [u.font_body, u.font_bold, u.font_title, u.font_small] {
        if !font.is_null() {
            unsafe { DeleteObject(font) };
        }
    }
    u.font_body = ptr::null_mut();
    u.font_bold = ptr::null_mut();
    u.font_title = ptr::null_mut();
    u.font_small = ptr::null_mut();
}

fn drop_brushes(u: &mut Ui) {
    for brush in [
        u.brush_bg,
        u.brush_card,
        u.brush_card2,
        u.brush_input,
        u.brush_accent,
    ] {
        if !brush.is_null() {
            unsafe { DeleteObject(brush) };
        }
    }
    u.brush_bg = ptr::null_mut();
    u.brush_card = ptr::null_mut();
    u.brush_card2 = ptr::null_mut();
    u.brush_input = ptr::null_mut();
    u.brush_accent = ptr::null_mut();
}

fn build_fonts(u: &mut Ui) {
    drop_fonts(u);
    u.font_body = make_font(9, u.dpi, FW_NORMAL as i32);
    u.font_bold = make_font(9, u.dpi, FW_SEMIBOLD as i32);
    u.font_title = make_font(15, u.dpi, FW_SEMIBOLD as i32);
    u.font_small = make_font(8, u.dpi, FW_NORMAL as i32);
}

fn build_palette(u: &mut Ui) {
    u.dark = theme_read(false).map(|light| !light).unwrap_or(false);

    if u.dark {
        u.col_bg = rgb(19, 20, 23);
        u.col_card = rgb(30, 32, 37);
        u.col_card2 = rgb(41, 44, 50);
        u.col_input = rgb(24, 26, 30);
        u.col_text = rgb(240, 242, 245);
        u.col_sub = rgb(150, 157, 166);
        u.col_border = rgb(56, 60, 67);
        u.col_accent = rgb(78, 190, 250);
        u.col_on_accent = rgb(6, 19, 27);
    } else {
        u.col_bg = rgb(245, 246, 249);
        u.col_card = rgb(255, 255, 255);
        u.col_card2 = rgb(241, 243, 247);
        u.col_input = rgb(255, 255, 255);
        u.col_text = rgb(22, 24, 28);
        u.col_sub = rgb(104, 110, 120);
        u.col_border = rgb(223, 226, 232);
        u.col_accent = rgb(15, 108, 189);
        u.col_on_accent = rgb(255, 255, 255);
    }

    drop_brushes(u);
    unsafe {
        u.brush_bg = CreateSolidBrush(u.col_bg);
        u.brush_card = CreateSolidBrush(u.col_card);
        u.brush_card2 = CreateSolidBrush(u.col_card2);
        u.brush_input = CreateSolidBrush(u.col_input);
        u.brush_accent = CreateSolidBrush(u.col_accent);

        let dark: i32 = i32::from(u.dark);
        if DwmSetWindowAttribute(
            u.hwnd,
            20,
            &dark as *const i32 as *const core::ffi::c_void,
            size_of::<i32>() as u32,
        ) != 0
        {
            DwmSetWindowAttribute(
                u.hwnd,
                19,
                &dark as *const i32 as *const core::ffi::c_void,
                size_of::<i32>() as u32,
            );
        }
    }
}

fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

fn apply_control_theme(u: &Ui) {
    let theme = if u.dark {
        Some(wide("DarkMode_CFD"))
    } else {
        None
    };
    let theme_ptr = theme.as_ref().map_or(ptr::null(), |value| value.as_ptr());
    for control in [
        u.h_light_h,
        u.h_light_m,
        u.h_dark_h,
        u.h_dark_m,
        u.h_lat,
        u.h_lon,
    ] {
        if !control.is_null() {
            unsafe { SetWindowTheme(control, theme_ptr, ptr::null()) };
        }
    }
}

fn hint_for(mode: Mode) -> &'static str {
    match mode {
        Mode::Fixed => "Switches to light and dark at the times you set, every day.",
        Mode::Solar => "Computes sunrise and sunset for your location, day by day.",
        Mode::NightLight => {
            "Follows the Windows Night light: the theme goes dark while night light is on."
        }
        Mode::Off => "Schedule off. The theme only changes with the shortcut or from the tray.",
    }
}

/// Accepts "12.5", "12,5" and "  +12.5 ".
fn parse_double(value: &str, low: f64, high: f64) -> Option<f64> {
    let cleaned: String = value
        .chars()
        .filter(|c| *c != ' ')
        .map(|c| match c {
            ',' => '.',
            '\u{2212}' => '-',
            other => other,
        })
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    let parsed: f64 = cleaned.parse().ok()?;
    if !parsed.is_finite() || parsed < low || parsed > high {
        return None;
    }
    Some(parsed)
}

fn sync_enabled(u: &Ui) {
    let fixed_mode = u.cfg.mode == Mode::Fixed;
    let solar = u.cfg.mode == Mode::Solar;

    unsafe {
        EnableWindow(u.h_light_h, i32::from(fixed_mode));
        EnableWindow(u.h_light_m, i32::from(fixed_mode));
        EnableWindow(u.h_dark_h, i32::from(fixed_mode));
        EnableWindow(u.h_dark_m, i32::from(fixed_mode));
        EnableWindow(u.h_lat, i32::from(solar));
        EnableWindow(u.h_lon, i32::from(solar));
    }
}

fn commit(u: &mut Ui) {
    if u.loading {
        return;
    }

    let mut stored = Config::defaults();
    if cfg_load(&mut stored) && stored.same_as(&u.cfg) {
        return;
    }

    cfg_save(&u.cfg);
    app_config_changed();
    sync_enabled(u);
    unsafe { InvalidateRect(u.hwnd, ptr::null(), 0) };
}

/// Returns false when a non-empty field cannot be parsed.
fn read_editors(u: &mut Ui) -> bool {
    if u.h_lat.is_null() || u.h_lon.is_null() {
        return true;
    }

    let mut buffer = [0u16; 64];
    let mut ok = true;

    unsafe { GetWindowTextW(u.h_lat, buffer.as_mut_ptr(), buffer.len() as i32) };
    let text_value = from_wide(&buffer);
    if let Some(value) = parse_double(&text_value, -90.0, 90.0) {
        u.cfg.lat = value;
    } else if !text_value.is_empty() {
        ok = false;
    }

    buffer = [0u16; 64];
    unsafe { GetWindowTextW(u.h_lon, buffer.as_mut_ptr(), buffer.len() as i32) };
    let text_value = from_wide(&buffer);
    if let Some(value) = parse_double(&text_value, -180.0, 180.0) {
        u.cfg.lon = value;
    } else if !text_value.is_empty() {
        ok = false;
    }

    ok
}

fn read_times(u: &mut Ui) {
    if u.h_light_h.is_null()
        || u.h_light_m.is_null()
        || u.h_dark_h.is_null()
        || u.h_dark_m.is_null()
    {
        return;
    }

    let light_hour = unsafe { SendMessageW(u.h_light_h, CB_GETCURSEL, 0, 0) } as i32;
    let light_minute = unsafe { SendMessageW(u.h_light_m, CB_GETCURSEL, 0, 0) } as i32;
    let dark_hour = unsafe { SendMessageW(u.h_dark_h, CB_GETCURSEL, 0, 0) } as i32;
    let dark_minute = unsafe { SendMessageW(u.h_dark_m, CB_GETCURSEL, 0, 0) } as i32;

    if (0..24).contains(&light_hour) && (0..60).contains(&light_minute) {
        u.cfg.light_min = light_hour * 60 + light_minute;
    }
    if (0..24).contains(&dark_hour) && (0..60).contains(&dark_minute) {
        u.cfg.dark_min = dark_hour * 60 + dark_minute;
    }
}

fn sync_controls(u: &mut Ui) {
    if u.h_light_h.is_null() || u.h_lat.is_null() {
        return;
    }

    u.loading = true;

    unsafe {
        SendMessageW(
            u.h_light_h,
            CB_SETCURSEL,
            (u.cfg.light_min / 60) as usize,
            0,
        );
        SendMessageW(
            u.h_light_m,
            CB_SETCURSEL,
            (u.cfg.light_min % 60) as usize,
            0,
        );
        SendMessageW(u.h_dark_h, CB_SETCURSEL, (u.cfg.dark_min / 60) as usize, 0);
        SendMessageW(u.h_dark_m, CB_SETCURSEL, (u.cfg.dark_min % 60) as usize, 0);
    }

    let lat = wide(&format!("{:.4}", u.cfg.lat));
    let lon = wide(&format!("{:.4}", u.cfg.lon));
    unsafe {
        SetWindowTextW(u.h_lat, lat.as_ptr());
        SetWindowTextW(u.h_lon, lon.as_ptr());
    }

    u.loading = false;

    sync_enabled(u);
    unsafe { InvalidateRect(u.hwnd, ptr::null(), 0) };
}

fn layout_children(u: &Ui) {
    let place = |control: HWND, x: i32, y: i32, w: i32, h: i32| {
        if control.is_null() {
            return;
        }
        unsafe {
            SetWindowPos(
                control,
                ptr::null_mut(),
                px(u, x),
                px(u, y),
                px(u, w),
                px(u, h),
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };
    };

    for i in 0..4 {
        place(u.h_mode[i], 28 + i as i32 * 103, 112, 95, 34);
    }

    place(u.h_light_h, 176, 156, 56, 200);
    place(u.h_light_m, 248, 156, 56, 200);
    place(u.h_dark_h, 176, 186, 56, 200);
    place(u.h_dark_m, 248, 186, 56, 200);

    place(u.h_lat, 84, 228, 100, 26);
    place(u.h_lon, 266, 228, 100, 26);

    place(u.h_sys, 28, 356, 404, 28);
    place(u.h_apps, 28, 388, 404, 28);
    place(u.h_auto, 28, 478, 404, 28);

    place(u.h_reset, 12, 536, 120, 30);
    place(u.h_close, 352, 536, 96, 30);
}

fn layout_window(u: &Ui, center: bool) {
    let mut box_ = RECT {
        left: 0,
        top: 0,
        right: px(u, WIDTH),
        bottom: px(u, HEIGHT),
    };
    unsafe {
        AdjustWindowRectExForDpi(
            &mut box_,
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
            0,
            0,
            u.dpi as u32,
        )
    };
    let width = box_.right - box_.left;
    let height = box_.bottom - box_.top;

    let mut x = CW_USEDEFAULT;
    let mut y = CW_USEDEFAULT;
    if center {
        let mut cursor = POINT { x: 0, y: 0 };
        unsafe { GetCursorPos(&mut cursor) };
        let monitor = unsafe { MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST) };
        let mut info: MONITORINFO = unsafe { std::mem::zeroed() };
        info.cbSize = size_of::<MONITORINFO>() as u32;
        if unsafe { GetMonitorInfoW(monitor, &mut info) } != 0 {
            x = info.rcWork.left + ((info.rcWork.right - info.rcWork.left) - width) / 2;
            y = info.rcWork.top + ((info.rcWork.bottom - info.rcWork.top) - height) / 2;
        }
    }

    unsafe {
        SetWindowPos(
            u.hwnd,
            ptr::null_mut(),
            if center { x } else { 0 },
            if center { y } else { 0 },
            width,
            height,
            SWP_NOZORDER | SWP_NOACTIVATE | if center { 0 } else { SWP_NOMOVE },
        )
    };
    layout_children(u);
}

fn paint_chrome(u: &Ui, dc: HDC) {
    unsafe {
        let mut client: RECT = std::mem::zeroed();
        GetClientRect(u.hwnd, &mut client);
        FillRect(dc, &client, u.brush_bg);

        let header = RECT {
            left: 0,
            top: 0,
            right: client.right,
            bottom: px(u, HEADER),
        };
        FillRect(dc, &header, u.brush_accent);

        if !u.h_icon.is_null() {
            DrawIconEx(
                dc,
                px(u, 16),
                px(u, 15),
                u.h_icon,
                px(u, 32),
                px(u, 32),
                0,
                ptr::null_mut(),
                DI_NORMAL,
            );
        }

        let title = RECT {
            left: px(u, 56),
            top: px(u, 6),
            right: client.right - px(u, 16),
            bottom: px(u, 34),
        };
        text(
            dc,
            APP_NAME,
            title,
            rgb(255, 255, 255),
            u.font_title,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE,
        );

        let version = RECT {
            left: client.right - px(u, 90),
            top: px(u, 12),
            right: client.right - px(u, 16),
            bottom: px(u, 30),
        };
        text(
            dc,
            &format!("v{}", APP_VERSION),
            version,
            rgb(255, 255, 255),
            u.font_small,
            DT_RIGHT | DT_VCENTER | DT_SINGLELINE,
        );

        let status = app_describe();
        let subtitle = RECT {
            left: px(u, 56),
            top: px(u, 34),
            right: client.right - px(u, 16),
            bottom: px(u, 56),
        };
        text(
            dc,
            &status,
            subtitle,
            rgb(235, 244, 252),
            u.font_small,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
        );

        let card = |x: i32, y: i32, w: i32, h: i32| {
            let box_ = RECT {
                left: px(u, x),
                top: px(u, y),
                right: px(u, x + w),
                bottom: px(u, y + h),
            };
            let old_brush = SelectObject(dc, u.brush_card);
            let pen = CreatePen(PS_SOLID, 1, u.col_border);
            let old_pen = SelectObject(dc, pen);
            RoundRect(
                dc,
                box_.left,
                box_.top,
                box_.right,
                box_.bottom,
                px(u, 12),
                px(u, 12),
            );
            SelectObject(dc, old_pen);
            SelectObject(dc, old_brush);
            DeleteObject(pen);
        };

        card(12, 74, 436, 236);
        card(12, 322, 436, 108);
        card(12, 442, 436, 76);

        let section = |x: i32, y: i32, label: &str| {
            let box_ = RECT {
                left: px(u, x),
                top: px(u, y),
                right: px(u, 440),
                bottom: px(u, y + 18),
            };
            text(
                dc,
                label,
                box_,
                u.col_text,
                u.font_bold,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );
        };

        section(28, 86, "Schedule");
        section(28, 334, "Apply theme to");
        section(28, 454, "System and startup");

        let label = |x: i32, y: i32, w: i32, value: &str| {
            let box_ = RECT {
                left: px(u, x),
                top: px(u, y),
                right: px(u, x + w),
                bottom: px(u, y + 26),
            };
            text(
                dc,
                value,
                box_,
                u.col_sub,
                u.font_body,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );
        };

        label(28, 156, 140, "Light from");
        label(28, 186, 140, "Dark from");
        label(236, 156, 10, ":");
        label(236, 186, 10, ":");
        label(28, 228, 54, "Latitude");
        label(200, 228, 64, "Longitude");

        let hint = RECT {
            left: px(u, 28),
            top: px(u, 262),
            right: px(u, 432),
            bottom: px(u, 302),
        };
        let missing_coords = u.cfg.mode == Mode::Solar && u.cfg.lat == 0.0 && u.cfg.lon == 0.0;
        let mut hint_text = hint_for(u.cfg.mode);
        let mut hint_color = u.col_sub;
        if u.bad_input {
            hint_text =
                "Invalid number. Use digits and a dot, for example -34.6037 (a comma works too).";
            hint_color = u.col_accent;
        } else if missing_coords {
            hint_text =
                "Enter your latitude and longitude: without them the solar schedule is useless.";
            hint_color = u.col_accent;
        }
        text(
            dc,
            hint_text,
            hint,
            hint_color,
            u.font_small,
            DT_LEFT | DT_TOP | DT_WORDBREAK,
        );
    }
}

fn check_value(u: &Ui, id: i32) -> bool {
    match id {
        ID_SYS => u.cfg.target_system,
        ID_APPS => u.cfg.target_apps,
        ID_AUTO => u.auto_start,
        _ => false,
    }
}

fn draw_item(u: &Ui, item: &DRAWITEMSTRUCT) {
    let dc = item.hDC;
    let box_ = item.rcItem;
    let id = item.CtlID as i32;
    let pressed = (item.itemState & ODS_SELECTED) != 0;

    if (ID_MODE_OFF..=ID_MODE_NL).contains(&id) {
        let selected = u.cfg.mode as i32 == id - ID_MODE_OFF;
        let fill = if selected { u.col_accent } else { u.col_card2 };
        let border = if selected { u.col_accent } else { u.col_border };
        unsafe { FillRect(dc, &box_, u.brush_card) };
        round_fill(dc, box_, px(u, 8), fill, border);

        let mut text_value = [0u16; 32];
        unsafe {
            GetWindowTextW(
                item.hwndItem,
                text_value.as_mut_ptr(),
                text_value.len() as i32,
            )
        };
        text(
            dc,
            &from_wide(&text_value),
            box_,
            if selected {
                u.col_on_accent
            } else {
                u.col_text
            },
            u.font_bold,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
        );
        return;
    }

    if id == ID_SYS || id == ID_APPS || id == ID_AUTO {
        unsafe { FillRect(dc, &box_, u.brush_card) };
        let on = check_value(u, id);
        let side = px(u, 18);
        let top = box_.top + ((box_.bottom - box_.top) - side) / 2;
        let marker = RECT {
            left: box_.left + px(u, 2),
            top,
            right: box_.left + px(u, 2) + side,
            bottom: top + side,
        };
        round_fill(
            dc,
            marker,
            px(u, 4),
            if on { u.col_accent } else { u.col_input },
            if on { u.col_accent } else { u.col_border },
        );
        if on {
            let width = px(u, 2).max(2);
            check_mark(dc, marker.left, marker.top, side, width, u.col_on_accent);
        }

        let mut text_value = [0u16; 128];
        unsafe {
            GetWindowTextW(
                item.hwndItem,
                text_value.as_mut_ptr(),
                text_value.len() as i32,
            )
        };
        let label_box = RECT {
            left: box_.left + px(u, 28),
            top: box_.top,
            right: box_.right,
            bottom: box_.bottom,
        };
        text(
            dc,
            &from_wide(&text_value),
            label_box,
            u.col_text,
            u.font_body,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
        );
        return;
    }

    let fill = if pressed { u.col_card2 } else { u.col_card };
    round_fill(dc, box_, px(u, 8), fill, u.col_border);
    let mut text_value = [0u16; 64];
    unsafe {
        GetWindowTextW(
            item.hwndItem,
            text_value.as_mut_ptr(),
            text_value.len() as i32,
        )
    };
    text(
        dc,
        &from_wide(&text_value),
        box_,
        u.col_text,
        u.font_body,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
    );
}

fn on_mode_clicked(u: &mut Ui, mode: Mode) {
    u.cfg.mode = mode;
    commit(u);
    for button in u.h_mode {
        unsafe { InvalidateRect(button, ptr::null(), 0) };
    }
}

unsafe extern "system" fn settings_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let state = state_of(hwnd);

    match msg {
        WM_ERASEBKGND => 1,

        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let dc = BeginPaint(hwnd, &mut ps);
            if !state.is_null() {
                paint_chrome(&*state, dc);
            }
            EndPaint(hwnd, &ps);
            0
        }

        WM_DRAWITEM => {
            if !state.is_null() {
                draw_item(&*state, &*(lparam as *const DRAWITEMSTRUCT));
                1
            } else {
                0
            }
        }

        WM_CTLCOLORBTN => {
            if !state.is_null() {
                let id = GetDlgCtrlID(lparam as HWND);
                let footer = id == ID_CLOSE || id == ID_RESET;
                let u = &*state;
                return if footer { u.brush_bg } else { u.brush_card } as isize;
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }

        WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
            if !state.is_null() {
                let u = &*state;
                let dc = wparam as HDC;
                SetTextColor(dc, u.col_text);
                SetBkColor(dc, u.col_input);
                u.brush_input as isize
            } else {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }

        WM_CTLCOLORSTATIC => {
            if !state.is_null() {
                let u = &*state;
                let dc = wparam as HDC;
                SetTextColor(
                    dc,
                    if IsWindowEnabled(lparam as HWND) != 0 {
                        u.col_text
                    } else {
                        u.col_sub
                    },
                );
                SetBkColor(dc, u.col_card);
                u.brush_card as isize
            } else {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }

        WM_COMMAND => {
            if state.is_null() {
                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }

            let u = &mut *state;
            let id = (wparam & 0xFFFF) as i32;
            let code = ((wparam >> 16) & 0xFFFF) as u32;

            match id {
                ID_MODE_OFF | ID_MODE_FIXED | ID_MODE_SOLAR | ID_MODE_NL => {
                    if code == BN_CLICKED {
                        on_mode_clicked(u, Mode::from_u32((id - ID_MODE_OFF) as u32));
                    }
                    0
                }

                ID_LIGHT_H | ID_LIGHT_M | ID_DARK_H | ID_DARK_M => {
                    if code == CBN_SELCHANGE && !u.loading {
                        read_times(u);
                        commit(u);
                    }
                    0
                }

                ID_LAT | ID_LON => {
                    if code == EN_CHANGE && !u.loading {
                        let valid = read_editors(u);
                        u.bad_input = !valid;
                        if valid {
                            commit(u);
                        }
                        InvalidateRect(hwnd, ptr::null(), 0);
                    } else if code == EN_KILLFOCUS && !u.loading {
                        let valid = read_editors(u);
                        u.bad_input = !valid;
                        if valid {
                            commit(u);
                        } else {
                            sync_controls(u); // restore the last accepted value
                        }
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                    0
                }

                ID_SYS | ID_APPS => {
                    if code != BN_CLICKED {
                        return 0;
                    }
                    if id == ID_SYS {
                        u.cfg.target_system = !u.cfg.target_system;
                    } else {
                        u.cfg.target_apps = !u.cfg.target_apps;
                    }
                    commit(u);
                    InvalidateRect(lparam as HWND, ptr::null(), 0);
                    0
                }

                ID_AUTO => {
                    if code != BN_CLICKED {
                        return 0;
                    }
                    u.auto_start = !u.auto_start;
                    autostart_set(u.auto_start);
                    InvalidateRect(u.h_auto, ptr::null(), 0);
                    0
                }

                ID_RESET => {
                    if code != BN_CLICKED {
                        return 0;
                    }
                    u.cfg = Config::defaults();
                    cfg_save(&u.cfg);
                    app_config_changed();
                    sync_controls(u);
                    0
                }

                ID_CLOSE => {
                    if code != BN_CLICKED {
                        return 0;
                    }
                    DestroyWindow(hwnd);
                    0
                }

                _ => DefWindowProcW(hwnd, msg, wparam, lparam),
            }
        }

        WM_KEYDOWN => {
            if wparam == VK_ESCAPE as usize {
                DestroyWindow(hwnd);
                0
            } else {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }

        WM_DPICHANGED => {
            if !state.is_null() {
                let u = &mut *state;
                u.dpi = ((wparam >> 16) & 0xFFFF) as i32;
                build_fonts(u);
                let suggested = &*(lparam as *const RECT);
                SetWindowPos(
                    hwnd,
                    ptr::null_mut(),
                    suggested.left,
                    suggested.top,
                    suggested.right - suggested.left,
                    suggested.bottom - suggested.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                layout_children(u);
                InvalidateRect(hwnd, ptr::null(), 1);
            }
            0
        }

        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }

        WM_DESTROY => {
            let u = state_of(hwnd);
            if !u.is_null() {
                let u = &mut *u;
                drop_fonts(u);
                drop_brushes(u);
                if !u.h_icon.is_null() {
                    DestroyIcon(u.h_icon);
                }
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                *ui_slot() = None;
                app_trim_memory();
            }
            0
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn make_control(u: &Ui, class: &str, value: &str, style: u32, extra: u32, id: i32) -> HWND {
    let class = wide(class);
    let value = wide(value);
    let instance = unsafe { GetWindowLongPtrW(u.hwnd, GWLP_HINSTANCE) as HINSTANCE };
    let control = unsafe {
        CreateWindowExW(
            extra,
            class.as_ptr(),
            value.as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            0,
            0,
            0,
            0,
            u.hwnd,
            id as usize as *mut core::ffi::c_void,
            instance,
            ptr::null(),
        )
    };
    if !control.is_null() {
        unsafe { SendMessageW(control, WM_SETFONT, u.font_body as WPARAM, 1) };
    }
    control
}

fn make_combo(u: &Ui, id: i32, count: i32) -> HWND {
    let combo = make_control(
        u,
        "COMBOBOX",
        "",
        (CBS_DROPDOWNLIST as u32) | WS_VSCROLL | WS_TABSTOP,
        WS_EX_CLIENTEDGE,
        id,
    );
    if combo.is_null() {
        return combo;
    }
    for i in 0..count {
        let item = wide(&format!("{:02}", i));
        unsafe { SendMessageW(combo, CB_ADDSTRING, 0, item.as_ptr() as isize) };
    }
    combo
}

fn create_children(u: &mut Ui) {
    // Creating the edit controls fires EN_CHANGE with their placeholder text,
    // which would be read back and saved over the real settings.
    u.loading = true;

    let modes = [
        (ID_MODE_OFF, "Off"),
        (ID_MODE_FIXED, "Fixed hours"),
        (ID_MODE_SOLAR, "Solar"),
        (ID_MODE_NL, "Night light"),
    ];
    for (index, (id, label)) in modes.iter().enumerate() {
        u.h_mode[index] = make_control(
            u,
            "BUTTON",
            label,
            (BS_OWNERDRAW as u32) | WS_TABSTOP,
            0,
            *id,
        );
    }

    u.h_light_h = make_combo(u, ID_LIGHT_H, 24);
    u.h_light_m = make_combo(u, ID_LIGHT_M, 60);
    u.h_dark_h = make_combo(u, ID_DARK_H, 24);
    u.h_dark_m = make_combo(u, ID_DARK_M, 60);

    u.h_lat = make_control(
        u,
        "EDIT",
        "0",
        (ES_AUTOHSCROLL as u32) | WS_TABSTOP,
        WS_EX_CLIENTEDGE,
        ID_LAT,
    );
    u.h_lon = make_control(
        u,
        "EDIT",
        "0",
        (ES_AUTOHSCROLL as u32) | WS_TABSTOP,
        WS_EX_CLIENTEDGE,
        ID_LON,
    );

    u.h_sys = make_control(
        u,
        "BUTTON",
        "Windows UI (taskbar, Start)",
        (BS_OWNERDRAW as u32) | WS_TABSTOP,
        0,
        ID_SYS,
    );
    u.h_apps = make_control(
        u,
        "BUTTON",
        "Applications and windows",
        (BS_OWNERDRAW as u32) | WS_TABSTOP,
        0,
        ID_APPS,
    );
    u.h_auto = make_control(
        u,
        "BUTTON",
        "Start with Windows",
        (BS_OWNERDRAW as u32) | WS_TABSTOP,
        0,
        ID_AUTO,
    );

    u.h_reset = make_control(
        u,
        "BUTTON",
        "Reset",
        (BS_OWNERDRAW as u32) | WS_TABSTOP,
        0,
        ID_RESET,
    );
    u.h_close = make_control(
        u,
        "BUTTON",
        "Close",
        (BS_OWNERDRAW as u32) | WS_TABSTOP,
        0,
        ID_CLOSE,
    );

    apply_control_theme(u);
    u.loading = false;
}

fn show_window(u: &mut Ui) {
    u.loading = true;
    cfg_load(&mut u.cfg);
    u.auto_start = autostart_get();
    u.loading = false;

    build_palette(u);
    apply_control_theme(u);
    sync_controls(u);
    sync_enabled(u);

    unsafe {
        ShowWindow(u.hwnd, SW_SHOWNORMAL);
        SetForegroundWindow(u.hwnd);
        InvalidateRect(u.hwnd, ptr::null(), 1);
    }
}

pub fn ui_show(owner: HWND) {
    let _ = owner;

    if let Some(u) = ui_slot().as_mut() {
        if !u.hwnd.is_null() {
            show_window(u);
            return;
        }
    }

    let instance = crate::app::app().instance;
    if instance.is_null() {
        return;
    }

    let mut boxed = Ui::new();
    boxed.dpi = unsafe { GetDpiForSystem() } as i32;

    let class_name = wide_static(crate::ipc::SETTINGS_WNDCLASS);
    let wc_icon =
        unsafe { LoadImageW(instance, crate::int_resource(1), IMAGE_ICON, 32, 32, 0) } as HICON;
    let wc_icon_small = unsafe {
        LoadImageW(
            instance,
            crate::int_resource(1),
            IMAGE_ICON,
            GetSystemMetrics(SM_CXSMICON),
            GetSystemMetrics(SM_CYSMICON),
            0,
        )
    } as HICON;

    let mut wc: WNDCLASSEXW = unsafe { std::mem::zeroed() };
    wc.cbSize = size_of::<WNDCLASSEXW>() as u32;
    wc.lpfnWndProc = Some(settings_proc);
    wc.hInstance = instance;
    wc.hCursor = unsafe { LoadCursorW(ptr::null_mut(), IDC_ARROW) };
    wc.lpszClassName = class_name;
    wc.hIcon = wc_icon;
    wc.hIconSm = wc_icon_small;

    if !*CLASS_READY.get() {
        let atom = unsafe { RegisterClassExW(&wc) };
        if atom == 0 && unsafe { GetLastError() } != ERROR_CLASS_ALREADY_EXISTS {
            *ui_slot() = None;
            return;
        }
        *CLASS_READY.get() = true;
    }

    cfg_load(&mut boxed.cfg);
    boxed.auto_start = autostart_get();
    boxed.h_icon = unsafe {
        LoadImageW(
            instance,
            crate::int_resource(1),
            IMAGE_ICON,
            GetSystemMetrics(SM_CXICON),
            GetSystemMetrics(SM_CYICON),
            0,
        )
    } as HICON;

    let mut frame = RECT {
        left: 0,
        top: 0,
        right: px(&boxed, WIDTH),
        bottom: px(&boxed, HEIGHT),
    };
    unsafe {
        AdjustWindowRectExForDpi(
            &mut frame,
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
            0,
            0,
            boxed.dpi as u32,
        )
    };

    let title = wide("LightSwitch Lite \u{2014} Settings");
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_APPWINDOW,
            class_name,
            title.as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            frame.right - frame.left,
            frame.bottom - frame.top,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null(),
        )
    };
    if hwnd.is_null() {
        if !boxed.h_icon.is_null() {
            unsafe { DestroyIcon(boxed.h_icon) };
        }
        *ui_slot() = None;
        return;
    }

    boxed.hwnd = hwnd;
    let ptr_ui = &mut *boxed as *mut Ui;
    *ui_slot() = Some(boxed);

    let u = unsafe { &mut *ptr_ui };
    u.dpi = unsafe { GetDpiForWindow(hwnd) } as i32;
    unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr_ui as isize) };

    build_fonts(u);
    build_palette(u);
    create_children(u);
    layout_window(u, true);
    sync_controls(u);

    unsafe {
        ShowWindow(u.hwnd, SW_SHOWNORMAL);
        UpdateWindow(u.hwnd);
    }
}

pub fn ui_refresh() {
    let Some(u) = ui_slot().as_mut() else {
        return;
    };
    if u.hwnd.is_null() || unsafe { IsWindowVisible(u.hwnd) } == 0 {
        return;
    }

    u.loading = true;
    cfg_load(&mut u.cfg);
    u.auto_start = autostart_get();
    u.loading = false;

    build_palette(u);
    apply_control_theme(u);
    sync_controls(u);
    unsafe { InvalidateRect(u.hwnd, ptr::null(), 1) };
}

pub fn ui_theme_changed() {
    let Some(u) = ui_slot().as_mut() else {
        return;
    };
    if u.hwnd.is_null() || unsafe { IsWindowVisible(u.hwnd) } == 0 {
        return;
    }

    build_palette(u);
    apply_control_theme(u);
    unsafe { InvalidateRect(u.hwnd, ptr::null(), 1) };
}

pub fn ui_destroy() {
    if let Some(u) = ui_slot().as_mut() {
        if !u.hwnd.is_null() {
            unsafe { DestroyWindow(u.hwnd) };
        }
    }
    *ui_slot() = None;
}
