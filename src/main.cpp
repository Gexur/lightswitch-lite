#include "core.h"
#include "ipc.h"

#include <shellapi.h>
#include <cstdio>
#include <cstring>
#include <cwchar>

namespace
{
    constexpr UINT kTrayMessage = WM_APP + 10;
    constexpr UINT kShowMessage = WM_APP + 11;
    constexpr UINT kTrayId = 1;
    constexpr UINT kTimerId = 1;
    constexpr UINT kHotkeyId = 1;
    constexpr UINT kHotkeyMods = MOD_WIN | MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT;
    constexpr UINT kHotkeyKey = 'D';

    HINSTANCE gInstance = nullptr;
    HWND gMain = nullptr;
    HANDLE gSingleMutex = nullptr;
    HANDLE gMapping = nullptr;
    LslStatus* gStatus = nullptr;

    Config gCfg;
    bool gOverride = false;
    bool gHasNight = false;
    bool gNightOn = false;
    bool gHasEval = false;
    ULONGLONG gEvalStamp = 0;
    bool gHasObsSys = false;
    bool gObsSys = false;
    bool gHasObsApps = false;
    bool gObsApps = false;

    HKEY gPersKey = nullptr;
    HANDLE gPersEvent = nullptr;
    bool gPersArmed = false;
    HKEY gNightKey = nullptr;
    HANDLE gNightEvent = nullptr;
    bool gNightArmed = false;

    UINT gTaskbarCreated = 0;
    bool gTrayAdded = false;
    HICON gIconSmall = nullptr;
    HICON gIconLarge = nullptr;

    DWORD unixNowPlus(DWORD ms)
    {
        FILETIME ft{};
        GetSystemTimeAsFileTime(&ft);
        ULARGE_INTEGER value;
        value.LowPart = ft.dwLowDateTime;
        value.HighPart = ft.dwHighDateTime;

        constexpr ULONGLONG kEpoch = 116444736000000000ULL;
        if (value.QuadPart < kEpoch)
            return 0;
        const ULONGLONG seconds = (value.QuadPart - kEpoch) / 10000000ULL + (ms + 999ULL) / 1000ULL;
        return static_cast<DWORD>(seconds);
    }

    void publish()
    {
        if (!gStatus)
            return;

        gStatus->magic = LSL_STATUS_MAGIC;
        gStatus->size = sizeof(LslStatus);
        gStatus->version = 1;
        gStatus->pid = GetCurrentProcessId();
        gStatus->mode = static_cast<DWORD>(gCfg.mode);
        gStatus->changeSystem = gCfg.targetSystem ? 1u : 0u;
        gStatus->changeApps = gCfg.targetApps ? 1u : 0u;
        gStatus->manualOverride = gOverride ? 1u : 0u;
        gStatus->nightLight = gHasNight ? (gNightOn ? 1u : 0u) : 2u;

        bool light = false;
        gStatus->systemLight = themeRead(true, light) ? (light ? 1u : 0u) : 2u;
        gStatus->appsLight = themeRead(false, light) ? (light ? 1u : 0u) : 2u;

        SYSTEMTIME now{};
        GetLocalTime(&now);
        bool toLight = false;
        const DWORD ms = msToNextChange(gCfg, now, toLight);
        gStatus->nextChange = ms ? unixNowPlus(ms) : 0u;
    }

    void recordObservations()
    {
        bool value = false;
        gHasObsSys = themeRead(true, value);
        gObsSys = value;
        gHasObsApps = themeRead(false, value);
        gObsApps = value;
    }

    void updateTrayTip()
    {
        if (!gTrayAdded)
            return;

        bool light = false;
        const bool known = themeRead(true, light);
        wchar_t tip[128];
        if (!known)
            swprintf_s(tip, L"%s - theme unknown", LSL_APP);
        else
            swprintf_s(tip, L"%s - now %s", LSL_APP, light ? L"light" : L"dark");

        NOTIFYICONDATAW data{};
        data.cbSize = sizeof(data);
        data.hWnd = gMain;
        data.uID = kTrayId;
        data.uFlags = NIF_TIP;
        wcscpy_s(data.szTip, tip);
        Shell_NotifyIconW(NIM_MODIFY, &data);
    }

