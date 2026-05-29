// Added: Unit tests for the SQLite-backed local cache (TMAIL-151)
// PURPOSE: Verify message/folder caching + outbound action queue persistence
//          + retention eviction without spinning up a device. Uses
//          sqflite_common_ffi so the test runs in-process on the host.

import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite_common_ffi/sqflite_ffi.dart';
import 'package:tasmail_mobile/models/email.dart';
import 'package:tasmail_mobile/services/local_database.dart';

Future<LocalDatabase> _newDb() async {
  final db = LocalDatabase(
    factory: databaseFactoryFfi,
    path: inMemoryDatabasePath,
  );
  // PURPOSE: Touch the DB so the schema is created before each assertion.
  await db.cachedFolders();
  return db;
}

void main() {
  setUpAll(() {
    sqfliteFfiInit();
  });

  group('LocalDatabase — cached folders', () {
    test('upsert + read round-trips folder summaries', () async {
      final db = await _newDb();
      await db.upsertFolders(const [
        MobileFolderSummary(name: 'INBOX', unreadCount: 3, totalCount: 42),
        MobileFolderSummary(name: 'Sent', unreadCount: 0, totalCount: 10),
      ]);

      final folders = await db.cachedFolders();
      expect(folders, hasLength(2));
      expect(folders.first.name, 'INBOX');
      expect(folders.first.unreadCount, 3);
      expect(folders.last.name, 'Sent');
      expect(folders.last.totalCount, 10);
      await db.close();
    });

    test('upsert overwrites existing folder counts', () async {
      final db = await _newDb();
      await db.upsertFolders(const [
        MobileFolderSummary(name: 'INBOX', unreadCount: 3, totalCount: 42),
      ]);
      await db.upsertFolders(const [
        MobileFolderSummary(name: 'INBOX', unreadCount: 1, totalCount: 43),
      ]);

      final folders = await db.cachedFolders();
      expect(folders, hasLength(1));
      expect(folders.first.unreadCount, 1);
      expect(folders.first.totalCount, 43);
      await db.close();
    });
  });

  group('LocalDatabase — cached messages', () {
    test('upsert + cachedMessages returns rows for that folder only', () async {
      final db = await _newDb();
      await db.upsertMessages(const [
        MobileMessageSummary(
          uid: 1,
          folder: 'INBOX',
          subject: 'Hi',
          date: '2026-05-01T10:00:00Z',
          isRead: false,
          isFlagged: false,
          hasAttachment: false,
        ),
        MobileMessageSummary(
          uid: 2,
          folder: 'INBOX',
          subject: 'Bye',
          date: '2026-05-02T10:00:00Z',
          isRead: true,
          isFlagged: true,
          hasAttachment: true,
        ),
        MobileMessageSummary(
          uid: 99,
          folder: 'Sent',
          subject: 'Outbound',
          date: '2026-05-03T10:00:00Z',
          isRead: true,
          isFlagged: false,
          hasAttachment: false,
        ),
      ]);

      final inbox = await db.cachedMessages('INBOX');
      expect(inbox.map((m) => m.uid), [2, 1]); // ORDER BY date DESC
      expect(inbox.first.isFlagged, isTrue);
      expect(inbox.first.hasAttachment, isTrue);

      final sent = await db.cachedMessages('Sent');
      expect(sent, hasLength(1));
      expect(sent.first.uid, 99);
      await db.close();
    });

    test('updateCachedMessageFlags toggles only the requested columns',
        () async {
      final db = await _newDb();
      await db.upsertMessages(const [
        MobileMessageSummary(
          uid: 1,
          folder: 'INBOX',
          subject: 'Hi',
          date: '2026-05-01T10:00:00Z',
          isRead: false,
          isFlagged: false,
          hasAttachment: false,
        ),
      ]);

      final updated = await db.updateCachedMessageFlags(
        'INBOX',
        1,
        isRead: true,
      );
      expect(updated, 1);

      final after = await db.cachedMessages('INBOX');
      expect(after.first.isRead, isTrue);
      expect(after.first.isFlagged, isFalse);
      await db.close();
    });

    test('deleteCachedMessage removes a single row', () async {
      final db = await _newDb();
      await db.upsertMessages(const [
        MobileMessageSummary(
          uid: 1,
          folder: 'INBOX',
          subject: 'Hi',
          date: '2026-05-01T10:00:00Z',
          isRead: false,
          isFlagged: false,
          hasAttachment: false,
        ),
      ]);

      final removed = await db.deleteCachedMessage('INBOX', 1);
      expect(removed, 1);
      expect(await db.cachedMessages('INBOX'), isEmpty);
      await db.close();
    });

    test('evictMessagesOlderThan drops stale rows only', () async {
      final db = await _newDb();
      await db.upsertMessages(const [
        MobileMessageSummary(
          uid: 1,
          folder: 'INBOX',
          subject: 'Old',
          date: '2026-01-01T10:00:00Z',
          isRead: false,
          isFlagged: false,
          hasAttachment: false,
        ),
        MobileMessageSummary(
          uid: 2,
          folder: 'INBOX',
          subject: 'Fresh',
          date: '2026-05-29T10:00:00Z',
          isRead: false,
          isFlagged: false,
          hasAttachment: false,
        ),
      ]);

      // PURPOSE: Cutoff far in the past evicts nothing; cutoff far in the
      //          future evicts everything. Covers the SQL boundary without
      //          having to back-date cached_at.
      expect(await db.evictMessagesOlderThan(DateTime(1970)), 0);
      expect(await db.cachedMessages('INBOX'), hasLength(2));

      expect(await db.evictMessagesOlderThan(DateTime(9999)), 2);
      expect(await db.cachedMessages('INBOX'), isEmpty);
      await db.close();
    });
  });

  group('LocalDatabase — pending actions queue', () {
    test('enqueue + pendingActions returns FIFO order with payload', () async {
      final db = await _newDb();
      final id1 = await db.enqueueAction(
        actionType: OutboundActionType.flag,
        folder: 'INBOX',
        uid: 1,
      );
      final id2 = await db.enqueueAction(
        actionType: OutboundActionType.move,
        folder: 'INBOX',
        uid: 2,
        payload: {'to_folder': 'Archive'},
      );
      expect(id1, isNot(id2));

      expect(await db.pendingActionCount(), 2);

      final actions = await db.pendingActions();
      expect(actions, hasLength(2));
      expect(actions.first.actionType, OutboundActionType.flag);
      expect(actions.first.folder, 'INBOX');
      expect(actions.first.uid, 1);
      expect(actions.last.actionType, OutboundActionType.move);
      expect(actions.last.payload['to_folder'], 'Archive');
      await db.close();
    });

    test('deleteAction + recordActionFailure track replay attempts',
        () async {
      final db = await _newDb();
      final id = await db.enqueueAction(
        actionType: OutboundActionType.delete,
        folder: 'INBOX',
        uid: 7,
      );

      await db.recordActionFailure(id, 'network_down');
      await db.recordActionFailure(id, 'network_down');

      final after = (await db.pendingActions()).single;
      expect(after.attempts, 2);
      expect(after.lastError, 'network_down');

      expect(await db.deleteAction(id), 1);
      expect(await db.pendingActionCount(), 0);
      await db.close();
    });
  });

  group('LocalDatabase — sync checkpoints', () {
    test('upsertCheckpoint persists per-folder CONDSTORE state', () async {
      final db = await _newDb();
      await db.upsertCheckpoint(
        folder: 'INBOX',
        lastUid: 42,
        lastModseq: 100,
        uidvalidity: 1234,
        lastSyncedAt: DateTime(2026, 5, 29),
      );

      final row = await db.readCheckpoint('INBOX');
      expect(row, isNotNull);
      expect(row!['last_uid'], 42);
      expect(row['last_modseq'], 100);
      expect(row['uidvalidity'], 1234);
      expect(row['last_synced_at'], DateTime(2026, 5, 29).millisecondsSinceEpoch);

      // PURPOSE: Upsert path: same folder writes through.
      await db.upsertCheckpoint(
        folder: 'INBOX',
        lastUid: 43,
        lastModseq: 101,
        uidvalidity: 1234,
      );
      final updated = await db.readCheckpoint('INBOX');
      expect(updated!['last_uid'], 43);
      await db.close();
    });

    test('readCheckpoint returns null for unknown folders', () async {
      final db = await _newDb();
      expect(await db.readCheckpoint('Nope'), isNull);
      await db.close();
    });
  });
}
