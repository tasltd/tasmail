// Added (TMAIL-127): Inbound iMIP parser for VEVENT invitations.
//
// Parses inbound `text/calendar; method=REQUEST|REPLY|CANCEL` MIME parts
// per RFC 5545 + RFC 6047 (iMIP). Extracts the VEVENT fields needed to
// upsert a row in `calendar_events` and to build a METHOD:REPLY back to
// the organizer.
//
// This is a deliberately small subset of iCalendar — only what's required
// for the "Accept invite" flow:
//   * VCALENDAR/VEVENT envelopes
//   * METHOD, UID, SUMMARY, DESCRIPTION, LOCATION
//   * DTSTART / DTEND in UTC (`Z` suffix), floating local, or VALUE=DATE
//   * ORGANIZER, ATTENDEE (with CN= and PARTSTAT= parameters)
//
// Anything more exotic (RRULE expansion, VTIMEZONE resolution, X-* props)
// is preserved as raw text on the event but not interpreted here.
use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use mailparse::ParsedMail;

/// PURPOSE: Decoded representation of a single inbound iMIP VEVENT.
/// EXTERNAL: Consumed by `handlers::calendar::accept_imip` to upsert
/// the event into `calendar_events` and to build the outbound REPLY.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedInvite {
    /// METHOD from the surrounding VCALENDAR (REQUEST, REPLY, CANCEL, …).
    /// Defaults to "REQUEST" when absent, since most senders default to that.
    pub method: String,
    pub uid: String,
    pub summary: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub all_day: bool,
    pub organizer_email: Option<String>,
    pub organizer_cn: Option<String>,
    pub attendees: Vec<ParsedAttendee>,
}

/// PURPOSE: One ATTENDEE line from the VEVENT.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedAttendee {
    pub email: String,
    pub cn: Option<String>,
    /// Raw PARTSTAT value (`ACCEPTED`, `DECLINED`, `NEEDS-ACTION`, …) when present.
    pub partstat: Option<String>,
}

/// PURPOSE: Locate a `text/calendar` MIME part anywhere in a parsed email.
/// Returns the part's decoded body when found.
/// CONSTRAINTS: Recurses into `multipart/*`. Stops at the first calendar part
/// since iMIP invites carry at most one VCALENDAR.
pub fn find_calendar_part(mail: &ParsedMail) -> Option<String> {
    if mail.ctype.mimetype.eq_ignore_ascii_case("text/calendar") {
        return mail.get_body().ok();
    }
    for part in &mail.subparts {
        if let Some(body) = find_calendar_part(part) {
            return Some(body);
        }
    }
    None
}

/// PURPOSE: Convenience wrapper: parse raw RFC822 bytes, find the calendar
/// part, decode it.
/// CONSTRAINTS: Returns Err if the message can't be parsed or contains no
/// calendar part.
pub fn parse_imip_from_email(raw: &[u8]) -> Result<ParsedInvite, String> {
    let mail = mailparse::parse_mail(raw).map_err(|e| format!("mailparse failed: {e}"))?;
    let ics = find_calendar_part(&mail)
        .ok_or_else(|| "Message contains no text/calendar part".to_string())?;
    parse_ics(&ics)
}

