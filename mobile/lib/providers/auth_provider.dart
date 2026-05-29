// Added: Authentication state provider for TMAIL-141
// Changed: TMAIL-150 — accept an optional `onAuthenticated` hook so the FCM
//          bootstrap can register the device token after every successful
//          login + cold-start session resume. Hook stays optional so existing
//          tests don't need updating.
// PURPOSE: Manages login/logout state, token persistence, and user info
// EXTERNAL: Uses ApiClient for HTTP calls, FlutterSecureStorage for token persistence

import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import '../api/api_client.dart';
import '../models/auth.dart';

// PURPOSE: Callback fired after a successful login or session-resume. Used by
//          main.dart to kick off `FcmBootstrap.register()` so the backend
//          knows where to deliver pushes for this device.
typedef AuthenticatedCallback = Future<void> Function();

class AuthProvider extends ChangeNotifier {
  final ApiClient _api = ApiClient();
  final FlutterSecureStorage _storage = const FlutterSecureStorage();

  // Added: TMAIL-150 — optional post-auth hook. Failures are swallowed so a
  //        broken push registration can never block the user from using the
  //        app.
  final AuthenticatedCallback? _onAuthenticated;

  bool _isAuthenticated = false;
  // Changed: Start as false, only set true during checkAuth/login
  bool _isLoading = false;
  UserInfo? _user;
  String? _error;

  AuthProvider({AuthenticatedCallback? onAuthenticated})
      : _onAuthenticated = onAuthenticated;

  bool get isAuthenticated => _isAuthenticated;
  bool get isLoading => _isLoading;
  UserInfo? get user => _user;
  String? get error => _error;

  // PURPOSE: Check for existing session on app startup
  Future<void> checkAuth() async {
    _isLoading = true;
    notifyListeners();

    try {
      final hasTokens = await _api.hasTokens();
      if (hasTokens) {
        // NOTE: Validate token by fetching user profile
        final userJson = await _storage.read(key: 'user_info');
        if (userJson != null) {
          _user = UserInfo.fromJson(
            json.decode(userJson) as Map<String, dynamic>,
          );
          _isAuthenticated = true;
        }
      }
    } catch (_) {
      _isAuthenticated = false;
    } finally {
      _isLoading = false;
      notifyListeners();
      // Added: TMAIL-150 — fire post-auth hook AFTER notifying listeners so
      //        the UI can route off the splash screen first. Errors here are
      //        non-fatal (push registration is best-effort).
      if (_isAuthenticated) {
        await _fireAuthenticated();
      }
    }
  }

  // PURPOSE: TMAIL-150 — invoke the optional post-auth hook. Swallows errors
  //          so a flaky FCM token fetch can't lock the user out of the app.
  Future<void> _fireAuthenticated() async {
    final hook = _onAuthenticated;
    if (hook == null) return;
    try {
      await hook();
    } catch (_) {
      // Intentionally silent — push registration is non-critical.
    }
  }

  // PURPOSE: Login with email and password
  Future<bool> login(String email, String password) async {
    _error = null;
    _isLoading = true;
    notifyListeners();

    try {
      final response = await _api.post(
        '/auth/login',
        data: LoginRequest(email: email, password: password).toJson(),
      );

      if (response.statusCode == 200) {
        final loginResponse = LoginResponse.fromJson(
          response.data as Map<String, dynamic>,
        );

        await _api.saveTokens(
          accessToken: loginResponse.accessToken,
          refreshToken: loginResponse.refreshToken,
        );

        // Added: Persist user info for offline access
        await _storage.write(
          key: 'user_info',
          value: json.encode(loginResponse.user.toJson()),
        );

        _user = loginResponse.user;
        _isAuthenticated = true;
        _isLoading = false;
        notifyListeners();
        // Added: TMAIL-150 — register device for push immediately after the
        //        access token is on disk so /push/register auth succeeds.
        await _fireAuthenticated();
        return true;
      }

      _error = 'Invalid credentials';
      _isLoading = false;
      notifyListeners();
      return false;
    } catch (e) {
      _error = _extractError(e);
      _isLoading = false;
      notifyListeners();
      return false;
    }
  }

  // PURPOSE: Logout and clear all stored data
  Future<void> logout() async {
    await _api.clearTokens();
    await _storage.delete(key: 'user_info');
    _isAuthenticated = false;
    _user = null;
    _error = null;
    notifyListeners();
  }

  // Added: Extract readable error message from exceptions
  String _extractError(dynamic e) {
    if (e is Exception) {
      final msg = e.toString();
      if (msg.contains('SocketException') || msg.contains('Connection refused')) {
        return 'Cannot connect to server. Check your network.';
      }
      if (msg.contains('401')) {
        return 'Invalid email or password.';
      }
    }
    return 'Login failed. Please try again.';
  }
}
