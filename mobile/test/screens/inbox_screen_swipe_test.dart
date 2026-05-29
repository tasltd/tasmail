// Added: Widget tests for InboxScreen swipe gestures (TMAIL-54)
// Changed: TMAIL-148 — extended to cover configurable swipe actions sourced
//          from SwipeActionsService (archive / delete / mark-unread / star).
// PURPOSE: Verify bidirectional swipe — left=delete, right=archive — calls the
//          matching MailProvider methods and renders the right snackbar, and
//          that picking a non-default action (markUnread, toggleFlag) routes
//          through the correct provider method without dismissing the tile.
// EXTERNAL: Uses a FakeMailProvider that overrides the network-touching
//          methods so the test is hermetic (no dio, no real HTTP).

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:provider/provider.dart';
import 'package:tasmail_mobile/models/email.dart';
import 'package:tasmail_mobile/providers/mail_provider.dart';
import 'package:tasmail_mobile/screens/inbox/inbox_screen.dart';
import 'package:tasmail_mobile/services/swipe_actions_service.dart';
import 'package:tasmail_mobile/services/swipe_preferences.dart';

/// Records `deleteMessage` / `archiveMessage` / `moveMessage` calls so the
/// swipe tests can assert what the screen actually did, without hitting dio.
class FakeMailProvider extends MailProvider {
  final List<MobileMessageSummary> seeded;
  bool archiveResult = true;
  bool deleteResult = true;
  bool markUnreadResult = true;
  bool setFlaggedResult = true;
  final List<(String, int, String)> moveCalls = [];
  final List<(String, int)> archiveCalls = [];
  final List<(String, int)> deleteCalls = [];
  final List<(String, int)> markUnreadCalls = [];
  final List<(String, int, bool)> setFlaggedCalls = [];

  FakeMailProvider({required this.seeded});

  @override
  List<MobileMessageSummary> get messages => _seededView;

  late final List<MobileMessageSummary> _seededView = List.of(seeded);

  @override
  bool get isLoadingInbox => false;
  @override
  String? get inboxError => null;
  @override
  bool get hasMore => false;
  @override
  int get totalUnreadCount => 0;
  @override
  String get selectedFolder => 'INBOX';

  @override
  Future<void> loadInbox({bool refresh = false}) async {}

  @override
  Future<void> loadUnreadCount() async {}

  @override
  Future<void> loadFolders() async {}

  @override
  Future<void> markAsRead(String folder, int uid) async {}

  @override
  Future<bool> deleteMessage(String folder, int uid) async {
    deleteCalls.add((folder, uid));
    if (deleteResult) {
      _seededView.removeWhere((m) => m.uid == uid && m.folder == folder);
      notifyListeners();
    }
    return deleteResult;
  }

  @override
  Future<bool> moveMessage(String folder, int uid, String toFolder) async {
    moveCalls.add((folder, uid, toFolder));
    if (archiveResult) {
      _seededView.removeWhere((m) => m.uid == uid && m.folder == folder);
      notifyListeners();
    }
    return archiveResult;
  }

  @override
  Future<bool> archiveMessage(String folder, int uid) async {
    archiveCalls.add((folder, uid));
    return moveMessage(folder, uid, 'Archive');
  }

  @override
  Future<bool> markAsUnread(String folder, int uid) async {
    markUnreadCalls.add((folder, uid));
    return markUnreadResult;
  }

  @override
  Future<bool> setFlagged(String folder, int uid, bool flagged) async {
    setFlaggedCalls.add((folder, uid, flagged));
    return setFlaggedResult;
  }
}

