// Added: Push notification device and log models for TMAIL-50
// PURPOSE: Stores device registrations (FCM/APNs/Web Push) and notification delivery history
// CONSTRAINTS: UNIQUE (user_id, device_token), RLS enforced at DB level for push_devices

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// Added: Push platform enum matching the push_platform DB enum (TMAIL-50)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PushPlatform {
    Fcm,
    Apns,
    Web,
}

impl PushPlatform {
    /// PURPOSE: Convert platform to database string representation
    pub fn as_str(&self) -> &str {
        match self {
            PushPlatform::Fcm => "fcm",
            PushPlatform::Apns => "apns",
            PushPlatform::Web => "web",
        }
    }

    /// PURPOSE: Parse platform from database string
    pub fn from_str(s: &str) -> Option<PushPlatform> {
        match s {
            "fcm" => Some(PushPlatform::Fcm),
            "apns" => Some(PushPlatform::Apns),
            "web" => Some(PushPlatform::Web),
            _ => None,
        }
    }
}

/// Added: A registered push notification device for a user (TMAIL-50)
/// NOTE: RLS enforced at DB level via app.current_user_id session var
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PushDevice {
    pub id: Uuid,
    pub user_id: Uuid,
    pub platform: String,
    pub device_token: String,
    pub device_name: Option<String>,
    pub app_version: Option<String>,
    pub active: Option<bool>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    // Added: Quiet hours + badge count for TMAIL-50 (migration 067)
    pub quiet_hours_start: Option<chrono::NaiveTime>,
    pub quiet_hours_end: Option<chrono::NaiveTime>,
    pub quiet_hours_timezone: Option<String>,
    #[serde(default)]
    pub badge_count: i32,
}

/// Added: Request body for registering a push device (TMAIL-50)
#[derive(Debug, Deserialize)]
pub struct RegisterPushDeviceRequest {
    pub platform: String,
    pub device_token: String,
    pub device_name: Option<String>,
    pub app_version: Option<String>,
}

/// Added: Request body for updating per-device quiet hours (TMAIL-50)
/// PURPOSE: Lets the client set/clear the do-not-disturb window for one device.
/// Setting all three to None clears quiet hours entirely.
#[derive(Debug, Deserialize)]
pub struct UpdateQuietHoursRequest {
    pub quiet_hours_start: Option<chrono::NaiveTime>,
    pub quiet_hours_end: Option<chrono::NaiveTime>,
    pub quiet_hours_timezone: Option<String>,
}

/// Added: Request body for syncing the unread badge count from a device (TMAIL-50)
/// PURPOSE: Mobile/web client posts its current unread count so the next outbound
/// push carries the right APNs badge / FCM data.badge.
#[derive(Debug, Deserialize)]
pub struct UpdateBadgeCountRequest {
    pub badge_count: i32,
}

/// Added: Push notification log entry for delivery tracking (TMAIL-50)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PushNotificationLog {
    pub id: Uuid,
    pub user_id: Uuid,
    pub device_id: Option<Uuid>,
    pub title: String,
    pub body: Option<String>,
    pub data: Option<serde_json::Value>,
    pub sent_at: Option<chrono::DateTime<chrono::Utc>>,
    pub delivered: Option<bool>,
    pub error: Option<String>,
}

/// Added: Payload for sending a push notification (TMAIL-50)
/// PURPOSE: `collapse_key` groups notifications on the device by sender or thread
/// (FCM `android.collapse_key`, APNs `aps.thread-id`). `badge` overrides the unread
/// count baked into the device row (e.g. when computed live from IMAP).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PushNotificationPayload {
    pub title: String,
    pub body: Option<String>,
    pub data: Option<serde_json::Value>,
    // Added: TMAIL-50 — used as FCM android.collapse_key and APNs aps.thread-id
    // so multiple emails from the same sender/thread coalesce in the tray.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collapse_key: Option<String>,
    // Added: TMAIL-50 — overrides PushDevice.badge_count for this send only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub badge: Option<u32>,
}

