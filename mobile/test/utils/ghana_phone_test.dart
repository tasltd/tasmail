// Added: Tests for +233 Ghana phone formatter (TMAIL-57)
import 'package:flutter_test/flutter_test.dart';
import 'package:tasmail_mobile/utils/ghana_phone.dart';

void main() {
  group('GhanaPhone', () {
    test('exposes +233 country code constants', () {
      expect(GhanaPhone.countryCode, '+233');
      expect(GhanaPhone.countryCodeDigits, '233');
      expect(GhanaPhone.subscriberLength, 9);
    });

    group('toE164', () {
      test('handles already-E.164 input', () {
        expect(GhanaPhone.toE164('+233241234567'), '+233241234567');
      });

      test('handles 00-prefixed international form', () {
        expect(GhanaPhone.toE164('00233241234567'), '+233241234567');
      });

      test('handles national trunk form with spaces', () {
        expect(GhanaPhone.toE164('024 123 4567'), '+233241234567');
      });

      test('handles bare subscriber digits', () {
        expect(GhanaPhone.toE164('241234567'), '+233241234567');
      });

      test('handles dashes and parentheses', () {
        expect(GhanaPhone.toE164('(024)-123-4567'), '+233241234567');
      });

      test('accepts Vodafone/Telecel 020 prefix', () {
        expect(GhanaPhone.toE164('020 555 0001'), '+233205550001');
      });

      test('accepts AirtelTigo 057 prefix', () {
        expect(GhanaPhone.toE164('057 555 0002'), '+233575550002');
      });

      test('rejects numbers that are too short', () {
        expect(GhanaPhone.toE164('024 123'), isNull);
      });

      test('rejects numbers that are too long', () {
        expect(GhanaPhone.toE164('+2332412345678'), isNull);
      });

      test('rejects invalid network prefix (landlines start with 3)', () {
        expect(GhanaPhone.toE164('+233312345678'), isNull);
      });

      test('rejects garbage input', () {
        expect(GhanaPhone.toE164('hello'), isNull);
        expect(GhanaPhone.toE164(''), isNull);
      });
    });

    group('toNationalDisplay', () {
      test('formats as 0XX XXX XXXX', () {
        expect(GhanaPhone.toNationalDisplay('+233241234567'), '024 123 4567');
      });

      test('round-trips from national input', () {
        expect(GhanaPhone.toNationalDisplay('0241234567'), '024 123 4567');
      });

      test('returns null for invalid input', () {
        expect(GhanaPhone.toNationalDisplay('garbage'), isNull);
      });
    });

    group('toInternationalDisplay', () {
      test('formats as +233 XX XXX XXXX', () {
        expect(
          GhanaPhone.toInternationalDisplay('0241234567'),
          '+233 24 123 4567',
        );
      });
    });

    group('isValid', () {
      test('returns true for valid mobile numbers', () {
        expect(GhanaPhone.isValid('+233241234567'), isTrue);
        expect(GhanaPhone.isValid('024 123 4567'), isTrue);
      });

      test('returns false for invalid numbers', () {
        expect(GhanaPhone.isValid('123'), isFalse);
        expect(GhanaPhone.isValid('+15551234567'), isFalse);
      });
    });
  });
}
