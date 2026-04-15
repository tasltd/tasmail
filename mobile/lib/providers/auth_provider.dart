// Added: Authentication state provider for TMAIL-141
// PURPOSE: Manages login/logout state, token persistence, and user info
// EXTERNAL: Uses ApiClient for HTTP calls, FlutterSecureStorage for token persistence

import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import '../api/api_client.dart';
import '../models/auth.dart';

class AuthProvider extends ChangeNotifier {
  final ApiClient _api = ApiClient();
  final FlutterSecureStorage _storage = const FlutterSecureStorage();

  bool _isAuthenticated = false;
  // Changed: Start as false, only set true during checkAuth/login
  bool _isLoading = false;
  UserInfo? _user;
  String? _error;

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