    void addTrayIcon()
    {
        NOTIFYICONDATAW data{};
        data.cbSize = sizeof(data);
        data.hWnd = gMain;
        data.uID = kTrayId;
        data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        data.uCallbackMessage = kTrayMessage;
        data.hIcon = gIconSmall;
        swprintf_s(data.szTip, L"%s", LSL_APP);
        Shell_NotifyIconW(gTrayAdded ? NIM_MODIFY : NIM_ADD, &data);
        gTrayAdded = true;
        updateTrayTip();
    }

    void removeTrayIcon()
    {
        if (!gTrayAdded)
            return;
        NOTIFYICONDATAW data{};
        data.cbSize = sizeof(data);
        data.hWnd = gMain;
        data.uID = kTrayId;
        Shell_NotifyIconW(NIM_DELETE, &data);
        gTrayAdded = false;
    }

    bool targetWantsLight(const SYSTEMTIME& now)
    {
        if (gCfg.mode == Mode::NightLight)
            return gHasNight ? !gNightOn : true;

        int lightMin = 0;
        int darkMin = 0;
        effTimes(gCfg, now, lightMin, darkMin);
        return isLightAt(now.wHour * 60 + now.wMinute, lightMin, darkMin);
    }

    bool applyThemeNow(bool light)
    {
        if (!gCfg.targetSystem && !gCfg.targetApps)
            return true;

        bool current = false;
        const bool needSystem = gCfg.targetSystem && (!themeRead(true, current) || current != light);
        const bool needApps = gCfg.targetApps && (!themeRead(false, current) || current != light);

        if (!needSystem && !needApps)
        {
            recordObservations();
            return true;
        }

        bool systemOk = true;
        bool appsOk = true;
        themeApply(light, needSystem, needApps, systemOk, appsOk);
        recordObservations();
        return systemOk && appsOk;
    }

    void resetTimer()
    {
        KillTimer(gMain, kTimerId);

        if (gCfg.mode == Mode::Off)
            return;

        if (gCfg.mode == Mode::NightLight)
        {
            // Without a reliable registry notification, poll slowly so state is not lost.
            if (!gNightArmed)
                SetTimer(gMain, kTimerId, 60000, nullptr);
            return;
        }

        SYSTEMTIME now{};
        GetLocalTime(&now);
        bool toLight = false;
        DWORD ms = msToNextChange(gCfg, now, toLight);
        if (ms == 0)
            return;

        if (gCfg.mode == Mode::Solar)
        {
            const long secondsLeft = 86400L - (now.wHour * 3600L + now.wMinute * 60L + now.wSecond);
            const DWORD toMidnight = static_cast<DWORD>(secondsLeft * 1000L - now.wMilliseconds);
            if (toMidnight < ms)
                ms = toMidnight; // sun times change with the date
        }

        if (ms < 200)
            ms = 200;
        SetTimer(gMain, kTimerId, ms + 200, nullptr);
    }

    void armPersonalize()
    {
        if (!gPersKey)
            gPersKey = regOpenPersonalize(KEY_NOTIFY | KEY_QUERY_VALUE);
        if (!gPersKey)
            return;
        if (!gPersEvent)
            gPersEvent = CreateEventW(nullptr, TRUE, FALSE, nullptr);
        if (!gPersEvent)
            return;

        ResetEvent(gPersEvent);
        gPersArmed = RegNotifyChangeKeyValue(gPersKey, FALSE, REG_NOTIFY_CHANGE_LAST_SET, gPersEvent, TRUE) ==
                     ERROR_SUCCESS;
    }

