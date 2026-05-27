// Added: Biometric authentication service for TMAIL-142
// PURPOSE: Wraps local_auth for fingerprint/face unlock with a salted-SHA-256 PIN
//          fallback. Both the enabled flag and the PIN hash live in flutter_secure_storage
//          (Keystore on Android, Keychain on iOS), so nothing sensitive is ever written
//          to plain SharedPreferences.
// EXTERNAL: Uses local_auth (platform biometrics) and crypto (PIN hashing).
// NOTE: Backend hooks (LocalAuthentication + secure storage) are injected via the
//       constructor so the unit tests can swap in fakes without touching platform
//       channels. The default constructor wires real implementations.

import 'dart:convert';
import 'dart:math';

import 'package:crypto/crypto.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:local_auth/local_auth.dart';

// PURPOSE: Outcome of an authenticate() call. The screen can branch on these without
//          having to know whether the OS actually fell back to PIN itself.
enum BiometricAuthResult {
  // OS biometric (fingerprint / Face ID) succeeded.
  biometricSuccess,
  // PIN fallback verified by us (user typed the right PIN).
  pinSuccess,
  // User cancelled at any step.
  cancelled,
  // No biometric hardware AND no PIN configured — caller should send the user
  // back to the lock-setup screen.
  notEnrolled,
  // Biometric attempt failed and PIN was wrong / not provided.
  failed,
  // Platform error (channel exception, missing plugin, etc.).
  error,
}

// PURPOSE: Thin abstraction over local_auth so the unit tests don't have to mock the
//          Flutter platform channel. The default impl just forwards to LocalAuthentication.
abstract class BiometricBackend {
  Future<bool> isDeviceSupported();
  Future<bool> canCheckBiometrics();
  Future<List<BiometricType>> getAvailableBiometrics();
  Future<bool> authenticate({
    required String localizedReason,
    required bool biometricOnly,
  });
}

class _RealBiometricBackend implements BiometricBackend {
  final LocalAuthentication _auth = LocalAuthentication();

  @override
  Future<bool> isDeviceSupported() => _auth.isDeviceSupported();

  @override
  Future<bool> canCheckBiometrics() => _auth.canCheckBiometrics;

  @override
  Future<List<BiometricType>> getAvailableBiometrics() =>
      _auth.getAvailableBiometrics();

  @override
  Future<bool> authenticate({
    required String localizedReason,
    required bool biometricOnly,
  }) {
    return _auth.authenticate(
      localizedReason: localizedReason,
      options: AuthenticationOptions(
        stickyAuth: true,
        biometricOnly: biometricOnly,
      ),
    );
  }
}

class BiometricService {
  // PURPOSE: Storage keys live as static constants so the settings screen can also
  //          observe them (e.g. for migration) without re-deriving the strings.
  static const String kBiometricEnabledKey = 'biometric_enabled';
  static const String kPinHashKey = 'biometric_pin_hash';
  static const String kPinSaltKey = 'biometric_pin_salt';
  // PURPOSE: At least 4 digits is the Android Keyguard minimum; we mirror it.
  static const int kMinPinLength = 4;
  static const int kMaxPinLength = 12;

  final FlutterSecureStorage _storage;
  final BiometricBackend _backend;
  // PURPOSE: Seeded random for tests; production picks fresh randomness.
  final Random _random;

  BiometricService({
    FlutterSecureStorage? storage,
    BiometricBackend? backend,
    Random? random,
  })  : _storage = storage ?? const FlutterSecureStorage(),
        _backend = backend ?? _RealBiometricBackend(),
        _random = random ?? Random.secure();

  // ---------------------------------------------------------------------------
  // Capability checks
  // ---------------------------------------------------------------------------

  // PURPOSE: True if the OS reports any biometric hardware (fingerprint, face, iris).
  //          Settings UI uses this to decide whether to show the toggle at all.
  Future<bool> isDeviceSupported() async {
    try {
      return await _backend.isDeviceSupported();
    } catch (_) {
      return false;
    }
  }

  // PURPOSE: True if biometrics can be checked RIGHT NOW (enrolled + not locked out).
  Future<bool> canCheckBiometrics() async {
    try {
      return await _backend.canCheckBiometrics();
    } catch (_) {
      return false;
    }
  }

  // PURPOSE: List of available biometrics so the UI can label the toggle as
  //          "Use Face ID" vs "Use Fingerprint" appropriately.
  Future<List<BiometricType>> getAvailableBiometrics() async {
    try {
      return await _backend.getAvailableBiometrics();
    } catch (_) {
      return const <BiometricType>[];
    }
  }

  // ---------------------------------------------------------------------------
  // Enable / disable
  // ---------------------------------------------------------------------------

  Future<bool> isBiometricEnabled() async {
    final value = await _storage.read(key: kBiometricEnabledKey);
    return value == 'true';
  }

