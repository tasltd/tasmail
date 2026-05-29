# Mobile FCM (Firebase Cloud Messaging) Setup Runbook

**Issue:** TMAIL-150 — Implement push notifications via FCM (mobile)
**Audience:** Operator / DevOps — one-time setup; not a Flutter dev task.
**Estimated time:** ~45 minutes (mostly waiting for Firebase Console + Apple
Developer Portal forms to load).

## Why this is a runbook, not a code change

The TASMail mobile app ships with the **full FCM integration code already
written and tested** — see:

- `mobile/lib/services/push_service.dart` — HTTP wrapper for `/api/push/*`
- `mobile/lib/services/fcm_bootstrap.dart` — token registration, tap navigation, token-refresh subscription
- `mobile/lib/providers/auth_provider.dart` — fires `fcmBootstrap.register()` after every successful login / session resume
- `mobile/lib/main.dart` — process-wide `fcmBootstrap` instance + navigator hook

What is **not** in the repo (because it can't be — it's account/credential
material that belongs to the operator):

- `mobile/android/app/google-services.json` (Firebase Android config)
- `mobile/ios/Runner/GoogleService-Info.plist` (Firebase iOS config)
- The APNs auth key uploaded into Firebase Console (iOS only)
- `firebase_messaging` + `firebase_core` packages in `pubspec.yaml`
  *(intentionally absent — adding them without `google-services.json` breaks the Android Gradle build)*

Follow the steps below. The final code swap is **two lines** in `main.dart`.

---

## Step 1 — Create the Firebase project

1. Visit <https://console.firebase.google.com> with the operator Google account.
2. **Add project** → name it `tasmail-prod` (or `tasmail-staging` for a
   staging environment). Disable Google Analytics — we don't use it.
3. Wait for project provisioning to finish (~30 seconds).

## Step 2 — Register the Android app

1. In the project home, click the Android icon (Add app).
2. **Android package name:** `io.techatscale.tasmail_mobile`
   *(must match `mobile/android/app/build.gradle.kts` `applicationId`)*
3. **App nickname:** `TASMail Android`.
4. **Debug signing certificate SHA-1** — optional for now; only needed for
   FCM phone-auth which we don't use.
5. **Download `google-services.json`.**
6. Place it at: `mobile/android/app/google-services.json`
   ⚠ Add it to `.gitignore` — it's environment-specific credential material.

## Step 3 — Register the iOS app *(skip if Android-only build)*

1. Same project home → click the iOS icon.
2. **Bundle ID:** `io.techatscale.tasmail_mobile`
   *(must match `mobile/ios/Runner.xcodeproj` `PRODUCT_BUNDLE_IDENTIFIER`)*
