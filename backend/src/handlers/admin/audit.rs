use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::audit_log::AuditLog;
use crate::services::auth_service::Claims;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct AuditLogQuery {
    pub mailbox_id: Option<Uuid>,
    pub action: Option<String>,
    pub limit: Option<i64>,
}

/// GET /api/admin/audit-log
pub async fn list_audit_logs(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Query(query): Query<AuditLogQuery>,
) -> Result<Json<Vec<AuditLog>>, AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    let limit = query.limit.unwrap_or(50).min(500);
    let logs = AuditLog::query(
        &state.db,
        query.mailbox_id,
        query.action.as_deref(),
        limit,
    )
    .await?;

    Ok(Json(logs))
}
