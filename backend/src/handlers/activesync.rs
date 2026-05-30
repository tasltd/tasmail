// Added: ActiveSync device management handlers for TMAIL-130
// PURPOSE: CRUD endpoints for managing user's ActiveSync devices and admin sync policies
// NOTE: TASMail manages device metadata; actual ActiveSync protocol handled by Z-Push or similar proxy

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::error::AppError;
use crate::models::activesync::{
    ActiveSyncDevice, ActiveSyncPolicy, CreatePolicyRequest, RegisterDeviceRequest,
    UpdatePolicyRequest,
};
use crate::services::audit::audit_admin_action;
use crate::services::auth_service::Claims;
use crate::state::AppState;

// --- Device endpoints (user-scoped) ---

/// PURPOSE: List all ActiveSync devices for the current user
/// GET /api/activesync/devices
pub async fn list_devices(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<ActiveSyncDevice>>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let devices = ActiveSyncDevice::list_by_user(&state.db, user_id).await?;
    Ok(Json(devices))
}

/// PURPOSE: Register a new ActiveSync device
/// POST /api/activesync/devices
pub async fn register_device(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<RegisterDeviceRequest>,
) -> Result<(StatusCode, Json<ActiveSyncDevice>), AppError> {
    let user_id = parse_user_id(&claims)?;

    // Added: Validate required fields are not empty
    if body.device_id.trim().is_empty() {
        return Err(AppError::BadRequest("device_id cannot be empty".to_string()));
    }
    if body.device_type.trim().is_empty() {
        return Err(AppError::BadRequest("device_type cannot be empty".to_string()));
    }

    let device = ActiveSyncDevice::register(
        &state.db,
        user_id,
        &body.device_id,
        &body.device_type,
        body.device_name.as_deref(),
        body.device_os.as_deref(),
    )
    .await?;

    Ok((StatusCode::CREATED, Json(device)))
}

/// PURPOSE: Block an ActiveSync device
/// POST /api/activesync/devices/{id}/block
pub async fn block_device(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<ActiveSyncDevice>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let device = ActiveSyncDevice::update_status(&state.db, id, user_id, "blocked")
        .await?
        .ok_or_else(|| AppError::NotFound("Device not found".to_string()))?;
    Ok(Json(device))
}

/// PURPOSE: Allow an ActiveSync device
/// POST /api/activesync/devices/{id}/allow
pub async fn allow_device(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<ActiveSyncDevice>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let device = ActiveSyncDevice::update_status(&state.db, id, user_id, "allowed")
        .await?
        .ok_or_else(|| AppError::NotFound("Device not found".to_string()))?;
    Ok(Json(device))
}

/// PURPOSE: Remote wipe an ActiveSync device
/// POST /api/activesync/devices/{id}/wipe
pub async fn wipe_device(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<ActiveSyncDevice>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let device = ActiveSyncDevice::update_status(&state.db, id, user_id, "wiped")
        .await?
        .ok_or_else(|| AppError::NotFound("Device not found".to_string()))?;
    Ok(Json(device))
}

/// PURPOSE: Remove a device registration
/// DELETE /api/activesync/devices/{id}
pub async fn delete_device(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, AppError> {
    let user_id = parse_user_id(&claims)?;
    let deleted = ActiveSyncDevice::delete(&state.db, id, user_id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("Device not found".to_string()))
    }
}

// --- Policy endpoints (admin-scoped) ---

/// PURPOSE: List all ActiveSync policies
/// GET /api/admin/activesync/policies
pub async fn list_policies(
    State(state): State<AppState>,
    axum::Extension(_claims): axum::Extension<Claims>,
) -> Result<Json<Vec<ActiveSyncPolicy>>, AppError> {
    let policies = ActiveSyncPolicy::list(&state.db).await?;
    Ok(Json(policies))
}

/// PURPOSE: Create a new ActiveSync policy
/// POST /api/admin/activesync/policies
pub async fn create_policy(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreatePolicyRequest>,
) -> Result<(StatusCode, Json<ActiveSyncPolicy>), AppError> {
    // Added: Validate policy name is not empty
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("Policy name cannot be empty".to_string()));
    }

    let policy = ActiveSyncPolicy::create(
        &state.db,
        &body.name,
        body.require_encryption.unwrap_or(true),
        body.max_inactivity_lock_mins,
        body.min_password_length,
        body.allow_simple_password.unwrap_or(false),
        body.max_failed_password_attempts,
        body.is_default.unwrap_or(false),
    )
    .await?;

    // Added (TMAIL-307): audit-log ActiveSync policy creation.
    audit_admin_action(
        &state.db,
        &claims,
        "activesync_policy.create",
        Some("activesync_policy"),
        Some(&policy.id.to_string()),
        Some(serde_json::json!({
            "name": policy.name,
            "is_default": policy.is_default,
        })),
    )
    .await;

    Ok((StatusCode::CREATED, Json(policy)))
}

