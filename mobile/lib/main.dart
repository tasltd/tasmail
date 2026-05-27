// Added: TASMail mobile app entry point for TMAIL-139
// PURPOSE: App initialization with providers, routing, and Material 3 theme
// EXTERNAL: Uses Provider for state management, MaterialApp for routing

import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:provider/provider.dart';
import 'l10n/app_localizations.dart';
import 'providers/auth_provider.dart';
import 'providers/mail_provider.dart';
import 'screens/auth/login_screen.dart';
import 'screens/home_screen.dart';
import 'screens/message/message_screen.dart';
import 'screens/compose/compose_screen.dart';
// Added: Biometric Lock settings route for TMAIL-142
import 'screens/settings/biometric_settings_screen.dart';
import 'models/email.dart';

void main() {
  runApp(const TasMailApp());
}

class TasMailApp extends StatelessWidget {
  const TasMailApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MultiProvider(
      providers: [
        ChangeNotifierProvider(create: (_) => AuthProvider()..checkAuth()),
        ChangeNotifierProvider(create: (_) => MailProvider()),
      ],
      child: Consumer<AuthProvider>(
        builder: (context, auth, _) {
          return MaterialApp(
            title: 'TASMail',
            debugShowCheckedModeBanner: false,
            // Added: Localization support for Ghana languages (Twi, Ewe, Ga, Hausa)
            localizationsDelegates: const [
              AppLocalizations.delegate,
              GlobalMaterialLocalizations.delegate,
              GlobalWidgetsLocalizations.delegate,
              GlobalCupertinoLocalizations.delegate,
            ],
            supportedLocales: AppLocalizations.supportedLocales,
            theme: ThemeData(
              colorScheme: ColorScheme.fromSeed(
                seedColor: const Color(0xFF1565C0),
                brightness: Brightness.light,
              ),
              useMaterial3: true,
            ),
            darkTheme: ThemeData(
              colorScheme: ColorScheme.fromSeed(
                seedColor: const Color(0xFF1565C0),
                brightness: Brightness.dark,
              ),
              useMaterial3: true,
            ),
            themeMode: ThemeMode.system,
            // Added: Route based on auth state
            home: auth.isLoading
                ? const _SplashScreen()
                : auth.isAuthenticated
                    ? const HomeScreen()
                    : const LoginScreen(),
            onGenerateRoute: (settings) {
              switch (settings.name) {
                case '/message':
                  final args = settings.arguments as Map<String, dynamic>;
                  return MaterialPageRoute(
                    builder: (_) => MessageScreen(
                      folder: args['folder'] as String,
                      uid: args['uid'] as int,
                    ),
                  );
                case '/compose':
                  final args = settings.arguments as Map<String, dynamic>?;
                  return MaterialPageRoute(
                    builder: (_) => ComposeScreen(
                      replyTo: args?['replyTo'] as MobileMessageDetail?,
                      forward: args?['forward'] as MobileMessageDetail?,
                    ),
                  );
                // Added: Biometric Lock settings route for TMAIL-142
                case '/settings/biometric':
                  return MaterialPageRoute(
                    builder: (_) => const BiometricSettingsScreen(),
                  );
                default:
                  return null;
              }
            },
          );
        },
      ),
    );
  }
}

// Added: Simple splash screen shown during auth check
class _SplashScreen extends StatelessWidget {
  const _SplashScreen();

  @override
  Widget build(BuildContext context) {
    return const Scaffold(
      body: Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(Icons.mail_outline, size: 64, color: Color(0xFF1565C0)),
            SizedBox(height: 16),
            Text(
              'TASMail',
              style: TextStyle(fontSize: 24, fontWeight: FontWeight.bold),
            ),
            SizedBox(height: 24),
            CircularProgressIndicator(),
          ],
        ),
      ),
    );
  }
}
