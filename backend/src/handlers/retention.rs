// Added: Retention policy and legal hold handlers for TMAIL-109

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::error::AppError;
use crate::models::retention_policy::{
    CreateLegalHoldRequest, CreateRetentionPolicyRequest, LegalHold, RetentionPolicy,
    UpdateRetentionPolicyRequest,
};
use crate::services::auth_service::Claims;
use crate::state::AppState;

/// PURPOSE: List all retention policies
/// GET /api/admin/retention
pub async fn list_retention_policies(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<RetentionPolicy>>, AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }
    let policies = RetentionPolicy::find_all(&state.db).await?;
    Ok(Json(policies))
}

/// PURPOSE: Create a new retention policy
/// POST /api/admin/retention
/// CONSTRAINTS: retention_days must be positive, name is required
pub async fn create_retention_policy(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreateRetentionPolicyRequest>,
) -> Result<(StatusCode, Json<RetentionPolicy>), AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    // Added: Validate retention_days is positive
    if body.retention_days <= 0 {
        return Err(AppError::BadRequest(
            "retention_days must be greater than 0".to_string(),
        ));
    }

    // Added: Validate name is not empty
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Policy name cannot be empty".to_string(),
        ));
    }

    let policy = RetentionPolicy::create(&state.db, &body).await?;
    Ok((StatusCode::CREATED, Json(policy)))
}

/// PURPOSE: Update an existing retention policy
/// PUT /api/admin/retention/:id
pub async fn update_retention_policy(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<UpdateRetentionPolicyRequest>,
) -> Result<Json<RetentionPolicy>, AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    // Added: Validate retention_days if provided
    if let Some(days) = body.retention_days {
        if days <= 0 {
            return Err(AppError::BadRequest(
                "retention_days must be greater than 0".to_string(),
            ));
        }
    }

    let policy = RetentionPolicy::update(&state.db, id, &body)
        .await?
        .ok_or_else(|| AppError::NotFound("Retention policy not found".to_string()))?;
    Ok(Json(policy))
}

/// PURPOSE: Delete a retention policy
/// DELETE /api/admin/retention/:id
pub async fn delete_retention_policy(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    if !RetentionPolicy::delete(&state.db, id).await? {
        return Err(AppError::NotFound("Retention policy not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

/// PURPOSE: List all legal holds
/// GET /api/admin/legal-holds
pub async fn list_legal_holds(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<LegalHold>>, AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }
    let holds = LegalHold::find_all(&state.db).await?;
    Ok(Json(holds))
}

/// PURPOSE: Place a legal hold on a user
/// POST /api/admin/legal-holds
/// CONSTRAINTS: user_id and reason are required
pub async fn create_legal_hold(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreateLegalHoldRequest>,
) -> Result<(StatusCode, Json<LegalHold>), AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    // Added: Validate reason is not empty
    if body.reason.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Legal hold reason cannot be empty".to_string(),
        ));
    }

    let placed_by = parse_user_id(&claims)?;
    let hold = LegalHold::create(&state.db, &body, placed_by).await?;
    Ok((StatusCode::CREATED, Json(hold)))
}

/// PURPOSE: Release a legal hold
/// PUT /api/admin/legal-holds/:id/release
pub async fn release_legal_hold(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<LegalHold>, AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    let hold = LegalHold::release(&state.db, id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound("Legal hold not found or already released".to_string())
        })?;
    Ok(Json(hold))
}

fn parse_user_id(claims: &Claims) -> Result<uuid::Uuid, AppError> {
    claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid user ID in JWT claims")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::auth_service::Claims;

    #[test]
    fn test_parse_user_id_valid() {
        let claims = Claims {
            sub: uuid::Uuid::new_v4().to_string(),
            username: "admin@example.com".into(),
            is_admin: true,
            exp: 0,
            iat: 0,
        };
        assert!(parse_user_id(&claims).is_ok());
    }

    #[test]
    fn test_parse_user_id_invalid() {
        let claims = Claims {
            sub: "not-a-uuid".into(),
            username: "admin@example.com".into(),
            is_admin: true,
            exp: 0,
            iat: 0,
        };
        assert!(parse_user_id(&claims).is_err());
    }
}
