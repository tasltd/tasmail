// Added: Theme mode provider for TMAIL-152 (settings: light/dark/system)
// PURPOSE: Notify MaterialApp when the user flips the theme so the change is
//          immediate without a restart. Persistence is delegated to
//          [ThemePreference] so this class stays trivially unit-testable.

import 'package:flutter/material.dart';
import '../services/theme_preference.dart';

class ThemeProvider extends ChangeNotifier {
  final ThemePreference _pref;

  ThemeMode _mode = ThemeMode.system;
  bool _loaded = false;

  ThemeProvider({ThemePreference? pref})
      : _pref = pref ?? ThemePreference();

  ThemeMode get mode => _mode;
  bool get loaded => _loaded;

  Future<void> load() async {
    _mode = await _pref.read();
    _loaded = true;
    notifyListeners();
  }

  Future<void> setMode(ThemeMode mode) async {
    if (_mode == mode) return;
    _mode = mode;
    notifyListeners();
    await _pref.write(mode);
  }
}
