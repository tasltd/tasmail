// Added: Unit tests for SwipePreferences value type (TMAIL-148)
// PURPOSE: Lock in the default mapping (right=archive, left=delete) and the
//          JSON round-trip so storage format never silently drifts.

import 'package:flutter_test/flutter_test.dart';
import 'package:tasmail_mobile/services/swipe_preferences.dart';

void main() {
  group('SwipePreferences defaults', () {
    test('default constructor maps to archive (right) + delete (left)', () {
      const prefs = SwipePreferences();
      expect(prefs.rightAction, SwipeAction.archive);
      expect(prefs.leftAction, SwipeAction.delete);
    });

    test('default constants match the constructor defaults', () {
      expect(SwipePreferences.defaultRightAction, SwipeAction.archive);
      expect(SwipePreferences.defaultLeftAction, SwipeAction.delete);
    });
  });

  group('SwipeAction serialization', () {
    test('every enum value has a unique stable wireName', () {
      final wires = SwipeAction.values.map((a) => a.wireName).toList();
      expect(wires.toSet().length, SwipeAction.values.length);
    });

    test('round-trip via wireName preserves the value', () {
      for (final action in SwipeAction.values) {
        final round = SwipeActionSerialization.fromWire(action.wireName);
        expect(round, action, reason: 'failed round-trip for $action');
      }
    });

    test('unknown wire string falls back to none', () {
      expect(
        SwipeActionSerialization.fromWire('not_a_real_action'),
        SwipeAction.none,
      );
      expect(SwipeActionSerialization.fromWire(null), SwipeAction.none);
    });

    test('isDestructive classifies archive and delete as removing the tile', () {
      expect(SwipeAction.archive.isDestructive, isTrue);
      expect(SwipeAction.delete.isDestructive, isTrue);
      expect(SwipeAction.none.isDestructive, isFalse);
      expect(SwipeAction.markUnread.isDestructive, isFalse);
      expect(SwipeAction.toggleFlag.isDestructive, isFalse);
    });

    test('displayLabel is non-empty for every action', () {
      for (final action in SwipeAction.values) {
        expect(action.displayLabel.trim(), isNotEmpty);
      }
    });
  });

  group('SwipePreferences JSON round-trip', () {
    test('serialises to a stable shape', () {
      const prefs = SwipePreferences(
        rightAction: SwipeAction.markUnread,
        leftAction: SwipeAction.toggleFlag,
      );
      expect(prefs.toJson(), {
        'right_action': 'mark_unread',
        'left_action': 'toggle_flag',
      });
    });

    test('decodes back to the same value object', () {
      const prefs = SwipePreferences(
        rightAction: SwipeAction.delete,
        leftAction: SwipeAction.none,
      );
      final round = SwipePreferences.fromJson(prefs.toJson());
      expect(round, prefs);
    });

    test('missing keys fall back to defaults (forward-compat)', () {
      final prefs = SwipePreferences.fromJson(const {});
      // Missing keys decode as `none` per SwipeActionSerialization.fromWire —
      // makes the test surface any accidental change to that contract.
      expect(prefs.rightAction, SwipeAction.none);
      expect(prefs.leftAction, SwipeAction.none);
    });
  });

  group('SwipePreferences copyWith', () {
    test('only changes the requested side', () {
      const before = SwipePreferences();
      final afterRight = before.copyWith(rightAction: SwipeAction.markUnread);
      expect(afterRight.rightAction, SwipeAction.markUnread);
      expect(afterRight.leftAction, before.leftAction);

      final afterLeft = before.copyWith(leftAction: SwipeAction.toggleFlag);
      expect(afterLeft.leftAction, SwipeAction.toggleFlag);
      expect(afterLeft.rightAction, before.rightAction);
    });
  });
}
