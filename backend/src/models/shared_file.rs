// Added: Shared file model for large file sharing via download links (TMAIL-138)
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// PURPOSE: Represents a file shared via a public download link
/// CONSTRAINTS: download_token must be unique; user_id references the uploader
/// EXTERNAL: PostgreSQL via sqlx, RLS enforces user isolation for list/detail queries
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SharedFile {
    pub id: Uuid,
    pub user_id: Uuid,
    pub filename: String,
    pub content_type: String,
    pub file_size: i64,
    pub storage_path: String,
    pub download_token: String,
    pub download_count: i32,
    pub max_downloads: Option<i32>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub password_hash: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// PURPOSE: Request payload for creating a shared file (after multipart upload)
#[derive(Debug, Deserialize)]
pub struct CreateSharedFileRequest {
    pub max_downloads: Option<i32>,
    pub expires_at: Option<String>,
    pub password: Option<String>,
}

/// PURPOSE: Public-facing shared file info returned by the download endpoint
/// NOTE: Excludes storage_path and password_hash for security
#[derive(Debug, Serialize)]
pub struct SharedFilePublicInfo {
    pub filename: String,
    pub content_type: String,
    pub file_size: i64,
    pub requires_password: bool,
}

impl SharedFile {
    /// PURPOSE: Insert a new shared file record
    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        filename: &str,
        content_type: &str,
        file_size: i64,
        storage_path: &str,
        download_token: &str,
        max_downloads: Option<i32>,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
        password_hash: Option<&str>,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as::<_, SharedFile>(
            "INSERT INTO shared_files (user_id, filename, content_type, file_size, storage_path, download_token, max_downloads, expires_at, password_hash)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING *",
        )
        .bind(user_id)
        .bind(filename)
        .bind(content_type)
        .bind(file_size)
        .bind(storage_path)
        .bind(download_token)
        .bind(max_downloads)
        .bind(expires_at)
        .bind(password_hash)
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: Find shared file by ID (RLS enforces user isolation)
    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, SharedFile>("SELECT * FROM shared_files WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    /// PURPOSE: Find shared file by public download token (bypasses RLS for public access)
    /// NOTE: This is called from the public download endpoint without auth context
    pub async fn find_by_token(pool: &PgPool, token: &str) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, SharedFile>(
            "SELECT * FROM shared_files WHERE download_token = $1",
        )
        .bind(token)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: List all shared files for the current user, newest first
    pub async fn list_by_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, SharedFile>(
            "SELECT * FROM shared_files WHERE user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Increment the download counter after a successful download
    pub async fn increment_download_count(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE shared_files SET download_count = download_count + 1 WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// PURPOSE: Delete a shared file record (caller should also delete from disk)
    pub async fn delete(pool: &PgPool, id: Uuid, user_id: Uuid) -> Result<bool, sqlx::Error> {
        let result =
            sqlx::query("DELETE FROM shared_files WHERE id = $1 AND user_id = $2")
                .bind(id)
                .bind(user_id)
                .execute(pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    /// PURPOSE: Check if the file has expired based on time or download count
    pub fn is_expired(&self) -> bool {
        // NOTE: Check time-based expiry
        if let Some(expires_at) = self.expires_at {
            if chrono::Utc::now() > expires_at {
                return true;
            }
        }
        // NOTE: Check download-count-based expiry
        if let Some(max_downloads) = self.max_downloads {
            if self.download_count >= max_downloads {
                return true;
            }
        }
        false
    }

    /// PURPOSE: Check if this file requires a password to download
    pub fn requires_password(&self) -> bool {
        self.password_hash.is_some()
    }
}

/// PURPOSE: Generate a cryptographically random download token
/// NOTE: Uses 32 random bytes encoded as URL-safe base64 (no padding)
pub fn generate_download_token() -> String {
    use rand::Rng;
    let mut rng = rand::rng();
    let bytes: [u8; 32] = rng.random();
    // Added: Use hex encoding for URL-safe tokens without padding characters
    hex::encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shared_file_serialization() {
        let id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let now = chrono::Utc::now();

        let shared_file = SharedFile {
            id,
            user_id,
            filename: "presentation.pdf".to_string(),
            content_type: "application/pdf".to_string(),
            file_size: 5242880,
            storage_path: "/data/shared/test/presentation.pdf".to_string(),
            download_token: "abc123def456".to_string(),
            download_count: 3,
            max_downloads: Some(10),
            expires_at: Some(now),
            password_hash: None,
            created_at: now,
        };

        let json = serde_json::to_value(&shared_file).unwrap();
        assert_eq!(json["id"], id.to_string());
        assert_eq!(json["user_id"], user_id.to_string());
        assert_eq!(json["filename"], "presentation.pdf");
        assert_eq!(json["content_type"], "application/pdf");
        assert_eq!(json["file_size"], 5242880);
        assert_eq!(json["download_count"], 3);
        assert_eq!(json["max_downloads"], 10);
    }

    #[test]
    fn test_shared_file_roundtrip() {
        let shared_file = SharedFile {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            filename: "image.png".to_string(),
            content_type: "image/png".to_string(),
            file_size: 5000,
            storage_path: "/data/shared/test.png".to_string(),
            download_token: "token123".to_string(),
            download_count: 0,
            max_downloads: None,
            expires_at: None,
            password_hash: None,
            created_at: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&shared_file).unwrap();
        let deserialized: SharedFile = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, shared_file.id);
        assert_eq!(deserialized.filename, "image.png");
        assert_eq!(deserialized.download_count, 0);
        assert!(deserialized.max_downloads.is_none());
    }

    #[test]
    fn test_is_expired_by_time() {
        let mut shared_file = SharedFile {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            filename: "test.txt".to_string(),
            content_type: "text/plain".to_string(),
            file_size: 100,
            storage_path: "/data/test.txt".to_string(),
            download_token: "tok".to_string(),
            download_count: 0,
            max_downloads: None,
            expires_at: None,
            password_hash: None,
            created_at: chrono::Utc::now(),
        };

        // NOTE: No expiry set — should not be expired
        assert!(!shared_file.is_expired());

        // NOTE: Set expiry in the past — should be expired
        shared_file.expires_at = Some(chrono::Utc::now() - chrono::Duration::hours(1));
        assert!(shared_file.is_expired());

        // NOTE: Set expiry in the future — should not be expired
        shared_file.expires_at = Some(chrono::Utc::now() + chrono::Duration::hours(1));
        assert!(!shared_file.is_expired());
    }

    #[test]
    fn test_is_expired_by_download_count() {
        let mut shared_file = SharedFile {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            filename: "test.txt".to_string(),
            content_type: "text/plain".to_string(),
            file_size: 100,
            storage_path: "/data/test.txt".to_string(),
            download_token: "tok".to_string(),
            download_count: 5,
            max_downloads: Some(10),
            expires_at: None,
            password_hash: None,
            created_at: chrono::Utc::now(),
        };

        // NOTE: 5 of 10 downloads used — not expired
        assert!(!shared_file.is_expired());

        // NOTE: All downloads used — expired
        shared_file.download_count = 10;
        assert!(shared_file.is_expired());

        // NOTE: Exceeded max — also expired
        shared_file.download_count = 15;
        assert!(shared_file.is_expired());
    }

    #[test]
    fn test_requires_password() {
        let mut shared_file = SharedFile {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            filename: "secret.pdf".to_string(),
            content_type: "application/pdf".to_string(),
            file_size: 1024,
            storage_path: "/data/secret.pdf".to_string(),
            download_token: "tok".to_string(),
            download_count: 0,
            max_downloads: None,
            expires_at: None,
            password_hash: None,
            created_at: chrono::Utc::now(),
        };

        assert!(!shared_file.requires_password());

        shared_file.password_hash = Some("$argon2id$hashed".to_string());
        assert!(shared_file.requires_password());
    }

    #[test]
    fn test_generate_download_token() {
        let token1 = generate_download_token();
        let token2 = generate_download_token();

        // NOTE: Tokens should be 64 hex chars (32 bytes)
        assert_eq!(token1.len(), 64);
        assert_eq!(token2.len(), 64);
        // NOTE: Two generated tokens must be unique
        assert_ne!(token1, token2);
        // NOTE: Should be valid hex
        assert!(token1.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_shared_file_optional_fields() {
        let json_str = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "user_id": "550e8400-e29b-41d4-a716-446655440001",
            "filename": "report.docx",
            "content_type": "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "file_size": 54321,
            "storage_path": "/data/shared/report.docx",
            "download_token": "abcdef1234567890",
            "download_count": 0,
            "max_downloads": null,
            "expires_at": null,
            "password_hash": null,
            "created_at": "2026-04-14T10:00:00Z"
        }"#;

        let shared_file: SharedFile = serde_json::from_str(json_str).unwrap();
        assert!(shared_file.max_downloads.is_none());
        assert!(shared_file.expires_at.is_none());
        assert!(shared_file.password_hash.is_none());
        assert_eq!(shared_file.filename, "report.docx");
    }
}
