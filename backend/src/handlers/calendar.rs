// Added: Calendar event handlers for meeting scheduling (TMAIL-127)
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::models::calendar_event::{
    CalendarEvent, CalendarEventWithAttendees, CreateEventRequest, EventAttendee, RsvpRequest,
    UpdateEventRequest,
};
use crate::services::auth_service::Claims;
use crate::services::ics_generator::{generate_ics, generate_ics_uid, IcsAttendee, IcsEventData};
use crate::state::AppState;

/// Added: Query params for date-range filtering on event list
#[derive(Debug, Deserialize)]
pub struct EventListQuery {
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
}

/// Added: Response for ICS file download with correct content type
#[derive(Debug, Serialize)]
pub struct IcsResponse {
    pub ics: String,
    pub filename: String,
}

/// PURPOSE: Parse user UUID from JWT claims
/// CONSTRAINTS: Claims.sub must be a valid UUID string
fn parse_user_id(claims: &Claims) -> Result<uuid::Uuid, AppError> {
    claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid user ID in claims")))
}

/// GET /api/calendar/events — list events with optional ?start=&end= date range
pub async fn list_events(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Query(query): Query<EventListQuery>,
) -> Result<Json<Vec<CalendarEvent>>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let events = CalendarEvent::list_for_user(&state.db, user_id, query.start, query.end).await?;
    Ok(Json(events))
}

/// POST /api/calendar/events — create event with attendees, generate ICS UID
pub async fn create_event(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreateEventRequest>,
) -> Result<(StatusCode, Json<CalendarEventWithAttendees>), AppError> {
    let user_id = parse_user_id(&claims)?;

    // NOTE: Validate title is not empty
    if body.title.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Event title cannot be empty".to_string(),
        ));
    }

    // NOTE: Validate end_time is after start_time
    if body.end_time <= body.start_time {
        return Err(AppError::BadRequest(
            "Event end_time must be after start_time".to_string(),
        ));
    }

    // Added: Generate unique ICS UID per RFC 5545
    let ics_uid = generate_ics_uid("tasmail.io");

    let event = CalendarEvent::create(&state.db, user_id, &body, &ics_uid).await?;

    // Added: Create attendees if provided
    let attendees = if let Some(ref attendee_list) = body.attendees {
        EventAttendee::create_bulk(&state.db, event.id, attendee_list).await?
    } else {
        vec![]
    };

    Ok((
        StatusCode::CREATED,
        Json(CalendarEventWithAttendees { event, attendees }),
    ))
}

/// GET /api/calendar/events/:id — get event detail with attendees
pub async fn get_event(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<CalendarEventWithAttendees>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let event = CalendarEvent::find_by_id(&state.db, id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Calendar event not found".to_string()))?;

    let attendees = EventAttendee::list_for_event(&state.db, event.id).await?;

    Ok(Json(CalendarEventWithAttendees { event, attendees }))
}

/// PUT /api/calendar/events/:id — update event fields
pub async fn update_event(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<UpdateEventRequest>,
) -> Result<Json<CalendarEvent>, AppError> {
    let user_id = parse_user_id(&claims)?;

    // NOTE: Validate title if provided
    if let Some(ref title) = body.title {
        if title.trim().is_empty() {
            return Err(AppError::BadRequest(
                "Event title cannot be empty".to_string(),
            ));
        }
    }

    // NOTE: Validate status if provided
    if let Some(ref status) = body.status {
        if !["tentative", "confirmed", "cancelled"].contains(&status.as_str()) {
            return Err(AppError::BadRequest(format!(
                "Invalid status '{}'. Must be one of: tentative, confirmed, cancelled",
                status
            )));
        }
    }

    let event = CalendarEvent::update(&state.db, id, user_id, &body)
        .await?
        .ok_or_else(|| AppError::NotFound("Calendar event not found".to_string()))?;

    Ok(Json(event))
}

/// DELETE /api/calendar/events/:id — cancel an event (sets status to cancelled)
pub async fn cancel_event(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, AppError> {
    let user_id = parse_user_id(&claims)?;
    let cancelled = CalendarEvent::cancel(&state.db, id, user_id).await?;
    if cancelled {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound(
            "Calendar event not found or already cancelled".to_string(),
        ))
    }
}

/// POST /api/calendar/events/:id/rsvp — RSVP to an event
pub async fn rsvp_event(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<RsvpRequest>,
) -> Result<Json<EventAttendee>, AppError> {
    // NOTE: Validate RSVP status value
    if !["accepted", "declined", "maybe"].contains(&body.status.as_str()) {
        return Err(AppError::BadRequest(format!(
            "Invalid RSVP status '{}'. Must be one of: accepted, declined, maybe",
            body.status
        )));
    }

    // NOTE: Use the authenticated user's email (from claims.username) to find their attendee record
    let attendee = EventAttendee::update_rsvp(&state.db, id, &claims.username, &body.status)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(
                "You are not an attendee of this event, or the event was not found".to_string(),
            )
        })?;

    Ok(Json(attendee))
}

