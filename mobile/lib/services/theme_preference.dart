// Added: Persisted theme-mode preference for TMAIL-152
// PURPOSE: Encapsulate the user-chosen light/dark/system preference so the
//          ThemeProvider stays a thin ChangeNotifier and the storage backend
//          can be swapped (test fakes, future migration off secure-storage).
// NOTE: We store the preference in FlutterSecureStorage to match the rest of
//       the mobile app — there's no plain SharedPreferences dependency wired
//       up yet and the value is tiny so the encryption cost is negligible.

import 'package:flutter/material.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';

class ThemePreference {
  static const String _key = 'theme_mode';

  final FlutterSecureStorage _storage;

  ThemePreference({FlutterSecureStorage? storage})
      : _storage = storage ?? const FlutterSecureStorage();

  Future<ThemeMode> read() async {
    final raw = await _storage.read(key: _key);
    return _decode(raw);
  }

  Future<void> write(ThemeMode mode) async {
    await _storage.write(key: _key, value: _encode(mode));
  }

  static String _encode(ThemeMode mode) => switch (mode) {
        ThemeMode.light => 'light',
        ThemeMode.dark => 'dark',
        ThemeMode.system => 'system',
      };

  static ThemeMode _decode(String? raw) => switch (raw) {
        'light' => ThemeMode.light,
        'dark' => ThemeMode.dark,
        _ => ThemeMode.system,
      };
}
