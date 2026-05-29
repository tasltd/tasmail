// Added: Tests for Ghana payment methods registry (TMAIL-57)
import 'package:flutter_test/flutter_test.dart';
import 'package:tasmail_mobile/utils/ghana_payment_methods.dart';

void main() {
  group('GhanaPaymentMethods registry', () {
    test('includes MTN MoMo, Telecel Cash and AirtelTigo Money', () {
      final ids = GhanaPaymentMethods.all.map((m) => m.id).toSet();
      expect(ids, contains(GhanaPaymentMethodId.mtnMomo));
      expect(ids, contains(GhanaPaymentMethodId.telecelCash));
      expect(ids, contains(GhanaPaymentMethodId.airteltigoMoney));
    });

    test('Telecel Cash entry preserves the legacy "Vodafone Cash" alias', () {
      final telecel =
          GhanaPaymentMethods.byId(GhanaPaymentMethodId.telecelCash);
      expect(telecel.displayName, 'Telecel Cash');
      expect(telecel.aliases, contains('Vodafone Cash'));
    });

    test('MTN MoMo entry has correct short label and prefixes', () {
      final mtn = GhanaPaymentMethods.byId(GhanaPaymentMethodId.mtnMomo);
      expect(mtn.shortLabel, 'MTN MoMo');
      expect(mtn.phonePrefixes, containsAll(<String>['24', '54', '55', '59']));
    });
  });

  group('GhanaPaymentMethods.fromPhone', () {
    test('detects MTN MoMo from 024 number', () {
      final p = GhanaPaymentMethods.fromPhone('024 123 4567');
      expect(p, isNotNull);
      expect(p!.id, GhanaPaymentMethodId.mtnMomo);
    });

    test('detects MTN MoMo from +233 59 prefix', () {
      final p = GhanaPaymentMethods.fromPhone('+233591234567');
      expect(p!.id, GhanaPaymentMethodId.mtnMomo);
    });

    test('detects Telecel Cash from 020 number', () {
      final p = GhanaPaymentMethods.fromPhone('0205550001');
      expect(p!.id, GhanaPaymentMethodId.telecelCash);
    });

    test('detects Telecel Cash from legacy Vodafone 050 number', () {
      final p = GhanaPaymentMethods.fromPhone('0505550001');
      expect(p!.id, GhanaPaymentMethodId.telecelCash);
    });

    test('detects AirtelTigo Money from 027 number', () {
      final p = GhanaPaymentMethods.fromPhone('0275550001');
      expect(p!.id, GhanaPaymentMethodId.airteltigoMoney);
    });

    test('detects AirtelTigo Money from 056 number', () {
      final p = GhanaPaymentMethods.fromPhone('0565550001');
      expect(p!.id, GhanaPaymentMethodId.airteltigoMoney);
    });

    test('returns null for invalid Ghana number', () {
      expect(GhanaPaymentMethods.fromPhone('hello'), isNull);
      expect(GhanaPaymentMethods.fromPhone('+15551234567'), isNull);
    });

    test('returns null for valid Ghana landline (not a mobile wallet)', () {
      // Landlines start with 3 — outside the [257] mobile range and so
      // GhanaPhone.subscriberDigits rejects them upstream.
      expect(GhanaPaymentMethods.fromPhone('+233302123456'), isNull);
    });
  });
}
