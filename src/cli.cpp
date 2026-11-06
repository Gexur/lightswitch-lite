// lsl.exe: command-line interface. A separate, tiny process.
#include "ipc.h"

#include <cstdio>
#include <cstring>
#include <cwchar>

namespace
{
    constexpr int kExitOk = 0;
    constexpr int kExitFailed = 1;
    constexpr int kExitUsage = 2;
    constexpr int kExitNoService = 3;

    bool sendCommand(HWND window, DWORD command, DWORD argument)
    {
        LslIpc request{};
        request.magic = LSL_IPC_MAGIC;
        request.cmd = command;
        request.arg = argument;

        COPYDATASTRUCT data{};
        data.dwData = 0;
        data.cbData = sizeof(request);
        data.lpData = &request;
        return SendMessageW(window, WM_COPYDATA, 0, reinterpret_cast<LPARAM>(&data)) == 1;
    }

    bool readStatus(LslStatus& status)
    {
        HANDLE mapping = OpenFileMappingW(FILE_MAP_READ, FALSE, LSL_MAPPING);
        if (!mapping)
            return false;

        bool ok = false;
        void* view = MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, sizeof(LslStatus));
        if (view)
        {
            memcpy(&status, view, sizeof(status));
            UnmapViewOfFile(view);
            ok = status.magic == LSL_STATUS_MAGIC && status.size == sizeof(LslStatus);
        }
        CloseHandle(mapping);
        return ok;
    }

    const wchar_t* themeName(DWORD value)
    {
        switch (value)
        {
        case 0:
            return L"dark";
        case 1:
            return L"light";
        default:
            return L"unknown";
        }
    }

    const wchar_t* modeName(DWORD mode)
    {
        switch (mode)
        {
        case 1:
            return L"FixedHours";
        case 2:
            return L"SunsetToSunrise";
        case 3:
            return L"FollowNightLight";
        default:
            return L"Off";
        }
    }

    bool parseMode(const wchar_t* value, DWORD& mode)
    {
        if (!_wcsicmp(value, L"off") || !_wcsicmp(value, L"disabled"))
            mode = 0;
        else if (!_wcsicmp(value, L"fixed") || !_wcsicmp(value, L"fixed-hours") || !_wcsicmp(value, L"fixedhours"))
            mode = 1;
        else if (!_wcsicmp(value, L"solar") || !_wcsicmp(value, L"sunrise") ||
                 !_wcsicmp(value, L"sunset-to-sunrise") || !_wcsicmp(value, L"sunsettosunrise"))
            mode = 2;
        else if (!_wcsicmp(value, L"nightlight") || !_wcsicmp(value, L"follow-night-light") ||
                 !_wcsicmp(value, L"night-light"))
            mode = 3;
        else
            return false;
        return true;
    }

    void printTime(DWORD epoch)
    {
        if (epoch == 0)
        {
            wprintf(L"none scheduled");
            return;
        }

        ULARGE_INTEGER value;
        value.QuadPart = static_cast<ULONGLONG>(epoch) * 10000000ULL + 116444736000000000ULL;
        FILETIME utcFile{};
        utcFile.dwLowDateTime = value.LowPart;
        utcFile.dwHighDateTime = value.HighPart;

        SYSTEMTIME utc{};
        SYSTEMTIME local{};
        if (FileTimeToSystemTime(&utcFile, &utc) && SystemTimeToTzSpecificLocalTime(nullptr, &utc, &local))
            wprintf(L"%02u:%02u", local.wHour, local.wMinute);
        else
            wprintf(L"unknown");
    }

    void printJson(const LslStatus& status)
    {
        wprintf(L"{\"version\":1,\"success\":true,\"state\":{\"systemTheme\":\"%s\",\"appsTheme\":\"%s\","
                L"\"changeSystem\":%s,\"changeApps\":%s,\"scheduleMode\":\"%s\",\"manualOverride\":%s}}\n",
                themeName(status.systemLight), themeName(status.appsLight),
                status.changeSystem ? L"true" : L"false", status.changeApps ? L"true" : L"false",
                modeName(status.mode), status.manualOverride ? L"true" : L"false");
    }

    void printHuman(const LslStatus& status)
    {
        wprintf(L"Mode: %s\n", modeName(status.mode));
        wprintf(L"System theme: %s\n", themeName(status.systemLight));
        wprintf(L"Apps theme: %s\n", themeName(status.appsLight));

        wprintf(L"Targets: ");
        if (status.changeSystem && status.changeApps)
            wprintf(L"system and apps\n");
        else if (status.changeSystem)
            wprintf(L"system\n");
        else if (status.changeApps)
            wprintf(L"apps\n");
        else
            wprintf(L"none\n");

        wprintf(L"Next change: ");
        printTime(status.nextChange);
        wprintf(L"\n");
        wprintf(L"Manual override: %s\n", status.manualOverride ? L"yes" : L"no");
    }

    int usage()
    {
        wprintf(L"LightSwitch Lite - control the Windows theme from a terminal\n\n");
        wprintf(L"  lsl status [--json]        current state\n");
        wprintf(L"  lsl light                  apply the light theme\n");
        wprintf(L"  lsl dark                   apply the dark theme\n");
        wprintf(L"  lsl toggle                 toggle the theme\n");
        wprintf(L"  lsl mode <mode>            off | fixed | solar | nightlight\n");
        wprintf(L"  lsl --version              print the version\n");
        wprintf(L"  lsl --help                 this help\n\n");
        wprintf(L"LightSwitchLite.exe must be running for this command to work.\n");
        return kExitUsage;
    }
}

