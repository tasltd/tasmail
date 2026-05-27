// Added: ICS (iCalendar) file generator for meeting scheduling (TMAIL-127)
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// PURPOSE: Represents a single attendee in an ICS VEVENT
/// EXTERNAL: Used by calendar handler to pass attendee data for ICS generation
pub struct IcsAttendee {
    pub email: String,
    pub display_name: Option<String>,
    pub rsvp_status: String,
}

/// PURPOSE: Represents all data needed to generate an ICS VEVENT
pub struct IcsEventData {
    pub uid: String,
    pub summary: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub all_day: bool,
    pub organizer_email: String,
    pub attendees: Vec<IcsAttendee>,
    pub status: String,
}

/// PURPOSE: Map internal RSVP status to iCalendar PARTSTAT values per RFC 5545
/// CONSTRAINTS: Only accepts known rsvp_status values; defaults to NEEDS-ACTION
fn partstat_from_rsvp(rsvp: &str) -> &str {
    match rsvp {
        "accepted" => "ACCEPTED",
        "declined" => "DECLINED",
        "maybe" => "TENTATIVE",
        _ => "NEEDS-ACTION",
    }
}

/// PURPOSE: Map internal event status to iCalendar STATUS values per RFC 5545
fn ics_status_from_event_status(status: &str) -> &str {
    match status {
        "confirmed" => "CONFIRMED",
        "cancelled" => "CANCELLED",
        _ => "TENTATIVE",
    }
}

/// PURPOSE: Format a DateTime<Utc> as an iCalendar UTC timestamp (YYYYMMDDTHHMMSSZ)
fn format_ics_datetime(dt: &DateTime<Utc>) -> String {
    dt.format("%Y%m%dT%H%M%SZ").to_string()
}

/// PURPOSE: Format a DateTime<Utc> as an iCalendar all-day date (YYYYMMDD)
fn format_ics_date(dt: &DateTime<Utc>) -> String {
    dt.format("%Y%m%d").to_string()
}

/// PURPOSE: Fold long ICS lines per RFC 5545 Section 3.1 (max 75 octets, continuation with CRLF+space)
fn fold_line(line: &str) -> String {
    // NOTE: RFC 5545 limits content lines to 75 octets; we fold at 74 to leave room
    if line.len() <= 75 {
        return line.to_string();
    }
    let mut result = String::new();
    let mut remaining = line;
    let mut first = true;
    while !remaining.is_empty() {
        let max_len = if first { 75 } else { 74 }; // After fold, leading space counts
        let chunk_len = remaining.len().min(max_len);
        if !first {
            result.push(' ');
        }
        result.push_str(&remaining[..chunk_len]);
        remaining = &remaining[chunk_len..];
        if !remaining.is_empty() {
            result.push_str("\r\n");
        }
        first = false;
    }
    result
}

/// PURPOSE: Generate a complete ICS (iCalendar) file string for a single VEVENT
/// CONSTRAINTS: Output conforms to RFC 5545 format with CRLF line endings
/// EXTERNAL: Called by calendar handler for ICS download endpoint
pub fn generate_ics(event: &IcsEventData) -> String {
    let mut lines: Vec<String> = Vec::new();

    lines.push("BEGIN:VCALENDAR".to_string());
    lines.push("VERSION:2.0".to_string());
    lines.push("PRODID:-//TASMail//Calendar//EN".to_string());
    lines.push("CALSCALE:GREGORIAN".to_string());
    lines.push("METHOD:REQUEST".to_string());

    lines.push("BEGIN:VEVENT".to_string());
    lines.push(fold_line(&format!("UID:{}", event.uid)));
    lines.push(format!("DTSTAMP:{}", format_ics_datetime(&Utc::now())));

    // Added: Use VALUE=DATE for all-day events per RFC 5545
    if event.all_day {
        lines.push(fold_line(&format!("DTSTART;VALUE=DATE:{}", format_ics_date(&event.start_time))));
        lines.push(fold_line(&format!("DTEND;VALUE=DATE:{}", format_ics_date(&event.end_time))));
    } else {
        lines.push(fold_line(&format!("DTSTART:{}", format_ics_datetime(&event.start_time))));
        lines.push(fold_line(&format!("DTEND:{}", format_ics_datetime(&event.end_time))));
    }

    lines.push(fold_line(&format!("SUMMARY:{}", event.summary)));

    if let Some(ref desc) = event.description {
        // NOTE: Escape special chars in description per RFC 5545
        let escaped = desc.replace('\\', "\\\\").replace('\n', "\\n").replace(',', "\\,").replace(';', "\\;");
        lines.push(fold_line(&format!("DESCRIPTION:{}", escaped)));
    }

    if let Some(ref loc) = event.location {
        let escaped = loc.replace('\\', "\\\\").replace(',', "\\,").replace(';', "\\;");
        lines.push(fold_line(&format!("LOCATION:{}", escaped)));
    }

    lines.push(fold_line(&format!("ORGANIZER;CN=Organizer:mailto:{}", event.organizer_email)));
    lines.push(format!("STATUS:{}", ics_status_from_event_status(&event.status)));

    // Added: Attendee entries with RSVP and PARTSTAT
    for attendee in &event.attendees {
        let cn = attendee.display_name.as_deref().unwrap_or(&attendee.email);
        let partstat = partstat_from_rsvp(&attendee.rsvp_status);
        lines.push(fold_line(&format!(
            "ATTENDEE;CN={};RSVP=TRUE;PARTSTAT={}:mailto:{}",
            cn, partstat, attendee.email
        )));
    }

    lines.push("END:VEVENT".to_string());
    lines.push("END:VCALENDAR".to_string());

    // NOTE: ICS files use CRLF line endings per RFC 5545
    lines.join("\r\n") + "\r\n"
}

