// Added: Widget tests for InboxScreen swipe gestures (TMAIL-54)
// PURPOSE: Verify bidirectional swipe — left=delete, right=archive — calls
//          the matching MailProvider methods and renders the right snackbar.
// EXTERNAL: Uses a FakeMailProvider that overrides the network-touching
//          methods so the test is hermetic (no dio, no real HTTP).

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:provider/provider.dart';
import 'package:tasmail_mobile/models/email.dart';
import 'package:tasmail_mobile/providers/mail_provider.dart';
import 'package:tasmail_mobile/screens/inbox/inbox_screen.dart';

/// Records `deleteMessage` / `archiveMessage` / `moveMessage` calls so the
/// swipe tests can assert what the screen actually did, without hitting dio.
class FakeMailProvider extends MailProvider {
  final List<MobileMessageSummary> seeded;
  bool archiveResult = true;
  bool deleteResult = true;
  final List<(String, int, String)> moveCalls = [];
  final List<(String, int)> archiveCalls = [];
  final List<(String, int)> deleteCalls = [];

  FakeMailProvider({required this.seeded});

  // Override the inbox getter via a backing list. Because MailProvider
  // stores messages in a private field we can't touch directly, we expose
  // the list through `_messages` by re-emitting it via notifyListeners.
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
  Future<void> loadInbox({bool refresh = false}) async {
    // no-op: messages are seeded
  }

  @override
  Future<void> loadUnreadCount() async {
    // no-op
  }

  @override
  Future<void> loadFolders() async {
    // no-op
  }

  @override
  Future<void> markAsRead(String folder, int uid) async {
    // no-op
  }

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
}

void main() {
  // NOTE: NoSplash.splashFactory avoids loading the ink_sparkle.frag shader,
  //       which fails to decode in Flutter 3.44+ widget tests.
  Widget createTestWidget(FakeMailProvider provider) {
    return ChangeNotifierProvider<MailProvider>.value(
      value: provider,
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

  group('InboxScreen swipe gestures (TMAIL-54)', () {
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

      // Swipe the first tile from right to left -> endToStart -> delete.
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

      // Swipe the first tile from left to right -> startToEnd -> archive.
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
      // NOTE: failure path renders no Undo action and the tile must stay put
      expect(find.text('Undo'), findsNothing);
      expect(find.text('Hello'), findsOneWidget);
    });

    testWidgets('compose FAB is present on the inbox', (tester) async {
      final provider = FakeMailProvider(seeded: fixtureMessages);
      await tester.pumpWidget(createTestWidget(provider));
      await tester.pump();

      expect(find.byKey(const Key('compose_fab')), findsOneWidget);
    });
  });
}
