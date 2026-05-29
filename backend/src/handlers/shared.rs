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

// TMAIL-289: list/grant/revoke previously gated on
//   `mailbox_id == claims.sub || claims.is_admin`
// which locked out users who had can_admin = true on the ACL itself — yet the
// SharedMailboxManager UI explicitly surfaces the grant form and ACL list to
// any user with the can_admin permission on a shared mailbox. The fix: also
// honour can_admin on the ACL. The mailbox-owner always passes the check by
// definition (their mailbox_id == current_user_id), and system admins keep
// the bypass.
async fn assert_can_manage_acl(
    state: &AppState,
    claims: &Claims,
    mailbox_id: Uuid,
) -> Result<Uuid, AppError> {
    let current_id = parse_mailbox_id(claims)?;
    if mailbox_id == current_id || claims.is_admin {
        return Ok(current_id);
    }
    // Delegated admin lookup — must be a can_admin = true row on this mailbox.
    let delegated: Option<(bool,)> = sqlx::query_as(
        "SELECT can_admin FROM shared_mailbox_acl
         WHERE mailbox_id = $1 AND granted_to = $2",
    )
    .bind(mailbox_id)
    .bind(current_id)
    .fetch_optional(&state.db)
    .await?;
    match delegated {
        Some((true,)) => Ok(current_id),
        _ => Err(AppError::Forbidden(
            "Not the mailbox owner or a delegated admin".to_string(),
        )),
    }
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
    // Mailbox owner, system admin, or delegated can_admin grantee.
    assert_can_manage_acl(&state, &claims, mailbox_id).await?;

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
    let current_id = assert_can_manage_acl(&state, &claims, mailbox_id).await?;

    // Prevent granting access to self (the mailbox owner already has access).
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
    assert_can_manage_acl(&state, &claims, mailbox_id).await?;

    SharedMailboxAcl::revoke(&state.db, mailbox_id, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::auth_service::Claims;

    fn fixture_claims(sub: Uuid, is_admin: bool) -> Claims {
        Claims {
            sub: sub.to_string(),
            username: "u@example.com".into(),
            is_admin,
            is_compliance_officer: false,
            exp: 0,
            iat: 0,
        }
    }

    #[test]
    fn parse_mailbox_id_extracts_uuid_from_sub() {
        let id = Uuid::new_v4();
        let c = fixture_claims(id, false);
        assert_eq!(parse_mailbox_id(&c).unwrap(), id);
    }

    #[test]
    fn parse_mailbox_id_rejects_non_uuid() {
        let c = Claims {
            sub: "not-a-uuid".into(),
            username: "u@example.com".into(),
            is_admin: false,
            is_compliance_officer: false,
            exp: 0,
            iat: 0,
        };
        assert!(parse_mailbox_id(&c).is_err());
    }
}
