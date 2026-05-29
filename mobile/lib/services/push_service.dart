// Added: Push notification service for TMAIL-150
// Changed: TMAIL-150 — refactored to inject a PushHttpClient delegate so the
//          wrapper is unit-testable without spinning up Dio + flutter_secure_storage.
//          Public method signatures unchanged; existing callers see no diff.
// PURPOSE: Thin REST wrapper around /api/push/* that the FCM bootstrap layer
//          calls once it has a real device token from firebase_messaging.
// EXTERNAL: Hits the backend endpoints exposed by `backend/src/handlers/push.rs`:
//             POST   /push/register
//             GET    /push/devices
//             DELETE /push/devices/{id}
//             POST   /push/test
//             PUT    /push/devices/{id}/quiet-hours
//             PUT    /push/devices/{id}/badge
// NOTE: Real FCM SDK wiring (firebase_messaging dep, google-services.json, APNs
//       key) is operator setup — see `docs/MOBILE-FCM-SETUP.md`. Until then
//       FcmBootstrap.register() is a no-op and no calls land here in practice.

import 'package:dio/dio.dart';

import '../api/api_client.dart';

// PURPOSE: Minimal HTTP delegate so PushService is testable without Dio +
//          flutter_secure_storage. ApiClient already satisfies this shape, so
//          production callers don't need to change.
abstract class PushHttpClient {
  Future<Response<dynamic>> post(String path, {dynamic data});
  Future<Response<dynamic>> get(String path, {Map<String, dynamic>? queryParams});
  Future<Response<dynamic>> put(String path, {dynamic data});
  Future<Response<dynamic>> delete(String path);
}

// PURPOSE: Default delegate — forwards to the singleton ApiClient. Pulled out
//          so the constructor stays ergonomic for production code.
class _ApiClientPushHttp implements PushHttpClient {
  final ApiClient _api;
  _ApiClientPushHttp(this._api);

  @override
  Future<Response<dynamic>> post(String path, {dynamic data}) =>
      _api.post(path, data: data);
  @override
  Future<Response<dynamic>> get(String path, {Map<String, dynamic>? queryParams}) =>
      _api.get(path, queryParams: queryParams);
  @override
  Future<Response<dynamic>> put(String path, {dynamic data}) =>
      _api.put(path, data: data);
  @override
  Future<Response<dynamic>> delete(String path) => _api.delete(path);
}

class PushService {
  final PushHttpClient _http;

  // Changed: TMAIL-150 — accept an optional http delegate. Falls back to the
  //          singleton ApiClient so existing call sites keep working.
  PushService({PushHttpClient? http})
      : _http = http ?? _ApiClientPushHttp(ApiClient());

  // PURPOSE: Register device push token with backend.
  //          Returns true on 2xx, false otherwise (network errors, 4xx, 5xx).
  // NOTE: `platform` must be one of 'fcm', 'apns', or 'web' to match
  //       `PushPlatform::from_str` in backend `models/push_notification.rs`.
  Future<bool> registerToken({
    required String token,
    required String platform,
    String? deviceName,
    String? appVersion,
  }) async {
    try {
      await _http.post('/push/register', data: {
        // Changed: TMAIL-150 — use the backend's actual field name
        //          (`device_token`, not `token`) so registration actually works.
        'device_token': token,
        'platform': platform,
        'device_name': deviceName,
        'app_version': appVersion,
      });
      return true;
    } catch (_) {
      return false;
    }
  }

  // PURPOSE: List registered devices for the current user.
  Future<List<Map<String, dynamic>>> listDevices() async {
    try {
      final response = await _http.get('/push/devices');
      final data = response.data;
      if (data is! List) return const [];
      return List<Map<String, dynamic>>.from(
        data.map((e) => Map<String, dynamic>.from(e as Map)),
      );
    } catch (_) {
      return const [];
    }
  }

  // PURPOSE: Unregister a device (e.g. on logout or when the FCM token rotates).
  Future<bool> unregisterDevice(String deviceId) async {
    try {
      await _http.delete('/push/devices/$deviceId');
      return true;
    } catch (_) {
      return false;
    }
  }

  // PURPOSE: Send a self-test push to every registered device for the user.
  //          Surfaced in the Settings screen so users can confirm push works
  //          end-to-end without waiting for a real email.
  Future<bool> sendTestPush() async {
    try {
      await _http.post('/push/test');
      return true;
    } catch (_) {
      return false;
    }
  }

  // Added: TMAIL-50 — Set the per-device quiet-hours window.
  // Pass null to all three to clear the window. Times are "HH:MM:SS" strings,
  // timezone is an IANA name (e.g. 'Africa/Accra').
  Future<bool> setQuietHours({
    required String deviceId,
    String? start,
    String? end,
    String? timezone,
  }) async {
    try {
      await _http.put('/push/devices/$deviceId/quiet-hours', data: {
        'quiet_hours_start': start,
        'quiet_hours_end': end,
        'quiet_hours_timezone': timezone,
      });
      return true;
    } catch (_) {
      return false;
    }
  }

  // Added: TMAIL-50 — Sync the unread badge count from the device so outbound
  // FCM/APNs payloads carry the right number.
  Future<bool> syncBadgeCount({
    required String deviceId,
    required int badgeCount,
  }) async {
    try {
      await _http.put('/push/devices/$deviceId/badge', data: {
        'badge_count': badgeCount,
      });
      return true;
    } catch (_) {
      return false;
    }
  }
}
