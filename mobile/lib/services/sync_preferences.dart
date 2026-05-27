// Added: User-tunable sync settings for offline-first behavior (TMAIL-51)
// PURPOSE: Encapsulate sync mode (auto / wifi-only / manual / disabled), local retention
//          days, and an optional max body size so background sync stays bandwidth-friendly.
// NOTE: Pure-Dart value type so it can be persisted via any backend (SharedPreferences,
//       secure storage, sqlite) without coupling to a specific package.

// PURPOSE: When background sync is allowed to run
enum SyncMode {
  // Sync over any network — fastest convergence, highest data cost.
  auto,
  // Sync only when on WiFi/unmetered — default per spec to protect cellular plans.
  wifiOnly,
  // Sync only when user explicitly pulls — strictest data control.
  manual,
  // No background sync at all — read-cached-only.
  disabled,
}

extension SyncModeSerialization on SyncMode {
  String get wireName => switch (this) {
        SyncMode.auto => 'auto',
        SyncMode.wifiOnly => 'wifi_only',
        SyncMode.manual => 'manual',
        SyncMode.disabled => 'disabled',
      };

  static SyncMode fromWire(String? value) => switch (value) {
        'auto' => SyncMode.auto,
        'wifi_only' => SyncMode.wifiOnly,
        'manual' => SyncMode.manual,
        'disabled' => SyncMode.disabled,
        _ => SyncMode.wifiOnly,
      };
}

class SyncPreferences {
  // PURPOSE: Default to wifi-only to honour Ghana market cellular costs (BYOK positioning).
  static const SyncMode defaultMode = SyncMode.wifiOnly;
  // PURPOSE: 30-day rolling cache per spec; tweakable from settings UI.
  static const int defaultRetentionDays = 30;
  // PURPOSE: Skip body download for messages over this size unless user opens them.
  static const int defaultMaxAutoBodyKb = 256;

  final SyncMode mode;
  final int retentionDays;
  final int maxAutoBodyKb;

  const SyncPreferences({
    this.mode = defaultMode,
    this.retentionDays = defaultRetentionDays,
    this.maxAutoBodyKb = defaultMaxAutoBodyKb,
  });

  // PURPOSE: Whether the sync engine may start a background sync given the current
  //          network type. `onWifi=true` means the device is on an unmetered connection.
  bool canSyncOnNetwork({required bool onWifi}) {
    return switch (mode) {
      SyncMode.disabled => false,
      SyncMode.manual => false,
      SyncMode.wifiOnly => onWifi,
      SyncMode.auto => true,
    };
  }

  // PURPOSE: Compute eviction cutoff for the local cache.
  DateTime retentionCutoff({DateTime? now}) {
    final base = now ?? DateTime.now();
    return base.subtract(Duration(days: retentionDays));
  }

  SyncPreferences copyWith({
    SyncMode? mode,
    int? retentionDays,
    int? maxAutoBodyKb,
  }) {
    return SyncPreferences(
      mode: mode ?? this.mode,
      retentionDays: retentionDays ?? this.retentionDays,
      maxAutoBodyKb: maxAutoBodyKb ?? this.maxAutoBodyKb,
    );
  }

  Map<String, dynamic> toJson() => {
        'mode': mode.wireName,
        'retention_days': retentionDays,
        'max_auto_body_kb': maxAutoBodyKb,
      };

  factory SyncPreferences.fromJson(Map<String, dynamic> json) {
    return SyncPreferences(
      mode: SyncModeSerialization.fromWire(json['mode'] as String?),
      retentionDays:
          (json['retention_days'] as num?)?.toInt() ?? defaultRetentionDays,
      maxAutoBodyKb:
          (json['max_auto_body_kb'] as num?)?.toInt() ?? defaultMaxAutoBodyKb,
    );
  }
}
