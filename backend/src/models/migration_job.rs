use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct MigrationJob {
    pub id: Uuid,
    pub mailbox_id: Uuid,
    pub job_type: String,
    pub status: String,
    pub source_host: Option<String>,
    pub source_port: Option<i32>,
    pub source_user: Option<String>,
    // NOTE: Never serialize the password
    #[serde(skip)]
    pub source_password_encrypted: Option<String>,
    pub source_use_ssl: Option<bool>,
    pub mbox_file_path: Option<String>,
    pub folders_total: Option<i32>,
    pub folders_done: Option<i32>,
    pub messages_total: Option<i32>,
    pub messages_done: Option<i32>,
    pub bytes_transferred: Option<i64>,
    pub error_message: Option<String>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateImapMigrationRequest {
    pub source_host: String,
    pub source_port: Option<i32>,
    pub source_user: String,
    pub source_password: String,
    pub source_use_ssl: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CreateMboxImportRequest {
    pub mbox_file_path: String,
}

impl MigrationJob {
    /// Create an IMAP migration job
    pub async fn create_imap(
        pool: &PgPool,
        mailbox_id: Uuid,
        req: &CreateImapMigrationRequest,
    ) -> Result<MigrationJob, sqlx::Error> {
        sqlx::query_as::<_, MigrationJob>(
            "INSERT INTO migration_jobs (mailbox_id, job_type, source_host, source_port, source_user, source_password_encrypted, source_use_ssl)
             VALUES ($1, 'imap', $2, $3, $4, $5, $6)
             RETURNING *"
        )
        .bind(mailbox_id)
        .bind(&req.source_host)
        .bind(req.source_port.unwrap_or(993))
        .bind(&req.source_user)
        .bind(&req.source_password) // NOTE: Should be encrypted in production
        .bind(req.source_use_ssl.unwrap_or(true))
        .fetch_one(pool)
        .await
    }

    /// Create an MBOX import job
    pub async fn create_mbox(
        pool: &PgPool,
        mailbox_id: Uuid,
        req: &CreateMboxImportRequest,
    ) -> Result<MigrationJob, sqlx::Error> {
        sqlx::query_as::<_, MigrationJob>(
            "INSERT INTO migration_jobs (mailbox_id, job_type, mbox_file_path)
             VALUES ($1, 'mbox', $2)
             RETURNING *"
        )
        .bind(mailbox_id)
        .bind(&req.mbox_file_path)
        .fetch_one(pool)
        .await
    }

    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<MigrationJob>, sqlx::Error> {
        sqlx::query_as::<_, MigrationJob>(
            "SELECT * FROM migration_jobs WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(pool)
        .await
    }

    pub async fn list_by_mailbox(pool: &PgPool, mailbox_id: Uuid) -> Result<Vec<MigrationJob>, sqlx::Error> {
        sqlx::query_as::<_, MigrationJob>(
            "SELECT * FROM migration_jobs WHERE mailbox_id = $1 ORDER BY created_at DESC"
        )
        .bind(mailbox_id)
        .fetch_all(pool)
        .await
    }

    pub async fn update_status(
        pool: &PgPool,
        id: Uuid,
        status: &str,
        error: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now();
        match status {
            "running" => {
                sqlx::query("UPDATE migration_jobs SET status = $2, started_at = $3 WHERE id = $1")
                    .bind(id)
                    .bind(status)
                    .bind(now)
                    .execute(pool)
                    .await?;
            }
            "completed" | "failed" => {
                sqlx::query("UPDATE migration_jobs SET status = $2, error_message = $3, completed_at = $4 WHERE id = $1")
                    .bind(id)
                    .bind(status)
                    .bind(error)
                    .bind(now)
                    .execute(pool)
                    .await?;
            }
            _ => {
                sqlx::query("UPDATE migration_jobs SET status = $2, error_message = $3 WHERE id = $1")
                    .bind(id)
                    .bind(status)
                    .bind(error)
                    .execute(pool)
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn update_progress(
        pool: &PgPool,
        id: Uuid,
        folders_done: i32,
        folders_total: i32,
        messages_done: i32,
        messages_total: i32,
        bytes_transferred: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE migration_jobs SET folders_done = $2, folders_total = $3, messages_done = $4, messages_total = $5, bytes_transferred = $6 WHERE id = $1"
        )
        .bind(id)
        .bind(folders_done)
        .bind(folders_total)
        .bind(messages_done)
        .bind(messages_total)
        .bind(bytes_transferred)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn cancel(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE migration_jobs SET status = 'cancelled', completed_at = NOW() WHERE id = $1 AND status IN ('pending', 'running')")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_imap_migration_request_deserialization() {
        let json = r#"{"source_host":"imap.gmail.com","source_user":"user@gmail.com","source_password":"apppassword"}"#;
        let req: CreateImapMigrationRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.source_host, "imap.gmail.com");
        assert_eq!(req.source_user, "user@gmail.com");
        assert!(req.source_port.is_none());
        assert!(req.source_use_ssl.is_none());
    }

    #[test]
    fn test_mbox_import_request() {
        let json = r#"{"mbox_file_path":"/tmp/uploads/takeout.mbox"}"#;
        let req: CreateMboxImportRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.mbox_file_path, "/tmp/uploads/takeout.mbox");
    }

    #[test]
    fn test_imap_request_with_all_fields() {
        let json = r#"{"source_host":"mail.example.com","source_port":143,"source_user":"user","source_password":"pass","source_use_ssl":false}"#;
        let req: CreateImapMigrationRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.source_port, Some(143));
        assert_eq!(req.source_use_ssl, Some(false));
    }
}
