#include "core.h"

namespace
{
    const wchar_t* kPersonalize =
        L"Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize";
    const wchar_t* kNightLight =
        L"Software\\Microsoft\\Windows\\CurrentVersion\\CloudStore\\Store\\DefaultAccount\\Current\\"
        L"default$windows.data.bluelightreduction.bluelightreductionstate\\"
        L"windows.data.bluelightreduction.bluelightreductionstate";

    // Tells the shell the immersive colors changed. Explorer repaints the
    // taskbar and Start; secondary-monitor taskbars need this as well.
    void notifyThemeChange()
    {
        DWORD_PTR ignored = 0;
        SendMessageTimeoutW(HWND_BROADCAST, WM_SETTINGCHANGE, 0,
                            reinterpret_cast<LPARAM>(L"ImmersiveColorSet"),
                            SMTO_ABORTIFHUNG, 1000, &ignored);
    }

    void notifyColorization()
    {
        DWORD_PTR ignored = 0;
        SendMessageTimeoutW(HWND_BROADCAST, WM_DWMCOLORIZATIONCOLORCHANGED, 0, 0,
                            SMTO_ABORTIFHUNG, 1000, &ignored);
    }
}


bool themeRead(bool system, bool& light)
{
    HKEY key = nullptr;
    if (RegOpenKeyExW(HKEY_CURRENT_USER, kPersonalize, 0, KEY_QUERY_VALUE, &key) != ERROR_SUCCESS)
        return false;

    DWORD value = 0;
    DWORD bytes = sizeof(value);
    DWORD type = 0;
    const LSTATUS st = RegQueryValueExW(key, system ? L"SystemUsesLightTheme" : L"AppsUseLightTheme", nullptr,
                                        &type, reinterpret_cast<LPBYTE>(&value), &bytes);
    RegCloseKey(key);

    if (st != ERROR_SUCCESS || type != REG_DWORD || bytes != sizeof(value))
        return false;

    light = value != 0;
    return true;
}

bool themeApply(bool light, bool system, bool apps, bool& systemOk, bool& appsOk)
{
    systemOk = true;
    appsOk = true;

    HKEY key = nullptr;
    if (RegOpenKeyExW(HKEY_CURRENT_USER, kPersonalize, 0, KEY_QUERY_VALUE | KEY_SET_VALUE, &key) != ERROR_SUCCESS)
    {
        systemOk = false;
        appsOk = false;
        return false;
    }

    const DWORD value = light ? 1u : 0u;
    bool wrote = false;

    auto put = [&](const wchar_t* name, bool& ok) {
        DWORD current = 0;
        DWORD bytes = sizeof(current);
        DWORD type = 0;
        if (RegQueryValueExW(key, name, nullptr, &type, reinterpret_cast<LPBYTE>(&current), &bytes) == ERROR_SUCCESS &&
            type == REG_DWORD && bytes == sizeof(current) && current == value)
            return; // already matches: leave the registry untouched
        ok = RegSetValueExW(key, name, 0, REG_DWORD, reinterpret_cast<const BYTE*>(&value), sizeof(value)) ==
            ERROR_SUCCESS;
        wrote = true;
    };

    if (system)
    {
        put(L"SystemUsesLightTheme", systemOk);
        if (systemOk && light)
        {
            // Windows paints the accent color on the title bar again if this stays on.
            const DWORD zero = 0;
            RegSetValueExW(key, L"ColorPrevalence", 0, REG_DWORD,
                           reinterpret_cast<const BYTE*>(&zero), sizeof(zero));
        }
    }
    if (apps)
        put(L"AppsUseLightTheme", appsOk);

    RegCloseKey(key);

    if (wrote)
    {
        notifyThemeChange();
        if (system && light)
            notifyColorization();
    }
    return systemOk && appsOk;
}

bool nightLightRead(bool& on)
{    HKEY key = nullptr;
    if (RegOpenKeyExW(HKEY_CURRENT_USER, kNightLight, 0, KEY_QUERY_VALUE, &key) != ERROR_SUCCESS)
        return false;

    BYTE data[1024] = {};
    DWORD bytes = sizeof(data);
    DWORD type = 0;
    const LSTATUS st = RegQueryValueExW(key, L"Data", nullptr, &type, data, &bytes);
    RegCloseKey(key);

    if (st != ERROR_SUCCESS || type != REG_BINARY || bytes < 25)
        return false;

    on = data[23] == 0x10 && data[24] == 0x00;
    return true;
}

HKEY regOpenPersonalize(DWORD access)
{
    HKEY key = nullptr;
    if (RegOpenKeyExW(HKEY_CURRENT_USER, kPersonalize, 0, access, &key) != ERROR_SUCCESS)
        return nullptr;
    return key;
}

HKEY regOpenNightLight(DWORD access)
{
    HKEY key = nullptr;
    if (RegOpenKeyExW(HKEY_CURRENT_USER, kNightLight, 0, access, &key) != ERROR_SUCCESS)
        return nullptr;
    return key;
}
