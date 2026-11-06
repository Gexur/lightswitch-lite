// IPC contract between LightSwitchLite.exe and lsl.exe.
// Depends only on <windows.h>, so both executables can include it.
#pragma once

#include <windows.h>

#define LSL_WNDCLASS L"LSL.MainWindow"
#define LSL_MAPPING L"Local\\LSL.Status"
#define LSL_MUTEX L"Local\\LSL.SingleInstance"
#define LSL_IPC_MAGIC 0x4C534C31u // "LSL1"
#define LSL_STATUS_MAGIC 0x4C535431u // "LST1"

enum LslCommand : DWORD
{
    LSL_CMD_PING = 0,
    LSL_CMD_SHOW = 1,
    LSL_CMD_LIGHT = 2,
    LSL_CMD_DARK = 3,
    LSL_CMD_TOGGLE = 4,
    LSL_CMD_MODE = 5,
    LSL_CMD_REFRESH = 6,
    LSL_CMD_QUIT = 7,
};

struct LslIpc
{
    DWORD magic;
    DWORD cmd;
    DWORD arg;
};

// Published by the app, read by the CLI.
struct LslStatus
{
    DWORD magic;
    DWORD size;
    DWORD version;
    DWORD pid;
    DWORD mode;          // Mode
    DWORD systemLight;   // 0 dark, 1 light, 2 unknown
    DWORD appsLight;
    DWORD manualOverride;
    DWORD changeSystem;
    DWORD changeApps;
    DWORD nightLight;    // 0 off, 1 on, 2 unknown
    DWORD nextChange;    // seconds since the Unix epoch (UTC), 0 = none
};
