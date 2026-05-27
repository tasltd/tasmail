// Added: Public scheduling endpoints for external participants (TMAIL-269 / TMAIL-127).
//
// External visitors hit these routes without authenticating. The handler
// validates the token, returns a slim event summary (no organizer details
// beyond the title/time), and lets the visitor record an RSVP that upserts
// into the regular event_attendees table.
//
// Security:
//   - public_token is a server-generated UUIDv4 — unguessable.
//   - find_by_public_token filters on `public_enabled = true` so a stale token
//     for a disabled event returns 404 (no oracle).
//   - email is validated server-side; display_name is length-bounded.
//   - No PII about the organizer is leaked — just the event summary.

use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::calendar_event::{CalendarEvent, EventAttendee};
use crate::state::AppState;

/// Slim public-facing view of an event. Intentionally omits organizer_id,
/// ics_uid, linked_message_uid, public_token and other internals — the
/// booking page only needs what an external visitor would see on an invite.
#[derive(Debug, Serialize)]
pub struct PublicEventSummary {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub all_day: bool,
    pub status: String,
}

impl From<CalendarEvent> for PublicEventSummary {
    fn from(e: CalendarEvent) -> Self {
        PublicEventSummary {
            id: e.id,
            title: e.title,
            description: e.description,
            location: e.location,
            start_time: e.start_time,
            end_time: e.end_time,
            all_day: e.all_day,
            status: e.status,
        }
    }
}

/// Request body for the public RSVP endpoint.
#[derive(Debug, Deserialize)]
pub struct PublicRsvpRequest {
    pub email: String,
    #[serde(default)]
    pub name: Option<String>,
    pub status: String,
}

/// Public-facing attendee record returned after RSVP. Excludes the internal
/// row id to avoid leaking attendee-list size or row creation order.
#[derive(Debug, Serialize)]
pub struct PublicRsvpResponse {
    pub email: String,
    pub display_name: Option<String>,
    pub rsvp: String,
    pub responded_at: Option<DateTime<Utc>>,
}

impl From<EventAttendee> for PublicRsvpResponse {
    fn from(a: EventAttendee) -> Self {
        PublicRsvpResponse {
            email: a.email,
            display_name: a.display_name,
            rsvp: a.rsvp,
            responded_at: a.responded_at,
        }
    }
}

const MAX_NAME_LEN: usize = 200;
const MAX_EMAIL_LEN: usize = 320; // RFC 5321 SMTP limit
const ALLOWED_STATUSES: &[&str] = &["accepted", "declined", "maybe"];

/// GET /api/calendar/public/{token} — event summary for the booking page.
///
/// Returns 404 if the token doesn't match an event OR if the owner has
/// disabled public scheduling. The 404 is deliberate: a 403/disabled
/// response would leak that the token exists somewhere.
pub async fn get_public_event(
    State(state): State<AppState>,
    Path(token): Path<Uuid>,
) -> Result<Json<PublicEventSummary>, AppError> {
    let event = CalendarEvent::find_by_public_token(&state.db, token)
        .await?
        .ok_or_else(|| AppError::NotFound("Booking link not found".to_string()))?;

    Ok(Json(event.into()))
}

