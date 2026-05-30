use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::scheduled_email::{CreateScheduledEmail, ScheduledEmail};
use crate::services::auth_service::Claims;
use crate::state::AppState;

/// Response after scheduling an email (includes cancel_token for undo)
#[derive(Debug, Serialize)]
pub struct ScheduleResponse {
    pub id: Uuid,
    pub cancel_token: Uuid,
    pub scheduled_at: chrono::DateTime<Utc>,
    pub can_undo_until: chrono::DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub status: Option<String>,
}

/// POST /api/messages/schedule — Schedule a message for future sending or delayed send (undo-send)
pub async fn schedule_send(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreateScheduledEmail>,
) -> Result<(StatusCode, Json<ScheduleResponse>), AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    // Determine scheduled time: explicit schedule_at, delay_seconds, or default 10s delay (undo-send)
    let scheduled_at = if let Some(at) = body.scheduled_at {
        if at <= Utc::now() {
            return Err(AppError::BadRequest("scheduled_at must be in the future".to_string()));
        }
        at
    } else {
        let delay = body.delay_seconds.unwrap_or(10);
        let delay = delay.clamp(5, 86400); // 5s to 24h
        Utc::now() + Duration::seconds(delay)
    };

    // TMAIL-319: thread reply/forward headers through to the persisted row
    // so the background scheduler can stamp `In-Reply-To` + `References` on
    // the outbound message. Compose-from-scratch leaves both unset.
    let references = body.references.as_deref().unwrap_or(&[]);
    let email = ScheduledEmail::create(
        &state.db,
        mailbox_id,
        &body.to,
        body.cc.as_deref().unwrap_or(&[]),
        body.bcc.as_deref().unwrap_or(&[]),
        &body.subject,
        body.text_body.as_deref(),
        body.html_body.as_deref(),
        scheduled_at,
        body.in_reply_to.as_deref(),
        references,
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(ScheduleResponse {
            id: email.id,
            cancel_token: email.cancel_token,
            scheduled_at: email.scheduled_at,
            can_undo_until: email.scheduled_at,
        }),
    ))
}

/// POST /api/messages/cancel/{cancel_token} — Cancel a scheduled email (undo-send)
pub async fn cancel_scheduled(
    State(state): State<AppState>,
    axum::Extension(_claims): axum::Extension<Claims>,
    Path(cancel_token): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let cancelled = ScheduledEmail::cancel_by_token(&state.db, cancel_token).await?;
    if !cancelled {
        return Err(AppError::NotFound(
            "Scheduled email not found or already sent".to_string(),
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/messages/scheduled — List scheduled emails for current user
pub async fn list_scheduled(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<ScheduledEmail>>, AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    let emails = ScheduledEmail::list_for_mailbox(
        &state.db,
        mailbox_id,
        query.status.as_deref(),
    )
    .await?;

    Ok(Json(emails))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schedule_response_serialization() {
        let resp = ScheduleResponse {
            id: Uuid::new_v4(),
            cancel_token: Uuid::new_v4(),
            scheduled_at: Utc::now(),
            can_undo_until: Utc::now(),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert!(json.get("id").is_some());
        assert!(json.get("cancel_token").is_some());
        assert!(json.get("scheduled_at").is_some());
        assert!(json.get("can_undo_until").is_some());
    }

    #[test]
    fn test_list_query_deserialization_with_status() {
        let json = r#"{"status": "pending"}"#;
        let query: ListQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.status.as_deref(), Some("pending"));
    }

    #[test]
    fn test_list_query_deserialization_without_status() {
        let json = r#"{}"#;
        let query: ListQuery = serde_json::from_str(json).unwrap();
        assert!(query.status.is_none());
    }
}
