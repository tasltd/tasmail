// Added: GHS currency formatting for TMAIL-57
// PURPOSE: TASMail BYOK pricing is published in Ghana cedis (GHS 1.00 / GB
//          · month, GHS 5 monthly minimum). The mobile app needs a single
//          place to format cedi amounts so billing / quota / receipt screens
//          render consistently (₵ symbol, two decimals, Ghana grouping).
// EXTERNAL: Used by quota / billing screens. Wraps `package:intl`'s
//          NumberFormat — no platform-channel calls, safe in unit tests.

import 'package:intl/intl.dart';

/// Ghana cedi currency helpers. Cedi (₵) is the ISO 4217 code GHS; the cedi
/// symbol is U+20B5 (₵). 1 cedi = 100 pesewas (two decimal places).
class GhanaCurrency {
  GhanaCurrency._();

  /// ISO 4217 currency code.
  static const String code = 'GHS';

  /// Unicode cedi symbol.
  static const String symbol = '₵';

  /// Plain human name. Useful for screen reader labels.
  static const String name = 'Ghana Cedi';

  /// Number of fractional digits (pesewas).
  static const int decimalDigits = 2;

  /// Format an amount as "₵1,234.56".
  static String format(num amount) {
    final fmt = NumberFormat.currency(
      locale: 'en_GH',
      symbol: symbol,
      decimalDigits: decimalDigits,
    );
    return fmt.format(amount);
  }

  /// Format with the GHS code prefix instead of the cedi glyph
  /// (e.g. "GHS 1,234.56"). Some receipts / SMS notifications prefer the
  /// ASCII-safe code.
  static String formatWithCode(num amount) {
    final fmt = NumberFormat.currency(
      locale: 'en_GH',
      symbol: '$code ',
      decimalDigits: decimalDigits,
    );
    return fmt.format(amount);
  }

  /// Format an amount in pesewas (smallest unit) — e.g. 12345 → "₵123.45".
  /// Mirrors how Paystack / Mastercard MPGS represent amounts on the wire.
  static String formatPesewas(int pesewas) => format(pesewas / 100);
}
