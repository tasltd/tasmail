// Added: Persistent outbound action queue for TMAIL-151 offline-first mobile sync
// PURPOSE: Capture user actions taken while offline (flag, unflag, mark-read,
//          mark-unread, delete, move) and replay them against the backend when
//          connectivity returns. Backed by the cached_messages + pending_actions
//          tables in LocalDatabase so the queue survives app restarts.
// EXTERNAL: Replay calls hit the same endpoints the live MailProvider already uses:
//            PUT  /folders/{folder}/messages/{uid}/flag    {flagged:bool}
//            PUT  /folders/{folder}/messages/{uid}/read
//            DELETE /folders/{folder}/messages/{uid}
//            POST /folders/{folder}/messages/{uid}/move    {to_folder:string}
// NOTE: Replay strategy: at-most-N attempts per action. After kMaxAttempts the
//       action is dropped with last_error set so the UI can surface the failure.

import '../api/api_client.dart';
import 'local_database.dart';

const int kMaxOutboundActionAttempts = 5;

// PURPOSE: Callback the queue uses to push a single action over the wire.
//          Returns true on a 2xx response, false otherwise. Pulled out as a
//          typedef so unit tests can inject a fake without touching ApiClient.
typedef OutboundActionSender = Future<bool> Function(PendingAction action);

class OutboundActionQueue {
  final LocalDatabase _db;
  final OutboundActionSender _send;

  OutboundActionQueue({
    required LocalDatabase db,
    OutboundActionSender? send,
    ApiClient? api,
  })  : _db = db,
        _send = send ?? _defaultSender(api ?? ApiClient());

  // PURPOSE: Standard ApiClient-backed sender. Mirrors MailProvider's own
  //          REST calls so behaviour is identical online vs replayed offline.
  static OutboundActionSender _defaultSender(ApiClient api) {
    return (action) async {
      try {
        switch (action.actionType) {
          case OutboundActionType.flag:
            await api.put(
              '/folders/${action.folder}/messages/${action.uid}/flag',
              data: {'flagged': true},
            );
            return true;
          case OutboundActionType.unflag:
            await api.put(
              '/folders/${action.folder}/messages/${action.uid}/flag',
              data: {'flagged': false},
            );
            return true;
          case OutboundActionType.markRead:
            await api.put(
              '/folders/${action.folder}/messages/${action.uid}/read',
            );
            return true;
          case OutboundActionType.markUnread:
            await api.put(
              '/folders/${action.folder}/messages/${action.uid}/read',
              data: {'read': false},
            );
            return true;
          case OutboundActionType.delete:
            await api.delete(
              '/folders/${action.folder}/messages/${action.uid}',
            );
            return true;
          case OutboundActionType.move:
            final toFolder = action.payload['to_folder'] as String?;
            if (toFolder == null) return false;
            await api.post(
              '/folders/${action.folder}/messages/${action.uid}/move',
              data: {'to_folder': toFolder},
            );
            return true;
          default:
            return false;
        }
      } catch (_) {
        return false;
      }
    };
  }

  // ---------------------------------------------------------------------------
  // Enqueue helpers — call these from MailProvider / swipe gestures when offline.
  // ---------------------------------------------------------------------------

  Future<int> enqueueFlag(String folder, int uid, {required bool flagged}) {
    return _db.enqueueAction(
      actionType: flagged ? OutboundActionType.flag : OutboundActionType.unflag,
      folder: folder,
      uid: uid,
    );
  }

  Future<int> enqueueRead(String folder, int uid, {required bool read}) {
    return _db.enqueueAction(
      actionType:
          read ? OutboundActionType.markRead : OutboundActionType.markUnread,
      folder: folder,
      uid: uid,
    );
  }

  Future<int> enqueueDelete(String folder, int uid) {
    return _db.enqueueAction(
      actionType: OutboundActionType.delete,
      folder: folder,
      uid: uid,
    );
  }

  Future<int> enqueueMove(String folder, int uid, String toFolder) {
    return _db.enqueueAction(
      actionType: OutboundActionType.move,
      folder: folder,
      uid: uid,
      payload: {'to_folder': toFolder},
    );
  }

  // ---------------------------------------------------------------------------
  // Replay
  // ---------------------------------------------------------------------------

  Future<int> pendingCount() => _db.pendingActionCount();

  // PURPOSE: Drain the queue in FIFO order. Returns the number of actions that
  //          successfully replayed. Caller (e.g. WorkManager / connectivity
  //          listener) decides when to invoke this — see SyncService.flushPending.
  Future<int> drain() async {
    final actions = await _db.pendingActions();
    if (actions.isEmpty) return 0;

    int sent = 0;
    for (final action in actions) {
      final ok = await _send(action);
      if (ok) {
        await _db.deleteAction(action.id);
        sent += 1;
        continue;
      }
      final nextAttempt = action.attempts + 1;
      if (nextAttempt >= kMaxOutboundActionAttempts) {
        // PURPOSE: Drop poison actions so we don't loop forever. UI can poll the
        //          last_error column to expose this to the user.
        await _db.deleteAction(action.id);
      } else {
        await _db.recordActionFailure(action.id, 'replay_failed');
      }
    }
    return sent;
  }
}
