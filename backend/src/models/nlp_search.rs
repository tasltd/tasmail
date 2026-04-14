// Added: NLP search history model for TMAIL-135
// PURPOSE: Stores and retrieves natural language search queries with their AI-parsed parameters
// CONSTRAINTS: RLS enforced at DB level via app.current_user_id session var

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// PURPOSE: A stored NLP search query with its parsed parameters and result count
/// NOTE: RLS enforced at DB level via app.current_user_id session var
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct NlpSearchHistory {
    pub id: Uuid,
    pub user_id: Uuid,
    pub query_text: String,
    pub parsed_params: serde_json::Value,
    pub result_count: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// PURPOSE: Structured search parameters parsed from a natural language query by the AI
/// NOTE: Each field is optional — the AI may only extract some parameters from a given query
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ParsedSearchParams {
    pub from: Option<String>,
    pub to: Option<String>,
    pub subject: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub folder: Option<String>,
    pub has_attachment: Option<bool>,
}

/// PURPOSE: Request body for NLP search — user provides a natural language query
#[derive(Debug, Deserialize)]
pub struct NlpSearchRequest {
    pub query: String,
}

/// PURPOSE: Response from NLP search — includes the parsed params and matching messages
#[derive(Debug, Serialize)]
pub struct NlpSearchResponse {
    pub query: String,
    pub parsed_params: ParsedSearchParams,
    pub result_count: i32,
    pub results: Vec<NlpSearchResultItem>,
}

/// PURPOSE: A single message result from NLP search
#[derive(Debug, Serialize)]
pub struct NlpSearchResultItem {
    pub folder: String,
    pub uid: i32,
    pub subject: Option<String>,
    pub from: Option<String>,
    pub date: Option<String>,
}

impl NlpSearchHistory {
    /// PURPOSE: Create a new NLP search history entry after executing a search
    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        query_text: &str,
        parsed_params: &serde_json::Value,
        result_count: i32,
    ) -> Result<NlpSearchHistory, sqlx::Error> {
        sqlx::query_as::<_, NlpSearchHistory>(
            "INSERT INTO nlp_search_history (user_id, query_text, parsed_params, result_count) \
             VALUES ($1, $2, $3, $4) \
             RETURNING id, user_id, query_text, parsed_params, result_count, created_at",
        )
        .bind(user_id)
        .bind(query_text)
        .bind(parsed_params)
        .bind(result_count)
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: List the user's NLP search history ordered by most recent first
    /// CONSTRAINTS: Limited to 50 entries to avoid excessive data transfer
    pub async fn list_by_user(
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<Vec<NlpSearchHistory>, sqlx::Error> {
        sqlx::query_as::<_, NlpSearchHistory>(
            "SELECT id, user_id, query_text, parsed_params, result_count, created_at \
             FROM nlp_search_history \
             WHERE user_id = $1 \
             ORDER BY created_at DESC \
             LIMIT 50",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Delete all NLP search history for a user
    pub async fn delete_all_by_user(
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM nlp_search_history WHERE user_id = $1",
        )
        .bind(user_id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parsed_search_params_default() {
        let params = ParsedSearchParams::default();
        assert!(params.from.is_none());
        assert!(params.to.is_none());
        assert!(params.subject.is_none());
        assert!(params.keywords.is_empty());
        assert!(params.date_from.is_none());
        assert!(params.date_to.is_none());
        assert!(params.folder.is_none());
        assert!(params.has_attachment.is_none());
    }

    #[test]
    fn test_parsed_search_params_serialization() {
        let params = ParsedSearchParams {
            from: Some("john@example.com".to_string()),
            to: None,
            subject: Some("budget".to_string()),
            keywords: vec!["quarterly".to_string(), "report".to_string()],
            date_from: Some("2025-01-01".to_string()),
            date_to: None,
            folder: Some("INBOX".to_string()),
            has_attachment: Some(true),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["from"], "john@example.com");
        assert_eq!(json["subject"], "budget");
        assert_eq!(json["keywords"].as_array().unwrap().len(), 2);
        assert_eq!(json["date_from"], "2025-01-01");
        assert!(json["to"].is_null());
        assert_eq!(json["folder"], "INBOX");
        assert_eq!(json["has_attachment"], true);
    }

    #[test]
    fn test_parsed_search_params_deserialization() {
        let json = serde_json::json!({
            "from": "alice@example.com",
            "subject": "meeting notes",
            "keywords": ["agenda", "action items"],
            "has_attachment": false
        });
        let params: ParsedSearchParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.from.unwrap(), "alice@example.com");
        assert_eq!(params.subject.unwrap(), "meeting notes");
        assert_eq!(params.keywords.len(), 2);
        assert_eq!(params.has_attachment, Some(false));
        assert!(params.to.is_none());
        assert!(params.date_from.is_none());
    }

    #[test]
    fn test_parsed_search_params_empty_json() {
        // NOTE: Empty JSON should deserialize with defaults due to Option and Default
        let json = serde_json::json!({});
        let params: ParsedSearchParams = serde_json::from_value(json).unwrap();
        assert!(params.from.is_none());
        assert!(params.keywords.is_empty());
    }

    #[test]
    fn test_nlp_search_request_deserialization() {
        let json = serde_json::json!({
            "query": "emails from John about the budget last week"
        });
        let request: NlpSearchRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.query, "emails from John about the budget last week");
    }

    #[test]
    fn test_nlp_search_request_missing_query_fails() {
        let json = serde_json::json!({});
        let result = serde_json::from_value::<NlpSearchRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_nlp_search_response_serialization() {
        let response = NlpSearchResponse {
            query: "find budget emails".to_string(),
            parsed_params: ParsedSearchParams {
                subject: Some("budget".to_string()),
                keywords: vec!["budget".to_string()],
                ..Default::default()
            },
            result_count: 2,
            results: vec![
                NlpSearchResultItem {
                    folder: "INBOX".to_string(),
                    uid: 42,
                    subject: Some("Budget Review Q3".to_string()),
                    from: Some("john@example.com".to_string()),
                    date: Some("2025-03-15".to_string()),
                },
                NlpSearchResultItem {
                    folder: "INBOX".to_string(),
                    uid: 55,
                    subject: Some("Budget Approval".to_string()),
                    from: Some("jane@example.com".to_string()),
                    date: None,
                },
            ],
        };
        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["query"], "find budget emails");
        assert_eq!(json["result_count"], 2);
        let results = json["results"].as_array().unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["uid"], 42);
        assert_eq!(results[0]["subject"], "Budget Review Q3");
        assert_eq!(results[1]["from"], "jane@example.com");
    }

    #[test]
    fn test_nlp_search_result_item_serialization() {
        let item = NlpSearchResultItem {
            folder: "Sent".to_string(),
            uid: 10,
            subject: None,
            from: Some("user@example.com".to_string()),
            date: Some("2025-04-01".to_string()),
        };
        let json = serde_json::to_value(&item).unwrap();
        assert_eq!(json["folder"], "Sent");
        assert_eq!(json["uid"], 10);
        assert!(json["subject"].is_null());
        assert_eq!(json["from"], "user@example.com");
        assert_eq!(json["date"], "2025-04-01");
    }
}
