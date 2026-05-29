// Added: Widget tests for TMAIL-145 reply-all prefill.
// PURPOSE: Validate To-only-sender, Cc = original(to+cc) - self, "Re:" subject,
//          and "Reply All" app-bar title.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:tasmail_mobile/models/email.dart';
import 'package:tasmail_mobile/models/contact_suggestion.dart';
import 'package:tasmail_mobile/screens/compose/compose_screen.dart';
import 'package:tasmail_mobile/services/contact_suggestion_service.dart';

class _NoopSuggestionService implements ContactSuggestionService {
  @override
  Future<List<ContactSuggestion>> suggest(String query) async => const [];
}

Widget _wrap({
  required MobileMessageDetail replyAll,
  String? currentUserEmail,
}) {
  return MaterialApp(
    theme: ThemeData(splashFactory: NoSplash.splashFactory),
    home: ComposeScreen(
      replyAll: replyAll,
      currentUserEmail: currentUserEmail,
      suggestionService: _NoopSuggestionService(),
    ),
  );
}

void main() {
  const original = MobileMessageDetail(
    uid: 7,
    folder: 'INBOX',
    from: 'sender@example.com',
    to: ['me@example.com', 'alice@example.com'],
    cc: ['bob@example.com', 'carol@example.com'],
    subject: 'Original Subject',
    bodyText: 'Hello team',
    isRead: true,
    isFlagged: false,
    hasAttachment: false,
    attachments: [],
  );

  group('ComposeScreen — reply-all (TMAIL-145)', () {
    testWidgets('title shows "Reply All"', (tester) async {
      await tester.pumpWidget(
        _wrap(replyAll: original, currentUserEmail: 'me@example.com'),
      );
      expect(find.text('Reply All'), findsOneWidget);
    });

    testWidgets('To is the original sender; Cc excludes self', (tester) async {
      await tester.pumpWidget(
        _wrap(replyAll: original, currentUserEmail: 'me@example.com'),
      );

      final toField = tester.widget<TextField>(find.byKey(const Key('to_field')));
      expect(toField.controller?.text, 'sender@example.com');

      // Cc must be visible (it had recipients) and must NOT contain self.
      final ccField =
          tester.widget<TextField>(find.byKey(const Key('cc_field')));
      final cc = ccField.controller?.text ?? '';
      expect(cc.contains('alice@example.com'), isTrue);
      expect(cc.contains('bob@example.com'), isTrue);
      expect(cc.contains('carol@example.com'), isTrue);
      expect(cc.contains('me@example.com'), isFalse);
    });

    testWidgets('Cc also excludes the original sender (already in To)',
        (tester) async {
      const echoed = MobileMessageDetail(
        uid: 8,
        folder: 'INBOX',
        from: 'sender@example.com',
        // Sender accidentally appears in to/cc — must not echo into Cc.
        to: ['sender@example.com', 'alice@example.com'],
        cc: [],
        subject: 'Hi',
        bodyText: '',
        isRead: true,
        isFlagged: false,
        hasAttachment: false,
        attachments: [],
      );
      await tester.pumpWidget(_wrap(replyAll: echoed));
      final ccField =
          tester.widget<TextField>(find.byKey(const Key('cc_field')));
      final cc = ccField.controller?.text ?? '';
      expect(cc.contains('alice@example.com'), isTrue);
      expect(cc.contains('sender@example.com'), isFalse);
    });

    testWidgets('Subject gets a single "Re:" prefix', (tester) async {
      await tester.pumpWidget(_wrap(replyAll: original));
      final subj =
          tester.widget<TextField>(find.byKey(const Key('subject_field')));
      expect(subj.controller?.text, 'Re: Original Subject');
    });

    testWidgets('No double "Re:" when subject already starts with it',
        (tester) async {
      const replied = MobileMessageDetail(
        uid: 9,
        folder: 'INBOX',
        from: 'sender@example.com',
        to: ['me@example.com'],
        cc: [],
        subject: 'Re: Hello',
        bodyText: '',
        isRead: true,
        isFlagged: false,
        hasAttachment: false,
        attachments: [],
      );
      await tester.pumpWidget(_wrap(replyAll: replied));
      final subj =
          tester.widget<TextField>(find.byKey(const Key('subject_field')));
      expect(subj.controller?.text, 'Re: Hello');
    });

    testWidgets('handles Name <email> format for self filtering',
        (tester) async {
      const formatted = MobileMessageDetail(
        uid: 10,
        folder: 'INBOX',
        from: 'Sender <sender@example.com>',
        to: ['My Name <me@example.com>', 'Alice <alice@example.com>'],
        cc: [],
        subject: 'Hi',
        bodyText: '',
        isRead: true,
        isFlagged: false,
        hasAttachment: false,
        attachments: [],
      );
      await tester.pumpWidget(_wrap(
        replyAll: formatted,
        currentUserEmail: 'me@example.com',
      ));

      final toField =
          tester.widget<TextField>(find.byKey(const Key('to_field')));
      expect(toField.controller?.text, 'Sender <sender@example.com>');

      final ccField =
          tester.widget<TextField>(find.byKey(const Key('cc_field')));
      final cc = ccField.controller?.text ?? '';
      expect(cc.contains('Alice <alice@example.com>'), isTrue);
      expect(cc.contains('me@example.com'), isFalse);
    });
  });
}
