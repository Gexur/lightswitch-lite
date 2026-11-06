#include "core.h"

#include <cmath>
#include <cstdlib>
#include <cstdio>
#include <cwchar>

namespace
{
    const wchar_t* kCfgKey = L"Software\\LightSwitchLite";
    const wchar_t* kRunKey = L"Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    const wchar_t* kRunValue = L"LightSwitchLite";

    DWORD readDword(HKEY key, const wchar_t* name, DWORD fallback)
    {
        DWORD value = 0;
        DWORD bytes = sizeof(value);
        DWORD type = 0;
        const LSTATUS st = RegQueryValueExW(key, name, nullptr, &type, reinterpret_cast<LPBYTE>(&value), &bytes);
        if (st != ERROR_SUCCESS || type != REG_DWORD || bytes != sizeof(value))
            return fallback;
        return value;
    }

    void writeDword(HKEY key, const wchar_t* name, DWORD value)
    {
        RegSetValueExW(key, name, 0, REG_DWORD, reinterpret_cast<const BYTE*>(&value), sizeof(value));
    }

    void writeString(HKEY key, const wchar_t* name, const wchar_t* value)
    {
        RegSetValueExW(key, name, 0, REG_SZ, reinterpret_cast<const BYTE*>(value),
                       static_cast<DWORD>((wcslen(value) + 1) * sizeof(wchar_t)));
    }

    int clampMinute(int v)
    {
        return ((v % 1440) + 1440) % 1440;
    }

    void readCoord(HKEY key, const wchar_t* name, double limit, double& out)
    {
        wchar_t buf[64] = {};
        DWORD bytes = sizeof(buf) - sizeof(wchar_t);
        DWORD type = 0;
        if (RegQueryValueExW(key, name, nullptr, &type, reinterpret_cast<LPBYTE>(buf), &bytes) != ERROR_SUCCESS ||
            type != REG_SZ)
            return;
        buf[sizeof(buf) / sizeof(buf[0]) - 1] = L'\0';
        wchar_t* end = nullptr;
        const double v = wcstod(buf, &end);
        if (end == buf || end == nullptr || *end != L'\0' || !std::isfinite(v) || v < -limit || v > limit)
            return;
        out = v;
    }
}

void cfgDefaults(Config& c)
{
    c.mode = Mode::Fixed;
    c.lightMin = 8 * 60;
    c.darkMin = 20 * 60;
    c.lat = 0.0;
    c.lon = 0.0;
    c.targetSystem = true;
    c.targetApps = true;
}

bool cfgLoad(Config& c)
{
    cfgDefaults(c);

    HKEY key = nullptr;
    if (RegOpenKeyExW(HKEY_CURRENT_USER, kCfgKey, 0, KEY_QUERY_VALUE, &key) != ERROR_SUCCESS)
        return false;

    const DWORD mode = readDword(key, L"Mode", static_cast<DWORD>(c.mode));
    c.mode = mode <= 3 ? static_cast<Mode>(mode) : Mode::Fixed;
    c.lightMin = clampMinute(static_cast<int>(readDword(key, L"LightMinutes", static_cast<DWORD>(c.lightMin))));
    c.darkMin = clampMinute(static_cast<int>(readDword(key, L"DarkMinutes", static_cast<DWORD>(c.darkMin))));
    c.targetSystem = readDword(key, L"TargetSystem", 1) != 0;
    c.targetApps = readDword(key, L"TargetApps", 1) != 0;
    readCoord(key, L"Latitude", 90.0, c.lat);
    readCoord(key, L"Longitude", 180.0, c.lon);

    RegCloseKey(key);
    return true;
}

bool cfgSave(const Config& c)
{
    HKEY key = nullptr;
    DWORD disposition = 0;
    if (RegCreateKeyExW(HKEY_CURRENT_USER, kCfgKey, 0, nullptr, 0, KEY_SET_VALUE, nullptr, &key, &disposition) !=
        ERROR_SUCCESS)
        return false;

    writeDword(key, L"Mode", static_cast<DWORD>(c.mode));
    writeDword(key, L"LightMinutes", static_cast<DWORD>(clampMinute(c.lightMin)));
    writeDword(key, L"DarkMinutes", static_cast<DWORD>(clampMinute(c.darkMin)));
    writeDword(key, L"TargetSystem", c.targetSystem ? 1u : 0u);
    writeDword(key, L"TargetApps", c.targetApps ? 1u : 0u);

    wchar_t buf[64];
    swprintf_s(buf, L"%.6f", c.lat);
    writeString(key, L"Latitude", buf);
    swprintf_s(buf, L"%.6f", c.lon);
    writeString(key, L"Longitude", buf);

    RegCloseKey(key);
    return true;
}

bool autostartGet()
{
    HKEY key = nullptr;
    if (RegOpenKeyExW(HKEY_CURRENT_USER, kRunKey, 0, KEY_QUERY_VALUE, &key) != ERROR_SUCCESS)
        return false;
    const LSTATUS st = RegQueryValueExW(key, kRunValue, nullptr, nullptr, nullptr, nullptr);
    RegCloseKey(key);
    return st == ERROR_SUCCESS;
}

bool autostartSet(bool on)
{
    HKEY key = nullptr;
    if (RegCreateKeyExW(HKEY_CURRENT_USER, kRunKey, 0, nullptr, 0, KEY_SET_VALUE, nullptr, &key, nullptr) !=
        ERROR_SUCCESS)
        return false;

    bool ok = true;
    if (on)
    {
        wchar_t path[MAX_PATH] = {};
        if (GetModuleFileNameW(nullptr, path, MAX_PATH) == 0)
        {
            RegCloseKey(key);
            return false;
        }
        wchar_t cmd[MAX_PATH + 32];
        swprintf_s(cmd, L"\"%s\" --background", path);
        ok = RegSetValueExW(key, kRunValue, 0, REG_SZ, reinterpret_cast<const BYTE*>(cmd),
                            static_cast<DWORD>((wcslen(cmd) + 1) * sizeof(wchar_t))) == ERROR_SUCCESS;
    }
    else
    {
        const LSTATUS st = RegDeleteValueW(key, kRunValue);
        ok = st == ERROR_SUCCESS || st == ERROR_FILE_NOT_FOUND;
    }

    RegCloseKey(key);
    return ok;
}
