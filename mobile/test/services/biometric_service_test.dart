// Added: Unit tests for BiometricService (TMAIL-142)
//
// Strategy: we mock the flutter_secure_storage method channel directly so the tests
// stay free of any platform plugin, and we swap in a [FakeBiometricBackend] so the
// local_auth platform side is also test-double rather than real native calls.
//
// Coverage:
//   - capability checks degrade gracefully on PlatformException
//   - enable/disable refuses to enable without a PIN configured
//   - PIN set/verify enforces length + digit rules, hashes salt + PIN with SHA-256
//   - clearLock wipes both PIN and biometric flag
//   - authenticate() honours the full state machine:
//       * not enrolled         → notEnrolled
//       * biometric ok         → biometricSuccess
//       * biometric fail + PIN → pinSuccess / failed
//       * biometric throws     → falls back to PIN
//       * PIN prompt returns null → cancelled

import 'dart:convert';
import 'dart:math';

import 'package:crypto/crypto.dart';
import 'package:flutter/services.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:local_auth/local_auth.dart';
import 'package:tasmail_mobile/services/biometric_service.dart';

class FakeBiometricBackend implements BiometricBackend {
  bool deviceSupported;
  bool canCheck;
  List<BiometricType> available;
  bool authenticateResult;
  Object? authenticateThrows;
  int authenticateCallCount = 0;
  String? lastReason;
  bool? lastBiometricOnly;

  FakeBiometricBackend({
    this.deviceSupported = true,
    this.canCheck = true,
    this.available = const [BiometricType.fingerprint],
    this.authenticateResult = true,
    this.authenticateThrows,
  });

  @override
  Future<bool> isDeviceSupported() async => deviceSupported;

  @override
  Future<bool> canCheckBiometrics() async => canCheck;

  @override
  Future<List<BiometricType>> getAvailableBiometrics() async => available;

  @override
  Future<bool> authenticate({
    required String localizedReason,
    required bool biometricOnly,
  }) async {
    authenticateCallCount++;
    lastReason = localizedReason;
    lastBiometricOnly = biometricOnly;
    if (authenticateThrows != null) {
      throw authenticateThrows!;
    }
    return authenticateResult;
  }
}

// PURPOSE: Install an in-memory backing store for flutter_secure_storage by mocking
//          its method channel. Returns the backing map so tests can inspect / mutate it.
Map<String, String> _installSecureStorageMock() {
  final store = <String, String>{};
  const channel = MethodChannel('plugins.it_nomads.com/flutter_secure_storage');
  TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
      .setMockMethodCallHandler(channel, (MethodCall call) async {
    final args = (call.arguments as Map?) ?? const {};
    final key = args['key'] as String?;
    switch (call.method) {
      case 'write':
        store[key!] = args['value'] as String;
        return null;
      case 'read':
        return store[key];
      case 'delete':
        store.remove(key);
        return null;
      case 'containsKey':
        return store.containsKey(key);
      case 'readAll':
        return Map<String, String>.from(store);
      case 'deleteAll':
        store.clear();
        return null;
      default:
        return null;
    }
  });
  return store;
}

