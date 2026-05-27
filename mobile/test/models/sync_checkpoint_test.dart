// Added: Tests for SyncCheckpoint model (TMAIL-51)
import 'package:flutter_test/flutter_test.dart';
import 'package:tasmail_mobile/models/sync_checkpoint.dart';

void main() {
  group('SyncCheckpoint serialization', () {
    test('parses checkpoint from backend JSON', () {
      final cp = SyncCheckpoint.fromJson({
        'folder_name': 'INBOX',
        'device_id': 'd-1',
        'last_uid': 42,
        'last_modseq': 1000,
        'uidvalidity': 1234567890,
        'last_synced_at': '2026-04-15T10:00:00Z',
      });
      expect(cp.folderName, 'INBOX');
      expect(cp.deviceId, 'd-1');
      expect(cp.lastUid, 42);
      expect(cp.lastModseq, 1000);
      expect(cp.uidvalidity, 1234567890);
      expect(cp.lastSyncedAt, isNotNull);
      expect(cp.needsFullSync, isFalse);
    });

    test('treats missing uidvalidity as needs_full_sync', () {
      final cp = SyncCheckpoint.fromJson({
        'folder_name': 'INBOX',
        'last_uid': 0,
        'last_modseq': 0,
        'uidvalidity': 0,
      });
      expect(cp.needsFullSync, isTrue);
    });

    test('roundtrips through JSON', () {
      final original = SyncCheckpoint(
        folderName: 'Sent',
        deviceId: null,
        lastUid: 7,
        lastModseq: 14,
        uidvalidity: 99,
        lastSyncedAt: DateTime.utc(2026, 5, 27),
      );
      final restored = SyncCheckpoint.fromJson(original.toJson());
      expect(restored.folderName, original.folderName);
      expect(restored.lastUid, 7);
      expect(restored.lastModseq, 14);
      expect(restored.uidvalidity, 99);
      expect(restored.lastSyncedAt, original.lastSyncedAt);
    });
  });

  group('SyncCheckpoint.requiresResyncAfter', () {
    test('returns true when client never synced', () {
      const client = SyncCheckpoint(folderName: 'INBOX');
      const server = SyncCheckpoint(folderName: 'INBOX', uidvalidity: 99);
      expect(client.requiresResyncAfter(server), isTrue);
    });

    test('returns true when server uidvalidity changes', () {
      final client = SyncCheckpoint(
        folderName: 'INBOX',
        uidvalidity: 1,
        lastUid: 10,
        lastSyncedAt: DateTime.utc(2026, 5, 1),
      );
      final server = SyncCheckpoint(folderName: 'INBOX', uidvalidity: 2);
      expect(client.requiresResyncAfter(server), isTrue);
    });

    test('returns false when both uidvalidity values match', () {
      final client = SyncCheckpoint(
        folderName: 'INBOX',
        uidvalidity: 42,
        lastUid: 10,
        lastSyncedAt: DateTime.utc(2026, 5, 1),
      );
      final server = SyncCheckpoint(folderName: 'INBOX', uidvalidity: 42);
      expect(client.requiresResyncAfter(server), isFalse);
    });
  });
}
