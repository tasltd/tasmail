// Added (TMAIL-266 / TMAIL-127): CalDAV free-busy query service.
//
// Issues a CalDAV `REPORT` with a `<C:free-busy-query>` body against a
// configured CalDAV server (typically from `dav_configurations`) and parses
// the resulting VFREEBUSY response into a flat list of busy windows.
//
// Per RFC 4791 §7.10 the server replies with `Content-Type: text/calendar`
// containing a VCALENDAR object that wraps one VFREEBUSY component. Some
// servers (older SOGo, Bynari) wrap that VCALENDAR inside a WebDAV
// multistatus `<C:calendar-data>` element; this module tolerates both by
// scanning for the `BEGIN:VFREEBUSY` line directly rather than treating the
// response as strict XML.
//
// The module is split into pure parsing/builder helpers (no I/O, easy to
// unit-test) and a thin async wrapper that performs the actual HTTP call.

use chrono::{DateTime, Duration, NaiveDateTime, TimeZone, Utc};

/// A merged busy window `[start, end)` returned to the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BusyWindow {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

/// Build the XML body for a CalDAV `REPORT` request against a calendar
/// collection. The `start`/`end` timestamps are emitted in iCalendar UTC
/// format (`YYYYMMDDTHHMMSSZ`) as required by RFC 4791 §9.9.
pub fn build_freebusy_query_xml(start: DateTime<Utc>, end: DateTime<Utc>) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\" ?>\n\
<C:free-busy-query xmlns:C=\"urn:ietf:params:xml:ns:caldav\">\n  \
<C:time-range start=\"{}\" end=\"{}\"/>\n\
</C:free-busy-query>\n",
        format_icalendar_utc(start),
        format_icalendar_utc(end),
    )
}

fn format_icalendar_utc(dt: DateTime<Utc>) -> String {
    dt.format("%Y%m%dT%H%M%SZ").to_string()
}

/// Parse a CalDAV free-busy REPORT response into busy windows.
///
/// Accepts both the canonical `text/calendar` body (a raw VCALENDAR with a
/// VFREEBUSY child) and the multistatus XML variant where the VCALENDAR is
/// embedded inside a `<C:calendar-data>` element. We do this by scanning the
/// already-unfolded text line-by-line and looking at FREEBUSY entries inside
/// any BEGIN:VFREEBUSY / END:VFREEBUSY block.
///
/// Lines whose `FBTYPE` parameter is `FREE` are skipped (RFC 5545 §3.2.9
/// allows FREE/BUSY/BUSY-UNAVAILABLE/BUSY-TENTATIVE; only the last three
/// represent unavailability).
pub fn parse_freebusy_response(body: &str) -> Result<Vec<BusyWindow>, String> {
    let unfolded = unfold_ical_lines(body);
    let mut out: Vec<BusyWindow> = Vec::new();
    let mut in_vfreebusy = false;
    for raw_line in unfolded.lines() {
        let line = raw_line.trim_end_matches('\r');
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("BEGIN:VFREEBUSY") {
            in_vfreebusy = true;
            continue;
        }
        if trimmed.eq_ignore_ascii_case("END:VFREEBUSY") {
            in_vfreebusy = false;
            continue;
        }
        if !in_vfreebusy {
            continue;
        }
        // Property name is "FREEBUSY" with optional ";PARAM=value" segments
        // before the ":value" body. Anything else inside the block is ignored.
        let (name, params, value) = match split_property(line) {
            Some(triple) => triple,
            None => continue,
        };
        if !name.eq_ignore_ascii_case("FREEBUSY") {
            continue;
        }
        if let Some(fbtype) = extract_param(params, "FBTYPE") {
            if fbtype.eq_ignore_ascii_case("FREE") {
                continue;
            }
        }
        // The FREEBUSY value may contain multiple comma-separated periods.
        for period in value.split(',') {
            let period = period.trim();
            if period.is_empty() {
                continue;
            }
            if let Some(window) = parse_freebusy_period(period) {
                out.push(window);
            }
        }
    }
    Ok(out)
}

