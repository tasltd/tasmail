// Added: Branding handlers for white-label customization (TMAIL-111)
use axum::{extract::State, Json};

use crate::error::AppError;
use crate::models::branding::{Branding, UpdateBrandingRequest};
use crate::state::AppState;

/// GET /api/branding — Get current branding configuration (PUBLIC, no auth required)
/// PURPOSE: Frontend loads branding at app startup to apply custom logo, colors, and app name
pub async fn get_branding(
    State(state): State<AppState>,
) -> Result<Json<Branding>, AppError> {
    let branding = Branding::get_current(&state.db).await?;
    Ok(Json(branding))
}

/// PUT /api/admin/branding — Update branding settings (admin only)
/// PURPOSE: Admins customize instance appearance via the settings UI
/// CONSTRAINTS: Requires admin authentication via auth_middleware
pub async fn update_branding(
    State(state): State<AppState>,
    Json(request): Json<UpdateBrandingRequest>,
) -> Result<Json<Branding>, AppError> {
    let branding = Branding::update(&state.db, &request).await?;
    Ok(Json(branding))
}

/// POST /api/admin/branding/reset — Reset branding to defaults (admin only)
/// PURPOSE: Allows admins to revert all branding to factory defaults
pub async fn reset_branding(
    State(state): State<AppState>,
) -> Result<Json<Branding>, AppError> {
    let branding = Branding::reset_to_defaults(&state.db).await?;
    Ok(Json(branding))
}
