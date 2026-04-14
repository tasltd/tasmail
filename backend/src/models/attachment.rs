// Added: Attachment model with storage metadata and ClamAV scan tracking for TMAIL-59
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// PURPOSE: Represents a stored attachment with virus scan metadata
/// CONSTRAINTS: mailbox_id must reference a valid mailbox; checksum is SHA-256 hex
/// EXTERNAL: PostgreSQL via sqlx
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Attachment {
    pub id: Uuid,
    pub mailbox_id: Uuid,
    pub message_uid: Option<i32>,
    pub folder: Option<String>,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: i64,
    pub storage_path: String,
    pub checksum: String,
    pub scan_status: String,
    pub scan_result: Option<String>,
    pub scanned_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// PURPOSE: Aggregated storage statistics for a mailbox
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    pub total_count: i64,
    pub total_size_bytes: i64,
    pub pending_scans: i64,
    pub infected_count: i64,
}

impl Attachment {
    /// PURPOSE: Insert a new attachment record
    pub async fn create(
        pool: &PgPool,
        mailbox_id: Uuid,
        filename: &str,
        content_type: &str,
        size_bytes: i64,
        storage_path: &str,
        checksum: &str,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as::<_, Attachment>(
            "INSERT INTO attachments (mailbox_id, filename, content_type, size_bytes, storage_path, checksum)
             VALUES ($1, $2, $3, $4, $5, $6) RETURNING *",
        )
        .bind(mailbox_id)
        .bind(filename)
        .bind(content_type)
        .bind(size_bytes)
        .bind(storage_path)
        .bind(checksum)
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: Find attachment by ID (no mailbox filter — RLS handles isolation)
    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Attachment>("SELECT * FROM attachments WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    /// PURPOSE: Deduplicate by checking if an identical file already exists for this mailbox
    pub async fn find_by_checksum(
        pool: &PgPool,
        mailbox_id: Uuid,
        checksum: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Attachment>(
            "SELECT * FROM attachments WHERE mailbox_id = $1 AND checksum = $2 LIMIT 1",
        )
        .bind(mailbox_id)
        .bind(checksum)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: List all attachments for a mailbox, newest first
    pub async fn list_by_mailbox(
        pool: &PgPool,
        mailbox_id: Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Attachment>(
            "SELECT * FROM attachments WHERE mailbox_id = $1 ORDER BY created_at DESC",
        )
        .bind(mailbox_id)
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Update virus scan status after ClamAV scan completes
    pub async fn update_scan_status(
        pool: &PgPool,
        id: Uuid,
        status: &str,
        result: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE attachments SET scan_status = $1, scan_result = $2, scanned_at = NOW() WHERE id = $3",
        )
        .bind(status)
        .bind(result)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// PURPOSE: Delete attachment record by ID with mailbox ownership check
    pub async fn delete(pool: &PgPool, id: Uuid, mailbox_id: Uuid) -> Result<bool, sqlx::Error> {
        let result =
            sqlx::query("DELETE FROM attachments WHERE id = $1 AND mailbox_id = $2")
                .bind(id)
                .bind(mailbox_id)
                .execute(pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    /// PURPOSE: Get aggregated storage stats for a mailbox
    pub async fn storage_stats(
        pool: &PgPool,
        mailbox_id: Uuid,
    ) -> Result<StorageStats, sqlx::Error> {
        // NOTE: Using COALESCE to handle empty result sets gracefully
        let row = sqlx::query_as::<_, (i64, i64, i64, i64)>(
            "SELECT
                COALESCE(COUNT(*), 0),
                COALESCE(SUM(size_bytes), 0),
                COALESCE(COUNT(*) FILTER (WHERE scan_status = 'pending'), 0),
                COALESCE(COUNT(*) FILTER (WHERE scan_status = 'infected'), 0)
             FROM attachments WHERE mailbox_id = $1",
        )
        .bind(mailbox_id)
        .fetch_one(pool)
        .await?;

        Ok(StorageStats {
            total_count: row.0,
            total_size_bytes: row.1,
            pending_scans: row.2,
            infected_count: row.3,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attachment_serialization() {
        let id = Uuid::new_v4();
        let mailbox_id = Uuid::new_v4();
        let now = chrono::Utc::now();

        let attachment = Attachment {
            id,
            mailbox_id,
            message_uid: Some(42),
            folder: Some("INBOX".to_string()),
            filename: "report.pdf".to_string(),
            content_type: "application/pdf".to_string(),
            size_bytes: 1024000,
            storage_path: "/data/attachments/test/file.pdf".to_string(),
            checksum: "abc123def456".to_string(),
            scan_status: "clean".to_string(),
            scan_result: None,
            scanned_at: Some(now),
            created_at: now,
        };

        let json = serde_json::to_value(&attachment).unwrap();
        assert_eq!(json["id"], id.to_string());
        assert_eq!(json["mailbox_id"], mailbox_id.to_string());
        assert_eq!(json["filename"], "report.pdf");
        assert_eq!(json["content_type"], "application/pdf");
        assert_eq!(json["size_bytes"], 1024000);
        assert_eq!(json["scan_status"], "clean");
        assert_eq!(json["message_uid"], 42);
    }

    #[test]
    fn test_attachment_roundtrip() {
        let attachment = Attachment {
            id: Uuid::new_v4(),
            mailbox_id: Uuid::new_v4(),
            message_uid: None,
            folder: None,
            filename: "image.png".to_string(),
            content_type: "image/png".to_string(),
            size_bytes: 5000,
            storage_path: "/data/test.png".to_string(),
            checksum: "sha256hash".to_string(),
            scan_status: "pending".to_string(),
            scan_result: None,
            scanned_at: None,
            created_at: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&attachment).unwrap();
        let deserialized: Attachment = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, attachment.id);
        assert_eq!(deserialized.filename, "image.png");
        assert_eq!(deserialized.scan_status, "pending");
        assert!(deserialized.scanned_at.is_none());
    }

    #[test]
    fn test_storage_stats_serialization() {
        let stats = StorageStats {
            total_count: 15,
            total_size_bytes: 52428800,
            pending_scans: 2,
            infected_count: 0,
        };

        let json = serde_json::to_value(&stats).unwrap();
        assert_eq!(json["total_count"], 15);
        assert_eq!(json["total_size_bytes"], 52428800);
        assert_eq!(json["pending_scans"], 2);
        assert_eq!(json["infected_count"], 0);
    }

    #[test]
    fn test_storage_stats_empty() {
        let stats = StorageStats {
            total_count: 0,
            total_size_bytes: 0,
            pending_scans: 0,
            infected_count: 0,
        };

        let json = serde_json::to_value(&stats).unwrap();
        assert_eq!(json["total_count"], 0);
        assert_eq!(json["total_size_bytes"], 0);
    }

    #[test]
    fn test_attachment_with_infected_status() {
        let attachment = Attachment {
            id: Uuid::new_v4(),
            mailbox_id: Uuid::new_v4(),
            message_uid: None,
            folder: None,
            filename: "malware.exe".to_string(),
            content_type: "application/octet-stream".to_string(),
            size_bytes: 99999,
            storage_path: "/data/quarantine/malware.exe".to_string(),
            checksum: "badfilehash".to_string(),
            scan_status: "infected".to_string(),
            scan_result: Some("Win.Trojan.Agent-123456".to_string()),
            scanned_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
        };

        let json = serde_json::to_value(&attachment).unwrap();
        assert_eq!(json["scan_status"], "infected");
        assert_eq!(json["scan_result"], "Win.Trojan.Agent-123456");
    }

    #[test]
    fn test_attachment_optional_fields() {
        // NOTE: message_uid and folder are optional — attachment may not be linked to a message yet
        let json_str = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "mailbox_id": "550e8400-e29b-41d4-a716-446655440001",
            "message_uid": null,
            "folder": null,
            "filename": "draft-attachment.docx",
            "content_type": "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "size_bytes": 12345,
            "storage_path": "/data/test.docx",
            "checksum": "abc",
            "scan_status": "pending",
            "scan_result": null,
            "scanned_at": null,
            "created_at": "2026-04-14T10:00:00Z"
        }"#;

        let attachment: Attachment = serde_json::from_str(json_str).unwrap();
        assert!(attachment.message_uid.is_none());
        assert!(attachment.folder.is_none());
        assert_eq!(attachment.filename, "draft-attachment.docx");
    }
}
