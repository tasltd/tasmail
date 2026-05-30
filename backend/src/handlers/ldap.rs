// Added: LDAP/Active Directory configuration handlers for TMAIL-100.
// Updated (2026-05-28): bind passwords are now AES-256-GCM encrypted at rest
// via EncryptionService, and trigger_sync calls the real ldap3-backed service
// instead of returning placeholder zero counts. A test-connection endpoint
// was added for the admin UI's "Test connection" button.
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::ldap_config::{
    CreateLdapConfigRequest, LdapConfiguration, LdapSyncLog, UpdateLdapConfigRequest,
};
use crate::services::audit::audit_admin_action;
use crate::services::auth_service::{self, Claims};
use crate::services::ldap_service::LdapService;
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

    // Fix (TMAIL-100): encrypt the bind password before it hits the DB so the
    // service-account credentials are never stored in plaintext.
    let encrypted_password = state
        .encryption
        .encrypt(&request.bind_password)
        .map_err(AppError::Internal)?;

    let config = LdapConfiguration::create(&state.db, &request, &encrypted_password).await?;

    // Added (TMAIL-307): audit-log LDAP config creation. Bind password is
    // intentionally omitted from the audit details.
    audit_admin_action(
        &state.db,
        &claims,
        "ldap_config.create",
        Some("ldap_configuration"),
        Some(&config.id.to_string()),
        Some(serde_json::json!({
            "name": config.name,
            "server_url": config.server_url,
            "bind_dn": config.bind_dn,
            "search_base": config.search_base,
        })),
    )
    .await;

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

    // Fix (TMAIL-100): when an admin rotates the bind password, encrypt the new
    // value before persisting. When the field is absent we leave the existing
    // ciphertext untouched.
    let encrypted_password = match request.bind_password.as_deref() {
        Some(pw) if !pw.trim().is_empty() => {
            Some(state.encryption.encrypt(pw).map_err(AppError::Internal)?)
        }
        _ => None,
    };

    let config = LdapConfiguration::update(
        &state.db,
        id,
        &request,
        encrypted_password.as_deref(),
    )
    .await?;

    // Added (TMAIL-307): audit-log LDAP config update. Include whether the
    // bind password was rotated, but never the value itself.
    audit_admin_action(
        &state.db,
        &claims,
        "ldap_config.update",
        Some("ldap_configuration"),
        Some(&id.to_string()),
        Some(serde_json::json!({
            "bind_password_rotated": encrypted_password.is_some(),
            "name": request.name,
            "server_url": request.server_url,
        })),
    )
    .await;

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

    // Added (TMAIL-307): audit-log LDAP config delete.
    audit_admin_action(
        &state.db,
        &claims,
        "ldap_config.delete",
        Some("ldap_configuration"),
        Some(&id.to_string()),
        None,
    )
    .await;

    Ok(StatusCode::NO_CONTENT)
}

/// PURPOSE: Verify the configured service account can bind. Returns 204 on
/// success, 400 with the LDAP error message on failure.
/// EXTERNAL: POST /api/admin/ldap/:id/test
pub async fn test_connection(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    auth_service::require_admin(&claims)?;
    let config = LdapConfiguration::get_by_id(&state.db, id)
        .await
        .map_err(|_| AppError::NotFound(format!("LDAP configuration {id} not found")))?;

    let bind_password = state
        .encryption
        .decrypt(&config.bind_password_encrypted)
        .map_err(|e| {
            AppError::BadRequest(format!(
                "stored bind password could not be decrypted — re-save the configuration: {e}"
            ))
        })?;

    LdapService::test_connection(&config.server_url, &config.bind_dn, &bind_password)
        .await
        .map_err(|e| AppError::BadRequest(format!("LDAP bind failed: {e}")))?;

    Ok(StatusCode::NO_CONTENT)
}

/// PURPOSE: Trigger a manual LDAP sync for a specific configuration.
/// CONSTRAINTS: Connects to the directory, runs the configured search, then
/// creates / updates / soft-disables `mailboxes` rows to match. Errors per
/// row are recorded in `ldap_sync_logs.errors`; the run is marked completed
/// regardless so the admin sees what happened.
/// EXTERNAL: POST /api/admin/ldap/:id/sync
pub async fn trigger_sync(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<(StatusCode, Json<LdapSyncLog>), AppError> {
    auth_service::require_admin(&claims)?; // TMAIL-210
    let config = LdapConfiguration::get_by_id(&state.db, id)
        .await
        .map_err(|_| AppError::NotFound(format!("LDAP configuration {id} not found")))?;

    if !config.active {
        return Err(AppError::BadRequest(
            "Cannot sync an inactive LDAP configuration. Enable it first.".to_string(),
        ));
    }

    let sync_log = LdapSyncLog::create(&state.db, id).await?;

    // Fix (TMAIL-100): replace the placeholder zero-count result with a real
    // ldap3 search + apply_sync. Failures at the bind/search layer surface as
    // a "failed" log entry with the error captured; per-row failures land in
    // the sync log's errors array.
    let bind_password = match state.encryption.decrypt(&config.bind_password_encrypted) {
        Ok(pw) => pw,
        Err(e) => {
            let errors = json!([{
                "stage": "decrypt_bind_password",
                "error": e.to_string(),
            }]);
            let failed = LdapSyncLog::complete(&state.db, sync_log.id, "failed", 0, 0, 0, &errors)
                .await?;
            LdapConfiguration::update_sync_status(&state.db, id, "failed", 0).await?;
            return Ok((StatusCode::CREATED, Json(failed)));
        }
    };

    let users = match LdapService::search_users(
        &config.server_url,
        &config.bind_dn,
        &bind_password,
        &config.search_base,
        &config.search_filter,
        &config.email_attribute,
        &config.name_attribute,
    )
    .await
    {
        Ok(u) => u,
        Err(e) => {
            let errors = json!([{
                "stage": "ldap_search",
                "error": e.to_string(),
            }]);
            let failed = LdapSyncLog::complete(&state.db, sync_log.id, "failed", 0, 0, 0, &errors)
                .await?;
            LdapConfiguration::update_sync_status(&state.db, id, "failed", 0).await?;
            return Ok((StatusCode::CREATED, Json(failed)));
        }
    };

    let result = LdapService::apply_sync(&state.db, users).await;
    let total_synced = result.created + result.updated;
    let completed_log = LdapSyncLog::complete(
        &state.db,
        sync_log.id,
        "completed",
        result.created,
        result.updated,
        result.disabled,
        &serde_json::Value::Array(result.errors.clone()),
    )
    .await?;

    LdapConfiguration::update_sync_status(&state.db, id, "completed", total_synced).await?;

    // Added (TMAIL-307): audit-log LDAP sync run — sync mutates the mailboxes
    // table so admins should see who triggered each run + the per-run counts.
    audit_admin_action(
        &state.db,
        &claims,
        "ldap_config.sync",
        Some("ldap_configuration"),
        Some(&id.to_string()),
        Some(serde_json::json!({
            "created": result.created,
            "updated": result.updated,
            "disabled": result.disabled,
            "errors": result.errors.len(),
        })),
    )
    .await;

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
