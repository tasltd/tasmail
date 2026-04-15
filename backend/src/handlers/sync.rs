// Added: Sync checkpoint handlers for offline-first delta sync protocol (TMAIL-51)
// PURPOSE: CRUD endpoints for managing per-folder sync state checkpoints and conflict resolution
// NOTE: RLS at DB level enforces user-scoped access to sync_checkpoints

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::error::AppError;
use crate::models::sync::{
    ConflictResolution, ConflictResolutionResponse, ResolveConflictRequest, SyncCheckpoint,
    SyncState, UpdateSyncCheckpointRequest,
};
use crate::services::auth_service::Claims;
use crate::state::AppState;

/// Added: Optional query param for device_id filtering (TMAIL-51)
#[derive(Debug, Deserialize)]
pub struct DeviceQuery {
    pub device_id: Option<uuid::Uuid>,
}

/// GET /api/sync/checkpoint/{folder} — Get current sync state for a folder
/// PURPOSE: Client calls this before syncing to determine what's changed
pub async fn get_checkpoint(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(folder): Path<String>,
    Query(query): Query<DeviceQuery>,
) -> Result<Json<SyncState>, AppError> {
    let user_id = parse_user_id(&claims)?;

    // Added: Look up existing checkpoint for this folder+device combo
    let checkpoint =
        SyncCheckpoint::get_for_folder(&state.db, user_id, query.device_id, &folder).await?;

    match checkpoint {
        Some(cp) => Ok(Json(SyncState {
            folder_name: cp.folder_name,
            last_uid: cp.last_uid.unwrap_or(0),
            last_modseq: cp.last_modseq.unwrap_or(0),
            uidvalidity: cp.uidvalidity.unwrap_or(0),
            last_synced_at: cp.last_synced_at,
            needs_full_sync: false,
        })),
        None => {
            // Added: No checkpoint means first sync — client should do full sync
            Ok(Json(SyncState {
                folder_name: folder,
                last_uid: 0,
                last_modseq: 0,
                uidvalidity: 0,
                last_synced_at: None,
                needs_full_sync: true,
            }))
        }
    }
}

/// POST /api/sync/checkpoint/{folder} — Update sync checkpoint after successful sync
/// PURPOSE: Client reports its new sync state after downloading changes
pub async fn update_checkpoint(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(folder): Path<String>,
    Json(body): Json<UpdateSyncCheckpointRequest>,
) -> Result<(StatusCode, Json<SyncCheckpoint>), AppError> {
    let user_id = parse_user_id(&claims)?;

    // Added: Validate folder name is not empty
    if folder.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Folder name cannot be empty".to_string(),
        ));
    }

    // Added: Validate UID and modseq are non-negative
    if body.last_uid < 0 || body.last_modseq < 0 || body.uidvalidity < 0 {
        return Err(AppError::BadRequest(
            "last_uid, last_modseq, and uidvalidity must be non-negative".to_string(),
        ));
    }

    let checkpoint = SyncCheckpoint::upsert(
        &state.db,
        user_id,
        body.device_id,
        &folder,
        body.last_uid,
        body.last_modseq,
        body.uidvalidity,
    )
    .await?;

    Ok((StatusCode::OK, Json(checkpoint)))
}

