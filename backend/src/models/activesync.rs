// Added: ActiveSync device and policy models for TMAIL-130
// PURPOSE: Stores device registrations and sync policies for ActiveSync device management
// CONSTRAINTS: UNIQUE (user_id, device_id), RLS enforced at DB level for devices

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// PURPOSE: Device status enum for ActiveSync device management
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DeviceStatus {
    Allowed,
    Blocked,
    Pending,
    Wiped,
}

impl DeviceStatus {
    /// PURPOSE: Convert status to database string representation
    pub fn as_str(&self) -> &str {
        match self {
            DeviceStatus::Allowed => "allowed",
            DeviceStatus::Blocked => "blocked",
            DeviceStatus::Pending => "pending",
            DeviceStatus::Wiped => "wiped",
        }
    }

    /// PURPOSE: Parse status from database string
    pub fn from_str(s: &str) -> Option<DeviceStatus> {
        match s {
            "allowed" => Some(DeviceStatus::Allowed),
            "blocked" => Some(DeviceStatus::Blocked),
            "pending" => Some(DeviceStatus::Pending),
            "wiped" => Some(DeviceStatus::Wiped),
            _ => None,
        }
    }
}

/// PURPOSE: A registered ActiveSync device for a user
/// NOTE: RLS enforced at DB level via app.current_user_id session var
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ActiveSyncDevice {
    pub id: Uuid,
    pub user_id: Uuid,
    pub device_id: String,
    pub device_type: String,
    pub device_name: Option<String>,
    pub device_os: Option<String>,
    pub last_sync_at: Option<chrono::DateTime<chrono::Utc>>,
    pub status: String,
    pub policy_key: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// PURPOSE: Request body for registering a new ActiveSync device
#[derive(Debug, Deserialize)]
pub struct RegisterDeviceRequest {
    pub device_id: String,
    pub device_type: String,
    pub device_name: Option<String>,
    pub device_os: Option<String>,
}

/// PURPOSE: ActiveSync sync policy defining security requirements for mobile devices
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ActiveSyncPolicy {
    pub id: Uuid,
    pub name: String,
    pub require_encryption: bool,
    pub max_inactivity_lock_mins: Option<i32>,
    pub min_password_length: Option<i32>,
    pub allow_simple_password: bool,
    pub max_failed_password_attempts: Option<i32>,
    pub is_default: bool,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// PURPOSE: Request body for creating or updating an ActiveSync policy
#[derive(Debug, Deserialize)]
pub struct CreatePolicyRequest {
    pub name: String,
    pub require_encryption: Option<bool>,
    pub max_inactivity_lock_mins: Option<i32>,
    pub min_password_length: Option<i32>,
    pub allow_simple_password: Option<bool>,
    pub max_failed_password_attempts: Option<i32>,
    pub is_default: Option<bool>,
}

/// PURPOSE: Request body for updating an existing ActiveSync policy
#[derive(Debug, Deserialize)]
pub struct UpdatePolicyRequest {
    pub name: Option<String>,
    pub require_encryption: Option<bool>,
    pub max_inactivity_lock_mins: Option<Option<i32>>,
    pub min_password_length: Option<Option<i32>>,
    pub allow_simple_password: Option<bool>,
    pub max_failed_password_attempts: Option<Option<i32>>,
    pub is_default: Option<bool>,
}

impl ActiveSyncDevice {
    /// PURPOSE: List all ActiveSync devices for a specific user
    pub async fn list_by_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<ActiveSyncDevice>, sqlx::Error> {
        sqlx::query_as::<_, ActiveSyncDevice>(
            "SELECT * FROM activesync_devices WHERE user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Register a new ActiveSync device for a user
    pub async fn register(
        pool: &PgPool,
        user_id: Uuid,
        device_id: &str,
        device_type: &str,
        device_name: Option<&str>,
        device_os: Option<&str>,
    ) -> Result<ActiveSyncDevice, sqlx::Error> {
        sqlx::query_as::<_, ActiveSyncDevice>(
            "INSERT INTO activesync_devices (user_id, device_id, device_type, device_name, device_os) \
             VALUES ($1, $2, $3, $4, $5) \
             ON CONFLICT (user_id, device_id) DO UPDATE SET \
                device_type = EXCLUDED.device_type, \
                device_name = EXCLUDED.device_name, \
                device_os = EXCLUDED.device_os, \
                updated_at = NOW() \
             RETURNING *",
        )
        .bind(user_id)
        .bind(device_id)
        .bind(device_type)
        .bind(device_name)
        .bind(device_os)
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: Update the status of a device (allow, block, wipe)
    pub async fn update_status(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
        status: &str,
    ) -> Result<Option<ActiveSyncDevice>, sqlx::Error> {
        sqlx::query_as::<_, ActiveSyncDevice>(
            "UPDATE activesync_devices SET status = $1, updated_at = NOW() \
             WHERE id = $2 AND user_id = $3 RETURNING *",
        )
        .bind(status)
        .bind(id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: Delete a device registration
    pub async fn delete(pool: &PgPool, id: Uuid, user_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM activesync_devices WHERE id = $1 AND user_id = $2",
        )
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// PURPOSE: Get a single device by ID and user
    pub async fn get_by_id(pool: &PgPool, id: Uuid, user_id: Uuid) -> Result<Option<ActiveSyncDevice>, sqlx::Error> {
        sqlx::query_as::<_, ActiveSyncDevice>(
            "SELECT * FROM activesync_devices WHERE id = $1 AND user_id = $2",
        )
        .bind(id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
    }
}

impl ActiveSyncPolicy {
    /// PURPOSE: List all ActiveSync policies
    pub async fn list(pool: &PgPool) -> Result<Vec<ActiveSyncPolicy>, sqlx::Error> {
        sqlx::query_as::<_, ActiveSyncPolicy>(
            "SELECT * FROM activesync_policies ORDER BY is_default DESC, created_at ASC",
        )
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Create a new ActiveSync policy
    pub async fn create(
        pool: &PgPool,
        name: &str,
        require_encryption: bool,
        max_inactivity_lock_mins: Option<i32>,
        min_password_length: Option<i32>,
        allow_simple_password: bool,
        max_failed_password_attempts: Option<i32>,
        is_default: bool,
    ) -> Result<ActiveSyncPolicy, sqlx::Error> {
        // Added: If this policy is default, unset any existing default first
        if is_default {
            sqlx::query("UPDATE activesync_policies SET is_default = false WHERE is_default = true")
                .execute(pool)
                .await?;
        }

        sqlx::query_as::<_, ActiveSyncPolicy>(
            "INSERT INTO activesync_policies \
             (name, require_encryption, max_inactivity_lock_mins, min_password_length, \
              allow_simple_password, max_failed_password_attempts, is_default) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING *",
        )
        .bind(name)
        .bind(require_encryption)
        .bind(max_inactivity_lock_mins)
        .bind(min_password_length)
        .bind(allow_simple_password)
        .bind(max_failed_password_attempts)
        .bind(is_default)
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: Update an existing ActiveSync policy
    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        name: &str,
        require_encryption: bool,
        max_inactivity_lock_mins: Option<i32>,
        min_password_length: Option<i32>,
        allow_simple_password: bool,
        max_failed_password_attempts: Option<i32>,
        is_default: bool,
    ) -> Result<Option<ActiveSyncPolicy>, sqlx::Error> {
        // Added: If this policy becomes default, unset any existing default first
        if is_default {
            sqlx::query("UPDATE activesync_policies SET is_default = false WHERE is_default = true AND id != $1")
                .bind(id)
                .execute(pool)
                .await?;
        }

        sqlx::query_as::<_, ActiveSyncPolicy>(
            "UPDATE activesync_policies SET \
             name = $1, require_encryption = $2, max_inactivity_lock_mins = $3, \
             min_password_length = $4, allow_simple_password = $5, \
             max_failed_password_attempts = $6, is_default = $7 \
             WHERE id = $8 RETURNING *",
        )
        .bind(name)
        .bind(require_encryption)
        .bind(max_inactivity_lock_mins)
        .bind(min_password_length)
        .bind(allow_simple_password)
        .bind(max_failed_password_attempts)
        .bind(is_default)
        .bind(id)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: Delete an ActiveSync policy
    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM activesync_policies WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// PURPOSE: Get the default ActiveSync policy
    pub async fn get_default(pool: &PgPool) -> Result<Option<ActiveSyncPolicy>, sqlx::Error> {
        sqlx::query_as::<_, ActiveSyncPolicy>(
            "SELECT * FROM activesync_policies WHERE is_default = true LIMIT 1",
        )
        .fetch_optional(pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_status_as_str() {
        assert_eq!(DeviceStatus::Allowed.as_str(), "allowed");
        assert_eq!(DeviceStatus::Blocked.as_str(), "blocked");
        assert_eq!(DeviceStatus::Pending.as_str(), "pending");
        assert_eq!(DeviceStatus::Wiped.as_str(), "wiped");
    }

    #[test]
    fn test_device_status_from_str() {
        assert_eq!(DeviceStatus::from_str("allowed"), Some(DeviceStatus::Allowed));
        assert_eq!(DeviceStatus::from_str("blocked"), Some(DeviceStatus::Blocked));
        assert_eq!(DeviceStatus::from_str("pending"), Some(DeviceStatus::Pending));
        assert_eq!(DeviceStatus::from_str("wiped"), Some(DeviceStatus::Wiped));
        assert_eq!(DeviceStatus::from_str("invalid"), None);
    }

    #[test]
    fn test_device_status_roundtrip() {
        // NOTE: Verify all statuses survive a roundtrip through as_str/from_str
        let statuses = vec![
            DeviceStatus::Allowed,
            DeviceStatus::Blocked,
            DeviceStatus::Pending,
            DeviceStatus::Wiped,
        ];
        for status in statuses {
            let s = status.as_str();
            let parsed = DeviceStatus::from_str(s).unwrap();
            assert_eq!(parsed, status);
        }
    }

    #[test]
    fn test_device_serialization() {
        let device = ActiveSyncDevice {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            device_id: "DEVICE123".to_string(),
            device_type: "iPhone".to_string(),
            device_name: Some("My iPhone 15".to_string()),
            device_os: Some("iOS 18.2".to_string()),
            last_sync_at: None,
            status: "allowed".to_string(),
            policy_key: None,
            created_at: Some(chrono::Utc::now()),
            updated_at: Some(chrono::Utc::now()),
        };

        let json = serde_json::to_value(&device).unwrap();
        assert_eq!(json["device_id"], "DEVICE123");
        assert_eq!(json["device_type"], "iPhone");
        assert_eq!(json["device_name"], "My iPhone 15");
        assert_eq!(json["device_os"], "iOS 18.2");
        assert_eq!(json["status"], "allowed");
        assert!(json["last_sync_at"].is_null());
    }

    #[test]
    fn test_device_serialization_minimal() {
        let device = ActiveSyncDevice {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            device_id: "DEV456".to_string(),
            device_type: "Android".to_string(),
            device_name: None,
            device_os: None,
            last_sync_at: None,
            status: "pending".to_string(),
            policy_key: None,
            created_at: None,
            updated_at: None,
        };

        let json = serde_json::to_value(&device).unwrap();
        assert_eq!(json["device_type"], "Android");
        assert!(json["device_name"].is_null());
        assert!(json["device_os"].is_null());
        assert!(json["created_at"].is_null());
    }

    #[test]
    fn test_device_deserialization() {
        let json = serde_json::json!({
            "id": "00000000-0000-0000-0000-000000000001",
            "user_id": "00000000-0000-0000-0000-000000000002",
            "device_id": "WINPHONE99",
            "device_type": "WindowsMail",
            "device_name": "Surface Pro",
            "device_os": "Windows 11",
            "last_sync_at": "2026-04-14T10:00:00Z",
            "status": "blocked",
            "policy_key": "key123",
            "created_at": "2026-04-14T00:00:00Z",
            "updated_at": "2026-04-14T00:00:00Z"
        });

        let device: ActiveSyncDevice = serde_json::from_value(json).unwrap();
        assert_eq!(device.device_id, "WINPHONE99");
        assert_eq!(device.device_type, "WindowsMail");
        assert_eq!(device.status, "blocked");
        assert_eq!(device.policy_key, Some("key123".to_string()));
        assert!(device.last_sync_at.is_some());
    }

    #[test]
    fn test_register_device_request_full() {
        let json = serde_json::json!({
            "device_id": "ABC123",
            "device_type": "iPhone",
            "device_name": "Work Phone",
            "device_os": "iOS 18"
        });

        let request: RegisterDeviceRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.device_id, "ABC123");
        assert_eq!(request.device_type, "iPhone");
        assert_eq!(request.device_name, Some("Work Phone".to_string()));
        assert_eq!(request.device_os, Some("iOS 18".to_string()));
    }

    #[test]
    fn test_register_device_request_minimal() {
        let json = serde_json::json!({
            "device_id": "DEV1",
            "device_type": "Android"
        });

        let request: RegisterDeviceRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.device_id, "DEV1");
        assert_eq!(request.device_type, "Android");
        assert!(request.device_name.is_none());
        assert!(request.device_os.is_none());
    }

    #[test]
    fn test_policy_serialization() {
        let policy = ActiveSyncPolicy {
            id: Uuid::new_v4(),
            name: "Strict Policy".to_string(),
            require_encryption: true,
            max_inactivity_lock_mins: Some(5),
            min_password_length: Some(8),
            allow_simple_password: false,
            max_failed_password_attempts: Some(5),
            is_default: true,
            created_at: Some(chrono::Utc::now()),
        };

        let json = serde_json::to_value(&policy).unwrap();
        assert_eq!(json["name"], "Strict Policy");
        assert_eq!(json["require_encryption"], true);
        assert_eq!(json["max_inactivity_lock_mins"], 5);
        assert_eq!(json["min_password_length"], 8);
        assert_eq!(json["allow_simple_password"], false);
        assert_eq!(json["max_failed_password_attempts"], 5);
        assert_eq!(json["is_default"], true);
    }

    #[test]
    fn test_policy_serialization_defaults() {
        let policy = ActiveSyncPolicy {
            id: Uuid::new_v4(),
            name: "Relaxed".to_string(),
            require_encryption: false,
            max_inactivity_lock_mins: None,
            min_password_length: None,
            allow_simple_password: true,
            max_failed_password_attempts: None,
            is_default: false,
            created_at: None,
        };

        let json = serde_json::to_value(&policy).unwrap();
        assert_eq!(json["require_encryption"], false);
        assert_eq!(json["allow_simple_password"], true);
        assert!(json["max_inactivity_lock_mins"].is_null());
        assert!(json["min_password_length"].is_null());
        assert!(json["created_at"].is_null());
    }

    #[test]
    fn test_create_policy_request_full() {
        let json = serde_json::json!({
            "name": "Corporate Policy",
            "require_encryption": true,
            "max_inactivity_lock_mins": 3,
            "min_password_length": 6,
            "allow_simple_password": false,
            "max_failed_password_attempts": 5,
            "is_default": true
        });

        let request: CreatePolicyRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.name, "Corporate Policy");
        assert_eq!(request.require_encryption, Some(true));
        assert_eq!(request.max_inactivity_lock_mins, Some(3));
        assert_eq!(request.min_password_length, Some(6));
        assert_eq!(request.allow_simple_password, Some(false));
        assert_eq!(request.max_failed_password_attempts, Some(5));
        assert_eq!(request.is_default, Some(true));
    }

    #[test]
    fn test_create_policy_request_name_only() {
        let json = serde_json::json!({
            "name": "Basic"
        });

        let request: CreatePolicyRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.name, "Basic");
        assert!(request.require_encryption.is_none());
        assert!(request.max_inactivity_lock_mins.is_none());
        assert!(request.is_default.is_none());
    }

    #[test]
    fn test_update_policy_request_partial() {
        let json = serde_json::json!({
            "name": "Updated Name",
            "is_default": true
        });

        let request: UpdatePolicyRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.name, Some("Updated Name".to_string()));
        assert_eq!(request.is_default, Some(true));
        assert!(request.require_encryption.is_none());
        assert!(request.max_inactivity_lock_mins.is_none());
    }

    #[test]
    fn test_device_status_serde() {
        // NOTE: Verify DeviceStatus round-trips through JSON
        let status = DeviceStatus::Wiped;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"wiped\"");
        let parsed: DeviceStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, DeviceStatus::Wiped);
    }
}
