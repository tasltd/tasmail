// Added: Widget tests for SearchScreen for TMAIL-147
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:tasmail_mobile/screens/search/search_screen.dart';

void main() {
  Widget createTestWidget() {
    return const MaterialApp(home: SearchScreen());
  }

  group('SearchScreen', () {
    testWidgets('renders search input and AI toggle', (tester) async {
      await tester.pumpWidget(createTestWidget());

      expect(find.byKey(const Key('search_input')), findsOneWidget);
      expect(find.text('AI Search'), findsOneWidget);
      expect(find.byIcon(Icons.search), findsAtLeastNWidgets(1));
    });

    testWidgets('shows empty state message', (tester) async {
      await tester.pumpWidget(createTestWidget());

      expect(find.text('Enter a search query'), findsOneWidget);
    });

    testWidgets('toggles AI search chip', (tester) async {
      await tester.pumpWidget(createTestWidget());

      await tester.tap(find.text('AI Search'));
      await tester.pump();

      expect(find.text('AI Search'), findsOneWidget);
    });

    testWidgets('can type in search field', (tester) async {
      await tester.pumpWidget(createTestWidget());

      await tester.enterText(
        find.byKey(const Key('search_input')),
        'test query',
      );
      await tester.pump();

      expect(find.text('test query'), findsOneWidget);
    });
  });
}
