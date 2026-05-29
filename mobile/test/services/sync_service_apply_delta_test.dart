// Added: Unit tests for SyncService.applyDelta + flushPendingActions + cache eviction
// (TMAIL-151). Validates that a delta-sync response from /api/mobile/sync is
// correctly persisted into the LocalDatabase cache.

import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite_common_ffi/sqflite_ffi.dart';
import 'package:tasmail_mobile/services/local_database.dart';
import 'package:tasmail_mobile/services/outbound_action_queue.dart';
import 'package:tasmail_mobile/services/sync_preferences.dart';
import 'package:tasmail_mobile/services/sync_service.dart';

Future<LocalDatabase> _newDb() async {
  final db = LocalDatabase(
    factory: databaseFactoryFfi,
    path: inMemoryDatabasePath,
  );
  await db.cachedFolders(); // PURPOSE: Force schema creation
  return db;
}

void main() {
  setUpAll(() {
    sqfliteFfiInit();
  });

  test('applyDelta inserts new_message rows into the cache', () async {
    final db = await _newDb();
    final service = SyncService(localDb: db);

    final delta = SyncDelta.fromJson({
      'changes': [
        {
          'change_type': 'new_message',
          'folder': 'INBOX',
          'uid': 100,
          'data': {
            'from': 'alice@example.com',
            'subject': 'Welcome',
            'date': '2026-05-29T12:00:00Z',
            'is_read': false,
            'is_flagged': false,
            'has_attachment': false,
          },
        },
      ],
      'checkpoint': '2026-05-29T12:00:00Z',
      'has_more': false,
    });

    expect(await service.applyDelta(delta), 1);
    final cached = await db.cachedMessages('INBOX');
    expect(cached, hasLength(1));
    expect(cached.first.uid, 100);
    expect(cached.first.subject, 'Welcome');
    expect(cached.first.from, 'alice@example.com');
    await db.close();
  });

  test('applyDelta toggles flags via flag_change', () async {
    final db = await _newDb();
    final service = SyncService(localDb: db);

    // PURPOSE: Seed an unread message first so the flag_change has something
    //          to update.
    await service.applyDelta(SyncDelta.fromJson({
      'changes': [
        {
          'change_type': 'new_message',
          'folder': 'INBOX',
          'uid': 1,
          'data': {'subject': 'Hi', 'is_read': false, 'is_flagged': false},
        },
      ],
      'checkpoint': 'cp1',
      'has_more': false,
    }));

    expect(
      await service.applyDelta(SyncDelta.fromJson({
        'changes': [
          {
            'change_type': 'flag_change',
            'folder': 'INBOX',
            'uid': 1,
            'data': {'is_read': true, 'is_flagged': true},
          },
        ],
        'checkpoint': 'cp2',
        'has_more': false,
      })),
      1,
    );

    final cached = (await db.cachedMessages('INBOX')).single;
    expect(cached.isRead, isTrue);
    expect(cached.isFlagged, isTrue);
    await db.close();
  });

  test('applyDelta deletion drops cached rows', () async {
    final db = await _newDb();
    final service = SyncService(localDb: db);
    await service.applyDelta(SyncDelta.fromJson({
      'changes': [
        {
          'change_type': 'new_message',
          'folder': 'INBOX',
          'uid': 5,
          'data': {'subject': 'Bye'},
        },
      ],
      'checkpoint': 'cp',
      'has_more': false,
    }));
    expect(await db.cachedMessages('INBOX'), hasLength(1));

    expect(
      await service.applyDelta(SyncDelta.fromJson({
        'changes': [
          {'change_type': 'deletion', 'folder': 'INBOX', 'uid': 5},
        ],
        'checkpoint': 'cp',
        'has_more': false,
      })),
      1,
    );
    expect(await db.cachedMessages('INBOX'), isEmpty);
    await db.close();
  });

  test('applyDelta ignores unknown change types instead of crashing',
      () async {
    final db = await _newDb();
    final service = SyncService(localDb: db);
    final delta = SyncDelta.fromJson({
      'changes': [
        {'change_type': 'mystery', 'folder': 'INBOX', 'uid': 99},
      ],
      'checkpoint': 'cp',
      'has_more': false,
    });
    expect(await service.applyDelta(delta), 0);
    await db.close();
  });

  test('applyDelta is a no-op when LocalDatabase is not wired', () async {
    final service = SyncService();
    final delta = SyncDelta.fromJson({
      'changes': [
        {'change_type': 'new_message', 'folder': 'INBOX', 'uid': 1},
      ],
      'checkpoint': 'cp',
      'has_more': false,
    });
    expect(await service.applyDelta(delta), 0);
    expect(await service.flushPendingActions(), 0);
    expect(await service.evictExpiredCache(), 0);
  });

  test('flushPendingActions drains the queue when DB is wired', () async {
    final db = await _newDb();
    final calls = <String>[];
    final queue = OutboundActionQueue(
      db: db,
      send: (action) async {
        calls.add(action.actionType);
        return true;
      },
    );
    final service = SyncService(localDb: db, outboundQueue: queue);

    await queue.enqueueFlag('INBOX', 1, flagged: true);
    await queue.enqueueDelete('INBOX', 2);

    expect(await service.flushPendingActions(), 2);
    expect(calls, [OutboundActionType.flag, OutboundActionType.delete]);
    await db.close();
  });

  test('evictExpiredCache honours the SyncPreferences retention window',
      () async {
    final db = await _newDb();
    final service = SyncService(
      localDb: db,
      // PURPOSE: 0-day retention so the cutoff is "now" and seeded rows fall
      //          on or before it.
      preferences: const SyncPreferences(retentionDays: 0),
    );
    await service.applyDelta(SyncDelta.fromJson({
      'changes': [
        {
          'change_type': 'new_message',
          'folder': 'INBOX',
          'uid': 1,
          'data': {'subject': 'old'},
        },
      ],
      'checkpoint': 'cp',
      'has_more': false,
    }));
    // PURPOSE: Move "now" forward so the cutoff is strictly after the seeded
    //          cached_at, ensuring the row is evicted.
    final futureNow = DateTime.now().add(const Duration(days: 1));
    expect(await service.evictExpiredCache(now: futureNow), 1);
    expect(await db.cachedMessages('INBOX'), isEmpty);
    await db.close();
  });
}
