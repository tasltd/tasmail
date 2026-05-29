// Added: Persists user-configured inbox swipe actions for TMAIL-148
// PURPOSE: Reads/writes SwipePreferences via flutter_secure_storage so the
//          choice survives app restarts. Secure storage is overkill from a
//          confidentiality standpoint, but the project already uses it for
//          biometric prefs — staying on one backend keeps the dependency
//          surface small.
// EXTERNAL: Uses flutter_secure_storage (Keystore / Keychain).
// NOTE: Constructor takes an optional FlutterSecureStorage so unit tests can
//       inject a fake without going through the platform channel.

import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';

import 'swipe_preferences.dart';

class SwipeActionsService extends ChangeNotifier {
  // PURPOSE: Storage key constants are public so tests / future migrations can
  //          reference them without re-deriving strings.
  static const String kStorageKey = 'swipe_actions_v1';

  final FlutterSecureStorage _storage;

  SwipePreferences _preferences = const SwipePreferences();
  bool _loaded = false;

  SwipeActionsService({FlutterSecureStorage? storage})
      : _storage = storage ?? const FlutterSecureStorage();

  // PURPOSE: Current value — callers read this synchronously. While the load
  //          is in flight (between construction and the first load() resolve)
  //          this returns the defaults, which matches the old hardcoded
  //          behaviour and avoids a flash of "empty" prefs.
  SwipePreferences get preferences => _preferences;

  bool get isLoaded => _loaded;

  // PURPOSE: Load persisted prefs from disk. Safe to call repeatedly; a corrupt
  //          payload falls back to defaults rather than crashing the inbox.
  Future<SwipePreferences> load() async {
    try {
      final raw = await _storage.read(key: kStorageKey);
      if (raw != null && raw.isNotEmpty) {
        final decoded = jsonDecode(raw);
        if (decoded is Map<String, dynamic>) {
          _preferences = SwipePreferences.fromJson(decoded);
        }
      }
    } catch (_) {
      // NOTE: Corrupt JSON or a platform-channel failure both fall through to
      // defaults — the user can re-pick from settings.
      _preferences = const SwipePreferences();
    }
    _loaded = true;
    notifyListeners();
    return _preferences;
  }

  // PURPOSE: Persist a new preference object and notify listeners. Returns the
  //          freshly-stored value so the caller doesn't need a separate read.
  Future<SwipePreferences> save(SwipePreferences next) async {
    _preferences = next;
    await _storage.write(
      key: kStorageKey,
      value: jsonEncode(next.toJson()),
    );
    _loaded = true;
    notifyListeners();
    return _preferences;
  }

  // PURPOSE: Convenience helpers used by the settings screen so a single picker
  //          tap can update one side without re-constructing the whole object.
  Future<SwipePreferences> setRight(SwipeAction action) =>
      save(_preferences.copyWith(rightAction: action));

  Future<SwipePreferences> setLeft(SwipeAction action) =>
      save(_preferences.copyWith(leftAction: action));

  // PURPOSE: Reset to factory defaults — handy for tests and a "Restore"
  //          button in the settings UI.
  Future<SwipePreferences> reset() => save(const SwipePreferences());
}
