// Added: TASMail mobile app entry point for TMAIL-139
// Changed: TMAIL-55 — wired native OS integrations (mailto: deep links + share
//          intents) via IntentDispatcher.
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
// Added: TMAIL-55 — native OS integration plumbing.
import 'services/native/deep_link_service.dart';
import 'services/native/intent_dispatcher.dart';
import 'services/native/share_intent_service.dart';

// Added: TMAIL-55 — global navigator key so IntentDispatcher can push routes
//        from outside the widget tree (cold-start share / mailto: handling).
final GlobalKey<NavigatorState> navigatorKey = GlobalKey<NavigatorState>();

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  runApp(const TasMailApp());
}

class TasMailApp extends StatefulWidget {
  const TasMailApp({super.key});

  @override
  State<TasMailApp> createState() => _TasMailAppState();
}

class _TasMailAppState extends State<TasMailApp> {
  IntentDispatcher? _dispatcher;

  @override
  void initState() {
    super.initState();
    // NOTE: dispatcher start is async; we don't await so the splash screen can
    //       render immediately. Failures inside start() are non-fatal —
    //       users can still open the app normally.
    _dispatcher = IntentDispatcher(
      navigatorKey: navigatorKey,
      shareService: ShareIntentServiceImpl(),
      deepLinkService: DeepLinkServiceImpl(),
    );
    _dispatcher!.start();
  }

  @override
  void dispose() {
    _dispatcher?.dispose();
    super.dispose();
  }

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
            // Added: TMAIL-55 — share/deep-link push needs a navigator key.
            navigatorKey: navigatorKey,
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
                      // Added: TMAIL-145 — Reply-All entry point + the current
                      //   user's email so the Cc list excludes self.
                      replyAll: args?['replyAll'] as MobileMessageDetail?,
                      currentUserEmail: args?['currentUserEmail'] as String?,
                      forward: args?['forward'] as MobileMessageDetail?,
                      // Added: TMAIL-55 — prefill from mailto: / share intent.
                      prefill: args?['prefill'] as ComposePrefill?,
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
