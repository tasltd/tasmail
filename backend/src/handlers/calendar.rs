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
use crate::services::slot_suggester::{
    suggest_slots, BusyInterval, SuggestSlotsInput, WorkingHours,
};
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

    // Added (TMAIL-127): Auto-send iMIP REQUEST invitation to every attendee
    // using the organizer's BYO-SMTP. Best-effort — SMTP failures are logged
    // but don't roll back event creation, mirroring the iMIP REPLY flow in
    // `accept_imip` so the user-visible win (event saved) always survives.
    if !attendees.is_empty() {
        send_imip_invitations(&state, user_id, &claims.username, &event, &attendees).await;
    }

    Ok((
        StatusCode::CREATED,
        Json(CalendarEventWithAttendees { event, attendees }),
    ))
}

/// PURPOSE: Send a METHOD:REQUEST iMIP invitation to each attendee of a newly
/// created event via the organizer's configured BYO-SMTP server (TMAIL-127).
/// CONSTRAINTS: Best-effort — every failure path (no SMTP config, decrypt
/// failure, transport failure per attendee) is logged via `tracing` and
/// swallowed so the event-creation response stays a 2xx.
/// EXTERNAL: Loads `smtp_configurations` for the user, decrypts the password
/// with the JWT-derived key, then dials the user's SMTP host once per
/// attendee with the same ICS payload (only To/recipient differs).
async fn send_imip_invitations(
    state: &AppState,
    user_id: uuid::Uuid,
    organizer_email: &str,
    event: &CalendarEvent,
    attendees: &[EventAttendee],
) {
    // ---- Load the user's default BYO-SMTP server -----------------------
    let smtp_cfg = match crate::models::smtp_config::SmtpConfiguration::find_default(
        &state.db, user_id,
    )
    .await
    {
        Ok(Some(c)) => c,
        Ok(None) => {
            tracing::warn!(
                user_id = %user_id,
                event_id = %event.id,
                "iMIP invite skipped: no default SMTP server configured"
            );
            return;
        }
        Err(e) => {
            tracing::warn!(
                user_id = %user_id,
                event_id = %event.id,
                error = ?e,
                "iMIP invite skipped: SMTP config lookup failed"
            );
            return;
        }
    };

    let enc_key = crate::models::ai_config::derive_encryption_key(&state.config.jwt.secret);
    let smtp_password = match smtp_cfg.decrypted_password(&enc_key) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(
                user_id = %user_id,
                event_id = %event.id,
                error = %e,
                "iMIP invite skipped: SMTP password decrypt failed"
            );
            return;
        }
    };
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

    // ---- Build the ICS payload once (same body for every attendee) -----
    let ics_data = IcsEventData {
        uid: event.ics_uid.clone(),
        summary: event.title.clone(),
        description: event.description.clone(),
        location: event.location.clone(),
        start_time: event.start_time,
        end_time: event.end_time,
        all_day: event.all_day,
        organizer_email: organizer_email.to_string(),
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
    let ics_payload = generate_ics(&ics_data);

    let subject = format!("Invitation: {}", event.title);
    let text_body = build_invite_text_body(event, organizer_email);

    // ---- Fan out one send per attendee --------------------------------
    for attendee in attendees {
        if let Err(e) = smtp_service
            .send_imip_request(
                &smtp_from,
                &smtp_password,
                &attendee.email,
                &subject,
                &text_body,
                &ics_payload,
            )
            .await
        {
            tracing::warn!(
                event_id = %event.id,
                attendee = %attendee.email,
                error = ?e,
                "iMIP invitation send failed"
            );
        }
    }
}

