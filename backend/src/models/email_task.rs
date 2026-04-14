// Added: Email task model for TMAIL-126 — tasks/to-do linked to emails
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// PURPOSE: Represents a user task/to-do, optionally linked to an email message
/// CONSTRAINTS: Priority must be one of: low, normal, high, urgent
/// EXTERNAL: PostgreSQL with RLS enforcing user isolation
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EmailTask {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub due_date: Option<chrono::DateTime<chrono::Utc>>,
    pub completed: bool,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub priority: String,
    pub linked_folder: Option<String>,
    pub linked_uid: Option<i32>,
    pub linked_subject: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub title: String,
    pub description: Option<String>,
    pub due_date: Option<chrono::DateTime<chrono::Utc>>,
    pub priority: Option<String>,
    pub linked_folder: Option<String>,
    pub linked_uid: Option<i32>,
    pub linked_subject: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTaskRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub due_date: Option<chrono::DateTime<chrono::Utc>>,
    pub priority: Option<String>,
    pub completed: Option<bool>,
    pub linked_folder: Option<String>,
    pub linked_uid: Option<i32>,
    pub linked_subject: Option<String>,
}

impl EmailTask {
    /// Added: List all tasks for a user, optionally filtered by completion status
    pub async fn list_for_user(
        pool: &PgPool,
        user_id: Uuid,
        completed_filter: Option<bool>,
    ) -> Result<Vec<EmailTask>, sqlx::Error> {
        // NOTE: Filter by completed status if provided, otherwise return all
        match completed_filter {
            Some(completed) => {
                sqlx::query_as::<_, EmailTask>(
                    "SELECT * FROM email_tasks WHERE user_id = $1 AND completed = $2 ORDER BY due_date ASC NULLS LAST, created_at DESC"
                )
                .bind(user_id)
                .bind(completed)
                .fetch_all(pool)
                .await
            }
            None => {
                sqlx::query_as::<_, EmailTask>(
                    "SELECT * FROM email_tasks WHERE user_id = $1 ORDER BY due_date ASC NULLS LAST, created_at DESC"
                )
                .bind(user_id)
                .fetch_all(pool)
                .await
            }
        }
    }

    /// Added: Find a single task by ID and user
    pub async fn find_by_id(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<EmailTask>, sqlx::Error> {
        sqlx::query_as::<_, EmailTask>(
            "SELECT * FROM email_tasks WHERE id = $1 AND user_id = $2"
        )
        .bind(id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
    }

    /// Added: Create a new task
    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        input: &CreateTaskRequest,
    ) -> Result<EmailTask, sqlx::Error> {
        let priority = input.priority.as_deref().unwrap_or("normal");
        sqlx::query_as::<_, EmailTask>(
            "INSERT INTO email_tasks (user_id, title, description, due_date, priority, linked_folder, linked_uid, linked_subject)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             RETURNING *"
        )
        .bind(user_id)
        .bind(&input.title)
        .bind(&input.description)
        .bind(input.due_date)
        .bind(priority)
        .bind(&input.linked_folder)
        .bind(input.linked_uid)
        .bind(&input.linked_subject)
        .fetch_one(pool)
        .await
    }

    /// Added: Update task fields (uses COALESCE for partial updates)
    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
        input: &UpdateTaskRequest,
    ) -> Result<Option<EmailTask>, sqlx::Error> {
        // NOTE: completed_at is set automatically when completed transitions to true
        sqlx::query_as::<_, EmailTask>(
            "UPDATE email_tasks SET
                title = COALESCE($3, title),
                description = COALESCE($4, description),
                due_date = COALESCE($5, due_date),
                priority = COALESCE($6, priority),
                completed = COALESCE($7, completed),
                completed_at = CASE
                    WHEN $7 = true AND NOT completed THEN NOW()
                    WHEN $7 = false THEN NULL
                    ELSE completed_at
                END,
                linked_folder = COALESCE($8, linked_folder),
                linked_uid = COALESCE($9, linked_uid),
                linked_subject = COALESCE($10, linked_subject),
                updated_at = NOW()
            WHERE id = $1 AND user_id = $2
            RETURNING *"
        )
        .bind(id)
        .bind(user_id)
        .bind(&input.title)
        .bind(&input.description)
        .bind(input.due_date)
        .bind(&input.priority)
        .bind(input.completed)
        .bind(&input.linked_folder)
        .bind(input.linked_uid)
        .bind(&input.linked_subject)
        .fetch_optional(pool)
        .await
    }

