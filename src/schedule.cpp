#include "core.h"

#include <cmath>

namespace
{
    constexpr double kPi = 3.14159265358979323846;

    double toRad(double deg) { return deg * kPi / 180.0; }
    double toDeg(double rad) { return rad * 180.0 / kPi; }

    double wrap360(double v)
    {
        v = fmod(v, 360.0);
        if (v < 0.0)
            v += 360.0;
        return v;
    }

    int wrapMin(int v) { return ((v % 1440) + 1440) % 1440; }

    double julianDay(int y, int m, int d)
    {
        if (m <= 2)
        {
            y -= 1;
            m += 12;
        }
        const int a = y / 100;
        const int b = 2 - a + a / 4;
        return floor(365.25 * (y + 4716)) + floor(30.6001 * (m + 1)) + d + b - 1524.5 + 0.5;
    }

    // Converts a UTC instant (minutes since midnight of day d) into local minutes,
    // honouring the daylight-saving rule in force on that specific date.
    bool utcMinutesToLocal(int y, int m, int d, double utcMin, int& outMin)
    {
        const int dayShift = static_cast<int>(floor(utcMin / 1440.0));
        double rest = utcMin - dayShift * 1440.0;
        int hour = static_cast<int>(rest / 60.0);
        int minute = static_cast<int>(lround(rest - hour * 60.0));
        if (minute == 60)
        {
            minute = 0;
            ++hour;
        }
        int extraDay = 0;
        if (hour >= 24)
        {
            hour -= 24;
            extraDay = 1;
        }

        SYSTEMTIME utc{};
        utc.wYear = static_cast<WORD>(y);
        utc.wMonth = static_cast<WORD>(m);
        utc.wDay = static_cast<WORD>(d);
        utc.wHour = static_cast<WORD>(hour);
        utc.wMinute = static_cast<WORD>(minute);

        FILETIME ft{};
        if (!SystemTimeToFileTime(&utc, &ft))
            return false;

        ULARGE_INTEGER value;
        value.LowPart = ft.dwLowDateTime;
        value.HighPart = ft.dwHighDateTime;
        value.QuadPart += static_cast<ULONGLONG>(dayShift + extraDay) * 864000000000ULL;

        ft.dwLowDateTime = value.LowPart;
        ft.dwHighDateTime = value.HighPart;

        SYSTEMTIME shifted{};
        if (!FileTimeToSystemTime(&ft, &shifted))
            return false;

        TIME_ZONE_INFORMATION tz{};
        if (!GetTimeZoneInformationForYear(static_cast<USHORT>(shifted.wYear), nullptr, &tz))
        {
            if (GetTimeZoneInformation(&tz) == TIME_ZONE_ID_INVALID)
                return false;
        }

        SYSTEMTIME local{};
        if (!SystemTimeToTzSpecificLocalTime(&tz, &shifted, &local))
            return false;

        outMin = local.wHour * 60 + local.wMinute;
        return true;
    }
}

bool isLightAt(int nowMin, int lightMin, int darkMin)
{
    const int light = wrapMin(lightMin);
    const int dark = wrapMin(darkMin);
    const int now = wrapMin(nowMin);

    if (light < dark)
        return now >= light && now < dark;
    return now >= light || now < dark;
}

bool sunTimes(double lat, double lon, int y, int m, int d, int& riseMin, int& setMin)
{
    if (lat > 89.5)
        lat = 89.5;
    if (lat < -89.5)
        lat = -89.5;

    const double n = julianDay(y, m, d) - 2451545.0;
    const double meanLon = wrap360(280.460 + 0.9856474 * n);
    const double anomaly = toRad(wrap360(357.528 + 0.9856003 * n));
    const double ecliptic = toRad(meanLon + 1.915 * sin(anomaly) + 0.020 * sin(2.0 * anomaly));
    const double tilt = toRad(23.439 - 0.0000004 * n);

    const double rightAsc = atan2(cos(tilt) * sin(ecliptic), cos(ecliptic));
    const double decl = asin(sin(tilt) * sin(ecliptic));

    // Equation of time in minutes, clamped to the physical range (+-20 min).
    double equation = 4.0 * (meanLon - toDeg(rightAsc));
    while (equation > 20.0)
        equation -= 1440.0;
    while (equation < -20.0)
        equation += 1440.0;

    const double latRad = toRad(lat);
    const double cosHour = (cos(toRad(90.833)) - sin(latRad) * sin(decl)) / (cos(latRad) * cos(decl));
    if (cosHour > 1.0 || cosHour < -1.0)
        return false; // midnight sun or polar night

    const double hourAngle = toDeg(acos(cosHour)); // grados
    const double noonUtc = 720.0 - 4.0 * lon - equation;
    const double riseUtc = noonUtc - 4.0 * hourAngle;
    const double setUtc = noonUtc + 4.0 * hourAngle;

    int rise = 0;
    int set = 0;
    if (!utcMinutesToLocal(y, m, d, riseUtc, rise))
        return false;
    if (!utcMinutesToLocal(y, m, d, setUtc, set))
        return false;

    riseMin = wrapMin(rise);
    setMin = wrapMin(set);
    return true;
}

