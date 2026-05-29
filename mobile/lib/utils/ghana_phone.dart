// Added: +233 Ghana phone-number formatting / validation for TMAIL-57
// PURPOSE: Mobile screens (signup, SMS-OTP, contacts, billing) accept phone
//          numbers in mixed shapes — local "024 123 4567", E.164
//          "+233241234567", paste with spaces / dashes, etc. This helper
//          normalises them to E.164 ("+233241234567") and renders the
//          national display form ("024 123 4567").
// EXTERNAL: Used by SMS-OTP enrollment, MoMo / Vodafone Cash payment flows,
//          contact picker normalisation. Pure-Dart string ops — no plugins.

/// Ghana phone-number helpers. Ghana uses country calling code +233 with
/// 9-digit subscriber numbers; mobile prefixes start with `2`, `5` or `7`
/// after the trunk `0` is stripped (e.g. MTN 024/054/055/059, Vodafone
/// 020/050, AirtelTigo 026/056/027/057).
class GhanaPhone {
  GhanaPhone._();

  /// ITU-T country calling code for Ghana.
  static const String countryCode = '+233';

  /// Numeric country code without the leading "+".
  static const String countryCodeDigits = '233';

  /// Length of a Ghana subscriber number once the trunk `0` is removed
  /// (e.g. `241234567`).
  static const int subscriberLength = 9;

  /// Strip every non-digit (and the leading "+") down to bare digits.
  static String _digitsOnly(String raw) {
    return raw.replaceAll(RegExp(r'[^0-9]'), '');
  }

  /// Return the 9-digit subscriber portion regardless of whether [raw] came
  /// in as `+233241234567`, `00233241234567`, `0241234567`, `241234567`, or
  /// any of those with spaces / dashes / parentheses. Returns `null` if the
  /// input cannot be coerced to a valid Ghana mobile number.
  static String? subscriberDigits(String raw) {
    var digits = _digitsOnly(raw);

    // International prefix `00` → drop it so 00233… becomes 233….
    if (digits.startsWith('00')) {
      digits = digits.substring(2);
    }
    // Country code prefix.
    if (digits.startsWith(countryCodeDigits)) {
      digits = digits.substring(countryCodeDigits.length);
    } else if (digits.startsWith('0')) {
      // National trunk prefix.
      digits = digits.substring(1);
    }

    if (digits.length != subscriberLength) return null;
    // First digit of the subscriber portion is the network indicator;
    // Ghana mobile lines start with 2, 5 or 7 (post-2020 numbering plan).
    if (!RegExp(r'^[257]').hasMatch(digits)) return null;
    return digits;
  }

  /// Normalise to E.164 — `+233241234567`. Returns `null` if [raw] isn't a
  /// recognisable Ghana mobile number.
  static String? toE164(String raw) {
    final sub = subscriberDigits(raw);
    if (sub == null) return null;
    return '$countryCode$sub';
  }

  /// Render in the conventional Ghana national display form
  /// `0XX XXX XXXX` (e.g. `024 123 4567`). Returns `null` for invalid input.
  static String? toNationalDisplay(String raw) {
    final sub = subscriberDigits(raw);
    if (sub == null) return null;
    // 9 subscriber digits → split as 2-3-4 after the trunk `0`.
    return '0${sub.substring(0, 2)} ${sub.substring(2, 5)} ${sub.substring(5)}';
  }

  /// Render in the international display form `+233 XX XXX XXXX`.
  static String? toInternationalDisplay(String raw) {
    final sub = subscriberDigits(raw);
    if (sub == null) return null;
    return '$countryCode ${sub.substring(0, 2)} ${sub.substring(2, 5)} ${sub.substring(5)}';
  }

  /// True when [raw] coerces to a valid Ghana mobile number.
  static bool isValid(String raw) => subscriberDigits(raw) != null;
}
