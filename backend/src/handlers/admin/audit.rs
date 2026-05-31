use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, HeaderValue},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::audit_log::{AuditLog, AuditLogFilter};
use crate::services::auth_service::Claims;
use crate::services::db_session;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct AuditLogQuery {
    pub mailbox_id: Option<Uuid>,
    pub action: Option<String>,
    pub limit: Option<i64>,
    // TMAIL-352: Modern UI's paginated viewer adds offset + inclusive
    // date-range bounds. All fields stay optional so the classic SPA's
    // unparameterised call site keeps working.
    pub offset: Option<i64>,
    pub from: Option<chrono::DateTime<chrono::Utc>>,
    pub to: Option<chrono::DateTime<chrono::Utc>>,
}

/// GET /api/admin/audit-log
pub async fn list_audit_logs(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Query(query): Query<AuditLogQuery>,
) -> Result<impl IntoResponse, AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    // Clamp pagination — page size hard-capped at 500 to protect the table;
    // negative offsets coerced to 0 so a buggy client never crashes the query.
    let limit = query.limit.unwrap_or(50).clamp(1, 500);
    let offset = query.offset.unwrap_or(0).max(0);

    let filter = AuditLogFilter {
        mailbox_id: query.mailbox_id,
        action: query.action,
        from: query.from,
        to: query.to,
        limit,
        offset,
    };

    // Fix: TMAIL-198 — audit_log RLS only returns rows when app.is_admin =
    // 'true' on the connection running the SELECT. AuditLog::query was
    // taking the bare pool, so the freshly-acquired connection had no RLS
    // context and the table looked empty. Pin the admin context first.
    let mut conn = db_session::acquire_with_rls(&state, &claims).await?;
    let logs = AuditLog::query_filtered(&mut conn, &filter).await?;
    // TMAIL-352: total-count drives the Modern UI's prev/next so the user
    // can see "Showing 1-50 of 412" without paging through to find the end.
    let total = AuditLog::count_filtered(&mut conn, &filter).await?;

    let mut headers = HeaderMap::new();
    headers.insert("X-Total-Count", HeaderValue::from(total));
    // CORS preflight strips non-simple response headers; expose so the
    // browser will surface X-Total-Count to fetch() in the Modern UI.
    headers.insert(
        header::ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_static("X-Total-Count"),
    );

    Ok((headers, Json(logs)))
}
