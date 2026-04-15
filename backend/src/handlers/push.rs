// Added: Push notification device management handlers for TMAIL-50
// PURPOSE: CRUD endpoints for registering/unregistering push devices and sending test notifications
// NOTE: RLS at DB level enforces user-scoped access to push_devices

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;

use crate::error::AppError;
use crate::models::push_notification::{
    PushDevice, PushNotificationPayload, PushPlatform, RegisterPushDeviceRequest,
};
use crate::services::auth_service::Claims;
use crate::services::push_service;
use crate::state::AppState;

/// Added: Response for test notification endpoint (TMAIL-50)
#[derive(Debug, Serialize)]
pub struct TestNotificationResponse {
    pub devices_notified: usize,
    pub successes: usize,
    pub failures: usize,
}

/// PURPOSE: Register a push notification device token
/// POST /api/push/register
pub async fn register_device(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<RegisterPushDeviceRequest>,
) -> Result<(StatusCode, Json<PushDevice>), AppError> {
    let user_id = parse_user_id(&claims)?;

    // Added: Validate platform is one of the supported values
    if PushPlatform::from_str(&body.platform).is_none() {
        return Err(AppError::BadRequest(
            "Invalid platform: must be 'fcm', 'apns', or 'web'".to_string(),
        ));
    }

    // Added: Validate device_token is not empty
    if body.device_token.trim().is_empty() {
        return Err(AppError::BadRequest(
            "device_token cannot be empty".to_string(),
        ));
    }

    let device = PushDevice::register(
        &state.db,
        user_id,
        &body.platform,
        &body.device_token,
        body.device_name.as_deref(),
        body.app_version.as_deref(),
    )
    .await?;

    Ok((StatusCode::CREATED, Json(device)))
}

/// PURPOSE: List all registered push devices for the current user
/// GET /api/push/devices
pub async fn list_devices(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<PushDevice>>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let devices = PushDevice::list_by_user(&state.db, user_id).await?;
    Ok(Json(devices))
}

/// PURPOSE: Unregister (delete) a push device
/// DELETE /api/push/devices/{id}
pub async fn unregister_device(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, AppError> {
    let user_id = parse_user_id(&claims)?;
    let deleted = PushDevice::delete(&state.db, id, user_id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("Device not found".to_string()))
    }
}

/// PURPOSE: Send a test notification to all of the current user's active devices
/// POST /api/push/test
pub async fn test_notification(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<TestNotificationResponse>, AppError> {
    let user_id = parse_user_id(&claims)?;

    let payload = PushNotificationPayload {
        title: "TASMail Test Notification".to_string(),
        body: Some("If you see this, push notifications are working!".to_string()),
        data: Some(serde_json::json!({"type": "test"})),
    };

    let results = push_service::send_notification(&state.db, &state.config, user_id, &payload)
        .await
        .map_err(|e| AppError::Internal(e))?;

    let successes = results.iter().filter(|r| r.delivered).count();
    let failures = results.len() - successes;

    Ok(Json(TestNotificationResponse {
        devices_notified: results.len(),
        successes,
        failures,
    }))
}

// Added: Parse user UUID from JWT claims
fn parse_user_id(claims: &Claims) -> Result<uuid::Uuid, AppError> {
    claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid user ID in JWT claims")))
}