/// Added: Decide whether a device is currently inside its quiet-hours window (TMAIL-50)
/// PURPOSE: Used by push_service to skip delivery during do-not-disturb.
/// - Handles overnight windows (e.g. 22:00 → 07:00) by inverting the comparison.
/// - Falls back to UTC when no timezone is set on the device.
/// - Returns false when the window is unset (start AND end both None).
pub fn is_in_quiet_hours(
    now_utc: chrono::DateTime<chrono::Utc>,
    start: Option<chrono::NaiveTime>,
    end: Option<chrono::NaiveTime>,
    tz: Option<&str>,
) -> bool {
    let (start, end) = match (start, end) {
        (Some(s), Some(e)) => (s, e),
        _ => return false,
    };
    if start == end {
        // NOTE: equal start/end is treated as "always quiet" (locked DND)
        return true;
    }
    let local_time = match tz.and_then(|name| name.parse::<chrono_tz::Tz>().ok()) {
        Some(tz) => now_utc.with_timezone(&tz).time(),
        None => now_utc.time(),
    };
    if start < end {
        // Same-day window, e.g. 13:00 -> 14:30
        local_time >= start && local_time < end
    } else {
        // Overnight window, e.g. 22:00 -> 07:00 spans midnight
        local_time >= start || local_time < end
    }
}

