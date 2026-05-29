// Added: Tests for Ghana timezone utility (TMAIL-57)
import 'package:flutter_test/flutter_test.dart';
import 'package:tasmail_mobile/utils/ghana_timezone.dart';

void main() {
  group('GhanaTimezone', () {
    test('exposes Africa/Accra IANA identifier', () {
      expect(GhanaTimezone.ianaId, 'Africa/Accra');
    });

    test('uses GMT abbreviation', () {
      expect(GhanaTimezone.abbreviation, 'GMT');
    });

    test('offset is zero minutes (no DST)', () {
      expect(GhanaTimezone.utcOffsetMinutes, 0);
    });

    test('toGhanaTime preserves UTC wall-clock components', () {
      final utc = DateTime.utc(2026, 5, 29, 14, 30, 15);
      final ghana = GhanaTimezone.toGhanaTime(utc);
      expect(ghana.year, 2026);
      expect(ghana.month, 5);
      expect(ghana.day, 29);
      expect(ghana.hour, 14);
      expect(ghana.minute, 30);
      expect(ghana.second, 15);
    });

    test('toGhanaTime normalises a local DateTime via UTC', () {
      // A DateTime constructed in local time is first converted to UTC,
      // then its wall-clock components are returned. The exact UTC value
      // depends on the host TZ, so we only assert the round-trip is stable.
      final local = DateTime(2026, 1, 1, 12, 0, 0);
      final ghana = GhanaTimezone.toGhanaTime(local);
      final expectedUtc = local.toUtc();
      expect(ghana.year, expectedUtc.year);
      expect(ghana.month, expectedUtc.month);
      expect(ghana.day, expectedUtc.day);
      expect(ghana.hour, expectedUtc.hour);
      expect(ghana.minute, expectedUtc.minute);
    });
  });
}
