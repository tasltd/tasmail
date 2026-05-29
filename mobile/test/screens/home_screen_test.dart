// Added: Widget tests for HomeScreen bottom navigation bar (TMAIL-149)
// PURPOSE: Verify the home shell exposes the 4 destinations required by the
//          spec (Inbox, Search, Calendar, Settings), shows the Compose FAB
//          across tabs, surfaces the Inbox unread badge from MailProvider, and
//          actually kicks off MailProvider.loadUnreadCount() on mount so the
//          badge populates from /api/mobile/unread-count.
// EXTERNAL: Uses Fake providers (FakeMailProvider, FakeAuthProvider) to keep
//          the test hermetic — no dio, no FlutterSecureStorage, no HTTP.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:provider/provider.dart';
import 'package:tasmail_mobile/providers/auth_provider.dart';
import 'package:tasmail_mobile/providers/mail_provider.dart';
import 'package:tasmail_mobile/screens/home_screen.dart';

/// Records MailProvider calls fired from HomeScreen.initState and exposes a
/// controllable unread count for badge assertions.
class FakeMailProvider extends MailProvider {
  FakeMailProvider({int unread = 0}) : _unread = unread;

  int _unread;
  int loadFoldersCalls = 0;
  int loadUnreadCountCalls = 0;

  // PURPOSE: Allow tests to push a new unread count and trigger the
  //          NavigationBar rebuild that re-evaluates the badge label.
  void setUnread(int value) {
    _unread = value;
    notifyListeners();
  }

  @override
  int get totalUnreadCount => _unread;

  @override
  Future<void> loadFolders() async {
    loadFoldersCalls += 1;
  }

  @override
  Future<void> loadUnreadCount() async {
    loadUnreadCountCalls += 1;
  }

  // NOTE: InboxScreen schedules loadInbox(refresh:true) + loadUnreadCount in
  // its own post-frame callback. We swallow them so the test doesn't try to
  // reach the real ApiClient.
  @override
  Future<void> loadInbox({bool refresh = false}) async {}
}

/// AuthProvider that never touches FlutterSecureStorage or the network. Returns
/// a null user; SettingsScreen and FolderDrawer are null-safe on auth.user.
class FakeAuthProvider extends AuthProvider {
  @override
  Future<void> logout() async {}
}

Widget _host(FakeMailProvider mail, FakeAuthProvider auth) {
  return MultiProvider(
    providers: [
      ChangeNotifierProvider<MailProvider>.value(value: mail),
      ChangeNotifierProvider<AuthProvider>.value(value: auth),
    ],
    child: MaterialApp(
      // NoSplash dodges the ink_sparkle.frag shader that fails to load in
      // widget tests on Flutter 3.44+.
      theme: ThemeData(splashFactory: NoSplash.splashFactory),
      home: const HomeScreen(),
    ),
  );
}

/// Drains the post-frame callbacks fired from HomeScreen.initState (loadFolders
/// + loadUnreadCount) plus the InboxScreen child's own post-frame load so
/// individual assertions measure stable state.
Future<void> _settleInitialBuild(WidgetTester tester) async {
  await tester.pump(); // initial build
  await tester.pump(); // post-frame callbacks
  await tester.pump(const Duration(milliseconds: 100));
}

