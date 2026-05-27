// Added: Widget tests for LoginScreen for TMAIL-141
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:provider/provider.dart';
import 'package:tasmail_mobile/providers/auth_provider.dart';
import 'package:tasmail_mobile/screens/auth/login_screen.dart';

void main() {
  Widget createTestWidget() {
    // NOTE: Don't call checkAuth() to avoid async flutter_secure_storage issues in tests
    // NOTE: NoSplash.splashFactory avoids loading shaders/ink_sparkle.frag,
    //       which fails to decode on Flutter 3.44+ (see docs/research/flutter-test-ink-sparkle-shader.md)
    return ChangeNotifierProvider(
      create: (_) => AuthProvider(),
      child: MaterialApp(
        theme: ThemeData(splashFactory: NoSplash.splashFactory),
        home: const LoginScreen(),
      ),
    );
  }

  group('LoginScreen', () {
    testWidgets('renders email and password fields', (tester) async {
      await tester.pumpWidget(createTestWidget());
      // NOTE: Need to wait for AuthProvider.checkAuth microtask
      await tester.pump();

      expect(find.text('TASMail'), findsOneWidget);
      expect(find.text('Sign in to your account'), findsOneWidget);
      expect(find.byKey(const Key('email_field')), findsOneWidget);
      expect(find.byKey(const Key('password_field')), findsOneWidget);
      expect(find.byKey(const Key('login_button')), findsOneWidget);
    });

    testWidgets('shows validation errors for empty fields', (tester) async {
      await tester.pumpWidget(createTestWidget());
      await tester.pump();
      // NOTE: Wait for isLoading to become false so button is enabled
      await tester.pump(const Duration(seconds: 1));

      // Tap login without filling fields
      await tester.tap(find.byKey(const Key('login_button')));
      await tester.pump();

      expect(find.text('Email is required'), findsOneWidget);
      expect(find.text('Password is required'), findsOneWidget);
    });

    testWidgets('validates email format', (tester) async {
      await tester.pumpWidget(createTestWidget());
      await tester.pump();
      await tester.pump(const Duration(seconds: 1));

      await tester.enterText(find.byKey(const Key('email_field')), 'notanemail');
      await tester.enterText(find.byKey(const Key('password_field')), 'pass123');
      await tester.tap(find.byKey(const Key('login_button')));
      await tester.pump();

      expect(find.text('Enter a valid email'), findsOneWidget);
    });

    testWidgets('toggles password visibility', (tester) async {
      await tester.pumpWidget(createTestWidget());
      await tester.pump();

      expect(find.byIcon(Icons.visibility_off), findsOneWidget);

      await tester.tap(find.byIcon(Icons.visibility_off));
      await tester.pump();

      expect(find.byIcon(Icons.visibility), findsOneWidget);
    });

    testWidgets('shows app logo', (tester) async {
      await tester.pumpWidget(createTestWidget());
      await tester.pump();

      expect(find.byIcon(Icons.mail_outline), findsOneWidget);
    });
  });
}