    void armNightLight()
    {
        if (!gNightKey)
            gNightKey = regOpenNightLight(KEY_NOTIFY | KEY_QUERY_VALUE);
        if (!gNightKey)
            return;
        if (!gNightEvent)
            gNightEvent = CreateEventW(nullptr, TRUE, FALSE, nullptr);
        if (!gNightEvent)
            return;

        ResetEvent(gNightEvent);
        gNightArmed = RegNotifyChangeKeyValue(gNightKey, FALSE, REG_NOTIFY_CHANGE_LAST_SET, gNightEvent, TRUE) ==
                      ERROR_SUCCESS;
    }

    void configureNightLight()
    {
        if (gCfg.mode == Mode::NightLight)
        {
            armNightLight();
            return;
        }

        gNightArmed = false;
        if (gNightKey)
        {
            RegCloseKey(gNightKey);
            gNightKey = nullptr;
        }
    }

    void evaluate()
    {
        SYSTEMTIME now{};
        GetLocalTime(&now);

        if (gCfg.mode == Mode::Off)
        {
            gHasEval = true;
            gEvalStamp = stampOf(now);
            publish();
            return;
        }

        if (gCfg.mode == Mode::NightLight)
        {
            bool on = false;
            if (nightLightRead(on))
            {
                if (!gHasNight || gNightOn != on)
                {
                    gHasNight = true;
                    gNightOn = on;
                    gOverride = false;
                }
                if (!gOverride)
                    applyThemeNow(!on);
            }
            gHasEval = true;
            gEvalStamp = stampOf(now);
            publish();
            return;
        }

        if (gOverride)
        {
            if (gHasEval && crossedBoundary(gCfg, gEvalStamp, now))
            {
                gOverride = false;
            }
            else
            {
                gEvalStamp = stampOf(now);
                gHasEval = true;
                publish();
                return;
            }
        }

        applyThemeNow(targetWantsLight(now));
        gHasEval = true;
        gEvalStamp = stampOf(now);
        publish();
    }

    bool setThemeManual(bool light)
    {
        if (!gCfg.targetSystem && !gCfg.targetApps)
            return false;

        SYSTEMTIME now{};
        GetLocalTime(&now);

        const bool ok = applyThemeNow(light);
        if (gCfg.mode != Mode::Off)
            gOverride = light != targetWantsLight(now);
        gEvalStamp = stampOf(now);
        gHasEval = true;

        resetTimer();
        publish();
        updateTrayTip();
        uiThemeChanged();
        return ok;
    }

    bool toggleTheme()
    {
        bool value = false;
        bool known = false;
        if (gCfg.targetSystem && themeRead(true, value))
            known = true;
        else if (gCfg.targetApps && themeRead(false, value))
            known = true;

        if (!known)
            return false;
        return setThemeManual(!value);
    }

    bool setMode(Mode mode)
    {
        gCfg.mode = mode;
        cfgSave(gCfg);
        gOverride = false;
        configureNightLight();
        evaluate();
        resetTimer();
        updateTrayTip();
        uiRefresh();
        return true;
    }

    void registerHotkey()
    {
        UnregisterHotKey(gMain, kHotkeyId);
        if (gCfg.hotkeyOn)
            RegisterHotKey(gMain, kHotkeyId, kHotkeyMods, kHotkeyKey);
    }

    void onPersonalizeChanged()
    {
        bool systemLight = false;
        bool appsLight = false;
        const bool hasSystem = themeRead(true, systemLight);
        const bool hasApps = themeRead(false, appsLight);
        if (!hasSystem && !hasApps)
            return;

        if (gCfg.mode != Mode::Off && !gOverride)
        {
            bool want = true;
            if (gCfg.mode == Mode::NightLight && !gHasNight)
            {
                recordObservations();
                publish();
                return;
            }
            SYSTEMTIME now{};
            GetLocalTime(&now);
            want = targetWantsLight(now);

            const bool mismatch = (gCfg.targetSystem && hasSystem && systemLight != want) ||
                                  (gCfg.targetApps && hasApps && appsLight != want);
            if (mismatch)
            {
                // The user changed the theme by hand: honour it until the next boundary.
                gOverride = true;
                gHasEval = true;
                gEvalStamp = stampOf(now);
            }
        }

        recordObservations();
        publish();
        updateTrayTip();
        uiThemeChanged();
    }

