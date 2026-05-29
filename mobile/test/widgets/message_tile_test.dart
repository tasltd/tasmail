// Added: Widget tests for MessageTile (TMAIL-143)
// PURPOSE: Verify the inbox list tile renders sender, subject, date, attachment
//          indicator, and read/unread styling, and that tap + flag toggle
//          callbacks fire correctly.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:tasmail_mobile/models/email.dart';
import 'package:tasmail_mobile/widgets/message_tile.dart';

MobileMessageSummary _summary({
  int uid = 1,
  String folder = 'INBOX',
  String? from = 'alice@example.com',
  String? subject = 'Hello there',
  String? date,
  bool isRead = false,
  bool isFlagged = false,
  bool hasAttachment = false,
}) {
  return MobileMessageSummary(
    uid: uid,
    folder: folder,
    from: from,
    subject: subject,
    date: date,
    isRead: isRead,
    isFlagged: isFlagged,
    hasAttachment: hasAttachment,
  );
}

Widget _host(Widget child) {
  return MaterialApp(
    theme: ThemeData(splashFactory: NoSplash.splashFactory),
    home: Scaffold(body: ListView(children: [child])),
  );
}

void main() {
  group('MessageTile rendering (TMAIL-143)', () {
    testWidgets('renders sender, subject, and avatar initial', (tester) async {
      await tester.pumpWidget(_host(MessageTile(
        message: _summary(from: 'Bob <bob@example.com>', subject: 'Lunch?'),
        onTap: () {},
      )));

      expect(find.text('Bob <bob@example.com>'), findsOneWidget);
      expect(find.text('Lunch?'), findsOneWidget);
      // Avatar uses the first character of `from`, uppercased.
      expect(find.text('B'), findsOneWidget);
    });

    testWidgets('falls back to "Unknown" sender and "(no subject)"',
        (tester) async {
      await tester.pumpWidget(_host(MessageTile(
        message: _summary(from: null, subject: null),
        onTap: () {},
      )));

      expect(find.text('Unknown'), findsOneWidget);
      expect(find.text('(no subject)'), findsOneWidget);
    });

    testWidgets('unread message uses bold title weight', (tester) async {
      await tester.pumpWidget(_host(MessageTile(
        message: _summary(isRead: false, subject: 'Important'),
        onTap: () {},
      )));

      final titleText = tester.widget<Text>(find.text('alice@example.com'));
      expect(titleText.style?.fontWeight, FontWeight.bold);
    });

    testWidgets('read message uses normal title weight', (tester) async {
      await tester.pumpWidget(_host(MessageTile(
        message: _summary(isRead: true),
        onTap: () {},
      )));

      final titleText = tester.widget<Text>(find.text('alice@example.com'));
      expect(titleText.style?.fontWeight, FontWeight.normal);
    });

    testWidgets('shows attachment icon when hasAttachment is true',
        (tester) async {
      await tester.pumpWidget(_host(MessageTile(
        message: _summary(hasAttachment: true),
        onTap: () {},
      )));

      expect(find.byIcon(Icons.attach_file), findsOneWidget);
    });

    testWidgets('hides attachment icon when hasAttachment is false',
        (tester) async {
      await tester.pumpWidget(_host(MessageTile(
        message: _summary(hasAttachment: false),
        onTap: () {},
      )));

      expect(find.byIcon(Icons.attach_file), findsNothing);
    });

    testWidgets('flagged message shows filled star, unflagged shows outline',
        (tester) async {
      await tester.pumpWidget(_host(MessageTile(
        message: _summary(isFlagged: true),
        onTap: () {},
      )));
      expect(find.byIcon(Icons.star), findsOneWidget);
      expect(find.byIcon(Icons.star_border), findsNothing);

      await tester.pumpWidget(_host(MessageTile(
        message: _summary(isFlagged: false),
        onTap: () {},
      )));
      expect(find.byIcon(Icons.star_border), findsOneWidget);
      expect(find.byIcon(Icons.star), findsNothing);
    });

    testWidgets('tapping the tile fires onTap', (tester) async {
      var tapped = 0;
      await tester.pumpWidget(_host(MessageTile(
        message: _summary(),
        onTap: () => tapped++,
      )));

      await tester.tap(find.byType(ListTile));
      await tester.pumpAndSettle();
      expect(tapped, 1);
    });

    testWidgets('tapping the star fires onFlagToggle (and not onTap)',
        (tester) async {
      var tapped = 0;
      var flagToggled = 0;
      await tester.pumpWidget(_host(MessageTile(
        message: _summary(isFlagged: false),
        onTap: () => tapped++,
        onFlagToggle: () => flagToggled++,
      )));

      await tester.tap(find.byIcon(Icons.star_border));
      await tester.pumpAndSettle();
      expect(flagToggled, 1);
      expect(tapped, 0);
    });
  });

  group('MessageTile date formatting (TMAIL-143)', () {
    testWidgets('today renders HH:mm', (tester) async {
      final now = DateTime.now();
      final iso = DateTime(now.year, now.month, now.day, 9, 5).toIso8601String();
      await tester.pumpWidget(_host(MessageTile(
        message: _summary(date: iso),
        onTap: () {},
      )));

      // Two-digit zero-padded HH:mm.
      expect(find.text('09:05'), findsOneWidget);
    });

    testWidgets('within last week renders weekday abbreviation', (tester) async {
      final threeDaysAgo = DateTime.now().subtract(const Duration(days: 3));
      await tester.pumpWidget(_host(MessageTile(
        message: _summary(date: threeDaysAgo.toIso8601String()),
        onTap: () {},
      )));

      const days = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
      expect(find.text(days[threeDaysAgo.weekday - 1]), findsOneWidget);
    });

    testWidgets('older than a week renders D/M/YYYY', (tester) async {
      // Stable date well in the past, regardless of when the test runs.
      await tester.pumpWidget(_host(MessageTile(
        message: _summary(date: '2024-01-15T10:00:00Z'),
        onTap: () {},
      )));

      expect(find.text('15/1/2024'), findsOneWidget);
    });

    testWidgets('unparseable date string is shown as-is', (tester) async {
      await tester.pumpWidget(_host(MessageTile(
        message: _summary(date: 'not-a-date'),
        onTap: () {},
      )));

      expect(find.text('not-a-date'), findsOneWidget);
    });
  });
}
