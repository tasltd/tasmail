// Added: Email archive models for Piler integration (TMAIL-107)
// PURPOSE: Structs and CRUD operations for archive policies, config, and search history

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// PURPOSE: Represents an admin-defined archiving policy
/// CONSTRAINTS: archive_after_days must be > 0, match_criteria is JSONB with domains/folders
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ArchivePolicy {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub match_criteria: serde_json::Value,
    pub archive_after_days: i32,
    pub delete_original: bool,
    pub enabled: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// PURPOSE: Tracks a user's archive search for audit trail and re-search
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ArchiveSearch {
    pub id: Uuid,
    pub user_id: Uuid,
    pub query: String,
    pub filters: Option<serde_json::Value>,
    pub result_count: Option<i32>,
    pub searched_at: chrono::DateTime<chrono::Utc>,
}

/// PURPOSE: Global Piler archive server configuration (admin-managed)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ArchiveConfig {
    pub id: Uuid,
    pub piler_url: Option<String>,
    pub piler_api_key_encrypted: Option<String>,
    pub retention_years: i32,
    pub enabled: bool,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

// Added: Request types for creating/updating archive policies
#[derive(Debug, Deserialize)]
pub struct CreateArchivePolicyRequest {
    pub name: String,
    pub description: Option<String>,
    pub match_criteria: Option<serde_json::Value>,
    pub archive_after_days: Option<i32>,
    pub delete_original: Option<bool>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateArchivePolicyRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub match_criteria: Option<serde_json::Value>,
    pub archive_after_days: Option<i32>,
    pub delete_original: Option<bool>,
    pub enabled: Option<bool>,
}

// Added: Request type for updating archive config
#[derive(Debug, Deserialize)]
pub struct UpdateArchiveConfigRequest {
    pub piler_url: Option<String>,
    pub piler_api_key: Option<String>,
    pub retention_years: Option<i32>,
    pub enabled: Option<bool>,
}

// Added: Request type for archive search
#[derive(Debug, Deserialize, Serialize)]
pub struct ArchiveSearchRequest {
    pub query: String,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub sender: Option<String>,
    pub recipient: Option<String>,
}

// Added: Response type for archive search results (from Piler or mock)
#[derive(Debug, Serialize, Deserialize)]
pub struct ArchiveSearchResult {
    pub id: String,
    pub subject: String,
    pub sender: String,
    pub recipients: Vec<String>,
    pub date: String,
    pub size: i64,
    pub has_attachment: bool,
}

impl ArchivePolicy {
    /// PURPOSE: List all archive policies ordered by creation date
    pub async fn find_all(pool: &PgPool) -> Result<Vec<ArchivePolicy>, sqlx::Error> {
        sqlx::query_as::<_, ArchivePolicy>(
            "SELECT * FROM archive_policies ORDER BY created_at DESC",
        )
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Create a new archive policy
    pub async fn create(
        pool: &PgPool,
        input: &CreateArchivePolicyRequest,
    ) -> Result<ArchivePolicy, sqlx::Error> {
        sqlx::query_as::<_, ArchivePolicy>(
            "INSERT INTO archive_policies (name, description, match_criteria, archive_after_days, delete_original, enabled) \
             VALUES ($1, $2, $3, $4, $5, $6) RETURNING *",
        )
        .bind(&input.name)
        .bind(&input.description)
        .bind(input.match_criteria.as_ref().unwrap_or(&serde_json::json!({})))
        .bind(input.archive_after_days.unwrap_or(90))
        .bind(input.delete_original.unwrap_or(false))
        .bind(input.enabled.unwrap_or(true))
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: Update an existing archive policy by ID
    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        input: &UpdateArchivePolicyRequest,
    ) -> Result<Option<ArchivePolicy>, sqlx::Error> {
        sqlx::query_as::<_, ArchivePolicy>(
            "UPDATE archive_policies SET \
                name = COALESCE($2, name), \
                description = COALESCE($3, description), \
                match_criteria = COALESCE($4, match_criteria), \
                archive_after_days = COALESCE($5, archive_after_days), \
                delete_original = COALESCE($6, delete_original), \
                enabled = COALESCE($7, enabled), \
                updated_at = NOW() \
             WHERE id = $1 RETURNING *",
        )
        .bind(id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&input.match_criteria)
        .bind(input.archive_after_days)
        .bind(input.delete_original)
        .bind(input.enabled)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: Delete an archive policy by ID
    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM archive_policies WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

impl ArchiveConfig {
    /// PURPOSE: Get the current archive config (first row, or None if not configured)
    pub async fn get(pool: &PgPool) -> Result<Option<ArchiveConfig>, sqlx::Error> {
        sqlx::query_as::<_, ArchiveConfig>(
            "SELECT * FROM archive_config ORDER BY updated_at DESC LIMIT 1",
        )
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: Upsert archive config — creates if none exists, updates if it does
    pub async fn upsert(
        pool: &PgPool,
        input: &UpdateArchiveConfigRequest,
    ) -> Result<ArchiveConfig, sqlx::Error> {
        // NOTE: Check if config exists, update or insert accordingly
        let existing = Self::get(pool).await?;
        if let Some(config) = existing {
            sqlx::query_as::<_, ArchiveConfig>(
                "UPDATE archive_config SET \
                    piler_url = COALESCE($2, piler_url), \
                    piler_api_key_encrypted = COALESCE($3, piler_api_key_encrypted), \
                    retention_years = COALESCE($4, retention_years), \
                    enabled = COALESCE($5, enabled), \
                    updated_at = NOW() \
                 WHERE id = $1 RETURNING *",
            )
            .bind(config.id)
            .bind(&input.piler_url)
            .bind(&input.piler_api_key)
            .bind(input.retention_years)
            .bind(input.enabled)
            .fetch_one(pool)
            .await
        } else {
            sqlx::query_as::<_, ArchiveConfig>(
                "INSERT INTO archive_config (piler_url, piler_api_key_encrypted, retention_years, enabled) \
                 VALUES ($1, $2, $3, $4) RETURNING *",
            )
            .bind(&input.piler_url)
            .bind(&input.piler_api_key)
            .bind(input.retention_years.unwrap_or(7))
            .bind(input.enabled.unwrap_or(false))
            .fetch_one(pool)
            .await
        }
    }
}

impl ArchiveSearch {
    /// PURPOSE: Record a new archive search entry for audit
    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        query: &str,
        filters: Option<&serde_json::Value>,
        result_count: Option<i32>,
    ) -> Result<ArchiveSearch, sqlx::Error> {
        sqlx::query_as::<_, ArchiveSearch>(
            "INSERT INTO archive_searches (user_id, query, filters, result_count) \
             VALUES ($1, $2, $3, $4) RETURNING *",
        )
        .bind(user_id)
        .bind(query)
        .bind(filters)
        .bind(result_count)
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: List search history for a user, most recent first
    pub async fn find_by_user(
        pool: &PgPool,
        user_id: Uuid,
        limit: i64,
    ) -> Result<Vec<ArchiveSearch>, sqlx::Error> {
        sqlx::query_as::<_, ArchiveSearch>(
            "SELECT * FROM archive_searches WHERE user_id = $1 ORDER BY searched_at DESC LIMIT $2",
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_archive_policy_serialization() {
        // Added: Verify ArchivePolicy serializes all fields correctly
        let policy = ArchivePolicy {
            id: Uuid::new_v4(),
            name: "Archive All INBOX".to_string(),
            description: Some("Archive all inbox emails after 90 days".to_string()),
            match_criteria: serde_json::json!({"domains": ["*"], "folders": ["INBOX"]}),
            archive_after_days: 90,
            delete_original: false,
            enabled: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let json = serde_json::to_value(&policy).unwrap();
        assert_eq!(json["name"], "Archive All INBOX");
        assert_eq!(json["archive_after_days"], 90);
        assert_eq!(json["delete_original"], false);
        assert_eq!(json["enabled"], true);
    }

    #[test]
    fn test_archive_policy_roundtrip() {
        // Added: Verify ArchivePolicy survives serialization roundtrip
        let policy = ArchivePolicy {
            id: Uuid::new_v4(),
            name: "Sent Archive".to_string(),
            description: None,
            match_criteria: serde_json::json!({"folders": ["Sent"]}),
            archive_after_days: 365,
            delete_original: true,
            enabled: false,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&policy).unwrap();
        let deserialized: ArchivePolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, policy.id);
        assert_eq!(deserialized.name, "Sent Archive");
        assert_eq!(deserialized.archive_after_days, 365);
        assert!(deserialized.delete_original);
        assert!(!deserialized.enabled);
    }

    #[test]
    fn test_archive_policy_match_criteria_structure() {
        // Added: Verify match_criteria JSONB contains expected keys
        let policy = ArchivePolicy {
            id: Uuid::new_v4(),
            name: "Selective Archive".to_string(),
            description: None,
            match_criteria: serde_json::json!({
                "domains": ["example.com", "test.org"],
                "folders": ["INBOX", "Sent"]
            }),
            archive_after_days: 60,
            delete_original: false,
            enabled: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let criteria = &policy.match_criteria;
        assert!(criteria["domains"].is_array());
        assert_eq!(criteria["domains"].as_array().unwrap().len(), 2);
        assert_eq!(criteria["folders"][0], "INBOX");
    }

    #[test]
    fn test_archive_config_serialization() {
        // Added: Verify ArchiveConfig serializes correctly
        let config = ArchiveConfig {
            id: Uuid::new_v4(),
            piler_url: Some("https://piler.example.com".to_string()),
            piler_api_key_encrypted: Some("enc-key-xxx".to_string()),
            retention_years: 7,
            enabled: true,
            updated_at: chrono::Utc::now(),
        };

        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["piler_url"], "https://piler.example.com");
        assert_eq!(json["retention_years"], 7);
        assert_eq!(json["enabled"], true);
    }

    #[test]
    fn test_archive_config_roundtrip() {
        // Added: Verify ArchiveConfig survives serialization roundtrip
        let config = ArchiveConfig {
            id: Uuid::new_v4(),
            piler_url: None,
            piler_api_key_encrypted: None,
            retention_years: 10,
            enabled: false,
            updated_at: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ArchiveConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, config.id);
        assert!(deserialized.piler_url.is_none());
        assert_eq!(deserialized.retention_years, 10);
        assert!(!deserialized.enabled);
    }

    #[test]
    fn test_archive_search_serialization() {
        // Added: Verify ArchiveSearch serializes correctly
        let search = ArchiveSearch {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            query: "invoice from:accounting".to_string(),
            filters: Some(serde_json::json!({"date_from": "2025-01-01"})),
            result_count: Some(42),
            searched_at: chrono::Utc::now(),
        };

        let json = serde_json::to_value(&search).unwrap();
        assert_eq!(json["query"], "invoice from:accounting");
        assert_eq!(json["result_count"], 42);
    }

    #[test]
    fn test_archive_search_roundtrip() {
        // Added: Verify ArchiveSearch roundtrip with null optional fields
        let search = ArchiveSearch {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            query: "contract".to_string(),
            filters: None,
            result_count: None,
            searched_at: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&search).unwrap();
        let deserialized: ArchiveSearch = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.query, "contract");
        assert!(deserialized.filters.is_none());
        assert!(deserialized.result_count.is_none());
    }

    #[test]
    fn test_create_archive_policy_request_deserialization() {
        // Added: Verify CreateArchivePolicyRequest parses full payload
        let json = serde_json::json!({
            "name": "Archive HR emails",
            "description": "Archive HR folder after 30 days",
            "match_criteria": {"folders": ["HR"]},
            "archive_after_days": 30,
            "delete_original": true,
            "enabled": true
        });

        let request: CreateArchivePolicyRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.name, "Archive HR emails");
        assert_eq!(request.archive_after_days, Some(30));
        assert_eq!(request.delete_original, Some(true));
    }

    #[test]
    fn test_create_archive_policy_request_minimal() {
        // Added: Verify CreateArchivePolicyRequest works with only required fields
        let json = serde_json::json!({
            "name": "Default archive"
        });

        let request: CreateArchivePolicyRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.name, "Default archive");
        assert!(request.description.is_none());
        assert!(request.match_criteria.is_none());
        assert!(request.archive_after_days.is_none());
    }

    #[test]
    fn test_update_archive_config_request_deserialization() {
        // Added: Verify UpdateArchiveConfigRequest partial update
        let json = serde_json::json!({
            "piler_url": "https://piler.local:8080",
            "enabled": true
        });

        let request: UpdateArchiveConfigRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.piler_url, Some("https://piler.local:8080".to_string()));
        assert_eq!(request.enabled, Some(true));
        assert!(request.piler_api_key.is_none());
        assert!(request.retention_years.is_none());
    }

    #[test]
    fn test_archive_search_request_deserialization() {
        // Added: Verify ArchiveSearchRequest with filters
        let json = serde_json::json!({
            "query": "quarterly report",
            "date_from": "2025-01-01",
            "date_to": "2025-12-31",
            "sender": "cfo@example.com"
        });

        let request: ArchiveSearchRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.query, "quarterly report");
        assert_eq!(request.date_from, Some("2025-01-01".to_string()));
        assert_eq!(request.sender, Some("cfo@example.com".to_string()));
    }

    #[test]
    fn test_archive_search_result_serialization() {
        // Added: Verify ArchiveSearchResult output format
        let result = ArchiveSearchResult {
            id: "piler-msg-001".to_string(),
            subject: "Q4 Financial Report".to_string(),
            sender: "cfo@example.com".to_string(),
            recipients: vec!["board@example.com".to_string()],
            date: "2025-12-15T10:00:00Z".to_string(),
            size: 102400,
            has_attachment: true,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["subject"], "Q4 Financial Report");
        assert_eq!(json["size"], 102400);
        assert_eq!(json["has_attachment"], true);
        assert_eq!(json["recipients"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_create_archive_policy_request_missing_name_fails() {
        // Added: Verify required field validation
        let json = serde_json::json!({
            "archive_after_days": 30
        });
        let result = serde_json::from_value::<CreateArchivePolicyRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_archive_search_request_minimal() {
        // Added: Verify ArchiveSearchRequest with only required query field
        let json = serde_json::json!({
            "query": "hello"
        });

        let request: ArchiveSearchRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.query, "hello");
        assert!(request.date_from.is_none());
        assert!(request.date_to.is_none());
        assert!(request.sender.is_none());
        assert!(request.recipient.is_none());
    }
}
