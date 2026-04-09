use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::error::AppError;
use crate::models::domain::{CreateDomain, Domain};
use crate::services::auth_service::Claims;
use crate::state::AppState;

/// GET /api/admin/domains
pub async fn list_domains(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<Domain>>, AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }
    let domains = Domain::find_all(&state.db).await?;
    Ok(Json(domains))
}

/// POST /api/admin/domains
pub async fn create_domain(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreateDomain>,
) -> Result<(StatusCode, Json<Domain>), AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    // Check for duplicate
    if Domain::find_by_name(&state.db, &body.name).await?.is_some() {
        return Err(AppError::Conflict(format!(
            "Domain '{}' already exists",
            body.name
        )));
    }

    let domain = Domain::create(&state.db, &body.name).await?;
    Ok((StatusCode::CREATED, Json(domain)))
}

/// DELETE /api/admin/domains/:id
pub async fn delete_domain(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    if !Domain::delete(&state.db, id).await? {
        return Err(AppError::NotFound("Domain not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}
