// Added: Email task/to-do handlers for TMAIL-126
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::error::AppError;
use crate::models::email_task::{CreateTaskRequest, EmailTask, UpdateTaskRequest};
use crate::services::auth_service::Claims;
use crate::state::AppState;

/// Added: Query params for filtering tasks by completion status
#[derive(Debug, Deserialize)]
pub struct TaskFilterQuery {
    pub completed: Option<bool>,
}

/// PURPOSE: Parse user UUID from JWT claims
/// CONSTRAINTS: Claims.sub must be a valid UUID string
fn parse_user_id(claims: &Claims) -> Result<uuid::Uuid, AppError> {
    claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid user ID in claims")))
}

/// GET /api/tasks — list user's tasks with optional ?completed=true/false filter
pub async fn list_tasks(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Query(query): Query<TaskFilterQuery>,
) -> Result<Json<Vec<EmailTask>>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let tasks = EmailTask::list_for_user(&state.db, user_id, query.completed).await?;
    Ok(Json(tasks))
}

/// POST /api/tasks — create a new task
pub async fn create_task(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreateTaskRequest>,
) -> Result<(StatusCode, Json<EmailTask>), AppError> {
    let user_id = parse_user_id(&claims)?;

    // NOTE: Validate title is not empty/whitespace
    if body.title.trim().is_empty() {
        return Err(AppError::BadRequest("Task title cannot be empty".to_string()));
    }

    // NOTE: Validate priority value if provided
    if let Some(ref priority) = body.priority {
        if !["low", "normal", "high", "urgent"].contains(&priority.as_str()) {
            return Err(AppError::BadRequest(
                format!("Invalid priority '{}'. Must be one of: low, normal, high, urgent", priority),
            ));
        }
    }

    let task = EmailTask::create(&state.db, user_id, &body).await?;
    Ok((StatusCode::CREATED, Json(task)))
}

/// GET /api/tasks/:id — get a single task
pub async fn get_task(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<EmailTask>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let task = EmailTask::find_by_id(&state.db, id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Task not found".to_string()))?;
    Ok(Json(task))
}

/// PUT /api/tasks/:id — update a task
pub async fn update_task(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<UpdateTaskRequest>,
) -> Result<Json<EmailTask>, AppError> {
    let user_id = parse_user_id(&claims)?;

    // NOTE: Validate title is not empty/whitespace if provided
    if let Some(ref title) = body.title {
        if title.trim().is_empty() {
            return Err(AppError::BadRequest("Task title cannot be empty".to_string()));
        }
    }

    // NOTE: Validate priority value if provided
    if let Some(ref priority) = body.priority {
        if !["low", "normal", "high", "urgent"].contains(&priority.as_str()) {
            return Err(AppError::BadRequest(
                format!("Invalid priority '{}'. Must be one of: low, normal, high, urgent", priority),
            ));
        }
    }

    let task = EmailTask::update(&state.db, id, user_id, &body)
        .await?
        .ok_or_else(|| AppError::NotFound("Task not found".to_string()))?;
    Ok(Json(task))
}

/// DELETE /api/tasks/:id — delete a task
pub async fn delete_task(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, AppError> {
    let user_id = parse_user_id(&claims)?;
    let deleted = EmailTask::delete(&state.db, id, user_id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("Task not found".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::email_task::{CreateTaskRequest, UpdateTaskRequest};
    use crate::services::auth_service::Claims;

    #[test]
    fn test_parse_user_id_valid() {
        let claims = Claims {
            sub: uuid::Uuid::new_v4().to_string(),
            username: "test@example.com".into(),
            is_admin: false,
            exp: 0,
            iat: 0,
        };
        assert!(parse_user_id(&claims).is_ok());
    }

    #[test]
    fn test_parse_user_id_invalid() {
        let claims = Claims {
            sub: "not-a-uuid".into(),
            username: "test@example.com".into(),
            is_admin: false,
            exp: 0,
            iat: 0,
        };
        assert!(parse_user_id(&claims).is_err());
    }

    #[test]
    fn test_task_filter_query_with_completed() {
        let json = r#"{"completed": true}"#;
        let query: TaskFilterQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.completed, Some(true));
    }

    #[test]
    fn test_task_filter_query_without_completed() {
        let json = r#"{}"#;
        let query: TaskFilterQuery = serde_json::from_str(json).unwrap();
        assert!(query.completed.is_none());
    }

    #[test]
    fn test_create_task_request_deserialization() {
        let json = r#"{
            "title": "Review proposal",
            "description": "Check pricing section",
            "priority": "high",
            "linked_folder": "INBOX",
            "linked_uid": 55,
            "linked_subject": "Q4 Proposal"
        }"#;
        let req: CreateTaskRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.title, "Review proposal");
        assert_eq!(req.description.as_deref(), Some("Check pricing section"));
        assert_eq!(req.priority.as_deref(), Some("high"));
        assert_eq!(req.linked_folder.as_deref(), Some("INBOX"));
        assert_eq!(req.linked_uid, Some(55));
        assert_eq!(req.linked_subject.as_deref(), Some("Q4 Proposal"));
    }

    #[test]
    fn test_create_task_request_minimal() {
        let json = r#"{"title": "Quick task"}"#;
        let req: CreateTaskRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.title, "Quick task");
        assert!(req.description.is_none());
        assert!(req.priority.is_none());
        assert!(req.linked_folder.is_none());
    }

    #[test]
    fn test_create_task_request_missing_title_fails() {
        let json = r#"{"description": "No title"}"#;
        let result = serde_json::from_str::<CreateTaskRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_task_request_partial() {
        let json = r#"{"completed": true}"#;
        let req: UpdateTaskRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.completed, Some(true));
        assert!(req.title.is_none());
        assert!(req.priority.is_none());
    }

    #[test]
    fn test_update_task_request_empty() {
        let json = r#"{}"#;
        let req: UpdateTaskRequest = serde_json::from_str(json).unwrap();
        assert!(req.title.is_none());
        assert!(req.completed.is_none());
        assert!(req.priority.is_none());
    }
}