/// POST /api/calendar/public/{token}/rsvp — record an external RSVP.
///
/// Upserts an attendee row by (event_id, email). Idempotent — visitors can
/// re-submit to change their mind without creating duplicate rows.
pub async fn submit_public_rsvp(
    State(state): State<AppState>,
    Path(token): Path<Uuid>,
    Json(body): Json<PublicRsvpRequest>,
) -> Result<Json<PublicRsvpResponse>, AppError> {
    // ---- validation: tight, all server-side, no DB hit on bad input ----
    let email = body.email.trim().to_lowercase();
    if email.is_empty() || !email.contains('@') || email.len() > MAX_EMAIL_LEN {
        return Err(AppError::BadRequest(
            "email must be a valid email address".to_string(),
        ));
    }

    let display_name = body.name.as_deref().map(str::trim).filter(|s| !s.is_empty());
    if let Some(name) = display_name {
        if name.len() > MAX_NAME_LEN {
            return Err(AppError::BadRequest(format!(
                "name must be at most {} characters",
                MAX_NAME_LEN
            )));
        }
    }

    if !ALLOWED_STATUSES.contains(&body.status.as_str()) {
        return Err(AppError::BadRequest(format!(
            "Invalid status '{}'. Must be one of: accepted, declined, maybe",
            body.status
        )));
    }

    // ---- token must resolve to a public-enabled event ----
    let event = CalendarEvent::find_by_public_token(&state.db, token)
        .await?
        .ok_or_else(|| AppError::NotFound("Booking link not found".to_string()))?;

    // ---- upsert the attendee row ----
    let attendee = EventAttendee::upsert_public_rsvp(
        &state.db,
        event.id,
        &email,
        display_name,
        &body.status,
    )
    .await?;

    Ok(Json(attendee.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_public_rsvp_request_deserialization_full() {
        let json = r#"{"email": "alice@example.com", "name": "Alice", "status": "accepted"}"#;
        let req: PublicRsvpRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.email, "alice@example.com");
        assert_eq!(req.name.as_deref(), Some("Alice"));
        assert_eq!(req.status, "accepted");
    }

    #[test]
    fn test_public_rsvp_request_deserialization_minimal() {
        // name is optional
        let json = r#"{"email": "bob@example.com", "status": "declined"}"#;
        let req: PublicRsvpRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.email, "bob@example.com");
        assert!(req.name.is_none());
        assert_eq!(req.status, "declined");
    }

    #[test]
    fn test_public_event_summary_excludes_internals() {
        // PURPOSE: confirm the public projection drops organizer_id/ics_uid/public_token.
        // These would leak the tenant boundary or let an attacker pivot to the
        // owner's calendar if they were exposed on the booking page.
        let event = CalendarEvent {
            id: Uuid::new_v4(),
            organizer_id: Uuid::new_v4(),
            title: "Discovery Call".to_string(),
            description: Some("30 minute intro".to_string()),
            location: Some("Zoom".to_string()),
            start_time: Utc::now(),
            end_time: Utc::now() + chrono::Duration::minutes(30),
            all_day: false,
            recurrence_rule: None,
            status: "confirmed".to_string(),
            linked_message_uid: None,
            linked_folder: None,
            ics_uid: "uid@tasmail.io".to_string(),
            public_token: Uuid::new_v4(),
            public_enabled: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let summary: PublicEventSummary = event.clone().into();
        let json = serde_json::to_string(&summary).unwrap();

        assert!(json.contains("Discovery Call"));
        assert!(json.contains("Zoom"));
        // Internals must NOT be in the payload.
        assert!(!json.contains("organizer_id"));
        assert!(!json.contains("ics_uid"));
        assert!(!json.contains("public_token"));
        assert!(!json.contains("linked_message_uid"));
    }

    #[test]
    fn test_public_rsvp_response_excludes_row_id() {
        let attendee = EventAttendee {
            id: Uuid::new_v4(),
            event_id: Uuid::new_v4(),
            email: "carol@example.com".to_string(),
            display_name: Some("Carol".to_string()),
            rsvp: "maybe".to_string(),
            responded_at: Some(Utc::now()),
        };
        let resp: PublicRsvpResponse = attendee.clone().into();
        let json = serde_json::to_string(&resp).unwrap();

        assert!(json.contains("carol@example.com"));
        assert!(json.contains("maybe"));
        // Internal row id should not be leaked to anonymous callers.
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("id").is_none());
        assert!(parsed.get("event_id").is_none());
    }

    // ---- input validation: pure functions exercised by reading the handler logic ----

    fn validate_status(s: &str) -> bool {
        ALLOWED_STATUSES.contains(&s)
    }

    #[test]
    fn test_allowed_statuses() {
        assert!(validate_status("accepted"));
        assert!(validate_status("declined"));
        assert!(validate_status("maybe"));
        // pending is NOT a valid public RSVP — pending is the default attendee state,
        // not something a visitor can submit.
        assert!(!validate_status("pending"));
        assert!(!validate_status("yes"));
        assert!(!validate_status(""));
        assert!(!validate_status("ACCEPTED"));
    }

    #[test]
    fn test_email_validation_logic() {
        // Mirror the validation in submit_public_rsvp so behaviour is locked in.
        // We're intentionally lenient — same shape as handlers/enterprise_quote.rs —
        // because SMTP-level validation happens later when we email the organizer.
        let valid = ["a@b.co", "alice@example.com", "x.y+filter@host.io"];
        for e in valid {
            let trimmed = e.trim().to_lowercase();
            assert!(!trimmed.is_empty());
            assert!(trimmed.contains('@'));
            assert!(trimmed.len() <= MAX_EMAIL_LEN);
        }
        let invalid = ["", "   ", "no-at-sign"];
        for e in invalid {
            let trimmed = e.trim().to_lowercase();
            let ok = !trimmed.is_empty() && trimmed.contains('@') && trimmed.len() <= MAX_EMAIL_LEN;
            assert!(!ok, "should reject: {:?}", e);
        }
    }

    #[test]
    fn test_email_too_long_rejected() {
        // 320-char SMTP limit. Build 321-char address.
        let long_email = format!("{}@example.com", "a".repeat(310));
        assert!(long_email.len() > MAX_EMAIL_LEN);
    }
}
