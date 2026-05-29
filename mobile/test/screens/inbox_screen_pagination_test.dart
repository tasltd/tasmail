// Added: Widget tests for InboxScreen pull-to-refresh + infinite scroll (TMAIL-143)
// PURPOSE: Verify that the RefreshIndicator triggers loadInbox(refresh: true)
//          and that scrolling near the end of the list triggers loadMore().
// EXTERNAL: Uses a FakeMailProvider that overrides the network-touching methods
//          so the test is hermetic (no dio, no real HTTP).
//
// NOTE: When `hasMore` is true the inbox renders a trailing CircularProgressIndicator
// which animates forever. We deliberately use sequenced `pump(duration)` calls
// instead of `pumpAndSettle()` in those scenarios — pumpAndSettle would hang
// waiting for the spinner to stop, and the spinner is by design perpetual until
// the next page arrives.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:provider/provider.dart';
import 'package:tasmail_mobile/models/email.dart';
import 'package:tasmail_mobile/providers/mail_provider.dart';
import 'package:tasmail_mobile/screens/inbox/inbox_screen.dart';

/// Records pagination + refresh calls so the inbox tests can assert what the
/// screen actually did, without hitting dio.
class FakeMailProvider extends MailProvider {
  FakeMailProvider({
    required List<MobileMessageSummary> seeded,
    bool hasMore = false,
  })  : _seededView = List.of(seeded),
        _hasMore = hasMore;

  final List<MobileMessageSummary> _seededView;
  final bool _hasMore;
  int refreshCalls = 0;
  int loadMoreCalls = 0;
  int unreadCountCalls = 0;

  @override
  List<MobileMessageSummary> get messages => _seededView;
  @override
  bool get isLoadingInbox => false;
  @override
  String? get inboxError => null;
  @override
  bool get hasMore => _hasMore;
  @override
  int get totalUnreadCount => 0;
  @override
  String get selectedFolder => 'INBOX';

  @override
  Future<void> loadInbox({bool refresh = false}) async {
    if (refresh) refreshCalls++;
  }

  @override
  Future<void> loadMore() async {
    loadMoreCalls++;
  }

  @override
  Future<void> loadUnreadCount() async {
    unreadCountCalls++;
  }

  @override
  Future<void> loadFolders() async {
    // no-op
  }
}

MobileMessageSummary _summary(int uid) => MobileMessageSummary(
      uid: uid,
      folder: 'INBOX',
      from: 'sender$uid@example.com',
      subject: 'Subject $uid',
      date: '2026-05-28T10:00:00Z',
      isRead: false,
      isFlagged: false,
      hasAttachment: false,
    );

Widget _host(FakeMailProvider provider) {
  return ChangeNotifierProvider<MailProvider>.value(
    value: provider,
    child: MaterialApp(
      theme: ThemeData(splashFactory: NoSplash.splashFactory),
      home: const InboxScreen(),
    ),
  );
}

/// Drains the initial post-frame callback that fires loadInbox(refresh: true)
/// + loadUnreadCount on mount, so individual tests can measure subsequent
/// calls in isolation.
Future<void> _settleInitialBuild(WidgetTester tester) async {
  await tester.pump(); // initial build
  await tester.pump(); // post-frame callback
  await tester.pump(const Duration(milliseconds: 100));
}

void main() {
  group('InboxScreen pull-to-refresh (TMAIL-143)', () {
    testWidgets('initial build calls loadInbox(refresh: true) and loadUnreadCount',
        (tester) async {
      final provider = FakeMailProvider(seeded: [_summary(1)]);
      await tester.pumpWidget(_host(provider));
      await _settleInitialBuild(tester);

      expect(provider.refreshCalls, 1);
      expect(provider.unreadCountCalls, 1);
    });

    testWidgets('RefreshIndicator.show() triggers loadInbox(refresh: true)',
        (tester) async {
      final provider = FakeMailProvider(seeded: List.generate(3, _summary));
      await tester.pumpWidget(_host(provider));
      await _settleInitialBuild(tester);

      // Reset counters so we measure just the refresh.
      provider.refreshCalls = 0;
      provider.unreadCountCalls = 0;

      // NOTE: We call `show()` on the RefreshIndicator state directly rather
      // than dispatch a fling. Gesture-based RefreshIndicator activation is
      // notoriously flaky in widget tests (the indicator needs a sustained
      // overscroll past its display threshold). `show()` exercises the exact
      // same code path the production gesture lands on.
      final state = tester.state<RefreshIndicatorState>(
        find.byType(RefreshIndicator),
      );
      state.show();
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 300));

      expect(provider.refreshCalls, greaterThanOrEqualTo(1));
      expect(provider.unreadCountCalls, greaterThanOrEqualTo(1));

      // Allow the indicator to retract so the test exits cleanly.
      await tester.pump(const Duration(seconds: 2));
    });
  });

  group('InboxScreen infinite-scroll (TMAIL-143)', () {
    testWidgets('scrolling near the end triggers loadMore', (tester) async {
      final provider = FakeMailProvider(
        seeded: List.generate(40, _summary),
        hasMore: true,
      );
      await tester.pumpWidget(_host(provider));
      await _settleInitialBuild(tester);

      provider.loadMoreCalls = 0;

      // Fling the ListView upwards. The screen's _onScroll listener checks
      // every frame whether pixels >= maxScrollExtent - 200, so a single
      // hard fling lands well inside the trigger threshold.
      await tester.fling(find.byType(ListView), const Offset(0, -4000), 4000);
      // Sequenced pumps drive the scroll animation forward without waiting
      // on the perpetual pagination spinner.
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 200));
      await tester.pump(const Duration(milliseconds: 300));
      await tester.pump(const Duration(milliseconds: 500));

      expect(provider.loadMoreCalls, greaterThanOrEqualTo(1));
    });

    testWidgets('renders the trailing spinner when hasMore is true',
        (tester) async {
      final provider = FakeMailProvider(
        seeded: List.generate(20, _summary),
        hasMore: true,
      );
      await tester.pumpWidget(_host(provider));
      await _settleInitialBuild(tester);

      // Scroll to the bottom so the +1 spinner item is built.
      await tester.fling(find.byType(ListView), const Offset(0, -4000), 4000);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 200));
      await tester.pump(const Duration(milliseconds: 500));

      // hasMore=true means itemCount == messages.length + 1, so the trailing
      // CircularProgressIndicator must be present.
      expect(find.byType(CircularProgressIndicator), findsWidgets);
    });

    testWidgets('does NOT render trailing spinner when hasMore is false',
        (tester) async {
      final provider = FakeMailProvider(
        seeded: List.generate(20, _summary),
        hasMore: false,
      );
      await tester.pumpWidget(_host(provider));
      await _settleInitialBuild(tester);

      // No pagination spinner should be in the tree because itemCount equals
      // messages.length (no +1). The screen's full-screen loading spinner is
      // also gated by isLoadingInbox + messages.isEmpty, which is false here.
      expect(find.byType(CircularProgressIndicator), findsNothing);
    });
  });

  group('InboxScreen empty / error states (TMAIL-143)', () {
    testWidgets('renders empty-state placeholder when there are no messages',
        (tester) async {
      final provider = FakeMailProvider(seeded: const []);
      await tester.pumpWidget(_host(provider));
      await _settleInitialBuild(tester);

      expect(find.text('No messages'), findsOneWidget);
      expect(find.byIcon(Icons.inbox_outlined), findsOneWidget);
    });
  });
}
