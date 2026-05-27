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
    AttendeeInput, CalendarEvent, CalendarEventWithAttendees, CreateEventRequest, EventAttendee,
    RsvpRequest, UpdateEventRequest,
};
use crate::services::auth_service::Claims;
use crate::services::ics_generator::{
    generate_ics, generate_ics_uid, generate_imip_reply, IcsAttendee, IcsEventData,
};
use crate::services::imap_service::ImapService;
use crate::services::imip_parser::parse_imip_from_email;
use crate::services::smtp_service::SmtpService;
use crate::state::AppState;
use futures::TryStreamExt;

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

/// Added (TMAIL-127): Request body for the iMIP accept endpoint. The caller
/// identifies the message that carried the invitation by `(folder, uid)`
/// rather than by a parsed-VEVENT UID, so the backend can re-read the
/// original raw RFC822 payload and trust the iCalendar bytes from IMAP.
#[derive(Debug, Deserialize)]
pub struct ImipAcceptRequest {
    pub folder: String,
    pub uid: u32,
    /// Optional override; defaults to "accepted". Accepting the same three
    /// values as the regular RSVP endpoint keeps the API symmetric so a
    /// future "Decline + auto-add to calendar" button can reuse this path.
    pub status: Option<String>,
}

/// Added (TMAIL-127): Combined response after accepting an iMIP invitation.
/// `reply_sent` lets the SPA show a "Reply sent to organizer" toast — false
/// when the message had no recoverable organizer email or the SMTP transport
/// rejected the reply (the event is still persisted in either case).
#[derive(Debug, Serialize)]
pub struct ImipAcceptResponse {
    #[serde(flatten)]
    pub event: CalendarEventWithAttendees,
    pub reply_sent: bool,
    pub reply_error: Option<String>,
}