int wmain(int argc, wchar_t** argv)
{
    if (argc < 2)
        return usage();

    const wchar_t* command = argv[1];

    if (!_wcsicmp(command, L"--help") || !_wcsicmp(command, L"-h") || !_wcsicmp(command, L"help"))
    {
        usage();
        return kExitOk;
    }

    if (!_wcsicmp(command, L"--version") || !_wcsicmp(command, L"-v"))
    {
        wprintf(L"1.0.0\n");
        return kExitOk;
    }

    bool json = false;
    for (int i = 2; i < argc; ++i)
    {
        if (!_wcsicmp(argv[i], L"--json"))
            json = true;
    }

    DWORD action = 0;
    DWORD argument = 0;
    bool statusOnly = false;

    if (!_wcsicmp(command, L"status"))
    {
        statusOnly = true;
    }
    else if (!_wcsicmp(command, L"light"))
    {
        action = LSL_CMD_LIGHT;
    }
    else if (!_wcsicmp(command, L"dark"))
    {
        action = LSL_CMD_DARK;
    }
    else if (!_wcsicmp(command, L"toggle"))
    {
        action = LSL_CMD_TOGGLE;
    }
    else if (!_wcsicmp(command, L"mode"))
    {
        if (argc < 3 || !parseMode(argv[2], argument))
        {
            fwprintf(stderr, L"Invalid mode. Use off, fixed, solar or nightlight.\n");
            return kExitUsage;
        }
        action = LSL_CMD_MODE;
    }
    else
    {
        fwprintf(stderr, L"Unknown command: %s\n", command);
        return usage();
    }

    HWND window = FindWindowW(LSL_WNDCLASS, nullptr);
    if (!window)
    {
        fwprintf(stderr, L"LightSwitch Lite is not running.\n");
        return kExitNoService;
    }

    if (statusOnly)
    {
        LslStatus status{};
        sendCommand(window, LSL_CMD_REFRESH, 0);
        if (!readStatus(status))
        {
            fwprintf(stderr, L"Could not read the service state.\n");
            return kExitFailed;
        }
        if (json)
            printJson(status);
        else
            printHuman(status);
        return kExitOk;
    }

    if (!sendCommand(window, action, argument))
    {
        fwprintf(stderr, L"The operation failed.\n");
        return kExitFailed;
    }

    if (json)
        wprintf(L"{\"version\":1,\"success\":true}\n");
    else
        wprintf(L"Applied.\n");
    return kExitOk;
}
