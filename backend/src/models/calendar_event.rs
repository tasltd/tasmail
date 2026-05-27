// Added: Calendar event and attendee models for meeting scheduling (TMAIL-127)
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// PURPOSE: Represents a calendar event/meeting with scheduling metadata
/// CONSTRAINTS: ics_uid must be globally unique per RFC 5545
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CalendarEvent {
    pub id: Uuid,
    pub organizer_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub all_day: bool,
    pub recurrence_rule: Option<String>,
    pub status: String,
    pub linked_message_uid: Option<i32>,
    pub linked_folder: Option<String>,
    pub ics_uid: String,
    // Added (TMAIL-269): unguessable token for the public /book/{token} page.
    // Always present (DB default = gen_random_uuid()); only exposed to external
    // visitors when `public_enabled` is true.
    pub public_token: Uuid,
    // Added (TMAIL-269): owner opt-in flag for external scheduling. New rows
    // default to false so existing events remain private.
    pub public_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// PURPOSE: Represents an attendee's RSVP record for a calendar event
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EventAttendee {
    pub id: Uuid,
    pub event_id: Uuid,
    pub email: String,
    pub display_name: Option<String>,
    pub rsvp: String,
    pub responded_at: Option<DateTime<Utc>>,
}

/// PURPOSE: Request body for creating a new calendar event with attendees
#[derive(Debug, Deserialize)]
pub struct CreateEventRequest {
    pub title: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub all_day: Option<bool>,
    pub recurrence_rule: Option<String>,
    pub attendees: Option<Vec<AttendeeInput>>,
    pub linked_message_uid: Option<i32>,
    pub linked_folder: Option<String>,
}

/// PURPOSE: Attendee email and optional display name for event creation
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AttendeeInput {
    pub email: String,
    pub display_name: Option<String>,
}

/// PURPOSE: Request body for updating an existing calendar event
#[derive(Debug, Deserialize)]
pub struct UpdateEventRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub all_day: Option<bool>,
    pub recurrence_rule: Option<String>,
    pub status: Option<String>,
    // Added (TMAIL-269): toggle external scheduling on/off without rotating the token.
    pub public_enabled: Option<bool>,
}

/// PURPOSE: Request body for RSVP-ing to an event
#[derive(Debug, Deserialize)]
pub struct RsvpRequest {
    pub status: String,
}

/// PURPOSE: Combined event with its attendees for detail responses
#[derive(Debug, Serialize)]
pub struct CalendarEventWithAttendees {
    #[serde(flatten)]
    pub event: CalendarEvent,
    pub attendees: Vec<EventAttendee>,
}

impl CalendarEvent {
    /// PURPOSE: Create a new calendar event with a generated ICS UID
    pub async fn create(
        pool: &PgPool,
        organizer_id: Uuid,
        req: &CreateEventRequest,
        ics_uid: &str,
    ) -> Result<CalendarEvent, sqlx::Error> {
        sqlx::query_as::<_, CalendarEvent>(
            "INSERT INTO calendar_events (organizer_id, title, description, location, start_time, end_time, all_day, recurrence_rule, linked_message_uid, linked_folder, ics_uid)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
             RETURNING *"
        )
        .bind(organizer_id)
        .bind(&req.title)
        .bind(&req.description)
        .bind(&req.location)
        .bind(req.start_time)
        .bind(req.end_time)
        .bind(req.all_day.unwrap_or(false))
        .bind(&req.recurrence_rule)
        .bind(req.linked_message_uid)
        .bind(&req.linked_folder)
        .bind(ics_uid)
        .fetch_one(pool)
        .await
    }

    /// PURPOSE: List events for a user within an optional date range
    pub async fn list_for_user(
        pool: &PgPool,
        organizer_id: Uuid,
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
    ) -> Result<Vec<CalendarEvent>, sqlx::Error> {
        // NOTE: Filter by date range if both start and end are provided
        if let (Some(start), Some(end)) = (start, end) {
            sqlx::query_as::<_, CalendarEvent>(
                "SELECT * FROM calendar_events
                 WHERE organizer_id = $1 AND start_time >= $2 AND end_time <= $3
                 ORDER BY start_time ASC"
            )
            .bind(organizer_id)
            .bind(start)
            .bind(end)
            .fetch_all(pool)
            .await
        } else {
            sqlx::query_as::<_, CalendarEvent>(
                "SELECT * FROM calendar_events
                 WHERE organizer_id = $1
                 ORDER BY start_time ASC
                 LIMIT 100"
            )
            .bind(organizer_id)
            .fetch_all(pool)
            .await
        }
    }

