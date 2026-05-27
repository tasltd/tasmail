// Added: Push notification service for TMAIL-150
// PURPOSE: Handles FCM token registration, notification display, and tap handling
// EXTERNAL: Uses /api/push/register endpoint and Firebase Cloud Messaging
// NOTE: FCM dependency (firebase_messaging) should be added when Firebase is configured.
//       This service provides the interface; actual FCM init requires google-services.json.

import '../api/api_client.dart';

class PushService {
  final ApiClient _api = ApiClient();

  // PURPOSE: Register device push token with backend
  Future<bool> registerToken({
    required String token,
    required String platform, // 'android' or 'ios'
    String? deviceName,
  }) async {
    try {
      await _api.post('/push/register', data: {
        'token': token,
        'platform': platform,
        'device_name': deviceName,
      });
      return true;
    } catch (_) {
      return false;
    }
  }

  // PURPOSE: List registered devices
  Future<List<Map<String, dynamic>>> listDevices() async {
    try {
      final response = await _api.get('/push/devices');
      return List<Map<String, dynamic>>.from(response.data as List);
    } catch (_) {
      return [];
    }
  }

  // PURPOSE: Unregister a device
  Future<bool> unregisterDevice(String deviceId) async {
    try {
      await _api.delete('/push/devices/$deviceId');
      return true;
    } catch (_) {
      return false;
    }
  }

  // PURPOSE: Send test push notification
  Future<bool> sendTestPush() async {
    try {
      await _api.post('/push/test');
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
      await _api.put('/push/devices/$deviceId/quiet-hours', data: {
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
      await _api.put('/push/devices/$deviceId/badge', data: {
        'badge_count': badgeCount,
      });
      return true;
    } catch (_) {
      return false;
    }
  }
}
