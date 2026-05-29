// Added: Unit tests for the outbound action queue replay logic (TMAIL-151)
// PURPOSE: Verify that user actions enqueued while offline (flag/read/delete/move)
//          are replayed in FIFO order, that successful replays drop the row,
//          and that the at-most-N retry policy drops poison actions.

import 'package:flutter_test/flutter_test.dart';
import 'package:sqflite_common_ffi/sqflite_ffi.dart';
import 'package:tasmail_mobile/services/local_database.dart';
import 'package:tasmail_mobile/services/outbound_action_queue.dart';

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

  test('drain replays actions in FIFO order and clears them on success',
      () async {
    final db = await _newDb();
    final calls = <String>[];
    final queue = OutboundActionQueue(
      db: db,
      send: (action) async {
        calls.add('${action.actionType}:${action.folder}:${action.uid}');
        return true;
      },
    );

    await queue.enqueueFlag('INBOX', 1, flagged: true);
    await queue.enqueueRead('INBOX', 2, read: true);
    await queue.enqueueMove('INBOX', 3, 'Archive');
    await queue.enqueueDelete('INBOX', 4);

    expect(await queue.pendingCount(), 4);

    final sent = await queue.drain();
    expect(sent, 4);
    expect(calls, [
      'flag:INBOX:1',
      'mark_read:INBOX:2',
      'move:INBOX:3',
      'delete:INBOX:4',
    ]);
    expect(await queue.pendingCount(), 0);
    await db.close();
  });

  test('failed sends bump attempts and keep the action queued', () async {
    final db = await _newDb();
    int callCount = 0;
    final queue = OutboundActionQueue(
      db: db,
      send: (action) async {
        callCount += 1;
        return false;
      },
    );

    await queue.enqueueFlag('INBOX', 1, flagged: true);

    expect(await queue.drain(), 0);
    expect(callCount, 1);
    expect(await queue.pendingCount(), 1);

    final after = (await db.pendingActions()).single;
    expect(after.attempts, 1);
    expect(after.lastError, 'replay_failed');
    await db.close();
  });

  test('drops actions after kMaxOutboundActionAttempts failures', () async {
    final db = await _newDb();
    final queue = OutboundActionQueue(
      db: db,
      send: (action) async => false,
    );
    await queue.enqueueFlag('INBOX', 1, flagged: true);

    // PURPOSE: Each drain bumps attempts by 1; after kMax-1 failures the next
    //          failure should drop the row.
    for (var i = 0; i < kMaxOutboundActionAttempts; i++) {
      await queue.drain();
    }
    expect(await queue.pendingCount(), 0);
    await db.close();
  });

  test('move action without to_folder is reported as failure', () async {
    final db = await _newDb();
    bool senderCalled = false;
    final queue = OutboundActionQueue(
      db: db,
      send: (action) async {
        senderCalled = true;
        return true;
      },
    );

    // PURPOSE: Manually enqueue a malformed move so we exercise the validation
    //          path inside the default sender. Use the default sender (no
    //          override) so the missing payload check is what fails.
    final defaultQueue = OutboundActionQueue(db: db);
    await db.enqueueAction(
      actionType: OutboundActionType.move,
      folder: 'INBOX',
      uid: 9,
      // PURPOSE: payload intentionally omits to_folder.
    );

    // PURPOSE: The default sender catches the network call inside try/catch and
    //          returns false. We just check the queue keeps the row.
    await defaultQueue.drain();
    expect(await defaultQueue.pendingCount(), 1);

    // Custom sender path stays untouched.
    expect(senderCalled, isFalse);
    expect(queue, isNotNull);
    await db.close();
  });

  test('enqueue/unflag/markUnread use the negative action types', () async {
    final db = await _newDb();
    final queue = OutboundActionQueue(db: db);
    await queue.enqueueFlag('INBOX', 1, flagged: false);
    await queue.enqueueRead('INBOX', 1, read: false);

    final pending = await db.pendingActions();
    expect(pending.map((a) => a.actionType), [
      OutboundActionType.unflag,
      OutboundActionType.markUnread,
    ]);
    await db.close();
  });
}
