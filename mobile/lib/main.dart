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
// Added: Configurable swipe actions for TMAIL-148
import 'screens/settings/swipe_actions_screen.dart';
import 'services/swipe_actions_service.dart';
import 'models/email.dart';
// Added: TMAIL-55 — native OS integration plumbing.
import 'services/native/deep_link_service.dart';
import 'services/native/intent_dispatcher.dart';
import 'services/native/share_intent_service.dart';
// Added: TMAIL-150 — FCM bootstrap (token registration + tap navigation).
//        Default token provider is a no-op until firebase_messaging is wired
//        per `docs/MOBILE-FCM-SETUP.md`. Already plumbed into AuthProvider so
//        flipping FCM on is a single-line provider swap below.
import 'services/fcm_bootstrap.dart';

// Added: TMAIL-55 — global navigator key so IntentDispatcher can push routes
//        from outside the widget tree (cold-start share / mailto: handling).
//        Reused by TMAIL-150 FcmBootstrap for cold-start notification taps.
final GlobalKey<NavigatorState> navigatorKey = GlobalKey<NavigatorState>();

// Added: TMAIL-150 — process-wide FcmBootstrap. Holds the most recent
//        registered token so repeat register() calls are idempotent across
//        login/logout cycles. The navigator callback drives `/message` route
//        pushes on notification tap via the global navigatorKey.
//
//        TO ENABLE REAL FCM (after Firebase project setup + `flutterfire
//        configure` — see docs/MOBILE-FCM-SETUP.md): swap `tokenProvider:`
//        to `() => FirebaseMessaging.instance.getToken()` and `refreshStream:`
//        to `() => FirebaseMessaging.instance.onTokenRefresh`, then register
//        the top-level background handler via
//        `FirebaseMessaging.onBackgroundMessage(firebaseMessagingBackgroundHandler)`
//        in `main()` before `runApp`.
final FcmBootstrap fcmBootstrap = FcmBootstrap(
  navigator: (route, args) {
    final nav = navigatorKey.currentState;
    if (nav == null) return;
    nav.pushNamed(route, arguments: args);
  },
  // Platform string sent to backend. Today we register as 'fcm' since the
  // mobile app targets Android-first (Ghana market — TMAIL-49). iOS swap to
  // 'apns' happens when the iOS build lands.
  platform: FcmPlatformId.fcm,
);

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
        // Changed: TMAIL-150 — pass FcmBootstrap.register as the post-auth
        //          hook so a fresh login (or session resume) re-registers the
        //          device with /api/push/register. Inert today (no token
        //          provider), live the moment firebase_messaging is wired.
        ChangeNotifierProvider(
          create: (_) => AuthProvider(
            onAuthenticated: () async {
              await fcmBootstrap.register();
            },
          )..checkAuth(),
        ),
        ChangeNotifierProvider(create: (_) => MailProvider()),
        // Added: TMAIL-148 — configurable swipe actions. Service hydrates from
        //        secure storage on first read; defaults match the pre-148
        //        behaviour so nothing flashes weird on cold start.
        ChangeNotifierProvider(create: (_) => SwipeActionsService()..load()),
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
                // Added: Configurable swipe actions route for TMAIL-148
                case '/settings/swipe-actions':
                  return MaterialPageRoute(
                    builder: (ctx) => SwipeActionsScreen(
                      service: Provider.of<SwipeActionsService>(
                        ctx,
                        listen: false,
                      ),
                    ),
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
