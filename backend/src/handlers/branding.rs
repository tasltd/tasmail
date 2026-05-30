// Added: Branding handlers for white-label customization (TMAIL-111)
// Changed: Added Redis caching for branding data — cached for 5 min, invalidated on update/reset
use axum::{extract::State, Json};

use crate::error::AppError;
use crate::models::branding::{Branding, UpdateBrandingRequest};
use crate::services::audit::audit_admin_action;
use crate::services::auth_service::{self, Claims};
use crate::state::AppState;

/// GET /api/branding — Get current branding configuration (PUBLIC, no auth required)
/// PURPOSE: Frontend loads branding at app startup to apply custom logo, colors, and app name
/// Changed: Checks Redis cache first, falls back to PostgreSQL on miss
pub async fn get_branding(
    State(state): State<AppState>,
) -> Result<Json<Branding>, AppError> {
    // Added: Check Redis cache first
    if let Some(cached) = state.cache.get_branding::<Branding>().await {
        return Ok(Json(cached));
    }

    let branding = Branding::get_current(&state.db).await?;

    // Added: Cache the result for subsequent requests
    state.cache.set_branding(&branding).await;

    Ok(Json(branding))
}

/// PUT /api/admin/branding — Update branding settings (admin only)
/// PURPOSE: Admins customize instance appearance via the settings UI
/// CONSTRAINTS: Requires admin authentication via auth_middleware
/// Changed: Invalidates Redis cache after update so next GET fetches fresh data
pub async fn update_branding(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(request): Json<UpdateBrandingRequest>,
) -> Result<Json<Branding>, AppError> {
    // Fix: TMAIL-210 — admin-only.
    auth_service::require_admin(&claims)?;
    let branding = Branding::update(&state.db, &request).await?;

    // Added: Invalidate cache so subsequent requests get the updated branding
    state.cache.invalidate_branding().await;

    // Added (TMAIL-307): audit-log branding update — visible-surface change.
    audit_admin_action(
        &state.db,
        &claims,
        "branding.update",
        Some("branding"),
        None,
        Some(serde_json::json!({
            "app_name": request.app_name,
            "logo_url": request.logo_url,
            "primary_color": request.primary_color,
            "accent_color": request.accent_color,
        })),
    )
    .await;

    Ok(Json(branding))
}

/// POST /api/admin/branding/reset — Reset branding to defaults (admin only)
/// PURPOSE: Allows admins to revert all branding to factory defaults
/// Changed: Invalidates Redis cache after reset
pub async fn reset_branding(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Branding>, AppError> {
    // Fix: TMAIL-210 — admin-only.
    auth_service::require_admin(&claims)?;
    let branding = Branding::reset_to_defaults(&state.db).await?;

    // Added: Invalidate cache after reset
    state.cache.invalidate_branding().await;

    // Added (TMAIL-307): audit-log branding reset to defaults.
    audit_admin_action(
        &state.db,
        &claims,
        "branding.reset",
        Some("branding"),
        None,
        None,
    )
    .await;

    Ok(Json(branding))
}
