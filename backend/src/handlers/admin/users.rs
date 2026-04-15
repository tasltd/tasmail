use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::error::AppError;
use crate::models::mailbox::{CreateMailbox, Mailbox, MailboxInfo};
use crate::services::auth_service::{hash_password, Claims};
use crate::state::AppState;
// Added: Input validation for user creation (TMAIL-37)
use crate::validation;

/// GET /api/admin/users
pub async fn list_users(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<MailboxInfo>>, AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    // List all mailboxes across all domains
    let users = sqlx::query_as::<_, Mailbox>("SELECT * FROM mailboxes ORDER BY username")
        .fetch_all(&state.db)
        .await?;

    let infos: Vec<MailboxInfo> = users.into_iter().map(Into::into).collect();
    Ok(Json(infos))
}

/// POST /api/admin/users
pub async fn create_user(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreateMailbox>,
) -> Result<(StatusCode, Json<MailboxInfo>), AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    // Added: Validate input before processing (TMAIL-37)
    validation::validate_username(&body.username)?;
    validation::validate_password(&body.password)?;
    if let Some(ref name) = body.display_name {
        validation::validate_display_name(name)?;
    }

    // Check for duplicate username
    if Mailbox::find_by_username(&state.db, &body.username)
        .await?
        .is_some()
    {
        return Err(AppError::Conflict(format!(
            "User '{}' already exists",
            body.username
        )));
    }

    let password_hash = hash_password(&body.password)?;
    let quota = body.quota_bytes.unwrap_or(1_073_741_824); // 1 GB default

    let mailbox = Mailbox::create(
        &state.db,
        &body.username,
        &password_hash,
        body.domain_id,
        body.display_name.as_deref(),
        quota,
    )
    .await?;

    Ok((StatusCode::CREATED, Json(mailbox.into())))
}

/// DELETE /api/admin/users/:id
pub async fn delete_user(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    if !Mailbox::delete(&state.db, id).await? {
        return Err(AppError::NotFound("User not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}