void _uninstallSecureStorageMock() {
  const channel = MethodChannel('plugins.it_nomads.com/flutter_secure_storage');
  TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
      .setMockMethodCallHandler(channel, null);
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  late Map<String, String> storage;
  late FakeBiometricBackend backend;
  late BiometricService service;

  setUp(() {
    storage = _installSecureStorageMock();
    backend = FakeBiometricBackend();
    // Seeded random keeps generated salts deterministic across runs so the salt
    // assertion below is reproducible.
    service = BiometricService(
      storage: const FlutterSecureStorage(),
      backend: backend,
      random: Random(0),
    );
  });

  tearDown(_uninstallSecureStorageMock);

  group('capability checks', () {
    test('reports device support from backend', () async {
      backend.deviceSupported = true;
      expect(await service.isDeviceSupported(), isTrue);
      backend.deviceSupported = false;
      expect(await service.isDeviceSupported(), isFalse);
    });

    test('canCheckBiometrics returns false on PlatformException', () async {
      backend = FakeBiometricBackend(
        canCheck: true,
        authenticateThrows: PlatformException(code: 'NotAvailable'),
      );
      // Use a tailored fake that throws on canCheckBiometrics specifically.
      final throwing = _ThrowingBackend();
      service = BiometricService(backend: throwing, random: Random(0));
      expect(await service.canCheckBiometrics(), isFalse);
      expect(await service.isDeviceSupported(), isFalse);
      expect(await service.getAvailableBiometrics(), isEmpty);
    });

    test('forwards available biometric types', () async {
      backend.available = const [BiometricType.face, BiometricType.fingerprint];
      final result = await service.getAvailableBiometrics();
      expect(result, containsAll(<BiometricType>[
        BiometricType.face,
        BiometricType.fingerprint,
      ]));
    });
  });

  group('PIN management', () {
    test('rejects PIN shorter than minimum', () async {
      expect(
        () => service.setPin('12'),
        throwsA(isA<ArgumentError>()),
      );
    });

    test('rejects PIN longer than maximum', () async {
      expect(
        () => service.setPin('1' * (BiometricService.kMaxPinLength + 1)),
        throwsA(isA<ArgumentError>()),
      );
    });

    test('rejects non-digit PIN', () async {
      expect(
        () => service.setPin('12ab'),
        throwsA(isA<ArgumentError>()),
      );
    });

    test('setPin stores salted SHA-256 (never the plaintext)', () async {
      await service.setPin('1234');
      expect(storage[BiometricService.kPinHashKey], isNotNull);
      expect(storage[BiometricService.kPinSaltKey], isNotNull);
      expect(storage.values, isNot(contains('1234')),
          reason: 'plaintext PIN must never be persisted');

      // Recompute the hash from the stored salt and confirm it matches what
      // verifyPin would compare against. This proves the storage format.
      final salt = storage[BiometricService.kPinSaltKey]!;
      final expected =
          sha256.convert(utf8.encode('$salt:1234')).toString();
      expect(storage[BiometricService.kPinHashKey], expected);
    });

    test('verifyPin returns true for correct PIN and false for wrong', () async {
      await service.setPin('246810');
      expect(await service.verifyPin('246810'), isTrue);
      expect(await service.verifyPin('000000'), isFalse);
    });

    test('verifyPin returns false when no PIN is set', () async {
      expect(await service.verifyPin('1234'), isFalse);
    });

    test('hasPin reflects storage state', () async {
      expect(await service.hasPin(), isFalse);
      await service.setPin('9876');
      expect(await service.hasPin(), isTrue);
    });

    test('clearLock wipes PIN and disables biometric', () async {
      await service.setPin('1234');
      await service.setBiometricEnabled(true);
      expect(await service.isBiometricEnabled(), isTrue);

      await service.clearLock();

      expect(await service.hasPin(), isFalse);
      expect(await service.isBiometricEnabled(), isFalse);
      expect(storage[BiometricService.kPinHashKey], isNull);
      expect(storage[BiometricService.kPinSaltKey], isNull);
    });

    test('two different PINs hash differently because the salts differ',
        () async {
      await service.setPin('1234');
      final firstHash = storage[BiometricService.kPinHashKey];
      final firstSalt = storage[BiometricService.kPinSaltKey];

      // Reseed the random with a different value to force a new salt.
      service = BiometricService(backend: backend, random: Random(99));
      await service.setPin('1234');

      expect(storage[BiometricService.kPinHashKey], isNot(firstHash),
          reason: 'salt changes ⇒ hash for same PIN must differ');
      expect(storage[BiometricService.kPinSaltKey], isNot(firstSalt));
    });
  });

  group('biometric enable/disable', () {
    test('refuses to enable without PIN configured', () async {
      expect(
        () => service.setBiometricEnabled(true),
        throwsA(isA<StateError>()),
      );
      expect(await service.isBiometricEnabled(), isFalse);
    });

    test('allows enabling once PIN is set', () async {
      await service.setPin('1234');
      await service.setBiometricEnabled(true);
      expect(await service.isBiometricEnabled(), isTrue);
      expect(storage[BiometricService.kBiometricEnabledKey], 'true');
    });

    test('allows disabling at any time', () async {
      await service.setPin('1234');
      await service.setBiometricEnabled(true);
      await service.setBiometricEnabled(false);
      expect(await service.isBiometricEnabled(), isFalse);
    });
  });

  group('authenticate() flow', () {
    test('returns notEnrolled when neither biometric nor PIN configured',
        () async {
      backend.deviceSupported = true;
      backend.canCheck = true;
      final result = await service.authenticate(
        pinPrompt: () async => '0000',
      );
      expect(result, BiometricAuthResult.notEnrolled);
      expect(backend.authenticateCallCount, 0);
    });

    test('returns biometricSuccess when local_auth succeeds', () async {
      await service.setPin('1234');
      await service.setBiometricEnabled(true);
      backend.authenticateResult = true;

      final result = await service.authenticate(
        pinPrompt: () async => fail('PIN must not be asked for on biometric ok'),
      );

      expect(result, BiometricAuthResult.biometricSuccess);
      expect(backend.authenticateCallCount, 1);
      // We always pass biometricOnly=false so the OS may fall back to device
      // credential; our own PIN is the *final* fallback.
      expect(backend.lastBiometricOnly, isFalse);
    });

    test(
        'falls back to PIN when biometric fails and returns pinSuccess on correct PIN',
        () async {
      await service.setPin('1234');
      await service.setBiometricEnabled(true);
      backend.authenticateResult = false;

      final result = await service.authenticate(
        pinPrompt: () async => '1234',
      );

      expect(result, BiometricAuthResult.pinSuccess);
      expect(backend.authenticateCallCount, 1);
    });

    test('falls back to PIN when local_auth throws PlatformException',
        () async {
      await service.setPin('1234');
      await service.setBiometricEnabled(true);
      backend.authenticateThrows =
          PlatformException(code: 'TooManyAttempts');

      final result = await service.authenticate(
        pinPrompt: () async => '1234',
      );

      expect(result, BiometricAuthResult.pinSuccess);
    });

    test('returns failed when PIN is wrong', () async {
      await service.setPin('1234');
      await service.setBiometricEnabled(true);
      backend.authenticateResult = false;

      final result = await service.authenticate(
        pinPrompt: () async => '9999',
      );

      expect(result, BiometricAuthResult.failed);
    });

    test('returns cancelled when user dismisses PIN prompt', () async {
      await service.setPin('1234');
      await service.setBiometricEnabled(true);
      backend.authenticateResult = false;

      final result = await service.authenticate(pinPrompt: () async => null);
      expect(result, BiometricAuthResult.cancelled);
    });

    test('skips biometric and uses PIN when biometric toggle is off', () async {
      await service.setPin('1234');
      // biometric stays disabled
      final result = await service.authenticate(pinPrompt: () async => '1234');
      expect(result, BiometricAuthResult.pinSuccess);
      expect(backend.authenticateCallCount, 0,
          reason: 'must not call local_auth when biometric toggle is off');
    });

    test('returns failed when biometric is disabled, PIN exists, '
        'but no pinPrompt is provided', () async {
      await service.setPin('1234');
      final result = await service.authenticate();
      expect(result, BiometricAuthResult.failed);
    });
  });
}

// PURPOSE: Separate fake to verify that thrown exceptions during capability checks
//          are swallowed (degrade-gracefully contract).
class _ThrowingBackend implements BiometricBackend {
  @override
  Future<bool> isDeviceSupported() => throw PlatformException(code: 'x');
  @override
  Future<bool> canCheckBiometrics() => throw PlatformException(code: 'x');
  @override
  Future<List<BiometricType>> getAvailableBiometrics() =>
      throw PlatformException(code: 'x');
  @override
  Future<bool> authenticate({
    required String localizedReason,
    required bool biometricOnly,
  }) =>
      throw PlatformException(code: 'x');
}
