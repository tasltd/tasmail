// Added: SQLite-backed local cache for TMAIL-151 offline-first mobile sync
// PURPOSE: Persist message summaries, folder counts, and a pending outbound-action
//          queue across app restarts so the mobile app can:
//            1. Render Inbox/folders from cache while offline.
//            2. Queue user actions (flag / read / delete / move) made offline and
//               replay them when connectivity returns.
//            3. Apply backend delta-sync changes (/api/mobile/sync) into a single
//               source of truth instead of an in-memory list.
// EXTERNAL: Schema is private to the mobile app — backend never touches this DB.
//           Wire types match models/email.dart (MobileMessageSummary) so the
//           cache and the network response are interchangeable to the UI layer.
// NOTE: Database factory is injectable so unit tests can swap in
//       `sqflite_common_ffi`'s `databaseFactoryFfi` and run with no emulator.

import 'dart:async';
import 'dart:convert';

import 'package:path/path.dart' as p;
import 'package:sqflite/sqflite.dart';

import '../models/email.dart';

// PURPOSE: Bump when changing the schema so onUpgrade can migrate or drop+recreate.
const int _kSchemaVersion = 1;

// PURPOSE: On-disk file name. One DB per app install — multi-account support is
//          a TMAIL-152+ concern and can prefix tables with an account_id when needed.
const String _kDbFileName = 'tasmail_cache.db';

// PURPOSE: Outbound action types we know how to replay. Kept as strings (not enum)
//          so unknown action types in older DB rows can still be inspected/dropped.
class OutboundActionType {
  static const String flag = 'flag';
  static const String unflag = 'unflag';
  static const String markRead = 'mark_read';
  static const String markUnread = 'mark_unread';
  static const String delete = 'delete';
  static const String move = 'move';
}

class PendingAction {
  final int id;
  final String actionType;
  final String folder;
  final int uid;
  final Map<String, dynamic> payload;
  final DateTime createdAt;
  final int attempts;
  final String? lastError;

  const PendingAction({
    required this.id,
    required this.actionType,
    required this.folder,
    required this.uid,
    required this.payload,
    required this.createdAt,
    required this.attempts,
    this.lastError,
  });

  factory PendingAction.fromRow(Map<String, Object?> row) {
    final raw = row['payload'] as String?;
    return PendingAction(
      id: row['id'] as int,
      actionType: row['action_type'] as String,
      folder: row['folder'] as String,
      uid: (row['uid'] as int?) ?? 0,
      payload: raw == null || raw.isEmpty
          ? const <String, dynamic>{}
          : (jsonDecode(raw) as Map<String, dynamic>),
      createdAt: DateTime.fromMillisecondsSinceEpoch(row['created_at'] as int),
      attempts: (row['attempts'] as int?) ?? 0,
      lastError: row['last_error'] as String?,
    );
  }
}

class LocalDatabase {
  final DatabaseFactory _factory;
  final String _path;
  Database? _db;

  // PURPOSE: Default constructor uses the platform sqflite factory and the
  //          standard databases directory. Tests pass `databaseFactoryFfi` plus
  //          `inMemoryDatabasePath` (or a temp file) for isolated runs.
  LocalDatabase({DatabaseFactory? factory, String? path})
      : _factory = factory ?? databaseFactory,
        _path = path ?? _kDbFileName;

  Future<Database> _open() async {
    if (_db != null) return _db!;
    // PURPOSE: Resolve relative path against the platform databases dir; pass
    //          `inMemoryDatabasePath` straight through for tests.
    String resolved = _path;
    if (_path != inMemoryDatabasePath && !p.isAbsolute(_path)) {
      final base = await _factory.getDatabasesPath();
      resolved = p.join(base, _path);
    }
    _db = await _factory.openDatabase(
      resolved,
      options: OpenDatabaseOptions(
        version: _kSchemaVersion,
        onConfigure: (db) async {
          await db.execute('PRAGMA foreign_keys = ON');
        },
        onCreate: (db, version) async {
          await _createSchema(db);
        },
        onUpgrade: (db, oldVersion, newVersion) async {
          // NOTE: While the schema is v1 we never hit this branch. Future
          //       versions should ALTER instead of dropping to preserve drafts.
          await _createSchema(db);
        },
      ),
    );
    return _db!;
  }