/// Issue the CalDAV REPORT and return the parsed busy windows.
///
/// `server_url` should point at a CalDAV calendar collection (the user
/// supplies this in their dav_config). We send `Depth: 1` so the server
/// considers the collection's child resources, matching the behaviour of
/// every CalDAV client we've inspected (Thunderbird/Apple Calendar/iOS).
///
/// Errors are intentionally coarse — the caller logs and downgrades the
/// attendee to "local events only" so a flaky external server can't take
/// down the whole free-busy endpoint.
pub async fn query_caldav_freebusy(
    server_url: &str,
    username: &str,
    password: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<BusyWindow>, String> {
    if end <= start {
        return Err("end must be after start".to_string());
    }
    let body = build_freebusy_query_xml(start, end);
    let method = reqwest::Method::from_bytes(b"REPORT")
        .map_err(|e| format!("invalid HTTP method REPORT: {e}"))?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("HTTP client build failed: {e}"))?;
    let resp = client
        .request(method, server_url)
        .basic_auth(username, Some(password))
        .header(reqwest::header::CONTENT_TYPE, "application/xml; charset=utf-8")
        .header(reqwest::header::ACCEPT, "text/calendar")
        .header("Depth", "1")
        .body(body)
        .send()
        .await
        .map_err(|e| format!("CalDAV request failed: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("CalDAV server returned HTTP {}", status.as_u16()));
    }
    let text = resp
        .text()
        .await
        .map_err(|e| format!("reading CalDAV response body failed: {e}"))?;
    parse_freebusy_response(&text)
}

// ---------------------------------------------------------------------------
// Internal helpers — exposed only inside the crate for unit-testing.
// ---------------------------------------------------------------------------

/// Unfold iCalendar continuation lines per RFC 5545 §3.1: a line that starts
/// with a single space or tab is a continuation of the previous line and
/// must be appended (with the leading whitespace stripped).
fn unfold_ical_lines(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for line in input.split('\n') {
        let stripped = line.trim_end_matches('\r');
        if stripped.starts_with(' ') || stripped.starts_with('\t') {
            // Drop the single leading whitespace byte and append.
            out.push_str(&stripped[1..]);
        } else {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(stripped);
        }
    }
    out
}

/// Split a content line of the form `NAME;PARAM=value:body` into
/// `(name, params, value)`. Returns `None` if there's no colon.
fn split_property(line: &str) -> Option<(&str, &str, &str)> {
    // The colon that separates params from value can also appear inside a
    // quoted parameter value, but the FREEBUSY/CN parameters we care about
    // are never quoted in well-formed output. Use the first colon for
    // simplicity — this matches every CalDAV server we've seen and the
    // parser falls back gracefully on malformed input.
    let colon = line.find(':')?;
    let head = &line[..colon];
    let value = &line[colon + 1..];
    if let Some(semi) = head.find(';') {
        Some((&head[..semi], &head[semi + 1..], value))
    } else {
        Some((head, "", value))
    }
}

fn extract_param<'a>(params: &'a str, name: &str) -> Option<&'a str> {
    if params.is_empty() {
        return None;
    }
    for part in params.split(';') {
        let mut it = part.splitn(2, '=');
        let key = it.next()?.trim();
        let val = it.next()?.trim();
        if key.eq_ignore_ascii_case(name) {
            return Some(val.trim_matches('"'));
        }
    }
    None
}

/// Parse an RFC 5545 period: `START/END` or `START/DURATION` where START/END
/// are iCalendar UTC timestamps and DURATION is an ISO-8601-ish duration.
fn parse_freebusy_period(period: &str) -> Option<BusyWindow> {
    let (start_s, rest) = period.split_once('/')?;
    let start = parse_icalendar_utc(start_s.trim())?;
    let rest = rest.trim();
    if let Some(end) = parse_icalendar_utc(rest) {
        if end <= start {
            return None;
        }
        return Some(BusyWindow { start, end });
    }
    let duration = parse_ical_duration(rest)?;
    if duration <= Duration::zero() {
        return None;
    }
    Some(BusyWindow {
        start,
        end: start + duration,
    })
}

