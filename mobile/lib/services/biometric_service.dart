// Added: Biometric authentication service for TMAIL-142
// PURPOSE: Wraps local_auth for fingerprint/face ID unlock with fallback to PIN
// EXTERNAL: Uses local_auth package (needs platform setup for Android/iOS)
// NOTE: local_auth dependency should be added: flutter pub add local_auth

import 'package:flutter_secure_storage/flutter_secure_storage.dart';

class BiometricService {
  final FlutterSecureStorage _storage = const FlutterSecureStorage();

  static const _biometricEnabledKey = 'biometric_enabled';

  // PURPOSE: Check if biometric is enabled by user
  Future<bool> isBiometricEnabled() async {
    final value = await _storage.read(key: _biometricEnabledKey);
    return value == 'true';
  }

  // PURPOSE: Enable or disable biometric auth
  Future<void> setBiometricEnabled(bool enabled) async {
    await _storage.write(
      key: _biometricEnabledKey,
      value: enabled.toString(),
    );
  }

  // PURPOSE: Check if device supports biometric authentication
  // NOTE: Actual implementation requires local_auth package
  Future<bool> isDeviceSupported() async {
    // NOTE: Placeholder — real implementation uses local_auth
    // final localAuth = LocalAuthentication();
    // return await localAuth.isDeviceSupported();
    return true;
  }

  // PURPOSE: Authenticate using biometric
  // NOTE: Actual implementation requires local_auth package
  Future<bool> authenticate({String reason = 'Authenticate to access TASMail'}) async {
    // NOTE: Placeholder — real implementation uses local_auth
    // final localAuth = LocalAuthentication();
    // return await localAuth.authenticate(
    //   localizedReason: reason,
    //   options: const AuthenticationOptions(
    //     stickyAuth: true,
    //     biometricOnly: false, // Allow PIN fallback
    //   ),
    // );
    return false;
  }
}
