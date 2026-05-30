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
///
/// Changed (TMAIL-309): this middleware no longer attempts to SET RLS session vars
/// on a one-off pool connection — that pattern was a no-op because each handler
/// query acquires a fresh connection. The actual RLS plumbing now lives in
/// `middleware::rls_context::rls_context_middleware`, which runs immediately
/// after this one and parks claims+state in request extensions so the
/// `RlsConn` extractor can lazily acquire a primed connection per handler.
///
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

    // Inject claims into request extensions for handlers to use.
    // The downstream `rls_context_middleware` reads these claims to populate
    // the RLS request context (see TMAIL-309).
    req.extensions_mut().insert(claims);

    Ok(next.run(req).await)
}

/// Extract claims from request extensions (use in handlers)
pub fn extract_claims(req: &Request) -> Result<&Claims, AppError> {
    req.extensions()
        .get::<Claims>()
        .ok_or_else(|| AppError::Unauthorized("No auth claims in request".to_string()))
}
