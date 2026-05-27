// Added: eDiscovery search handlers for compliance and legal investigations (TMAIL-137)
//
// Authorization model:
//   * Every handler is gated by `require_compliance` which admits is_admin OR
//     is_compliance_officer. This satisfies the "dedicated RBAC role"
//     requirement without forcing investigators to be full admins.
//
// Audit trail:
//   * Every entry point records to audit_log with action prefix `ediscovery.`
//     so the existing /api/admin/audit endpoint surfaces who searched what when.
//
// Legal-hold scoping:
//   * On create, if `legal_hold_only=true` the requested target_users are
//     intersected with the set of users currently under an active legal hold
//     (or, when target_users is omitted, ALL active legal-hold users become
//     the scope). This prevents accidental searches against mailboxes
//     outside an existing hold.
//
// Export format:
//   * Format (mbox|eml|pdf) is captured at create time and enforced at export
//     time via the DB CHECK constraint added in migration 069. The export
//     path filename uses the format's natural extension (.mbox / .zip / .pdf).

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::error::AppError;
use crate::models::audit_log::AuditLog;
use crate::models::ediscovery::{
    CreateEdiscoveryRequest, EdiscoveryResult, EdiscoverySearch, EdiscoverySearchWithResults,
    EdiscoveryStatus, ExportFormat,
};
use crate::models::retention_policy::LegalHold;
use crate::services::auth_service::{require_compliance, Claims};
use crate::state::AppState;

/// PURPOSE: List all eDiscovery searches
/// GET /api/admin/ediscovery
pub async fn list_searches(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<EdiscoverySearch>>, AppError> {
    require_compliance(&claims)?;
    let searches = EdiscoverySearch::find_all(&state.db).await?;
    record_audit(
        &state,
        &claims,
        "ediscovery.search.list",
        None,
        Some(serde_json::json!({ "count": searches.len() })),
    )
    .await;
    Ok(Json(searches))
}

/// PURPOSE: Create a new eDiscovery search
/// POST /api/admin/ediscovery
/// CONSTRAINTS: name and search_query are required; legal_hold_only=true
///              forces target_users to intersect with the active legal-hold
///              set.
pub async fn create_search(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreateEdiscoveryRequest>,
) -> Result<(StatusCode, Json<EdiscoverySearch>), AppError> {
    require_compliance(&claims)?;

    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Search name cannot be empty".to_string(),
        ));
    }
    if body.search_query.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Search query cannot be empty".to_string(),
        ));
    }

    // Validate the requested export format before any DB write so we fail
    // fast with a useful message instead of a CHECK constraint violation.
    let export_format = match &body.export_format {
        Some(s) => s.parse::<ExportFormat>().map_err(AppError::BadRequest)?,
        None => ExportFormat::Mbox,
    };

    // Resolve the effective target_users set, honoring legal_hold_only.
    let effective_targets =
        resolve_target_users(&state, body.legal_hold_only.unwrap_or(false), &body.target_users)
            .await?;

    let admin_id = parse_admin_id(&claims)?;
    let search = EdiscoverySearch::create(
        &state.db,
        admin_id,
        &body,
        effective_targets,
        export_format.as_str(),
    )
    .await?;

    record_audit(
        &state,
        &claims,
        "ediscovery.search.create",
        Some(search.id.to_string().as_str()),
        Some(serde_json::json!({
            "name": search.name,
            "search_query": search.search_query,
            "legal_hold_only": search.legal_hold_only,
            "export_format": search.export_format,
            "target_user_count": search.target_users.as_ref().map(|t| t.len()).unwrap_or(0),
        })),
    )
    .await;

    Ok((StatusCode::CREATED, Json(search)))
}

/// PURPOSE: Get a single eDiscovery search with its results
/// GET /api/admin/ediscovery/:id
pub async fn get_search(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<EdiscoverySearchWithResults>, AppError> {
    require_compliance(&claims)?;

    let search = EdiscoverySearch::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("eDiscovery search not found".to_string()))?;

    let results = EdiscoveryResult::find_by_search(&state.db, id).await?;

    record_audit(
        &state,
        &claims,
        "ediscovery.search.view",
        Some(id.to_string().as_str()),
        Some(serde_json::json!({ "result_count": results.len() })),
    )
    .await;

    Ok(Json(EdiscoverySearchWithResults { search, results }))
}

