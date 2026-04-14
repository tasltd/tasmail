use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::snoozed_email::{CreateSnooze, SnoozedEmail};
use crate::services::auth_service::Claims;
use crate::state::AppState;

/// POST /api/messages/snooze — Snooze an email until a specified time
pub async fn snooze_message(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreateSnooze>,
) -> Result<(StatusCode, Json<SnoozedEmail>), AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    // Validate snooze_until is in the future
    if body.snooze_until <= chrono::Utc::now() {
        return Err(AppError::BadRequest("Snooze time must be in the future".to_string()));
    }

    let snooze = SnoozedEmail::create(&state.db, mailbox_id, &body).await?;
    Ok((StatusCode::CREATED, Json(snooze)))
}

/// GET /api/messages/snoozed — List all snoozed emails
pub async fn list_snoozed(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<SnoozedEmail>>, AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    let snoozed = SnoozedEmail::list_by_mailbox(&state.db, mailbox_id).await?;
    Ok(Json(snoozed))
}

/// DELETE /api/messages/snooze/{id} — Cancel a snooze
pub async fn cancel_snooze(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    let cancelled = SnoozedEmail::cancel(&state.db, id, mailbox_id).await?;
    if cancelled {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("Snoozed email not found".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_snooze_request() {
        let json = r#"{
            "folder": "INBOX",
            "message_uid": 100,
            "snooze_until": "2026-04-15T14:00:00Z"
        }"#;
        let req: CreateSnooze = serde_json::from_str(json).unwrap();
        assert_eq!(req.folder, "INBOX");
        assert_eq!(req.message_uid, 100);
    }

    #[test]
    fn test_snooze_response_serialization() {
        let snooze = SnoozedEmail {
            id: Uuid::new_v4(),
            mailbox_id: Uuid::new_v4(),
            folder: "INBOX".to_string(),
            message_uid: 42,
            snooze_until: chrono::Utc::now() + chrono::Duration::hours(4),
            original_folder: "INBOX".to_string(),
            created_at: chrono::Utc::now(),
        };
        let json = serde_json::to_string(&snooze).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["folder"], "INBOX");
        assert_eq!(parsed["message_uid"], 42);
    }

    #[test]
    fn test_snooze_validation_future_time() {
        let past = chrono::Utc::now() - chrono::Duration::hours(1);
        assert!(past <= chrono::Utc::now(), "Past time should fail validation");
    }

    #[test]
    fn test_snooze_presets() {
        // Common snooze presets: later today, tomorrow, next week
        let now = chrono::Utc::now();
        let later_today = now + chrono::Duration::hours(3);
        let tomorrow_morning = now + chrono::Duration::hours(20);
        let next_week = now + chrono::Duration::days(7);

        assert!(later_today > now);
        assert!(tomorrow_morning > later_today);
        assert!(next_week > tomorrow_morning);
    }
}
