// Added: Branding handlers for white-label customization (TMAIL-111)
// Changed: Added Redis caching for branding data — cached for 5 min, invalidated on update/reset
use axum::{extract::State, Json};

use crate::error::AppError;
use crate::models::branding::{Branding, UpdateBrandingRequest};
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
    Json(request): Json<UpdateBrandingRequest>,
) -> Result<Json<Branding>, AppError> {
    let branding = Branding::update(&state.db, &request).await?;

    // Added: Invalidate cache so subsequent requests get the updated branding
    state.cache.invalidate_branding().await;

    Ok(Json(branding))
}

/// POST /api/admin/branding/reset — Reset branding to defaults (admin only)
/// PURPOSE: Allows admins to revert all branding to factory defaults
/// Changed: Invalidates Redis cache after reset
pub async fn reset_branding(
    State(state): State<AppState>,
) -> Result<Json<Branding>, AppError> {
    let branding = Branding::reset_to_defaults(&state.db).await?;

    // Added: Invalidate cache after reset
    state.cache.invalidate_branding().await;

    Ok(Json(branding))
}
