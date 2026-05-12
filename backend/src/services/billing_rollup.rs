// TMAIL-177: usage-based billing rollup loop.
//
// Once a day (or whatever poll_interval_secs the operator configures) the
// rollup wakes up and:
//
//   1. Snapshots every mailbox's current quota_usage row into usage_samples.
//   2. Upserts the in-progress billing_period row for the calendar month,
//      recomputing avg_storage_bytes / peak_storage_bytes from the samples
//      that fall inside the period.
//   3. Closes any prior periods that are still 'open' and writes the
//      matching billing_invoice with amount = invoice_amount(...).
//
// Pricing inputs are read from env at startup so an operator can adjust the
// rate without a code change:
//
//   TASMAIL_GHS_PER_GB        default 1.00
//   TASMAIL_GHS_MONTHLY_MIN   default 5.00
//
// invoice_amount() lives in billing_math::compute_invoice_ghs so it can be
// unit-tested without spinning up Postgres (TMAIL-180).

use std::sync::Arc;

use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use sqlx::PgPool;
use tokio_util::sync::CancellationToken;

use crate::services::billing_math;

const METRIC_BILLING_OPEN_PERIODS: &str = "tasmail_billing_open_periods";
const METRIC_BILLING_INVOICES_CREATED: &str = "tasmail_billing_invoices_created_total";
const METRIC_BILLING_ROLLUP_LATENCY: &str = "tasmail_billing_rollup_latency_seconds";

pub struct BillingRollup {
    pool: Arc<PgPool>,
    poll_interval_secs: u64,
    ghs_per_gb: f64,
    ghs_monthly_min: f64,
    cancel: CancellationToken,
}

