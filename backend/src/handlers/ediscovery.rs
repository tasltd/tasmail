// Added: eDiscovery search handlers for compliance and legal investigations (TMAIL-137)

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::error::AppError;
use crate::models::ediscovery::{
    CreateEdiscoveryRequest, EdiscoveryResult, EdiscoverySearch, EdiscoverySearchWithResults,
    EdiscoveryStatus,
};
use crate::services::auth_service::Claims;
use crate::state::AppState;

/// PURPOSE: List all eDiscovery searches
/// GET /api/admin/ediscovery
pub async fn list_searches(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<EdiscoverySearch>>, AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }
    let searches = EdiscoverySearch::find_all(&state.db).await?;
    Ok(Json(searches))
}

/// PURPOSE: Create a new eDiscovery search
/// POST /api/admin/ediscovery
/// CONSTRAINTS: name and search_query are required, must be admin
pub async fn create_search(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreateEdiscoveryRequest>,
) -> Result<(StatusCode, Json<EdiscoverySearch>), AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    // Added: Validate name is not empty
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Search name cannot be empty".to_string(),
        ));
    }

    // Added: Validate search_query is not empty
    if body.search_query.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Search query cannot be empty".to_string(),
        ));
    }

    let admin_id = parse_admin_id(&claims)?;
    let search = EdiscoverySearch::create(&state.db, admin_id, &body).await?;
    Ok((StatusCode::CREATED, Json(search)))
}

/// PURPOSE: Get a single eDiscovery search with its results
/// GET /api/admin/ediscovery/:id
pub async fn get_search(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<EdiscoverySearchWithResults>, AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    let search = EdiscoverySearch::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("eDiscovery search not found".to_string()))?;

    let results = EdiscoveryResult::find_by_search(&state.db, id).await?;

    Ok(Json(EdiscoverySearchWithResults { search, results }))
}

/// PURPOSE: Delete an eDiscovery search and all its results
/// DELETE /api/admin/ediscovery/:id
pub async fn delete_search(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    if !EdiscoverySearch::delete(&state.db, id).await? {
        return Err(AppError::NotFound("eDiscovery search not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

/// PURPOSE: Execute an eDiscovery search across target users' mailboxes
/// POST /api/admin/ediscovery/:id/execute
/// NOTE: In production this would use IMAP SEARCH across user mailboxes;
///       this handler updates the status to simulate the flow
pub async fn execute_search(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<EdiscoverySearch>, AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    let search = EdiscoverySearch::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("eDiscovery search not found".to_string()))?;

    // Added: Only pending searches can be executed
    if search.status != EdiscoveryStatus::Pending {
        return Err(AppError::BadRequest(format!(
            "Search cannot be executed: current status is '{:?}'. Only 'Pending' searches can be executed.",
            search.status
        )));
    }

    // Added: Mark search as running
    let updated = EdiscoverySearch::update_status(
        &state.db,
        id,
        &EdiscoveryStatus::Running,
        None,
    )
    .await?
    .ok_or_else(|| AppError::NotFound("eDiscovery search not found".to_string()))?;

    // NOTE: In a full implementation, this would spawn a background task to:
    // 1. Connect to each target user's IMAP mailbox
    // 2. Run IMAP SEARCH with the query
    // 3. Fetch matching message headers/snippets
    // 4. Insert results into ediscovery_results
    // 5. Update status to 'completed' or 'failed'
    // For now, we mark it as running and return immediately.

    Ok(Json(updated))
}

/// PURPOSE: Export eDiscovery search results to MBOX format
/// POST /api/admin/ediscovery/:id/export
/// NOTE: In production this would generate an MBOX file from IMAP;
///       this handler updates the status to 'exported'
pub async fn export_results(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<EdiscoverySearch>, AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    let search = EdiscoverySearch::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("eDiscovery search not found".to_string()))?;

    // Added: Only completed searches can be exported
    if search.status != EdiscoveryStatus::Completed {
        return Err(AppError::BadRequest(format!(
            "Search cannot be exported: current status is '{:?}'. Only 'Completed' searches can be exported.",
            search.status
        )));
    }

    // Added: Generate a placeholder export path
    let export_path = format!("/exports/ediscovery/{}.mbox", id);
    let updated = EdiscoverySearch::set_export_path(&state.db, id, &export_path)
        .await?
        .ok_or_else(|| AppError::NotFound("eDiscovery search not found".to_string()))?;

    Ok(Json(updated))
}

/// PURPOSE: Parse admin UUID from JWT claims
fn parse_admin_id(claims: &Claims) -> Result<uuid::Uuid, AppError> {
    claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid admin user ID in JWT claims")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::auth_service::Claims;

    #[test]
    fn test_parse_admin_id_valid() {
        let claims = Claims {
            sub: uuid::Uuid::new_v4().to_string(),
            username: "admin@example.com".into(),
            is_admin: true,
            exp: 0,
            iat: 0,
        };
        assert!(parse_admin_id(&claims).is_ok());
    }

    #[test]
    fn test_parse_admin_id_invalid() {
        let claims = Claims {
            sub: "not-a-uuid".into(),
            username: "admin@example.com".into(),
            is_admin: true,
            exp: 0,
            iat: 0,
        };
        assert!(parse_admin_id(&claims).is_err());
    }
}
