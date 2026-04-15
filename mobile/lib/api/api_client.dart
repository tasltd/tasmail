// Added: Core API client with JWT auth and auto-refresh for TMAIL-140
// PURPOSE: Dio-based HTTP client that handles Bearer token injection,
//          401 auto-refresh via /api/auth/refresh, and secure token storage
// EXTERNAL: Connects to TASMail backend (default http://10.0.2.2:3000 for Android emulator)

import 'package:dio/dio.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';

class ApiClient {
  // NOTE: 10.0.2.2 maps to host machine localhost from Android emulator
  static const String _defaultBaseUrl = 'http://10.0.2.2:3000/api';

  late final Dio _dio;
  final FlutterSecureStorage _storage = const FlutterSecureStorage();

  // Added: Singleton pattern for consistent state across the app
  static final ApiClient _instance = ApiClient._internal();
  factory ApiClient() => _instance;

  ApiClient._internal() {
    _dio = Dio(BaseOptions(
      baseUrl: _defaultBaseUrl,
      connectTimeout: const Duration(seconds: 10),
      receiveTimeout: const Duration(seconds: 30),
      headers: {'Content-Type': 'application/json'},
    ));

    // Added: Request interceptor to inject Bearer token
    _dio.interceptors.add(InterceptorsWrapper(
      onRequest: (options, handler) async {
        final token = await _storage.read(key: 'access_token');
        if (token != null) {
          options.headers['Authorization'] = 'Bearer $token';
        }
        return handler.next(options);
      },
      onError: (error, handler) async {
        // Added: Auto-refresh on 401 Unauthorized
        if (error.response?.statusCode == 401) {
          final refreshed = await _tryRefreshToken();
          if (refreshed) {
            // NOTE: Retry the original request with new token
            final token = await _storage.read(key: 'access_token');
            error.requestOptions.headers['Authorization'] = 'Bearer $token';
            final retryResponse = await _dio.fetch(error.requestOptions);
            return handler.resolve(retryResponse);
          }
        }
        return handler.next(error);
      },
    ));
  }

  // PURPOSE: Configure base URL (e.g., for different environments)
  void setBaseUrl(String url) {
    _dio.options.baseUrl = url;
  }

  // PURPOSE: Attempt token refresh using stored refresh token
  Future<bool> _tryRefreshToken() async {
    try {
      final refreshToken = await _storage.read(key: 'refresh_token');
      if (refreshToken == null) return false;

      // NOTE: Use a separate Dio instance to avoid interceptor loops
      final refreshDio = Dio(BaseOptions(baseUrl: _dio.options.baseUrl));
      final response = await refreshDio.post(
        '/auth/refresh',
        data: {'refresh_token': refreshToken},
      );

      if (response.statusCode == 200) {
        await _storage.write(
          key: 'access_token',
          value: response.data['access_token'],
        );
        if (response.data['refresh_token'] != null) {
          await _storage.write(
            key: 'refresh_token',
            value: response.data['refresh_token'],
          );
        }
        return true;
      }
      return false;
    } catch (_) {
      return false;
    }
  }

  // PURPOSE: Store tokens after successful login
  Future<void> saveTokens({
    required String accessToken,
    required String refreshToken,
  }) async {
    await _storage.write(key: 'access_token', value: accessToken);
    await _storage.write(key: 'refresh_token', value: refreshToken);
  }

  // PURPOSE: Clear tokens on logout
  Future<void> clearTokens() async {
    await _storage.delete(key: 'access_token');
    await _storage.delete(key: 'refresh_token');
  }

  // PURPOSE: Check if user has stored tokens (for auto-login check)
  Future<bool> hasTokens() async {
    final token = await _storage.read(key: 'access_token');
    return token != null;
  }

  // Added: Standard HTTP methods
  Future<Response> get(String path, {Map<String, dynamic>? queryParams}) {
    return _dio.get(path, queryParameters: queryParams);
  }

  Future<Response> post(String path, {dynamic data}) {
    return _dio.post(path, data: data);
  }

  Future<Response> put(String path, {dynamic data}) {
    return _dio.put(path, data: data);
  }

  Future<Response> delete(String path) {
    return _dio.delete(path);
  }

  // Added: Multipart upload for attachments
  Future<Response> upload(String path, String filePath, String fileName) {
    final formData = FormData.fromMap({
      'file': MultipartFile.fromFileSync(filePath, filename: fileName),
    });
    return _dio.post(path, data: formData);
  }
}
