// Added: Tests for SyncPreferences (TMAIL-51)
import 'package:flutter_test/flutter_test.dart';
import 'package:tasmail_mobile/services/sync_preferences.dart';

void main() {
  group('SyncPreferences defaults', () {
    test('default mode is wifi-only to protect cellular plans', () {
      const prefs = SyncPreferences();
      expect(prefs.mode, SyncMode.wifiOnly);
      expect(prefs.retentionDays, 30);
      expect(prefs.maxAutoBodyKb, 256);
    });
  });

  group('SyncPreferences.canSyncOnNetwork', () {
    test('wifi-only mode blocks cellular and allows wifi', () {
      const prefs = SyncPreferences();
      expect(prefs.canSyncOnNetwork(onWifi: false), isFalse);
      expect(prefs.canSyncOnNetwork(onWifi: true), isTrue);
    });

    test('auto mode allows both networks', () {
      const prefs = SyncPreferences(mode: SyncMode.auto);
      expect(prefs.canSyncOnNetwork(onWifi: false), isTrue);
      expect(prefs.canSyncOnNetwork(onWifi: true), isTrue);
    });

    test('manual and disabled never auto-sync', () {
      const manual = SyncPreferences(mode: SyncMode.manual);
      const disabled = SyncPreferences(mode: SyncMode.disabled);
      expect(manual.canSyncOnNetwork(onWifi: true), isFalse);
      expect(disabled.canSyncOnNetwork(onWifi: true), isFalse);
    });
  });

  group('SyncPreferences.retentionCutoff', () {
    test('subtracts retention days from supplied now', () {
      final now = DateTime.utc(2026, 6, 1);
      const prefs = SyncPreferences(retentionDays: 7);
      expect(prefs.retentionCutoff(now: now), DateTime.utc(2026, 5, 25));
    });
  });

  group('SyncPreferences serialization', () {
    test('roundtrips through JSON', () {
      const original = SyncPreferences(
        mode: SyncMode.auto,
        retentionDays: 14,
        maxAutoBodyKb: 512,
      );
      final restored = SyncPreferences.fromJson(original.toJson());
      expect(restored.mode, SyncMode.auto);
      expect(restored.retentionDays, 14);
      expect(restored.maxAutoBodyKb, 512);
    });

    test('falls back to defaults on unknown mode', () {
      final restored = SyncPreferences.fromJson({'mode': 'nonsense'});
      expect(restored.mode, SyncMode.wifiOnly);
      expect(restored.retentionDays, SyncPreferences.defaultRetentionDays);
    });
  });

  group('SyncPreferences.copyWith', () {
    test('preserves untouched fields', () {
      const prefs = SyncPreferences();
      final updated = prefs.copyWith(mode: SyncMode.auto);
      expect(updated.mode, SyncMode.auto);
      expect(updated.retentionDays, prefs.retentionDays);
      expect(updated.maxAutoBodyKb, prefs.maxAutoBodyKb);
    });
  });
}