/// POST /api/calendar/imip/accept — parse the text/calendar; method=REQUEST
/// part of an inbound message, upsert into `calendar_events` (matched on the
/// iCalendar UID so subsequent REPLY/CANCEL deliveries flow through), and
/// send a METHOD:REPLY ICS back to the organizer with PARTSTAT=ACCEPTED.
pub async fn accept_imip(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<ImipAcceptRequest>,
) -> Result<Json<ImipAcceptResponse>, AppError> {
    let user_id = parse_user_id(&claims)?;
    let attendee_email = claims.username.clone();

    let partstat = match body.status.as_deref().unwrap_or("accepted") {
        "accepted" => "ACCEPTED",
        "declined" => "DECLINED",
        "maybe" => "TENTATIVE",
        other => {
            return Err(AppError::BadRequest(format!(
                "Invalid status '{}'. Must be one of: accepted, declined, maybe",
                other
            )))
        }
    };

    // NOTE: Reach for the user's IMAP server to pull the raw RFC822 bytes
    // — trusting whatever the SPA already cached would let a client spoof
    // the organizer email and trick us into sending a REPLY in their name.
    let imap_service = ImapService::for_user(&state, user_id).await?;
    let (imap_user, imap_pass) = imap_service
        .user_creds()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("BYOK IMAP credentials missing")))?;
    let raw_bytes = fetch_message_rfc822(&imap_service, imap_user, imap_pass, &body.folder, body.uid)
        .await?;

    let invite = parse_imip_from_email(&raw_bytes).map_err(AppError::BadRequest)?;

    // NOTE: An equal start/end is RFC-valid (point-in-time event) but our
    // CHECK constraint / business logic elsewhere assumes end > start. Bump
    // a same-instant event by 30 minutes so the row passes downstream
    // validation. The 30-minute default mirrors what Outlook uses when an
    // ICS lacks DTEND.
    let mut end_time = invite.end_time;
    if end_time <= invite.start_time {
        end_time = invite.start_time + chrono::Duration::minutes(30);
    }

    // NOTE: The application status maps from the iMIP METHOD: REQUEST → confirmed,
    // CANCEL → cancelled. Anything else stays tentative until further updates.
    let event_status = match invite.method.to_ascii_uppercase().as_str() {
        "CANCEL" => "cancelled",
        "REQUEST" | "PUBLISH" => "confirmed",
        _ => "tentative",
    };

    let event = CalendarEvent::upsert_by_ics_uid(
        &state.db,
        user_id,
        &invite.uid,
        &invite.summary,
        invite.description.as_deref(),
        invite.location.as_deref(),
        invite.start_time,
        end_time,
        invite.all_day,
        event_status,
        Some(body.uid as i32),
        Some(&body.folder),
    )
    .await?;

    // NOTE: Persist the parsed ATTENDEE list (organizer + invited people) so
    // the SPA can render the participant chips. The current user is appended
    // separately below with an explicit RSVP value reflecting their choice.
    if !invite.attendees.is_empty() {
        let to_create: Vec<AttendeeInput> = invite
            .attendees
            .iter()
            .filter(|a| !a.email.eq_ignore_ascii_case(&attendee_email))
            .map(|a| AttendeeInput {
                email: a.email.clone(),
                display_name: a.cn.clone(),
            })
            .collect();
        if !to_create.is_empty() {
            // NOTE: Best effort — failures here shouldn't roll back the event
            // upsert because the iMIP REPLY is the user-visible win.
            if let Err(e) = EventAttendee::create_bulk(&state.db, event.id, &to_create).await {
                tracing::warn!(event_id = %event.id, error = ?e, "iMIP attendee bulk-insert failed");
            }
        }
    }

    let rsvp_value = match partstat {
        "ACCEPTED" => "accepted",
        "DECLINED" => "declined",
        "TENTATIVE" => "maybe",
        _ => "accepted",
    };
    let _ = EventAttendee::upsert_public_rsvp(
        &state.db,
        event.id,
        &attendee_email,
        None,
        rsvp_value,
    )
    .await;

    // NOTE: Send the REPLY only when we have a real organizer email. Some
    // calendar systems omit ORGANIZER for personal events — in that case
    // there is no one to notify and we surface that as `reply_sent: false`.
    let (reply_sent, reply_error) = if let Some(ref organizer_email) = invite.organizer_email {
        match send_imip_reply_for_user(
            &state,
            user_id,
            &attendee_email,
            organizer_email,
            &invite,
            end_time,
            partstat,
        )
        .await
        {
            Ok(()) => (true, None),
            Err(e) => {
                tracing::warn!(error = ?e, "iMIP REPLY send failed");
                (false, Some(format!("{}", e)))
            }
        }
    } else {
        (false, Some("Invitation has no ORGANIZER address".to_string()))
    };

    let attendees = EventAttendee::list_for_event(&state.db, event.id).await?;
    Ok(Json(ImipAcceptResponse {
        event: CalendarEventWithAttendees { event, attendees },
        reply_sent,
        reply_error,
    }))
}

/// PURPOSE: Pull the raw RFC822 bytes for one message from the user's IMAP
/// server. Replicated locally rather than reusing `imap_service.get_message`
/// because that helper decodes MIME parts; we need the bytes intact so the
/// iCalendar parser sees the original calendar payload.
async fn fetch_message_rfc822(
    imap_service: &ImapService,
    username: &str,
    password: &str,
    folder: &str,
    uid: u32,
) -> Result<Vec<u8>, AppError> {
    let tcp_stream = tokio::net::TcpStream::connect((
        &*imap_service.imap_config().host,
        imap_service.imap_config().port,
    ))
    .await
    .map_err(|e| AppError::Imap(format!("TCP connection failed: {}", e)))?;

    let compat_stream = tokio_util::compat::TokioAsyncReadCompatExt::compat(tcp_stream);
    let tls = async_native_tls::TlsConnector::new();
    let tls_stream = tls
        .connect(&imap_service.imap_config().host, compat_stream)
        .await
        .map_err(|e| AppError::Imap(format!("TLS connection failed: {}", e)))?;
    let client = async_imap::Client::new(tls_stream);
    let mut session = client
        .login(username, password)
        .await
        .map_err(|e| AppError::Imap(format!("Login failed: {}", e.0)))?;
    session
        .select(folder)
        .await
        .map_err(|e| AppError::Imap(format!("SELECT '{}' failed: {}", folder, e)))?;

    let messages: Vec<_> = session
        .uid_fetch(uid.to_string(), "RFC822")
        .await
        .map_err(|e| AppError::Imap(format!("UID FETCH {} RFC822 failed: {}", uid, e)))?
        .try_collect()
        .await
        .map_err(|e| AppError::Imap(format!("UID FETCH stream failed: {}", e)))?;

    let msg = messages
        .first()
        .ok_or_else(|| AppError::NotFound(format!("Message UID {} not found in '{}'", uid, folder)))?;
    let body_bytes = msg
        .body()
        .ok_or_else(|| AppError::Imap(format!("No body for UID {}", uid)))?
        .to_vec();
    let _ = session.logout().await;
    Ok(body_bytes)
}