/// GET /api/calendar/events/:id/ics — download ICS file for an event
pub async fn download_ics(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<(StatusCode, [(axum::http::header::HeaderName, String); 2], String), AppError> {
    let user_id = parse_user_id(&claims)?;
    let event = CalendarEvent::find_by_id(&state.db, id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Calendar event not found".to_string()))?;

    let attendees = EventAttendee::list_for_event(&state.db, event.id).await?;

    // Added: Build ICS event data from database records
    let ics_data = IcsEventData {
        uid: event.ics_uid.clone(),
        summary: event.title.clone(),
        description: event.description.clone(),
        location: event.location.clone(),
        start_time: event.start_time,
        end_time: event.end_time,
        all_day: event.all_day,
        organizer_email: claims.username.clone(),
        attendees: attendees
            .iter()
            .map(|a| IcsAttendee {
                email: a.email.clone(),
                display_name: a.display_name.clone(),
                rsvp_status: a.rsvp.clone(),
            })
            .collect(),
        status: event.status.clone(),
    };

    let ics_content = generate_ics(&ics_data);
    let filename = format!("{}.ics", event.title.replace(' ', "_").to_lowercase());

    Ok((
        StatusCode::OK,
        [
            (
                axum::http::header::CONTENT_TYPE,
                "text/calendar; charset=utf-8".to_string(),
            ),
            (
                axum::http::header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", filename),
            ),
        ],
        ics_content,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::calendar_event::{CreateEventRequest, UpdateEventRequest, RsvpRequest, AttendeeInput};
    use crate::services::auth_service::Claims;

    #[test]
    fn test_parse_user_id_valid() {
        let claims = Claims {
            sub: uuid::Uuid::new_v4().to_string(),
            username: "test@example.com".into(),
            is_admin: false,
            exp: 0,
            iat: 0,
        };
        assert!(parse_user_id(&claims).is_ok());
    }

    #[test]
    fn test_parse_user_id_invalid() {
        let claims = Claims {
            sub: "not-a-uuid".into(),
            username: "test@example.com".into(),
            is_admin: false,
            exp: 0,
            iat: 0,
        };
        assert!(parse_user_id(&claims).is_err());
    }

    #[test]
    fn test_event_list_query_with_dates() {
        let json = r#"{"start": "2026-04-01T00:00:00Z", "end": "2026-04-30T23:59:59Z"}"#;
        let query: EventListQuery = serde_json::from_str(json).unwrap();
        assert!(query.start.is_some());
        assert!(query.end.is_some());
    }

    #[test]
    fn test_event_list_query_empty() {
        let json = r#"{}"#;
        let query: EventListQuery = serde_json::from_str(json).unwrap();
        assert!(query.start.is_none());
        assert!(query.end.is_none());
    }

    #[test]
    fn test_create_event_request_validation_scenarios() {
        // Full request with attendees
        let json = r#"{
            "title": "Review",
            "start_time": "2026-04-20T10:00:00Z",
            "end_time": "2026-04-20T11:00:00Z",
            "attendees": [{"email": "a@b.com"}]
        }"#;
        let req: CreateEventRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.title, "Review");
        assert_eq!(req.attendees.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_rsvp_request_all_valid_values() {
        for status in &["accepted", "declined", "maybe"] {
            let json = format!(r#"{{"status": "{}"}}"#, status);
            let req: RsvpRequest = serde_json::from_str(&json).unwrap();
            assert_eq!(req.status, *status);
        }
    }

    #[test]
    fn test_ics_response_serialization() {
        let resp = IcsResponse {
            ics: "BEGIN:VCALENDAR...".to_string(),
            filename: "meeting.ics".to_string(),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["filename"], "meeting.ics");
    }

    #[test]
    fn test_update_event_request_status_values() {
        for status in &["tentative", "confirmed", "cancelled"] {
            let json = format!(r#"{{"status": "{}"}}"#, status);
            let req: UpdateEventRequest = serde_json::from_str(&json).unwrap();
            assert_eq!(req.status.as_deref(), Some(*status));
        }
    }
}
