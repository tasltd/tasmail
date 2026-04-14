// Added: Email embedding model for semantic search (TMAIL-106)
// PURPOSE: Stores vector embeddings of emails for cosine similarity search via pgvector
// CONSTRAINTS: embedding dimension must be 1536 (OpenAI text-embedding-3-small default)
// EXTERNAL: Uses pgvector extension for vector storage and similarity queries

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// PURPOSE: A stored email embedding with its associated metadata
/// NOTE: RLS enforced at DB level via app.current_user_id session var
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EmailEmbedding {
    pub id: Uuid,
    pub user_id: Uuid,
    pub folder: String,
    pub uid: i32,
    pub subject: Option<String>,
    // NOTE: pgvector column is not directly mapped; embeddings are inserted/queried via raw SQL
    pub model_used: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// PURPOSE: Request body for semantic search — user provides a natural language query
#[derive(Debug, Deserialize)]
pub struct SemanticSearchRequest {
    pub query: String,
    #[serde(default = "default_search_limit")]
    pub limit: i64,
}

fn default_search_limit() -> i64 {
    20
}

/// PURPOSE: A single result from a semantic search query with similarity score
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SemanticSearchResult {
    pub folder: String,
    pub uid: i32,
    pub subject: Option<String>,
    pub similarity_score: f64,
}

/// PURPOSE: Request body for indexing a specific email's embedding
#[derive(Debug, Deserialize)]
pub struct IndexEmailRequest {
    pub folder: String,
    pub uid: i32,
    pub subject: Option<String>,
    pub text: String,
}

/// PURPOSE: Statistics about the user's indexed email embeddings
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct IndexStats {
    pub total_indexed: i64,
}

/// PURPOSE: Per-folder count for index statistics breakdown
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct FolderIndexCount {
    pub folder: String,
    pub count: i64,
}

/// PURPOSE: Combined index stats response with total and per-folder breakdown
#[derive(Debug, Serialize)]
pub struct IndexStatsResponse {
    pub total_indexed: i64,
    pub per_folder: Vec<FolderIndexCount>,
}

impl EmailEmbedding {
    /// PURPOSE: Insert or update an email embedding (upsert on user_id+folder+uid)
    /// CONSTRAINTS: embedding_vec must have exactly 1536 dimensions
    pub async fn upsert(
        pool: &PgPool,
        user_id: Uuid,
        folder: &str,
        uid: i32,
        subject: Option<&str>,
        embedding_vec: &[f32],
        model_used: &str,
    ) -> Result<EmailEmbedding, sqlx::Error> {
        // NOTE: Cast the f32 slice to a pgvector-compatible string format
        let embedding_str = format!(
            "[{}]",
            embedding_vec
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );

        sqlx::query_as::<_, EmailEmbedding>(
            "INSERT INTO email_embeddings (user_id, folder, uid, subject, embedding, model_used) \
             VALUES ($1, $2, $3, $4, $5::vector, $6) \
             ON CONFLICT (user_id, folder, uid) DO UPDATE SET \
                subject = EXCLUDED.subject, \
                embedding = EXCLUDED.embedding, \
                model_used = EXCLUDED.model_used, \
                created_at = now() \
             RETURNING id, user_id, folder, uid, subject, model_used, created_at",
        )
        .bind(user_id)
        .bind(folder)
        .bind(uid)
        .bind(subject)
        .bind(&embedding_str)
        .bind(model_used)
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: Search for similar emails using cosine distance on pgvector
    /// CONSTRAINTS: query_embedding must have exactly 1536 dimensions
    pub async fn search_similar(
        pool: &PgPool,
        user_id: Uuid,
        query_embedding: &[f32],
        limit: i64,
    ) -> Result<Vec<SemanticSearchResult>, sqlx::Error> {
        let embedding_str = format!(
            "[{}]",
            query_embedding
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );

        // NOTE: 1 - cosine_distance gives similarity (1.0 = identical, 0.0 = orthogonal)
        sqlx::query_as::<_, SemanticSearchResult>(
            "SELECT folder, uid, subject, \
                    1 - (embedding <=> $2::vector) AS similarity_score \
             FROM email_embeddings \
             WHERE user_id = $1 AND embedding IS NOT NULL \
             ORDER BY embedding <=> $2::vector ASC \
             LIMIT $3",
        )
        .bind(user_id)
        .bind(&embedding_str)
        .bind(limit)
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Get total count of indexed embeddings for a user
    pub async fn count_by_user(pool: &PgPool, user_id: Uuid) -> Result<i64, sqlx::Error> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM email_embeddings WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    /// PURPOSE: Get per-folder counts of indexed embeddings for a user
    pub async fn count_by_folder(
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<Vec<FolderIndexCount>, sqlx::Error> {
        sqlx::query_as::<_, FolderIndexCount>(
            "SELECT folder, COUNT(*) as count \
             FROM email_embeddings \
             WHERE user_id = $1 \
             GROUP BY folder \
             ORDER BY count DESC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semantic_search_request_deserialization() {
        let json = serde_json::json!({
            "query": "emails about quarterly report"
        });
        let request: SemanticSearchRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.query, "emails about quarterly report");
        // NOTE: Default limit should be 20
        assert_eq!(request.limit, 20);
    }

    #[test]
    fn test_semantic_search_request_with_custom_limit() {
        let json = serde_json::json!({
            "query": "meeting notes",
            "limit": 5
        });
        let request: SemanticSearchRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.query, "meeting notes");
        assert_eq!(request.limit, 5);
    }

    #[test]
    fn test_semantic_search_request_missing_query_fails() {
        let json = serde_json::json!({
            "limit": 10
        });
        let result = serde_json::from_value::<SemanticSearchRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_index_email_request_deserialization() {
        let json = serde_json::json!({
            "folder": "INBOX",
            "uid": 42,
            "subject": "Quarterly Report",
            "text": "Please find attached the quarterly report for Q3."
        });
        let request: IndexEmailRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.folder, "INBOX");
        assert_eq!(request.uid, 42);
        assert_eq!(request.subject, Some("Quarterly Report".to_string()));
        assert!(request.text.contains("quarterly report"));
    }

    #[test]
    fn test_index_email_request_minimal() {
        let json = serde_json::json!({
            "folder": "Sent",
            "uid": 10,
            "text": "Some email text"
        });
        let request: IndexEmailRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.folder, "Sent");
        assert_eq!(request.uid, 10);
        assert!(request.subject.is_none());
    }

    #[test]
    fn test_index_email_request_missing_text_fails() {
        let json = serde_json::json!({
            "folder": "INBOX",
            "uid": 1
        });
        let result = serde_json::from_value::<IndexEmailRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_semantic_search_result_serialization() {
        let result = SemanticSearchResult {
            folder: "INBOX".to_string(),
            uid: 42,
            subject: Some("Quarterly Report".to_string()),
            similarity_score: 0.89,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["folder"], "INBOX");
        assert_eq!(json["uid"], 42);
        assert_eq!(json["subject"], "Quarterly Report");
        let score = json["similarity_score"].as_f64().unwrap();
        assert!((score - 0.89).abs() < 0.001);
    }

    #[test]
    fn test_semantic_search_result_null_subject() {
        let result = SemanticSearchResult {
            folder: "Sent".to_string(),
            uid: 10,
            subject: None,
            similarity_score: 0.75,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert!(json["subject"].is_null());
    }

    #[test]
    fn test_index_stats_response_serialization() {
        let response = IndexStatsResponse {
            total_indexed: 150,
            per_folder: vec![
                FolderIndexCount {
                    folder: "INBOX".to_string(),
                    count: 100,
                },
                FolderIndexCount {
                    folder: "Sent".to_string(),
                    count: 50,
                },
            ],
        };
        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["total_indexed"], 150);
        let folders = json["per_folder"].as_array().unwrap();
        assert_eq!(folders.len(), 2);
        assert_eq!(folders[0]["folder"], "INBOX");
        assert_eq!(folders[0]["count"], 100);
    }

    #[test]
    fn test_folder_index_count_serialization() {
        let count = FolderIndexCount {
            folder: "INBOX".to_string(),
            count: 42,
        };
        let json = serde_json::to_value(&count).unwrap();
        assert_eq!(json["folder"], "INBOX");
        assert_eq!(json["count"], 42);
    }
}