/// PURPOSE: Generate a unique ICS UID for a new event
/// CONSTRAINTS: Must be globally unique; uses UUID + domain format per RFC 5545 recommendation
pub fn generate_ics_uid(domain: &str) -> String {
    format!("{}@{}", Uuid::new_v4(), domain)
}

/// PURPOSE: Build a METHOD:REPLY iCalendar payload that records a single
/// attendee's PARTSTAT for an invitation. Used by the inbound iMIP accept
/// flow (TMAIL-127) to confirm acceptance back to the organizer.
/// CONSTRAINTS: `partstat` must be one of "ACCEPTED", "DECLINED",
/// "TENTATIVE" per RFC 5546 §3.2.3. `uid` must echo the original VEVENT's
/// UID so the organizer's client can correlate the reply.
pub fn generate_imip_reply(
    uid: &str,
    summary: &str,
    start_time: &DateTime<Utc>,
    end_time: &DateTime<Utc>,
    all_day: bool,
    organizer_email: &str,
    attendee_email: &str,
    attendee_cn: Option<&str>,
    partstat: &str,
) -> String {
    let mut lines: Vec<String> = Vec::new();
    lines.push("BEGIN:VCALENDAR".to_string());
    lines.push("VERSION:2.0".to_string());
    lines.push("PRODID:-//TASMail//Calendar//EN".to_string());
    lines.push("CALSCALE:GREGORIAN".to_string());
    lines.push("METHOD:REPLY".to_string());

    lines.push("BEGIN:VEVENT".to_string());
    lines.push(fold_line(&format!("UID:{}", uid)));
    lines.push(format!("DTSTAMP:{}", format_ics_datetime(&Utc::now())));
    if all_day {
        lines.push(fold_line(&format!("DTSTART;VALUE=DATE:{}", format_ics_date(start_time))));
        lines.push(fold_line(&format!("DTEND;VALUE=DATE:{}", format_ics_date(end_time))));
    } else {
        lines.push(fold_line(&format!("DTSTART:{}", format_ics_datetime(start_time))));
        lines.push(fold_line(&format!("DTEND:{}", format_ics_datetime(end_time))));
    }
    lines.push(fold_line(&format!("SUMMARY:{}", summary)));
    lines.push(fold_line(&format!("ORGANIZER:mailto:{}", organizer_email)));

    let cn = attendee_cn.unwrap_or(attendee_email);
    lines.push(fold_line(&format!(
        "ATTENDEE;CN={};PARTSTAT={}:mailto:{}",
        cn, partstat, attendee_email
    )));

    lines.push("END:VEVENT".to_string());
    lines.push("END:VCALENDAR".to_string());
    lines.join("\r\n") + "\r\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample_event() -> IcsEventData {
        IcsEventData {
            uid: "test-uid-123@tasmail.io".to_string(),
            summary: "Team Meeting".to_string(),
            description: Some("Weekly sync with the team".to_string()),
            location: Some("Room 42".to_string()),
            start_time: Utc.with_ymd_and_hms(2026, 4, 20, 10, 0, 0).unwrap(),
            end_time: Utc.with_ymd_and_hms(2026, 4, 20, 11, 0, 0).unwrap(),
            all_day: false,
            organizer_email: "organizer@tasmail.io".to_string(),
            attendees: vec![
                IcsAttendee {
                    email: "alice@example.com".to_string(),
                    display_name: Some("Alice".to_string()),
                    rsvp_status: "accepted".to_string(),
                },
                IcsAttendee {
                    email: "bob@example.com".to_string(),
                    display_name: None,
                    rsvp_status: "pending".to_string(),
                },
            ],
            status: "confirmed".to_string(),
        }
    }

    #[test]
    fn test_generate_ics_contains_vcalendar_wrapper() {
        let ics = generate_ics(&sample_event());
        assert!(ics.contains("BEGIN:VCALENDAR"));
        assert!(ics.contains("END:VCALENDAR"));
        assert!(ics.contains("VERSION:2.0"));
        assert!(ics.contains("PRODID:-//TASMail//Calendar//EN"));
    }

    #[test]
    fn test_generate_ics_contains_vevent() {
        let ics = generate_ics(&sample_event());
        assert!(ics.contains("BEGIN:VEVENT"));
        assert!(ics.contains("END:VEVENT"));
        assert!(ics.contains("UID:test-uid-123@tasmail.io"));
    }

    #[test]
    fn test_generate_ics_datetime_format() {
        let ics = generate_ics(&sample_event());
        assert!(ics.contains("DTSTART:20260420T100000Z"));
        assert!(ics.contains("DTEND:20260420T110000Z"));
    }

    #[test]
    fn test_generate_ics_summary_and_description() {
        let ics = generate_ics(&sample_event());
        assert!(ics.contains("SUMMARY:Team Meeting"));
        assert!(ics.contains("DESCRIPTION:Weekly sync with the team"));
    }

    #[test]
    fn test_generate_ics_location() {
        let ics = generate_ics(&sample_event());
        assert!(ics.contains("LOCATION:Room 42"));
    }

    #[test]
    fn test_generate_ics_organizer() {
        let ics = generate_ics(&sample_event());
        assert!(ics.contains("ORGANIZER;CN=Organizer:mailto:organizer@tasmail.io"));
    }

    #[test]
    fn test_generate_ics_attendees_with_partstat() {
        let ics = generate_ics(&sample_event());
        assert!(ics.contains("ATTENDEE;CN=Alice;RSVP=TRUE;PARTSTAT=ACCEPTED:mailto:alice@example.com"));
        // NOTE: Bob's attendee line is >75 chars so it gets folded; unfold before checking
        let unfolded = ics.replace("\r\n ", "");
        assert!(unfolded.contains("ATTENDEE;CN=bob@example.com;RSVP=TRUE;PARTSTAT=NEEDS-ACTION:mailto:bob@example.com"));
    }

    #[test]
    fn test_generate_ics_status_confirmed() {
        let ics = generate_ics(&sample_event());
        assert!(ics.contains("STATUS:CONFIRMED"));
    }

    #[test]
    fn test_generate_ics_status_cancelled() {
        let mut event = sample_event();
        event.status = "cancelled".to_string();
        let ics = generate_ics(&event);
        assert!(ics.contains("STATUS:CANCELLED"));
    }

    #[test]
    fn test_generate_ics_status_tentative() {
        let mut event = sample_event();
        event.status = "tentative".to_string();
        let ics = generate_ics(&event);
        assert!(ics.contains("STATUS:TENTATIVE"));
    }

    #[test]
    fn test_generate_ics_all_day_event() {
        let mut event = sample_event();
        event.all_day = true;
        let ics = generate_ics(&event);
        assert!(ics.contains("DTSTART;VALUE=DATE:20260420"));
        assert!(ics.contains("DTEND;VALUE=DATE:20260420"));
        // NOTE: All-day events should NOT have time component
        assert!(!ics.contains("DTSTART:2026"));
    }

    #[test]
    fn test_generate_ics_no_attendees() {
        let mut event = sample_event();
        event.attendees = vec![];
        let ics = generate_ics(&event);
        assert!(!ics.contains("ATTENDEE"));
        assert!(ics.contains("BEGIN:VEVENT"));
    }

    #[test]
    fn test_generate_ics_no_description() {
        let mut event = sample_event();
        event.description = None;
        let ics = generate_ics(&event);
        assert!(!ics.contains("DESCRIPTION"));
    }

    #[test]
    fn test_generate_ics_no_location() {
        let mut event = sample_event();
        event.location = None;
        let ics = generate_ics(&event);
        assert!(!ics.contains("LOCATION"));
    }

    #[test]
    fn test_generate_ics_crlf_line_endings() {
        let ics = generate_ics(&sample_event());
        // NOTE: Every line should end with \r\n
        assert!(ics.contains("\r\n"));
        assert!(ics.ends_with("\r\n"));
    }

    #[test]
    fn test_generate_ics_description_escaping() {
        let mut event = sample_event();
        event.description = Some("Line1\nLine2, with comma; and semicolon".to_string());
        let ics = generate_ics(&event);
        assert!(ics.contains("DESCRIPTION:Line1\\nLine2\\, with comma\\; and semicolon"));
    }

    #[test]
    fn test_generate_ics_uid_format() {
        let uid = generate_ics_uid("tasmail.io");
        assert!(uid.ends_with("@tasmail.io"));
        // NOTE: UUID part should be 36 chars (8-4-4-4-12)
        let parts: Vec<&str> = uid.splitn(2, '@').collect();
        assert_eq!(parts[1], "tasmail.io");
        assert_eq!(parts[0].len(), 36);
    }

    #[test]
    fn test_partstat_mapping() {
        assert_eq!(partstat_from_rsvp("accepted"), "ACCEPTED");
        assert_eq!(partstat_from_rsvp("declined"), "DECLINED");
        assert_eq!(partstat_from_rsvp("maybe"), "TENTATIVE");
        assert_eq!(partstat_from_rsvp("pending"), "NEEDS-ACTION");
        assert_eq!(partstat_from_rsvp("unknown"), "NEEDS-ACTION");
    }

    #[test]
    fn test_ics_status_mapping() {
        assert_eq!(ics_status_from_event_status("confirmed"), "CONFIRMED");
        assert_eq!(ics_status_from_event_status("cancelled"), "CANCELLED");
        assert_eq!(ics_status_from_event_status("tentative"), "TENTATIVE");
        assert_eq!(ics_status_from_event_status("other"), "TENTATIVE");
    }

    #[test]
    fn test_format_ics_datetime_format() {
        let dt = Utc.with_ymd_and_hms(2026, 1, 15, 9, 30, 0).unwrap();
        assert_eq!(format_ics_datetime(&dt), "20260115T093000Z");
    }

    #[test]
    fn test_format_ics_date_format() {
        let dt = Utc.with_ymd_and_hms(2026, 12, 25, 0, 0, 0).unwrap();
        assert_eq!(format_ics_date(&dt), "20261225");
    }

    #[test]
    fn test_fold_line_short() {
        let line = "SHORT LINE";
        assert_eq!(fold_line(line), "SHORT LINE");
    }

    #[test]
    fn test_generate_imip_reply_contains_method_reply_and_partstat() {
        let start = Utc.with_ymd_and_hms(2026, 4, 20, 10, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 4, 20, 11, 0, 0).unwrap();
        let reply = generate_imip_reply(
            "abc-123@example.com",
            "Project sync",
            &start,
            &end,
            false,
            "alice@example.com",
            "bob@example.com",
            Some("Bob"),
            "ACCEPTED",
        );
        assert!(reply.contains("METHOD:REPLY"));
        assert!(reply.contains("UID:abc-123@example.com"));
        assert!(reply.contains("ORGANIZER:mailto:alice@example.com"));
        assert!(reply.contains("ATTENDEE;CN=Bob;PARTSTAT=ACCEPTED:mailto:bob@example.com"));
        assert!(reply.contains("DTSTART:20260420T100000Z"));
        assert!(reply.contains("DTEND:20260420T110000Z"));
        assert!(reply.contains("BEGIN:VEVENT"));
        assert!(reply.contains("END:VEVENT"));
    }

    #[test]
    fn test_generate_imip_reply_all_day_uses_date_value() {
        let start = Utc.with_ymd_and_hms(2026, 4, 20, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 4, 21, 0, 0, 0).unwrap();
        let reply = generate_imip_reply(
            "all-day@example.com",
            "Offsite",
            &start,
            &end,
            true,
            "alice@example.com",
            "bob@example.com",
            None,
            "ACCEPTED",
        );
        assert!(reply.contains("DTSTART;VALUE=DATE:20260420"));
        assert!(reply.contains("DTEND;VALUE=DATE:20260421"));
        // CN defaults to attendee email when display name is missing.
        assert!(reply.contains("ATTENDEE;CN=bob@example.com;PARTSTAT=ACCEPTED:mailto:bob@example.com"));
    }

    #[test]
    fn test_fold_line_long() {
        let line = "A".repeat(100);
        let folded = fold_line(&line);
        // NOTE: Folded output should contain CRLF continuation
        assert!(folded.contains("\r\n"));
        // Verify all content is preserved after unfolding
        let unfolded: String = folded.replace("\r\n ", "");
        assert_eq!(unfolded, line);
    }
}
