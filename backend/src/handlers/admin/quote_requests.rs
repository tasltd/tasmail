// TMAIL-183: admin endpoints for the enterprise_quote_requests inbox.
//
//   GET    /api/admin/quote-requests                — paginated list, filter by status
//   GET    /api/admin/quote-requests/{id}           — single record
//   PATCH  /api/admin/quote-requests/{id}           — transition status, append notes,
//                                                     reassign to admin
//
// Audit-logs every state change so sales attribution stays clean.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::audit_log::AuditLog;
use crate::services::auth_service::Claims;
use crate::state::AppState;

const VALID_STATUSES: &[&str] = &["new", "contacted", "quoted", "won", "lost"];

#[derive(Debug, Serialize, FromRow)]
pub struct QuoteRequestSummary {
    pub id: Uuid,
    pub contact_name: String,
    pub contact_email: String,
    pub company: Option<String>,
    pub estimated_users: Option<i32>,
    pub message: String,
    pub status: String,
    pub internal_notes: Option<String>,
    pub assigned_to: Option<Uuid>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub contacted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub quoted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub closed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 { 50 }

#[derive(Debug, Serialize)]
pub struct ListResponse {
    pub items: Vec<QuoteRequestSummary>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

/// GET /api/admin/quote-requests — paginated, optional status filter
pub async fn list_quote_requests(
    State(state): State<AppState>,
    axum::Extension(_claims): axum::Extension<Claims>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse>, AppError> {
    // Clamp the pagination so a hostile client can't ask for 1M rows.
    let limit = q.limit.clamp(1, 200);
    let offset = q.offset.max(0);

    let status_filter = q.status.as_deref();
    if let Some(s) = status_filter {
        if !VALID_STATUSES.contains(&s) {
            return Err(AppError::BadRequest(format!(
                "status must be one of: {}",
                VALID_STATUSES.join(", ")
            )));
        }
    }

    // Count + page in two cheap queries — much simpler than wrestling with a window function.
    let total: i64 = match status_filter {
        Some(s) => {
            sqlx::query_scalar("SELECT COUNT(*) FROM enterprise_quote_requests WHERE status = $1")
                .bind(s)
                .fetch_one(&state.db)
                .await?
        }
        None => sqlx::query_scalar("SELECT COUNT(*) FROM enterprise_quote_requests")
            .fetch_one(&state.db)
            .await?,
    };

    let items: Vec<QuoteRequestSummary> = match status_filter {
        Some(s) => {
            sqlx::query_as::<_, QuoteRequestSummary>(
                "SELECT id, contact_name, contact_email, company, estimated_users, message,
                        status, internal_notes, assigned_to, created_at, updated_at,
                        contacted_at, quoted_at, closed_at
                 FROM enterprise_quote_requests
                 WHERE status = $1
                 ORDER BY created_at DESC
                 LIMIT $2 OFFSET $3",
            )
            .bind(s)
            .bind(limit)
            .bind(offset)
            .fetch_all(&state.db)
            .await?
        }
        None => {
            sqlx::query_as::<_, QuoteRequestSummary>(
                "SELECT id, contact_name, contact_email, company, estimated_users, message,
                        status, internal_notes, assigned_to, created_at, updated_at,
                        contacted_at, quoted_at, closed_at
                 FROM enterprise_quote_requests
                 ORDER BY created_at DESC
                 LIMIT $1 OFFSET $2",
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(&state.db)
            .await?
        }
    };

    Ok(Json(ListResponse { items, total, limit, offset }))
}

pub async fn get_quote_request(
    State(state): State<AppState>,
    axum::Extension(_claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<Json<QuoteRequestSummary>, AppError> {
    let row = sqlx::query_as::<_, QuoteRequestSummary>(
        "SELECT id, contact_name, contact_email, company, estimated_users, message,
                status, internal_notes, assigned_to, created_at, updated_at,
                contacted_at, quoted_at, closed_at
         FROM enterprise_quote_requests
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("quote request {}", id)))?;
    Ok(Json(row))
}

#[derive(Debug, Deserialize)]
pub struct UpdateQuoteRequest {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub internal_notes: Option<String>,
    #[serde(default)]
    pub assigned_to: Option<Uuid>,
}

/// PATCH /api/admin/quote-requests/{id}
///
/// Partial update — caller can transition status, append notes, or reassign in any
/// combination. Status changes auto-stamp the matching contacted_at/quoted_at/
/// closed_at timestamp so the dashboard can render an audit trail without a
/// separate state-history table.
pub async fn update_quote_request(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateQuoteRequest>,
) -> Result<Json<QuoteRequestSummary>, AppError> {
    if body.status.is_none() && body.internal_notes.is_none() && body.assigned_to.is_none() {
        return Err(AppError::BadRequest("Specify at least one of: status, internal_notes, assigned_to".into()));
    }

    if let Some(s) = body.status.as_deref() {
        if !VALID_STATUSES.contains(&s) {
            return Err(AppError::BadRequest(format!(
                "status must be one of: {}",
                VALID_STATUSES.join(", ")
            )));
        }
    }

    // Capture the prior state for the audit log.
    let prior: QuoteRequestSummary = sqlx::query_as::<_, QuoteRequestSummary>(
        "SELECT id, contact_name, contact_email, company, estimated_users, message,
                status, internal_notes, assigned_to, created_at, updated_at,
                contacted_at, quoted_at, closed_at
         FROM enterprise_quote_requests WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("quote request {}", id)))?;

    // Build the COALESCE-friendly UPDATE: skip columns the caller didn't supply.
    // Status transitions also set the matching timestamp column to NOW().
    let updated: QuoteRequestSummary = sqlx::query_as::<_, QuoteRequestSummary>(
        "UPDATE enterprise_quote_requests
         SET
            status = COALESCE($2, status),
            internal_notes = COALESCE($3, internal_notes),
            assigned_to = CASE WHEN $4::uuid IS NULL THEN assigned_to ELSE $4::uuid END,
            contacted_at = CASE WHEN $2 = 'contacted' AND contacted_at IS NULL THEN NOW() ELSE contacted_at END,
            quoted_at    = CASE WHEN $2 = 'quoted'    AND quoted_at    IS NULL THEN NOW() ELSE quoted_at    END,
            closed_at    = CASE WHEN $2 IN ('won','lost') AND closed_at IS NULL THEN NOW() ELSE closed_at END
         WHERE id = $1
         RETURNING id, contact_name, contact_email, company, estimated_users, message,
                   status, internal_notes, assigned_to, created_at, updated_at,
                   contacted_at, quoted_at, closed_at",
    )
    .bind(id)
    .bind(body.status.as_deref())
    .bind(body.internal_notes.as_deref())
    .bind(body.assigned_to)
    .fetch_one(&state.db)
    .await?;

    // Audit-log only the status transition — internal notes are fine to update silently.
    if prior.status != updated.status {
        let actor = Uuid::parse_str(&claims.sub).ok();
        let metadata = serde_json::json!({
            "from": prior.status,
            "to":   updated.status,
            "quote_id": id.to_string(),
        });
        let id_str = id.to_string();
        let _ = AuditLog::record(
            &state.db,
            actor,
            "quote_request.status_change",
            Some("quote_request"),
            Some(id_str.as_str()),
            Some(metadata),
            None,
            None,
        )
        .await;
    }

    Ok(Json(updated))
}

#[derive(Debug, Serialize, FromRow)]
pub struct StatusCount {
    pub status: String,
    pub count: i64,
}

/// GET /api/admin/quote-requests/stats — counts per status, for the dashboard header
pub async fn quote_request_stats(
    State(state): State<AppState>,
    axum::Extension(_claims): axum::Extension<Claims>,
) -> Result<Json<Vec<StatusCount>>, AppError> {
    let rows = sqlx::query_as::<_, StatusCount>(
        "SELECT status, COUNT(*)::bigint AS count
         FROM enterprise_quote_requests
         GROUP BY status
         ORDER BY status",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

// Status returned to the route layer when a delete-by-id endpoint is added later.
#[allow(dead_code)]
pub fn _unused_status_code() -> StatusCode { StatusCode::NO_CONTENT }
