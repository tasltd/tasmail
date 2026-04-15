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
}
