// Added: Offline sync service for TMAIL-51 offline-first protocol
// PURPOSE: Delta sync with backend + per-folder checkpoint management + conflict
//          resolution. Wraps the /api/sync/* endpoints (checkpoints, resolve-conflict)
//          and the /api/mobile/sync delta endpoint.
// EXTERNAL: Uses /api/mobile/sync, /api/sync/checkpoints, /api/sync/checkpoint/{folder},
//           /api/sync/resolve-conflict — see backend/src/handlers/sync.rs.

import '../api/api_client.dart';
import '../models/email.dart';
import '../models/sync_checkpoint.dart';
import 'local_database.dart';
import 'offline_draft_queue.dart';
import 'outbound_action_queue.dart';
import 'sync_preferences.dart';

// Added: Represents a single sync change from the backend
class SyncChange {
  final String changeType; // 'new_message', 'flag_change', 'deletion'
  final String folder;
  final int uid;
  final Map<String, dynamic>? data;

  const SyncChange({
    required this.changeType,
    required this.folder,
    required this.uid,
    this.data,
  });

  factory SyncChange.fromJson(Map<String, dynamic> json) {
    return SyncChange(
      changeType: json['change_type'] as String,
      folder: json['folder'] as String,
      uid: json['uid'] as int,
      data: json['data'] as Map<String, dynamic>?,
    );
  }
}

// Added: Delta sync response
class SyncDelta {
  final List<SyncChange> changes;
  final String checkpoint;
  final bool hasMore;

  const SyncDelta({
    required this.changes,
    required this.checkpoint,
    required this.hasMore,
  });

  factory SyncDelta.fromJson(Map<String, dynamic> json) {
    return SyncDelta(
      changes: (json['changes'] as List<dynamic>)
          .map((c) => SyncChange.fromJson(c as Map<String, dynamic>))
          .toList(),
      checkpoint: json['checkpoint'] as String,
      hasMore: json['has_more'] as bool? ?? false,
    );
  }
}

class SyncService {
  // PURPOSE: Defaults preserve the original singleton-style call sites.
  final ApiClient _api;
  // PURPOSE: User-tunable sync mode + retention; settable from the settings UI.
  SyncPreferences preferences;
  // PURPOSE: Offline drafts composed while the device cannot reach the backend.
  final OfflineDraftQueue draftQueue;
  // PURPOSE: SQLite-backed cache for messages/folders + outbound action queue.
  //          Optional so legacy call sites that only used network sync still work.
  final LocalDatabase? localDb;
  // PURPOSE: Replay flag/read/delete/move actions queued while offline.
  final OutboundActionQueue? outboundQueue;

  SyncService({
    ApiClient? api,
    SyncPreferences? preferences,
    OfflineDraftQueue? draftQueue,
    this.localDb,
    OutboundActionQueue? outboundQueue,
  })  : _api = api ?? ApiClient(),
        preferences = preferences ?? const SyncPreferences(),
        draftQueue = draftQueue ?? OfflineDraftQueue(),
        outboundQueue = outboundQueue ??
            (localDb != null
                ? OutboundActionQueue(db: localDb, api: api)
                : null);

  // PURPOSE: Fetch changes since a given timestamp (legacy delta endpoint).
  Future<SyncDelta?> fetchDelta(String since) async {
    try {
      final response = await _api.get('/mobile/sync', queryParams: {
        'since': since,
      });
      return SyncDelta.fromJson(response.data as Map<String, dynamic>);
    } catch (_) {
      return null;
    }
  }

  // PURPOSE: Fetch every folder's checkpoint in one round-trip on app launch / wake.
  //          Backed by GET /api/sync/checkpoints.
  Future<List<SyncCheckpoint>> listCheckpoints() async {
    try {
      final response = await _api.get('/sync/checkpoints');
      final data = response.data as Map<String, dynamic>;
      final raw = data['checkpoints'] as List<dynamic>? ?? const [];
      return raw
          .map((c) => SyncCheckpoint.fromJson(c as Map<String, dynamic>))
          .toList();
    } catch (_) {
      return const [];
    }
  }

  // PURPOSE: Fetch a single folder's full checkpoint (returns needs_full_sync=true on
  //          first sync). Backed by GET /api/sync/checkpoint/{folder}.
  Future<SyncCheckpoint?> getFolderCheckpoint(String folder, {String? deviceId}) async {
    try {
      final response = await _api.get(
        '/sync/checkpoint/$folder',
        queryParams: deviceId != null ? {'device_id': deviceId} : null,
      );
      return SyncCheckpoint.fromJson(response.data as Map<String, dynamic>);
    } catch (_) {
      return null;
    }
  }

