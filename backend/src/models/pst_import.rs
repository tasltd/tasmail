// Added: PST import model for TMAIL-115 (Outlook PST file import)
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

/// PURPOSE: Represents a PST file import job record in the database
/// CONSTRAINTS: RLS enforced — users can only see their own imports
/// EXTERNAL: Maps to pst_imports table created in migration 027
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PstImport {
    pub id: Uuid,
    pub user_id: Uuid,
    pub filename: String,
    pub file_size: i64,
    pub status: String,
    pub target_folder: String,
    pub messages_found: Option<i32>,
    pub messages_imported: Option<i32>,
    pub error_message: Option<String>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// PURPOSE: Request payload for creating a new PST import (used internally after file upload)
#[derive(Debug, Deserialize)]
pub struct CreatePstImportRequest {
    pub filename: String,
    pub file_size: i64,
    pub target_folder: Option<String>,
}

impl PstImport {
    /// PURPOSE: Insert a new PST import record with pending status
    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        filename: &str,
        file_size: i64,
        target_folder: &str,
    ) -> Result<PstImport, sqlx::Error> {
        sqlx::query_as::<_, PstImport>(
            "INSERT INTO pst_imports (user_id, filename, file_size, target_folder)
             VALUES ($1, $2, $3, $4)
             RETURNING *",
        )
        .bind(user_id)
        .bind(filename)
        .bind(file_size)
        .bind(target_folder)
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: Retrieve a single PST import by ID
    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<PstImport>, sqlx::Error> {
        sqlx::query_as::<_, PstImport>("SELECT * FROM pst_imports WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    /// PURPOSE: List all PST imports for a given user, newest first
    pub async fn list_by_user(
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<Vec<PstImport>, sqlx::Error> {
        sqlx::query_as::<_, PstImport>(
            "SELECT * FROM pst_imports WHERE user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Transition import status to processing and record start time
    pub async fn mark_processing(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE pst_imports SET status = 'processing', started_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// PURPOSE: Update the messages_found count after PST extraction
    pub async fn set_messages_found(
        pool: &PgPool,
        id: Uuid,
        count: i32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE pst_imports SET messages_found = $2 WHERE id = $1")
            .bind(id)
            .bind(count)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// PURPOSE: Increment the messages_imported counter as emails are appended via IMAP
    pub async fn increment_imported(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE pst_imports SET messages_imported = COALESCE(messages_imported, 0) + 1 WHERE id = $1",
        )
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// PURPOSE: Mark import as completed with final timestamp
    pub async fn mark_completed(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE pst_imports SET status = 'completed', completed_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// PURPOSE: Mark import as failed with an error message
    pub async fn mark_failed(
        pool: &PgPool,
        id: Uuid,
        error_message: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE pst_imports SET status = 'failed', error_message = $2, completed_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .bind(error_message)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// PURPOSE: Delete a PST import record (only if pending or failed)
    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM pst_imports WHERE id = $1 AND status IN ('pending', 'failed')",
        )
        .bind(id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_pst_import_request_deserialization() {
        // Added: Verify JSON deserialization with all fields
        let json = r#"{"filename":"outlook.pst","file_size":52428800,"target_folder":"INBOX"}"#;
        let req: CreatePstImportRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.filename, "outlook.pst");
        assert_eq!(req.file_size, 52428800);
        assert_eq!(req.target_folder, Some("INBOX".to_string()));
    }

    #[test]
    fn test_create_pst_import_request_optional_folder() {
        // Added: Verify target_folder is optional and defaults to None
        let json = r#"{"filename":"mail.pst","file_size":1024}"#;
        let req: CreatePstImportRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.filename, "mail.pst");
        assert_eq!(req.file_size, 1024);
        assert!(req.target_folder.is_none());
    }

    #[test]
    fn test_create_pst_import_request_missing_required_fields() {
        // Added: Verify deserialization fails when filename is missing
        let json = r#"{"file_size":1024}"#;
        assert!(serde_json::from_str::<CreatePstImportRequest>(json).is_err());
    }

    #[test]
    fn test_create_pst_import_request_missing_file_size() {
        // Added: Verify deserialization fails when file_size is missing
        let json = r#"{"filename":"test.pst"}"#;
        assert!(serde_json::from_str::<CreatePstImportRequest>(json).is_err());
    }

    #[test]
    fn test_pst_import_serialization() {
        // Added: Verify PstImport struct serializes to JSON correctly
        let import = PstImport {
            id: Uuid::nil(),
            user_id: Uuid::nil(),
            filename: "export.pst".to_string(),
            file_size: 10_000_000,
            status: "pending".to_string(),
            target_folder: "INBOX".to_string(),
            messages_found: None,
            messages_imported: Some(0),
            error_message: None,
            started_at: None,
            completed_at: None,
            created_at: chrono::Utc::now(),
        };

        let json_value = serde_json::to_value(&import).unwrap();
        assert_eq!(json_value["filename"], "export.pst");
        assert_eq!(json_value["file_size"], 10_000_000);
        assert_eq!(json_value["status"], "pending");
        assert_eq!(json_value["target_folder"], "INBOX");
        assert!(json_value["messages_found"].is_null());
        assert_eq!(json_value["messages_imported"], 0);
    }

    #[test]
    fn test_pst_import_status_values() {
        // Added: Verify all expected status string values
        let valid_statuses = ["pending", "processing", "completed", "failed"];
        for status_value in valid_statuses {
            let import = PstImport {
                id: Uuid::nil(),
                user_id: Uuid::nil(),
                filename: "test.pst".to_string(),
                file_size: 100,
                status: status_value.to_string(),
                target_folder: "INBOX".to_string(),
                messages_found: None,
                messages_imported: None,
                error_message: None,
                started_at: None,
                completed_at: None,
                created_at: chrono::Utc::now(),
            };
            assert_eq!(import.status, status_value);
        }
    }
}
