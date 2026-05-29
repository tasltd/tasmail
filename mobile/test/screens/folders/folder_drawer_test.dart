// Added: Widget tests for FolderDrawer (TMAIL-146)
// PURPOSE: Verify the side drawer renders the folder tree from MailProvider,
//          shows per-folder unread badges, maps the five special folder names
//          (Inbox, Sent, Drafts, Trash, Spam) to the right Material icons,
//          highlights the selected folder, and routes folder taps through
//          MailProvider.selectFolder and Sign Out through AuthProvider.logout.
// EXTERNAL: Uses Fake providers so the test is hermetic — no dio, no
//          FlutterSecureStorage, no real HTTP.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:provider/provider.dart';
import 'package:tasmail_mobile/models/auth.dart';
import 'package:tasmail_mobile/models/email.dart';
import 'package:tasmail_mobile/providers/auth_provider.dart';
import 'package:tasmail_mobile/providers/mail_provider.dart';
import 'package:tasmail_mobile/screens/folders/folder_drawer.dart';

/// Records selectFolder calls and exposes a controllable folder list so the
/// drawer tests can assert what the user's tap actually did, without hitting
/// the real /api/mobile/folders endpoint or pulling in dio.
class FakeMailProvider extends MailProvider {
  FakeMailProvider({required List<MobileFolderSummary> seeded, String? selected})
      : _seededFolders = List.of(seeded),
        _selected = selected ?? 'INBOX';

  final List<MobileFolderSummary> _seededFolders;
  String _selected;
  final List<String> selectFolderCalls = [];

  @override
  List<MobileFolderSummary> get folders => _seededFolders;

  @override
  bool get isLoadingFolders => false;

  @override
  String get selectedFolder => _selected;

  @override
  Future<void> selectFolder(String folder) async {
    selectFolderCalls.add(folder);
    _selected = folder;
    notifyListeners();
  }

  @override
  Future<void> loadFolders() async {
    // no-op: folders are seeded
  }

  @override
  Future<void> loadInbox({bool refresh = false}) async {
    // no-op: drawer doesn't load messages directly
  }
}

/// Records logout calls and serves a fixed user so the header renders without
/// touching FlutterSecureStorage or the real API client.
class FakeAuthProvider extends AuthProvider {
  FakeAuthProvider({UserInfo? seededUser}) : _seededUser = seededUser;

  final UserInfo? _seededUser;
  int logoutCalls = 0;

  @override
  UserInfo? get user => _seededUser;

  @override
  Future<void> logout() async {
    logoutCalls += 1;
    notifyListeners();
  }
}

