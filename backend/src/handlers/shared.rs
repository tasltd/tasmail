use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::shared_mailbox::{
    GrantAccessRequest, SharedMailboxAcl, SharedMailboxAclWithUser, SharedMailboxView,
};
use crate::services::auth_service::Claims;
use crate::state::AppState;

fn parse_mailbox_id(claims: &Claims) -> Result<Uuid, AppError> {
    claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))
}

/// GET /api/shared-mailboxes — List shared mailboxes accessible by the current user
pub async fn list_accessible(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<SharedMailboxView>>, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;
    let shared = SharedMailboxAcl::list_accessible(&state.db, mailbox_id).await?;
    Ok(Json(shared))
}

/// GET /api/shared-mailboxes/:mailbox_id/acl — List ACL entries for a mailbox
pub async fn list_acl(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(mailbox_id): Path<Uuid>,
) -> Result<Json<Vec<SharedMailboxAclWithUser>>, AppError> {
    let current_id = parse_mailbox_id(&claims)?;

    // Only mailbox owner or admin can view ACL
    if mailbox_id != current_id && !claims.is_admin {
        return Err(AppError::Forbidden("Not the mailbox owner".to_string()));
    }

    let acls = SharedMailboxAcl::list_for_mailbox(&state.db, mailbox_id).await?;
    Ok(Json(acls))
}

/// POST /api/shared-mailboxes/:mailbox_id/acl — Grant access to a shared mailbox
pub async fn grant_access(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(mailbox_id): Path<Uuid>,
    Json(body): Json<GrantAccessRequest>,
) -> Result<(StatusCode, Json<SharedMailboxAcl>), AppError> {
    let current_id = parse_mailbox_id(&claims)?;

    // Only mailbox owner or admin can grant access
    if mailbox_id != current_id && !claims.is_admin {
        return Err(AppError::Forbidden("Not the mailbox owner".to_string()));
    }

    // Prevent granting access to self
    if body.granted_to == mailbox_id {
        return Err(AppError::BadRequest("Cannot grant access to the mailbox owner".to_string()));
    }

    let acl = SharedMailboxAcl::grant(&state.db, mailbox_id, &body, current_id).await?;
    Ok((StatusCode::CREATED, Json(acl)))
}

/// DELETE /api/shared-mailboxes/:mailbox_id/acl/:user_id — Revoke access
pub async fn revoke_access(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path((mailbox_id, user_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    let current_id = parse_mailbox_id(&claims)?;

    if mailbox_id != current_id && !claims.is_admin {
        return Err(AppError::Forbidden("Not the mailbox owner".to_string()));
    }

    SharedMailboxAcl::revoke(&state.db, mailbox_id, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