    void onNightLightChanged()
    {
        bool on = false;
        if (!nightLightRead(on))
        {
            publish();
            return;
        }

        const bool changed = !gHasNight || gNightOn != on;
        gHasNight = true;
        gNightOn = on;

        if (gCfg.mode == Mode::NightLight && changed)
        {
            gOverride = false;
            applyThemeNow(!on);
            SYSTEMTIME now{};
            GetLocalTime(&now);
            gHasEval = true;
            gEvalStamp = stampOf(now);
            resetTimer();
        }

        publish();
        updateTrayTip();
        uiThemeChanged();
    }

    void showTrayMenu(HWND owner)
    {
        HMENU menu = CreatePopupMenu();
        HMENU modes = CreatePopupMenu();
        if (!menu || !modes)
        {
            if (modes)
                DestroyMenu(modes);
            if (menu)
                DestroyMenu(menu);
            return;
        }

        struct Entry
        {
            Mode mode;
            const wchar_t* label;
        };
        const Entry entries[] = {
            {Mode::Off, L"Off"},
            {Mode::Fixed, L"Fixed hours"},
            {Mode::Solar, L"Sunrise and sunset"},
            {Mode::NightLight, L"Night light"},
        };
        for (int i = 0; i < 4; ++i)
        {
            AppendMenuW(modes, MF_STRING | (gCfg.mode == entries[i].mode ? MF_CHECKED : 0), 100 + i,
                        entries[i].label);
        }

        AppendMenuW(menu, MF_POPUP, reinterpret_cast<UINT_PTR>(modes), L"Schedule");
        AppendMenuW(menu, MF_SEPARATOR, 0, nullptr);
        AppendMenuW(menu, MF_STRING, 110, L"Light theme");
        AppendMenuW(menu, MF_STRING, 111, L"Dark theme");
        AppendMenuW(menu, MF_STRING, 112, L"Toggle theme\tWin+Ctrl+Shift+D");
        AppendMenuW(menu, MF_SEPARATOR, 0, nullptr);
        AppendMenuW(menu, MF_STRING, 120, L"Settings...");
        AppendMenuW(menu, MF_STRING | (autostartGet() ? MF_CHECKED : 0), 121, L"Start with Windows");
        AppendMenuW(menu, MF_SEPARATOR, 0, nullptr);
        AppendMenuW(menu, MF_STRING, 122, L"Exit");

        POINT cursor{};
        GetCursorPos(&cursor);
        SetForegroundWindow(owner);
        TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_BOTTOMALIGN, cursor.x, cursor.y, 0, owner, nullptr);
        PostMessageW(owner, WM_NULL, 0, 0);
        DestroyMenu(menu);
    }

    LRESULT handleCommand(const LslIpc& request)
    {
        switch (request.cmd)
        {
        case LSL_CMD_PING:
        case LSL_CMD_REFRESH:
            publish();
            return 1;
        case LSL_CMD_SHOW:
            PostMessageW(gMain, kShowMessage, 0, 0);
            return 1;
        case LSL_CMD_LIGHT:
            return setThemeManual(true) ? 1 : 0;
        case LSL_CMD_DARK:
            return setThemeManual(false) ? 1 : 0;
        case LSL_CMD_TOGGLE:
            return toggleTheme() ? 1 : 0;
        case LSL_CMD_MODE:
            if (request.arg > 3)
                return 0;
            return setMode(static_cast<Mode>(request.arg)) ? 1 : 0;
        case LSL_CMD_QUIT:
            PostQuitMessage(0);
            return 1;
        default:
            return 0;
        }
    }

    LRESULT CALLBACK mainWindowProc(HWND hwnd, UINT msg, WPARAM wparam, LPARAM lparam)
    {
        if (msg == gTaskbarCreated && gTaskbarCreated != 0)
        {
            gTrayAdded = false;
            addTrayIcon();
            return 0;
        }

        switch (msg)
        {
        case kTrayMessage:
            switch (LOWORD(lparam))
            {
            case WM_RBUTTONUP:
            case WM_CONTEXTMENU:
                showTrayMenu(hwnd);
                return 0;
            case WM_LBUTTONDBLCLK:
                uiShow(hwnd);
                return 0;
            default:
                return 0;
            }

        case kShowMessage:
            uiShow(hwnd);
            return 0;

        case WM_TIMER:
            if (wparam == kTimerId)
            {
                evaluate();
                resetTimer();
            }
            return 0;

        case WM_HOTKEY:
            if (wparam == kHotkeyId)
                toggleTheme();
            return 0;

        case WM_POWERBROADCAST:
            if (wparam == PBT_APMRESUMEAUTOMATIC || wparam == PBT_APMRESUMESUSPEND)
            {
                armPersonalize();
                if (gCfg.mode == Mode::NightLight)
                    armNightLight();
                evaluate();
                resetTimer();
            }
            return TRUE;

        case WM_TIMECHANGE:
            evaluate();
            resetTimer();
            return 0;

        case WM_COPYDATA:
        {
            const auto* copy = reinterpret_cast<const COPYDATASTRUCT*>(lparam);
            if (copy && copy->cbData == sizeof(LslIpc) && copy->lpData)
            {
                LslIpc request{};
                memcpy(&request, copy->lpData, sizeof(request));
                if (request.magic == LSL_IPC_MAGIC)
                    return handleCommand(request);
            }
            return 0;
        }

        case WM_COMMAND:
        {
            const int id = LOWORD(wparam);
            if (id >= 100 && id <= 103)
                setMode(static_cast<Mode>(id - 100));
            else if (id == 110)
                setThemeManual(true);
            else if (id == 111)
                setThemeManual(false);
            else if (id == 112)
                toggleTheme();
            else if (id == 120)
                uiShow(hwnd);
            else if (id == 121)
            {
                autostartSet(!autostartGet());
                uiRefresh();
            }
            else if (id == 122)
                PostQuitMessage(0);
            return 0;
        }

        case WM_QUERYENDSESSION:
            return TRUE;

        case WM_ENDSESSION:
            if (wparam)
                PostQuitMessage(0);
            return 0;

        case WM_CLOSE:
            return 0;

        case WM_DESTROY:
            PostQuitMessage(0);
            return 0;

        default:
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
    }

    void describeStatus(wchar_t* out, int cch)
    {
        bool light = false;
        const bool known = themeRead(true, light);

        SYSTEMTIME now{};
        GetLocalTime(&now);
        bool toLight = false;
        DWORD ms = msToNextChange(gCfg, now, toLight);

        wchar_t when[16] = {};
        if (ms)
        {
            FILETIME ft{};
            SystemTimeToFileTime(&now, &ft);
            ULARGE_INTEGER value;
            value.LowPart = ft.dwLowDateTime;
            value.HighPart = ft.dwHighDateTime;
            value.QuadPart += static_cast<ULONGLONG>(ms) * 10000ULL;
            ft.dwLowDateTime = value.LowPart;
            ft.dwHighDateTime = value.HighPart;

            SYSTEMTIME target{};
            FileTimeToSystemTime(&ft, &target);
            swprintf_s(when, L"%02d:%02d", target.wHour, target.wMinute);
        }

        const wchar_t* nowText = known ? (light ? L"light" : L"dark") : L"unknown";

        if (gCfg.mode == Mode::Off)
        {
            swprintf_s(out, cch, L"Now %s \u00b7 schedule off", nowText);
        }
        else if (gCfg.mode == Mode::NightLight)
        {
            const wchar_t* nl = gHasNight ? (gNightOn ? L"on" : L"off") : L"unknown";
            swprintf_s(out, cch, L"Now %s \u00b7 night light %s%s", nowText, nl,
                       gOverride ? L" \u00b7 manual" : L"");
        }
        else if (ms)
        {
            swprintf_s(out, cch, L"Now %s \u00b7 %s at %s%s", nowText, toLight ? L"light" : L"dark", when,
                       gOverride ? L" \u00b7 manual" : L"");
        }
        else
        {
            swprintf_s(out, cch, L"Now %s \u00b7 no scheduled changes", nowText);
        }
    }
}

