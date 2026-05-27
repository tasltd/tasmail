// Added: NLP search handlers for TMAIL-135
// PURPOSE: Endpoints for AI-powered natural language email search, with history tracking
// EXTERNAL: Uses nlp_parser service for AI query parsing and ai_client for LLM calls

use axum::{
    extract::State,
    http::StatusCode,
    Json,
};

use crate::error::AppError;
use crate::models::nlp_search::{
    NlpSearchHistory, NlpSearchRequest, NlpSearchResponse, NlpSearchResultItem, ParsedSearchParams,
};
use crate::services::auth_service::Claims;
use crate::services::embedding_service;
use crate::services::nlp_parser;
use crate::state::AppState;

/// PURPOSE: Search emails using natural language — parses query via AI, then executes IMAP search
/// POST /api/search/nlp
/// CONSTRAINTS: Requires at least one active AI config for NLP parsing
pub async fn nlp_search(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<NlpSearchRequest>,
) -> Result<Json<NlpSearchResponse>, AppError> {
    let user_id = parse_user_id(&claims)?;

    if body.query.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Search query is required for NLP search".to_string(),
        ));
    }

    // Added: Get the user's active AI config and decrypt the API key
    let (config, api_key) =
        embedding_service::get_active_ai_config(&state.db, user_id, &state.config.jwt.secret)
            .await
            .map_err(|err| AppError::BadRequest(err))?;

    // Added: Parse the natural language query into structured search parameters
    let parsed_params = nlp_parser::parse_natural_query(
        &config.provider,
        &api_key,
        &config.model_name,
        config.base_url.as_deref(),
        &body.query,
    )
    .await
    .map_err(|err| AppError::BadRequest(format!("Failed to parse search query: {}", err)))?;

    // Added: Build IMAP search command from parsed params
    let _imap_search = nlp_parser::build_imap_search(&parsed_params);

    // NOTE: In a full implementation, we would execute the IMAP search here via imap_service.
    // For now, return the parsed parameters so the frontend can display what the AI understood.
    // The IMAP search execution will be connected when IMAP service supports programmatic search.
    let results: Vec<NlpSearchResultItem> = Vec::new();
    let result_count = results.len() as i32;

    // Added: Save search to history
    let params_json = serde_json::to_value(&parsed_params)
        .unwrap_or_else(|_| serde_json::json!({}));

    let _ = NlpSearchHistory::create(
        &state.db,
        user_id,
        &body.query,
        &params_json,
        result_count,
    )
    .await;

    Ok(Json(NlpSearchResponse {
        query: body.query,
        parsed_params,
        result_count,
        results,
    }))
}

/// PURPOSE: List the user's NLP search history
/// GET /api/search/nlp/history
pub async fn list_nlp_history(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<NlpSearchHistory>>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let history = NlpSearchHistory::list_by_user(&state.db, user_id).await?;
    Ok(Json(history))
}

/// PURPOSE: Clear all NLP search history for the current user
/// DELETE /api/search/nlp/history
pub async fn clear_nlp_history(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let user_id = parse_user_id(&claims)?;
    let deleted = NlpSearchHistory::delete_all_by_user(&state.db, user_id).await?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "deleted": deleted,
            "message": "Search history cleared"
        })),
    ))
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
    fn test_nlp_search_request_deserialization() {
        let json = serde_json::json!({
            "query": "find emails from John about the budget last week"
        });
        let request: NlpSearchRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.query, "find emails from John about the budget last week");
    }

    #[test]
    fn test_nlp_search_request_empty_query() {
        let json = serde_json::json!({
            "query": ""
        });
        let request: NlpSearchRequest = serde_json::from_value(json).unwrap();
        assert!(request.query.trim().is_empty());
    }

    #[test]
    fn test_nlp_search_response_serialization() {
        let response = NlpSearchResponse {
            query: "emails about project".to_string(),
            parsed_params: ParsedSearchParams {
                subject: Some("project".to_string()),
                keywords: vec!["project".to_string()],
                ..Default::default()
            },
            result_count: 0,
            results: vec![],
        };
        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["query"], "emails about project");
        assert_eq!(json["result_count"], 0);
        assert_eq!(json["parsed_params"]["subject"], "project");
    }
}
