// Added: Offline sync service for TMAIL-151
// PURPOSE: Delta sync with backend, checkpoint management, and offline queue
// EXTERNAL: Uses /api/mobile/sync and /api/sync/checkpoint endpoints

import '../api/api_client.dart';

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
  final ApiClient _api = ApiClient();

  // PURPOSE: Fetch changes since a given timestamp
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

  // PURPOSE: Get sync checkpoint for a folder
  Future<String?> getCheckpoint(String folder) async {
    try {
      final response = await _api.get('/sync/checkpoint/$folder');
      return response.data['checkpoint'] as String?;
    } catch (_) {
      return null;
    }
  }

  // PURPOSE: Resolve a sync conflict
  Future<bool> resolveConflict({
    required String folder,
    required int uid,
    required String resolution, // 'local', 'remote', 'merge'
  }) async {
    try {
      await _api.post('/sync/resolve-conflict', data: {
        'folder': folder,
        'uid': uid,
        'resolution': resolution,
      });
      return true;
    } catch (_) {
      return false;
    }
  }
}
