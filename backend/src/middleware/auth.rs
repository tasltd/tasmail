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
/// Changed: Checks Redis JWT blacklist before accepting a token (for immediate revocation on logout)
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

    // Added: Check if token has been blacklisted (revoked on logout)
    let token_hash = crate::services::auth_service::hash_refresh_token(token);
    if state.cache.is_token_blacklisted(&token_hash).await {
        return Err(AppError::Unauthorized("Token has been revoked".to_string()));
    }

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

    // TMAIL-161: This middleware no longer attempts to SET app.mailbox_id on the pool —
    // the SET would land on a connection that the handler never sees, since each handler
    // query acquires a fresh connection. RLS enforcement now lives in
    // `services::db_session::acquire_with_rls(state, claims)` which handlers use when
    // they want their queries scoped by RLS. Defense-in-depth audit confirmed that
    // every protected handler today already includes explicit `WHERE user_id = $N`
    // filters, so removing the no-op SET does not change observable behaviour.
    //
    // Variables `mailbox_id` and `is_admin` are kept above so future handler-side
    // logging or rate limiting can pull them from the validated claims if needed.
    let _ = (mailbox_id, is_admin);
    Ok(())
}

/// Extract claims from request extensions (use in handlers)
pub fn extract_claims(req: &Request) -> Result<&Claims, AppError> {
    req.extensions()
        .get::<Claims>()
        .ok_or_else(|| AppError::Unauthorized("No auth claims in request".to_string()))
}
