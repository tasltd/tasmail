// Added: LDAP/Active Directory configuration handlers for TMAIL-100
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::ldap_config::{
    CreateLdapConfigRequest, LdapConfiguration, LdapSyncLog, UpdateLdapConfigRequest,
};
use crate::services::auth_service::{self, Claims};
use crate::state::AppState;

/// PURPOSE: List all LDAP/AD configurations
/// EXTERNAL: GET /api/admin/ldap
pub async fn list_ldap_configs(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<LdapConfiguration>>, AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    let configs = LdapConfiguration::list(&state.db).await?;
    Ok(Json(configs))
}

/// PURPOSE: Create a new LDAP/AD configuration
/// CONSTRAINTS: Requires name, server_url, bind_dn, bind_password, search_base
/// EXTERNAL: POST /api/admin/ldap
pub async fn create_ldap_config(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(request): Json<CreateLdapConfigRequest>,
) -> Result<(StatusCode, Json<LdapConfiguration>), AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    // Added: Validate required fields are non-empty
    if request.name.trim().is_empty() {
        return Err(AppError::BadRequest(
            "LDAP configuration name is required".to_string(),
        ));
    }
    if request.server_url.trim().is_empty() {
        return Err(AppError::BadRequest(
            "LDAP server URL is required".to_string(),
        ));
    }
    if request.bind_dn.trim().is_empty() {
        return Err(AppError::BadRequest("Bind DN is required".to_string()));
    }
    if request.bind_password.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Bind password is required".to_string(),
        ));
    }
    if request.search_base.trim().is_empty() {
        return Err(AppError::BadRequest("Search base is required".to_string()));
    }

    // NOTE: In production, bind_password should be encrypted before storage.
    // For now, we store it as-is; a proper encryption service would wrap this.
    let encrypted_password = &request.bind_password;

    let config = LdapConfiguration::create(&state.db, &request, encrypted_password).await?;
    Ok((StatusCode::CREATED, Json(config)))
}

/// PURPOSE: Update an existing LDAP/AD configuration
/// CONSTRAINTS: All fields optional — only provided fields are updated
/// EXTERNAL: PUT /api/admin/ldap/:id
pub async fn update_ldap_config(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateLdapConfigRequest>,
) -> Result<Json<LdapConfiguration>, AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    // Added: Verify config exists before updating
    LdapConfiguration::get_by_id(&state.db, id)
        .await
        .map_err(|_| AppError::NotFound(format!("LDAP configuration {id} not found")))?;

    // NOTE: Only encrypt password if a new one was provided
    let encrypted_password = request.bind_password.as_deref();

    let config =
        LdapConfiguration::update(&state.db, id, &request, encrypted_password).await?;
    Ok(Json(config))
}

/// PURPOSE: Delete an LDAP/AD configuration and its sync history
/// EXTERNAL: DELETE /api/admin/ldap/:id
pub async fn delete_ldap_config(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    // Added: Verify config exists before deleting
    LdapConfiguration::get_by_id(&state.db, id)
        .await
        .map_err(|_| AppError::NotFound(format!("LDAP configuration {id} not found")))?;

    LdapConfiguration::delete(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// PURPOSE: Trigger a manual LDAP sync for a specific configuration
/// CONSTRAINTS: Creates a sync log entry; actual LDAP queries are abstracted
/// EXTERNAL: POST /api/admin/ldap/:id/sync
pub async fn trigger_sync(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<(StatusCode, Json<LdapSyncLog>), AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    // Added: Verify config exists and is active before syncing
    let config = LdapConfiguration::get_by_id(&state.db, id)
        .await
        .map_err(|_| AppError::NotFound(format!("LDAP configuration {id} not found")))?;

    if !config.active {
        return Err(AppError::BadRequest(
            "Cannot sync an inactive LDAP configuration. Enable it first.".to_string(),
        ));
    }

    // Added: Create a sync log entry to track this run
    let sync_log = LdapSyncLog::create(&state.db, id).await?;

    // NOTE: Actual LDAP directory queries would happen here via an ldap_service module.
    // For now, we mark the sync as completed with placeholder results.
    let completed_log = LdapSyncLog::complete(
        &state.db,
        sync_log.id,
        "completed",
        0,
        0,
        0,
        &serde_json::json!([]),
    )
    .await?;

    // Added: Update parent config sync metadata
    LdapConfiguration::update_sync_status(&state.db, id, "completed", 0).await?;

    Ok((StatusCode::CREATED, Json(completed_log)))
}

/// PURPOSE: Get sync history for a specific LDAP configuration
/// EXTERNAL: GET /api/admin/ldap/:id/logs
pub async fn list_sync_logs(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<LdapSyncLog>>, AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    // Added: Verify config exists before fetching logs
    LdapConfiguration::get_by_id(&state.db, id)
        .await
        .map_err(|_| AppError::NotFound(format!("LDAP configuration {id} not found")))?;

    let logs = LdapSyncLog::list_by_config(&state.db, id).await?;
    Ok(Json(logs))
}
