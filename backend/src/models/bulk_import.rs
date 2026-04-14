// Added: Bulk user import model for TMAIL-136 (CSV bulk provisioning)
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

/// PURPOSE: Represents a bulk user import job record in the database
/// CONSTRAINTS: Only admins can create bulk imports; errors stored as JSONB array
/// EXTERNAL: Maps to bulk_user_imports table created in migration 029
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct BulkUserImport {
    pub id: Uuid,
    pub admin_id: Uuid,
    pub filename: String,
    pub total_rows: i32,
    pub processed_rows: i32,
    pub success_count: i32,
    pub error_count: i32,
    // NOTE: JSONB column — sqlx maps it to serde_json::Value
    pub errors: serde_json::Value,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// PURPOSE: Represents a single row parsed from the CSV upload
/// CONSTRAINTS: email and password are required; role must be 'user' or 'admin'
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkImportRow {
    pub email: String,
    pub display_name: String,
    pub password: String,
    pub role: String,
}

/// PURPOSE: Represents an error encountered while processing a CSV row
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkImportError {
    pub row: usize,
    pub field: String,
    pub message: String,
}

impl BulkUserImport {
    /// PURPOSE: Insert a new bulk import record with pending status
    pub async fn create(
        pool: &PgPool,
        admin_id: Uuid,
        filename: &str,
        total_rows: i32,
    ) -> Result<BulkUserImport, sqlx::Error> {
        sqlx::query_as::<_, BulkUserImport>(
            "INSERT INTO bulk_user_imports (admin_id, filename, total_rows)
             VALUES ($1, $2, $3)
             RETURNING *",
        )
        .bind(admin_id)
        .bind(filename)
        .bind(total_rows)
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: Retrieve a single bulk import by ID
    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<BulkUserImport>, sqlx::Error> {
        sqlx::query_as::<_, BulkUserImport>("SELECT * FROM bulk_user_imports WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    /// PURPOSE: List all bulk imports for a given admin, newest first
    pub async fn list_by_admin(
        pool: &PgPool,
        admin_id: Uuid,
    ) -> Result<Vec<BulkUserImport>, sqlx::Error> {
        sqlx::query_as::<_, BulkUserImport>(
            "SELECT * FROM bulk_user_imports WHERE admin_id = $1 ORDER BY created_at DESC",
        )
        .bind(admin_id)
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Update import record after processing completes
    pub async fn mark_completed(
        pool: &PgPool,
        id: Uuid,
        processed_rows: i32,
        success_count: i32,
        error_count: i32,
        errors: &serde_json::Value,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE bulk_user_imports
             SET status = 'completed', processed_rows = $2, success_count = $3,
                 error_count = $4, errors = $5, completed_at = NOW()
             WHERE id = $1",
        )
        .bind(id)
        .bind(processed_rows)
        .bind(success_count)
        .bind(error_count)
        .bind(errors)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// PURPOSE: Mark import as failed with error details
    pub async fn mark_failed(
        pool: &PgPool,
        id: Uuid,
        errors: &serde_json::Value,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE bulk_user_imports
             SET status = 'failed', errors = $2, completed_at = NOW()
             WHERE id = $1",
        )
        .bind(id)
        .bind(errors)
        .execute(pool)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bulk_import_row_deserialization() {
        // Added: Verify CSV row struct deserializes from JSON correctly
        let json = r#"{"email":"user@example.com","display_name":"Test User","password":"SecurePass1!","role":"user"}"#;
        let row: BulkImportRow = serde_json::from_str(json).unwrap();
        assert_eq!(row.email, "user@example.com");
        assert_eq!(row.display_name, "Test User");
        assert_eq!(row.password, "SecurePass1!");
        assert_eq!(row.role, "user");
    }

    #[test]
    fn test_bulk_import_row_missing_field() {
        // Added: Verify deserialization fails when required field is missing
        let json = r#"{"email":"user@example.com","display_name":"Test"}"#;
        assert!(serde_json::from_str::<BulkImportRow>(json).is_err());
    }

    #[test]
    fn test_bulk_import_error_serialization() {
        // Added: Verify error struct serializes to JSON correctly
        let error = BulkImportError {
            row: 3,
            field: "email".to_string(),
            message: "Invalid email format".to_string(),
        };
        let json_value = serde_json::to_value(&error).unwrap();
        assert_eq!(json_value["row"], 3);
        assert_eq!(json_value["field"], "email");
        assert_eq!(json_value["message"], "Invalid email format");
    }

    #[test]
    fn test_bulk_import_error_deserialization() {
        // Added: Verify error struct deserializes from JSON correctly
        let json = r#"{"row":5,"field":"password","message":"Too short"}"#;
        let error: BulkImportError = serde_json::from_str(json).unwrap();
        assert_eq!(error.row, 5);
        assert_eq!(error.field, "password");
        assert_eq!(error.message, "Too short");
    }

    #[test]
    fn test_bulk_user_import_serialization() {
        // Added: Verify BulkUserImport struct serializes to JSON correctly
        let import = BulkUserImport {
            id: Uuid::nil(),
            admin_id: Uuid::nil(),
            filename: "users.csv".to_string(),
            total_rows: 50,
            processed_rows: 45,
            success_count: 40,
            error_count: 5,
            errors: serde_json::json!([{"row": 3, "field": "email", "message": "duplicate"}]),
            status: "completed".to_string(),
            created_at: chrono::Utc::now(),
            completed_at: Some(chrono::Utc::now()),
        };

        let json_value = serde_json::to_value(&import).unwrap();
        assert_eq!(json_value["filename"], "users.csv");
        assert_eq!(json_value["total_rows"], 50);
        assert_eq!(json_value["processed_rows"], 45);
        assert_eq!(json_value["success_count"], 40);
        assert_eq!(json_value["error_count"], 5);
        assert_eq!(json_value["status"], "completed");
        assert!(json_value["errors"].is_array());
    }

    #[test]
    fn test_bulk_user_import_status_values() {
        // Added: Verify all expected status string values are valid
        let valid_statuses = ["pending", "processing", "completed", "failed"];
        for status_value in valid_statuses {
            let import = BulkUserImport {
                id: Uuid::nil(),
                admin_id: Uuid::nil(),
                filename: "test.csv".to_string(),
                total_rows: 0,
                processed_rows: 0,
                success_count: 0,
                error_count: 0,
                errors: serde_json::json!([]),
                status: status_value.to_string(),
                created_at: chrono::Utc::now(),
                completed_at: None,
            };
            assert_eq!(import.status, status_value);
        }
    }

    #[test]
    fn test_errors_json_array_format() {
        // Added: Verify errors field holds a proper JSON array of BulkImportError
        let errors = vec![
            BulkImportError { row: 1, field: "email".to_string(), message: "Invalid format".to_string() },
            BulkImportError { row: 4, field: "role".to_string(), message: "Must be 'user' or 'admin'".to_string() },
        ];
        let json_value = serde_json::to_value(&errors).unwrap();
        assert!(json_value.is_array());
        assert_eq!(json_value.as_array().unwrap().len(), 2);
    }
}
