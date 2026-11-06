// Desktop test for the scheduling logic (not shipped).
#include "../src/core.h"

#include <cstdio>
#include <cstdlib>
#include <cmath>

static int failures = 0;

static void expect(bool condition, const char* what)
{
    if (!condition)
    {
        printf("FAIL: %s\n", what);
        ++failures;
    }
    else
    {
        printf("ok    %s\n", what);
    }
}

static void printSun(const char* city, double lat, double lon, int y, int m, int d)
{
    int rise = 0;
    int set = 0;
    if (!sunTimes(lat, lon, y, m, d, rise, set))
    {
        printf("%-14s no sunrise/sunset\n", city);
        return;
    }
    printf("%-14s %04d-%02d-%02d  sunrise %02d:%02d  sunset %02d:%02d\n", city, y, m, d, rise / 60, rise % 60,
           set / 60, set % 60);
}

int main()
{
    // Normal range: light 08:00-20:00.
    expect(isLightAt(8 * 60, 8 * 60, 20 * 60), "08:00 is light");
    expect(isLightAt(12 * 60, 8 * 60, 20 * 60), "12:00 is light");
    expect(!isLightAt(20 * 60, 8 * 60, 20 * 60), "20:00 is already dark");
    expect(!isLightAt(7 * 60 + 59, 8 * 60, 20 * 60), "07:59 is dark");

    // Inverted range: light 20:00-08:00.
    expect(isLightAt(22 * 60, 20 * 60, 8 * 60), "22:00 is light in an inverted range");
    expect(isLightAt(2 * 60, 20 * 60, 8 * 60), "02:00 is light in an inverted range");
    expect(!isLightAt(9 * 60, 20 * 60, 8 * 60), "09:00 is dark in an inverted range");

    // Horas solares.
    printSun("Buenos Aires", -34.6037, -58.3816, 2026, 11, 5);
    printSun("Quito", -0.1807, -78.4678, 2026, 11, 5);
    printSun("Madrid", 40.4168, -3.7038, 2026, 11, 5);
    printSun("Londres", 51.5074, -0.1278, 2026, 11, 5);
    printSun("Tromso", 69.6492, 18.9553, 2026, 12, 21);

    {
        int rise = 0;
        int set = 0;
        const bool ok = sunTimes(-34.6037, -58.3816, 2026, 11, 5, rise, set);
        expect(ok, "Buenos Aires computes solar times");
        expect(std::abs(rise - (5 * 60 + 46)) <= 5, "Buenos Aires sunrise ~05:46");
        expect(std::abs(set - (19 * 60 + 31)) <= 5, "Buenos Aires sunset ~19:31");
    }

    // Next change: at 21:05 with light 08:00 / dark 20:00 the next switch is light at 08:00.
    {
        SYSTEMTIME now{};
        now.wYear = 2026;
        now.wMonth = 11;
        now.wDay = 5;
        now.wHour = 21;
        now.wMinute = 5;
        now.wSecond = 0;

        Config cfg{};
        cfg.mode = Mode::Fixed;
        cfg.lightMin = 8 * 60;
        cfg.darkMin = 20 * 60;

        bool toLight = false;
        const DWORD ms = msToNextChange(cfg, now, toLight);
        expect(toLight, "at 21:05 the next change is to light");
        expect(ms == 10 * 3600 * 1000 + 55 * 60 * 1000, "10 h 55 min remain until 08:00");
    }

    // Boundary crossing: from 19:55 to 20:05 crosses the dark boundary.
    {
        SYSTEMTIME before{};
        before.wYear = 2026;
        before.wMonth = 11;
        before.wDay = 5;
        before.wHour = 19;
        before.wMinute = 55;
        before.wSecond = 0;

        SYSTEMTIME after{};
        after = before;
        after.wHour = 20;
        after.wMinute = 5;

        Config cfg{};
        cfg.mode = Mode::Fixed;
        cfg.lightMin = 8 * 60;
        cfg.darkMin = 20 * 60;

        expect(crossedBoundary(cfg, stampOf(before), after), "crosses the 20:00 boundary");
        expect(!crossedBoundary(cfg, stampOf(after), after), "no clock advance means no crossing");
    }

    printf("\n%s (%d fallos)\n", failures == 0 ? "ALL OK" : "FAILURES", failures);
    return failures == 0 ? 0 : 1;
}
