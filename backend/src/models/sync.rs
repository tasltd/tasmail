// Added: Sync protocol models for offline-first delta sync (TMAIL-51)
// PURPOSE: Per-folder sync state tracking with conflict resolution for mobile/offline clients
// CONSTRAINTS: RLS enforced at DB level via app.current_user_id session var

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// Added: Conflict resolution strategy for sync conflicts (TMAIL-51)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolution {
    /// Server state wins — client discards local changes
    ServerWins,
    /// Client state wins — server accepts client changes
    ClientWins,
    /// Merge both — attempt to combine server and client changes
    Merge,
}

impl ConflictResolution {
    /// PURPOSE: Parse conflict resolution strategy from string
    pub fn from_str(s: &str) -> Option<ConflictResolution> {
        match s {
            "server_wins" => Some(ConflictResolution::ServerWins),
            "client_wins" => Some(ConflictResolution::ClientWins),
            "merge" => Some(ConflictResolution::Merge),
            _ => None,
        }
    }

    /// PURPOSE: Convert to database string representation
    pub fn as_str(&self) -> &str {
        match self {
            ConflictResolution::ServerWins => "server_wins",
            ConflictResolution::ClientWins => "client_wins",
            ConflictResolution::Merge => "merge",
        }
    }
}

/// Added: Per-folder sync checkpoint stored in DB per user+device (TMAIL-51)
/// NOTE: Tracks IMAP CONDSTORE/QRESYNC state for efficient delta sync
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SyncCheckpoint {
    pub id: Uuid,
    pub user_id: Uuid,
    pub device_id: Option<Uuid>,
    pub folder_name: String,
    pub last_uid: Option<i64>,
    pub last_modseq: Option<i64>,
    pub uidvalidity: Option<i64>,
    pub last_synced_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Added: Request body for updating a sync checkpoint (TMAIL-51)
#[derive(Debug, Deserialize)]
pub struct UpdateSyncCheckpointRequest {
    pub device_id: Option<Uuid>,
    pub last_uid: i64,
    pub last_modseq: i64,
    pub uidvalidity: i64,
}

/// Added: Request body for resolving sync conflicts (TMAIL-51)
#[derive(Debug, Deserialize)]
pub struct ResolveConflictRequest {
    pub folder: String,
    pub uid: u32,
    pub resolution: String,
    pub client_flags: Option<Vec<String>>,
}

/// Added: Response for conflict resolution (TMAIL-51)
#[derive(Debug, Serialize)]
pub struct ConflictResolutionResponse {
    pub folder: String,
    pub uid: u32,
    pub resolution: String,
    pub applied: bool,
    pub message: String,
}

/// Added: Sync state summary combining checkpoint data with folder metadata (TMAIL-51)
#[derive(Debug, Serialize)]
pub struct SyncState {
    pub folder_name: String,
    pub last_uid: i64,
    pub last_modseq: i64,
    pub uidvalidity: i64,
    pub last_synced_at: Option<chrono::DateTime<chrono::Utc>>,
    pub needs_full_sync: bool,
}

impl SyncCheckpoint {
    /// PURPOSE: Get sync checkpoint for a specific folder and device
    pub async fn get_for_folder(
        pool: &PgPool,
        user_id: Uuid,
        device_id: Option<Uuid>,
        folder_name: &str,
    ) -> Result<Option<SyncCheckpoint>, sqlx::Error> {
        if let Some(dev_id) = device_id {
            sqlx::query_as::<_, SyncCheckpoint>(
                "SELECT * FROM sync_checkpoints WHERE user_id = $1 AND device_id = $2 AND folder_name = $3",
            )
            .bind(user_id)
            .bind(dev_id)
            .bind(folder_name)
            .fetch_optional(pool)
            .await
        } else {
            sqlx::query_as::<_, SyncCheckpoint>(
                "SELECT * FROM sync_checkpoints WHERE user_id = $1 AND device_id IS NULL AND folder_name = $2",
            )
            .bind(user_id)
            .bind(folder_name)
            .fetch_optional(pool)
            .await
        }
    }

    /// PURPOSE: Upsert sync checkpoint after successful folder sync
    pub async fn upsert(
        pool: &PgPool,
        user_id: Uuid,
        device_id: Option<Uuid>,
        folder_name: &str,
        last_uid: i64,
        last_modseq: i64,
        uidvalidity: i64,
    ) -> Result<SyncCheckpoint, sqlx::Error> {
        sqlx::query_as::<_, SyncCheckpoint>(
            "INSERT INTO sync_checkpoints (user_id, device_id, folder_name, last_uid, last_modseq, uidvalidity, last_synced_at) \
             VALUES ($1, $2, $3, $4, $5, $6, NOW()) \
             ON CONFLICT (user_id, device_id, folder_name) DO UPDATE SET \
                last_uid = EXCLUDED.last_uid, \
                last_modseq = EXCLUDED.last_modseq, \
                uidvalidity = EXCLUDED.uidvalidity, \
                last_synced_at = NOW() \
             RETURNING *",
        )
        .bind(user_id)
        .bind(device_id)
        .bind(folder_name)
        .bind(last_uid)
        .bind(last_modseq)
        .bind(uidvalidity)
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: List all sync checkpoints for a user (across all folders and devices)
    pub async fn list_by_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<SyncCheckpoint>, sqlx::Error> {
        sqlx::query_as::<_, SyncCheckpoint>(
            "SELECT * FROM sync_checkpoints WHERE user_id = $1 ORDER BY folder_name, device_id",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Delete all sync checkpoints for a user+device (e.g., device reset)
    pub async fn delete_for_device(
        pool: &PgPool,
        user_id: Uuid,
        device_id: Uuid,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM sync_checkpoints WHERE user_id = $1 AND device_id = $2",
        )
        .bind(user_id)
        .bind(device_id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conflict_resolution_from_str() {
        assert_eq!(
            ConflictResolution::from_str("server_wins"),
            Some(ConflictResolution::ServerWins)
        );
        assert_eq!(
            ConflictResolution::from_str("client_wins"),
            Some(ConflictResolution::ClientWins)
        );
        assert_eq!(
            ConflictResolution::from_str("merge"),
            Some(ConflictResolution::Merge)
        );
        assert_eq!(ConflictResolution::from_str("invalid"), None);
    }

    #[test]
    fn test_conflict_resolution_as_str() {
        assert_eq!(ConflictResolution::ServerWins.as_str(), "server_wins");
        assert_eq!(ConflictResolution::ClientWins.as_str(), "client_wins");
        assert_eq!(ConflictResolution::Merge.as_str(), "merge");
    }

    #[test]
    fn test_conflict_resolution_roundtrip() {
        // NOTE: Verify all strategies survive a roundtrip through as_str/from_str
        let strategies = vec![
            ConflictResolution::ServerWins,
            ConflictResolution::ClientWins,
            ConflictResolution::Merge,
        ];
        for strategy in strategies {
            let s = strategy.as_str();
            let parsed = ConflictResolution::from_str(s).unwrap();
            assert_eq!(parsed, strategy);
        }
    }

    #[test]
    fn test_conflict_resolution_serde() {
        // NOTE: Verify ConflictResolution round-trips through JSON
        let strategy = ConflictResolution::ServerWins;
        let json = serde_json::to_string(&strategy).unwrap();
        assert_eq!(json, "\"server_wins\"");
        let parsed: ConflictResolution = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, ConflictResolution::ServerWins);
    }

    #[test]
    fn test_sync_checkpoint_serialization() {
        let checkpoint = SyncCheckpoint {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            device_id: Some(Uuid::new_v4()),
            folder_name: "INBOX".to_string(),
            last_uid: Some(1500),
            last_modseq: Some(42000),
            uidvalidity: Some(1234567890),
            last_synced_at: Some(chrono::Utc::now()),
            created_at: Some(chrono::Utc::now()),
        };

        let json = serde_json::to_value(&checkpoint).unwrap();
        assert_eq!(json["folder_name"], "INBOX");
        assert_eq!(json["last_uid"], 1500);
        assert_eq!(json["last_modseq"], 42000);
        assert_eq!(json["uidvalidity"], 1234567890);
        assert!(json["device_id"].is_string());
    }

    #[test]
    fn test_sync_checkpoint_serialization_no_device() {
        let checkpoint = SyncCheckpoint {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            device_id: None,
            folder_name: "Sent".to_string(),
            last_uid: Some(0),
            last_modseq: Some(0),
            uidvalidity: Some(0),
            last_synced_at: None,
            created_at: None,
        };

        let json = serde_json::to_value(&checkpoint).unwrap();
        assert_eq!(json["folder_name"], "Sent");
        assert!(json["device_id"].is_null());
        assert!(json["last_synced_at"].is_null());
    }

    #[test]
    fn test_sync_checkpoint_deserialization() {
        let json = serde_json::json!({
            "id": "00000000-0000-0000-0000-000000000001",
            "user_id": "00000000-0000-0000-0000-000000000002",
            "device_id": null,
            "folder_name": "Drafts",
            "last_uid": 250,
            "last_modseq": 8000,
            "uidvalidity": 999,
            "last_synced_at": "2026-04-14T12:00:00Z",
            "created_at": "2026-04-01T00:00:00Z"
        });

        let checkpoint: SyncCheckpoint = serde_json::from_value(json).unwrap();
        assert_eq!(checkpoint.folder_name, "Drafts");
        assert_eq!(checkpoint.last_uid, Some(250));
        assert_eq!(checkpoint.last_modseq, Some(8000));
        assert!(checkpoint.device_id.is_none());
    }

    #[test]
    fn test_update_sync_checkpoint_request() {
        let json = serde_json::json!({
            "device_id": "00000000-0000-0000-0000-000000000003",
            "last_uid": 500,
            "last_modseq": 12000,
            "uidvalidity": 1234567890
        });

        let req: UpdateSyncCheckpointRequest = serde_json::from_value(json).unwrap();
        assert!(req.device_id.is_some());
        assert_eq!(req.last_uid, 500);
        assert_eq!(req.last_modseq, 12000);
        assert_eq!(req.uidvalidity, 1234567890);
    }

    #[test]
    fn test_update_sync_checkpoint_request_no_device() {
        let json = serde_json::json!({
            "last_uid": 100,
            "last_modseq": 5000,
            "uidvalidity": 999
        });

        let req: UpdateSyncCheckpointRequest = serde_json::from_value(json).unwrap();
        assert!(req.device_id.is_none());
        assert_eq!(req.last_uid, 100);
    }

    #[test]
    fn test_resolve_conflict_request() {
        let json = serde_json::json!({
            "folder": "INBOX",
            "uid": 42,
            "resolution": "server_wins",
            "client_flags": ["\\Seen", "\\Flagged"]
        });

        let req: ResolveConflictRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.folder, "INBOX");
        assert_eq!(req.uid, 42);
        assert_eq!(req.resolution, "server_wins");
        assert_eq!(req.client_flags.unwrap().len(), 2);
    }

    #[test]
    fn test_resolve_conflict_request_minimal() {
        let json = serde_json::json!({
            "folder": "Sent",
            "uid": 10,
            "resolution": "merge"
        });

        let req: ResolveConflictRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.resolution, "merge");
        assert!(req.client_flags.is_none());
    }

    #[test]
    fn test_conflict_resolution_response_serialization() {
        let resp = ConflictResolutionResponse {
            folder: "INBOX".to_string(),
            uid: 42,
            resolution: "server_wins".to_string(),
            applied: true,
            message: "Server state applied successfully".to_string(),
        };

        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["folder"], "INBOX");
        assert_eq!(json["uid"], 42);
        assert_eq!(json["applied"], true);
    }

    #[test]
    fn test_sync_state_serialization() {
        let state = SyncState {
            folder_name: "INBOX".to_string(),
            last_uid: 1500,
            last_modseq: 42000,
            uidvalidity: 1234567890,
            last_synced_at: Some(chrono::Utc::now()),
            needs_full_sync: false,
        };

        let json = serde_json::to_value(&state).unwrap();
        assert_eq!(json["folder_name"], "INBOX");
        assert_eq!(json["needs_full_sync"], false);
    }

    #[test]
    fn test_sync_state_needs_full_sync() {
        // Added: When uidvalidity changes, full sync is needed
        let state = SyncState {
            folder_name: "INBOX".to_string(),
            last_uid: 0,
            last_modseq: 0,
            uidvalidity: 0,
            last_synced_at: None,
            needs_full_sync: true,
        };

        let json = serde_json::to_value(&state).unwrap();
        assert_eq!(json["needs_full_sync"], true);
        assert!(json["last_synced_at"].is_null());
    }

    #[test]
    fn test_resolve_conflict_request_rejects_missing_folder() {
        let json = serde_json::json!({
            "uid": 42,
            "resolution": "server_wins"
        });
        let result = serde_json::from_value::<ResolveConflictRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_checkpoint_request_rejects_missing_fields() {
        let json = serde_json::json!({
            "last_uid": 100
        });
        let result = serde_json::from_value::<UpdateSyncCheckpointRequest>(json);
        assert!(result.is_err());
    }
}
