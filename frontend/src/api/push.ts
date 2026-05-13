// TMAIL-204: push notification device client.
//
// Wraps the four /api/push/* routes: register, list, unregister, test.
// Web Push subscription (navigator.serviceWorker → pushManager.subscribe)
// is intentionally out of scope here — the backend's push_service does not
// yet ship VAPID auth, so a real browser subscription wouldn't be deliverable
// even if we created it. The manager component instead surfaces:
//   - the device list (mobile FCM/APNs registrations come from the Flutter app)
//   - per-device unregister
//   - a "send test notification" button that exercises /api/push/test
//
// VAPID-backed Web Push subscription is tracked separately (TBD ticket).
import { apiClient } from './client';

export type PushPlatform = 'fcm' | 'apns' | 'web';

export interface PushDevice {
  id: string;
  user_id: string;
  platform: PushPlatform;
  device_token: string;
  device_name: string | null;
  app_version: string | null;
  active: boolean;
  created_at: string;
  last_notified_at: string | null;
}

export interface RegisterDeviceRequest {
  platform: PushPlatform;
  device_token: string;
  device_name?: string;
  app_version?: string;
}

export interface TestNotificationResponse {
  devices_notified: number;
  successes: number;
  failures: number;
}

export const pushApi = {
  register: (body: RegisterDeviceRequest) =>
    apiClient.post<PushDevice>('/push/register', body),
  list: () => apiClient.get<PushDevice[]>('/push/devices'),
  unregister: (id: string) => apiClient.delete<void>(`/push/devices/${id}`),
  test: () => apiClient.post<TestNotificationResponse>('/push/test', {}),
};
