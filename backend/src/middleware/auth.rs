use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::Response,
};

use crate::error::AppError;
use crate::services::auth_service::{validate_access_token, Claims};
use crate::state::AppState;

/// Extract JWT claims from the Authorization header and inject into request extensions.
/// Also sets PostgreSQL session variables for Row-Level Security enforcement.
pub async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("Missing authorization header".to_string()))?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| AppError::Unauthorized("Invalid authorization format".to_string()))?;

    let claims = validate_access_token(&state.config.jwt, token)?;

    // Added: Set RLS session variables for database-level row isolation
    set_rls_context(&state, &claims).await?;

    // Inject claims into request extensions for handlers to use
    req.extensions_mut().insert(claims);

    Ok(next.run(req).await)
}

/// Set PostgreSQL session-level variables for RLS policy evaluation.
/// Uses SET LOCAL so the settings are scoped to the current transaction.
async fn set_rls_context(state: &AppState, claims: &Claims) -> Result<(), AppError> {
    // NOTE: Using raw SQL with parameterized values is not possible for SET commands,
    // so we validate the UUID format to prevent injection.
    let mailbox_id = &claims.sub;
    uuid::Uuid::parse_str(mailbox_id)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID format in claims")))?;

    let is_admin = if claims.is_admin { "true" } else { "false" };

    sqlx::query(&format!(
        "SET app.mailbox_id = '{}'; SET app.is_admin = '{}';",
        mailbox_id, is_admin
    ))
    .execute(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("Failed to set RLS context: {}", e);
        AppError::Internal(anyhow::anyhow!("Failed to set RLS context"))
    })?;

    Ok(())
}

/// Extract claims from request extensions (use in handlers)
pub fn extract_claims(req: &Request) -> Result<&Claims, AppError> {
    req.extensions()
        .get::<Claims>()
        .ok_or_else(|| AppError::Unauthorized("No auth claims in request".to_string()))
}
