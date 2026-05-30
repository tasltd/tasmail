// TMAIL-165 admin endpoints for feature_flags.
//
//   GET    /api/admin/feature-flags             — list all flags (admin)
//   PATCH  /api/admin/feature-flags/{key}       — update enabled / value (admin)
//   GET    /api/feature-flags                   — public subset (no auth, for SPA)

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::feature_flag::FeatureFlag;
use crate::services::audit::audit_admin_action;
use crate::services::auth_service::{self, Claims};
use crate::services::feature_flags as flag_cache;
use crate::state::AppState;

pub async fn list_flags(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<FeatureFlag>>, AppError> {
    // Fix: TMAIL-210 — gate admin endpoints on the is_admin claim.
    auth_service::require_admin(&claims)?;
    let flags = FeatureFlag::list_all(&state.db).await?;
    Ok(Json(flags))
}

#[derive(Debug, Deserialize)]
pub struct UpdateFlagRequest {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub value: Option<serde_json::Value>,
}

pub async fn update_flag(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(key): Path<String>,
    Json(body): Json<UpdateFlagRequest>,
) -> Result<Json<FeatureFlag>, AppError> {
    // Fix: TMAIL-210 — admin-only.
    auth_service::require_admin(&claims)?;
    // Validate the flag exists before touching it (gives a 404 instead of a silent UPDATE 0 ROWS).
    if FeatureFlag::find(&state.db, &key).await?.is_none() {
        return Err(AppError::NotFound(format!("feature flag '{}' not found", key)));
    }
    if body.enabled.is_none() && body.value.is_none() {
        return Err(AppError::BadRequest("Specify enabled and/or value".into()));
    }
    let actor = Uuid::parse_str(&claims.sub).ok();
    let updated = FeatureFlag::upsert(&state.db, &key, body.enabled, body.value.clone(), actor).await?;
    // Bust the per-flag cache so the new state is visible immediately to all workers.
    flag_cache::invalidate(&state, &key).await;

    // Added (TMAIL-307): audit-log every flag toggle / value change. Flags
    // gate billing, signup, and self-host paths — flips need a compliance trail.
    audit_admin_action(
        &state.db,
        &claims,
        "feature_flag.update",
        Some("feature_flag"),
        Some(&key),
        Some(serde_json::json!({
            "enabled": body.enabled,
            "value": body.value,
        })),
    )
    .await;

    Ok(Json(updated))
}

/// PURPOSE: Public feature-flag list — only flags marked is_public=true.
/// Used by the SPA's signup/landing page to know which onboarding paths exist.
pub async fn list_public_flags(
    State(state): State<AppState>,
) -> Result<Json<Vec<FeatureFlag>>, AppError> {
    let flags = FeatureFlag::list_public(&state.db).await?;
    Ok(Json(flags))
}
