// Added: IP warm-up schedule models for TMAIL-17
// PURPOSE: Structs for the 8-week IP warm-up schedule generator and tracking
// CONSTRAINTS: Admin-only feature — no RLS needed, access controlled at handler level

use serde::{Deserialize, Serialize};

/// Added: A single day within the warm-up schedule (TMAIL-17)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WarmupDay {
    pub day: u32,
    pub week: u32,
    pub daily_limit: u32,
    pub description: String,
}

/// Added: Full 8-week warm-up schedule (TMAIL-17)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarmupSchedule {
    pub weeks: Vec<WarmupWeek>,
    pub total_days: u32,
}

/// Added: A single week within the warm-up schedule (TMAIL-17)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarmupWeek {
    pub week: u32,
    pub daily_limit: u32,
    pub description: String,
}

/// Added: Current warm-up status for a sending IP (TMAIL-17)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarmupStatus {
    pub ip_address: String,
    pub current_day: u32,
    pub current_week: u32,
    pub daily_limit: u32,
    pub emails_sent_today: u32,
    pub total_emails_sent: u64,
    pub remaining_today: u32,
    pub started_at: Option<String>,
    pub paused: bool,
    pub completed: bool,
}

/// Added: Request body for starting warm-up tracking (TMAIL-17)
#[derive(Debug, Deserialize)]
pub struct StartWarmupRequest {
    pub ip_address: String,
}

// Added: Weekly send limits for the 8-week warm-up progression (TMAIL-17)
// NOTE: These are conservative limits following industry best practices
pub const WARMUP_WEEKLY_LIMITS: [(u32, u32, &str); 8] = [
    (1, 50, "Initial warm-up — low volume, establish reputation"),
    (2, 100, "Gradual increase — monitor bounce rates"),
    (3, 250, "Moderate volume — enroll in Google Postmaster Tools and check spam placement"),
    (4, 500, "Steady growth — review engagement metrics in Google Postmaster Tools"),
    (5, 1000, "Scaling up — maintain consistent sending patterns"),
    (6, 2500, "High volume ramp — monitor deliverability scores"),
    (7, 5000, "Near-full capacity — verify inbox placement rates"),
    (8, 0, "Warm-up complete — unlimited sending"),
];

impl WarmupSchedule {
    /// PURPOSE: Generate the full 8-week warm-up schedule
    pub fn generate() -> WarmupSchedule {
        let weeks: Vec<WarmupWeek> = WARMUP_WEEKLY_LIMITS
            .iter()
            .map(|(week, limit, desc)| WarmupWeek {
                week: *week,
                daily_limit: *limit,
                description: desc.to_string(),
            })
            .collect();

        WarmupSchedule {
            weeks,
            total_days: 56, // 8 weeks * 7 days
        }
    }

    /// PURPOSE: Get the daily limit for a given day number (1-56)
    pub fn limit_for_day(day: u32) -> u32 {
        if day == 0 || day > 56 {
            return 0;
        }
        // Added: Calculate which week this day falls in (1-indexed)
        let week = ((day - 1) / 7) + 1;
        WarmupSchedule::limit_for_week(week)
    }

    /// PURPOSE: Get the daily limit for a given week number (1-8)
    pub fn limit_for_week(week: u32) -> u32 {
        match WARMUP_WEEKLY_LIMITS.iter().find(|(w, _, _)| *w == week) {
            Some((_, limit, _)) => *limit,
            None => 0,
        }
    }

    /// PURPOSE: Check if warm-up is complete (past day 56)
    pub fn is_complete(day: u32) -> bool {
        day > 56
    }
}

