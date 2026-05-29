// Added: Ghana timezone defaults for TMAIL-57
// PURPOSE: Ghana (and the entire country, no DST, no regional splits) sits on
//          UTC+0 year-round. The mobile app uses this constant when rendering
//          dates/times for users whose device timezone isn't set or who
//          explicitly opted into a "Ghana time" display preference.
// EXTERNAL: Used by message list/detail screens and the (future) settings
//          screen's regional preferences pane. Pure constants + helpers — no
//          plugin / platform-channel dependency, safe to use anywhere.

/// Ghana Mean Time. Ghana does not observe daylight saving and has used
/// UTC+0 (GMT) without offset since 1936. See <https://www.timeanddate.com/time/zone/ghana>.
class GhanaTimezone {
  GhanaTimezone._();

  /// IANA time-zone identifier for Ghana. Africa/Accra is the only zone
  /// covering the country.
  static const String ianaId = 'Africa/Accra';

  /// Common display abbreviation (Ghana Mean Time / GMT).
  static const String abbreviation = 'GMT';

  /// Offset from UTC, in minutes. Always 0 — no DST adjustment.
  static const int utcOffsetMinutes = 0;

  /// Convert any [DateTime] (UTC or local) to its Ghana-local wall-clock
  /// representation. Since Ghana is UTC+0, this returns the UTC instant
  /// expressed as a local-style DateTime (no DST math required).
  static DateTime toGhanaTime(DateTime dt) {
    final utc = dt.isUtc ? dt : dt.toUtc();
    // NOTE: Returned DateTime is constructed with the local-DateTime
    //       constructor for downstream formatters (e.g. intl DateFormat
    //       without an explicit locale) to render the wall-clock without
    //       reapplying the device offset.
    return DateTime(
      utc.year,
      utc.month,
      utc.day,
      utc.hour,
      utc.minute,
      utc.second,
      utc.millisecond,
      utc.microsecond,
    );
  }
}