3. **App nickname:** `TASMail iOS`.
4. **Download `GoogleService-Info.plist`.**
5. In Xcode, drag the plist into the `Runner` target (NOT just the folder —
   it must be added to the target's build phase).
6. **APNs key upload** (mandatory for iOS pushes to work):
   - Apple Developer Portal → Keys → `+` → Enable **Apple Push Notifications service (APNs)**.
   - Download the `.p8` file (one-time download — store securely).
   - Note the Key ID and Team ID.
   - Firebase Console → Project Settings → Cloud Messaging → APNs auth key → Upload the `.p8` with the Key ID and Team ID.

## Step 4 — Wire the FlutterFire Gradle plugin (Android)

Edit `mobile/android/build.gradle.kts` (project-level):

```kotlin
plugins {
    // ...existing...
    id("com.google.gms.google-services") version "4.4.2" apply false
}
```

Edit `mobile/android/app/build.gradle.kts` (app-level):

```kotlin
plugins {
    id("com.android.application")
    id("kotlin-android")
    id("dev.flutter.flutter-gradle-plugin")
    // Added: Firebase
    id("com.google.gms.google-services")
}
```

## Step 5 — Add Android notification permission *(API 33+)*

Edit `mobile/android/app/src/main/AndroidManifest.xml`, inside `<manifest>`:

```xml
<!-- Added: TMAIL-150 — Android 13+ runtime notification permission. -->
<uses-permission android:name="android.permission.POST_NOTIFICATIONS"/>
```

The app must request this permission at runtime on first run (see Step 8).

## Step 6 — Install the FlutterFire packages

From `mobile/`:

```bash
# One-time: install the FlutterFire CLI globally
dart pub global activate flutterfire_cli

# Generate lib/firebase_options.dart from the project config
# (reads the credentials from your `firebase` CLI login)
flutterfire configure --project=tasmail-prod

# Install runtime deps
flutter pub add firebase_core firebase_messaging
flutter pub get
```

After this you should have:
- `mobile/lib/firebase_options.dart` (generated — safe to commit, no secrets)
- `mobile/android/app/google-services.json` (gitignore'd)
- `mobile/ios/Runner/GoogleService-Info.plist` (gitignore'd)
- `firebase_core` + `firebase_messaging` in `pubspec.yaml`

## Step 7 — Activate FCM in `main.dart`

Two edits in `mobile/lib/main.dart`:

**(a) Initialize Firebase + register the background handler before `runApp`:**

```dart
import 'package:firebase_core/firebase_core.dart';
import 'package:firebase_messaging/firebase_messaging.dart';
import 'firebase_options.dart';

// PURPOSE: Runs in a separate isolate when a push arrives while the app is
//          terminated / backgrounded. MUST be a top-level function annotated
//          with @pragma('vm:entry-point') — see firebase_messaging docs.
@pragma('vm:entry-point')
Future<void> firebaseMessagingBackgroundHandler(RemoteMessage message) async {
  // Intentionally minimal — full nav is deferred until the user taps the
  // notification (handleTap runs in the foreground isolate then).
}

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await Firebase.initializeApp(options: DefaultFirebaseOptions.currentPlatform);
  FirebaseMessaging.onBackgroundMessage(firebaseMessagingBackgroundHandler);
  runApp(const TasMailApp());
}
```

**(b) Swap the token provider on the existing `fcmBootstrap` declaration:**

```dart
final FcmBootstrap fcmBootstrap = FcmBootstrap(
  navigator: (route, args) {
    final nav = navigatorKey.currentState;
    if (nav == null) return;
    nav.pushNamed(route, arguments: args);
  },
  platform: FcmPlatformId.fcm,
  // Added: real FCM wiring
  tokenProvider: () => FirebaseMessaging.instance.getToken(),
  refreshStream: () => FirebaseMessaging.instance.onTokenRefresh,
);
```

**(c) Wire foreground-tap + cold-start-tap navigation** in `_TasMailAppState.initState`:

```dart
// Foreground tap (notification tray → app already running)
FirebaseMessaging.onMessageOpenedApp.listen((RemoteMessage m) {
  fcmBootstrap.handleTap(m.data);
});
// Cold-start tap (notification opened from killed app)
FirebaseMessaging.instance.getInitialMessage().then((RemoteMessage? m) {
  if (m != null) fcmBootstrap.handleTap(m.data);
});
// Subscribe to token rotations so /push/register stays in sync
fcmBootstrap.subscribeToTokenRefresh();
```

## Step 8 — Request the runtime notification permission (Android 13+)

In `LoginScreen` (or wherever first-run UX lives), after successful login:

```dart
await FirebaseMessaging.instance.requestPermission(
  alert: true,
  badge: true,
  sound: true,
);
```

This is also required on iOS for any push to display.

## Step 9 — Verify end-to-end

1. **Build + install** on a real device (FCM does NOT work in the Android
   emulator without Google Play services — use a physical device or a
   Play-services-enabled AVD):

   ```bash
   cd mobile && flutter run --release
   ```

2. **Log in** through the app. Check the backend logs:

   ```bash
   journalctl --user -u tasmail-backend.service -f | grep -i 'push'
   ```

   You should see `POST /api/push/register 201` followed by an INSERT into
   `push_devices` with `platform = 'fcm'`.

3. **Confirm via API:**

   ```bash
   curl -H "Authorization: Bearer $TOKEN" https://mail.techatscale.io/api/push/devices
   ```

   The response should include the device with the FCM token.

4. **Send a test push:**

   ```bash
   curl -X POST -H "Authorization: Bearer $TOKEN" \
     https://mail.techatscale.io/api/push/test
   ```

   A notification should appear on the device within ~2 seconds. Tap it —
   the app should open to the home screen (test pushes don't navigate per
   `FcmBootstrap.handleTap` switch).

5. **Test new-mail tap navigation:**
   - Send the test device a real email.
   - The backend `push_service` will dispatch a push with payload
     `{type:"new_mail", folder:"INBOX", uid:"<n>"}`.
   - Tap the notification — the app should land on `/message` showing that
     specific email.

## Step 10 — Production hardening

- **APNs production environment:** in Firebase Console → Cloud Messaging →
  iOS app, set "APNs authentication key" to use the **production** APNs server
  (not sandbox) for App Store / TestFlight builds.
- **Quiet hours:** users can configure per-device quiet windows via the
  `/api/push/devices/{id}/quiet-hours` endpoint (already wired in
  `PushService.setQuietHours`). Add a Settings UI when ready.
- **Badge sync:** call `PushService.syncBadgeCount` whenever the inbox unread
  count changes so iOS push badges stay accurate.
- **Token cleanup:** on logout, call
  `pushService.unregisterDevice(deviceId)` to stop pushes to that device.

## Why this code is opt-in instead of always-on

The Android Gradle `com.google.gms.google-services` plugin **fails the build
at configuration time if `google-services.json` is absent.** Shipping
`firebase_messaging` in `pubspec.yaml` without that file would mean nobody
can `flutter run` until they manually obtain Firebase credentials. The
opt-in design lets contributors build the app without Firebase access, and
flips on the moment the operator drops in the credentials and follows this
runbook.

## References

- FlutterFire docs: <https://firebase.flutter.dev/docs/messaging/overview>
- FCM HTTP v1 API: <https://firebase.google.com/docs/cloud-messaging/migrate-v1>
- APNs auth key setup: <https://developer.apple.com/documentation/usernotifications/setting_up_a_remote_notification_server/establishing_a_token-based_connection_to_apns>
- Backend handler reference: `backend/src/handlers/push.rs`
- Backend dispatch service: `backend/src/services/push_service.rs`
- Mobile bootstrap: `mobile/lib/services/fcm_bootstrap.dart`
- Mobile HTTP wrapper: `mobile/lib/services/push_service.dart`
