// Added: Widget tests for SettingsScreen for TMAIL-152
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:provider/provider.dart';
import 'package:tasmail_mobile/providers/auth_provider.dart';
import 'package:tasmail_mobile/screens/settings/settings_screen.dart';

void main() {
  Widget createTestWidget() {
    return ChangeNotifierProvider(
      create: (_) => AuthProvider(),
      child: const MaterialApp(home: SettingsScreen()),
    );
  }

  group('SettingsScreen', () {
    testWidgets('renders settings title and mail section', (tester) async {
      await tester.pumpWidget(createTestWidget());

      expect(find.text('Settings'), findsOneWidget);
      expect(find.text('Mail'), findsOneWidget);
      expect(find.text('Signatures'), findsOneWidget);
      expect(find.text('Contacts'), findsOneWidget);
    });

    testWidgets('renders labels and filters items', (tester) async {
      await tester.pumpWidget(createTestWidget());

      expect(find.text('Labels & Folders'), findsOneWidget);
      expect(find.text('Filters'), findsOneWidget);
      expect(find.text('Auto Reply'), findsOneWidget);
    });

    testWidgets('renders security section after scrolling', (tester) async {
      await tester.pumpWidget(createTestWidget());

      // Scroll down to find security section
      await tester.scrollUntilVisible(
        find.text('Two-Factor Auth'),
        100,
      );
      expect(find.text('Two-Factor Auth'), findsOneWidget);
    });

    testWidgets('renders sign out after scrolling', (tester) async {
      await tester.pumpWidget(createTestWidget());

      await tester.scrollUntilVisible(
        find.text('Sign Out'),
        100,
      );
      expect(find.text('Sign Out'), findsOneWidget);
    });

    testWidgets('shows coming soon snackbar on item tap', (tester) async {
      await tester.pumpWidget(createTestWidget());

      await tester.tap(find.text('Signatures'));
      await tester.pumpAndSettle();

      expect(find.text('Signatures - coming soon'), findsOneWidget);
    });
  });
}