void main() {
  // NOTE: NoSplash.splashFactory avoids loading the ink_sparkle.frag shader,
  //       which fails to decode in Flutter 3.44+ widget tests.
  Widget createTestWidget({
    required MailProvider mail,
    required FakeAuthProvider auth,
  }) {
    return MultiProvider(
      providers: [
        ChangeNotifierProvider<MailProvider>.value(value: mail),
        ChangeNotifierProvider<AuthProvider>.value(value: auth),
      ],
      child: MaterialApp(
        theme: ThemeData(splashFactory: NoSplash.splashFactory),
        // Drawer must live inside a Scaffold to be openable in tests.
        home: Builder(
          builder: (context) => Scaffold(
            drawer: const FolderDrawer(),
            body: Builder(
              builder: (innerContext) => Center(
                child: ElevatedButton(
                  key: const Key('open_drawer_btn'),
                  onPressed: () => Scaffold.of(innerContext).openDrawer(),
                  child: const Text('Open'),
                ),
              ),
            ),
          ),
        ),
        // Settings is referenced by Navigator.pushNamed in the drawer; provide
        // a stub route so the tap doesn't blow up on missing route.
        routes: {
          '/settings': (_) => const Scaffold(body: Text('SETTINGS_STUB')),
        },
      ),
    );
  }

  // TMAIL-146 fixture: the five special folders the issue explicitly calls out.
  // Sized to fit the default 800x600 test viewport once the
  // UserAccountsDrawerHeader (~160dp) and footer (Divider + 2 ListTiles +
  // padding ~140dp) are subtracted.
  const specialFolders = [
    MobileFolderSummary(name: 'INBOX', unreadCount: 5, totalCount: 42),
    MobileFolderSummary(name: 'Sent', unreadCount: 0, totalCount: 18),
    MobileFolderSummary(name: 'Drafts', unreadCount: 2, totalCount: 2),
    MobileFolderSummary(name: 'Trash', unreadCount: 0, totalCount: 99),
    MobileFolderSummary(name: 'Spam', unreadCount: 3, totalCount: 7),
  ];

  Future<void> openDrawer(WidgetTester tester) async {
    await tester.tap(find.byKey(const Key('open_drawer_btn')));
    await tester.pumpAndSettle();
  }

  group('FolderDrawer (TMAIL-146)', () {
    testWidgets('renders user header with display name and email',
        (tester) async {
      final mail = FakeMailProvider(seeded: specialFolders);
      final auth = FakeAuthProvider(
        seededUser: const UserInfo(
          id: 'u1',
          email: 'alice@example.com',
          displayName: 'Alice Example',
        ),
      );

      await tester.pumpWidget(createTestWidget(mail: mail, auth: auth));
      await openDrawer(tester);

      expect(find.text('Alice Example'), findsOneWidget);
      expect(find.text('alice@example.com'), findsOneWidget);
      // Avatar initial is uppercased first character of email.
      expect(find.text('A'), findsOneWidget);
    });

    testWidgets('renders every seeded special folder by name', (tester) async {
      final mail = FakeMailProvider(seeded: specialFolders);
      final auth = FakeAuthProvider(
        seededUser: const UserInfo(id: 'u1', email: 'a@b.co'),
      );

      await tester.pumpWidget(createTestWidget(mail: mail, auth: auth));
      await openDrawer(tester);

      for (final folder in specialFolders) {
        expect(find.text(folder.name), findsOneWidget,
            reason: 'folder ${folder.name} should render');
      }
    });

    testWidgets('renders unread badge only on folders with unread > 0',
        (tester) async {
      final mail = FakeMailProvider(seeded: specialFolders);
      final auth = FakeAuthProvider(
        seededUser: const UserInfo(id: 'u1', email: 'a@b.co'),
      );

      await tester.pumpWidget(createTestWidget(mail: mail, auth: auth));
      await openDrawer(tester);

      // INBOX=5, Drafts=2, Spam=3  ->  3 badges. Sent and Trash have 0 and
      // must NOT render a badge.
      expect(find.byType(Badge), findsNWidgets(3));
      expect(find.text('5'), findsOneWidget);
      expect(find.text('2'), findsOneWidget);
      expect(find.text('3'), findsOneWidget);
      expect(find.text('0'), findsNothing);
    });

    testWidgets(
        'maps the five special folder names to the right Material icons',
        (tester) async {
      final mail = FakeMailProvider(seeded: specialFolders);
      final auth = FakeAuthProvider(
        seededUser: const UserInfo(id: 'u1', email: 'a@b.co'),
      );

      await tester.pumpWidget(createTestWidget(mail: mail, auth: auth));
      await openDrawer(tester);

      // Walk each special folder's ListTile and verify its leading Icon's
      // codePoint. We check codePoint rather than the IconData instance so the
      // test doesn't depend on whether find.byIcon matches outlined variants.
      const expected = <String, IconData>{
        'INBOX': Icons.inbox,
        'Sent': Icons.send,
        'Drafts': Icons.drafts,
        'Trash': Icons.delete,
        'Spam': Icons.report,
      };

      for (final entry in expected.entries) {
        final tile = tester.widget<ListTile>(
          find.ancestor(
            of: find.text(entry.key),
            matching: find.byType(ListTile),
          ),
        );
        final leading = tile.leading;
        expect(leading, isA<Icon>(),
            reason: 'leading of ${entry.key} should be an Icon');
        final icon = leading! as Icon;
        expect(icon.icon?.codePoint, entry.value.codePoint,
            reason: '${entry.key} should use ${entry.value} icon');
      }

      // Footer icons must also be present.
      expect(find.byIcon(Icons.settings), findsOneWidget);
      expect(find.byIcon(Icons.logout), findsOneWidget);
    });

    testWidgets('non-special folder falls back to the default folder icon',
        (tester) async {
      // Keep the list short so 'Projects' definitely renders within the
      // test viewport.
      const folders = [
        MobileFolderSummary(name: 'INBOX', unreadCount: 0, totalCount: 1),
        MobileFolderSummary(name: 'Projects', unreadCount: 0, totalCount: 1),
      ];
      final mail = FakeMailProvider(seeded: folders);
      final auth = FakeAuthProvider(
        seededUser: const UserInfo(id: 'u1', email: 'a@b.co'),
      );

      await tester.pumpWidget(createTestWidget(mail: mail, auth: auth));
      await openDrawer(tester);

      final tile = tester.widget<ListTile>(
        find.ancestor(
          of: find.text('Projects'),
          matching: find.byType(ListTile),
        ),
      );
      final leading = tile.leading;
      expect(leading, isA<Icon>());
      expect((leading! as Icon).icon?.codePoint, Icons.folder.codePoint);
    });

    testWidgets('highlights the currently selected folder', (tester) async {
      final mail = FakeMailProvider(seeded: specialFolders, selected: 'Drafts');
      final auth = FakeAuthProvider(
        seededUser: const UserInfo(id: 'u1', email: 'a@b.co'),
      );

      await tester.pumpWidget(createTestWidget(mail: mail, auth: auth));
      await openDrawer(tester);

      final draftsTile = tester.widget<ListTile>(
        find.ancestor(of: find.text('Drafts'), matching: find.byType(ListTile)),
      );
      expect(draftsTile.selected, isTrue);

      final inboxTile = tester.widget<ListTile>(
        find.ancestor(of: find.text('INBOX'), matching: find.byType(ListTile)),
      );
      expect(inboxTile.selected, isFalse);
    });

    testWidgets('tapping a folder calls selectFolder and closes the drawer',
        (tester) async {
      final mail = FakeMailProvider(seeded: specialFolders);
      final auth = FakeAuthProvider(
        seededUser: const UserInfo(id: 'u1', email: 'a@b.co'),
      );

      await tester.pumpWidget(createTestWidget(mail: mail, auth: auth));
      await openDrawer(tester);
      expect(find.text('Sent'), findsOneWidget);

      await tester.tap(find.text('Sent'));
      await tester.pumpAndSettle();

      expect(mail.selectFolderCalls, ['Sent']);
      // Drawer should have popped — Sent label no longer visible.
      expect(find.text('Sent'), findsNothing);
    });

    testWidgets('tapping Sign Out calls AuthProvider.logout', (tester) async {
      final mail = FakeMailProvider(seeded: specialFolders);
      final auth = FakeAuthProvider(
        seededUser: const UserInfo(id: 'u1', email: 'a@b.co'),
      );

      await tester.pumpWidget(createTestWidget(mail: mail, auth: auth));
      await openDrawer(tester);

      await tester.tap(find.text('Sign Out'));
      await tester.pumpAndSettle();

      expect(auth.logoutCalls, 1);
    });

    testWidgets(
        'shows progress indicator while folders are loading and list is empty',
        (tester) async {
      final mail = _LoadingFoldersProvider();
      final auth = FakeAuthProvider(
        seededUser: const UserInfo(id: 'u1', email: 'a@b.co'),
      );

      await tester.pumpWidget(createTestWidget(mail: mail, auth: auth));
      // NOTE: CircularProgressIndicator animates forever, so pumpAndSettle
      // would hang. Drive the drawer open with explicit pumps instead.
      await tester.tap(find.byKey(const Key('open_drawer_btn')));
      await tester.pump(); // start drawer open animation
      await tester.pump(const Duration(milliseconds: 400)); // finish slide-in

      expect(find.byType(CircularProgressIndicator), findsOneWidget);
    });

    testWidgets('falls back to email when display name is null', (tester) async {
      final mail = FakeMailProvider(seeded: specialFolders);
      final auth = FakeAuthProvider(
        seededUser: const UserInfo(id: 'u1', email: 'noreply@tasmail.io'),
      );

      await tester.pumpWidget(createTestWidget(mail: mail, auth: auth));
      await openDrawer(tester);

      // Email is rendered twice: once as the "account name" (since displayName
      // is null) and once as the "account email" — both come from the same
      // UserAccountsDrawerHeader fields.
      expect(find.text('noreply@tasmail.io'), findsNWidgets(2));
      // Avatar initial uppercased: 'N'
      expect(find.text('N'), findsOneWidget);
    });
  });
}

/// Provider variant that reports a loading state with no folders so the
/// drawer renders the CircularProgressIndicator branch.
class _LoadingFoldersProvider extends MailProvider {
  @override
  List<MobileFolderSummary> get folders => const [];

  @override
  bool get isLoadingFolders => true;

  @override
  String get selectedFolder => 'INBOX';

  @override
  Future<void> loadFolders() async {}

  @override
  Future<void> loadInbox({bool refresh = false}) async {}
}