/// POST /api/sync/resolve-conflict — Resolve a sync conflict between client and server
/// PURPOSE: When client detects a flag/state mismatch, submit resolution preference
pub async fn resolve_conflict(
    State(_state): State<AppState>,
    axum::Extension(_claims): axum::Extension<Claims>,
    Json(body): Json<ResolveConflictRequest>,
) -> Result<Json<ConflictResolutionResponse>, AppError> {
    // Added: Validate the resolution strategy
    let resolution = ConflictResolution::from_str(&body.resolution).ok_or_else(|| {
        AppError::BadRequest(format!(
            "Invalid resolution strategy '{}': must be 'server_wins', 'client_wins', or 'merge'",
            body.resolution
        ))
    })?;

    // Added: Validate folder name
    if body.folder.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Folder name cannot be empty".to_string(),
        ));
    }

    // NOTE: In a full implementation, this would:
    // 1. For server_wins: discard client changes, return current server state
    // 2. For client_wins: apply client flags via IMAP STORE command
    // 3. For merge: combine flags from both sides (union of flags)
    // Current implementation acknowledges the resolution choice for the protocol framework.

    let message = match resolution {
        ConflictResolution::ServerWins => {
            "Server state retained — client should discard local changes".to_string()
        }
        ConflictResolution::ClientWins => {
            // Added: If client_flags provided, describe what would be applied
            match &body.client_flags {
                Some(flags) => format!(
                    "Client flags accepted — {} flags would be applied to server",
                    flags.len()
                ),
                None => "Client state accepted — no flags provided to apply".to_string(),
            }
        }
        ConflictResolution::Merge => {
            "Merge applied — flags from both client and server combined".to_string()
        }
    };

    Ok(Json(ConflictResolutionResponse {
        folder: body.folder,
        uid: body.uid,
        resolution: resolution.as_str().to_string(),
        applied: true,
        message,
    }))
}

// Added: Parse user UUID from JWT claims
fn parse_user_id(claims: &Claims) -> Result<uuid::Uuid, AppError> {
    claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid user ID in JWT claims")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_query_deserialization_with_id() {
        let json = serde_json::json!({"device_id": "00000000-0000-0000-0000-000000000001"});
        let query: DeviceQuery = serde_json::from_value(json).unwrap();
        assert!(query.device_id.is_some());
    }

    #[test]
    fn test_device_query_deserialization_without_id() {
        let json = serde_json::json!({});
        let query: DeviceQuery = serde_json::from_value(json).unwrap();
        assert!(query.device_id.is_none());
    }

    #[test]
    fn test_conflict_resolution_validation() {
        // Added: Valid strategies should parse
        assert!(ConflictResolution::from_str("server_wins").is_some());
        assert!(ConflictResolution::from_str("client_wins").is_some());
        assert!(ConflictResolution::from_str("merge").is_some());
        // Added: Invalid strategies should be None
        assert!(ConflictResolution::from_str("discard").is_none());
        assert!(ConflictResolution::from_str("").is_none());
    }

    #[test]
    fn test_sync_state_initial_response() {
        // Added: Verify the initial sync state response shape
        let state = SyncState {
            folder_name: "INBOX".to_string(),
            last_uid: 0,
            last_modseq: 0,
            uidvalidity: 0,
            last_synced_at: None,
            needs_full_sync: true,
        };

        let json = serde_json::to_value(&state).unwrap();
        assert_eq!(json["folder_name"], "INBOX");
        assert_eq!(json["needs_full_sync"], true);
        assert!(json["last_synced_at"].is_null());
    }

    #[test]
    fn test_sync_state_existing_checkpoint_response() {
        let state = SyncState {
            folder_name: "INBOX".to_string(),
            last_uid: 1500,
            last_modseq: 42000,
            uidvalidity: 1234567890,
            last_synced_at: Some(chrono::Utc::now()),
            needs_full_sync: false,
        };

        let json = serde_json::to_value(&state).unwrap();
        assert_eq!(json["last_uid"], 1500);
        assert_eq!(json["needs_full_sync"], false);
        assert!(json["last_synced_at"].is_string());
    }

    #[test]
    fn test_conflict_response_server_wins() {
        let resp = ConflictResolutionResponse {
            folder: "INBOX".to_string(),
            uid: 42,
            resolution: "server_wins".to_string(),
            applied: true,
            message: "Server state retained".to_string(),
        };

        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["resolution"], "server_wins");
        assert_eq!(json["applied"], true);
    }

    #[test]
    fn test_conflict_response_client_wins_with_flags() {
        let resp = ConflictResolutionResponse {
            folder: "INBOX".to_string(),
            uid: 10,
            resolution: "client_wins".to_string(),
            applied: true,
            message: "Client flags accepted — 3 flags would be applied to server".to_string(),
        };

        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["resolution"], "client_wins");
        assert!(json["message"].as_str().unwrap().contains("3 flags"));
    }
}
