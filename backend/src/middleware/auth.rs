use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::Response,
};

use crate::error::AppError;
use crate::services::auth_service::{validate_access_token, Claims};
use crate::state::AppState;

/// Extract JWT claims from the Authorization header and inject into request extensions
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

    // Inject claims into request extensions for handlers to use
    req.extensions_mut().insert(claims);

    Ok(next.run(req).await)
}

/// Extract claims from request extensions (use in handlers)
pub fn extract_claims(req: &Request) -> Result<&Claims, AppError> {
    req.extensions()
        .get::<Claims>()
        .ok_or_else(|| AppError::Unauthorized("No auth claims in request".to_string()))
}
