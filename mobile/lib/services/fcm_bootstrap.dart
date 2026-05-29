// Added: FCM bootstrap contract for TMAIL-150
// PURPOSE: Glue layer between firebase_messaging (when it's wired up by the
//          operator — see `docs/MOBILE-FCM-SETUP.md`) and the rest of the app.
//          Today this layer ships with a no-op default token provider so the
//          app builds without firebase_messaging in pubspec.yaml. The moment
//          the operator drops in `google-services.json` + the FlutterFire deps,
//          enabling real FCM is a one-line swap in `main.dart`:
//
//              tokenProvider: () => FirebaseMessaging.instance.getToken(
//                vapidKey: kVapidKeyOrNull,
//              ),
//
//          …and registering the top-level background handler:
//
//              @pragma('vm:entry-point')
//              Future<void> firebaseMessagingBackgroundHandler(RemoteMessage m) async {
//                await FcmBootstrap.handleTapData(m.data); // no UI side-effects
//              }
//              FirebaseMessaging.onBackgroundMessage(firebaseMessagingBackgroundHandler);
//
// EXTERNAL: Calls `PushService.registerToken` (→ POST /api/push/register).
//           Navigation goes through a `GlobalKey<NavigatorState>` so cold-start
//           taps from FCM work even before the widget tree is mounted.

import 'push_service.dart';

// PURPOSE: Async getter for the current device's push token. Default returns
//          null so the bootstrap is inert until firebase_messaging is wired.
typedef FcmTokenProvider = Future<String?> Function();

// PURPOSE: Listener for token-rotation events from firebase_messaging. Default
//          is a no-op stream that never emits.
typedef FcmTokenRefreshStream = Stream<String> Function();

// PURPOSE: Push-notification payload (FCM `data` map) handler. The bootstrap
//          owns the canonical parse + navigation logic so background-isolate
//          handlers can call the same code path.
typedef FcmTapHandler = void Function(Map<String, dynamic> data);

// NOTE: A "tap navigator" is just an in-app router callback. Pulled out so the
//       test suite can drive tap-handling without a NavigatorState.
typedef FcmTapNavigator = void Function(
  String routeName,
  Object? arguments,
);

// PURPOSE: Platform identifier sent to the backend. Backend accepts only
//          'fcm', 'apns', 'web' (see `PushPlatform::from_str`).
//          We deliberately don't auto-detect from `Platform.isAndroid` here —
//          the token *source* (FCM vs APNs) is what matters, and a real device
//          on iOS could still use FCM via the firebase_messaging plugin. The
//          operator decides per-platform when wiring real FCM.
class FcmPlatformId {
  static const String fcm = 'fcm';
  static const String apns = 'apns';
  static const String web = 'web';
}

class FcmBootstrap {
  final PushService _push;
  final FcmTokenProvider _tokenProvider;
  final FcmTokenRefreshStream _refreshStream;
  final FcmTapNavigator _navigator;
  final String _platform;
  final String? _deviceName;
  final String? _appVersion;

  // PURPOSE: Most recently-registered token, so `register()` is idempotent
  //          when called more than once with the same token (e.g. on every
  //          successful login).
  String? _lastRegisteredToken;
  String? get lastRegisteredToken => _lastRegisteredToken;

  FcmBootstrap({
    required FcmTapNavigator navigator,
    required String platform,
    PushService? pushService,
    FcmTokenProvider? tokenProvider,
    FcmTokenRefreshStream? refreshStream,
    String? deviceName,
    String? appVersion,
  })  : _push = pushService ?? PushService(),
        _navigator = navigator,
        _platform = platform,
        _tokenProvider = tokenProvider ?? _nullTokenProvider,
        _refreshStream = refreshStream ?? _emptyRefreshStream,
        _deviceName = deviceName,
        _appVersion = appVersion;

  // PURPOSE: Default no-op token provider. Returns null until the operator
  //          wires firebase_messaging — see docs/MOBILE-FCM-SETUP.md.
  static Future<String?> _nullTokenProvider() async => null;
  static Stream<String> _emptyRefreshStream() => const Stream.empty();

  // PURPOSE: Read the current device token (if any) and register it with the
  //          backend. Safe to call repeatedly — re-registration is a no-op
  //          when the token hasn't changed since the last call.
  //          Returns true if a token was registered (or was already current),
  //          false if no token is available (no Firebase config / user denied
  //          notification permission) OR the backend call failed.
  Future<bool> register() async {
    final String? token = await _tokenProvider();
    if (token == null || token.isEmpty) {
      // NOTE: This is the steady state today — no Firebase = no token =
      //       silent skip. NOT an error condition.
      return false;
    }
    if (token == _lastRegisteredToken) {
      // Already registered this exact token. Backend upserts on
      // (user_id, device_token) anyway, but skipping saves a network round trip.
      return true;
    }
    final ok = await _push.registerToken(
      token: token,
      platform: _platform,
      deviceName: _deviceName,
      appVersion: _appVersion,
    );
    if (ok) {
      _lastRegisteredToken = token;
    }
    return ok;
  }

  // PURPOSE: Subscribe to token-refresh events so a rotated FCM token is
  //          re-registered without waiting for the next manual `register()`.
  //          Returns a cancel function (caller manages lifecycle).
  void Function() subscribeToTokenRefresh() {
    final sub = _refreshStream().listen((newToken) async {
      if (newToken.isEmpty) return;
      final ok = await _push.registerToken(
        token: newToken,
        platform: _platform,
        deviceName: _deviceName,
        appVersion: _appVersion,
      );
      if (ok) {
        _lastRegisteredToken = newToken;
      }
    });
    return sub.cancel;
  }

  // PURPOSE: Convert an FCM `data` payload into a navigation. Backend pushes
  //          carry at minimum `type` and (for new-mail pushes) `folder` + `uid`.
  //          Unknown / malformed payloads are silently ignored so a bad push
  //          can't crash the app.
  //
  //          Example payload from backend `push_service.rs`:
  //              { "type": "new_mail", "folder": "INBOX", "uid": "12345" }
  //
  //          UID is sent as a string by FCM (data values are always strings)
  //          and parsed back to int here.
  void handleTap(Map<String, dynamic> data) {
    if (data.isEmpty) return;
    final type = data['type']?.toString() ?? 'new_mail';

    switch (type) {
      case 'new_mail':
        final folder = data['folder']?.toString();
        final uidRaw = data['uid']?.toString();
        if (folder == null || folder.isEmpty || uidRaw == null) return;
        final uid = int.tryParse(uidRaw);
        if (uid == null) return;
        _navigator('/message', {'folder': folder, 'uid': uid});
        break;
      case 'test':
        // NOTE: Test pushes from POST /api/push/test have no destination —
        //       just acknowledge the tap by opening the inbox. The home
        //       screen is the auth-state default, so no explicit nav needed.
        break;
      default:
        // Unknown push type — ignore. Forward-compatible with new push
        // categories the backend may add.
        break;
    }
  }
}