/// PURPOSE: Delete an eDiscovery search and all its results
/// DELETE /api/admin/ediscovery/:id
pub async fn delete_search(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, AppError> {
    require_compliance(&claims)?;

    if !EdiscoverySearch::delete(&state.db, id).await? {
        return Err(AppError::NotFound("eDiscovery search not found".to_string()));
    }

    record_audit(
        &state,
        &claims,
        "ediscovery.search.delete",
        Some(id.to_string().as_str()),
        None,
    )
    .await;

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
    require_compliance(&claims)?;

    let search = EdiscoverySearch::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("eDiscovery search not found".to_string()))?;

    // Only pending searches can be executed
    if search.status != EdiscoveryStatus::Pending {
        return Err(AppError::BadRequest(format!(
            "Search cannot be executed: current status is '{:?}'. Only 'Pending' searches can be executed.",
            search.status
        )));
    }

    // Mark search as running
    let updated = EdiscoverySearch::update_status(
        &state.db,
        id,
        &EdiscoveryStatus::Running,
        None,
    )
    .await?
    .ok_or_else(|| AppError::NotFound("eDiscovery search not found".to_string()))?;

    record_audit(
        &state,
        &claims,
        "ediscovery.search.execute",
        Some(id.to_string().as_str()),
        Some(serde_json::json!({
            "search_query": updated.search_query,
            "target_user_count": updated.target_users.as_ref().map(|t| t.len()).unwrap_or(0),
            "legal_hold_only": updated.legal_hold_only,
        })),
    )
    .await;

    // NOTE: In a full implementation, this would spawn a background task to:
    // 1. Connect to each target user's IMAP mailbox (or query the Piler archive)
    // 2. Run IMAP SEARCH with the query
    // 3. Fetch matching message headers/snippets
    // 4. Insert results into ediscovery_results
    // 5. Update status to 'completed' or 'failed'
    // For now, we mark it as running and return immediately.

    Ok(Json(updated))
}

/// PURPOSE: Query string for the export endpoint — lets caller override the
/// format chosen at create time without rebuilding the search row.
#[derive(Debug, Deserialize)]
pub struct ExportQuery {
    pub format: Option<String>,
}

/// PURPOSE: Export eDiscovery search results in the configured format.
/// POST /api/admin/ediscovery/:id/export?format=mbox|eml|pdf
/// NOTE: In production this would generate the actual file from IMAP / Piler;
///       this handler stamps the export_path with the right extension.
pub async fn export_results(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
    Query(q): Query<ExportQuery>,
) -> Result<Json<EdiscoverySearch>, AppError> {
    require_compliance(&claims)?;

    let search = EdiscoverySearch::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("eDiscovery search not found".to_string()))?;

    if search.status != EdiscoveryStatus::Completed {
        return Err(AppError::BadRequest(format!(
            "Search cannot be exported: current status is '{:?}'. Only 'Completed' searches can be exported.",
            search.status
        )));
    }

    // Format resolution: query string wins, otherwise fall back to the value
    // captured when the search was created.
    let format = match q.format.as_deref() {
        Some(s) => s.parse::<ExportFormat>().map_err(AppError::BadRequest)?,
        None => search
            .export_format
            .parse::<ExportFormat>()
            .map_err(AppError::BadRequest)?,
    };

    let export_path = format!(
        "/exports/ediscovery/{}.{}",
        id,
        format.file_extension()
    );
    let updated = EdiscoverySearch::set_export_path(&state.db, id, &export_path)
        .await?
        .ok_or_else(|| AppError::NotFound("eDiscovery search not found".to_string()))?;

    record_audit(
        &state,
        &claims,
        "ediscovery.search.export",
        Some(id.to_string().as_str()),
        Some(serde_json::json!({
            "format": format.as_str(),
            "export_path": export_path,
        })),
    )
    .await;

    Ok(Json(updated))
}

