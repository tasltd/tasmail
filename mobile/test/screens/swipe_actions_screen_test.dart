// Added: Widget tests for SwipeActionsScreen (TMAIL-148)
// PURPOSE: Verify the settings screen renders one RadioListTile per supported
//          action for each direction, that tapping a radio updates the
//          underlying SwipeActionsService, and that the reset button restores
//          defaults.
// EXTERNAL: Uses an in-memory StubSwipeActionsService so no
//          flutter_secure_storage / platform channel is involved.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:tasmail_mobile/screens/settings/swipe_actions_screen.dart';
import 'package:tasmail_mobile/services/swipe_actions_service.dart';
import 'package:tasmail_mobile/services/swipe_preferences.dart';

class StubSwipeActionsService extends SwipeActionsService {
  SwipePreferences _seed;

  StubSwipeActionsService(this._seed);

  @override
  SwipePreferences get preferences => _seed;

  @override
  bool get isLoaded => true;

  @override
  Future<SwipePreferences> load() async => _seed;

  @override
  Future<SwipePreferences> save(SwipePreferences next) async {
    _seed = next;
    notifyListeners();
    return _seed;
  }
}

void main() {
  Widget createWidget(StubSwipeActionsService service) {
    return MaterialApp(
      theme: ThemeData(splashFactory: NoSplash.splashFactory),
      home: SwipeActionsScreen(service: service),
    );
  }

  group('SwipeActionsScreen layout', () {
    testWidgets('renders both pickers and all action options', (tester) async {
      final service = StubSwipeActionsService(const SwipePreferences());
      await tester.pumpWidget(createWidget(service));
      await tester.pumpAndSettle();

      expect(find.text('Swipe right'), findsOneWidget);
      expect(find.text('Swipe left'), findsOneWidget);

      // Each picker should expose the full SwipeAction palette — 5 actions
      // × 2 pickers = 10 RadioListTiles.
      expect(find.byType(RadioListTile<SwipeAction>), findsNWidgets(10));
    });

    testWidgets('reflects the currently saved preference', (tester) async {
      final service = StubSwipeActionsService(const SwipePreferences(
        rightAction: SwipeAction.markUnread,
        leftAction: SwipeAction.toggleFlag,
      ));
      await tester.pumpWidget(createWidget(service));
      await tester.pumpAndSettle();

      final markUnreadRight = tester.widget<RadioListTile<SwipeAction>>(
        find.byKey(const Key('swipe_right_mark_unread')),
      );
      final toggleFlagLeft = tester.widget<RadioListTile<SwipeAction>>(
        find.byKey(const Key('swipe_left_toggle_flag')),
      );
      expect(markUnreadRight.groupValue, SwipeAction.markUnread);
      expect(toggleFlagLeft.groupValue, SwipeAction.toggleFlag);
    });
  });

  group('SwipeActionsScreen mutations', () {
    testWidgets('tapping a right-side action persists via the service',
        (tester) async {
      final service = StubSwipeActionsService(const SwipePreferences());
      await tester.pumpWidget(createWidget(service));
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('swipe_right_mark_unread')));
      await tester.pumpAndSettle();

      expect(service.preferences.rightAction, SwipeAction.markUnread);
      expect(service.preferences.leftAction, SwipeAction.delete);
    });

    testWidgets('tapping a left-side action persists via the service',
        (tester) async {
      final service = StubSwipeActionsService(const SwipePreferences());
      await tester.pumpWidget(createWidget(service));
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('swipe_left_none')));
      await tester.pumpAndSettle();

      expect(service.preferences.leftAction, SwipeAction.none);
      expect(service.preferences.rightAction, SwipeAction.archive);
    });

    testWidgets('reset button restores defaults and shows snackbar',
        (tester) async {
      final service = StubSwipeActionsService(const SwipePreferences(
        rightAction: SwipeAction.none,
        leftAction: SwipeAction.toggleFlag,
      ));
      await tester.pumpWidget(createWidget(service));
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('swipe_actions_reset')));
      await tester.pumpAndSettle();

      expect(service.preferences.rightAction, SwipeAction.archive);
      expect(service.preferences.leftAction, SwipeAction.delete);
      expect(find.text('Swipe actions reset to defaults'), findsOneWidget);
    });
  });
}