void appDescribe(wchar_t* out, int cch)
{
    describeStatus(out, cch);
}

// Drops the process out of resident memory while it waits. Safe because the next
// event (schedule, tray or CLI) pages back only what it touches.
void appTrimMemory()
{
    SetProcessWorkingSetSizeEx(GetCurrentProcess(), static_cast<SIZE_T>(-1), static_cast<SIZE_T>(-1), 0);
}

void appShowSettings()
{
    if (gMain)
        PostMessageW(gMain, kShowMessage, 0, 0);
}

void appConfigChanged()
{
    cfgLoad(gCfg);
    registerHotkey();
    gOverride = false;
    configureNightLight();
    evaluate();
    resetTimer();
    updateTrayTip();
    publish();
}

int WINAPI wWinMain(HINSTANCE instance, HINSTANCE, PWSTR, int)
{
    gInstance = instance;

    bool background = false;
    DWORD command = LSL_CMD_SHOW;
    DWORD modeArg = 0;
    bool hasMode = false;

    int argc = 0;
    LPWSTR* argv = CommandLineToArgvW(GetCommandLineW(), &argc);
    for (int i = 1; argv && i < argc; ++i)
    {
        if (!lstrcmpiW(argv[i], L"--background") || !lstrcmpiW(argv[i], L"--minimized"))
            background = true;
        else if (!lstrcmpiW(argv[i], L"--light"))
            command = LSL_CMD_LIGHT;
        else if (!lstrcmpiW(argv[i], L"--dark"))
            command = LSL_CMD_DARK;
        else if (!lstrcmpiW(argv[i], L"--toggle"))
            command = LSL_CMD_TOGGLE;
        else if (!lstrcmpiW(argv[i], L"--mode") && i + 1 < argc)
        {
            const wchar_t* name = argv[++i];
            if (!lstrcmpiW(name, L"off"))
            {
                hasMode = true;
                modeArg = 0;
            }
            else if (!lstrcmpiW(name, L"fixed"))
            {
                hasMode = true;
                modeArg = 1;
            }
            else if (!lstrcmpiW(name, L"solar") || !lstrcmpiW(name, L"sunrise"))
            {
                hasMode = true;
                modeArg = 2;
            }
            else if (!lstrcmpiW(name, L"nightlight"))
            {
                hasMode = true;
                modeArg = 3;
            }
        }
    }
    if (argv)
        LocalFree(argv);

    gSingleMutex = CreateMutexW(nullptr, TRUE, LSL_MUTEX);
    const bool alreadyRunning = gSingleMutex != nullptr && GetLastError() == ERROR_ALREADY_EXISTS;

    if (alreadyRunning)
    {
        // A second silent start (for example a duplicated autostart entry) must not
        // pop the window open.
        if (background && !hasMode && command == LSL_CMD_SHOW)
            return 0;

        const HWND other = FindWindowW(LSL_WNDCLASS, nullptr);
        if (other)
        {
            LslIpc request{LSL_IPC_MAGIC, hasMode ? LSL_CMD_MODE : command, modeArg};
            COPYDATASTRUCT data{};
            data.dwData = 0;
            data.cbData = sizeof(request);
            data.lpData = &request;
            SendMessageW(other, WM_COPYDATA, 0, reinterpret_cast<LPARAM>(&data));
        }
        return 0;
    }

    cfgLoad(gCfg);

    WNDCLASSEXW wc{};
    wc.cbSize = sizeof(wc);
    wc.lpfnWndProc = mainWindowProc;
    wc.hInstance = instance;
    wc.lpszClassName = LSL_WNDCLASS;
    wc.hCursor = LoadCursorW(nullptr, IDC_ARROW);
    if (!RegisterClassExW(&wc))
        return 1;

    gMain = CreateWindowExW(0, LSL_WNDCLASS, LSL_APP, 0, 0, 0, 0, 0, nullptr, nullptr, instance, nullptr);
    if (!gMain)
        return 1;

    gTaskbarCreated = RegisterWindowMessageW(L"TaskbarCreated");

    gIconLarge = static_cast<HICON>(LoadImageW(instance, MAKEINTRESOURCEW(1), IMAGE_ICON, 0, 0, LR_DEFAULTSIZE));
    gIconSmall = static_cast<HICON>(LoadImageW(instance, MAKEINTRESOURCEW(1), IMAGE_ICON,
                                               GetSystemMetrics(SM_CXSMICON), GetSystemMetrics(SM_CYSMICON), 0));

    gMapping = CreateFileMappingW(INVALID_HANDLE_VALUE, nullptr, PAGE_READWRITE, 0, sizeof(LslStatus), LSL_MAPPING);
    if (gMapping)
        gStatus = static_cast<LslStatus*>(MapViewOfFile(gMapping, FILE_MAP_WRITE, 0, 0, sizeof(LslStatus)));
    if (gStatus)
        ZeroMemory(gStatus, sizeof(*gStatus));

    addTrayIcon();
    registerHotkey();
    armPersonalize();
    resetTimer();
    evaluate();
    configureNightLight();
    publish();

    if (hasMode)
        setMode(static_cast<Mode>(modeArg));
    else if (command == LSL_CMD_LIGHT)
        setThemeManual(true);
    else if (command == LSL_CMD_DARK)
        setThemeManual(false);
    else if (command == LSL_CMD_TOGGLE)
        toggleTheme();
    else if (!background)
        uiShow(gMain);
    else
        appTrimMemory();

    MSG msg{};
    bool quit = false;
    while (!quit)
    {
        HANDLE events[2];
        DWORD count = 0;
        if (gPersArmed)
            events[count++] = gPersEvent;
        if (gNightArmed)
            events[count++] = gNightEvent;

        const DWORD result = MsgWaitForMultipleObjectsEx(count, events, INFINITE, QS_ALLINPUT, MWMO_INPUTAVAILABLE);

        if (result == WAIT_OBJECT_0 + count)
        {
            while (PeekMessageW(&msg, nullptr, 0, 0, PM_REMOVE))
            {
                if (msg.message == WM_QUIT)
                {
                    quit = true;
                    break;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        else if (result < WAIT_OBJECT_0 + count)
        {
            const HANDLE signaled = events[result - WAIT_OBJECT_0];
            if (signaled == gPersEvent)
            {
                gPersArmed = false;
                onPersonalizeChanged();
                armPersonalize();
            }
            else if (signaled == gNightEvent)
            {
                gNightArmed = false;
                onNightLightChanged();
                if (gCfg.mode == Mode::NightLight)
                    armNightLight();
            }
        }
        else
        {
            break;
        }
    }

    removeTrayIcon();
    uiDestroy();
    UnregisterHotKey(gMain, kHotkeyId);

    if (gStatus)
        UnmapViewOfFile(gStatus);
    if (gMapping)
        CloseHandle(gMapping);
    if (gPersEvent)
        CloseHandle(gPersEvent);
    if (gNightEvent)
        CloseHandle(gNightEvent);
    if (gPersKey)
        RegCloseKey(gPersKey);
    if (gNightKey)
        RegCloseKey(gNightKey);

    if (gIconLarge)
        DestroyIcon(gIconLarge);
    if (gIconSmall)
        DestroyIcon(gIconSmall);

    if (gSingleMutex)
    {
        ReleaseMutex(gSingleMutex);
        CloseHandle(gSingleMutex);
    }

    if (gMain)
        DestroyWindow(gMain);

    return 0;
}
