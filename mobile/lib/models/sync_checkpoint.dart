// Added: SyncCheckpoint model mirroring backend models/sync.rs::SyncCheckpoint (TMAIL-51)
// PURPOSE: Per-folder IMAP CONDSTORE state (last_uid, last_modseq, uidvalidity) used by
//          the mobile sync engine to compute deltas against the backend.
// EXTERNAL: Wire-compatible with GET /api/sync/checkpoints and /api/sync/checkpoint/{folder}.

class SyncCheckpoint {
  final String folderName;
  final String? deviceId;
  final int lastUid;
  final int lastModseq;
  final int uidvalidity;
  final DateTime? lastSyncedAt;

  const SyncCheckpoint({
    required this.folderName,
    this.deviceId,
    this.lastUid = 0,
    this.lastModseq = 0,
    this.uidvalidity = 0,
    this.lastSyncedAt,
  });

  // PURPOSE: First-run sentinel — caller should request full sync, not delta
  bool get needsFullSync => uidvalidity == 0 || lastSyncedAt == null;

  factory SyncCheckpoint.fromJson(Map<String, dynamic> json) {
    return SyncCheckpoint(
      folderName: json['folder_name'] as String,
      deviceId: json['device_id'] as String?,
      lastUid: (json['last_uid'] as num?)?.toInt() ?? 0,
      lastModseq: (json['last_modseq'] as num?)?.toInt() ?? 0,
      uidvalidity: (json['uidvalidity'] as num?)?.toInt() ?? 0,
      lastSyncedAt: json['last_synced_at'] != null
          ? DateTime.tryParse(json['last_synced_at'] as String)
          : null,
    );
  }

  Map<String, dynamic> toJson() => {
        'folder_name': folderName,
        'device_id': deviceId,
        'last_uid': lastUid,
        'last_modseq': lastModseq,
        'uidvalidity': uidvalidity,
        'last_synced_at': lastSyncedAt?.toIso8601String(),
      };

  // PURPOSE: Server treats a uidvalidity bump as "full re-sync required" per RFC 3501
  bool requiresResyncAfter(SyncCheckpoint server) {
    if (uidvalidity == 0) return true;
    return server.uidvalidity != uidvalidity;
  }
}