/// PURPOSE: Parse a chunk of iCalendar text into a `ParsedInvite`.
/// CONSTRAINTS: Looks for the first VEVENT inside the VCALENDAR. Folded
/// lines (CRLF + space/tab) are unfolded first, per RFC 5545 §3.1.
/// EXTERNAL: Public so handlers and tests can drive it directly.
pub fn parse_ics(ics_text: &str) -> Result<ParsedInvite, String> {
    let unfolded = unfold_lines(ics_text);

    // NOTE: Track METHOD at the VCALENDAR level — it sits outside VEVENT.
    let mut method = "REQUEST".to_string();
    let mut in_vevent = false;
    let mut vevent_lines: Vec<&str> = Vec::new();

    for line in unfolded.lines() {
        let trimmed = line.trim_end_matches('\r');
        let upper = trimmed.to_ascii_uppercase();
        if upper.starts_with("METHOD:") && !in_vevent {
            method = trimmed[7..].trim().to_string();
            continue;
        }
        if upper == "BEGIN:VEVENT" {
            in_vevent = true;
            continue;
        }
        if upper == "END:VEVENT" {
            // NOTE: First VEVENT wins — recurring instances are out of scope here.
            // Setting in_vevent back to false is unnecessary because we break, but
            // keep the structure clear for any future per-VEVENT loop.
            let _ = in_vevent;
            break;
        }
        if in_vevent {
            vevent_lines.push(trimmed);
        }
    }

    if vevent_lines.is_empty() {
        return Err("No VEVENT block found in calendar payload".to_string());
    }

    let mut uid: Option<String> = None;
    let mut summary: Option<String> = None;
    let mut description: Option<String> = None;
    let mut location: Option<String> = None;
    let mut dtstart: Option<(DateTime<Utc>, bool)> = None;
    let mut dtend: Option<(DateTime<Utc>, bool)> = None;
    let mut organizer_email: Option<String> = None;
    let mut organizer_cn: Option<String> = None;
    let mut attendees: Vec<ParsedAttendee> = Vec::new();

    for line in vevent_lines {
        let (name_with_params, value) = match line.split_once(':') {
            Some(pair) => pair,
            None => continue,
        };
        let mut parts = name_with_params.split(';');
        let prop = parts.next().unwrap_or("").trim().to_ascii_uppercase();
        let params: Vec<&str> = parts.collect();

        match prop.as_str() {
            "UID" => uid = Some(value.trim().to_string()),
            "SUMMARY" => summary = Some(unescape_text(value)),
            "DESCRIPTION" => description = Some(unescape_text(value)),
            "LOCATION" => location = Some(unescape_text(value)),
            "DTSTART" => {
                dtstart = Some(parse_ical_datetime(&params, value)?);
            }
            "DTEND" => {
                dtend = Some(parse_ical_datetime(&params, value)?);
            }
            "ORGANIZER" => {
                organizer_email = extract_mailto(value);
                organizer_cn = extract_cn(&params);
            }
            "ATTENDEE" => {
                if let Some(email) = extract_mailto(value) {
                    attendees.push(ParsedAttendee {
                        email,
                        cn: extract_cn(&params),
                        partstat: extract_param(&params, "PARTSTAT"),
                    });
                }
            }
            _ => {}
        }
    }

    let uid = uid.ok_or_else(|| "VEVENT missing required UID property".to_string())?;
    let summary = summary.unwrap_or_else(|| "(no title)".to_string());
    let (start_time, start_all_day) =
        dtstart.ok_or_else(|| "VEVENT missing required DTSTART property".to_string())?;
    let (end_time, end_all_day) = match dtend {
        Some(v) => v,
        None => {
            // RFC 5545 §3.6.1: when DTEND is absent, the event is instantaneous
            // (single point in time). Use start == end so downstream upsert
            // doesn't violate the application's `end > start` invariant — the
            // handler will still reject equal times.
            (start_time, start_all_day)
        }
    };

    Ok(ParsedInvite {
        method,
        uid,
        summary,
        description,
        location,
        start_time,
        end_time,
        all_day: start_all_day || end_all_day,
        organizer_email,
        organizer_cn,
        attendees,
    })
}

/// PURPOSE: Unfold RFC 5545 line continuations — a CRLF followed by a single
/// space or tab is part of the previous logical line.
fn unfold_lines(input: &str) -> String {
    // NOTE: We normalise to LF first so the unfolding logic doesn't need
    // to know which line ending the sender used.
    let normalised = input.replace("\r\n", "\n");
    let mut out = String::with_capacity(normalised.len());
    let mut chars = normalised.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\n' {
            if matches!(chars.peek(), Some(' ') | Some('\t')) {
                // Skip the leading whitespace of the continuation.
                chars.next();
                continue;
            }
            out.push('\n');
        } else {
            out.push(ch);
        }
    }
    out
}

