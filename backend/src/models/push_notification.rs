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
}

/// Added: Request body for registering a push device (TMAIL-50)
#[derive(Debug, Deserialize)]
pub struct RegisterPushDeviceRequest {
    pub platform: String,
    pub device_token: String,
    pub device_name: Option<String>,
    pub app_version: Option<String>,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushNotificationPayload {
    pub title: String,
    pub body: Option<String>,
    pub data: Option<serde_json::Value>,
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
            "INSERT INTO push_devices (user_id, platform, device_token, device_name, app_version) \
             VALUES ($1, $2::push_platform, $3, $4, $5) \
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
        };

        let json = serde_json::to_value(&device).unwrap();
        assert_eq!(json["platform"], "fcm");
        assert_eq!(json["device_token"], "token123abc");
        assert_eq!(json["device_name"], "Pixel 9 Pro");
        assert_eq!(json["app_version"], "2.1.0");
        assert_eq!(json["active"], true);
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
        };

        let json = serde_json::to_value(&device).unwrap();
        assert_eq!(json["platform"], "apns");
        assert!(json["device_name"].is_null());
        assert!(json["app_version"].is_null());
    }

    #[test]
    fn test_push_device_deserialization() {
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
