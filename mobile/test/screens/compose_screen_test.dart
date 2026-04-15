// Added: Widget tests for ComposeScreen for TMAIL-145
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:tasmail_mobile/screens/compose/compose_screen.dart';
import 'package:tasmail_mobile/models/email.dart';

void main() {
  Widget createTestWidget({
    MobileMessageDetail? replyTo,
    MobileMessageDetail? forward,
  }) {
    return MaterialApp(
      home: ComposeScreen(replyTo: replyTo, forward: forward),
    );
  }

  group('ComposeScreen', () {
    testWidgets('renders compose form fields', (tester) async {
      await tester.pumpWidget(createTestWidget());

      expect(find.text('Compose'), findsOneWidget);
      expect(find.byKey(const Key('to_field')), findsOneWidget);
      expect(find.byKey(const Key('subject_field')), findsOneWidget);
      expect(find.byKey(const Key('body_field')), findsOneWidget);
      expect(find.byKey(const Key('send_button')), findsOneWidget);
    });

    testWidgets('CC field is hidden by default', (tester) async {
      await tester.pumpWidget(createTestWidget());

      expect(find.byKey(const Key('cc_field')), findsNothing);
    });

    testWidgets('CC field appears when toggled', (tester) async {
      await tester.pumpWidget(createTestWidget());

      // Tap "Cc" button to show CC field
      await tester.tap(find.text('Cc'));
      await tester.pump();

      expect(find.byKey(const Key('cc_field')), findsOneWidget);
    });

    testWidgets('prefills subject for reply', (tester) async {
      const original = MobileMessageDetail(
        uid: 1,
        folder: 'INBOX',
        from: 'sender@example.com',
        to: ['me@example.com'],
        cc: [],
        subject: 'Original Subject',
        bodyText: 'Original body',
        isRead: true,
        isFlagged: false,
        hasAttachment: false,
        attachments: [],
      );

      await tester.pumpWidget(createTestWidget(replyTo: original));

      expect(find.text('Reply'), findsOneWidget);
      // NOTE: To field should be prefilled with sender
      final toField = tester.widget<TextField>(find.byKey(const Key('to_field')));
      expect(toField.controller?.text, 'sender@example.com');
    });

    testWidgets('prefills subject for forward', (tester) async {
      const original = MobileMessageDetail(
        uid: 2,
        folder: 'INBOX',
        from: 'sender@example.com',
        to: ['me@example.com'],
        cc: [],
        subject: 'Original Subject',
        bodyText: 'Body content',
        isRead: true,
        isFlagged: false,
        hasAttachment: false,
        attachments: [],
      );

      await tester.pumpWidget(createTestWidget(forward: original));

      expect(find.text('Forward'), findsOneWidget);
      // NOTE: Subject should have Fwd: prefix
      final subjectField = tester.widget<TextField>(
        find.byKey(const Key('subject_field')),
      );
      expect(subjectField.controller?.text, 'Fwd: Original Subject');
    });

    testWidgets('shows snackbar when sending without recipient', (tester) async {
      await tester.pumpWidget(createTestWidget());

      await tester.tap(find.byKey(const Key('send_button')));
      await tester.pumpAndSettle();

      expect(find.text('Please enter a recipient'), findsOneWidget);
    });
  });
}