impl WarmupStatus {
    /// PURPOSE: Build warm-up status from tracking data
    pub fn from_tracking(
        ip_address: &str,
        current_day: u32,
        emails_sent_today: u32,
        total_emails_sent: u64,
        started_at: Option<String>,
        paused: bool,
    ) -> WarmupStatus {
        let current_week = if current_day == 0 {
            0
        } else {
            ((current_day - 1) / 7) + 1
        };
        let daily_limit = WarmupSchedule::limit_for_day(current_day);
        let completed = WarmupSchedule::is_complete(current_day);

        // Added: Calculate remaining sends — 0 means unlimited for week 8+
        let remaining_today = if completed || daily_limit == 0 {
            u32::MAX
        } else {
            daily_limit.saturating_sub(emails_sent_today)
        };

        WarmupStatus {
            ip_address: ip_address.to_string(),
            current_day,
            current_week,
            daily_limit,
            emails_sent_today,
            total_emails_sent,
            remaining_today,
            started_at,
            paused,
            completed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_warmup_schedule_generation() {
        let schedule = WarmupSchedule::generate();
        assert_eq!(schedule.weeks.len(), 8);
        assert_eq!(schedule.total_days, 56);
    }

    #[test]
    fn test_warmup_weekly_limits() {
        let schedule = WarmupSchedule::generate();
        assert_eq!(schedule.weeks[0].week, 1);
        assert_eq!(schedule.weeks[0].daily_limit, 50);
        assert_eq!(schedule.weeks[1].daily_limit, 100);
        assert_eq!(schedule.weeks[2].daily_limit, 250);
        assert_eq!(schedule.weeks[3].daily_limit, 500);
        assert_eq!(schedule.weeks[4].daily_limit, 1000);
        assert_eq!(schedule.weeks[5].daily_limit, 2500);
        assert_eq!(schedule.weeks[6].daily_limit, 5000);
        // Added: Week 8 has 0 limit meaning unlimited
        assert_eq!(schedule.weeks[7].daily_limit, 0);
    }

    #[test]
    fn test_limit_for_day_week_boundaries() {
        // Added: Day 1-7 = week 1 = 50/day
        assert_eq!(WarmupSchedule::limit_for_day(1), 50);
        assert_eq!(WarmupSchedule::limit_for_day(7), 50);
        // Added: Day 8-14 = week 2 = 100/day
        assert_eq!(WarmupSchedule::limit_for_day(8), 100);
        assert_eq!(WarmupSchedule::limit_for_day(14), 100);
        // Added: Day 15-21 = week 3 = 250/day
        assert_eq!(WarmupSchedule::limit_for_day(15), 250);
        // Added: Day 43-49 = week 7 = 5000/day
        assert_eq!(WarmupSchedule::limit_for_day(49), 5000);
        // Added: Day 50-56 = week 8 = unlimited (0)
        assert_eq!(WarmupSchedule::limit_for_day(50), 0);
        assert_eq!(WarmupSchedule::limit_for_day(56), 0);
    }

    #[test]
    fn test_limit_for_day_out_of_range() {
        assert_eq!(WarmupSchedule::limit_for_day(0), 0);
        assert_eq!(WarmupSchedule::limit_for_day(57), 0);
        assert_eq!(WarmupSchedule::limit_for_day(100), 0);
    }

    #[test]
    fn test_limit_for_week() {
        assert_eq!(WarmupSchedule::limit_for_week(1), 50);
        assert_eq!(WarmupSchedule::limit_for_week(4), 500);
        assert_eq!(WarmupSchedule::limit_for_week(8), 0);
        assert_eq!(WarmupSchedule::limit_for_week(9), 0);
        assert_eq!(WarmupSchedule::limit_for_week(0), 0);
    }

    #[test]
    fn test_is_complete() {
        assert!(!WarmupSchedule::is_complete(1));
        assert!(!WarmupSchedule::is_complete(56));
        assert!(WarmupSchedule::is_complete(57));
        assert!(WarmupSchedule::is_complete(100));
    }

    #[test]
    fn test_warmup_status_from_tracking() {
        let status = WarmupStatus::from_tracking(
            "203.0.113.10",
            10, // day 10 = week 2
            45,
            345,
            Some("2026-04-01T00:00:00Z".to_string()),
            false,
        );

        assert_eq!(status.ip_address, "203.0.113.10");
        assert_eq!(status.current_day, 10);
        assert_eq!(status.current_week, 2);
        assert_eq!(status.daily_limit, 100);
        assert_eq!(status.emails_sent_today, 45);
        assert_eq!(status.remaining_today, 55); // 100 - 45
        assert!(!status.completed);
        assert!(!status.paused);
    }

    #[test]
    fn test_warmup_status_completed() {
        let status = WarmupStatus::from_tracking(
            "203.0.113.10",
            57, // past 56 days
            0,
            15000,
            Some("2026-02-01T00:00:00Z".to_string()),
            false,
        );

        assert!(status.completed);
        assert_eq!(status.remaining_today, u32::MAX);
    }

    #[test]
    fn test_warmup_status_paused() {
        let status = WarmupStatus::from_tracking("203.0.113.10", 5, 0, 100, None, true);
        assert!(status.paused);
        assert_eq!(status.daily_limit, 50);
    }

    #[test]
    fn test_warmup_status_week8_unlimited() {
        // Added: Week 8 has 0 daily_limit meaning unlimited
        let status = WarmupStatus::from_tracking(
            "203.0.113.10",
            50, // day 50 = week 8
            500,
            10000,
            Some("2026-02-01T00:00:00Z".to_string()),
            false,
        );

        assert_eq!(status.daily_limit, 0);
        assert_eq!(status.remaining_today, u32::MAX);
    }

    #[test]
    fn test_warmup_day_serialization() {
        let day = WarmupDay {
            day: 1,
            week: 1,
            daily_limit: 50,
            description: "Initial warm-up".to_string(),
        };

        let json = serde_json::to_value(&day).unwrap();
        assert_eq!(json["day"], 1);
        assert_eq!(json["week"], 1);
        assert_eq!(json["daily_limit"], 50);
    }

    #[test]
    fn test_warmup_schedule_serialization() {
        let schedule = WarmupSchedule::generate();
        let json = serde_json::to_value(&schedule).unwrap();
        assert_eq!(json["total_days"], 56);
        assert!(json["weeks"].is_array());
        assert_eq!(json["weeks"].as_array().unwrap().len(), 8);
    }

    #[test]
    fn test_warmup_status_serialization() {
        let status = WarmupStatus::from_tracking("10.0.0.1", 3, 25, 75, None, false);
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["ip_address"], "10.0.0.1");
        assert_eq!(json["current_day"], 3);
        assert_eq!(json["emails_sent_today"], 25);
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

    // Added: TMAIL-17 — Postmaster Tools enrollment must be captured in week 3-4 guidance
    #[test]
    fn test_postmaster_tools_in_week_3_and_4_descriptions() {
        let schedule = WarmupSchedule::generate();
        let w3 = schedule.weeks.iter().find(|w| w.week == 3).unwrap();
        let w4 = schedule.weeks.iter().find(|w| w.week == 4).unwrap();
        assert!(
            w3.description.contains("Google Postmaster Tools"),
            "Week 3 description should mention Google Postmaster Tools enrollment, got: {}",
            w3.description
        );
        assert!(
            w4.description.contains("Google Postmaster Tools"),
            "Week 4 description should mention Google Postmaster Tools monitoring, got: {}",
            w4.description
        );
    }

    #[test]
    fn test_warmup_remaining_when_over_limit() {
        // Added: saturating_sub prevents underflow when already over limit
        let status = WarmupStatus::from_tracking("10.0.0.1", 1, 60, 60, None, false);
        assert_eq!(status.remaining_today, 0); // 50 - 60 saturates to 0
    }
}