/// PURPOSE: Build the iCalendar REPLY payload and dispatch it via the user's
/// configured SMTP server (BYO-SMTP, per TMAIL-48). Lives in this handler
/// module rather than on SmtpService because it needs to load the user's
/// SMTP credentials, which is application-level concern.
async fn send_imip_reply_for_user(
    state: &AppState,
    user_id: uuid::Uuid,
    attendee_email: &str,
    organizer_email: &str,
    invite: &crate::services::imip_parser::ParsedInvite,
    end_time: DateTime<Utc>,
    partstat: &str,
) -> Result<(), AppError> {
    let ics_payload = generate_imip_reply(
        &invite.uid,
        &invite.summary,
        &invite.start_time,
        &end_time,
        invite.all_day,
        organizer_email,
        attendee_email,
        None,
        partstat,
    );

    // NOTE: Mirror the BYO-SMTP loader from handlers/messages.rs. We don't
    // factor this into a shared helper yet because there are only two call
    // sites; if a third comes along, lift this into services/smtp_service.rs.
    let smtp_cfg: crate::models::smtp_config::SmtpConfiguration =
        crate::models::smtp_config::SmtpConfiguration::find_default(&state.db, user_id)
            .await?
            .ok_or_else(|| {
                AppError::ServiceUnavailable(
                    "No SMTP server configured. Complete onboarding to enable replies.".into(),
                )
            })?;

    let enc_key = crate::models::ai_config::derive_encryption_key(&state.config.jwt.secret);
    let smtp_password = smtp_cfg
        .decrypted_password(&enc_key)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to decrypt SMTP password: {}", e)))?;
    let smtp_from = smtp_cfg
        .from_address
        .clone()
        .unwrap_or_else(|| smtp_cfg.username.clone());

    let smtp_runtime_cfg = crate::config::SmtpConfig {
        host: smtp_cfg.host.clone(),
        port: smtp_cfg.port as u16,
        tls: matches!(smtp_cfg.encryption.as_str(), "ssl" | "starttls"),
        notification_from: None,
        notification_username: None,
        notification_password: None,
    };
    let smtp_service = SmtpService::new(smtp_runtime_cfg);

    let verb = match partstat {
        "ACCEPTED" => "Accepted",
        "DECLINED" => "Declined",
        "TENTATIVE" => "Tentatively accepted",
        _ => "Responded to",
    };
    let subject = format!("{}: {}", verb, invite.summary);
    let text_body = format!(
        "{} has {} the invitation: {}\n",
        attendee_email,
        verb.to_lowercase(),
        invite.summary
    );

    smtp_service
        .send_imip_reply(
            &smtp_from,
            &smtp_password,
            organizer_email,
            &subject,
            &text_body,
            &ics_payload,
        )
        .await
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
            is_compliance_officer: false,
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
            is_compliance_officer: false,
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
    fn test_imip_accept_request_deserialization() {
        let json = r#"{"folder": "INBOX", "uid": 42, "status": "accepted"}"#;
        let req: ImipAcceptRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.folder, "INBOX");
        assert_eq!(req.uid, 42);
        assert_eq!(req.status.as_deref(), Some("accepted"));
    }

    #[test]
    fn test_imip_accept_request_defaults_status_to_none() {
        let json = r#"{"folder": "INBOX", "uid": 42}"#;
        let req: ImipAcceptRequest = serde_json::from_str(json).unwrap();
        assert!(req.status.is_none());
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
