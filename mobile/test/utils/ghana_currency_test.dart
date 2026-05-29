// Added: Tests for GHS currency formatter (TMAIL-57)
import 'package:flutter_test/flutter_test.dart';
import 'package:tasmail_mobile/utils/ghana_currency.dart';

void main() {
  group('GhanaCurrency', () {
    test('exposes ISO 4217 code and cedi symbol', () {
      expect(GhanaCurrency.code, 'GHS');
      expect(GhanaCurrency.symbol, '₵');
      expect(GhanaCurrency.decimalDigits, 2);
    });

    test('format renders cedi symbol with two decimals', () {
      expect(GhanaCurrency.format(1234.5), contains('₵'));
      expect(GhanaCurrency.format(1234.5), contains('1,234.50'));
    });

    test('format pads short amounts to two decimals', () {
      expect(GhanaCurrency.format(5), contains('5.00'));
    });

    test('formatWithCode prefixes the GHS code', () {
      final s = GhanaCurrency.formatWithCode(99.99);
      expect(s, startsWith('GHS '));
      expect(s, contains('99.99'));
    });

    test('formatPesewas converts 100 pesewas to ₵1.00', () {
      expect(GhanaCurrency.formatPesewas(100), contains('1.00'));
    });

    test('formatPesewas handles minimum billing threshold (GHS 5)', () {
      // The published minimum is GHS 5 / month → 500 pesewas on the wire.
      final s = GhanaCurrency.formatPesewas(500);
      expect(s, contains('5.00'));
    });
  });
}