void main() {
  group('HomeScreen bottom navigation (TMAIL-149)', () {
    testWidgets('renders Inbox, Search, Calendar, Settings destinations',
        (tester) async {
      final mail = FakeMailProvider();
      final auth = FakeAuthProvider();
      await tester.pumpWidget(_host(mail, auth));
      await _settleInitialBuild(tester);

      final nav = find.byType(NavigationBar);
      expect(nav, findsOneWidget);

      // Spec order: Inbox, Search, Calendar, Settings. Compose lives on the FAB.
      expect(find.descendant(of: nav, matching: find.text('Inbox')),
          findsOneWidget);
      expect(find.descendant(of: nav, matching: find.text('Search')),
          findsOneWidget);
      expect(find.descendant(of: nav, matching: find.text('Calendar')),
          findsOneWidget);
      expect(find.descendant(of: nav, matching: find.text('Settings')),
          findsOneWidget);

      // The destination icons should match the labels above.
      expect(find.descendant(of: nav, matching: find.byIcon(Icons.inbox)),
          findsOneWidget);
      expect(find.descendant(of: nav, matching: find.byIcon(Icons.search)),
          findsOneWidget);
      expect(
          find.descendant(of: nav, matching: find.byIcon(Icons.calendar_month)),
          findsOneWidget);
      expect(find.descendant(of: nav, matching: find.byIcon(Icons.settings)),
          findsOneWidget);
    });

    testWidgets('Compose FAB is present and routes to /compose', (tester) async {
      final mail = FakeMailProvider();
      final auth = FakeAuthProvider();

      // PURPOSE: Capture the Navigator push so we can assert the FAB wires up
      //          the right route without needing the real ComposeScreen tree.
      final pushed = <String>[];

      await tester.pumpWidget(
        MultiProvider(
          providers: [
            ChangeNotifierProvider<MailProvider>.value(value: mail),
            ChangeNotifierProvider<AuthProvider>.value(value: auth),
          ],
          child: MaterialApp(
            theme: ThemeData(splashFactory: NoSplash.splashFactory),
            home: const HomeScreen(),
            onGenerateRoute: (settings) {
              pushed.add(settings.name ?? '');
              // Land on a no-op page so the test framework doesn't trip on a
              // null route.
              return MaterialPageRoute(
                builder: (_) => const Scaffold(body: SizedBox.shrink()),
              );
            },
          ),
        ),
      );
      await _settleInitialBuild(tester);

      final fab = find.byKey(const Key('compose_fab'));
      expect(fab, findsOneWidget);

      await tester.tap(fab);
      await tester.pumpAndSettle();

      expect(pushed, contains('/compose'));
    });
  });

  group('HomeScreen Inbox unread badge (TMAIL-149)', () {
    testWidgets('hides badge when unread count is 0', (tester) async {
      final mail = FakeMailProvider(unread: 0);
      final auth = FakeAuthProvider();
      await tester.pumpWidget(_host(mail, auth));
      await _settleInitialBuild(tester);

      // The Badge widget is always in the tree but isLabelVisible flips with
      // the unread count. Drill into the actual widget to assert visibility.
      final badge = tester.widget<Badge>(
        find.descendant(
          of: find.byType(NavigationBar),
          matching: find.byType(Badge),
        ),
      );
      expect(badge.isLabelVisible, isFalse);
    });

    testWidgets('shows unread count from MailProvider.totalUnreadCount',
        (tester) async {
      final mail = FakeMailProvider(unread: 7);
      final auth = FakeAuthProvider();
      await tester.pumpWidget(_host(mail, auth));
      await _settleInitialBuild(tester);

      final badge = tester.widget<Badge>(
        find.descendant(
          of: find.byType(NavigationBar),
          matching: find.byType(Badge),
        ),
      );
      expect(badge.isLabelVisible, isTrue);
      expect(
        find.descendant(of: find.byType(NavigationBar), matching: find.text('7')),
        findsOneWidget,
      );
    });

    testWidgets('badge updates when MailProvider pushes a new unread count',
        (tester) async {
      final mail = FakeMailProvider(unread: 0);
      final auth = FakeAuthProvider();
      await tester.pumpWidget(_host(mail, auth));
      await _settleInitialBuild(tester);

      mail.setUnread(3);
      await tester.pump();

      final badge = tester.widget<Badge>(
        find.descendant(
          of: find.byType(NavigationBar),
          matching: find.byType(Badge),
        ),
      );
      expect(badge.isLabelVisible, isTrue);
      expect(
        find.descendant(of: find.byType(NavigationBar), matching: find.text('3')),
        findsOneWidget,
      );
    });
  });

  group('HomeScreen initState wiring (TMAIL-149)', () {
    testWidgets(
        'kicks off loadFolders + loadUnreadCount on first frame',
        (tester) async {
      final mail = FakeMailProvider();
      final auth = FakeAuthProvider();
      await tester.pumpWidget(_host(mail, auth));
      await _settleInitialBuild(tester);

      // loadFolders feeds the FolderDrawer; loadUnreadCount feeds the badge.
      // Both must fire on home mount, otherwise the badge stays at 0 until
      // the user navigates somewhere that triggers a refresh.
      expect(mail.loadFoldersCalls, greaterThanOrEqualTo(1));
      expect(mail.loadUnreadCountCalls, greaterThanOrEqualTo(1));
    });
  });

  group('HomeScreen tab switching (TMAIL-149)', () {
    testWidgets('tapping Calendar destination switches the IndexedStack',
        (tester) async {
      final mail = FakeMailProvider();
      final auth = FakeAuthProvider();
      await tester.pumpWidget(_host(mail, auth));
      await _settleInitialBuild(tester);

      // Initial index is 0 (Inbox).
      var stack = tester.widget<IndexedStack>(find.byType(IndexedStack));
      expect(stack.index, 0);

      await tester.tap(
        find.descendant(
          of: find.byType(NavigationBar),
          matching: find.text('Calendar'),
        ),
      );
      await tester.pumpAndSettle();

      stack = tester.widget<IndexedStack>(find.byType(IndexedStack));
      // Calendar is the 3rd destination (index 2): Inbox=0, Search=1, Calendar=2,
      // Settings=3.
      expect(stack.index, 2);

      // The CalendarScreen placeholder's hero copy should now be visible.
      expect(find.text('Calendar coming soon'), findsOneWidget);
    });

    testWidgets('tapping Settings destination switches to index 3',
        (tester) async {
      final mail = FakeMailProvider();
      final auth = FakeAuthProvider();
      await tester.pumpWidget(_host(mail, auth));
      await _settleInitialBuild(tester);

      await tester.tap(
        find.descendant(
          of: find.byType(NavigationBar),
          matching: find.text('Settings'),
        ),
      );
      await tester.pumpAndSettle();

      final stack = tester.widget<IndexedStack>(find.byType(IndexedStack));
      expect(stack.index, 3);
    });
  });
}