/// PURPOSE: Build the plain-text fallback body shown by mail clients that
/// don't recognise the text/calendar part. Keep it short — the calendar
/// app gets all the structured detail from the ICS half.
fn build_invite_text_body(event: &CalendarEvent, organizer_email: &str) -> String {
    let mut body = format!("You have been invited to: {}\n\n", event.title);
    body.push_str(&format!("Organizer: {}\n", organizer_email));
    body.push_str(&format!(
        "Start: {}\n",
        event.start_time.to_rfc3339()
    ));
    body.push_str(&format!("End: {}\n", event.end_time.to_rfc3339()));
    if let Some(ref loc) = event.location {
        body.push_str(&format!("Location: {}\n", loc));
    }
    if let Some(ref desc) = event.description {
        body.push_str(&format!("\n{}\n", desc));
    }
    body
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

// ============================================================================
// Added (TMAIL-127): Free-busy + slot-suggestion endpoints
// ============================================================================
//
// `/api/calendar/free-busy` returns the busy windows for one or more attendees
// in the requested date range. Internal users (those who exist in the
// `mailboxes` table) have their actual calendar consulted; external attendees
// come back with a `not_resolved` status so the UI can grey them out instead
// of silently treating them as free.
//
// `/api/calendar/suggest-slots` layers on top: pull busy intervals for every
// attendee, hand them to the pure `slot_suggester` algorithm, and return up
// to `max_slots` candidate windows. This is the dependency surface for the
// composer's "Suggest Slots" panel — keep the request shape stable.

// NOTE: Maximum date span the suggester is willing to consider in a single
// request. Caps response time and protects the DB. Two weeks is enough for
// 95th-percentile scheduling workflows; longer ranges should paginate.
const MAX_SUGGEST_RANGE_DAYS: i64 = 14;

// NOTE: Maximum attendees in a single free-busy / suggest call. Beyond this
// the union of busy intervals is so dense that suggestions are useless, and
// the per-attendee DB lookups become a fan-out hazard.
const MAX_ATTENDEES_PER_REQUEST: usize = 25;

#[derive(Debug, Deserialize)]
pub struct FreeBusyRequest {
    pub attendees: Vec<String>,
    pub range_start: DateTime<Utc>,
    pub range_end: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct AttendeeBusy {
    pub email: String,
    /// One of: "resolved" (busy times populated from DB) | "not_resolved"
    /// (no matching mailbox — treated as unknown availability, the UI should
    /// flag this rather than assume they're free).
    pub status: String,
    pub busy: Vec<BusySpan>,
}

#[derive(Debug, Serialize, Clone, Copy)]
pub struct BusySpan {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct FreeBusyResponse {
    pub attendees: Vec<AttendeeBusy>,
}

#[derive(Debug, Deserialize)]
pub struct SuggestSlotsRequest {
    pub attendees: Vec<String>,
    pub duration_minutes: i64,
    pub range_start: DateTime<Utc>,
    pub range_end: DateTime<Utc>,
    /// Working day window in UTC minutes-from-midnight. Defaults to 09:00–17:00.
    pub working_start_minute: Option<u32>,
    pub working_end_minute: Option<u32>,
    pub include_weekends: Option<bool>,
    /// Number of slots requested. Server caps at 50.
    pub max_slots: Option<usize>,
    /// Slot start alignment in minutes (15, 30, 60). Defaults to 30.
    pub step_minutes: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct SuggestedSlotDto {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct SuggestSlotsResponse {
    pub slots: Vec<SuggestedSlotDto>,
    /// Attendees whose calendars couldn't be loaded (external recipients).
    /// They're treated as "always free" during slot search but surfaced
    /// here so the caller can warn the user.
    pub unresolved_attendees: Vec<String>,
}

/// Look up the mailbox id for a (case-insensitive) email address. Returns
/// None for external attendees so the caller can decide how to handle them.
async fn resolve_mailbox_id(
    pool: &sqlx::PgPool,
    email: &str,
) -> Result<Option<uuid::Uuid>, AppError> {
    let row: Option<(uuid::Uuid,)> = sqlx::query_as(
        "SELECT id FROM mailboxes WHERE LOWER(username) = LOWER($1) AND active = true LIMIT 1",
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.0))
}

/// Shared validation for the date range / attendee list used by both the
/// free-busy and suggest-slots endpoints.
fn validate_range(
    attendees: &[String],
    range_start: DateTime<Utc>,
    range_end: DateTime<Utc>,
) -> Result<(), AppError> {
    if attendees.is_empty() {
        return Err(AppError::BadRequest(
            "At least one attendee is required".to_string(),
        ));
    }
    if attendees.len() > MAX_ATTENDEES_PER_REQUEST {
        return Err(AppError::BadRequest(format!(
            "Maximum {MAX_ATTENDEES_PER_REQUEST} attendees per request"
        )));
    }
    if range_end <= range_start {
        return Err(AppError::BadRequest(
            "range_end must be after range_start".to_string(),
        ));
    }
    let span = range_end - range_start;
    if span > chrono::Duration::days(MAX_SUGGEST_RANGE_DAYS) {
        return Err(AppError::BadRequest(format!(
            "Date range must be <= {MAX_SUGGEST_RANGE_DAYS} days"
        )));
    }
    Ok(())
}

/// POST /api/calendar/free-busy — return busy intervals for a list of
/// attendees inside the given date range. The authenticated user is always
/// implicitly included if present in the attendees list.
pub async fn get_free_busy(
    State(state): State<AppState>,
    axum::Extension(_claims): axum::Extension<Claims>,
    Json(body): Json<FreeBusyRequest>,
) -> Result<Json<FreeBusyResponse>, AppError> {
    validate_range(&body.attendees, body.range_start, body.range_end)?;

    let mut out: Vec<AttendeeBusy> = Vec::with_capacity(body.attendees.len());
    // NOTE: Dedupe attendees case-insensitively before hitting the DB —
    // the composer often passes the current user in both To and Cc.
    let mut seen = std::collections::HashSet::new();
    for email in &body.attendees {
        let trimmed = email.trim();
        if trimmed.is_empty() {
            continue;
        }
        let key = trimmed.to_lowercase();
        if !seen.insert(key.clone()) {
            continue;
        }

        match resolve_mailbox_id(&state.db, trimmed).await? {
            Some(mailbox_id) => {
                let intervals = CalendarEvent::busy_intervals_for_organizer(
                    &state.db,
                    mailbox_id,
                    body.range_start,
                    body.range_end,
                )
                .await?;
                let busy = intervals
                    .into_iter()
                    .map(|(s, e)| BusySpan { start: s, end: e })
                    .collect();
                out.push(AttendeeBusy {
                    email: trimmed.to_string(),
                    status: "resolved".to_string(),
                    busy,
                });
            }
            None => {
                out.push(AttendeeBusy {
                    email: trimmed.to_string(),
                    status: "not_resolved".to_string(),
                    busy: vec![],
                });
            }
        }
    }

    Ok(Json(FreeBusyResponse { attendees: out }))
}

/// POST /api/calendar/suggest-slots — return up to N candidate meeting slots
/// where every internal attendee is free and that fall inside the working
/// hours window. External attendees are surfaced under `unresolved_attendees`
/// so the caller can warn the user that their availability is unknown.
pub async fn suggest_meeting_slots(
    State(state): State<AppState>,
    axum::Extension(_claims): axum::Extension<Claims>,
    Json(body): Json<SuggestSlotsRequest>,
) -> Result<Json<SuggestSlotsResponse>, AppError> {
    validate_range(&body.attendees, body.range_start, body.range_end)?;

    if body.duration_minutes <= 0 {
        return Err(AppError::BadRequest(
            "duration_minutes must be > 0".to_string(),
        ));
    }
    if body.duration_minutes > 24 * 60 {
        return Err(AppError::BadRequest(
            "duration_minutes must be <= 1440 (24 hours)".to_string(),
        ));
    }

    // ---- Collect busy intervals across every resolvable attendee --------
    let mut combined: Vec<BusyInterval> = Vec::new();
    let mut unresolved: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for email in &body.attendees {
        let trimmed = email.trim();
        if trimmed.is_empty() {
            continue;
        }
        let key = trimmed.to_lowercase();
        if !seen.insert(key) {
            continue;
        }

        match resolve_mailbox_id(&state.db, trimmed).await? {
            Some(mailbox_id) => {
                let intervals = CalendarEvent::busy_intervals_for_organizer(
                    &state.db,
                    mailbox_id,
                    body.range_start,
                    body.range_end,
                )
                .await?;
                for (start, end) in intervals {
                    combined.push(BusyInterval { start, end });
                }
            }
            None => {
                unresolved.push(trimmed.to_string());
            }
        }
    }

    // ---- Build the input + delegate to the pure suggester --------------
    let working_hours = WorkingHours {
        start_minute: body.working_start_minute.unwrap_or(9 * 60),
        end_minute: body.working_end_minute.unwrap_or(17 * 60),
        include_weekends: body.include_weekends.unwrap_or(false),
    };
    let input = SuggestSlotsInput {
        busy: combined,
        range_start: body.range_start,
        range_end: body.range_end,
        duration: chrono::Duration::minutes(body.duration_minutes),
        working_hours,
        max_slots: body.max_slots.unwrap_or(5),
        step_minutes: body.step_minutes.unwrap_or(30),
    };

    let slots = suggest_slots(input).map_err(AppError::BadRequest)?;
    let dto = slots
        .into_iter()
        .map(|s| SuggestedSlotDto {
            start: s.start,
            end: s.end,
        })
        .collect();

    Ok(Json(SuggestSlotsResponse {
        slots: dto,
        unresolved_attendees: unresolved,
    }))
}

// ============================================================================
// Added (TMAIL-266 / TMAIL-127): GET /api/calendar/free-busy
// ============================================================================
//
// CalDAV-aware variant of the POST free-busy endpoint above. Accepts the same
// inputs as query parameters (so the composer can hit it from a TanStack
// Query hook keyed by URL) and additionally fans out to the authenticated
// user's configured CalDAV servers (`dav_configurations`, TMAIL-117) — their
// busy windows are unioned with the local `calendar_events` data for every
// attendee that matches the auth user's email.
//
// Why limit external fan-out to the auth user only:
//   * The DAV credentials stored in dav_configurations belong to a single
//     mailbox and may include user-specific scopes (e.g. an app password
//     issued to "Dominic on TASMail"). Re-using one user's creds to query
//     another user's external calendar would be a privacy violation.
//   * For *internal* attendees we already have the source of truth — the
//     calendar_events table holds every event they organize or accept.
//
// The endpoint degrades gracefully: if a DAV server is down or returns a
// non-2xx the failure is logged and the response falls back to local-only
// for the affected attendee. We never bubble external errors up to the
// caller because the composer must stay responsive.

/// Query string for GET /api/calendar/free-busy.
///
/// Example: `?emails=a@x.com,b@y.com&start=...&end=...`
#[derive(Debug, Deserialize)]
pub struct FreeBusyQuery {
    /// Comma-separated attendee emails. Empty/whitespace entries are
    /// silently dropped before validation.
    pub emails: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

/// GET /api/calendar/free-busy?emails=...&start=...&end=...
///
/// Returns merged busy windows per attendee. Local `calendar_events`
/// always contribute; the authenticated user's CalDAV servers contribute
/// to the auth user's own attendee row when their email is in the list.
pub async fn get_free_busy_query(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Query(params): Query<FreeBusyQuery>,
) -> Result<Json<FreeBusyResponse>, AppError> {
    let emails: Vec<String> = params
        .emails
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    validate_range(&emails, params.start, params.end)?;

    // Pre-fetch the auth user's CalDAV busy intervals once — the same
    // payload feeds every attendee row that matches their email.
    let auth_user_id = parse_user_id(&claims)?;
    let auth_dav_busy =
        fetch_user_caldav_busy(&state, auth_user_id, params.start, params.end).await;
    let auth_email_lc = claims.username.trim().to_lowercase();

    let mut out: Vec<AttendeeBusy> = Vec::with_capacity(emails.len());
    let mut seen = std::collections::HashSet::new();
    for email in &emails {
        let trimmed = email.trim();
        let key = trimmed.to_lowercase();
        if !seen.insert(key.clone()) {
            continue;
        }

        match resolve_mailbox_id(&state.db, trimmed).await? {
            Some(mailbox_id) => {
                let local = CalendarEvent::busy_intervals_for_organizer(
                    &state.db,
                    mailbox_id,
                    params.start,
                    params.end,
                )
                .await?;
                let mut all: Vec<BusyInterval> = local
                    .into_iter()
                    .map(|(s, e)| BusyInterval { start: s, end: e })
                    .collect();
                if !auth_dav_busy.is_empty() && key == auth_email_lc {
                    all.extend(auth_dav_busy.iter().copied());
                }
                let merged = crate::services::slot_suggester::merge_busy(all);
                let busy = merged
                    .into_iter()
                    .map(|i| BusySpan {
                        start: i.start,
                        end: i.end,
                    })
                    .collect();
                out.push(AttendeeBusy {
                    email: trimmed.to_string(),
                    status: "resolved".to_string(),
                    busy,
                });
            }
            None => {
                out.push(AttendeeBusy {
                    email: trimmed.to_string(),
                    status: "not_resolved".to_string(),
                    busy: vec![],
                });
            }
        }
    }

    Ok(Json(FreeBusyResponse { attendees: out }))
}

/// Fetch every enabled CalDAV server configured for `user_id` and union
/// their FREEBUSY responses into a single Vec<BusyInterval>.
///
/// All errors (HTTP failure, decrypt failure, unparseable VFREEBUSY) are
/// logged and swallowed — see the module-level comment for the rationale.
async fn fetch_user_caldav_busy(
    state: &AppState,
    user_id: uuid::Uuid,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Vec<BusyInterval> {
    use crate::models::ai_config::{decrypt_api_key, derive_encryption_key};
    use crate::models::dav_config::DavConfiguration;
    use crate::services::caldav_freebusy::query_caldav_freebusy;

    let configs = match DavConfiguration::find_by_user(&state.db, user_id).await {
        Ok(c) => c,
        Err(err) => {
            tracing::warn!(error = ?err, user_id = %user_id, "free-busy: failed to load dav_configurations");
            return Vec::new();
        }
    };
    if configs.is_empty() {
        return Vec::new();
    }
    let key = derive_encryption_key(&state.config.jwt.secret);
    let mut out: Vec<BusyInterval> = Vec::new();
    for cfg in configs {
        if !cfg.enabled {
            continue;
        }
        // Only CalDAV (or "both") configs participate — CardDAV-only ones
        // hold contacts, not calendars.
        if cfg.dav_type == "carddav" {
            continue;
        }
        let password = match decrypt_api_key(&cfg.encrypted_password, &key) {
            Ok(p) => p,
            Err(err) => {
                tracing::warn!(
                    error = ?err,
                    dav_config = %cfg.id,
                    "free-busy: failed to decrypt CalDAV password — skipping"
                );
                continue;
            }
        };
        match query_caldav_freebusy(&cfg.server_url, &cfg.username, &password, start, end).await {
            Ok(windows) => {
                for w in windows {
                    out.push(BusyInterval {
                        start: w.start,
                        end: w.end,
                    });
                }
            }
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    dav_config = %cfg.id,
                    server = %cfg.server_url,
                    "free-busy: CalDAV REPORT failed — falling back to local events only"
                );
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::calendar_event::{CreateEventRequest, UpdateEventRequest, RsvpRequest, AttendeeInput};
    use crate::services::auth_service::Claims;
    use chrono::TimeZone;

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

    // Added (TMAIL-127): plain-text invite body must echo title, organizer,
    // start/end and optional location/description so non-calendar mail
    // clients still show something useful.
    #[test]
    fn test_build_invite_text_body_includes_all_fields() {
        let event = CalendarEvent {
            id: uuid::Uuid::new_v4(),
            organizer_id: uuid::Uuid::new_v4(),
            title: "Q3 Review".to_string(),
            description: Some("Quarterly business review".to_string()),
            location: Some("Conference Room 1".to_string()),
            start_time: chrono::Utc.with_ymd_and_hms(2026, 7, 15, 10, 0, 0).unwrap(),
            end_time: chrono::Utc.with_ymd_and_hms(2026, 7, 15, 11, 0, 0).unwrap(),
            all_day: false,
            recurrence_rule: None,
            status: "confirmed".to_string(),
            linked_message_uid: None,
            linked_folder: None,
            ics_uid: "abc@tasmail.io".to_string(),
            public_token: uuid::Uuid::new_v4(),
            public_enabled: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let body = build_invite_text_body(&event, "alice@tasmail.io");
        assert!(body.contains("Q3 Review"));
        assert!(body.contains("alice@tasmail.io"));
        assert!(body.contains("2026-07-15T10:00:00+00:00"));
        assert!(body.contains("2026-07-15T11:00:00+00:00"));
        assert!(body.contains("Conference Room 1"));
        assert!(body.contains("Quarterly business review"));
    }

    #[test]
    fn test_build_invite_text_body_omits_optional_fields_when_absent() {
        let event = CalendarEvent {
            id: uuid::Uuid::new_v4(),
            organizer_id: uuid::Uuid::new_v4(),
            title: "Quick Chat".to_string(),
            description: None,
            location: None,
            start_time: chrono::Utc.with_ymd_and_hms(2026, 7, 15, 10, 0, 0).unwrap(),
            end_time: chrono::Utc.with_ymd_and_hms(2026, 7, 15, 10, 15, 0).unwrap(),
            all_day: false,
            recurrence_rule: None,
            status: "confirmed".to_string(),
            linked_message_uid: None,
            linked_folder: None,
            ics_uid: "xyz@tasmail.io".to_string(),
            public_token: uuid::Uuid::new_v4(),
            public_enabled: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let body = build_invite_text_body(&event, "alice@tasmail.io");
        assert!(!body.contains("Location:"));
        assert!(body.contains("Quick Chat"));
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

    // ---- TMAIL-127: free-busy + suggest-slots request shapes -----------

    #[test]
    fn free_busy_request_deserialization() {
        let json = r#"{
            "attendees": ["a@x.com", "b@y.com"],
            "range_start": "2026-06-01T00:00:00Z",
            "range_end":   "2026-06-08T00:00:00Z"
        }"#;
        let req: FreeBusyRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.attendees.len(), 2);
    }

    #[test]
    fn suggest_slots_request_defaults_apply_on_missing_fields() {
        let json = r#"{
            "attendees": ["a@x.com"],
            "duration_minutes": 30,
            "range_start": "2026-06-01T00:00:00Z",
            "range_end":   "2026-06-05T00:00:00Z"
        }"#;
        let req: SuggestSlotsRequest = serde_json::from_str(json).unwrap();
        assert!(req.working_start_minute.is_none());
        assert!(req.working_end_minute.is_none());
        assert!(req.include_weekends.is_none());
        assert!(req.max_slots.is_none());
        assert!(req.step_minutes.is_none());
        assert_eq!(req.duration_minutes, 30);
    }

    #[test]
    fn suggest_slots_request_accepts_all_optional_fields() {
        let json = r#"{
            "attendees": ["a@x.com"],
            "duration_minutes": 45,
            "range_start": "2026-06-01T00:00:00Z",
            "range_end":   "2026-06-05T00:00:00Z",
            "working_start_minute": 480,
            "working_end_minute":   1020,
            "include_weekends": true,
            "max_slots": 10,
            "step_minutes": 15
        }"#;
        let req: SuggestSlotsRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.working_start_minute, Some(480));
        assert_eq!(req.working_end_minute, Some(1020));
        assert_eq!(req.include_weekends, Some(true));
        assert_eq!(req.max_slots, Some(10));
        assert_eq!(req.step_minutes, Some(15));
    }

    #[test]
    fn validate_range_rejects_empty_attendees() {
        let err = validate_range(
            &[],
            Utc::now(),
            Utc::now() + chrono::Duration::hours(1),
        )
        .unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn validate_range_rejects_reversed_range() {
        let now = Utc::now();
        let err = validate_range(
            &["a@x.com".to_string()],
            now + chrono::Duration::hours(1),
            now,
        )
        .unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn validate_range_rejects_too_long_range() {
        let now = Utc::now();
        let err = validate_range(
            &["a@x.com".to_string()],
            now,
            now + chrono::Duration::days(MAX_SUGGEST_RANGE_DAYS + 1),
        )
        .unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn validate_range_rejects_too_many_attendees() {
        let attendees: Vec<String> = (0..MAX_ATTENDEES_PER_REQUEST + 1)
            .map(|i| format!("user{i}@example.com"))
            .collect();
        let now = Utc::now();
        let err = validate_range(
            &attendees,
            now,
            now + chrono::Duration::days(1),
        )
        .unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn validate_range_accepts_valid_input() {
        let now = Utc::now();
        assert!(validate_range(
            &["a@x.com".to_string()],
            now,
            now + chrono::Duration::days(7),
        )
        .is_ok());
    }

    #[test]
    fn suggest_slots_response_serializes_fields() {
        let resp = SuggestSlotsResponse {
            slots: vec![SuggestedSlotDto {
                start: Utc::now(),
                end: Utc::now() + chrono::Duration::minutes(30),
            }],
            unresolved_attendees: vec!["external@other.com".to_string()],
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert!(json["slots"].is_array());
        assert_eq!(json["slots"].as_array().unwrap().len(), 1);
        assert_eq!(json["unresolved_attendees"][0], "external@other.com");
    }

    #[test]
    fn attendee_busy_serializes_status_field() {
        let ab = AttendeeBusy {
            email: "a@x.com".into(),
            status: "resolved".into(),
            busy: vec![BusySpan {
                start: Utc::now(),
                end: Utc::now() + chrono::Duration::hours(1),
            }],
        };
        let json = serde_json::to_value(&ab).unwrap();
        assert_eq!(json["email"], "a@x.com");
        assert_eq!(json["status"], "resolved");
        assert_eq!(json["busy"].as_array().unwrap().len(), 1);
    }

    // ---- TMAIL-266: GET /api/calendar/free-busy query-string shape ------

    #[test]
    fn free_busy_query_deserializes_from_querystring() {
        // axum::extract::Query uses serde_urlencoded, so the canonical
        // querystring shape must round-trip through that codec.
        let qs = "emails=a%40x.com%2Cb%40y.com&start=2026-06-01T00:00:00Z&end=2026-06-08T00:00:00Z";
        let q: FreeBusyQuery = serde_urlencoded::from_str(qs).unwrap();
        assert_eq!(q.emails, "a@x.com,b@y.com");
        assert_eq!(q.start.to_rfc3339(), "2026-06-01T00:00:00+00:00");
        assert_eq!(q.end.to_rfc3339(), "2026-06-08T00:00:00+00:00");
    }

    #[test]
    fn free_busy_query_splits_and_trims_emails() {
        let q = FreeBusyQuery {
            emails: " a@x.com , , b@y.com,  ".to_string(),
            start: Utc::now(),
            end: Utc::now() + chrono::Duration::hours(1),
        };
        let parsed: Vec<String> = q
            .emails
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        assert_eq!(parsed, vec!["a@x.com".to_string(), "b@y.com".to_string()]);
    }

    #[test]
    fn free_busy_query_rejects_missing_required_fields() {
        // Missing start
        let err = serde_urlencoded::from_str::<FreeBusyQuery>(
            "emails=a%40x.com&end=2026-06-08T00:00:00Z",
        );
        assert!(err.is_err());
        // Missing emails
        let err = serde_urlencoded::from_str::<FreeBusyQuery>(
            "start=2026-06-01T00:00:00Z&end=2026-06-08T00:00:00Z",
        );
        assert!(err.is_err());
    }
}
