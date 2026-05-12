// TMAIL-178/179: usage-based billing endpoints for the in-app dashboard.
//
//   GET /api/billing/usage     — current month's storage + projected charge
//   GET /api/billing/invoices  — historical invoices
//
// Both pull from the new billing_periods / billing_invoices tables populated
// by the BillingRollup loop (TMAIL-177). They scope to the authenticated
// caller's mailbox via an explicit WHERE — the per-request RLS connection
// helper from TMAIL-161 is overkill for these read-only endpoints.

use axum::{extract::State, Json};
use chrono::{Datelike, NaiveDate};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

use crate::error::AppError;
use crate::services::auth_service::Claims;
use crate::services::billing_math::compute_invoice_ghs;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct UsageResponse {
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub avg_storage_bytes: i64,
    pub peak_storage_bytes: i64,
    pub current_storage_bytes: i64,
    pub sample_count: i64,
    pub projected_amount_ghs: f64,
    pub projected_minimum_applied: bool,
    pub projected_billed_gb: u64,
    pub ghs_per_gb: f64,
    pub ghs_monthly_min: f64,
}

fn rate() -> (f64, f64) {
    let per_gb: f64 = std::env::var("TASMAIL_GHS_PER_GB").ok().and_then(|s| s.parse().ok()).unwrap_or(1.00);
    let min: f64   = std::env::var("TASMAIL_GHS_MONTHLY_MIN").ok().and_then(|s| s.parse().ok()).unwrap_or(5.00);
    (per_gb, min)
}

pub async fn get_usage(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<UsageResponse>, AppError> {
    let mailbox_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox id")))?;

    let today = chrono::Utc::now().date_naive();
    let period_start = NaiveDate::from_ymd_opt(today.year(), today.month(), 1)
        .expect("first-of-month is always valid");
    let period_end = next_month_first(period_start).pred_opt().expect("last day of month exists");

    // Pull whatever the rollup has written for this period; fall back to
    // current quota_usage if the rollup hasn't fired for this user yet.
    let period_row: Option<(i64, i64, i64)> = sqlx::query_as(
        "SELECT avg_storage_bytes, peak_storage_bytes, sample_count
         FROM billing_periods
         WHERE mailbox_id = $1 AND period_start = $2",
    )
    .bind(mailbox_id)
    .bind(period_start)
    .fetch_optional(&state.db)
    .await?;

    let current_bytes: i64 = sqlx::query_scalar(
        "SELECT COALESCE(used_bytes, 0) FROM quota_usage WHERE mailbox_id = $1",
    )
    .bind(mailbox_id)
    .fetch_optional(&state.db)
    .await?
    .unwrap_or(0);

    let (avg, peak, samples) = period_row.unwrap_or((current_bytes, current_bytes, 0));
    let (per_gb, min) = rate();
    let projection = compute_invoice_ghs(avg, per_gb, min);

    Ok(Json(UsageResponse {
        period_start,
        period_end,
        avg_storage_bytes: avg,
        peak_storage_bytes: peak,
        current_storage_bytes: current_bytes,
        sample_count: samples,
        projected_amount_ghs: projection.amount_ghs,
        projected_minimum_applied: projection.minimum_applied,
        projected_billed_gb: projection.billed_gb,
        ghs_per_gb: per_gb,
        ghs_monthly_min: min,
    }))
}

#[derive(Debug, Serialize, FromRow)]
pub struct InvoiceRow {
    pub id: Uuid,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub avg_storage_bytes: i64,
    pub amount_ghs: f64,
    pub minimum_applied: bool,
    pub status: String,
    pub provider: Option<String>,
    pub provider_reference: Option<String>,
    pub paid_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn list_invoices(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<InvoiceRow>>, AppError> {
    let mailbox_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox id")))?;

    let rows = sqlx::query_as::<_, InvoiceRow>(
        "SELECT id, period_start, period_end, avg_storage_bytes, amount_ghs::float8 AS amount_ghs,
                minimum_applied, status, provider, provider_reference, paid_at, created_at
         FROM billing_invoices
         WHERE mailbox_id = $1
         ORDER BY period_end DESC
         LIMIT 24",
    )
    .bind(mailbox_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows))
}

fn next_month_first(d: NaiveDate) -> NaiveDate {
    if d.month() == 12 {
        NaiveDate::from_ymd_opt(d.year() + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(d.year(), d.month() + 1, 1).unwrap()
    }
}