  // PURPOSE: Push a fresh sync state to the backend after the client has applied
  //          the latest IMAP changes for a folder. Backed by POST
  //          /api/sync/checkpoint/{folder}.
  Future<bool> updateFolderCheckpoint({
    required String folder,
    required int lastUid,
    required int lastModseq,
    required int uidvalidity,
    String? deviceId,
  }) async {
    try {
      await _api.post('/sync/checkpoint/$folder', data: {
        if (deviceId != null) 'device_id': deviceId,
        'last_uid': lastUid,
        'last_modseq': lastModseq,
        'uidvalidity': uidvalidity,
      });
      return true;
    } catch (_) {
      return false;
    }
  }

  // PURPOSE: Legacy single-string checkpoint accessor — kept for prior call sites.
  Future<String?> getCheckpoint(String folder) async {
    try {
      final response = await _api.get('/sync/checkpoint/$folder');
      return response.data['checkpoint'] as String?;
    } catch (_) {
      return null;
    }
  }

  // PURPOSE: Resolve a sync conflict — flag/state mismatch between client and server.
  //          Mirrors backend ConflictResolution (server_wins / client_wins / merge).
  Future<bool> resolveConflict({
    required String folder,
    required int uid,
    required String resolution,
    List<String>? clientFlags,
  }) async {
    try {
      await _api.post('/sync/resolve-conflict', data: {
        'folder': folder,
        'uid': uid,
        'resolution': resolution,
        if (clientFlags != null) 'client_flags': clientFlags,
      });
      return true;
    } catch (_) {
      return false;
    }
  }

  // PURPOSE: Drain the offline draft queue using the standard send endpoint. Caller
  //          should invoke this from a connectivity listener or background task.
  Future<int> flushDraftQueue() async {
    return draftQueue.flush((draft) async {
      try {
        await _api.post('/messages/send', data: {
          if (draft.to != null) 'to': draft.to,
          if (draft.cc != null) 'cc': draft.cc,
          if (draft.bcc != null) 'bcc': draft.bcc,
          if (draft.subject != null) 'subject': draft.subject,
          if (draft.body != null) 'body': draft.body,
        });
        return true;
      } catch (_) {
        return false;
      }
    });
  }

  // PURPOSE: Single decision point for background workers (WorkManager / BGTaskScheduler)
  //          asking "should I sync now?". Combines user prefs with network state.
  bool shouldSyncNow({required bool onWifi}) {
    return preferences.canSyncOnNetwork(onWifi: onWifi);
  }

  // ---------------------------------------------------------------------------
  // Local-cache integration (TMAIL-151)
  // ---------------------------------------------------------------------------

  // PURPOSE: Apply a server delta to the local SQLite cache. `new_message`
  //          inserts a summary, `flag_change` toggles is_read / is_flagged in
  //          place, `deletion` drops the row. No-op if the DB was never wired.
  Future<int> applyDelta(SyncDelta delta) async {
    final db = localDb;
    if (db == null) return 0;
    int applied = 0;
    for (final change in delta.changes) {
      switch (change.changeType) {
        case 'new_message':
          final data = change.data ?? const <String, dynamic>{};
          await db.upsertMessages([
            MobileMessageSummary(
              uid: change.uid,
              folder: change.folder,
              from: data['from'] as String?,
              subject: data['subject'] as String?,
              date: data['date'] as String?,
              isRead: data['is_read'] as bool? ?? false,
              isFlagged: data['is_flagged'] as bool? ?? false,
              hasAttachment: data['has_attachment'] as bool? ?? false,
            ),
          ]);
          applied += 1;
          break;
        case 'flag_change':
          final data = change.data ?? const <String, dynamic>{};
          await db.updateCachedMessageFlags(
            change.folder,
            change.uid,
            isRead: data['is_read'] as bool?,
            isFlagged: data['is_flagged'] as bool?,
          );
          applied += 1;
          break;
        case 'deletion':
          final removed =
              await db.deleteCachedMessage(change.folder, change.uid);
          applied += removed;
          break;
        default:
          // PURPOSE: Unknown change type — skip rather than crash so older
          //          mobile builds keep working when the backend adds new
          //          change types in the future.
          break;
      }
    }
    return applied;
  }

  // PURPOSE: Drain the outbound action queue after reconnect. Returns the
  //          number of actions successfully replayed (zero if the queue
  //          wasn't wired or there was nothing pending).
  Future<int> flushPendingActions() async {
    final queue = outboundQueue;
    if (queue == null) return 0;
    return queue.drain();
  }

  // PURPOSE: Evict cached messages older than the user's retention window.
  Future<int> evictExpiredCache({DateTime? now}) async {
    final db = localDb;
    if (db == null) return 0;
    return db.evictMessagesOlderThan(preferences.retentionCutoff(now: now));
  }
}