    /// Added: Delete a task by ID and user
    pub async fn delete(pool: &PgPool, id: Uuid, user_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM email_tasks WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_task_serialization() {
        let id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let now = chrono::Utc::now();

        let task = EmailTask {
            id,
            user_id,
            title: "Reply to client proposal".to_string(),
            description: Some("Review and respond by EOD".to_string()),
            due_date: Some(now),
            completed: false,
            completed_at: None,
            priority: "high".to_string(),
            linked_folder: Some("INBOX".to_string()),
            linked_uid: Some(42),
            linked_subject: Some("Re: Q4 Proposal".to_string()),
            created_at: now,
            updated_at: now,
        };

        let json = serde_json::to_value(&task).unwrap();
        assert_eq!(json["id"], id.to_string());
        assert_eq!(json["user_id"], user_id.to_string());
        assert_eq!(json["title"], "Reply to client proposal");
        assert_eq!(json["description"], "Review and respond by EOD");
        assert_eq!(json["completed"], false);
        assert_eq!(json["priority"], "high");
        assert_eq!(json["linked_folder"], "INBOX");
        assert_eq!(json["linked_uid"], 42);
        assert_eq!(json["linked_subject"], "Re: Q4 Proposal");
    }

    #[test]
    fn test_email_task_serialization_minimal() {
        let task = EmailTask {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            title: "Simple task".to_string(),
            description: None,
            due_date: None,
            completed: false,
            completed_at: None,
            priority: "normal".to_string(),
            linked_folder: None,
            linked_uid: None,
            linked_subject: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let json = serde_json::to_value(&task).unwrap();
        assert_eq!(json["title"], "Simple task");
        assert_eq!(json["priority"], "normal");
        assert!(json["description"].is_null());
        assert!(json["due_date"].is_null());
        assert!(json["linked_folder"].is_null());
        assert!(json["linked_uid"].is_null());
        assert!(json["linked_subject"].is_null());
    }

    #[test]
    fn test_email_task_roundtrip() {
        let task = EmailTask {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            title: "Follow up on invoice".to_string(),
            description: Some("Send reminder".to_string()),
            due_date: None,
            completed: true,
            completed_at: Some(chrono::Utc::now()),
            priority: "urgent".to_string(),
            linked_folder: None,
            linked_uid: None,
            linked_subject: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&task).unwrap();
        let deserialized: EmailTask = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, task.id);
        assert_eq!(deserialized.title, "Follow up on invoice");
        assert_eq!(deserialized.completed, true);
        assert_eq!(deserialized.priority, "urgent");
    }

    #[test]
    fn test_create_task_request_full() {
        let json = serde_json::json!({
            "title": "Review contract",
            "description": "Check section 4.2",
            "due_date": "2026-04-20T10:00:00Z",
            "priority": "high",
            "linked_folder": "INBOX",
            "linked_uid": 99,
            "linked_subject": "Contract Draft v2"
        });

        let req: CreateTaskRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.title, "Review contract");
        assert_eq!(req.description.as_deref(), Some("Check section 4.2"));
        assert_eq!(req.priority.as_deref(), Some("high"));
        assert_eq!(req.linked_folder.as_deref(), Some("INBOX"));
        assert_eq!(req.linked_uid, Some(99));
        assert_eq!(req.linked_subject.as_deref(), Some("Contract Draft v2"));
    }

    #[test]
    fn test_create_task_request_minimal() {
        let json = serde_json::json!({
            "title": "Quick task"
        });

        let req: CreateTaskRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.title, "Quick task");
        assert!(req.description.is_none());
        assert!(req.due_date.is_none());
        assert!(req.priority.is_none());
        assert!(req.linked_folder.is_none());
        assert!(req.linked_uid.is_none());
        assert!(req.linked_subject.is_none());
    }

    #[test]
    fn test_create_task_request_missing_title_fails() {
        let json = serde_json::json!({
            "description": "No title provided"
        });
        let result = serde_json::from_value::<CreateTaskRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_task_request_partial() {
        let json = serde_json::json!({
            "completed": true
        });

        let req: UpdateTaskRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.completed, Some(true));
        assert!(req.title.is_none());
        assert!(req.description.is_none());
        assert!(req.priority.is_none());
    }

    #[test]
    fn test_update_task_request_all_fields() {
        let json = serde_json::json!({
            "title": "Updated title",
            "description": "Updated desc",
            "due_date": "2026-05-01T12:00:00Z",
            "priority": "low",
            "completed": false,
            "linked_folder": "Sent",
            "linked_uid": 10,
            "linked_subject": "Updated subject"
        });

        let req: UpdateTaskRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.title.as_deref(), Some("Updated title"));
        assert_eq!(req.description.as_deref(), Some("Updated desc"));
        assert_eq!(req.priority.as_deref(), Some("low"));
        assert_eq!(req.completed, Some(false));
        assert_eq!(req.linked_folder.as_deref(), Some("Sent"));
        assert_eq!(req.linked_uid, Some(10));
        assert_eq!(req.linked_subject.as_deref(), Some("Updated subject"));
    }

    #[test]
    fn test_update_task_request_empty() {
        let json = serde_json::json!({});
        let req: UpdateTaskRequest = serde_json::from_value(json).unwrap();
        assert!(req.title.is_none());
        assert!(req.description.is_none());
        assert!(req.due_date.is_none());
        assert!(req.priority.is_none());
        assert!(req.completed.is_none());
    }
}
