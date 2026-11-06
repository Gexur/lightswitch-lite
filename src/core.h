// LightSwitch Lite - shared core.
// Original implementation: no service, no JSON, no WinRT.
#pragma once

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>

#define LSL_APP L"LightSwitch Lite"
#define LSL_VER L"1.0.0"

// Schedule mode. Values are shared with lsl.exe: do not renumber.
enum class Mode : DWORD
{
    Off = 0,
    Fixed = 1,
    Solar = 2,
    NightLight = 3,
};

struct Config
{
    Mode mode;
    int lightMin;   // minute of day when the light theme starts
    int darkMin;    // minute of day when the dark theme starts
    double lat;
    double lon;
    int riseOffset; // minutes added to sunrise
    int setOffset;  // minutes added to sunset
    bool targetSystem;
    bool targetApps;
    bool hotkeyOn;
};

// --- settings (HKCU\Software\LightSwitchLite) ---
void cfgDefaults(Config& c);
bool cfgLoad(Config& c);
bool cfgSave(const Config& c);

// --- startup (HKCU\...\Run) ---
bool autostartGet();
bool autostartSet(bool on);

// --- system theme ---
bool themeRead(bool system, bool& light);
bool themeApply(bool light, bool system, bool apps, bool& systemOk, bool& appsOk);
bool nightLightRead(bool& on);
HKEY regOpenPersonalize(DWORD access);
HKEY regOpenNightLight(DWORD access);

// --- scheduling ---
bool isLightAt(int nowMin, int lightMin, int darkMin);
bool sunTimes(double lat, double lon, int y, int m, int d, int& riseMin, int& setMin);
void effTimes(const Config& c, const SYSTEMTIME& now, int& lightMin, int& darkMin);
ULONGLONG stampOf(const SYSTEMTIME& t);
bool crossedBoundary(const Config& c, ULONGLONG fromStamp, const SYSTEMTIME& now);
// Milliseconds until the next theme change (0 when no schedule is active).
DWORD msToNextChange(const Config& c, const SYSTEMTIME& now, bool& toLight);

// --- bridge to the UI ---
void appConfigChanged();
void appDescribe(wchar_t* out, int cch);
void appShowSettings();
void appTrimMemory();

// --- settings window (ui.cpp) ---
void uiShow(HWND owner);
void uiRefresh();
void uiThemeChanged();
void uiDestroy();