  // PURPOSE: Enable or disable biometric unlock. We refuse to enable without a PIN
  //          configured so the user can never lock themselves out of the app when
  //          the OS later drops the enrolment (e.g. they switch fingers).
  Future<void> setBiometricEnabled(bool enabled) async {
    if (enabled && !await hasPin()) {
      throw StateError(
        'Cannot enable biometric lock without a PIN fallback configured',
      );
    }
    await _storage.write(
      key: kBiometricEnabledKey,
      value: enabled ? 'true' : 'false',
    );
  }

  // ---------------------------------------------------------------------------
  // PIN fallback
  // ---------------------------------------------------------------------------

  Future<bool> hasPin() async {
    final hash = await _storage.read(key: kPinHashKey);
    final salt = await _storage.read(key: kPinSaltKey);
    return hash != null && hash.isNotEmpty && salt != null && salt.isNotEmpty;
  }

  // PURPOSE: Persist a fresh PIN. We store SHA-256(salt + pin), never the PIN itself.
  Future<void> setPin(String pin) async {
    if (pin.length < kMinPinLength || pin.length > kMaxPinLength) {
      throw ArgumentError(
        'PIN must be between $kMinPinLength and $kMaxPinLength characters',
      );
    }
    if (!RegExp(r'^\d+$').hasMatch(pin)) {
      throw ArgumentError('PIN must contain digits only');
    }
    final salt = _generateSalt();
    final hash = _hashPin(pin: pin, salt: salt);
    await _storage.write(key: kPinSaltKey, value: salt);
    await _storage.write(key: kPinHashKey, value: hash);
  }

  // PURPOSE: Verify a typed PIN against the stored hash.
  Future<bool> verifyPin(String pin) async {
    final salt = await _storage.read(key: kPinSaltKey);
    final hash = await _storage.read(key: kPinHashKey);
    if (salt == null || hash == null) return false;
    final candidate = _hashPin(pin: pin, salt: salt);
    return _constantTimeEquals(candidate, hash);
  }

  // PURPOSE: Wipe PIN + disable biometric. Used from settings "Remove Lock".
  Future<void> clearLock() async {
    await _storage.delete(key: kPinHashKey);
    await _storage.delete(key: kPinSaltKey);
    await _storage.write(key: kBiometricEnabledKey, value: 'false');
  }

  // ---------------------------------------------------------------------------
  // Authentication flow
  // ---------------------------------------------------------------------------

  // PURPOSE: Run the full unlock flow. If biometric is enabled AND currently usable,
  //          prompt for it first; otherwise fall back to PIN via [pinPrompt]. The
  //          caller supplies the PIN prompt as a closure so this service stays
  //          UI-free and remains unit-testable.
  Future<BiometricAuthResult> authenticate({
    String reason = 'Authenticate to access TASMail',
    Future<String?> Function()? pinPrompt,
  }) async {
    final hasPinSet = await hasPin();
    final biometricEnabled = await isBiometricEnabled();
    final canBiometric =
        biometricEnabled && await isDeviceSupported() && await canCheckBiometrics();

    if (!hasPinSet && !canBiometric) {
      return BiometricAuthResult.notEnrolled;
    }

    if (canBiometric) {
      try {
        final ok = await _backend.authenticate(
          localizedReason: reason,
          // Allow OS-level device-credential fallback (system PIN / pattern). We
          // still run our own PIN check below if the OS-level attempt fails or the
          // user cancels.
          biometricOnly: false,
        );
        if (ok) return BiometricAuthResult.biometricSuccess;
      } catch (_) {
        // Fall through to PIN — a thrown PlatformException means the OS prompt
        // failed (e.g. too many attempts). PIN is still a valid recovery path.
      }
    }

    if (hasPinSet) {
      if (pinPrompt == null) return BiometricAuthResult.failed;
      final entered = await pinPrompt();
      if (entered == null) return BiometricAuthResult.cancelled;
      final ok = await verifyPin(entered);
      return ok ? BiometricAuthResult.pinSuccess : BiometricAuthResult.failed;
    }

    return BiometricAuthResult.failed;
  }

  // ---------------------------------------------------------------------------
  // Internals
  // ---------------------------------------------------------------------------

  String _generateSalt() {
    final bytes = List<int>.generate(16, (_) => _random.nextInt(256));
    return base64UrlEncode(bytes);
  }

  String _hashPin({required String pin, required String salt}) {
    final material = utf8.encode('$salt:$pin');
    return sha256.convert(material).toString();
  }

  // PURPOSE: Compare two hex digests in constant time so attackers can't time us.
  bool _constantTimeEquals(String a, String b) {
    if (a.length != b.length) return false;
    var diff = 0;
    for (var i = 0; i < a.length; i++) {
      diff |= a.codeUnitAt(i) ^ b.codeUnitAt(i);
    }
    return diff == 0;
  }
}