/// PURPOSE: Update an existing ActiveSync policy
/// PUT /api/admin/activesync/policies/{id}
pub async fn update_policy(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<UpdatePolicyRequest>,
) -> Result<Json<ActiveSyncPolicy>, AppError> {
    // Added: Fetch existing policy to merge partial updates
    let existing = ActiveSyncPolicy::list(&state.db)
        .await?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| AppError::NotFound("Policy not found".to_string()))?;

    let name = body.name.unwrap_or(existing.name);
    if name.trim().is_empty() {
        return Err(AppError::BadRequest("Policy name cannot be empty".to_string()));
    }

    let require_encryption = body.require_encryption.unwrap_or(existing.require_encryption);
    let max_inactivity_lock_mins = match body.max_inactivity_lock_mins {
        Some(val) => val,
        None => existing.max_inactivity_lock_mins,
    };
    let min_password_length = match body.min_password_length {
        Some(val) => val,
        None => existing.min_password_length,
    };
    let allow_simple_password = body.allow_simple_password.unwrap_or(existing.allow_simple_password);
    let max_failed_password_attempts = match body.max_failed_password_attempts {
        Some(val) => val,
        None => existing.max_failed_password_attempts,
    };
    let is_default = body.is_default.unwrap_or(existing.is_default);

    let policy = ActiveSyncPolicy::update(
        &state.db,
        id,
        &name,
        require_encryption,
        max_inactivity_lock_mins,
        min_password_length,
        allow_simple_password,
        max_failed_password_attempts,
        is_default,
    )
    .await?
    .ok_or_else(|| AppError::NotFound("Policy not found".to_string()))?;

    // Added (TMAIL-307): audit-log ActiveSync policy update.
    audit_admin_action(
        &state.db,
        &claims,
        "activesync_policy.update",
        Some("activesync_policy"),
        Some(&id.to_string()),
        Some(serde_json::json!({
            "name": name,
            "is_default": is_default,
            "require_encryption": require_encryption,
        })),
    )
    .await;

    Ok(Json(policy))
}

/// PURPOSE: Delete an ActiveSync policy
/// DELETE /api/admin/activesync/policies/{id}
pub async fn delete_policy(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, AppError> {
    let deleted = ActiveSyncPolicy::delete(&state.db, id).await?;
    if deleted {
        // Added (TMAIL-307): audit-log ActiveSync policy delete.
        audit_admin_action(
            &state.db,
            &claims,
            "activesync_policy.delete",
            Some("activesync_policy"),
            Some(&id.to_string()),
            None,
        )
        .await;
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("Policy not found".to_string()))
    }
}

fn parse_user_id(claims: &Claims) -> Result<uuid::Uuid, AppError> {
    claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid user ID in JWT claims")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::auth_service::Claims;

    #[test]
    fn test_parse_user_id_valid() {
        let claims = Claims {
            sub: uuid::Uuid::new_v4().to_string(),
            username: "test@example.com".into(),
            is_admin: false,
            is_compliance_officer: false,
            exp: 0,
            iat: 0,
        };
        assert!(parse_user_id(&claims).is_ok());
    }

    #[test]
    fn test_parse_user_id_invalid() {
        let claims = Claims {
            sub: "not-a-uuid".into(),
            username: "test@example.com".into(),
            is_admin: false,
            is_compliance_officer: false,
            exp: 0,
            iat: 0,
        };
        assert!(parse_user_id(&claims).is_err());
    }

    #[test]
    fn test_register_device_request_deserialization() {
        let json = serde_json::json!({
            "device_id": "TEST123",
            "device_type": "iPhone",
            "device_name": "Test Device",
            "device_os": "iOS 18"
        });

        let request: RegisterDeviceRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.device_id, "TEST123");
        assert_eq!(request.device_type, "iPhone");
        assert_eq!(request.device_name, Some("Test Device".to_string()));
    }

    #[test]
    fn test_create_policy_request_deserialization() {
        let json = serde_json::json!({
            "name": "Test Policy",
            "require_encryption": true,
            "min_password_length": 8
        });

        let request: CreatePolicyRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.name, "Test Policy");
        assert_eq!(request.require_encryption, Some(true));
        assert_eq!(request.min_password_length, Some(8));
    }

    #[test]
    fn test_update_policy_request_partial() {
        let json = serde_json::json!({
            "name": "Renamed"
        });

        let request: UpdatePolicyRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.name, Some("Renamed".to_string()));
        assert!(request.require_encryption.is_none());
        assert!(request.is_default.is_none());
    }

    #[test]
    fn test_update_policy_request_empty() {
        let json = serde_json::json!({});
        let request: UpdatePolicyRequest = serde_json::from_value(json).unwrap();
        assert!(request.name.is_none());
        assert!(request.require_encryption.is_none());
        assert!(request.allow_simple_password.is_none());
        assert!(request.is_default.is_none());
    }
}