impl BillingRollup {
    pub fn new(pool: Arc<PgPool>, poll_interval_secs: u64) -> Self {
        let ghs_per_gb = std::env::var("TASMAIL_GHS_PER_GB")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1.00);
        let ghs_monthly_min = std::env::var("TASMAIL_GHS_MONTHLY_MIN")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5.00);
        Self { pool, poll_interval_secs, ghs_per_gb, ghs_monthly_min, cancel: CancellationToken::new() }
    }

    pub fn cancel_token(&self) -> CancellationToken { self.cancel.clone() }

    pub fn start(self) {
        metrics::describe_gauge!(METRIC_BILLING_OPEN_PERIODS, "Number of open billing_periods rows");
        metrics::describe_counter!(METRIC_BILLING_INVOICES_CREATED, "Billing invoices closed by the rollup");
        metrics::describe_histogram!(METRIC_BILLING_ROLLUP_LATENCY, "Wall-clock per rollup tick");

        let cancel = self.cancel.clone();
        tokio::spawn(async move {
            tracing::info!(
                "Billing rollup started (interval={}s, rate=GHS {:.2}/GB-month, min=GHS {:.2})",
                self.poll_interval_secs, self.ghs_per_gb, self.ghs_monthly_min
            );
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(self.poll_interval_secs));
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        tracing::info!("Billing rollup received shutdown — exiting");
                        break;
                    }
                    _ = interval.tick() => {
                        let start = std::time::Instant::now();
                        if let Err(e) = self.tick().await {
                            tracing::error!("Billing rollup tick error: {}", e);
                        }
                        metrics::histogram!(METRIC_BILLING_ROLLUP_LATENCY).record(start.elapsed().as_secs_f64());
                    }
                }
            }
        });
    }

    async fn tick(&self) -> anyhow::Result<()> {
        // 1) snapshot every mailbox's current usage into usage_samples
        let samples_inserted: i64 = sqlx::query_scalar(
            "WITH latest AS (
                SELECT DISTINCT ON (mailbox_id) mailbox_id, used_bytes, message_count
                FROM quota_usage
                ORDER BY mailbox_id, last_synced_at DESC
             ),
             ins AS (
                INSERT INTO usage_samples (mailbox_id, sampled_at, used_bytes, message_count)
                SELECT mailbox_id, NOW(), used_bytes, message_count FROM latest
                ON CONFLICT (mailbox_id, sampled_at) DO NOTHING
                RETURNING 1
             )
             SELECT COUNT(*)::bigint FROM ins",
        )
        .fetch_one(&*self.pool)
        .await?;
        if samples_inserted > 0 {
            tracing::info!("Billing rollup: inserted {} usage samples", samples_inserted);
        }

        // 2) upsert open periods for the current calendar month and recompute avg/peak
        let today = Utc::now().date_naive();
        let period_start = NaiveDate::from_ymd_opt(today.year(), today.month(), 1)
            .expect("month start always exists");
        let period_end = next_month_first(period_start).pred_opt().expect("month end exists");

        sqlx::query(
            "WITH latest AS (
                SELECT DISTINCT ON (mailbox_id) mailbox_id, used_bytes
                FROM quota_usage
             )
             INSERT INTO billing_periods
                (mailbox_id, period_start, period_end, avg_storage_bytes, peak_storage_bytes, sample_count, status)
             SELECT mailbox_id, $1, $2, used_bytes, used_bytes, 1, 'open'
             FROM latest
             ON CONFLICT (mailbox_id, period_start) DO UPDATE
             SET avg_storage_bytes = (
                    SELECT COALESCE(AVG(used_bytes)::bigint, 0)
                    FROM usage_samples us
                    WHERE us.mailbox_id = billing_periods.mailbox_id
                      AND us.sampled_at >= billing_periods.period_start
                      AND us.sampled_at <  billing_periods.period_end + interval '1 day'
                 ),
                 peak_storage_bytes = (
                    SELECT COALESCE(MAX(used_bytes), 0)
                    FROM usage_samples us
                    WHERE us.mailbox_id = billing_periods.mailbox_id
                      AND us.sampled_at >= billing_periods.period_start
                      AND us.sampled_at <  billing_periods.period_end + interval '1 day'
                 ),
                 sample_count = (
                    SELECT COUNT(*) FROM usage_samples us
                    WHERE us.mailbox_id = billing_periods.mailbox_id
                      AND us.sampled_at >= billing_periods.period_start
                      AND us.sampled_at <  billing_periods.period_end + interval '1 day'
                 )",
        )
        .bind(period_start)
        .bind(period_end)
        .execute(&*self.pool)
        .await?;

        // 3) close any prior open periods + write invoices
        let to_close = sqlx::query_as::<_, (uuid::Uuid, uuid::Uuid, NaiveDate, NaiveDate, i64)>(
            "SELECT id, mailbox_id, period_start, period_end, avg_storage_bytes
             FROM billing_periods
             WHERE status = 'open' AND period_end < $1",
        )
        .bind(period_start)
        .fetch_all(&*self.pool)
        .await?;

        for (period_id, mailbox_id, p_start, p_end, avg_bytes) in to_close {
            let amount = billing_math::compute_invoice_ghs(avg_bytes, self.ghs_per_gb, self.ghs_monthly_min);
            let mut tx = self.pool.begin().await?;
            sqlx::query("UPDATE billing_periods SET status='closed', closed_at=NOW() WHERE id=$1")
                .bind(period_id)
                .execute(&mut *tx).await?;
            sqlx::query(
                "INSERT INTO billing_invoices
                    (mailbox_id, period_id, period_start, period_end,
                     avg_storage_bytes, ghs_per_gb, ghs_monthly_min, amount_ghs, minimum_applied)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
                 ON CONFLICT (mailbox_id, period_id) DO NOTHING",
            )
            .bind(mailbox_id)
            .bind(period_id)
            .bind(p_start)
            .bind(p_end)
            .bind(avg_bytes)
            .bind(self.ghs_per_gb)
            .bind(self.ghs_monthly_min)
            .bind(amount.amount_ghs)
            .bind(amount.minimum_applied)
            .execute(&mut *tx).await?;
            tx.commit().await?;
            metrics::counter!(METRIC_BILLING_INVOICES_CREATED).increment(1);
            tracing::info!(
                "Closed billing period {} for mailbox {} ({}–{}): GHS {:.2}",
                period_id, mailbox_id, p_start, p_end, amount.amount_ghs
            );
        }

        // 4) refresh open-period gauge
        if let Ok(open_count) = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM billing_periods WHERE status = 'open'",
        )
        .fetch_one(&*self.pool)
        .await
        {
            metrics::gauge!(METRIC_BILLING_OPEN_PERIODS).set(open_count as f64);
        }

        Ok(())
    }
}

fn next_month_first(d: NaiveDate) -> NaiveDate {
    if d.month() == 12 {
        NaiveDate::from_ymd_opt(d.year() + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(d.year(), d.month() + 1, 1).unwrap()
    }
}

// `_` to silence unused-import lint when only the chrono date math runs.
#[allow(dead_code)]
fn _unused_tz() -> chrono::DateTime<chrono::Utc> { Utc.timestamp_opt(0, 0).single().unwrap() }
