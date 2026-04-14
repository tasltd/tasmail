// Added: eDiscovery search models for compliance and legal investigations (TMAIL-137)

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// PURPOSE: Represents the status of an eDiscovery search
/// CONSTRAINTS: Must match the ediscovery_status enum in PostgreSQL
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[sqlx(type_name = "ediscovery_status", rename_all = "lowercase")]
pub enum EdiscoveryStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Exported,
}

/// PURPOSE: Represents an eDiscovery search created by an admin
/// CONSTRAINTS: admin_id must reference a valid admin user
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EdiscoverySearch {
    pub id: Uuid,
    pub admin_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub search_query: String,
    pub target_users: Option<Vec<Uuid>>,
    pub date_from: Option<chrono::DateTime<chrono::Utc>>,
    pub date_to: Option<chrono::DateTime<chrono::Utc>>,
    pub include_attachments: bool,
    pub status: EdiscoveryStatus,
    pub results_count: Option<i32>,
    pub export_path: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// PURPOSE: Represents a single result from an eDiscovery search
/// NOTE: References an email by user_id + folder + uid (IMAP coordinates)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EdiscoveryResult {
    pub id: Uuid,
    pub search_id: Uuid,
    pub user_id: Uuid,
    pub folder: String,
    pub uid: i32,
    pub subject: Option<String>,
    pub from_address: Option<String>,
    pub date: Option<chrono::DateTime<chrono::Utc>>,
    pub snippet: Option<String>,
    pub relevance_score: Option<f32>,
}

/// PURPOSE: Request body for creating a new eDiscovery search
#[derive(Debug, Deserialize)]
pub struct CreateEdiscoveryRequest {
    pub name: String,
    pub description: Option<String>,
    pub search_query: String,
    pub target_users: Option<Vec<Uuid>>,
    pub date_from: Option<chrono::DateTime<chrono::Utc>>,
    pub date_to: Option<chrono::DateTime<chrono::Utc>>,
    pub include_attachments: Option<bool>,
}

/// PURPOSE: Combined search with its results for the detail endpoint
#[derive(Debug, Serialize)]
pub struct EdiscoverySearchWithResults {
    #[serde(flatten)]
    pub search: EdiscoverySearch,
    pub results: Vec<EdiscoveryResult>,
}

