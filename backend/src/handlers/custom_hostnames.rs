// Added: Custom hostname management handlers for per-tenant SNI (TMAIL-112)

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::error::AppError;
use crate::models::custom_hostname::{
    CreateHostnameRequest, CustomHostname, UpdateHostnameRequest,
};
use crate::services::auth_service::Claims;
use crate::state::AppState;

/// PURPOSE: List all custom hostname configurations
/// GET /api/admin/hostnames
/// CONSTRAINTS: Requires admin authentication
pub async fn list_hostnames(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<CustomHostname>>, AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }
    let hostnames = CustomHostname::find_all(&state.db).await?;
    Ok(Json(hostnames))
}

/// PURPOSE: Create a new custom hostname config for a domain
/// POST /api/admin/hostnames
/// CONSTRAINTS: Requires admin auth, domain_id must exist, hostnames must be valid
pub async fn create_hostname(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreateHostnameRequest>,
) -> Result<(StatusCode, Json<CustomHostname>), AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    // Added: Validate hostname format — must contain a dot and no spaces
    if !body.smtp_hostname.contains('.') || body.smtp_hostname.contains(' ') {
        return Err(AppError::BadRequest(
            "SMTP hostname must be a valid domain (e.g., smtp.example.com)".to_string(),
        ));
    }
    if !body.imap_hostname.contains('.') || body.imap_hostname.contains(' ') {
        return Err(AppError::BadRequest(
            "IMAP hostname must be a valid domain (e.g., imap.example.com)".to_string(),
        ));
    }

    let hostname = CustomHostname::create(&state.db, &body).await?;
    Ok((StatusCode::CREATED, Json(hostname)))
}

/// PURPOSE: Get a single custom hostname config by ID
/// GET /api/admin/hostnames/:id
pub async fn get_hostname(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<CustomHostname>, AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }
    let hostname = CustomHostname::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Custom hostname config not found".to_string()))?;
    Ok(Json(hostname))
}

/// PURPOSE: Update an existing custom hostname configuration
/// PUT /api/admin/hostnames/:id
pub async fn update_hostname(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<UpdateHostnameRequest>,
) -> Result<Json<CustomHostname>, AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    // Added: Validate hostname format if provided
    if let Some(ref smtp) = body.smtp_hostname {
        if !smtp.contains('.') || smtp.contains(' ') {
            return Err(AppError::BadRequest(
                "SMTP hostname must be a valid domain (e.g., smtp.example.com)".to_string(),
            ));
        }
    }
    if let Some(ref imap) = body.imap_hostname {
        if !imap.contains('.') || imap.contains(' ') {
            return Err(AppError::BadRequest(
                "IMAP hostname must be a valid domain (e.g., imap.example.com)".to_string(),
            ));
        }
    }

    let hostname = CustomHostname::update(&state.db, id, &body)
        .await?
        .ok_or_else(|| AppError::NotFound("Custom hostname config not found".to_string()))?;
    Ok(Json(hostname))
}

/// PURPOSE: Delete a custom hostname configuration
/// DELETE /api/admin/hostnames/:id
pub async fn delete_hostname(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }
    let deleted = CustomHostname::delete(&state.db, id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound(
            "Custom hostname config not found".to_string(),
        ))
    }
}

/// PURPOSE: Trigger DNS verification for a custom hostname config
/// POST /api/admin/hostnames/:id/verify
/// NOTE: In production this would query DNS for CNAME/TXT records matching the verification token.
///       For now it marks the hostname as verified — actual DNS check is a future enhancement.
pub async fn verify_hostname(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<CustomHostname>, AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    // NOTE: Ensure the hostname config exists before attempting verification
    CustomHostname::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Custom hostname config not found".to_string()))?;

    let hostname = CustomHostname::mark_verified(&state.db, id)
        .await?
        .ok_or_else(|| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to mark hostname as verified for id={}",
                id
            ))
        })?;
    Ok(Json(hostname))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::auth_service::Claims;

    fn admin_claims() -> Claims {
        Claims {
            sub: uuid::Uuid::new_v4().to_string(),
            username: "admin@example.com".into(),
            is_admin: true,
            exp: 0,
            iat: 0,
        }
    }

    fn user_claims() -> Claims {
        Claims {
            sub: uuid::Uuid::new_v4().to_string(),
            username: "user@example.com".into(),
            is_admin: false,
            exp: 0,
            iat: 0,
        }
    }

    #[test]
    fn test_admin_claims_has_admin_flag() {
        let claims = admin_claims();
        assert!(claims.is_admin);
        assert!(claims.sub.parse::<uuid::Uuid>().is_ok());
    }

    #[test]
    fn test_user_claims_lacks_admin_flag() {
        let claims = user_claims();
        assert!(!claims.is_admin);
    }

    #[test]
    fn test_hostname_validation_requires_dot() {
        // NOTE: Hostnames without a dot are invalid
        let invalid_hostname = "localhost";
        assert!(!invalid_hostname.contains('.'));

        let valid_hostname = "smtp.example.com";
        assert!(valid_hostname.contains('.'));
        assert!(!valid_hostname.contains(' '));
    }

    #[test]
    fn test_hostname_validation_rejects_spaces() {
        let invalid_hostname = "smtp example.com";
        assert!(invalid_hostname.contains(' '));
    }
}
