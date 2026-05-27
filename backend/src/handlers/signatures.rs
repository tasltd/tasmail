use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::error::AppError;
use crate::models::signature::{CreateSignature, Signature, UpdateSignature};
use crate::services::auth_service::Claims;
use crate::state::AppState;

/// GET /api/signatures — list all signatures for the current user
pub async fn list_signatures(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<Signature>>, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;
    let signatures = Signature::find_by_mailbox(&state.db, mailbox_id).await?;
    Ok(Json(signatures))
}

/// POST /api/signatures — create a new signature
pub async fn create_signature(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreateSignature>,
) -> Result<(StatusCode, Json<Signature>), AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;
    let signature = Signature::create(&state.db, mailbox_id, &body).await?;
    Ok((StatusCode::CREATED, Json(signature)))
}

/// PUT /api/signatures/:id — update a signature
pub async fn update_signature(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<UpdateSignature>,
) -> Result<Json<Signature>, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;
    let signature = Signature::update(&state.db, id, mailbox_id, &body)
        .await?
        .ok_or_else(|| AppError::NotFound("Signature not found".to_string()))?;
    Ok(Json(signature))
}

/// DELETE /api/signatures/:id — delete a signature
pub async fn delete_signature(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;
    let deleted = Signature::delete(&state.db, id, mailbox_id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("Signature not found".to_string()))
    }
}

fn parse_mailbox_id(claims: &Claims) -> Result<uuid::Uuid, AppError> {
    claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::auth_service::Claims;

    #[test]
    fn test_parse_mailbox_id_valid() {
        let claims = Claims {
            sub: uuid::Uuid::new_v4().to_string(),
            username: "test".into(),
            is_admin: false,
            is_compliance_officer: false,
            exp: 0,
            iat: 0,
        };
        assert!(parse_mailbox_id(&claims).is_ok());
    }

    #[test]
    fn test_parse_mailbox_id_invalid() {
        let claims = Claims {
            sub: "not-a-uuid".into(),
            username: "test".into(),
            is_admin: false,
            is_compliance_officer: false,
            exp: 0,
            iat: 0,
        };
        assert!(parse_mailbox_id(&claims).is_err());
    }
}
