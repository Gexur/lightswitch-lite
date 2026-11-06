#include "core.h"

#include <dwmapi.h>
#include <uxtheme.h>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cwchar>

namespace
{
    enum : int
    {
        ID_MODE_OFF = 1000,
        ID_MODE_FIXED,
        ID_MODE_SOLAR,
        ID_MODE_NL,
        ID_LIGHT_H = 1010,
        ID_LIGHT_M,
        ID_DARK_H,
        ID_DARK_M,
        ID_LAT = 1020,
        ID_LON,
        ID_SYS = 1030,
        ID_APPS,
        ID_AUTO,
        ID_CLOSE = 1040,
        ID_RESET,
    };

    // Layout is designed for 96 DPI and scaled per monitor.
    constexpr int kWidth = 460;
    constexpr int kHeight = 578;
    constexpr int kHeader = 64;

    constexpr wchar_t kClassName[] = L"LSL.SettingsWindow";

    struct Ui
    {
        HWND hwnd;
        Config cfg;
        bool autoStart;
        bool dark;
        bool loading;
        bool badInput;
        int dpi;

        HFONT fontBody;
        HFONT fontBold;
        HFONT fontTitle;
        HFONT fontSmall;

        HBRUSH brushBg;
        HBRUSH brushCard;
        HBRUSH brushCard2;
        HBRUSH brushInput;
        HBRUSH brushAccent;

        COLORREF colBg;
        COLORREF colCard;
        COLORREF colCard2;
        COLORREF colInput;
        COLORREF colText;
        COLORREF colSub;
        COLORREF colBorder;
        COLORREF colAccent;
        COLORREF colOnAccent;

        HWND hMode[4];
        HWND hLightH;
        HWND hLightM;
        HWND hDarkH;
        HWND hDarkM;
        HWND hLat;
        HWND hLon;
        HWND hSys;
        HWND hApps;
        HWND hAuto;
        HWND hClose;
        HWND hReset;
        HICON hIcon;
    };

    Ui* gUi = nullptr;
    bool gClassReady = false;

    int px(const Ui* u, int value)
    {
        return MulDiv(value, u->dpi, 96);
    }

    Ui* stateOf(HWND hwnd)
    {
        return reinterpret_cast<Ui*>(GetWindowLongPtrW(hwnd, GWLP_USERDATA));
    }