fn parse_icalendar_utc(s: &str) -> Option<DateTime<Utc>> {
    // Strict form: YYYYMMDDTHHMMSSZ (16 chars). Anything else — including
    // floating times or DATE-only values — is not a valid free-busy period
    // boundary per RFC 5545 §3.8.4.7.
    if s.len() != 16 || !s.ends_with('Z') {
        return None;
    }
    let naive = NaiveDateTime::parse_from_str(&s[..15], "%Y%m%dT%H%M%S").ok()?;
    Some(Utc.from_utc_datetime(&naive))
}

/// Minimal RFC 5545 duration parser: handles `P[n]W`, `P[n]D`, and
/// `P[n]DT[n]H[n]M[n]S` / `PT[n]H[n]M[n]S` combos. Negative durations
/// (`-P...`) and fractional seconds are rejected — they don't appear in
/// well-formed FREEBUSY responses.
fn parse_ical_duration(s: &str) -> Option<Duration> {
    let mut chars = s.chars().peekable();
    if chars.next()? != 'P' {
        return None;
    }
    let mut total = Duration::zero();
    let mut buf = String::new();
    let mut in_time = false;
    for ch in chars {
        if ch == 'T' {
            if !buf.is_empty() {
                return None;
            }
            in_time = true;
            continue;
        }
        if ch.is_ascii_digit() {
            buf.push(ch);
            continue;
        }
        let n: i64 = buf.parse().ok()?;
        buf.clear();
        let inc = match (ch, in_time) {
            ('W', false) => Duration::weeks(n),
            ('D', false) => Duration::days(n),
            ('H', true) => Duration::hours(n),
            ('M', true) => Duration::minutes(n),
            ('S', true) => Duration::seconds(n),
            _ => return None,
        };
        total = total.checked_add(&inc)?;
    }
    if !buf.is_empty() {
        return None;
    }
    Some(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, s).single().unwrap()
    }

    // ---- builder ----------------------------------------------------------

    #[test]
    fn build_freebusy_query_xml_emits_namespace_and_range() {
        let xml = build_freebusy_query_xml(
            utc(2026, 6, 1, 9, 0, 0),
            utc(2026, 6, 1, 17, 0, 0),
        );
        assert!(xml.contains("xmlns:C=\"urn:ietf:params:xml:ns:caldav\""));
        assert!(xml.contains("<C:free-busy-query"));
        assert!(xml.contains("<C:time-range start=\"20260601T090000Z\" end=\"20260601T170000Z\"/>"));
    }

    #[test]
    fn build_freebusy_query_xml_well_formed_prologue() {
        let xml = build_freebusy_query_xml(
            utc(2026, 1, 5, 0, 0, 0),
            utc(2026, 1, 6, 0, 0, 0),
        );
        assert!(xml.starts_with("<?xml version=\"1.0\""));
        assert!(xml.contains("</C:free-busy-query>"));
    }

    // ---- duration parser --------------------------------------------------

    #[test]
    fn parse_duration_hours() {
        assert_eq!(parse_ical_duration("PT1H"), Some(Duration::hours(1)));
    }

    #[test]
    fn parse_duration_hours_minutes_seconds() {
        assert_eq!(
            parse_ical_duration("PT1H30M15S"),
            Some(Duration::hours(1) + Duration::minutes(30) + Duration::seconds(15))
        );
    }

    #[test]
    fn parse_duration_days() {
        assert_eq!(parse_ical_duration("P2D"), Some(Duration::days(2)));
    }

    #[test]
    fn parse_duration_days_and_time() {
        assert_eq!(
            parse_ical_duration("P1DT2H"),
            Some(Duration::days(1) + Duration::hours(2))
        );
    }

    #[test]
    fn parse_duration_weeks() {
        assert_eq!(parse_ical_duration("P1W"), Some(Duration::weeks(1)));
    }

    #[test]
    fn parse_duration_rejects_missing_p() {
        assert_eq!(parse_ical_duration("T1H"), None);
    }

    #[test]
    fn parse_duration_rejects_trailing_digits() {
        assert_eq!(parse_ical_duration("PT1"), None);
    }

    // ---- period parser ----------------------------------------------------

    #[test]
    fn parse_period_with_explicit_end() {
        let w = parse_freebusy_period("20060104T210000Z/20060104T220000Z").unwrap();
        assert_eq!(w.start, utc(2006, 1, 4, 21, 0, 0));
        assert_eq!(w.end, utc(2006, 1, 4, 22, 0, 0));
    }

    #[test]
    fn parse_period_with_duration() {
        let w = parse_freebusy_period("20060104T180000Z/PT1H").unwrap();
        assert_eq!(w.start, utc(2006, 1, 4, 18, 0, 0));
        assert_eq!(w.end, utc(2006, 1, 4, 19, 0, 0));
    }

    #[test]
    fn parse_period_rejects_zero_length() {
        assert!(parse_freebusy_period("20060104T180000Z/20060104T180000Z").is_none());
    }

    #[test]
    fn parse_period_rejects_inverted() {
        assert!(parse_freebusy_period("20060104T190000Z/20060104T180000Z").is_none());
    }

    #[test]
    fn parse_period_rejects_floating_time() {
        // No trailing Z — not a UTC timestamp.
        assert!(parse_freebusy_period("20060104T180000/PT1H").is_none());
    }

    // ---- VFREEBUSY parser -------------------------------------------------

    /// Canonical RFC 4791 §7.10 example response.
    const RFC4791_EXAMPLE: &str = "\
BEGIN:VCALENDAR\r
VERSION:2.0\r
PRODID:-//Example Corp.//CalDAV Client//EN\r
BEGIN:VFREEBUSY\r
DTSTART:20060104T140000Z\r
DTEND:20060105T220000Z\r
DTSTAMP:20060101T120000Z\r
FREEBUSY:20060104T180000Z/PT1H\r
FREEBUSY:20060104T190000Z/PT1H\r
FREEBUSY:20060104T210000Z/20060104T220000Z\r
END:VFREEBUSY\r
END:VCALENDAR\r
";

    #[test]
    fn parse_response_rfc4791_example_returns_three_windows() {
        let windows = parse_freebusy_response(RFC4791_EXAMPLE).unwrap();
        assert_eq!(windows.len(), 3);
        assert_eq!(windows[0].start, utc(2006, 1, 4, 18, 0, 0));
        assert_eq!(windows[0].end, utc(2006, 1, 4, 19, 0, 0));
        assert_eq!(windows[1].start, utc(2006, 1, 4, 19, 0, 0));
        assert_eq!(windows[1].end, utc(2006, 1, 4, 20, 0, 0));
        assert_eq!(windows[2].start, utc(2006, 1, 4, 21, 0, 0));
        assert_eq!(windows[2].end, utc(2006, 1, 4, 22, 0, 0));
    }

    #[test]
    fn parse_response_skips_free_fbtype() {
        let body = "\
BEGIN:VCALENDAR\r
BEGIN:VFREEBUSY\r
FREEBUSY;FBTYPE=FREE:20260601T100000Z/PT1H\r
FREEBUSY;FBTYPE=BUSY:20260601T140000Z/PT1H\r
END:VFREEBUSY\r
END:VCALENDAR\r
";
        let windows = parse_freebusy_response(body).unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].start, utc(2026, 6, 1, 14, 0, 0));
    }

    #[test]
    fn parse_response_accepts_busy_tentative_and_unavailable() {
        let body = "\
BEGIN:VFREEBUSY\r
FREEBUSY;FBTYPE=BUSY-TENTATIVE:20260601T100000Z/PT1H\r
FREEBUSY;FBTYPE=BUSY-UNAVAILABLE:20260601T120000Z/PT30M\r
END:VFREEBUSY\r
";
        let windows = parse_freebusy_response(body).unwrap();
        assert_eq!(windows.len(), 2);
    }

    #[test]
    fn parse_response_handles_comma_separated_periods() {
        let body = "\
BEGIN:VFREEBUSY\r
FREEBUSY:20260601T100000Z/PT1H,20260601T120000Z/20260601T130000Z\r
END:VFREEBUSY\r
";
        let windows = parse_freebusy_response(body).unwrap();
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].end, utc(2026, 6, 1, 11, 0, 0));
        assert_eq!(windows[1].start, utc(2026, 6, 1, 12, 0, 0));
        assert_eq!(windows[1].end, utc(2026, 6, 1, 13, 0, 0));
    }

    #[test]
    fn parse_response_handles_folded_lines() {
        // RFC 5545 line folding — a continuation begins with a single space.
        let body = "BEGIN:VFREEBUSY\r\nFREEBUSY:20260601T100\r\n 000Z/PT1H\r\nEND:VFREEBUSY\r\n";
        let windows = parse_freebusy_response(body).unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].start, utc(2026, 6, 1, 10, 0, 0));
        assert_eq!(windows[0].end, utc(2026, 6, 1, 11, 0, 0));
    }

    #[test]
    fn parse_response_ignores_lines_outside_vfreebusy() {
        let body = "\
BEGIN:VCALENDAR\r
FREEBUSY:20260601T080000Z/PT1H\r
BEGIN:VFREEBUSY\r
FREEBUSY:20260601T100000Z/PT1H\r
END:VFREEBUSY\r
FREEBUSY:20260601T200000Z/PT1H\r
END:VCALENDAR\r
";
        let windows = parse_freebusy_response(body).unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].start, utc(2026, 6, 1, 10, 0, 0));
    }

    #[test]
    fn parse_response_handles_multistatus_wrapped_calendar_data() {
        // Some servers wrap the VFREEBUSY inside a WebDAV multistatus
        // response under a <C:calendar-data> element. The parser just looks
        // for BEGIN:VFREEBUSY anywhere, so the XML wrapper is harmless.
        let body = r#"<?xml version="1.0" encoding="utf-8" ?>
<D:multistatus xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:response>
    <D:propstat>
      <D:prop>
        <C:calendar-data>BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VFREEBUSY
DTSTART:20260601T000000Z
DTEND:20260602T000000Z
FREEBUSY:20260601T140000Z/PT30M
END:VFREEBUSY
END:VCALENDAR</C:calendar-data>
      </D:prop>
    </D:propstat>
  </D:response>
</D:multistatus>
"#;
        let windows = parse_freebusy_response(body).unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].start, utc(2026, 6, 1, 14, 0, 0));
        assert_eq!(windows[0].end, utc(2026, 6, 1, 14, 30, 0));
    }

    #[test]
    fn parse_response_empty_when_no_vfreebusy() {
        let body = "BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n";
        let windows = parse_freebusy_response(body).unwrap();
        assert!(windows.is_empty());
    }

    #[test]
    fn parse_response_lf_only_line_endings() {
        // Not every server emits CRLF — make sure plain LF is handled too.
        let body = "BEGIN:VFREEBUSY\nFREEBUSY:20260601T100000Z/PT1H\nEND:VFREEBUSY\n";
        let windows = parse_freebusy_response(body).unwrap();
        assert_eq!(windows.len(), 1);
    }

    #[test]
    fn parse_response_ignores_other_properties_inside_block() {
        let body = "\
BEGIN:VFREEBUSY\r
DTSTART:20260601T000000Z\r
DTEND:20260602T000000Z\r
DTSTAMP:20260530T120000Z\r
ORGANIZER:mailto:user@example.com\r
ATTENDEE:mailto:other@example.com\r
FREEBUSY:20260601T100000Z/PT1H\r
END:VFREEBUSY\r
";
        let windows = parse_freebusy_response(body).unwrap();
        assert_eq!(windows.len(), 1);
    }

    #[test]
    fn extract_param_handles_quoted_values() {
        assert_eq!(extract_param("FBTYPE=\"BUSY\"", "FBTYPE"), Some("BUSY"));
        assert_eq!(extract_param("FBTYPE=BUSY", "FBTYPE"), Some("BUSY"));
    }

    #[test]
    fn extract_param_is_case_insensitive_in_keys() {
        assert_eq!(extract_param("fbtype=BUSY", "FBTYPE"), Some("BUSY"));
    }

    #[test]
    fn extract_param_returns_none_when_missing() {
        assert_eq!(extract_param("OTHER=value", "FBTYPE"), None);
        assert_eq!(extract_param("", "FBTYPE"), None);
    }

    #[test]
    fn query_caldav_freebusy_rejects_inverted_range() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            query_caldav_freebusy(
                "http://127.0.0.1:1/never",
                "u",
                "p",
                utc(2026, 6, 2, 0, 0, 0),
                utc(2026, 6, 1, 0, 0, 0),
            )
            .await
        });
        assert!(result.is_err());
    }
}
