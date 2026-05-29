// Added: Widget tests for the TMAIL-145 EmailRecipientField (To/Cc autocomplete).
// PURPOSE: Confirm typing fires the suggestion service, the overlay renders,
//          selecting a suggestion replaces the last token, and the existing
//          tokens are preserved.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:tasmail_mobile/models/contact_suggestion.dart';
import 'package:tasmail_mobile/services/contact_suggestion_service.dart';
import 'package:tasmail_mobile/widgets/email_recipient_field.dart';

class _FakeSuggestionService implements ContactSuggestionService {
  final List<ContactSuggestion> results;
  final List<String> queries = [];
  _FakeSuggestionService(this.results);

  @override
  Future<List<ContactSuggestion>> suggest(String query) async {
    queries.add(query);
    return results;
  }
}

Widget _wrap(EmailRecipientField field) {
  return MaterialApp(
    theme: ThemeData(splashFactory: NoSplash.splashFactory),
    home: Scaffold(body: field),
  );
}

void main() {
  group('EmailRecipientField (TMAIL-145)', () {
    testWidgets('renders the inner TextField with the supplied key',
        (tester) async {
      final controller = TextEditingController();
      addTearDown(controller.dispose);
      final service = _FakeSuggestionService(const []);

      await tester.pumpWidget(_wrap(EmailRecipientField(
        fieldKey: const Key('to_field'),
        controller: controller,
        suggestionService: service,
        label: 'To',
        // Zero debounce keeps tests deterministic.
        debounce: Duration.zero,
      )));

      expect(find.byKey(const Key('to_field')), findsOneWidget);
      expect(find.text('To'), findsOneWidget);
    });

    testWidgets('typing >=2 chars queries the suggestion service',
        (tester) async {
      final controller = TextEditingController();
      addTearDown(controller.dispose);
      final service = _FakeSuggestionService(const [
        ContactSuggestion(email: 'alice@example.com', displayName: 'Alice'),
      ]);

      await tester.pumpWidget(_wrap(EmailRecipientField(
        fieldKey: const Key('to_field'),
        controller: controller,
        suggestionService: service,
        label: 'To',
        debounce: Duration.zero,
      )));

      await tester.tap(find.byKey(const Key('to_field')));
      await tester.enterText(find.byKey(const Key('to_field')), 'al');
      await tester.pumpAndSettle();

      expect(service.queries, contains('al'));
    });

    testWidgets('selecting a suggestion appends formatted email + ", "',
        (tester) async {
      final controller = TextEditingController();
      addTearDown(controller.dispose);
      final service = _FakeSuggestionService(const [
        ContactSuggestion(email: 'alice@example.com', displayName: 'Alice'),
      ]);

      await tester.pumpWidget(_wrap(EmailRecipientField(
        fieldKey: const Key('to_field'),
        controller: controller,
        suggestionService: service,
        label: 'To',
        debounce: Duration.zero,
      )));

      await tester.tap(find.byKey(const Key('to_field')));
      await tester.enterText(find.byKey(const Key('to_field')), 'al');
      await tester.pumpAndSettle();

      // The overlay shows the contact name; tap it.
      await tester.tap(
        find.byKey(const Key('contact_suggestion_alice@example.com')),
      );
      await tester.pumpAndSettle();

      expect(controller.text, 'Alice <alice@example.com>, ');
    });

    testWidgets('selection preserves earlier tokens and only replaces the last',
        (tester) async {
      final controller =
          TextEditingController(text: 'first@example.com, al');
      addTearDown(controller.dispose);
      final service = _FakeSuggestionService(const [
        ContactSuggestion(email: 'alice@example.com', displayName: 'Alice'),
      ]);

      await tester.pumpWidget(_wrap(EmailRecipientField(
        fieldKey: const Key('to_field'),
        controller: controller,
        suggestionService: service,
        label: 'To',
        debounce: Duration.zero,
      )));

      // Focus + nudge to trigger the options builder.
      await tester.tap(find.byKey(const Key('to_field')));
      await tester.enterText(
        find.byKey(const Key('to_field')),
        'first@example.com, al',
      );
      await tester.pumpAndSettle();

      await tester.tap(
        find.byKey(const Key('contact_suggestion_alice@example.com')),
      );
      await tester.pumpAndSettle();

      expect(
        controller.text,
        'first@example.com, Alice <alice@example.com>, ',
      );
    });

    testWidgets('short queries (<2 chars) do NOT call the service',
        (tester) async {
      final controller = TextEditingController();
      addTearDown(controller.dispose);
      final service = _FakeSuggestionService(const []);

      await tester.pumpWidget(_wrap(EmailRecipientField(
        fieldKey: const Key('to_field'),
        controller: controller,
        suggestionService: service,
        label: 'To',
        debounce: Duration.zero,
      )));

      await tester.tap(find.byKey(const Key('to_field')));
      await tester.enterText(find.byKey(const Key('to_field')), 'a');
      await tester.pumpAndSettle();

      expect(service.queries, isEmpty);
    });
  });

  group('EmailRecipientField.applySelection (unit)', () {
    // NOTE: applySelection is exposed for test-only sanity checks of the
    //       comma-token replacement math. Production code reaches it via
    //       onSelected in the RawAutocomplete callback.
    test('replaces the bare query when there are no earlier tokens', () {
      final state = _ApplyHarness();
      expect(
        state.apply(
          'al',
          const ContactSuggestion(email: 'a@x.com', displayName: 'Alice'),
        ),
        'Alice <a@x.com>, ',
      );
    });

    test('preserves earlier tokens and the comma between them', () {
      final state = _ApplyHarness();
      expect(
        state.apply(
          'one@x.com, two@x.com, al',
          const ContactSuggestion(email: 'a@x.com', displayName: 'Alice'),
        ),
        'one@x.com, two@x.com, Alice <a@x.com>, ',
      );
    });

    test('falls back to bare email when display name is missing', () {
      final state = _ApplyHarness();
      expect(
        state.apply(
          'al',
          const ContactSuggestion(email: 'a@x.com'),
        ),
        'a@x.com, ',
      );
    });
  });
}

/// Tiny harness that exposes the private _applySelection logic via a public
/// EmailRecipientField instance. We don't need a live widget tree because the
/// method is pure — it operates on the input text and the suggestion.
class _ApplyHarness {
  String apply(String current, ContactSuggestion s) {
    final controller = TextEditingController(text: current);
    final field = EmailRecipientField(
      controller: controller,
      suggestionService: _StubService(),
      label: 'To',
    );
    final state = field.createState() as dynamic;
    final result = state.applySelection(current, s) as String;
    controller.dispose();
    return result;
  }
}

class _StubService implements ContactSuggestionService {
  @override
  Future<List<ContactSuggestion>> suggest(String query) async => const [];
}
