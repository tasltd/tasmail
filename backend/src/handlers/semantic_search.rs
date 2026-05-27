// Added: Semantic search handlers for TMAIL-106
// PURPOSE: Endpoints for vector similarity search, email indexing, and index statistics
// EXTERNAL: Uses embedding_service for AI provider calls and pgvector queries

use axum::{
    extract::State,
    http::StatusCode,
    Json,
};

use crate::error::AppError;
use crate::models::email_embedding::{
    EmailEmbedding, IndexEmailRequest, IndexStatsResponse, SemanticSearchRequest,
    SemanticSearchResult,
};
use crate::services::auth_service::Claims;
use crate::services::embedding_service;
use crate::state::AppState;

/// PURPOSE: Search emails by meaning using vector similarity
/// POST /api/search/semantic
/// CONSTRAINTS: Requires at least one active AI config for embedding generation
pub async fn semantic_search(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<SemanticSearchRequest>,
) -> Result<Json<Vec<SemanticSearchResult>>, AppError> {
    let user_id = parse_user_id(&claims)?;

    if body.query.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Search query is required for semantic search".to_string(),
        ));
    }

    // Added: Clamp limit to a reasonable range
    let limit = body.limit.clamp(1, 100);

    // Added: Get the user's active AI config and decrypt the API key
    let (config, api_key) =
        embedding_service::get_active_ai_config(&state.db, user_id, &state.config.jwt.secret)
            .await
            .map_err(|err| AppError::BadRequest(err))?;

    // Added: Generate embedding for the search query
    let query_embedding = embedding_service::generate_embedding(
        &config.provider,
        &api_key,
        &config.model_name,
        config.base_url.as_deref(),
        &body.query,
    )
    .await
    .map_err(|err| AppError::BadRequest(format!("Failed to generate search embedding: {}", err)))?;

    // Added: Search for similar emails using pgvector cosine distance
    let results =
        embedding_service::search_similar(&state.db, user_id, &query_embedding, limit).await?;

    Ok(Json(results))
}

/// PURPOSE: Index a specific email by generating and storing its embedding
/// POST /api/search/index
/// CONSTRAINTS: Requires at least one active AI config for embedding generation
pub async fn index_email(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<IndexEmailRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let user_id = parse_user_id(&claims)?;

    if body.text.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Email text is required for indexing".to_string(),
        ));
    }

    // Added: Get the user's active AI config
    let (config, api_key) =
        embedding_service::get_active_ai_config(&state.db, user_id, &state.config.jwt.secret)
            .await
            .map_err(|err| AppError::BadRequest(err))?;

    // Added: Prepare text for embedding — combine subject and body for richer context
    let embedding_text = match &body.subject {
        Some(subject) => format!("{}\n\n{}", subject, body.text),
        None => body.text.clone(),
    };

    // Added: Generate the embedding vector
    let embedding_vec = embedding_service::generate_embedding(
        &config.provider,
        &api_key,
        &config.model_name,
        config.base_url.as_deref(),
        &embedding_text,
    )
    .await
    .map_err(|err| AppError::BadRequest(format!("Failed to generate embedding: {}", err)))?;

    // Added: Store the embedding in the database (upsert)
    let record = EmailEmbedding::upsert(
        &state.db,
        user_id,
        &body.folder,
        body.uid,
        body.subject.as_deref(),
        &embedding_vec,
        &config.model_name,
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "id": record.id,
            "folder": record.folder,
            "uid": record.uid,
            "model_used": record.model_used,
            "indexed": true
        })),
    ))
}

/// PURPOSE: Get indexing statistics — total indexed count and per-folder breakdown
/// GET /api/search/index/stats
pub async fn index_stats(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<IndexStatsResponse>, AppError> {
    let user_id = parse_user_id(&claims)?;

    let total_indexed = EmailEmbedding::count_by_user(&state.db, user_id).await?;
    let per_folder = EmailEmbedding::count_by_folder(&state.db, user_id).await?;

    Ok(Json(IndexStatsResponse {
        total_indexed,
        per_folder,
    }))
}

fn parse_user_id(claims: &Claims) -> Result<uuid::Uuid, AppError> {
    claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid user ID in JWT claims")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::auth_service::Claims;

    #[test]
    fn test_parse_user_id_valid() {
        let claims = Claims {
            sub: uuid::Uuid::new_v4().to_string(),
            username: "test@example.com".into(),
            is_admin: false,
            is_compliance_officer: false,
            exp: 0,
            iat: 0,
        };
        assert!(parse_user_id(&claims).is_ok());
    }

    #[test]
    fn test_parse_user_id_invalid() {
        let claims = Claims {
            sub: "not-a-uuid".into(),
            username: "test@example.com".into(),
            is_admin: false,
            is_compliance_officer: false,
            exp: 0,
            iat: 0,
        };
        assert!(parse_user_id(&claims).is_err());
    }

    #[test]
    fn test_semantic_search_request_validation() {
        // NOTE: Handler validates empty queries; test the request struct
        let json = serde_json::json!({
            "query": "find emails about budget review",
            "limit": 10
        });
        let request: SemanticSearchRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.query, "find emails about budget review");
        assert_eq!(request.limit, 10);
    }

    #[test]
    fn test_index_email_request_validation() {
        let json = serde_json::json!({
            "folder": "INBOX",
            "uid": 99,
            "subject": "Project Update",
            "text": "Here is the latest status on the project..."
        });
        let request: IndexEmailRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.folder, "INBOX");
        assert_eq!(request.uid, 99);
        assert_eq!(request.subject.unwrap(), "Project Update");
    }
}
