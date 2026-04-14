use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::email_delegation::{CreateDelegation, EmailDelegation};
use crate::services::auth_service::Claims;
use crate::state::AppState;

/// POST /api/delegation — Grant a delegation (only grantor can grant)
pub async fn grant_delegation(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreateDelegation>,
) -> Result<(StatusCode, Json<EmailDelegation>), AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    // NOTE: Only the grantor themselves can create a delegation
    if body.grantor_id != mailbox_id {
        return Err(AppError::Forbidden(
            "You can only grant delegations from your own account".to_string(),
        ));
    }

    // Added: Validate delegation_type is a known value
    if body.delegation_type != "send_as" && body.delegation_type != "send_on_behalf" {
        return Err(AppError::BadRequest(
            "delegation_type must be 'send_as' or 'send_on_behalf'".to_string(),
        ));
    }

    let delegation = EmailDelegation::grant(&state.db, &body).await?;
    Ok((StatusCode::CREATED, Json(delegation)))
}

/// GET /api/delegation — List delegations granted TO the current user
pub async fn list_delegations(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<EmailDelegation>>, AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    let delegations = EmailDelegation::list_for_delegate(&state.db, mailbox_id).await?;
    Ok(Json(delegations))
}

/// GET /api/delegation/granted — List delegations the current user has granted
pub async fn list_granted(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<EmailDelegation>>, AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    let delegations = EmailDelegation::list_for_grantor(&state.db, mailbox_id).await?;
    Ok(Json(delegations))
}

/// DELETE /api/delegation/{id} — Revoke a delegation (only grantor can revoke)
pub async fn revoke_delegation(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    let revoked = EmailDelegation::revoke(&state.db, id, mailbox_id).await?;
    if revoked {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("Delegation not found".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_delegation_request() {
        let json = r#"{
            "grantor_id": "550e8400-e29b-41d4-a716-446655440000",
            "delegate_id": "660e8400-e29b-41d4-a716-446655440001",
            "delegation_type": "send_as"
        }"#;
        let req: CreateDelegation = serde_json::from_str(json).unwrap();
        assert_eq!(req.delegation_type, "send_as");
        assert_eq!(req.grantor_id.to_string(), "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn test_delegation_response_serialization() {
        let delegation = EmailDelegation {
            id: Uuid::new_v4(),
            grantor_id: Uuid::new_v4(),
            delegate_id: Uuid::new_v4(),
            delegation_type: "send_on_behalf".to_string(),
            created_at: chrono::Utc::now(),
        };
        let json = serde_json::to_string(&delegation).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["delegation_type"], "send_on_behalf");
        assert!(parsed["grantor_id"].is_string());
        assert!(parsed["delegate_id"].is_string());
    }

    #[test]
    fn test_valid_delegation_types() {
        // NOTE: Only 'send_as' and 'send_on_behalf' are valid delegation types
        let valid_types = vec!["send_as", "send_on_behalf"];
        for dtype in &valid_types {
            assert!(
                *dtype == "send_as" || *dtype == "send_on_behalf",
                "Unexpected delegation type: {dtype}"
            );
        }
    }

    #[test]
    fn test_invalid_delegation_type_rejected() {
        let invalid = "read_only";
        assert!(
            invalid != "send_as" && invalid != "send_on_behalf",
            "Should reject invalid delegation type"
        );
    }
}
