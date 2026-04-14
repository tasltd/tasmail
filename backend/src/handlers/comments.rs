// Added: Email comment handlers for TMAIL-128 — internal comments on emails
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::error::AppError;
use crate::models::email_comment::{CreateComment, EmailComment, UpdateComment};
use crate::services::auth_service::Claims;
use crate::state::AppState;

/// PURPOSE: Parse mailbox UUID from JWT claims
/// CONSTRAINTS: Claims.sub must be a valid UUID string
fn parse_mailbox_id(claims: &Claims) -> Result<uuid::Uuid, AppError> {
    claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID in claims")))
}

/// GET /api/folders/{folder}/messages/{uid}/comments — list comments for a message
pub async fn list_comments(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path((folder, uid)): Path<(String, i32)>,
) -> Result<Json<Vec<EmailComment>>, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;
    let comments = EmailComment::list_for_message(&state.db, mailbox_id, &folder, uid).await?;
    Ok(Json(comments))
}

/// POST /api/folders/{folder}/messages/{uid}/comments — add a comment to a message
pub async fn create_comment(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path((folder, uid)): Path<(String, i32)>,
    Json(body): Json<CreateComment>,
) -> Result<(StatusCode, Json<EmailComment>), AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;

    // NOTE: Validate content is not empty/whitespace
    if body.content.trim().is_empty() {
        return Err(AppError::BadRequest("Comment content cannot be empty".to_string()));
    }

    let comment = EmailComment::create(
        &state.db,
        mailbox_id,
        uid,
        &folder,
        &body.content,
        &claims.username,
        &claims.username, // NOTE: username is the email address in TASMail
    )
    .await?;

    Ok((StatusCode::CREATED, Json(comment)))
}

/// PUT /api/comments/{id} — edit a comment
pub async fn update_comment(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<UpdateComment>,
) -> Result<Json<EmailComment>, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;

    // NOTE: Validate content is not empty/whitespace
    if body.content.trim().is_empty() {
        return Err(AppError::BadRequest("Comment content cannot be empty".to_string()));
    }

    let comment = EmailComment::update(&state.db, id, mailbox_id, &body.content)
        .await?
        .ok_or_else(|| AppError::NotFound("Comment not found".to_string()))?;

    Ok(Json(comment))
}

/// DELETE /api/comments/{id} — delete a comment
pub async fn delete_comment(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;
    let deleted = EmailComment::delete(&state.db, id, mailbox_id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("Comment not found".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::auth_service::Claims;

    #[test]
    fn test_parse_mailbox_id_valid() {
        let claims = Claims {
            sub: uuid::Uuid::new_v4().to_string(),
            username: "test@example.com".into(),
            is_admin: false,
            exp: 0,
            iat: 0,
        };
        assert!(parse_mailbox_id(&claims).is_ok());
    }

    #[test]
    fn test_parse_mailbox_id_invalid() {
        let claims = Claims {
            sub: "not-a-uuid".into(),
            username: "test@example.com".into(),
            is_admin: false,
            exp: 0,
            iat: 0,
        };
        assert!(parse_mailbox_id(&claims).is_err());
    }
}
