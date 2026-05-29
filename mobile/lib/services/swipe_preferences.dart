// Added: User-configurable swipe actions for the inbox tile (TMAIL-148)
// PURPOSE: Encapsulate which action runs on a left vs right swipe so the settings
//          screen can rebind them without the InboxScreen knowing about storage.
// NOTE: Pure-Dart value type — persistence lives in SwipeActionsService so this
//       class stays trivially unit-testable (no storage, no platform channels).

// PURPOSE: Discrete actions a swipe can trigger. Keep this list small and only
//          include actions whose backend wiring is known-good — adding broken
//          options leaves the user with a setting that silently does nothing.
enum SwipeAction {
  // Do nothing — lets a user disable one direction entirely.
  none,
  // Move to the Archive folder (POST /folders/:f/messages/:u/move).
  archive,
  // Soft-delete via DELETE /folders/:f/messages/:u (server moves to Trash).
  delete,
  // Mark the message unread by clearing the IMAP \Seen flag.
  markUnread,
  // Toggle the IMAP \Flagged ("starred") flag.
  toggleFlag,
}

extension SwipeActionSerialization on SwipeAction {
  // PURPOSE: Stable on-disk key — DO NOT rename existing entries, only add new
  //          ones, otherwise stored prefs from older app versions will reset.
  String get wireName => switch (this) {
        SwipeAction.none => 'none',
        SwipeAction.archive => 'archive',
        SwipeAction.delete => 'delete',
        SwipeAction.markUnread => 'mark_unread',
        SwipeAction.toggleFlag => 'toggle_flag',
      };

  // PURPOSE: Short label shown in the settings picker. Localisable in a
  //          follow-up — kept in code for now to avoid a localisation diff.
  String get displayLabel => switch (this) {
        SwipeAction.none => 'None',
        SwipeAction.archive => 'Archive',
        SwipeAction.delete => 'Delete',
        SwipeAction.markUnread => 'Mark unread',
        SwipeAction.toggleFlag => 'Toggle star',
      };

  // PURPOSE: True for actions that remove the tile from the list so the swipe
  //          can complete the Dismissible animation. Non-destructive actions
  //          (markUnread, toggleFlag) must NOT dismiss the tile because the
  //          message still belongs in the current folder.
  bool get isDestructive => switch (this) {
        SwipeAction.archive => true,
        SwipeAction.delete => true,
        SwipeAction.none => false,
        SwipeAction.markUnread => false,
        SwipeAction.toggleFlag => false,
      };

  static SwipeAction fromWire(String? value) => switch (value) {
        'none' => SwipeAction.none,
        'archive' => SwipeAction.archive,
        'delete' => SwipeAction.delete,
        'mark_unread' => SwipeAction.markUnread,
        'toggle_flag' => SwipeAction.toggleFlag,
        _ => SwipeAction.none,
      };
}

class SwipePreferences {
  // PURPOSE: Defaults preserve the pre-TMAIL-148 behaviour — right=archive,
  //          left=delete — so users who never open the settings screen see no
  //          change after the upgrade.
  static const SwipeAction defaultRightAction = SwipeAction.archive;
  static const SwipeAction defaultLeftAction = SwipeAction.delete;

  // PURPOSE: Action triggered by swipe-right (startToEnd / leading-edge swipe).
  final SwipeAction rightAction;
  // PURPOSE: Action triggered by swipe-left (endToStart / trailing-edge swipe).
  final SwipeAction leftAction;

  const SwipePreferences({
    this.rightAction = defaultRightAction,
    this.leftAction = defaultLeftAction,
  });

  SwipePreferences copyWith({
    SwipeAction? rightAction,
    SwipeAction? leftAction,
  }) {
    return SwipePreferences(
      rightAction: rightAction ?? this.rightAction,
      leftAction: leftAction ?? this.leftAction,
    );
  }

  Map<String, dynamic> toJson() => {
        'right_action': rightAction.wireName,
        'left_action': leftAction.wireName,
      };

  factory SwipePreferences.fromJson(Map<String, dynamic> json) {
    return SwipePreferences(
      rightAction: SwipeActionSerialization.fromWire(
        json['right_action'] as String?,
      ),
      leftAction: SwipeActionSerialization.fromWire(
        json['left_action'] as String?,
      ),
    );
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is SwipePreferences &&
          other.rightAction == rightAction &&
          other.leftAction == leftAction;

  @override
  int get hashCode => Object.hash(rightAction, leftAction);
}