    void text(HDC dc, const wchar_t* value, RECT box, COLORREF color, HFONT font, UINT flags)
    {
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, color);
        HGDIOBJ previous = SelectObject(dc, font);
        DrawTextW(dc, value, -1, &box, flags);
        SelectObject(dc, previous);
    }

    void roundFill(HDC dc, RECT box, int radius, COLORREF fill, COLORREF border)
    {
        HBRUSH brush = CreateSolidBrush(fill);
        HPEN pen = CreatePen(PS_SOLID, 1, border);
        HGDIOBJ oldBrush = SelectObject(dc, brush);
        HGDIOBJ oldPen = SelectObject(dc, pen);
        RoundRect(dc, box.left, box.top, box.right, box.bottom, radius, radius);
        SelectObject(dc, oldBrush);
        SelectObject(dc, oldPen);
        DeleteObject(brush);
        DeleteObject(pen);
    }

    void checkMark(HDC dc, int x, int y, int size, int width, COLORREF color)
    {
        HPEN pen = CreatePen(PS_SOLID, width, color);
        HGDIOBJ oldPen = SelectObject(dc, pen);
        MoveToEx(dc, x + size * 22 / 100, y + size * 52 / 100, nullptr);
        LineTo(dc, x + size * 42 / 100, y + size * 72 / 100);
        LineTo(dc, x + size * 78 / 100, y + size * 28 / 100);
        SelectObject(dc, oldPen);
        DeleteObject(pen);
    }

    HFONT makeFont(int points, int dpi, int weight)
    {
        return CreateFontW(-MulDiv(points, dpi, 72), 0, 0, 0, weight, FALSE, FALSE, FALSE, DEFAULT_CHARSET,
                           OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY,
                           DEFAULT_PITCH | FF_DONTCARE, L"Segoe UI");
    }

    void dropFonts(Ui* u)
    {
        HFONT fonts[] = {u->fontBody, u->fontBold, u->fontTitle, u->fontSmall};
        for (HFONT font : fonts)
        {
            if (font)
                DeleteObject(font);
        }
        u->fontBody = nullptr;
        u->fontBold = nullptr;
        u->fontTitle = nullptr;
        u->fontSmall = nullptr;
    }

    void dropBrushes(Ui* u)
    {
        HBRUSH brushes[] = {u->brushBg, u->brushCard, u->brushCard2, u->brushInput, u->brushAccent};
        for (HBRUSH brush : brushes)
        {
            if (brush)
                DeleteObject(brush);
        }
        u->brushBg = nullptr;
        u->brushCard = nullptr;
        u->brushCard2 = nullptr;
        u->brushInput = nullptr;
        u->brushAccent = nullptr;
    }

    void buildFonts(Ui* u)
    {
        dropFonts(u);
        u->fontBody = makeFont(9, u->dpi, FW_NORMAL);
        u->fontBold = makeFont(9, u->dpi, FW_SEMIBOLD);
        u->fontTitle = makeFont(15, u->dpi, FW_SEMIBOLD);
        u->fontSmall = makeFont(8, u->dpi, FW_NORMAL);
    }

    void buildPalette(Ui* u)
    {
        bool light = false;
        const bool known = themeRead(false, light);
        u->dark = known ? !light : false;

        if (u->dark)
        {
            u->colBg = RGB(19, 20, 23);
            u->colCard = RGB(30, 32, 37);
            u->colCard2 = RGB(41, 44, 50);
            u->colInput = RGB(24, 26, 30);
            u->colText = RGB(240, 242, 245);
            u->colSub = RGB(150, 157, 166);
            u->colBorder = RGB(56, 60, 67);
            u->colAccent = RGB(78, 190, 250);
            u->colOnAccent = RGB(6, 19, 27);
        }
        else
        {
            u->colBg = RGB(245, 246, 249);
            u->colCard = RGB(255, 255, 255);
            u->colCard2 = RGB(241, 243, 247);
            u->colInput = RGB(255, 255, 255);
            u->colText = RGB(22, 24, 28);
            u->colSub = RGB(104, 110, 120);
            u->colBorder = RGB(223, 226, 232);
            u->colAccent = RGB(15, 108, 189);
            u->colOnAccent = RGB(255, 255, 255);
        }

        dropBrushes(u);
        u->brushBg = CreateSolidBrush(u->colBg);
        u->brushCard = CreateSolidBrush(u->colCard);
        u->brushCard2 = CreateSolidBrush(u->colCard2);
        u->brushInput = CreateSolidBrush(u->colInput);
        u->brushAccent = CreateSolidBrush(u->colAccent);

        const BOOL dark = u->dark ? TRUE : FALSE;
        if (FAILED(DwmSetWindowAttribute(u->hwnd, 20, &dark, sizeof(dark))))
            DwmSetWindowAttribute(u->hwnd, 19, &dark, sizeof(dark));
    }

    void applyControlTheme(Ui* u)
    {
        const wchar_t* theme = u->dark ? L"DarkMode_CFD" : nullptr;
        HWND controls[] = {u->hLightH, u->hLightM, u->hDarkH, u->hDarkM, u->hLat, u->hLon};
        for (HWND control : controls)
        {
            if (control)
                SetWindowTheme(control, theme, nullptr);
        }
    }

    const wchar_t* hintFor(Mode mode)
    {
        switch (mode)
        {
        case Mode::Fixed:
            return L"Switches to light and dark at the times you set, every day.";
        case Mode::Solar:
            return L"Computes sunrise and sunset for your location, day by day.";
        case Mode::NightLight:
            return L"Follows the Windows Night light: the theme goes dark while night light "
                   L"is on.";
        default:
            return L"Schedule off. The theme only changes with the shortcut or from the tray.";
        }
    }

    // Accepts "12.5", "12,5" and "  +12.5 ". A comma is what most keyboards
    // produce in Spanish-speaking regions, and silently rejecting it made the
    // field look like it never saved.
    bool parseDouble(const wchar_t* value, double low, double high, double& out)
    {
        wchar_t buffer[64] = {};
        int used = 0;
        for (const wchar_t* p = value; *p && used < 63; ++p)
        {
            if (*p == L' ')
                continue;
            buffer[used++] = (*p == L',') ? L'.' : (*p == 0x2212 ? L'-' : *p);
        }
        buffer[used] = L'\0';
        if (used == 0)
            return false;

        wchar_t* end = nullptr;
        const double parsed = wcstod(buffer, &end);
        if (end == buffer || *end != L'\0' || !std::isfinite(parsed) || parsed < low || parsed > high)
            return false;
        out = parsed;
        return true;
    }



    void syncEnabled(Ui* u)
    {
        const bool fixedMode = u->cfg.mode == Mode::Fixed;
        const bool solar = u->cfg.mode == Mode::Solar;

        EnableWindow(u->hLightH, fixedMode);
        EnableWindow(u->hLightM, fixedMode);
        EnableWindow(u->hDarkH, fixedMode);
        EnableWindow(u->hDarkM, fixedMode);
        EnableWindow(u->hLat, solar);
        EnableWindow(u->hLon, solar);
    }

    void syncControls(Ui* u);

    bool sameConfig(const Config& a, const Config& b)
    {
        return a.mode == b.mode && a.lightMin == b.lightMin && a.darkMin == b.darkMin && a.lat == b.lat &&
               a.lon == b.lon && a.targetSystem == b.targetSystem && a.targetApps == b.targetApps;
    }

    void commit(Ui* u)
    {
        if (u->loading)
            return;

        Config stored{};
        if (cfgLoad(stored) && sameConfig(stored, u->cfg))
            return;

        cfgSave(u->cfg);
        appConfigChanged();
        syncEnabled(u);
        InvalidateRect(u->hwnd, nullptr, FALSE);
    }

    // Returns false when a non-empty field cannot be parsed, so the window can
    // say so instead of silently reverting the text.
    bool readEditors(Ui* u)
    {
        if (!u->hLat || !u->hLon)
            return true;

        wchar_t buffer[64] = {};
        bool ok = true;

        GetWindowTextW(u->hLat, buffer, 64);
        double value = 0.0;
        if (parseDouble(buffer, -90.0, 90.0, value))
            u->cfg.lat = value;
        else if (buffer[0] != L'\0')
            ok = false;

        GetWindowTextW(u->hLon, buffer, 64);
        if (parseDouble(buffer, -180.0, 180.0, value))
            u->cfg.lon = value;
        else if (buffer[0] != L'\0')
            ok = false;

        return ok;
    }

    void readTimes(Ui* u)
    {
        if (!u->hLightH || !u->hLightM || !u->hDarkH || !u->hDarkM)
            return;

        const int lightHour = static_cast<int>(SendMessageW(u->hLightH, CB_GETCURSEL, 0, 0));
        const int lightMinute = static_cast<int>(SendMessageW(u->hLightM, CB_GETCURSEL, 0, 0));
        const int darkHour = static_cast<int>(SendMessageW(u->hDarkH, CB_GETCURSEL, 0, 0));
        const int darkMinute = static_cast<int>(SendMessageW(u->hDarkM, CB_GETCURSEL, 0, 0));

        if (lightHour >= 0 && lightHour <= 23 && lightMinute >= 0 && lightMinute <= 59)
            u->cfg.lightMin = lightHour * 60 + lightMinute;
        if (darkHour >= 0 && darkHour <= 23 && darkMinute >= 0 && darkMinute <= 59)
            u->cfg.darkMin = darkHour * 60 + darkMinute;
    }

    void syncControls(Ui* u)
    {
        if (!u->hLightH || !u->hLat)
            return;

        u->loading = true;

        SendMessageW(u->hLightH, CB_SETCURSEL, u->cfg.lightMin / 60, 0);
        SendMessageW(u->hLightM, CB_SETCURSEL, u->cfg.lightMin % 60, 0);
        SendMessageW(u->hDarkH, CB_SETCURSEL, u->cfg.darkMin / 60, 0);
        SendMessageW(u->hDarkM, CB_SETCURSEL, u->cfg.darkMin % 60, 0);

        wchar_t buffer[64];
        swprintf_s(buffer, L"%.4f", u->cfg.lat);
        SetWindowTextW(u->hLat, buffer);
        swprintf_s(buffer, L"%.4f", u->cfg.lon);
        SetWindowTextW(u->hLon, buffer);

        u->loading = false;

        syncEnabled(u);
        InvalidateRect(u->hwnd, nullptr, FALSE);
    }

    void layoutChildren(Ui* u)
    {
        auto place = [&](HWND control, int x, int y, int w, int h) {
            if (!control)
                return;
            SetWindowPos(control, nullptr, px(u, x), px(u, y), px(u, w), px(u, h),
                         SWP_NOZORDER | SWP_NOACTIVATE);
        };

        for (int i = 0; i < 4; ++i)
            place(u->hMode[i], 28 + i * 103, 112, 95, 34);

        place(u->hLightH, 176, 156, 56, 200);
        place(u->hLightM, 248, 156, 56, 200);
        place(u->hDarkH, 176, 186, 56, 200);
        place(u->hDarkM, 248, 186, 56, 200);

        place(u->hLat, 84, 228, 100, 26);
        place(u->hLon, 266, 228, 100, 26);

        place(u->hSys, 28, 356, 404, 28);
        place(u->hApps, 28, 388, 404, 28);
        place(u->hAuto, 28, 478, 404, 28);

        place(u->hReset, 12, 536, 120, 30);
        place(u->hClose, 352, 536, 96, 30);
    }

    void layoutWindow(Ui* u, bool center)
    {
        RECT box{0, 0, px(u, kWidth), px(u, kHeight)};
        AdjustWindowRectExForDpi(&box, WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU, FALSE, 0, u->dpi);
        const int width = box.right - box.left;
        const int height = box.bottom - box.top;

        int x = CW_USEDEFAULT;
        int y = CW_USEDEFAULT;
        if (center)
        {
            MONITORINFO info{};
            info.cbSize = sizeof(info);
            POINT cursor{};
            GetCursorPos(&cursor);
            if (GetMonitorInfoW(MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST), &info))
            {
                x = info.rcWork.left + ((info.rcWork.right - info.rcWork.left) - width) / 2;
                y = info.rcWork.top + ((info.rcWork.bottom - info.rcWork.top) - height) / 2;
            }
        }

        SetWindowPos(u->hwnd, nullptr, center ? x : 0, center ? y : 0, width, height,
                     SWP_NOZORDER | SWP_NOACTIVATE | (center ? 0 : SWP_NOMOVE));
        layoutChildren(u);
    }

    void paintChrome(Ui* u, HDC dc)
    {
        RECT client{};
        GetClientRect(u->hwnd, &client);
        FillRect(dc, &client, u->brushBg);

        RECT header{0, 0, client.right, px(u, kHeader)};
        FillRect(dc, &header, u->brushAccent);

        if (u->hIcon)
            DrawIconEx(dc, px(u, 16), px(u, 15), u->hIcon, px(u, 32), px(u, 32), 0, nullptr, DI_NORMAL);

        RECT title{px(u, 56), px(u, 6), client.right - px(u, 16), px(u, 34)};
        text(dc, LSL_APP, title, RGB(255, 255, 255), u->fontTitle, DT_LEFT | DT_VCENTER | DT_SINGLELINE);

        RECT version{client.right - px(u, 90), px(u, 12), client.right - px(u, 16), px(u, 30)};
        text(dc, L"v" LSL_VER, version, RGB(255, 255, 255), u->fontSmall,
             DT_RIGHT | DT_VCENTER | DT_SINGLELINE);

        wchar_t status[192] = {};
        appDescribe(status, 192);
        RECT subtitle{px(u, 56), px(u, 34), client.right - px(u, 16), px(u, 56)};
        text(dc, status, subtitle, RGB(235, 244, 252), u->fontSmall,
             DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);

        auto card = [&](int x, int y, int w, int h) {
            RECT box{px(u, x), px(u, y), px(u, x + w), px(u, y + h)};
            HGDIOBJ oldBrush = SelectObject(dc, u->brushCard);
            HPEN pen = CreatePen(PS_SOLID, 1, u->colBorder);
            HGDIOBJ oldPen = SelectObject(dc, pen);
            RoundRect(dc, box.left, box.top, box.right, box.bottom, px(u, 12), px(u, 12));
            SelectObject(dc, oldPen);
            SelectObject(dc, oldBrush);
            DeleteObject(pen);
        };

        card(12, 74, 436, 236);
        card(12, 322, 436, 108);
        card(12, 442, 436, 76);

        auto section = [&](int x, int y, const wchar_t* label) {
            RECT box{px(u, x), px(u, y), px(u, 440), px(u, y + 18)};
            text(dc, label, box, u->colText, u->fontBold, DT_LEFT | DT_VCENTER | DT_SINGLELINE);
        };

        section(28, 86, L"Schedule");
        section(28, 334, L"Apply theme to");
        section(28, 454, L"System and startup");

        auto label = [&](int x, int y, int w, const wchar_t* value) {
            RECT box{px(u, x), px(u, y), px(u, x + w), px(u, y + 26)};
            text(dc, value, box, u->colSub, u->fontBody, DT_LEFT | DT_VCENTER | DT_SINGLELINE);
        };

        label(28, 156, 140, L"Light from");
        label(28, 186, 140, L"Dark from");
        label(236, 156, 10, L":");
        label(236, 186, 10, L":");
        label(28, 228, 54, L"Latitude");
        label(200, 228, 64, L"Longitude");
        RECT hint{px(u, 28), px(u, 262), px(u, 432), px(u, 302)};
        const bool missingCoords = u->cfg.mode == Mode::Solar && u->cfg.lat == 0.0 && u->cfg.lon == 0.0;
        const wchar_t* hintText = hintFor(u->cfg.mode);
        COLORREF hintColor = u->colSub;
        if (u->badInput)
        {
            hintText = L"Invalid number. Use digits and a dot, for example -34.6037 (a comma works too).";
            hintColor = u->colAccent;
        }
        else if (missingCoords)
        {
            hintText = L"Enter your latitude and longitude: without them the solar schedule is useless.";
            hintColor = u->colAccent;
        }
        text(dc, hintText, hint, hintColor, u->fontSmall, DT_LEFT | DT_TOP | DT_WORDBREAK);
    }

    bool checkValue(const Ui* u, int id)
    {
        switch (id)
        {
        case ID_SYS:
            return u->cfg.targetSystem;
        case ID_APPS:
            return u->cfg.targetApps;
        case ID_AUTO:
            return u->autoStart;
        default:
            return false;
        }
    }

    void drawItem(Ui* u, const DRAWITEMSTRUCT* item)
    {
        HDC dc = item->hDC;
        RECT box = item->rcItem;
        const int id = static_cast<int>(item->CtlID);
        const bool pressed = (item->itemState & ODS_SELECTED) != 0;

        if (id >= ID_MODE_OFF && id <= ID_MODE_NL)
        {
            const bool selected = static_cast<int>(u->cfg.mode) == id - ID_MODE_OFF;
            const COLORREF fill = selected ? u->colAccent : u->colCard2;
            const COLORREF border = selected ? u->colAccent : u->colBorder;
            FillRect(dc, &box, u->brushCard);
            roundFill(dc, box, px(u, 8), fill, border);

            wchar_t textValue[32] = {};
            GetWindowTextW(item->hwndItem, textValue, 32);
            text(dc, textValue, box, selected ? u->colOnAccent : u->colText, u->fontBold,
                 DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);
            return;
        }

        if (id == ID_SYS || id == ID_APPS || id == ID_AUTO)
        {
            FillRect(dc, &box, u->brushCard);
            const bool on = checkValue(u, id);
            const int side = px(u, 18);
            RECT marker{box.left + px(u, 2), box.top + ((box.bottom - box.top) - side) / 2,
                        box.left + px(u, 2) + side, box.top + ((box.bottom - box.top) - side) / 2 + side};
            roundFill(dc, marker, px(u, 4), on ? u->colAccent : u->colInput,
                      on ? u->colAccent : u->colBorder);
            if (on)
                checkMark(dc, marker.left, marker.top, side, px(u, 2) < 2 ? 2 : px(u, 2), u->colOnAccent);

            wchar_t textValue[128] = {};
            GetWindowTextW(item->hwndItem, textValue, 128);
            RECT labelBox{box.left + px(u, 28), box.top, box.right, box.bottom};
            text(dc, textValue, labelBox, u->colText, u->fontBody,
                 DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);
            return;
        }

        const COLORREF fill = pressed ? u->colCard2 : u->colCard;
        roundFill(dc, box, px(u, 8), fill, u->colBorder);
        wchar_t textValue[64] = {};
        GetWindowTextW(item->hwndItem, textValue, 64);
        text(dc, textValue, box, u->colText, u->fontBody,
             DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);
    }

    void onModeClicked(Ui* u, Mode mode)
    {
        u->cfg.mode = mode;
        commit(u);
        for (HWND button : u->hMode)
            InvalidateRect(button, nullptr, FALSE);
    }

    LRESULT CALLBACK settingsProc(HWND hwnd, UINT msg, WPARAM wparam, LPARAM lparam)
    {
        Ui* u = stateOf(hwnd);

        switch (msg)
        {
        case WM_ERASEBKGND:
            return 1;

        case WM_PAINT:
        {
            PAINTSTRUCT ps{};
            HDC dc = BeginPaint(hwnd, &ps);
            if (u)
                paintChrome(u, dc);
            EndPaint(hwnd, &ps);
            return 0;
        }

        case WM_DRAWITEM:
            if (u)
            {
                drawItem(u, reinterpret_cast<const DRAWITEMSTRUCT*>(lparam));
                return TRUE;
            }
            return FALSE;

        case WM_CTLCOLORBTN:
            if (u)
            {
                const int id = GetDlgCtrlID(reinterpret_cast<HWND>(lparam));
                const bool footer = id == ID_CLOSE || id == ID_RESET;
                return reinterpret_cast<LRESULT>(footer ? u->brushBg : u->brushCard);
            }
            break;

        case WM_CTLCOLOREDIT:
        case WM_CTLCOLORLISTBOX:
            if (u)
            {
                HDC dc = reinterpret_cast<HDC>(wparam);
                SetTextColor(dc, u->colText);
                SetBkColor(dc, u->colInput);
                return reinterpret_cast<LRESULT>(u->brushInput);
            }
            break;

        case WM_CTLCOLORSTATIC:
            if (u)
            {
                HDC dc = reinterpret_cast<HDC>(wparam);
                SetTextColor(dc, IsWindowEnabled(reinterpret_cast<HWND>(lparam)) ? u->colText : u->colSub);
                SetBkColor(dc, u->colCard);
                return reinterpret_cast<LRESULT>(u->brushCard);
            }
            break;

        case WM_COMMAND:
            if (!u)
                break;

            switch (LOWORD(wparam))
            {
            case ID_MODE_OFF:
            case ID_MODE_FIXED:
            case ID_MODE_SOLAR:
            case ID_MODE_NL:
                if (HIWORD(wparam) != BN_CLICKED)
                    return 0;
                onModeClicked(u, static_cast<Mode>(LOWORD(wparam) - ID_MODE_OFF));
                return 0;

            case ID_LIGHT_H:
            case ID_LIGHT_M:
            case ID_DARK_H:
            case ID_DARK_M:
                if (HIWORD(wparam) == CBN_SELCHANGE && !u->loading)
                {
                    readTimes(u);
                    commit(u);
                }
                return 0;

            case ID_LAT:
            case ID_LON:
                if (HIWORD(wparam) == EN_CHANGE && !u->loading)
                {
                    const bool valid = readEditors(u);
                    u->badInput = !valid;
                    if (valid)
                        commit(u);
                    InvalidateRect(u->hwnd, nullptr, FALSE);
                }
                else if (HIWORD(wparam) == EN_KILLFOCUS && !u->loading)
                {
                    const bool valid = readEditors(u);
                    u->badInput = !valid;
                    if (valid)
                        commit(u);
                    else
                        syncControls(u); // restore the last accepted value
                    InvalidateRect(u->hwnd, nullptr, FALSE);
                }
                return 0;

            case ID_SYS:
            case ID_APPS:
                if (HIWORD(wparam) != BN_CLICKED)
                    return 0;
                if (LOWORD(wparam) == ID_SYS)
                    u->cfg.targetSystem = !u->cfg.targetSystem;
                else
                    u->cfg.targetApps = !u->cfg.targetApps;
                commit(u);
                InvalidateRect(reinterpret_cast<HWND>(lparam), nullptr, FALSE);
                return 0;

            case ID_AUTO:
                if (HIWORD(wparam) != BN_CLICKED)
                    return 0;
                u->autoStart = !u->autoStart;
                autostartSet(u->autoStart);
                InvalidateRect(u->hAuto, nullptr, FALSE);
                return 0;

            case ID_RESET:
                if (HIWORD(wparam) != BN_CLICKED)
                    return 0;
                cfgDefaults(u->cfg);
                cfgSave(u->cfg);
                appConfigChanged();
                syncControls(u);
                return 0;

            case ID_CLOSE:
                if (HIWORD(wparam) != BN_CLICKED)
                    return 0;
                DestroyWindow(hwnd);
                return 0;

            default:
                break;
            }
            break;

        case WM_KEYDOWN:
            if (wparam == VK_ESCAPE)
            {
                DestroyWindow(hwnd);
                return 0;
            }
            break;

        case WM_DPICHANGED:
            if (u)
            {
                u->dpi = HIWORD(wparam);
                buildFonts(u);
                const RECT* suggested = reinterpret_cast<const RECT*>(lparam);
                SetWindowPos(hwnd, nullptr, suggested->left, suggested->top, suggested->right - suggested->left,
                             suggested->bottom - suggested->top, SWP_NOZORDER | SWP_NOACTIVATE);
                layoutChildren(u);
                InvalidateRect(hwnd, nullptr, TRUE);
            }
            return 0;

        case WM_CLOSE:
            DestroyWindow(hwnd);
            return 0;

        case WM_DESTROY:
            u = stateOf(hwnd);
            if (u)
            {
                if (gUi == u)
                    gUi = nullptr;
                dropFonts(u);
                dropBrushes(u);
                if (u->hIcon)
                    DestroyIcon(u->hIcon);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                delete u;
                appTrimMemory();
            }
            return 0;

        default:
            break;
        }

        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }

    HWND makeControl(Ui* u, const wchar_t* klass, const wchar_t* value, DWORD style, DWORD extra, int id)
    {
        HWND control = CreateWindowExW(extra, klass, value, WS_CHILD | WS_VISIBLE | style, 0, 0, 0, 0, u->hwnd,
                                       reinterpret_cast<HMENU>(static_cast<INT_PTR>(id)),
                                       reinterpret_cast<HINSTANCE>(GetWindowLongPtrW(u->hwnd, GWLP_HINSTANCE)),
                                       nullptr);
        if (control)
            SendMessageW(control, WM_SETFONT, reinterpret_cast<WPARAM>(u->fontBody), TRUE);
        return control;
    }

    HWND makeCombo(Ui* u, int id, int count)
    {
        HWND combo = makeControl(u, L"COMBOBOX", nullptr, CBS_DROPDOWNLIST | WS_VSCROLL | WS_TABSTOP,
                                 WS_EX_CLIENTEDGE, id);
        if (!combo)
            return nullptr;
        wchar_t item[8];
        for (int i = 0; i < count; ++i)
        {
            swprintf_s(item, L"%02d", i);
            SendMessageW(combo, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(item));
        }
        return combo;
    }

    void createChildren(Ui* u)
    {
        // Creating the edit controls fires EN_CHANGE with their placeholder
        // text, which would be read back and saved over the real settings.
        u->loading = true;
        struct ModeButton
        {
            int id;
            const wchar_t* label;
        };
        const ModeButton modes[] = {
            {ID_MODE_OFF, L"Off"},
            {ID_MODE_FIXED, L"Fixed hours"},
            {ID_MODE_SOLAR, L"Solar"},
            {ID_MODE_NL, L"Night light"},
        };
        for (int i = 0; i < 4; ++i)
        {
            u->hMode[i] = makeControl(u, L"BUTTON", modes[i].label,
                                      BS_OWNERDRAW | WS_TABSTOP, 0, modes[i].id);
        }

        u->hLightH = makeCombo(u, ID_LIGHT_H, 24);
        u->hLightM = makeCombo(u, ID_LIGHT_M, 60);
        u->hDarkH = makeCombo(u, ID_DARK_H, 24);
        u->hDarkM = makeCombo(u, ID_DARK_M, 60);

        u->hLat = makeControl(u, L"EDIT", L"0", ES_AUTOHSCROLL | WS_TABSTOP, WS_EX_CLIENTEDGE, ID_LAT);
        u->hLon = makeControl(u, L"EDIT", L"0", ES_AUTOHSCROLL | WS_TABSTOP, WS_EX_CLIENTEDGE, ID_LON);

        u->hSys = makeControl(u, L"BUTTON", L"Windows UI (taskbar, Start)",
                              BS_OWNERDRAW | WS_TABSTOP, 0, ID_SYS);
        u->hApps = makeControl(u, L"BUTTON", L"Applications and windows", BS_OWNERDRAW | WS_TABSTOP, 0, ID_APPS);
        u->hAuto = makeControl(u, L"BUTTON", L"Start with Windows", BS_OWNERDRAW | WS_TABSTOP, 0, ID_AUTO);

        u->hReset = makeControl(u, L"BUTTON", L"Reset", BS_OWNERDRAW | WS_TABSTOP, 0, ID_RESET);
        u->hClose = makeControl(u, L"BUTTON", L"Close", BS_OWNERDRAW | WS_TABSTOP, 0, ID_CLOSE);

        applyControlTheme(u);
        u->loading = false;
    }

    void showWindow(Ui* u)
    {
        u->loading = true;
        cfgLoad(u->cfg);
        u->autoStart = autostartGet();
        u->loading = false;

        buildPalette(u);
        applyControlTheme(u);
        syncControls(u);
        syncEnabled(u);

        ShowWindow(u->hwnd, SW_SHOWNORMAL);
        SetForegroundWindow(u->hwnd);
        InvalidateRect(u->hwnd, nullptr, TRUE);
    }
}