  Future<void> _createSchema(Database db) async {
    await db.execute('''
      CREATE TABLE IF NOT EXISTS cached_folders (
        name TEXT PRIMARY KEY,
        unread_count INTEGER NOT NULL DEFAULT 0,
        total_count INTEGER NOT NULL DEFAULT 0,
        cached_at INTEGER NOT NULL
      )
    ''');
    await db.execute('''
      CREATE TABLE IF NOT EXISTS cached_messages (
        folder TEXT NOT NULL,
        uid INTEGER NOT NULL,
        from_addr TEXT,
        subject TEXT,
        date TEXT,
        is_read INTEGER NOT NULL DEFAULT 0,
        is_flagged INTEGER NOT NULL DEFAULT 0,
        has_attachment INTEGER NOT NULL DEFAULT 0,
        payload TEXT NOT NULL,
        cached_at INTEGER NOT NULL,
        PRIMARY KEY (folder, uid)
      )
    ''');
    await db.execute(
      'CREATE INDEX IF NOT EXISTS idx_cached_messages_folder_date '
      'ON cached_messages(folder, date DESC)',
    );
    await db.execute('''
      CREATE TABLE IF NOT EXISTS pending_actions (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        action_type TEXT NOT NULL,
        folder TEXT NOT NULL,
        uid INTEGER NOT NULL,
        payload TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        attempts INTEGER NOT NULL DEFAULT 0,
        last_error TEXT
      )
    ''');
    await db.execute(
      'CREATE INDEX IF NOT EXISTS idx_pending_actions_created '
      'ON pending_actions(created_at ASC)',
    );
    await db.execute('''
      CREATE TABLE IF NOT EXISTS sync_checkpoints (
        folder TEXT PRIMARY KEY,
        last_uid INTEGER NOT NULL DEFAULT 0,
        last_modseq INTEGER NOT NULL DEFAULT 0,
        uidvalidity INTEGER NOT NULL DEFAULT 0,
        last_synced_at INTEGER
      )
    ''');
  }

  // ---------------------------------------------------------------------------
  // Cached folders
  // ---------------------------------------------------------------------------

  Future<void> upsertFolders(List<MobileFolderSummary> folders) async {
    final db = await _open();
    final now = DateTime.now().millisecondsSinceEpoch;
    final batch = db.batch();
    for (final f in folders) {
      batch.insert(
        'cached_folders',
        {
          'name': f.name,
          'unread_count': f.unreadCount,
          'total_count': f.totalCount,
          'cached_at': now,
        },
        conflictAlgorithm: ConflictAlgorithm.replace,
      );
    }
    await batch.commit(noResult: true);
  }

  Future<List<MobileFolderSummary>> cachedFolders() async {
    final db = await _open();
    final rows = await db.query('cached_folders', orderBy: 'name ASC');
    return rows
        .map((r) => MobileFolderSummary(
              name: r['name'] as String,
              unreadCount: r['unread_count'] as int? ?? 0,
              totalCount: r['total_count'] as int? ?? 0,
            ))
        .toList();
  }

  // ---------------------------------------------------------------------------
  // Cached messages
  // ---------------------------------------------------------------------------

  Future<void> upsertMessages(List<MobileMessageSummary> messages) async {
    if (messages.isEmpty) return;
    final db = await _open();
    final now = DateTime.now().millisecondsSinceEpoch;
    final batch = db.batch();
    for (final m in messages) {
      batch.insert(
        'cached_messages',
        {
          'folder': m.folder,
          'uid': m.uid,
          'from_addr': m.from,
          'subject': m.subject,
          'date': m.date,
          'is_read': m.isRead ? 1 : 0,
          'is_flagged': m.isFlagged ? 1 : 0,
          'has_attachment': m.hasAttachment ? 1 : 0,
          'payload': jsonEncode({
            'uid': m.uid,
            'folder': m.folder,
            'from': m.from,
            'subject': m.subject,
            'date': m.date,
            'is_read': m.isRead,
            'is_flagged': m.isFlagged,
            'has_attachment': m.hasAttachment,
          }),
          'cached_at': now,
        },
        conflictAlgorithm: ConflictAlgorithm.replace,
      );
    }
    await batch.commit(noResult: true);
  }

  Future<List<MobileMessageSummary>> cachedMessages(
    String folder, {
    int? limit,
  }) async {
    final db = await _open();
    final rows = await db.query(
      'cached_messages',
      where: 'folder = ?',
      whereArgs: [folder],
      orderBy: 'date DESC, uid DESC',
      limit: limit,
    );
    return rows
        .map((r) => MobileMessageSummary(
              uid: r['uid'] as int,
              folder: r['folder'] as String,
              from: r['from_addr'] as String?,
              subject: r['subject'] as String?,
              date: r['date'] as String?,
              isRead: (r['is_read'] as int? ?? 0) == 1,
              isFlagged: (r['is_flagged'] as int? ?? 0) == 1,
              hasAttachment: (r['has_attachment'] as int? ?? 0) == 1,
            ))
        .toList();
  }

