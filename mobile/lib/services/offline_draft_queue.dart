// Added: Offline draft composition queue for TMAIL-51 offline-first protocol
// PURPOSE: Hold composed drafts when the device is offline (or on cellular while sync
//          mode is wifi-only) and replay them in FIFO order once the network/policy allows.
// NOTE: In-memory store with optional JSON serialization hook so callers can persist to
//       any storage backend (SharedPreferences, secure storage, sqflite) without forcing
//       a heavy native dependency on the mobile app today.

import 'dart:async';

class OfflineDraft {
  final String id;
  final String? to;
  final String? cc;
  final String? bcc;
  final String? subject;
  final String? body;
  final List<String> attachmentPaths;
  final DateTime createdAt;
  int attempts;
  String? lastError;

  OfflineDraft({
    required this.id,
    this.to,
    this.cc,
    this.bcc,
    this.subject,
    this.body,
    List<String>? attachmentPaths,
    DateTime? createdAt,
    this.attempts = 0,
    this.lastError,
  })  : attachmentPaths = attachmentPaths ?? const [],
        createdAt = createdAt ?? DateTime.now();

  Map<String, dynamic> toJson() => {
        'id': id,
        'to': to,
        'cc': cc,
        'bcc': bcc,
        'subject': subject,
        'body': body,
        'attachment_paths': attachmentPaths,
        'created_at': createdAt.toIso8601String(),
        'attempts': attempts,
        'last_error': lastError,
      };

  factory OfflineDraft.fromJson(Map<String, dynamic> json) {
    return OfflineDraft(
      id: json['id'] as String,
      to: json['to'] as String?,
      cc: json['cc'] as String?,
      bcc: json['bcc'] as String?,
      subject: json['subject'] as String?,
      body: json['body'] as String?,
      attachmentPaths: (json['attachment_paths'] as List<dynamic>? ?? const [])
          .map((e) => e as String)
          .toList(),
      createdAt:
          DateTime.tryParse(json['created_at'] as String? ?? '') ?? DateTime.now(),
      attempts: (json['attempts'] as num?)?.toInt() ?? 0,
      lastError: json['last_error'] as String?,
    );
  }
}

// PURPOSE: Drop drafts that have failed too many times so we don't loop forever.
const int kMaxOfflineDraftAttempts = 5;

typedef DraftSender = Future<bool> Function(OfflineDraft draft);

class OfflineDraftQueue {
  final List<OfflineDraft> _queue = [];
  final void Function(List<Map<String, dynamic>> snapshot)? onPersist;

  OfflineDraftQueue({this.onPersist});

  int get length => _queue.length;
  bool get isEmpty => _queue.isEmpty;
  List<OfflineDraft> get pending => List.unmodifiable(_queue);

  // PURPOSE: Enqueue a freshly composed draft. Persists via [onPersist] if supplied.
  void enqueue(OfflineDraft draft) {
    if (_queue.any((d) => d.id == draft.id)) return;
    _queue.add(draft);
    _persist();
  }

  // PURPOSE: Remove a draft by id (e.g., user cancelled it from the queue UI).
  bool remove(String id) {
    final before = _queue.length;
    _queue.removeWhere((d) => d.id == id);
    final removed = _queue.length != before;
    if (removed) _persist();
    return removed;
  }

  // PURPOSE: Rehydrate the queue from persisted JSON (e.g., on app launch).
  void loadFromJson(List<Map<String, dynamic>> snapshot) {
    _queue
      ..clear()
      ..addAll(snapshot.map(OfflineDraft.fromJson));
  }

  // PURPOSE: Flush queued drafts using the provided send callback. Returns the number
  //          of drafts successfully sent. Drafts that fail are retried up to
  //          [kMaxOfflineDraftAttempts] before being dropped with [lastError].
  Future<int> flush(DraftSender send) async {
    if (_queue.isEmpty) return 0;
    int sent = 0;
    final survivors = <OfflineDraft>[];

    for (final draft in List<OfflineDraft>.from(_queue)) {
      try {
        final ok = await send(draft);
        if (ok) {
          sent += 1;
          continue;
        }
        draft.attempts += 1;
        if (draft.attempts < kMaxOfflineDraftAttempts) {
          survivors.add(draft);
        } else {
          draft.lastError ??= 'max_attempts_exceeded';
        }
      } catch (e) {
        draft.attempts += 1;
        draft.lastError = e.toString();
        if (draft.attempts < kMaxOfflineDraftAttempts) survivors.add(draft);
      }
    }

    _queue
      ..clear()
      ..addAll(survivors);
    _persist();
    return sent;
  }

  void _persist() {
    if (onPersist == null) return;
    onPersist!(_queue.map((d) => d.toJson()).toList());
  }
}