void uiShow(HWND owner)
{
    (void)owner;

    if (gUi && gUi->hwnd)
    {
        showWindow(gUi);
        return;
    }

    HINSTANCE instance = GetModuleHandleW(nullptr);

    Ui* u = new Ui{};
    if (!u)
        return;

    u->dpi = GetDpiForSystem();

    WNDCLASSEXW wc{};
    wc.cbSize = sizeof(wc);
    wc.lpfnWndProc = settingsProc;
    wc.hInstance = instance;
    wc.hCursor = LoadCursorW(nullptr, IDC_ARROW);
    wc.lpszClassName = kClassName;
    wc.hIcon = static_cast<HICON>(LoadImageW(instance, MAKEINTRESOURCEW(1), IMAGE_ICON, 32, 32, 0));
    wc.hIconSm = static_cast<HICON>(LoadImageW(instance, MAKEINTRESOURCEW(1), IMAGE_ICON,
                                               GetSystemMetrics(SM_CXSMICON), GetSystemMetrics(SM_CYSMICON), 0));

    if (!gClassReady)
    {
        if (!RegisterClassExW(&wc) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS)
        {
            delete u;
            return;
        }
        gClassReady = true;
    }

    cfgLoad(u->cfg);
    u->autoStart = autostartGet();
    u->hIcon = static_cast<HICON>(LoadImageW(instance, MAKEINTRESOURCEW(1), IMAGE_ICON, GetSystemMetrics(SM_CXICON),
                                             GetSystemMetrics(SM_CYICON), 0));

    RECT box{0, 0, px(u, kWidth), px(u, kHeight)};
    AdjustWindowRectExForDpi(&box, WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU, FALSE, 0, u->dpi);

    u->hwnd = CreateWindowExW(WS_EX_APPWINDOW, kClassName, L"LightSwitch Lite \u2014 Settings",
                              WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU, CW_USEDEFAULT, CW_USEDEFAULT,
                              box.right - box.left, box.bottom - box.top, nullptr, nullptr, instance, nullptr);
    if (!u->hwnd)
    {
        if (u->hIcon)
            DestroyIcon(u->hIcon);
        delete u;
        return;
    }

    SetWindowLongPtrW(u->hwnd, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(u));
    gUi = u;

    u->dpi = GetDpiForWindow(u->hwnd);
    buildFonts(u);
    buildPalette(u);

    createChildren(u);
    layoutWindow(u, true);
    syncControls(u);

    ShowWindow(u->hwnd, SW_SHOWNORMAL);
    UpdateWindow(u->hwnd);
}

void uiRefresh()
{
    if (!gUi || !gUi->hwnd || !IsWindowVisible(gUi->hwnd))
        return;

    gUi->loading = true;
    cfgLoad(gUi->cfg);
    gUi->autoStart = autostartGet();
    gUi->loading = false;

    buildPalette(gUi);
    applyControlTheme(gUi);
    syncControls(gUi);
    InvalidateRect(gUi->hwnd, nullptr, TRUE);
}

void uiThemeChanged()
{
    if (!gUi || !gUi->hwnd || !IsWindowVisible(gUi->hwnd))
        return;
    buildPalette(gUi);
    applyControlTheme(gUi);
    InvalidateRect(gUi->hwnd, nullptr, TRUE);
}

void uiDestroy()
{
    if (gUi && gUi->hwnd)
        DestroyWindow(gUi->hwnd);
    gUi = nullptr;
}