  Future<int> deleteCachedMessage(String folder, int uid) async {
    final db = await _open();
    return db.delete(
      'cached_messages',
      where: 'folder = ? AND uid = ?',
      whereArgs: [folder, uid],
    );
  }

  // PURPOSE: Apply a flag/read change to the cache after the user toggles it
  //          locally — keeps optimistic UI consistent across app restarts.
  Future<int> updateCachedMessageFlags(
    String folder,
    int uid, {
    bool? isRead,
    bool? isFlagged,
  }) async {
    if (isRead == null && isFlagged == null) return 0;
    final db = await _open();
    final values = <String, Object?>{
      if (isRead != null) 'is_read': isRead ? 1 : 0,
      if (isFlagged != null) 'is_flagged': isFlagged ? 1 : 0,
    };
    return db.update(
      'cached_messages',
      values,
      where: 'folder = ? AND uid = ?',
      whereArgs: [folder, uid],
    );
  }

  // PURPOSE: Honour SyncPreferences.retentionDays — drop summaries older than
  //          the cutoff. Drafts/pending actions are intentionally not affected.
  Future<int> evictMessagesOlderThan(DateTime cutoff) async {
    final db = await _open();
    return db.delete(
      'cached_messages',
      where: 'cached_at < ?',
      whereArgs: [cutoff.millisecondsSinceEpoch],
    );
  }

  // ---------------------------------------------------------------------------
  // Outbound action queue
  // ---------------------------------------------------------------------------

  Future<int> enqueueAction({
    required String actionType,
    required String folder,
    required int uid,
    Map<String, dynamic>? payload,
  }) async {
    final db = await _open();
    return db.insert('pending_actions', {
      'action_type': actionType,
      'folder': folder,
      'uid': uid,
      'payload': jsonEncode(payload ?? const <String, dynamic>{}),
      'created_at': DateTime.now().millisecondsSinceEpoch,
      'attempts': 0,
    });
  }

  Future<List<PendingAction>> pendingActions({int? limit}) async {
    final db = await _open();
    final rows = await db.query(
      'pending_actions',
      orderBy: 'created_at ASC, id ASC',
      limit: limit,
    );
    return rows.map(PendingAction.fromRow).toList();
  }

  Future<int> pendingActionCount() async {
    final db = await _open();
    final result = await db.rawQuery('SELECT COUNT(*) AS c FROM pending_actions');
    return (result.first['c'] as int?) ?? 0;
  }

  Future<int> deleteAction(int id) async {
    final db = await _open();
    return db.delete('pending_actions', where: 'id = ?', whereArgs: [id]);
  }

  Future<int> recordActionFailure(int id, String error) async {
    final db = await _open();
    return db.rawUpdate(
      'UPDATE pending_actions SET attempts = attempts + 1, last_error = ? WHERE id = ?',
      [error, id],
    );
  }

  // ---------------------------------------------------------------------------
  // Sync checkpoints (per-folder)
  // ---------------------------------------------------------------------------

  Future<void> upsertCheckpoint({
    required String folder,
    required int lastUid,
    required int lastModseq,
    required int uidvalidity,
    DateTime? lastSyncedAt,
  }) async {
    final db = await _open();
    await db.insert(
      'sync_checkpoints',
      {
        'folder': folder,
        'last_uid': lastUid,
        'last_modseq': lastModseq,
        'uidvalidity': uidvalidity,
        'last_synced_at':
            (lastSyncedAt ?? DateTime.now()).millisecondsSinceEpoch,
      },
      conflictAlgorithm: ConflictAlgorithm.replace,
    );
  }

  Future<Map<String, Object?>?> readCheckpoint(String folder) async {
    final db = await _open();
    final rows = await db.query(
      'sync_checkpoints',
      where: 'folder = ?',
      whereArgs: [folder],
      limit: 1,
    );
    return rows.isEmpty ? null : rows.first;
  }

  // ---------------------------------------------------------------------------
  // Lifecycle
  // ---------------------------------------------------------------------------

  Future<void> close() async {
    final db = _db;
    if (db != null) {
      await db.close();
      _db = null;
    }
  }

  // PURPOSE: Test helper — wipe all rows without dropping the schema.
  Future<void> resetForTests() async {
    final db = await _open();
    await db.delete('cached_messages');
    await db.delete('cached_folders');
    await db.delete('pending_actions');
    await db.delete('sync_checkpoints');
  }
}
