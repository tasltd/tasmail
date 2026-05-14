// Added: Email archive handlers for Piler integration (TMAIL-107)
// PURPOSE: Admin endpoints for archive policy/config CRUD, user endpoints for archive search

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::archive::{
    ArchiveConfig, ArchivePolicy, ArchiveSearch, ArchiveSearchRequest,
    ArchiveSearchResult, CreateArchivePolicyRequest, UpdateArchiveConfigRequest,
    UpdateArchivePolicyRequest,
};
use crate::services::auth_service::{self, Claims};
use crate::state::AppState;

/// GET /api/admin/archive/policies — List all archive policies
pub async fn list_policies(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<ArchivePolicy>>, AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    let policies = ArchivePolicy::find_all(&state.db).await?;
    Ok(Json(policies))
}

/// POST /api/admin/archive/policies — Create a new archive policy
pub async fn create_policy(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreateArchivePolicyRequest>,
) -> Result<(StatusCode, Json<ArchivePolicy>), AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    // Added: Validate archive_after_days if provided
    if let Some(days) = body.archive_after_days {
        if days < 1 {
            return Err(AppError::BadRequest(
                "archive_after_days must be at least 1".to_string(),
            ));
        }
    }

    let policy = ArchivePolicy::create(&state.db, &body).await?;
    Ok((StatusCode::CREATED, Json(policy)))
}

/// PUT /api/admin/archive/policies/{id} — Update an archive policy
pub async fn update_policy(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateArchivePolicyRequest>,
) -> Result<Json<ArchivePolicy>, AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    // Added: Validate archive_after_days if provided in update
    if let Some(days) = body.archive_after_days {
        if days < 1 {
            return Err(AppError::BadRequest(
                "archive_after_days must be at least 1".to_string(),
            ));
        }
    }

    let policy = ArchivePolicy::update(&state.db, id, &body)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Archive policy '{}' not found", id)))?;
    Ok(Json(policy))
}

/// DELETE /api/admin/archive/policies/{id} — Delete an archive policy
pub async fn delete_policy(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    let deleted = ArchivePolicy::delete(&state.db, id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound(format!(
            "Archive policy '{}' not found",
            id
        )))
    }
}

/// GET /api/admin/archive/config — Get archive server configuration
pub async fn get_config(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Option<ArchiveConfig>>, AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    let config = ArchiveConfig::get(&state.db).await?;
    Ok(Json(config))
}

/// PUT /api/admin/archive/config — Update archive server configuration (Piler URL, API key, etc.)
pub async fn update_config(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<UpdateArchiveConfigRequest>,
) -> Result<Json<ArchiveConfig>, AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    // Added: Validate retention_years if provided
    if let Some(years) = body.retention_years {
        if years < 1 {
            return Err(AppError::BadRequest(
                "retention_years must be at least 1".to_string(),
            ));
        }
    }

    let config = ArchiveConfig::upsert(&state.db, &body).await?;
    Ok(Json(config))
}

/// POST /api/archive/search — Search archived emails (proxies to Piler API or returns mock)
pub async fn search_archive(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<ArchiveSearchRequest>,
) -> Result<Json<Vec<ArchiveSearchResult>>, AppError> {
    // NOTE: In production, this would proxy to the Piler search API
    // For now, return mock results and record the search in history
    let mock_results: Vec<ArchiveSearchResult> = vec![];

    // Added: Record the search in history for audit trail
    let filters = serde_json::json!({
        "date_from": body.date_from,
        "date_to": body.date_to,
        "sender": body.sender,
        "recipient": body.recipient,
    });

    // Fix: Parse user_id from claims.sub string to Uuid
    let user_id: uuid::Uuid = claims.sub.parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid user ID in JWT claims")))?;
    let _ = ArchiveSearch::create(
        &state.db,
        user_id,
        &body.query,
        Some(&filters),
        Some(mock_results.len() as i32),
    )
    .await;

    Ok(Json(mock_results))
}

/// GET /api/archive/search/history — Get user's archive search history
pub async fn search_history(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<ArchiveSearch>>, AppError> {
    // Fix: Parse user_id from claims.sub string to Uuid
    let user_id: uuid::Uuid = claims.sub.parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid user ID in JWT claims")))?;
    let history = ArchiveSearch::find_by_user(&state.db, user_id, 50).await?;
    Ok(Json(history))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_policy_request_deserialization() {
        // Added: Verify handler can deserialize a typical create request
        let json = r#"{
            "name": "Archive All",
            "match_criteria": {"domains": ["*"]},
            "archive_after_days": 90
        }"#;
        let req: CreateArchivePolicyRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "Archive All");
        assert_eq!(req.archive_after_days, Some(90));
    }

    #[test]
    fn test_update_config_request_deserialization() {
        // Added: Verify handler can deserialize config update
        let json = r#"{"piler_url": "https://piler.local", "enabled": true}"#;
        let req: UpdateArchiveConfigRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.piler_url, Some("https://piler.local".to_string()));
        assert_eq!(req.enabled, Some(true));
    }

    #[test]
    fn test_search_request_deserialization() {
        // Added: Verify search request parsing
        let json = r#"{"query": "invoice", "date_from": "2025-01-01"}"#;
        let req: ArchiveSearchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.query, "invoice");
        assert_eq!(req.date_from, Some("2025-01-01".to_string()));
    }

    #[test]
    fn test_search_request_minimal() {
        // Added: Search request with only required query field
        let json = r#"{"query": "hello world"}"#;
        let req: ArchiveSearchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.query, "hello world");
        assert!(req.sender.is_none());
    }

    #[test]
    fn test_update_policy_request_partial() {
        // Added: Partial update request with only enabled field
        let json = r#"{"enabled": false}"#;
        let req: UpdateArchivePolicyRequest = serde_json::from_str(json).unwrap();
        assert!(req.name.is_none());
        assert_eq!(req.enabled, Some(false));
    }

    #[test]
    fn test_update_policy_request_empty() {
        // Added: Empty update request should parse successfully
        let json = r#"{}"#;
        let req: UpdateArchivePolicyRequest = serde_json::from_str(json).unwrap();
        assert!(req.name.is_none());
        assert!(req.archive_after_days.is_none());
    }
}