impl PushDevice {
    /// PURPOSE: List all push devices for a specific user
    pub async fn list_by_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<PushDevice>, sqlx::Error> {
        sqlx::query_as::<_, PushDevice>(
            "SELECT * FROM push_devices WHERE user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Register a new push device (upsert on duplicate token)
    pub async fn register(
        pool: &PgPool,
        user_id: Uuid,
        platform: &str,
        device_token: &str,
        device_name: Option<&str>,
        app_version: Option<&str>,
    ) -> Result<PushDevice, sqlx::Error> {
        sqlx::query_as::<_, PushDevice>(
            // Changed: TMAIL-204 — migration 061 widened platform from the
            // push_platform ENUM to TEXT + CHECK; the cast is no longer valid
            // (the type was dropped) and not needed (sqlx binds &str directly).
            "INSERT INTO push_devices (user_id, platform, device_token, device_name, app_version) \
             VALUES ($1, $2, $3, $4, $5) \
             ON CONFLICT (user_id, device_token) DO UPDATE SET \
                platform = EXCLUDED.platform, \
                device_name = EXCLUDED.device_name, \
                app_version = EXCLUDED.app_version, \
                active = true, \
                last_used_at = NOW() \
             RETURNING *",
        )
        .bind(user_id)
        .bind(platform)
        .bind(device_token)
        .bind(device_name)
        .bind(app_version)
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: Get active devices for a user (for sending notifications)
    pub async fn list_active_by_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<PushDevice>, sqlx::Error> {
        sqlx::query_as::<_, PushDevice>(
            "SELECT * FROM push_devices WHERE user_id = $1 AND active = true ORDER BY last_used_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Delete a device registration
    pub async fn delete(pool: &PgPool, id: Uuid, user_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM push_devices WHERE id = $1 AND user_id = $2",
        )
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// PURPOSE: Deactivate a device (mark as inactive instead of deleting)
    pub async fn deactivate(pool: &PgPool, id: Uuid, user_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE push_devices SET active = false WHERE id = $1 AND user_id = $2",
        )
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Added: Update the per-device quiet-hours window (TMAIL-50)
    /// PURPOSE: Sets/clears the do-not-disturb window for a single device.
    /// Setting all three params to None clears the window.
    pub async fn update_quiet_hours(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
        start: Option<chrono::NaiveTime>,
        end: Option<chrono::NaiveTime>,
        tz: Option<&str>,
    ) -> Result<Option<PushDevice>, sqlx::Error> {
        sqlx::query_as::<_, PushDevice>(
            "UPDATE push_devices \
             SET quiet_hours_start = $3, quiet_hours_end = $4, quiet_hours_timezone = $5 \
             WHERE id = $1 AND user_id = $2 \
             RETURNING *",
        )
        .bind(id)
        .bind(user_id)
        .bind(start)
        .bind(end)
        .bind(tz)
        .fetch_optional(pool)
        .await
    }

    /// Added: Sync the unread badge count from the device (TMAIL-50)
    /// PURPOSE: Outbound APNs/FCM payloads read this value so the system-tray
    /// badge tracks the device's last-known unread count even when no new mail
    /// has arrived since the last sync.
    pub async fn update_badge_count(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
        badge_count: i32,
    ) -> Result<Option<PushDevice>, sqlx::Error> {
        // NOTE: clamp at 0 — DB also enforces this via CHECK constraint
        let clamped = badge_count.max(0);
        sqlx::query_as::<_, PushDevice>(
            "UPDATE push_devices SET badge_count = $3 \
             WHERE id = $1 AND user_id = $2 RETURNING *",
        )
        .bind(id)
        .bind(user_id)
        .bind(clamped)
        .fetch_optional(pool)
        .await
    }

    /// Added: Convenience check — is this device inside its quiet-hours window? (TMAIL-50)
    pub fn is_in_quiet_hours_now(&self, now_utc: chrono::DateTime<chrono::Utc>) -> bool {
        is_in_quiet_hours(
            now_utc,
            self.quiet_hours_start,
            self.quiet_hours_end,
            self.quiet_hours_timezone.as_deref(),
        )
    }
}

impl PushNotificationLog {
    /// PURPOSE: Record a sent notification in the log
    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        device_id: Option<Uuid>,
        title: &str,
        body: Option<&str>,
        data: Option<&serde_json::Value>,
        delivered: bool,
        error: Option<&str>,
    ) -> Result<PushNotificationLog, sqlx::Error> {
        sqlx::query_as::<_, PushNotificationLog>(
            "INSERT INTO push_notification_log (user_id, device_id, title, body, data, delivered, error) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING *",
        )
        .bind(user_id)
        .bind(device_id)
        .bind(title)
        .bind(body)
        .bind(data)
        .bind(delivered)
        .bind(error)
        .fetch_one(pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_platform_as_str() {
        assert_eq!(PushPlatform::Fcm.as_str(), "fcm");
        assert_eq!(PushPlatform::Apns.as_str(), "apns");
        assert_eq!(PushPlatform::Web.as_str(), "web");
    }

    #[test]
    fn test_push_platform_from_str() {
        assert_eq!(PushPlatform::from_str("fcm"), Some(PushPlatform::Fcm));
        assert_eq!(PushPlatform::from_str("apns"), Some(PushPlatform::Apns));
        assert_eq!(PushPlatform::from_str("web"), Some(PushPlatform::Web));
        assert_eq!(PushPlatform::from_str("invalid"), None);
    }

    #[test]
    fn test_push_platform_roundtrip() {
        // NOTE: Verify all platforms survive a roundtrip through as_str/from_str
        let platforms = vec![PushPlatform::Fcm, PushPlatform::Apns, PushPlatform::Web];
        for platform in platforms {
            let s = platform.as_str();
            let parsed = PushPlatform::from_str(s).unwrap();
            assert_eq!(parsed, platform);
        }
    }

    #[test]
    fn test_push_platform_serde() {
        // NOTE: Verify PushPlatform round-trips through JSON
        let platform = PushPlatform::Fcm;
        let json = serde_json::to_string(&platform).unwrap();
        assert_eq!(json, "\"fcm\"");
        let parsed: PushPlatform = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, PushPlatform::Fcm);
    }

    #[test]
    fn test_push_device_serialization() {
        let device = PushDevice {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            platform: "fcm".to_string(),
            device_token: "token123abc".to_string(),
            device_name: Some("Pixel 9 Pro".to_string()),
            app_version: Some("2.1.0".to_string()),
            active: Some(true),
            last_used_at: Some(chrono::Utc::now()),
            created_at: Some(chrono::Utc::now()),
            quiet_hours_start: None,
            quiet_hours_end: None,
            quiet_hours_timezone: None,
            badge_count: 0,
        };

        let json = serde_json::to_value(&device).unwrap();
        assert_eq!(json["platform"], "fcm");
        assert_eq!(json["device_token"], "token123abc");
        assert_eq!(json["device_name"], "Pixel 9 Pro");
        assert_eq!(json["app_version"], "2.1.0");
        assert_eq!(json["active"], true);
        assert_eq!(json["badge_count"], 0);
    }

    #[test]
    fn test_push_device_serialization_minimal() {
        let device = PushDevice {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            platform: "apns".to_string(),
            device_token: "apns-token-xyz".to_string(),
            device_name: None,
            app_version: None,
            active: Some(true),
            last_used_at: None,
            created_at: None,
            quiet_hours_start: None,
            quiet_hours_end: None,
            quiet_hours_timezone: None,
            badge_count: 0,
        };

        let json = serde_json::to_value(&device).unwrap();
        assert_eq!(json["platform"], "apns");
        assert!(json["device_name"].is_null());
        assert!(json["app_version"].is_null());
    }

    #[test]
    fn test_push_device_deserialization() {
        // NOTE: badge_count omitted to verify #[serde(default)] kicks in
        let json = serde_json::json!({
            "id": "00000000-0000-0000-0000-000000000001",
            "user_id": "00000000-0000-0000-0000-000000000002",
            "platform": "web",
            "device_token": "web-push-endpoint-url",
            "device_name": "Chrome Desktop",
            "app_version": "1.0.0",
            "active": true,
            "last_used_at": "2026-04-14T10:00:00Z",
            "created_at": "2026-04-14T00:00:00Z"
        });

        let device: PushDevice = serde_json::from_value(json).unwrap();
        assert_eq!(device.platform, "web");
        assert_eq!(device.device_token, "web-push-endpoint-url");
        assert_eq!(device.device_name, Some("Chrome Desktop".to_string()));
        assert_eq!(device.active, Some(true));
        assert!(device.last_used_at.is_some());
        assert_eq!(device.badge_count, 0);
        assert!(device.quiet_hours_start.is_none());
    }

    // Added: Quiet hours window tests (TMAIL-50) — cover same-day, overnight,
    // null inputs, equal start/end, and timezone-aware checks.
    #[test]
    fn test_quiet_hours_unset_returns_false() {
        let now = chrono::Utc::now();
        assert!(!is_in_quiet_hours(now, None, None, None));
        assert!(!is_in_quiet_hours(now, Some(chrono::NaiveTime::from_hms_opt(22, 0, 0).unwrap()), None, None));
        assert!(!is_in_quiet_hours(now, None, Some(chrono::NaiveTime::from_hms_opt(7, 0, 0).unwrap()), None));
    }

    #[test]
    fn test_quiet_hours_same_day_window() {
        // 13:00 -> 14:30 UTC window
        let start = chrono::NaiveTime::from_hms_opt(13, 0, 0).unwrap();
        let end = chrono::NaiveTime::from_hms_opt(14, 30, 0).unwrap();

        let inside = chrono::DateTime::parse_from_rfc3339("2026-04-14T13:30:00Z").unwrap().with_timezone(&chrono::Utc);
        let before = chrono::DateTime::parse_from_rfc3339("2026-04-14T12:59:00Z").unwrap().with_timezone(&chrono::Utc);
        let after = chrono::DateTime::parse_from_rfc3339("2026-04-14T14:30:00Z").unwrap().with_timezone(&chrono::Utc);
        let way_after = chrono::DateTime::parse_from_rfc3339("2026-04-14T20:00:00Z").unwrap().with_timezone(&chrono::Utc);

        assert!(is_in_quiet_hours(inside, Some(start), Some(end), None));
        assert!(!is_in_quiet_hours(before, Some(start), Some(end), None));
        assert!(!is_in_quiet_hours(after, Some(start), Some(end), None));
        assert!(!is_in_quiet_hours(way_after, Some(start), Some(end), None));
    }

    #[test]
    fn test_quiet_hours_overnight_window() {
        // 22:00 -> 07:00 UTC, spans midnight
        let start = chrono::NaiveTime::from_hms_opt(22, 0, 0).unwrap();
        let end = chrono::NaiveTime::from_hms_opt(7, 0, 0).unwrap();

        let late_night = chrono::DateTime::parse_from_rfc3339("2026-04-14T23:30:00Z").unwrap().with_timezone(&chrono::Utc);
        let early_morning = chrono::DateTime::parse_from_rfc3339("2026-04-14T03:00:00Z").unwrap().with_timezone(&chrono::Utc);
        let midday = chrono::DateTime::parse_from_rfc3339("2026-04-14T12:00:00Z").unwrap().with_timezone(&chrono::Utc);
        let evening = chrono::DateTime::parse_from_rfc3339("2026-04-14T21:59:00Z").unwrap().with_timezone(&chrono::Utc);

        assert!(is_in_quiet_hours(late_night, Some(start), Some(end), None));
        assert!(is_in_quiet_hours(early_morning, Some(start), Some(end), None));
        assert!(!is_in_quiet_hours(midday, Some(start), Some(end), None));
        assert!(!is_in_quiet_hours(evening, Some(start), Some(end), None));
    }

    #[test]
    fn test_quiet_hours_equal_start_end_is_always_quiet() {
        // NOTE: locked DND — start == end means quiet 24/7
        let t = chrono::NaiveTime::from_hms_opt(10, 0, 0).unwrap();
        let any_moment = chrono::Utc::now();
        assert!(is_in_quiet_hours(any_moment, Some(t), Some(t), None));
    }

    #[test]
    fn test_quiet_hours_respects_timezone() {
        // 22:00 -> 07:00 Africa/Accra (UTC+0) — but pretend we want UTC-5 (America/New_York)
        // 02:00 UTC == 22:00 NY previous day → should be inside quiet hours for NY user.
        let start = chrono::NaiveTime::from_hms_opt(22, 0, 0).unwrap();
        let end = chrono::NaiveTime::from_hms_opt(7, 0, 0).unwrap();

        let utc_0200 = chrono::DateTime::parse_from_rfc3339("2026-04-14T02:00:00Z").unwrap().with_timezone(&chrono::Utc);
        // 02:00 UTC == 22:00 EDT (UTC-4 in April) -> inside the 22:00-07:00 window
        assert!(is_in_quiet_hours(utc_0200, Some(start), Some(end), Some("America/New_York")));

        // 15:00 UTC == 11:00 EDT -> outside the window
        let utc_1500 = chrono::DateTime::parse_from_rfc3339("2026-04-14T15:00:00Z").unwrap().with_timezone(&chrono::Utc);
        assert!(!is_in_quiet_hours(utc_1500, Some(start), Some(end), Some("America/New_York")));
    }

    #[test]
    fn test_quiet_hours_invalid_timezone_falls_back_to_utc() {
        let start = chrono::NaiveTime::from_hms_opt(13, 0, 0).unwrap();
        let end = chrono::NaiveTime::from_hms_opt(14, 0, 0).unwrap();
        let inside_utc = chrono::DateTime::parse_from_rfc3339("2026-04-14T13:30:00Z").unwrap().with_timezone(&chrono::Utc);
        assert!(is_in_quiet_hours(inside_utc, Some(start), Some(end), Some("Not/A/Real/Zone")));
    }

    #[test]
    fn test_update_quiet_hours_request_parses() {
        let json = serde_json::json!({
            "quiet_hours_start": "22:00:00",
            "quiet_hours_end": "07:00:00",
            "quiet_hours_timezone": "Africa/Accra"
        });
        let req: UpdateQuietHoursRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.quiet_hours_start.unwrap().to_string(), "22:00:00");
        assert_eq!(req.quiet_hours_end.unwrap().to_string(), "07:00:00");
        assert_eq!(req.quiet_hours_timezone.as_deref(), Some("Africa/Accra"));
    }

    #[test]
    fn test_update_quiet_hours_request_accepts_null_to_clear() {
        let json = serde_json::json!({
            "quiet_hours_start": null,
            "quiet_hours_end": null,
            "quiet_hours_timezone": null
        });
        let req: UpdateQuietHoursRequest = serde_json::from_value(json).unwrap();
        assert!(req.quiet_hours_start.is_none());
        assert!(req.quiet_hours_end.is_none());
        assert!(req.quiet_hours_timezone.is_none());
    }

    #[test]
    fn test_update_badge_count_request_parses() {
        let req: UpdateBadgeCountRequest = serde_json::from_value(serde_json::json!({
            "badge_count": 7
        })).unwrap();
        assert_eq!(req.badge_count, 7);
    }

    #[test]
    fn test_push_notification_payload_grouping_and_badge_serialize() {
        let payload = PushNotificationPayload {
            title: "New mail".to_string(),
            body: Some("subject line".to_string()),
            data: None,
            collapse_key: Some("thread:abc123".to_string()),
            badge: Some(5),
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["collapse_key"], "thread:abc123");
        assert_eq!(json["badge"], 5);
    }

    #[test]
    fn test_push_notification_payload_omits_optional_fields_when_unset() {
        let payload = PushNotificationPayload {
            title: "Hi".to_string(),
            body: None,
            data: None,
            collapse_key: None,
            badge: None,
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert!(json.get("collapse_key").is_none());
        assert!(json.get("badge").is_none());
    }

    #[test]
    fn test_register_push_device_request_full() {
        let json = serde_json::json!({
            "platform": "fcm",
            "device_token": "fcm-token-abc",
            "device_name": "Samsung Galaxy S25",
            "app_version": "3.0.0"
        });

        let request: RegisterPushDeviceRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.platform, "fcm");
        assert_eq!(request.device_token, "fcm-token-abc");
        assert_eq!(request.device_name, Some("Samsung Galaxy S25".to_string()));
        assert_eq!(request.app_version, Some("3.0.0".to_string()));
    }

    #[test]
    fn test_register_push_device_request_minimal() {
        let json = serde_json::json!({
            "platform": "apns",
            "device_token": "apns-token-123"
        });

        let request: RegisterPushDeviceRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.platform, "apns");
        assert_eq!(request.device_token, "apns-token-123");
        assert!(request.device_name.is_none());
        assert!(request.app_version.is_none());
    }

    #[test]
    fn test_push_notification_log_serialization() {
        let log = PushNotificationLog {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            device_id: Some(Uuid::new_v4()),
            title: "New Email".to_string(),
            body: Some("You have a new email from alice@example.com".to_string()),
            data: Some(serde_json::json!({"folder": "INBOX", "uid": 42})),
            sent_at: Some(chrono::Utc::now()),
            delivered: Some(true),
            error: None,
        };

        let json = serde_json::to_value(&log).unwrap();
        assert_eq!(json["title"], "New Email");
        assert_eq!(json["delivered"], true);
        assert!(json["error"].is_null());
        assert_eq!(json["data"]["folder"], "INBOX");
    }

    #[test]
    fn test_push_notification_log_with_error() {
        let log = PushNotificationLog {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            device_id: None,
            title: "Test Notification".to_string(),
            body: None,
            data: None,
            sent_at: Some(chrono::Utc::now()),
            delivered: Some(false),
            error: Some("InvalidRegistration".to_string()),
        };

        let json = serde_json::to_value(&log).unwrap();
        assert_eq!(json["title"], "Test Notification");
        assert_eq!(json["delivered"], false);
        assert_eq!(json["error"], "InvalidRegistration");
        assert!(json["device_id"].is_null());
    }

    #[test]
    fn test_push_notification_payload_construction() {
        let payload = PushNotificationPayload {
            title: "New Email from Bob".to_string(),
            body: Some("Re: Project Update".to_string()),
            data: Some(serde_json::json!({"type": "new_email", "folder": "INBOX"})),
            collapse_key: None,
            badge: None,
        };

        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["title"], "New Email from Bob");
        assert_eq!(json["body"], "Re: Project Update");
        assert_eq!(json["data"]["type"], "new_email");
    }

    #[test]
    fn test_push_notification_payload_minimal() {
        let payload = PushNotificationPayload {
            title: "Notification".to_string(),
            body: None,
            data: None,
            collapse_key: None,
            badge: None,
        };

        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["title"], "Notification");
        assert!(json["body"].is_null());
        assert!(json["data"].is_null());
    }

    #[test]
    fn test_register_request_rejects_missing_platform() {
        let json = serde_json::json!({
            "device_token": "token123"
        });
        let result = serde_json::from_value::<RegisterPushDeviceRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_register_request_rejects_missing_token() {
        let json = serde_json::json!({
            "platform": "fcm"
        });
        let result = serde_json::from_value::<RegisterPushDeviceRequest>(json);
        assert!(result.is_err());
    }
}
