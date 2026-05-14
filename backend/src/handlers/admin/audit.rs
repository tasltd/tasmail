use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::audit_log::AuditLog;
use crate::services::auth_service::Claims;
use crate::services::db_session;
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

    // Fix: TMAIL-198 — audit_log RLS only returns rows when app.is_admin =
    // 'true' on the connection running the SELECT. AuditLog::query was
    // taking the bare pool, so the freshly-acquired connection had no RLS
    // context and the table looked empty. Pin the admin context first.
    let mut conn = db_session::acquire_with_rls(&state, &claims).await?;
    let logs = AuditLog::query_with_conn(
        &mut conn,
        query.mailbox_id,
        query.action.as_deref(),
        limit,
    )
    .await?;

    Ok(Json(logs))
}