void effTimes(const Config& c, const SYSTEMTIME& now, int& lightMin, int& darkMin)
{
    if (c.mode == Mode::Solar)
    {
        int rise = 0;
        int set = 0;
        if (sunTimes(c.lat, c.lon, now.wYear, now.wMonth, now.wDay, rise, set))
        {
            lightMin = wrapMin(rise);
            darkMin = wrapMin(set);
            return;
        }
        // No sunrise/sunset today: fall back to a default schedule.
        lightMin = 8 * 60;
        darkMin = 20 * 60;
        return;
    }

    lightMin = wrapMin(c.lightMin);
    darkMin = wrapMin(c.darkMin);
}

ULONGLONG stampOf(const SYSTEMTIME& t)
{
    FILETIME ft{};
    if (!SystemTimeToFileTime(&t, &ft))
        return 0;
    ULARGE_INTEGER value;
    value.LowPart = ft.dwLowDateTime;
    value.HighPart = ft.dwHighDateTime;
    return value.QuadPart / 600000000ULL; // minutes
}

bool crossedBoundary(const Config& c, ULONGLONG fromStamp, const SYSTEMTIME& now)
{
    const ULONGLONG current = stampOf(now);
    if (fromStamp == 0 || current == 0 || current <= fromStamp)
        return false;
    if (current - fromStamp >= 1440ULL)
        return true;

    const ULONGLONG fileTime = fromStamp * 600000000ULL;
    FILETIME ft{};
    ft.dwLowDateTime = static_cast<DWORD>(fileTime & 0xFFFFFFFFu);
    ft.dwHighDateTime = static_cast<DWORD>(fileTime >> 32);

    SYSTEMTIME previous{};
    if (!FileTimeToSystemTime(&ft, &previous))
        return true;

    int lightA = 0;
    int darkA = 0;
    int lightB = 0;
    int darkB = 0;
    effTimes(c, previous, lightA, darkA);
    effTimes(c, now, lightB, darkB);

    const bool wasLight = isLightAt(previous.wHour * 60 + previous.wMinute, lightA, darkA);
    const bool isLight = isLightAt(now.wHour * 60 + now.wMinute, lightB, darkB);
    return wasLight != isLight;
}

DWORD msToNextChange(const Config& c, const SYSTEMTIME& now, bool& toLight)
{
    toLight = false;
    if (c.mode == Mode::Off || c.mode == Mode::NightLight)
        return 0;

    int lightMin = 0;
    int darkMin = 0;
    effTimes(c, now, lightMin, darkMin);
    if (lightMin == darkMin)
        return 0; // degenerate schedule: nothing to program

    const long nowSec = now.wHour * 3600L + now.wMinute * 60L + now.wSecond;

    auto until = [&](int targetMin) -> long {
        long target = static_cast<long>(targetMin) * 60L;
        long diff = (target - nowSec) % 86400L;
        if (diff <= 0)
            diff += 86400L;
        long ms = diff * 1000L - static_cast<long>(now.wMilliseconds);
        if (ms <= 0)
            ms += 86400000L;
        return ms;
    };

    const long toLightMs = until(lightMin);
    const long toDarkMs = until(darkMin);

    if (toLightMs <= toDarkMs)
    {
        toLight = true;
        return static_cast<DWORD>(toLightMs);
    }
    toLight = false;
    return static_cast<DWORD>(toDarkMs);
}