/// PURPOSE: Unescape RFC 5545 TEXT values — `\n` → newline, `\,` → comma,
/// `\;` → semicolon, `\\` → backslash.
fn unescape_text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') | Some('N') => out.push('\n'),
                Some(',') => out.push(','),
                Some(';') => out.push(';'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

/// PURPOSE: Parse a DTSTART/DTEND value into a UTC instant + an all_day flag.
/// CONSTRAINTS: Accepts three shapes from RFC 5545 §3.3.5:
///   * `YYYYMMDDTHHMMSSZ` (UTC instant)
///   * `YYYYMMDDTHHMMSS`  (floating local — treated as UTC since we don't
///     ship a tz database here; close enough for the accept flow)
///   * `YYYYMMDD` with `VALUE=DATE` parameter (all-day)
fn parse_ical_datetime(params: &[&str], value: &str) -> Result<(DateTime<Utc>, bool), String> {
    let value = value.trim();
    let is_date_only = params
        .iter()
        .any(|p| p.trim().eq_ignore_ascii_case("VALUE=DATE"));

    if is_date_only || (value.len() == 8 && !value.contains('T')) {
        let date = NaiveDate::parse_from_str(value, "%Y%m%d")
            .map_err(|e| format!("Invalid date '{value}': {e}"))?;
        let dt = date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| "Invalid time-of-day for all-day event".to_string())?;
        return Ok((Utc.from_utc_datetime(&dt), true));
    }

    let (stripped, _is_utc) = match value.strip_suffix('Z') {
        Some(s) => (s, true),
        None => (value, false),
    };

    let naive = NaiveDateTime::parse_from_str(stripped, "%Y%m%dT%H%M%S")
        .map_err(|e| format!("Invalid datetime '{value}': {e}"))?;
    Ok((Utc.from_utc_datetime(&naive), false))
}

/// PURPOSE: Pull the email address out of a `mailto:` URI value. Returns
/// None for non-mailto values (URN attendees, room calendars, etc).
fn extract_mailto(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let lower = trimmed.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("mailto:") {
        // NOTE: lowercase only the scheme; preserve the original casing of
        // the local-part so `Bob@example.com` round-trips intact.
        let original_rest = &trimmed[trimmed.len() - rest.len()..];
        Some(original_rest.to_string())
    } else {
        None
    }
}

/// PURPOSE: Extract the CN parameter value, stripping surrounding quotes if
/// the sender wrapped a name with spaces in double quotes.
fn extract_cn(params: &[&str]) -> Option<String> {
    extract_param(params, "CN").map(|v| v.trim_matches('"').to_string())
}

