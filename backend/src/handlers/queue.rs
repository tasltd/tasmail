// Added: Email queue management handlers for TMAIL-58
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::email_queue::{EmailQueueItem, QueueStats};
use crate::services::auth_service::Claims;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ListQueueQuery {
    pub status: Option<String>,
}

/// GET /api/queue — List queued emails for current user's mailbox
pub async fn list_queue(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Query(query): Query<ListQueueQuery>,
) -> Result<Json<Vec<EmailQueueItem>>, AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID in JWT claims")))?;

    let items = EmailQueueItem::list_by_mailbox(
        &state.db,
        mailbox_id,
        query.status.as_deref(),
    )
    .await?;

    Ok(Json(items))
}

/// GET /api/queue/stats — Queue statistics (counts by status)
pub async fn queue_stats(
    State(state): State<AppState>,
    axum::Extension(_claims): axum::Extension<Claims>,
) -> Result<Json<QueueStats>, AppError> {
    let stats = EmailQueueItem::queue_stats(&state.db).await?;
    Ok(Json(stats))
}

/// DELETE /api/queue/{id} — Cancel/remove a queued email
/// CONSTRAINTS: Only works for pending, failed, or dead_letter items (not currently sending)
pub async fn cancel_queued(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID in JWT claims")))?;

    let deleted = EmailQueueItem::delete(&state.db, id, mailbox_id).await?;
    if !deleted {
        return Err(AppError::NotFound(
            "Queue item not found, not owned by you, or currently sending".to_string(),
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/queue/{id}/retry — Retry a failed or dead_letter email
pub async fn retry_queued(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID in JWT claims")))?;

    let retried = EmailQueueItem::retry(&state.db, id, mailbox_id).await?;
    if !retried {
        return Err(AppError::NotFound(
            "Queue item not found, not owned by you, or not in failed/dead_letter status".to_string(),
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_queue_query_deserialization_with_status() {
        let json = r#"{"status": "pending"}"#;
        let query: ListQueueQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.status.as_deref(), Some("pending"));
    }

    #[test]
    fn test_list_queue_query_deserialization_without_status() {
        let json = r#"{}"#;
        let query: ListQueueQuery = serde_json::from_str(json).unwrap();
        assert!(query.status.is_none());
    }

    #[test]
    fn test_list_queue_query_with_failed_status() {
        let json = r#"{"status": "failed"}"#;
        let query: ListQueueQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.status.as_deref(), Some("failed"));
    }

    #[test]
    fn test_list_queue_query_with_dead_letter_status() {
        let json = r#"{"status": "dead_letter"}"#;
        let query: ListQueueQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.status.as_deref(), Some("dead_letter"));
    }
}