/// In-memory SwipeActionsService that returns whatever prefs the test seeds it
/// with — no flutter_secure_storage round-trip needed.
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
  Widget createTestWidget(
    FakeMailProvider provider, {
    StubSwipeActionsService? swipeService,
  }) {
    return MultiProvider(
      providers: [
        ChangeNotifierProvider<MailProvider>.value(value: provider),
        // PURPOSE: TMAIL-148 — the screen pulls swipe prefs from this service
        // when present; if absent, the InboxScreen falls back to the
        // hardcoded defaults so the original test cases still pass.
        if (swipeService != null)
          ChangeNotifierProvider<SwipeActionsService>.value(
            value: swipeService,
          ),
      ],
      child: MaterialApp(
        theme: ThemeData(splashFactory: NoSplash.splashFactory),
        home: const InboxScreen(),
      ),
    );
  }

  final fixtureMessages = [
    const MobileMessageSummary(
      uid: 101,
      folder: 'INBOX',
      from: 'alice@example.com',
      subject: 'Hello',
      date: '2026-05-28T10:00:00Z',
      isRead: false,
      isFlagged: false,
      hasAttachment: false,
    ),
    const MobileMessageSummary(
      uid: 102,
      folder: 'INBOX',
      from: 'bob@example.com',
      subject: 'World',
      date: '2026-05-28T10:01:00Z',
      isRead: true,
      isFlagged: false,
      hasAttachment: false,
    ),
  ];

  group('InboxScreen swipe gestures — defaults (TMAIL-54)', () {
    testWidgets('renders message tiles for seeded inbox', (tester) async {
      final provider = FakeMailProvider(seeded: fixtureMessages);
      await tester.pumpWidget(createTestWidget(provider));
      await tester.pump();

      expect(find.text('Hello'), findsOneWidget);
      expect(find.text('World'), findsOneWidget);
    });

    testWidgets('swipe-left calls deleteMessage and shows delete snackbar',
        (tester) async {
      final provider = FakeMailProvider(seeded: fixtureMessages);
      await tester.pumpWidget(createTestWidget(provider));
      await tester.pump();

      await tester.drag(find.text('Hello'), const Offset(-500, 0));
      await tester.pumpAndSettle();

      expect(provider.deleteCalls, [('INBOX', 101)]);
      expect(provider.archiveCalls, isEmpty);
      expect(find.text('Message deleted'), findsOneWidget);
      expect(find.text('Hello'), findsNothing);
    });

    testWidgets('swipe-right calls archiveMessage and shows archive snackbar',
        (tester) async {
      final provider = FakeMailProvider(seeded: fixtureMessages);
      await tester.pumpWidget(createTestWidget(provider));
      await tester.pump();

      await tester.drag(find.text('Hello'), const Offset(500, 0));
      await tester.pumpAndSettle();

      expect(provider.archiveCalls, [('INBOX', 101)]);
      expect(provider.moveCalls, [('INBOX', 101, 'Archive')]);
      expect(provider.deleteCalls, isEmpty);
      expect(find.text('Message archived'), findsOneWidget);
      expect(find.text('Hello'), findsNothing);
    });

    testWidgets('archive failure surfaces a non-undoable error snackbar',
        (tester) async {
      final provider = FakeMailProvider(seeded: fixtureMessages)
        ..archiveResult = false;
      await tester.pumpWidget(createTestWidget(provider));
      await tester.pump();

      await tester.drag(find.text('Hello'), const Offset(500, 0));
      await tester.pumpAndSettle();

      expect(provider.archiveCalls, [('INBOX', 101)]);
      expect(find.text('Archive failed'), findsOneWidget);
      expect(find.text('Undo'), findsNothing);
      expect(find.text('Hello'), findsOneWidget);
    });

    // Changed: TMAIL-149 — the Compose FAB moved from InboxScreen to
    //          HomeScreen so it's visible on every bottom-nav tab. When the
    //          InboxScreen is rendered standalone (as in this test), it
    //          intentionally has no FAB of its own. The home-shell FAB is
    //          covered by test/screens/home_screen_test.dart.
    testWidgets('inbox standalone no longer owns the compose FAB', (tester) async {
      final provider = FakeMailProvider(seeded: fixtureMessages);
      await tester.pumpWidget(createTestWidget(provider));
      await tester.pump();

      expect(find.byKey(const Key('compose_fab')), findsNothing);
    });
  });

  group('InboxScreen swipe gestures — configurable (TMAIL-148)', () {
    testWidgets('swipe-right honours markUnread when configured', (tester) async {
      final provider = FakeMailProvider(seeded: fixtureMessages);
      final swipe = StubSwipeActionsService(
        const SwipePreferences(
          rightAction: SwipeAction.markUnread,
          leftAction: SwipeAction.delete,
        ),
      );

      await tester.pumpWidget(
        createTestWidget(provider, swipeService: swipe),
      );
      await tester.pumpAndSettle();

      await tester.drag(find.text('Hello'), const Offset(500, 0));
      await tester.pumpAndSettle();

      expect(provider.markUnreadCalls, [('INBOX', 101)]);
      expect(provider.archiveCalls, isEmpty);
      expect(provider.deleteCalls, isEmpty);
      expect(find.text('Marked as unread'), findsOneWidget);
      // Non-destructive: tile must stay
      expect(find.text('Hello'), findsOneWidget);
    });

    testWidgets('swipe-left honours toggleFlag when configured', (tester) async {
      final provider = FakeMailProvider(seeded: fixtureMessages);
      final swipe = StubSwipeActionsService(
        const SwipePreferences(
          rightAction: SwipeAction.archive,
          leftAction: SwipeAction.toggleFlag,
        ),
      );

      await tester.pumpWidget(
        createTestWidget(provider, swipeService: swipe),
      );
      await tester.pumpAndSettle();

      await tester.drag(find.text('Hello'), const Offset(-500, 0));
      await tester.pumpAndSettle();

      expect(provider.setFlaggedCalls, [('INBOX', 101, true)]);
      expect(provider.deleteCalls, isEmpty);
      expect(find.text('Message starred'), findsOneWidget);
      expect(find.text('Hello'), findsOneWidget);
    });

    testWidgets('setting an action to none disables that swipe direction',
        (tester) async {
      final provider = FakeMailProvider(seeded: fixtureMessages);
      final swipe = StubSwipeActionsService(
        const SwipePreferences(
          rightAction: SwipeAction.none,
          leftAction: SwipeAction.delete,
        ),
      );

      await tester.pumpWidget(
        createTestWidget(provider, swipeService: swipe),
      );
      await tester.pumpAndSettle();

      // Try to swipe right (disabled) — should be a no-op.
      await tester.drag(find.text('Hello'), const Offset(500, 0));
      await tester.pumpAndSettle();

      expect(provider.archiveCalls, isEmpty);
      expect(provider.markUnreadCalls, isEmpty);
      expect(find.text('Hello'), findsOneWidget);

      // Swipe left (still wired to delete) — should fire.
      await tester.drag(find.text('Hello'), const Offset(-500, 0));
      await tester.pumpAndSettle();
      expect(provider.deleteCalls, [('INBOX', 101)]);
    });

    testWidgets('swipe-right delete works when configured for both sides',
        (tester) async {
      final provider = FakeMailProvider(seeded: fixtureMessages);
      final swipe = StubSwipeActionsService(
        const SwipePreferences(
          rightAction: SwipeAction.delete,
          leftAction: SwipeAction.delete,
        ),
      );

      await tester.pumpWidget(
        createTestWidget(provider, swipeService: swipe),
      );
      await tester.pumpAndSettle();

      await tester.drag(find.text('Hello'), const Offset(500, 0));
      await tester.pumpAndSettle();

      expect(provider.deleteCalls, [('INBOX', 101)]);
      expect(find.text('Message deleted'), findsOneWidget);
      expect(find.text('Hello'), findsNothing);
    });
  });
}