    /// PURPOSE: Find a single event by ID and organizer
    pub async fn find_by_id(
        pool: &PgPool,
        id: Uuid,
        organizer_id: Uuid,
    ) -> Result<Option<CalendarEvent>, sqlx::Error> {
        sqlx::query_as::<_, CalendarEvent>(
            "SELECT * FROM calendar_events WHERE id = $1 AND organizer_id = $2"
        )
        .bind(id)
        .bind(organizer_id)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: Update event fields (only non-None fields are applied)
    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        organizer_id: Uuid,
        req: &UpdateEventRequest,
    ) -> Result<Option<CalendarEvent>, sqlx::Error> {
        sqlx::query_as::<_, CalendarEvent>(
            "UPDATE calendar_events SET
                title = COALESCE($3, title),
                description = COALESCE($4, description),
                location = COALESCE($5, location),
                start_time = COALESCE($6, start_time),
                end_time = COALESCE($7, end_time),
                all_day = COALESCE($8, all_day),
                recurrence_rule = COALESCE($9, recurrence_rule),
                status = COALESCE($10, status),
                public_enabled = COALESCE($11, public_enabled),
                updated_at = now()
             WHERE id = $1 AND organizer_id = $2
             RETURNING *"
        )
        .bind(id)
        .bind(organizer_id)
        .bind(&req.title)
        .bind(&req.description)
        .bind(&req.location)
        .bind(req.start_time)
        .bind(req.end_time)
        .bind(req.all_day)
        .bind(&req.recurrence_rule)
        .bind(&req.status)
        .bind(req.public_enabled)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: Look up an event by its public booking token. Only returns
    /// events whose owner has explicitly enabled external scheduling, so a
    /// token leaked while disabled cannot be used to peek at a private event.
    /// CONSTRAINTS: token must be a valid UUID; caller is unauthenticated.
    pub async fn find_by_public_token(
        pool: &PgPool,
        token: Uuid,
    ) -> Result<Option<CalendarEvent>, sqlx::Error> {
        sqlx::query_as::<_, CalendarEvent>(
            "SELECT * FROM calendar_events
             WHERE public_token = $1 AND public_enabled = true"
        )
        .bind(token)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: Cancel an event by setting status to 'cancelled'
    pub async fn cancel(
        pool: &PgPool,
        id: Uuid,
        organizer_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE calendar_events SET status = 'cancelled', updated_at = now()
             WHERE id = $1 AND organizer_id = $2 AND status != 'cancelled'"
        )
        .bind(id)
        .bind(organizer_id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }
}

impl EventAttendee {
    /// PURPOSE: Add attendees in bulk for a newly created event
    pub async fn create_bulk(
        pool: &PgPool,
        event_id: Uuid,
        attendees: &[AttendeeInput],
    ) -> Result<Vec<EventAttendee>, sqlx::Error> {
        let mut result = Vec::with_capacity(attendees.len());
        for attendee in attendees {
            let row = sqlx::query_as::<_, EventAttendee>(
                "INSERT INTO event_attendees (event_id, email, display_name)
                 VALUES ($1, $2, $3)
                 RETURNING *"
            )
            .bind(event_id)
            .bind(&attendee.email)
            .bind(&attendee.display_name)
            .fetch_one(pool)
            .await?;
            result.push(row);
        }
        Ok(result)
    }

    /// PURPOSE: List all attendees for a given event
    pub async fn list_for_event(
        pool: &PgPool,
        event_id: Uuid,
    ) -> Result<Vec<EventAttendee>, sqlx::Error> {
        sqlx::query_as::<_, EventAttendee>(
            "SELECT * FROM event_attendees WHERE event_id = $1 ORDER BY email ASC"
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
    }

    /// PURPOSE: Update RSVP status for an attendee by email and event
    pub async fn update_rsvp(
        pool: &PgPool,
        event_id: Uuid,
        email: &str,
        rsvp: &str,
    ) -> Result<Option<EventAttendee>, sqlx::Error> {
        sqlx::query_as::<_, EventAttendee>(
            "UPDATE event_attendees SET rsvp = $3, responded_at = now()
             WHERE event_id = $1 AND email = $2
             RETURNING *"
        )
        .bind(event_id)
        .bind(email)
        .bind(rsvp)
        .fetch_optional(pool)
        .await
    }

    /// PURPOSE: Upsert an attendee row for the public booking flow.
    /// External visitors may not yet be on the attendee list, so we insert
    /// them on first RSVP and update their status on subsequent visits.
    /// Idempotent: clicking "Accept" twice records one row in the final state.
    /// CONSTRAINTS: caller must have already validated that `rsvp` is one of
    /// the allowed status values ('accepted' | 'declined' | 'maybe').
    pub async fn upsert_public_rsvp(
        pool: &PgPool,
        event_id: Uuid,
        email: &str,
        display_name: Option<&str>,
        rsvp: &str,
    ) -> Result<EventAttendee, sqlx::Error> {
        // NOTE: event_attendees doesn't have a unique index on (event_id, email),
        // so we can't rely on ON CONFLICT here. Do the lookup-then-update-or-insert
        // dance explicitly inside a transaction to stay race-safe.
        let mut tx = pool.begin().await?;

        let existing: Option<EventAttendee> = sqlx::query_as::<_, EventAttendee>(
            "SELECT * FROM event_attendees
             WHERE event_id = $1 AND email = $2
             FOR UPDATE"
        )
        .bind(event_id)
        .bind(email)
        .fetch_optional(&mut *tx)
        .await?;

        let row = if let Some(existing) = existing {
            sqlx::query_as::<_, EventAttendee>(
                "UPDATE event_attendees
                    SET rsvp = $2,
                        display_name = COALESCE($3, display_name),
                        responded_at = now()
                  WHERE id = $1
                  RETURNING *"
            )
            .bind(existing.id)
            .bind(rsvp)
            .bind(display_name)
            .fetch_one(&mut *tx)
            .await?
        } else {
            sqlx::query_as::<_, EventAttendee>(
                "INSERT INTO event_attendees (event_id, email, display_name, rsvp, responded_at)
                 VALUES ($1, $2, $3, $4, now())
                 RETURNING *"
            )
            .bind(event_id)
            .bind(email)
            .bind(display_name)
            .bind(rsvp)
            .fetch_one(&mut *tx)
            .await?
        };

        tx.commit().await?;
        Ok(row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calendar_event_serialization() {
        let token = Uuid::new_v4();
        let event = CalendarEvent {
            id: Uuid::new_v4(),
            organizer_id: Uuid::new_v4(),
            title: "Team Standup".to_string(),
            description: Some("Daily standup meeting".to_string()),
            location: Some("Conference Room A".to_string()),
            start_time: Utc::now(),
            end_time: Utc::now() + chrono::Duration::hours(1),
            all_day: false,
            recurrence_rule: None,
            status: "tentative".to_string(),
            linked_message_uid: Some(42),
            linked_folder: Some("INBOX".to_string()),
            ics_uid: "uid-12345@tasmail.io".to_string(),
            public_token: token,
            public_enabled: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("Team Standup"));
        assert!(json.contains("tentative"));
        assert!(json.contains("uid-12345@tasmail.io"));
        // TMAIL-269: serialized payload includes the new public-scheduling fields.
        assert!(json.contains("public_token"));
        assert!(json.contains("public_enabled"));

        let deserialized: CalendarEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.title, "Team Standup");
        assert_eq!(deserialized.location.as_deref(), Some("Conference Room A"));
        assert_eq!(deserialized.public_token, token);
        assert!(!deserialized.public_enabled);
    }

    #[test]
    fn test_event_attendee_serialization() {
        let attendee = EventAttendee {
            id: Uuid::new_v4(),
            event_id: Uuid::new_v4(),
            email: "alice@example.com".to_string(),
            display_name: Some("Alice".to_string()),
            rsvp: "pending".to_string(),
            responded_at: None,
        };

        let json = serde_json::to_string(&attendee).unwrap();
        assert!(json.contains("alice@example.com"));
        assert!(json.contains("pending"));

        let deserialized: EventAttendee = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.email, "alice@example.com");
        assert_eq!(deserialized.display_name.as_deref(), Some("Alice"));
    }

    #[test]
    fn test_create_event_request_full() {
        let json = r#"{
            "title": "Sprint Planning",
            "description": "Plan next sprint",
            "location": "Zoom",
            "start_time": "2026-04-20T10:00:00Z",
            "end_time": "2026-04-20T11:00:00Z",
            "all_day": false,
            "attendees": [
                {"email": "bob@example.com", "display_name": "Bob"},
                {"email": "carol@example.com"}
            ],
            "linked_message_uid": 99,
            "linked_folder": "INBOX"
        }"#;
        let req: CreateEventRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.title, "Sprint Planning");
        assert_eq!(req.location.as_deref(), Some("Zoom"));
        assert_eq!(req.attendees.as_ref().unwrap().len(), 2);
        assert_eq!(req.attendees.as_ref().unwrap()[0].email, "bob@example.com");
        assert_eq!(req.linked_message_uid, Some(99));
    }

    #[test]
    fn test_create_event_request_minimal() {
        let json = r#"{
            "title": "Quick Sync",
            "start_time": "2026-04-20T14:00:00Z",
            "end_time": "2026-04-20T14:30:00Z"
        }"#;
        let req: CreateEventRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.title, "Quick Sync");
        assert!(req.description.is_none());
        assert!(req.location.is_none());
        assert!(req.attendees.is_none());
        assert!(req.all_day.is_none());
    }

    #[test]
    fn test_create_event_request_missing_required_fields() {
        let json = r#"{"title": "No times"}"#;
        let result = serde_json::from_str::<CreateEventRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_event_request_partial() {
        let json = r#"{"title": "Renamed Meeting", "status": "confirmed"}"#;
        let req: UpdateEventRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.title.as_deref(), Some("Renamed Meeting"));
        assert_eq!(req.status.as_deref(), Some("confirmed"));
        assert!(req.start_time.is_none());
        assert!(req.location.is_none());
    }

    #[test]
    fn test_update_event_request_empty() {
        let json = r#"{}"#;
        let req: UpdateEventRequest = serde_json::from_str(json).unwrap();
        assert!(req.title.is_none());
        assert!(req.status.is_none());
        assert!(req.public_enabled.is_none());
    }

    #[test]
    fn test_update_event_request_public_enabled() {
        // TMAIL-269: owners can toggle external scheduling via PATCH-like update.
        let json = r#"{"public_enabled": true}"#;
        let req: UpdateEventRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.public_enabled, Some(true));

        let json = r#"{"public_enabled": false}"#;
        let req: UpdateEventRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.public_enabled, Some(false));
    }

    #[test]
    fn test_rsvp_request_deserialization() {
        let json = r#"{"status": "accepted"}"#;
        let req: RsvpRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.status, "accepted");
    }

    #[test]
    fn test_rsvp_request_all_statuses() {
        for status in &["pending", "accepted", "declined", "maybe"] {
            let json = format!(r#"{{"status": "{}"}}"#, status);
            let req: RsvpRequest = serde_json::from_str(&json).unwrap();
            assert_eq!(req.status, *status);
        }
    }

    #[test]
    fn test_attendee_input_serialization() {
        let input = AttendeeInput {
            email: "dave@example.com".to_string(),
            display_name: Some("Dave".to_string()),
        };
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("dave@example.com"));
        assert!(json.contains("Dave"));
    }

    #[test]
    fn test_calendar_event_with_attendees_serialization() {
        let event = CalendarEvent {
            id: Uuid::new_v4(),
            organizer_id: Uuid::new_v4(),
            title: "Flattened Test".to_string(),
            description: None,
            location: None,
            start_time: Utc::now(),
            end_time: Utc::now() + chrono::Duration::hours(1),
            all_day: false,
            recurrence_rule: None,
            status: "confirmed".to_string(),
            linked_message_uid: None,
            linked_folder: None,
            ics_uid: "flat-test@tasmail.io".to_string(),
            public_token: Uuid::new_v4(),
            public_enabled: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let combined = CalendarEventWithAttendees {
            event,
            attendees: vec![],
        };
        let json = serde_json::to_value(&combined).unwrap();
        // NOTE: #[serde(flatten)] puts event fields at top level
        assert!(json.get("title").is_some());
        assert!(json.get("attendees").is_some());
        assert_eq!(json["title"], "Flattened Test");
    }
}
