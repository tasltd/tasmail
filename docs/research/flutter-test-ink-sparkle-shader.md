# Flutter Widget Test Failure: `ink_sparkle.frag` Shader Incompatibility

## Context

While verifying TMAIL-141 (mobile login screen), running `flutter test test/screens/login_screen_test.dart`
against Flutter 3.44.0 (snap-installed, Dart 3.12.0) produced this failure on the second test:

```
Exception: Asset 'shaders/ink_sparkle.frag' manifest could not be decoded: INVALID_ARGUMENT:
Unsupported runtime stages format version. Expected 2, got 1.

  #0  new FragmentProgram._fromAsset (dart:ui/painting.dart:5433:7)
  #1  FragmentProgram.fromAsset.<anonymous closure> (dart:ui/painting.dart:5461:39)
```

The failing test was `shows validation errors for empty fields`, which is the first test that
taps a `FilledButton`. Material 3's `FilledButton` defaults to `InkSparkle` for its splash
factory, and `InkSparkle.create` triggers `FragmentProgram.fromAsset('shaders/ink_sparkle.frag')`.
The bundled shader format in the SDK is older than what the runtime expects, so the asset
fails to decode in the test environment.

## Root cause

- Material 3 sets `splashFactory: InkSparkle.splashFactory` on most filled buttons.
- `InkSparkle` lazily loads `shaders/ink_sparkle.frag` from the Flutter SDK on first ripple.
- Flutter 3.44.0 ships a `runtime stages` v1 shader, but the SDK's runtime expects v2.
- Tests pass on whichever runs first (before the asset is touched) and fail thereafter.

## Decision

The fix lives in the **test**, not in production code. We swap the splash factory in the
test-only `MaterialApp` to `NoSplash.splashFactory`. This avoids loading the shader entirely
without changing user-visible behavior in the app itself.

```dart
return ChangeNotifierProvider(
  create: (_) => AuthProvider(),
  child: MaterialApp(
    theme: ThemeData(splashFactory: NoSplash.splashFactory),
    home: const LoginScreen(),
  ),
);
```

This is the documented workaround in flutter/flutter#104084 and applies whenever a Material 3
button is tapped inside a widget test.

## Sources

- [flutter/flutter#143806 — Asset 'shaders/ink_sparkle.frag' does not contain appropriate runtime stage data](https://github.com/flutter/flutter/issues/143806)
- [flutter/flutter#104084 — Tests fail with 'Unable to load asset: shaders/ink_sparkle.frag' when useMaterial3 is true](https://github.com/flutter/flutter/issues/104084)
- [flutter/flutter#133325 — InkSparkle has too many bindings on iOS simulator](https://github.com/flutter/flutter/issues/133325)
- [flutter/flutter#157886 — ShaderCompilerException could not write ink_sparkle.frag](https://github.com/flutter/flutter/issues/157886)

## Key facts extracted from sources

- Issue #104084 explicitly recommends `splashFactory: NoSplash.splashFactory` for widget tests
  that exercise Material 3 buttons. The same pattern is repeated across reproducers in
  #143806 and #133325.
- The InkSparkle splash is purely visual — disabling it in tests does NOT change the result
  of tap callbacks, form validation, or any state assertions.
- Production code should keep the default Material 3 splash. Only the test harness needs the
  override.
