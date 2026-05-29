// Added: Ghana mobile-money payment method registry for TMAIL-57
// PURPOSE: TASMail's BYOK pricing is settled in Ghana cedis. The mobile
//          billing screen needs a single canonical registry of the local
//          mobile-money providers users can pay with, plus the matching
//          network phone-number prefixes so the UI can auto-detect the
//          provider from a typed MoMo number.
// EXTERNAL: Used by the (future) billing / top-up screen and any code that
//          maps a phone prefix to a wallet provider. Static data — no
//          plugins, safe in unit tests.
//
// NOTE: This registry is data-driven so adding / renaming a provider is a
//       single-entry change (no controller / switch updates). Vodafone Cash
//       rebranded to Telecel Cash in 2024 after Telecel Group acquired
//       Vodafone Ghana — both labels are retained as aliases for legacy UI
//       and historical receipt rendering.

import 'ghana_phone.dart';

/// Stable identifier for a Ghana mobile-money provider. Persist this in
/// the DB / wire format — display labels can change with rebrands, IDs
/// should not.
enum GhanaPaymentMethodId {
  mtnMomo,
  telecelCash,
  airteltigoMoney,
}

class GhanaPaymentMethod {
  final GhanaPaymentMethodId id;

  /// Current marketing label (e.g. "MTN Mobile Money").
  final String displayName;

  /// Short label fit for chips / radio buttons (e.g. "MTN MoMo").
  final String shortLabel;

  /// Historical aliases (e.g. "Vodafone Cash" before the Telecel rebrand) so
  /// search / receipts can still resolve the right provider.
  final List<String> aliases;

  /// Network phone prefixes (post-trunk-strip 2-digit prefixes — e.g. "24"
  /// for `024…`). Used to auto-detect the provider from a typed MoMo number.
  final List<String> phonePrefixes;

  const GhanaPaymentMethod({
    required this.id,
    required this.displayName,
    required this.shortLabel,
    required this.aliases,
    required this.phonePrefixes,
  });
}

/// Canonical registry of supported Ghana mobile-money providers.
class GhanaPaymentMethods {
  GhanaPaymentMethods._();

  static const List<GhanaPaymentMethod> all = <GhanaPaymentMethod>[
    GhanaPaymentMethod(
      id: GhanaPaymentMethodId.mtnMomo,
      displayName: 'MTN Mobile Money',
      shortLabel: 'MTN MoMo',
      aliases: <String>['MTN MoMo', 'Mobile Money'],
      // MTN GH: 024, 054, 055, 059.
      phonePrefixes: <String>['24', '54', '55', '59'],
    ),
    GhanaPaymentMethod(
      id: GhanaPaymentMethodId.telecelCash,
      displayName: 'Telecel Cash',
      shortLabel: 'Telecel Cash',
      // Telecel acquired Vodafone Ghana in 2024 — keep the legacy label
      // discoverable for old invoices / customer recall.
      aliases: <String>['Vodafone Cash'],
      // Vodafone / Telecel GH: 020, 050.
      phonePrefixes: <String>['20', '50'],
    ),
    GhanaPaymentMethod(
      id: GhanaPaymentMethodId.airteltigoMoney,
      displayName: 'AirtelTigo Money',
      shortLabel: 'AT Money',
      aliases: <String>['AT Money', 'Airtel Money', 'Tigo Cash'],
      // AirtelTigo: 026, 056 (Airtel) and 027, 057 (Tigo).
      phonePrefixes: <String>['26', '56', '27', '57'],
    ),
  ];

  /// Look up a provider by its stable [GhanaPaymentMethodId].
  static GhanaPaymentMethod byId(GhanaPaymentMethodId id) =>
      all.firstWhere((m) => m.id == id);

  /// Resolve a provider from a phone number (any format
  /// `GhanaPhone.subscriberDigits` accepts). Returns `null` if the prefix
  /// doesn't match any registered provider or the number is invalid.
  static GhanaPaymentMethod? fromPhone(String phone) {
    final sub = GhanaPhone.subscriberDigits(phone);
    if (sub == null) return null;
    final prefix = sub.substring(0, 2);
    for (final m in all) {
      if (m.phonePrefixes.contains(prefix)) return m;
    }
    return null;
  }
}