/// PURPOSE: Intersect the requested target user list with the set of users
/// currently under an active legal hold, when legal_hold_only is requested.
///
/// Semantics:
///   * legal_hold_only=false → return targets unchanged
///   * legal_hold_only=true  + targets=None → all held users
///   * legal_hold_only=true  + targets=Some(list) → list ∩ held_users
///   * legal_hold_only=true  but the intersection is empty → BadRequest
///     (refuse to run a search with no scope, otherwise the search silently
///     matches nothing and investigators can't tell why)
async fn resolve_target_users(
    state: &AppState,
    legal_hold_only: bool,
    requested: &Option<Vec<uuid::Uuid>>,
) -> Result<Option<Vec<uuid::Uuid>>, AppError> {
    if !legal_hold_only {
        return Ok(requested.clone());
    }

    let held = LegalHold::find_active_user_ids(&state.db).await?;
    if held.is_empty() {
        return Err(AppError::BadRequest(
            "legal_hold_only=true but no users are currently under an active legal hold"
                .to_string(),
        ));
    }

    let effective: Vec<uuid::Uuid> = match requested {
        Some(list) if !list.is_empty() => {
            let held_set: std::collections::HashSet<_> = held.iter().copied().collect();
            list.iter()
                .copied()
                .filter(|u| held_set.contains(u))
                .collect()
        }
        _ => held,
    };

    if effective.is_empty() {
        return Err(AppError::BadRequest(
            "legal_hold_only=true: none of the requested target_users are under an active legal hold"
                .to_string(),
        ));
    }
    Ok(Some(effective))
}

/// PURPOSE: Best-effort audit-log write. Failures are logged but never block
/// the eDiscovery operation — the operation already succeeded by this point
/// and we don't want a stray audit-table issue to mask that from the caller.
async fn record_audit(
    state: &AppState,
    claims: &Claims,
    action: &str,
    resource_id: Option<&str>,
    details: Option<serde_json::Value>,
) {
    let mailbox_id = claims.sub.parse::<uuid::Uuid>().ok();
    if let Err(e) = AuditLog::record(
        &state.db,
        mailbox_id,
        action,
        Some("ediscovery_search"),
        resource_id,
        details,
        None,
        None,
    )
    .await
    {
        tracing::warn!(
            error = %e,
            action = action,
            "ediscovery: audit_log.record failed (non-fatal)"
        );
    }
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

    fn admin_claims() -> Claims {
        Claims {
            sub: uuid::Uuid::new_v4().to_string(),
            username: "admin@example.com".into(),
            is_admin: true,
            is_compliance_officer: false,
            exp: 0,
            iat: 0,
        }
    }

    fn compliance_claims() -> Claims {
        Claims {
            sub: uuid::Uuid::new_v4().to_string(),
            username: "compliance@example.com".into(),
            is_admin: false,
            is_compliance_officer: true,
            exp: 0,
            iat: 0,
        }
    }

    fn ordinary_claims() -> Claims {
        Claims {
            sub: uuid::Uuid::new_v4().to_string(),
            username: "user@example.com".into(),
            is_admin: false,
            is_compliance_officer: false,
            exp: 0,
            iat: 0,
        }
    }

    #[test]
    fn test_parse_admin_id_valid() {
        assert!(parse_admin_id(&admin_claims()).is_ok());
    }

    #[test]
    fn test_parse_admin_id_invalid() {
        let claims = Claims {
            sub: "not-a-uuid".into(),
            username: "admin@example.com".into(),
            is_admin: true,
            is_compliance_officer: false,
            exp: 0,
            iat: 0,
        };
        assert!(parse_admin_id(&claims).is_err());
    }

    #[test]
    fn test_require_compliance_admits_admin() {
        assert!(require_compliance(&admin_claims()).is_ok());
    }

    #[test]
    fn test_require_compliance_admits_compliance_officer() {
        assert!(require_compliance(&compliance_claims()).is_ok());
    }

    #[test]
    fn test_require_compliance_rejects_ordinary_user() {
        let err = require_compliance(&ordinary_claims()).unwrap_err();
        match err {
            AppError::Forbidden(msg) => assert!(msg.contains("Compliance officer")),
            other => panic!("expected Forbidden, got {:?}", other),
        }
    }

    #[test]
    fn test_export_query_format_parses_mbox_eml_pdf() {
        for fmt in &["mbox", "eml", "pdf"] {
            let parsed: Result<ExportFormat, _> = fmt.parse();
            assert!(parsed.is_ok(), "format {} must parse", fmt);
        }
    }

    #[test]
    fn test_export_query_format_rejects_csv() {
        let parsed: Result<ExportFormat, _> = "csv".parse();
        assert!(parsed.is_err());
    }
}