/// PURPOSE: Generic case-insensitive parameter lookup. Returns the value
/// portion of the first `NAME=VALUE` pair where NAME matches.
fn extract_param(params: &[&str], name: &str) -> Option<String> {
    for p in params {
        let p = p.trim();
        if let Some((k, v)) = p.split_once('=') {
            if k.trim().eq_ignore_ascii_case(name) {
                return Some(v.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    const SAMPLE_REQUEST: &str = "BEGIN:VCALENDAR\r\n\
VERSION:2.0\r\n\
PRODID:-//Example Corp//Test//EN\r\n\
METHOD:REQUEST\r\n\
BEGIN:VEVENT\r\n\
UID:abc-123@example.com\r\n\
DTSTAMP:20260420T080000Z\r\n\
DTSTART:20260420T100000Z\r\n\
DTEND:20260420T110000Z\r\n\
SUMMARY:Project sync\r\n\
DESCRIPTION:Weekly sync\\, with the whole team\\nAgenda below.\r\n\
LOCATION:Room 42\r\n\
ORGANIZER;CN=Alice Organizer:mailto:alice@example.com\r\n\
ATTENDEE;CN=Bob;RSVP=TRUE;PARTSTAT=NEEDS-ACTION:mailto:bob@example.com\r\n\
ATTENDEE;CN=\"Carol Smith\";PARTSTAT=ACCEPTED:mailto:carol@example.com\r\n\
END:VEVENT\r\n\
END:VCALENDAR\r\n";

    #[test]
    fn parse_ics_extracts_core_fields() {
        let parsed = parse_ics(SAMPLE_REQUEST).expect("parse should succeed");
        assert_eq!(parsed.method, "REQUEST");
        assert_eq!(parsed.uid, "abc-123@example.com");
        assert_eq!(parsed.summary, "Project sync");
        assert_eq!(
            parsed.description.as_deref(),
            Some("Weekly sync, with the whole team\nAgenda below.")
        );
        assert_eq!(parsed.location.as_deref(), Some("Room 42"));
        assert!(!parsed.all_day);
    }

    #[test]
    fn parse_ics_extracts_dtstart_dtend_in_utc() {
        let parsed = parse_ics(SAMPLE_REQUEST).unwrap();
        assert_eq!(parsed.start_time.year(), 2026);
        assert_eq!(parsed.start_time.month(), 4);
        assert_eq!(parsed.start_time.day(), 20);
        assert_eq!(parsed.start_time.hour_minute_second(), (10, 0, 0));
        assert_eq!(parsed.end_time.hour_minute_second(), (11, 0, 0));
    }

    // NOTE: helper trait to keep the assertion above readable.
    trait Hms {
        fn hour_minute_second(&self) -> (u32, u32, u32);
    }
    impl Hms for DateTime<Utc> {
        fn hour_minute_second(&self) -> (u32, u32, u32) {
            use chrono::Timelike;
            (self.hour(), self.minute(), self.second())
        }
    }

    #[test]
    fn parse_ics_extracts_organizer_and_attendees() {
        let parsed = parse_ics(SAMPLE_REQUEST).unwrap();
        assert_eq!(parsed.organizer_email.as_deref(), Some("alice@example.com"));
        assert_eq!(parsed.organizer_cn.as_deref(), Some("Alice Organizer"));
        assert_eq!(parsed.attendees.len(), 2);
        assert_eq!(parsed.attendees[0].email, "bob@example.com");
        assert_eq!(parsed.attendees[0].cn.as_deref(), Some("Bob"));
        assert_eq!(parsed.attendees[0].partstat.as_deref(), Some("NEEDS-ACTION"));
        assert_eq!(parsed.attendees[1].email, "carol@example.com");
        assert_eq!(parsed.attendees[1].cn.as_deref(), Some("Carol Smith"));
        assert_eq!(parsed.attendees[1].partstat.as_deref(), Some("ACCEPTED"));
    }

    #[test]
    fn parse_ics_handles_all_day_event() {
        let all_day = "BEGIN:VCALENDAR\r\n\
VERSION:2.0\r\n\
METHOD:REQUEST\r\n\
BEGIN:VEVENT\r\n\
UID:all-day@example.com\r\n\
DTSTART;VALUE=DATE:20260420\r\n\
DTEND;VALUE=DATE:20260421\r\n\
SUMMARY:Offsite day\r\n\
END:VEVENT\r\n\
END:VCALENDAR\r\n";
        let parsed = parse_ics(all_day).unwrap();
        assert!(parsed.all_day);
        assert_eq!(parsed.start_time.day(), 20);
        assert_eq!(parsed.end_time.day(), 21);
    }

    #[test]
    fn parse_ics_handles_folded_lines() {
        // RFC 5545 §3.1: long lines are folded with CRLF + space.
        let folded = "BEGIN:VCALENDAR\r\n\
VERSION:2.0\r\n\
METHOD:REQUEST\r\n\
BEGIN:VEVENT\r\n\
UID:folded@example.com\r\n\
DTSTART:20260420T100000Z\r\n\
DTEND:20260420T110000Z\r\n\
SUMMARY:This is a very long summa\r\n ry that got folded mid-word\r\n\
END:VEVENT\r\n\
END:VCALENDAR\r\n";
        let parsed = parse_ics(folded).unwrap();
        assert_eq!(parsed.summary, "This is a very long summary that got folded mid-word");
    }

    #[test]
    fn parse_ics_rejects_missing_uid() {
        let bad = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nDTSTART:20260420T100000Z\r\nDTEND:20260420T110000Z\r\nSUMMARY:No UID\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let err = parse_ics(bad).unwrap_err();
        assert!(err.contains("UID"));
    }

    #[test]
    fn parse_ics_rejects_missing_dtstart() {
        let bad = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:x@y\r\nSUMMARY:No start\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let err = parse_ics(bad).unwrap_err();
        assert!(err.contains("DTSTART"));
    }

    #[test]
    fn parse_ics_rejects_empty_vevent_section() {
        let bad = "BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n";
        let err = parse_ics(bad).unwrap_err();
        assert!(err.contains("No VEVENT"));
    }

    #[test]
    fn parse_ics_method_defaults_to_request() {
        // Some senders (older Outlook variants) ship VEVENTs without a top-level METHOD.
        let no_method = "BEGIN:VCALENDAR\r\n\
BEGIN:VEVENT\r\n\
UID:nm@example.com\r\n\
DTSTART:20260420T100000Z\r\n\
DTEND:20260420T110000Z\r\n\
SUMMARY:No method\r\n\
END:VEVENT\r\n\
END:VCALENDAR\r\n";
        let parsed = parse_ics(no_method).unwrap();
        assert_eq!(parsed.method, "REQUEST");
    }

    #[test]
    fn parse_ics_method_reply_round_trip() {
        let reply = "BEGIN:VCALENDAR\r\nMETHOD:REPLY\r\nBEGIN:VEVENT\r\nUID:r@x\r\nDTSTART:20260420T100000Z\r\nDTEND:20260420T110000Z\r\nSUMMARY:Re: meeting\r\nATTENDEE;PARTSTAT=ACCEPTED:mailto:bob@example.com\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let parsed = parse_ics(reply).unwrap();
        assert_eq!(parsed.method, "REPLY");
        assert_eq!(parsed.attendees[0].partstat.as_deref(), Some("ACCEPTED"));
    }

    #[test]
    fn parse_ics_ignores_attendee_without_mailto() {
        // Room resource attendees often use a non-mailto URN we don't support.
        let with_room = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:rr@x\r\nDTSTART:20260420T100000Z\r\nDTEND:20260420T110000Z\r\nSUMMARY:T\r\nATTENDEE:urn:resource:room42\r\nATTENDEE:mailto:bob@example.com\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let parsed = parse_ics(with_room).unwrap();
        assert_eq!(parsed.attendees.len(), 1);
        assert_eq!(parsed.attendees[0].email, "bob@example.com");
    }

    #[test]
    fn parse_ics_uses_dtstart_when_dtend_missing() {
        let no_end = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:p@x\r\nDTSTART:20260420T100000Z\r\nSUMMARY:Point-in-time\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let parsed = parse_ics(no_end).unwrap();
        assert_eq!(parsed.start_time, parsed.end_time);
    }

    #[test]
    fn find_calendar_part_locates_text_calendar_subpart() {
        let raw = b"From: a@b.com\r\nTo: c@d.com\r\nSubject: Invite\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=BOUND\r\n\r\n--BOUND\r\nContent-Type: text/plain\r\n\r\nPlease join.\r\n--BOUND\r\nContent-Type: text/calendar; method=REQUEST; charset=UTF-8\r\n\r\nBEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:mp@x\r\nDTSTART:20260420T100000Z\r\nDTEND:20260420T110000Z\r\nSUMMARY:Multipart invite\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n--BOUND--\r\n";
        let mail = mailparse::parse_mail(raw).unwrap();
        let ics = find_calendar_part(&mail).expect("calendar part should be found");
        assert!(ics.contains("BEGIN:VCALENDAR"));
        let parsed = parse_ics(&ics).unwrap();
        assert_eq!(parsed.uid, "mp@x");
        assert_eq!(parsed.summary, "Multipart invite");
    }

    #[test]
    fn parse_imip_from_email_end_to_end() {
        let raw = b"From: alice@example.com\r\nTo: bob@example.com\r\nSubject: Invite\r\nMIME-Version: 1.0\r\nContent-Type: text/calendar; method=REQUEST; charset=UTF-8\r\n\r\nBEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:e2e@example.com\r\nDTSTART:20260420T100000Z\r\nDTEND:20260420T110000Z\r\nSUMMARY:E2E invite\r\nORGANIZER:mailto:alice@example.com\r\nATTENDEE:mailto:bob@example.com\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let parsed = parse_imip_from_email(raw).unwrap();
        assert_eq!(parsed.uid, "e2e@example.com");
        assert_eq!(parsed.organizer_email.as_deref(), Some("alice@example.com"));
    }

    #[test]
    fn parse_imip_from_email_errors_when_no_calendar_part() {
        let raw = b"From: a@b.com\r\nSubject: hi\r\n\r\nNo calendar here.";
        let err = parse_imip_from_email(raw).unwrap_err();
        assert!(err.contains("text/calendar"));
    }

    #[test]
    fn unescape_text_decodes_all_escapes() {
        assert_eq!(unescape_text("a\\nb"), "a\nb");
        assert_eq!(unescape_text("a\\,b"), "a,b");
        assert_eq!(unescape_text("a\\;b"), "a;b");
        assert_eq!(unescape_text("a\\\\b"), "a\\b");
        // Trailing backslash with no escape char — keep verbatim.
        assert_eq!(unescape_text("a\\"), "a\\");
    }

    #[test]
    fn extract_mailto_preserves_local_part_casing() {
        assert_eq!(extract_mailto("MAILTO:Bob@Example.com").as_deref(), Some("Bob@Example.com"));
        assert_eq!(extract_mailto("mailto:bob@example.com").as_deref(), Some("bob@example.com"));
        assert_eq!(extract_mailto("urn:room:42"), None);
    }

    #[test]
    fn extract_cn_strips_surrounding_quotes() {
        let params = vec!["CN=\"Carol Smith\"", "PARTSTAT=ACCEPTED"];
        assert_eq!(extract_cn(&params).as_deref(), Some("Carol Smith"));
    }
}