impl EdiscoverySearch {
    /// PURPOSE: List all eDiscovery searches ordered by creation date (newest first)
    pub async fn find_all(pool: &PgPool) -> Result<Vec<EdiscoverySearch>, sqlx::Error> {
        sqlx::query_as::<_, EdiscoverySearch>(
            "SELECT * FROM ediscovery_searches ORDER BY created_at DESC",
        )
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Get a single eDiscovery search by ID
    pub async fn find_by_id(
        pool: &PgPool,
        id: Uuid,
    ) -> Result<Option<EdiscoverySearch>, sqlx::Error> {
        sqlx::query_as::<_, EdiscoverySearch>(
            "SELECT * FROM ediscovery_searches WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: Create a new eDiscovery search with pending status
    pub async fn create(
        pool: &PgPool,
        admin_id: Uuid,
        input: &CreateEdiscoveryRequest,
    ) -> Result<EdiscoverySearch, sqlx::Error> {
        sqlx::query_as::<_, EdiscoverySearch>(
            "INSERT INTO ediscovery_searches (admin_id, name, description, search_query, target_users, date_from, date_to, include_attachments) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING *",
        )
        .bind(admin_id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&input.search_query)
        .bind(&input.target_users)
        .bind(input.date_from)
        .bind(input.date_to)
        .bind(input.include_attachments.unwrap_or(false))
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: Update the status of an eDiscovery search
    pub async fn update_status(
        pool: &PgPool,
        id: Uuid,
        status: &EdiscoveryStatus,
        results_count: Option<i32>,
    ) -> Result<Option<EdiscoverySearch>, sqlx::Error> {
        sqlx::query_as::<_, EdiscoverySearch>(
            "UPDATE ediscovery_searches SET status = $2, results_count = COALESCE($3, results_count), \
             completed_at = CASE WHEN $2 IN ('completed', 'failed', 'exported') THEN NOW() ELSE completed_at END \
             WHERE id = $1 RETURNING *",
        )
        .bind(id)
        .bind(status)
        .bind(results_count)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: Set the export path after results are exported to MBOX
    pub async fn set_export_path(
        pool: &PgPool,
        id: Uuid,
        export_path: &str,
    ) -> Result<Option<EdiscoverySearch>, sqlx::Error> {
        sqlx::query_as::<_, EdiscoverySearch>(
            "UPDATE ediscovery_searches SET export_path = $2, status = 'exported' WHERE id = $1 RETURNING *",
        )
        .bind(id)
        .bind(export_path)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: Delete an eDiscovery search and its results (cascaded)
    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM ediscovery_searches WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

impl EdiscoveryResult {
    /// PURPOSE: Find all results for a given search ID
    pub async fn find_by_search(
        pool: &PgPool,
        search_id: Uuid,
    ) -> Result<Vec<EdiscoveryResult>, sqlx::Error> {
        sqlx::query_as::<_, EdiscoveryResult>(
            "SELECT * FROM ediscovery_results WHERE search_id = $1 ORDER BY relevance_score DESC NULLS LAST",
        )
        .bind(search_id)
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Insert a batch of search results
    /// NOTE: Uses a single INSERT with multiple value tuples for efficiency
    pub async fn insert_batch(
        pool: &PgPool,
        search_id: Uuid,
        results: &[EdiscoveryResult],
    ) -> Result<u64, sqlx::Error> {
        if results.is_empty() {
            return Ok(0);
        }

        // Added: Build batched insert for efficiency
        let mut query_builder = sqlx::QueryBuilder::new(
            "INSERT INTO ediscovery_results (search_id, user_id, folder, uid, subject, from_address, date, snippet, relevance_score) ",
        );

        query_builder.push_values(results.iter(), |mut builder, result| {
            builder
                .push_bind(search_id)
                .push_bind(result.user_id)
                .push_bind(&result.folder)
                .push_bind(result.uid)
                .push_bind(&result.subject)
                .push_bind(&result.from_address)
                .push_bind(result.date)
                .push_bind(&result.snippet)
                .push_bind(result.relevance_score);
        });

        let query = query_builder.build();
        let result = query.execute(pool).await?;
        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ediscovery_status_serialization() {
        let status = EdiscoveryStatus::Pending;
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json, "Pending");

        let status = EdiscoveryStatus::Completed;
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json, "Completed");
    }

    #[test]
    fn test_ediscovery_search_serialization() {
        let search = EdiscoverySearch {
            id: Uuid::new_v4(),
            admin_id: Uuid::new_v4(),
            name: "Q1 Compliance Review".to_string(),
            description: Some("Search for financial data".to_string()),
            search_query: "quarterly report".to_string(),
            target_users: Some(vec![Uuid::new_v4()]),
            date_from: Some(chrono::Utc::now()),
            date_to: None,
            include_attachments: true,
            status: EdiscoveryStatus::Pending,
            results_count: Some(0),
            export_path: None,
            created_at: chrono::Utc::now(),
            completed_at: None,
        };

        let json = serde_json::to_value(&search).unwrap();
        assert_eq!(json["name"], "Q1 Compliance Review");
        assert_eq!(json["search_query"], "quarterly report");
        assert_eq!(json["include_attachments"], true);
        assert_eq!(json["status"], "Pending");
    }

    #[test]
    fn test_ediscovery_search_roundtrip() {
        let search = EdiscoverySearch {
            id: Uuid::new_v4(),
            admin_id: Uuid::new_v4(),
            name: "Legal Investigation".to_string(),
            description: None,
            search_query: "contract breach".to_string(),
            target_users: None,
            date_from: None,
            date_to: None,
            include_attachments: false,
            status: EdiscoveryStatus::Running,
            results_count: None,
            export_path: None,
            created_at: chrono::Utc::now(),
            completed_at: None,
        };

        let json = serde_json::to_string(&search).unwrap();
        let deserialized: EdiscoverySearch = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, search.id);
        assert_eq!(deserialized.name, "Legal Investigation");
        assert_eq!(deserialized.status, EdiscoveryStatus::Running);
    }

    #[test]
    fn test_ediscovery_result_serialization() {
        let result = EdiscoveryResult {
            id: Uuid::new_v4(),
            search_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            folder: "INBOX".to_string(),
            uid: 42,
            subject: Some("Re: Contract Discussion".to_string()),
            from_address: Some("alice@example.com".to_string()),
            date: Some(chrono::Utc::now()),
            snippet: Some("Regarding the contract terms...".to_string()),
            relevance_score: Some(0.95),
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["folder"], "INBOX");
        assert_eq!(json["uid"], 42);
        assert_eq!(json["subject"], "Re: Contract Discussion");
        assert_eq!(json["from_address"], "alice@example.com");
    }

    #[test]
    fn test_ediscovery_result_roundtrip() {
        let result = EdiscoveryResult {
            id: Uuid::new_v4(),
            search_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            folder: "Sent".to_string(),
            uid: 100,
            subject: None,
            from_address: None,
            date: None,
            snippet: None,
            relevance_score: None,
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: EdiscoveryResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, result.id);
        assert_eq!(deserialized.folder, "Sent");
        assert_eq!(deserialized.uid, 100);
        assert!(deserialized.subject.is_none());
    }

    #[test]
    fn test_create_ediscovery_request_deserialization() {
        let json = serde_json::json!({
            "name": "Investigation Alpha",
            "description": "Looking for sensitive data",
            "search_query": "confidential",
            "target_users": [Uuid::new_v4().to_string()],
            "date_from": "2026-01-01T00:00:00Z",
            "date_to": "2026-04-01T00:00:00Z",
            "include_attachments": true
        });

        let request: CreateEdiscoveryRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.name, "Investigation Alpha");
        assert_eq!(request.search_query, "confidential");
        assert!(request.include_attachments.unwrap());
        assert!(request.target_users.unwrap().len() == 1);
    }

    #[test]
    fn test_create_ediscovery_request_minimal() {
        let json = serde_json::json!({
            "name": "Quick Search",
            "search_query": "keyword"
        });

        let request: CreateEdiscoveryRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.name, "Quick Search");
        assert_eq!(request.search_query, "keyword");
        assert!(request.description.is_none());
        assert!(request.target_users.is_none());
        assert!(request.date_from.is_none());
        assert!(request.include_attachments.is_none());
    }

    #[test]
    fn test_create_ediscovery_request_missing_required_fails() {
        let json = serde_json::json!({
            "name": "Missing query"
        });
        let result = serde_json::from_value::<CreateEdiscoveryRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_ediscovery_search_with_results_serialization() {
        let search = EdiscoverySearch {
            id: Uuid::new_v4(),
            admin_id: Uuid::new_v4(),
            name: "Test Search".to_string(),
            description: None,
            search_query: "test".to_string(),
            target_users: None,
            date_from: None,
            date_to: None,
            include_attachments: false,
            status: EdiscoveryStatus::Completed,
            results_count: Some(1),
            export_path: None,
            created_at: chrono::Utc::now(),
            completed_at: Some(chrono::Utc::now()),
        };

        let with_results = EdiscoverySearchWithResults {
            search,
            results: vec![],
        };

        let json = serde_json::to_value(&with_results).unwrap();
        // NOTE: #[serde(flatten)] merges search fields into top level
        assert_eq!(json["name"], "Test Search");
        assert_eq!(json["results"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_ediscovery_status_all_variants() {
        let variants = vec![
            EdiscoveryStatus::Pending,
            EdiscoveryStatus::Running,
            EdiscoveryStatus::Completed,
            EdiscoveryStatus::Failed,
            EdiscoveryStatus::Exported,
        ];

        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let deserialized: EdiscoveryStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, variant);
        }
    }
}
