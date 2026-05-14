// Added: IP warm-up schedule handlers for TMAIL-17
// PURPOSE: Admin endpoints for generating, viewing, and tracking IP warm-up schedules
// CONSTRAINTS: Admin-only endpoints — no RLS needed

use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::Serialize;

use crate::error::AppError;
use crate::models::warmup::{StartWarmupRequest, WarmupSchedule, WarmupStatus};
use crate::services::auth_service::Claims;
use crate::state::AppState;

/// Added: Response wrapper for the full warm-up schedule (TMAIL-17)
#[derive(Debug, Serialize)]
pub struct WarmupScheduleResponse {
    pub schedule: WarmupSchedule,
    pub description: String,
}

/// GET /api/admin/warmup/status — Get warm-up progress for the tracked IP
/// PURPOSE: Shows current day, daily limit, emails sent, and remaining capacity
pub async fn get_warmup_status(
    State(state): State<AppState>,
    axum::Extension(_claims): axum::Extension<Claims>,
) -> Result<Json<Vec<WarmupStatus>>, AppError> {
    // Added: Query all tracked IPs from the database
    let rows = sqlx::query_as::<_, WarmupTrackingRow>(
        "SELECT ip_address, current_day, emails_sent_today, total_emails_sent, \
         started_at, paused FROM ip_warmup_tracking ORDER BY started_at DESC",
    )
    .fetch_all(&state.db)
    .await?;

    let statuses: Vec<WarmupStatus> = rows
        .iter()
        .map(|row| {
            WarmupStatus::from_tracking(
                &row.ip_address,
                row.current_day as u32,
                row.emails_sent_today as u32,
                row.total_emails_sent as u64,
                Some(row.started_at.to_rfc3339()),
                row.paused,
            )
        })
        .collect();

    Ok(Json(statuses))
}

/// GET /api/admin/warmup/schedule — Get the full 8-week warm-up schedule
/// PURPOSE: Returns the standard warm-up progression for planning purposes
pub async fn get_warmup_schedule(
    axum::Extension(_claims): axum::Extension<Claims>,
) -> Result<Json<WarmupScheduleResponse>, AppError> {
    let schedule = WarmupSchedule::generate();

    Ok(Json(WarmupScheduleResponse {
        schedule,
        description: "8-week IP warm-up schedule. Week 8 (daily_limit=0) means unlimited sending."
            .to_string(),
    }))
}

/// POST /api/admin/warmup/start — Start warm-up tracking for a new sending IP
/// PURPOSE: Initializes warm-up state at day 1 for the given IP address
pub async fn start_warmup(
    State(state): State<AppState>,
    axum::Extension(_claims): axum::Extension<Claims>,
    Json(body): Json<StartWarmupRequest>,
) -> Result<(StatusCode, Json<WarmupStatus>), AppError> {
    // Added: Validate IP address is not empty
    let ip = body.ip_address.trim();
    if ip.is_empty() {
        return Err(AppError::BadRequest(
            "ip_address cannot be empty".to_string(),
        ));
    }

    // Added: Basic IP format validation (v4 or v6)
    if ip.parse::<std::net::IpAddr>().is_err() {
        return Err(AppError::BadRequest(format!(
            "Invalid IP address format: '{}'",
            ip
        )));
    }

    // Fix: TMAIL-203 — `RETURNING 1` types as INT4 in Postgres. Original
    // i64 binding 500'd every call with a sqlx type-mismatch. Match the
    // actual return type.
    let result = sqlx::query_scalar::<_, i32>(
        "INSERT INTO ip_warmup_tracking (ip_address, current_day, emails_sent_today, total_emails_sent) \
         VALUES ($1, 1, 0, 0) \
         ON CONFLICT (ip_address) DO NOTHING \
         RETURNING 1",
    )
    .bind(ip)
    .fetch_optional(&state.db)
    .await?;

    if result.is_none() {
        return Err(AppError::Conflict(format!(
            "Warm-up tracking already exists for IP '{}'",
            ip
        )));
    }

    let status = WarmupStatus::from_tracking(ip, 1, 0, 0, Some(chrono::Utc::now().to_rfc3339()), false);

    Ok((StatusCode::CREATED, Json(status)))
}

/// Added: Internal DB row mapping for ip_warmup_tracking table (TMAIL-17)
#[derive(Debug, sqlx::FromRow)]
struct WarmupTrackingRow {
    ip_address: String,
    current_day: i32,
    emails_sent_today: i32,
    total_emails_sent: i64,
    started_at: chrono::DateTime<chrono::Utc>,
    paused: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::warmup::WarmupSchedule;

    #[test]
    fn test_warmup_schedule_response_serialization() {
        let resp = WarmupScheduleResponse {
            schedule: WarmupSchedule::generate(),
            description: "8-week schedule".to_string(),
        };

        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["description"], "8-week schedule");
        assert!(json["schedule"]["weeks"].is_array());
        assert_eq!(json["schedule"]["total_days"], 56);
    }

    #[test]
    fn test_warmup_schedule_has_correct_weeks() {
        let schedule = WarmupSchedule::generate();
        assert_eq!(schedule.weeks.len(), 8);
        assert_eq!(schedule.weeks[0].daily_limit, 50);
        assert_eq!(schedule.weeks[7].daily_limit, 0); // unlimited
    }

    #[test]
    fn test_ip_validation_v4() {
        assert!("203.0.113.10".parse::<std::net::IpAddr>().is_ok());
        assert!("10.0.0.1".parse::<std::net::IpAddr>().is_ok());
        assert!("not-an-ip".parse::<std::net::IpAddr>().is_err());
        assert!("".parse::<std::net::IpAddr>().is_err());
    }

    #[test]
    fn test_ip_validation_v6() {
        assert!("::1".parse::<std::net::IpAddr>().is_ok());
        assert!("2001:db8::1".parse::<std::net::IpAddr>().is_ok());
        assert!("fe80::1".parse::<std::net::IpAddr>().is_ok());
    }

    #[test]
    fn test_warmup_status_day_1() {
        let status = WarmupStatus::from_tracking("10.0.0.1", 1, 0, 0, None, false);
        assert_eq!(status.current_week, 1);
        assert_eq!(status.daily_limit, 50);
        assert_eq!(status.remaining_today, 50);
        assert!(!status.completed);
    }

    #[test]
    fn test_warmup_status_mid_schedule() {
        let status = WarmupStatus::from_tracking(
            "10.0.0.1",
            22, // day 22 = week 4
            200,
            3000,
            Some("2026-03-01T00:00:00Z".to_string()),
            false,
        );
        assert_eq!(status.current_week, 4);
        assert_eq!(status.daily_limit, 500);
        assert_eq!(status.remaining_today, 300);
    }

    #[test]
    fn test_warmup_status_completed_schedule() {
        let status = WarmupStatus::from_tracking("10.0.0.1", 57, 0, 50000, None, false);
        assert!(status.completed);
        assert_eq!(status.remaining_today, u32::MAX);
    }

    #[test]
    fn test_start_warmup_request_deserialization() {
        let json = serde_json::json!({"ip_address": "203.0.113.10"});
        let req: StartWarmupRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.ip_address, "203.0.113.10");
    }

    #[test]
    fn test_start_warmup_request_rejects_missing_ip() {
        let json = serde_json::json!({});
        let result = serde_json::from_value::<StartWarmupRequest>(json);
        assert!(result.is_err());
    }
}
