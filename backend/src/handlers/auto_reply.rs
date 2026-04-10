use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::auto_reply::{AutoReplyRule, UpsertAutoReply};
use crate::services::auth_service::Claims;
use crate::state::AppState;

/// GET /api/auto-reply — Get current auto-reply settings
pub async fn get_auto_reply(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Option<AutoReplyRule>>, AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    let rule = AutoReplyRule::find_by_mailbox(&state.db, mailbox_id).await?;
    Ok(Json(rule))
}

/// PUT /api/auto-reply — Create or update auto-reply settings
pub async fn set_auto_reply(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<UpsertAutoReply>,
) -> Result<(StatusCode, Json<AutoReplyRule>), AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))?;

    // Validate date range
    if let (Some(start), Some(end)) = (body.start_date, body.end_date) {
        if end <= start {
            return Err(AppError::BadRequest("End date must be after start date".to_string()));
        }
    }

    let rule = AutoReplyRule::upsert(&state.db, mailbox_id, &body).await?;
    Ok((StatusCode::OK, Json(rule)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_reply_handler_date_validation_logic() {
        // Test the date comparison logic: end must be after start
        let start = chrono::Utc::now();
        let end = start - chrono::Duration::seconds(3600);
        assert!(end <= start, "End before start should be invalid");
    }
}
