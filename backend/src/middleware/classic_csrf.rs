// Added (TMAIL-358): CSRF middleware for the `/classic` no-JS surface.
//
// WHY THIS EXISTS
// ---------------
// The `/classic` UI cannot rely on the SPA's CORS + Bearer-token model for
// CSRF protection — every form posts straight from a cookie-authenticated
// browser session. The cookie set by `classic_session_middleware` is already
// `SameSite=Lax` (layer 1 defence; blocks cross-origin top-level POSTs),
// but per OWASP cheat-sheet guidance we add a per-session synchroniser
// token as layer 2:
//
//   * The `classic_sessions.csrf_token` column (TMAIL-357) carries 32 random
//     bytes, URL-safe base64. Generated on session create.
//   * Every Classic UI form template renders the token as a hidden
//     `<input name="_csrf" value="…">` field (handler-side template render —
//     not this middleware's job).
//   * This middleware runs on every state-changing request under `/classic/`
//     and refuses anything where the submitted `_csrf` doesn't byte-equal
//     `session.csrf_token` (constant-time compare).
//
// REJECTION PATH
// --------------
// On any rejection (missing session, missing `_csrf`, mismatched value,
// unsupported content type) the response is a 403 HTML page that renders
// the original `retry_path` so the user can reload the form and try again.
// JSON envelopes are reserved for `/api/*`.
//
// METHOD SCOPE
// ------------
// GET / HEAD / OPTIONS / TRACE / CONNECT are *safe methods* per RFC 9110 §9.2.1
// (no server state change) — they pass through untouched. Only POST / PUT /
// PATCH / DELETE are validated. This matches OWASP guidance verbatim.
//
// CONTENT-TYPE SCOPE
// ------------------
//   * `application/x-www-form-urlencoded`: fully validated here (covers
//     login, logout, delete, move, flag, settings — the bulk of Classic UI
//     forms).
//   * `multipart/form-data`: this middleware extracts the `_csrf` part by a
//     minimal RFC-7578 boundary scan over the buffered body. The compose
//     form (TMAIL-364) is the only multipart user; total request size is
//     bounded by axum's body-limit layer so the buffer cost is acceptable.
//   * Any other content type on a state-changing method (e.g. `application/
//     json` from a misrouted SPA call) is rejected — there's no legitimate
//     non-form POST on the Classic UI surface.
//
// COMPOSITION
// -----------
// Wire AFTER `classic_session_middleware` so the `ClassicSession` row is in
// `req.extensions()` when this layer runs. `handlers::classic::router()`
// gains an `authenticated_router()` sibling that stacks both layers; new
// `/classic/...` POST routes mount into that router.

use axum::{
    body::{to_bytes, Body, Bytes},
    extract::Request,
    http::{header, Method},
    middleware::Next,
    response::Response,
};

use crate::error::AppError;
use crate::handlers::classic::{render_csrf_error_response, CSRF_FIELD_NAME};
use crate::models::classic_session::ClassicSession;

/// Maximum body size this middleware will buffer to find the `_csrf` field.
///
/// Form-urlencoded Classic UI POSTs (login, logout, delete, move, flag,
/// settings) are all <1 KB. The compose form (multipart) is capped at 25 MB
/// by spec — we allow a small safety margin so an oversized attachment 413s
/// in a dedicated layer rather than 403'ing here as "CSRF body too big".
const MAX_BODY_BYTES: usize = 32 * 1024 * 1024; // 32 MB

/// Constant-time byte-equality. Returns `false` for any length mismatch
/// (length is non-secret in this context — the CSRF token is fixed 43 chars).
///
/// Hand-rolled to avoid pulling in `subtle` just for one comparison; the
/// same shape is used in `classic_session::verify_signature`.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Public validator — callers (this middleware AND any handler that does its
/// own multipart parsing) share one comparison so a constant-time bug fix
/// fans out to every caller.
pub fn validate_csrf_token(submitted: &str, expected: &str) -> bool {
    constant_time_eq(submitted.as_bytes(), expected.as_bytes())
}

/// Lower-case helper for content-type prefix matching that's tolerant of the
/// `; charset=…; boundary=…` parameter tail every browser appends.
fn content_type_prefix<'a>(headers: &'a axum::http::HeaderMap) -> Option<&'a str> {
    headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok())
}

/// Hand-parse a `application/x-www-form-urlencoded` body for the `_csrf`
/// value. Stops at the first match — duplicate fields don't matter because
/// `validate_csrf_token` always uses the FIRST occurrence and `serde_urlencoded`
/// also picks the first. Returns `None` if the field is absent or empty.
///
/// We URL-decode the value via `urlencoding::decode` (already in the workspace
/// — see Cargo.toml). The token alphabet is URL-safe base64 (no encoding
/// needed) but a malicious client could percent-encode it and we still want
/// to validate.
fn extract_form_urlencoded_csrf(body: &[u8]) -> Option<String> {
    let body_str = std::str::from_utf8(body).ok()?;
    for pair in body_str.split('&') {
        // A field with no `=` is malformed for our purposes — skip it rather
        // than aborting the whole scan (an attacker could otherwise prepend
        // a bare token to short-circuit `_csrf` discovery).
        let Some((name, value)) = pair.split_once('=') else {
            continue;
        };
        // Form-urlencoded uses `+` for space; CSRF tokens never contain
        // those but be precise so other fields don't trip a future
        // additional check. Bind the `replace()` result to a local so the
        // `&str` borrow used by `urlencoding::decode` outlives the call.
        let name_spaced = name.replace('+', " ");
        let Ok(decoded_name) = urlencoding::decode(&name_spaced) else {
            continue;
        };
        if decoded_name != CSRF_FIELD_NAME {
            continue;
        }
        let value_spaced = value.replace('+', " ");
        let Ok(decoded_value) = urlencoding::decode(&value_spaced) else {
            return None;
        };
        if decoded_value.is_empty() {
            return None;
        }
        return Some(decoded_value.into_owned());
    }
    None
}

/// Minimal RFC-7578 multipart scan that returns the value of the first part
/// whose `Content-Disposition: form-data; name="_csrf"` header matches.
///
/// This is intentionally narrow: it does NOT do nested multipart, does NOT
/// honour Content-Transfer-Encoding (the spec discourages encoded transfer
/// for `_csrf`), and does NOT support filename parts (CSRF is never a file).
/// Anything more elaborate than the templates we render is rejected as
/// "Missing CSRF token" which is the safe default.
///
/// `boundary` is the value of the `boundary=` directive on the request's
/// Content-Type header (RFC 2046 §5.1.1).
fn extract_multipart_csrf(body: &[u8], boundary: &str) -> Option<String> {
    // Boundary delimiters per RFC 7578 §4.1: `--<boundary>` between parts
    // and `--<boundary>--` at the end.
    let delim = format!("--{boundary}").into_bytes();
    // Search for each delimiter, then locate the first part whose headers
    // include `name="_csrf"`. Bytes-level scan avoids paying UTF-8 validation
    // on the entire (possibly binary) body.
    let mut cursor = 0usize;
    while cursor < body.len() {
        let Some(start) = find_subslice(&body[cursor..], &delim) else {
            return None;
        };
        // Skip past the delimiter + the trailing CRLF.
        let after_delim = cursor + start + delim.len();
        // End-of-multipart marker `--<boundary>--` — bail out.
        if body
            .get(after_delim..after_delim + 2)
            .map(|b| b == b"--")
            .unwrap_or(false)
        {
            return None;
        }
        // Per RFC 7578 each delimiter is followed by `\r\n` then the part
        // headers, then a `\r\n\r\n`, then the part body, then a `\r\n`
        // before the next delimiter. Find headers block.
        let headers_start = after_delim + if body.get(after_delim..after_delim + 2) == Some(&b"\r\n"[..]) { 2 } else { 0 };
        let Some(headers_end_rel) = find_subslice(&body[headers_start..], b"\r\n\r\n") else {
            return None;
        };
        let headers_end = headers_start + headers_end_rel;
        let headers_slice = &body[headers_start..headers_end];
        // Locate the next delimiter to bound this part's body. `next_delim_at`
        // is RELATIVE to the absolute byte index, not to `body_start`.
        let body_start = headers_end + 4;
        let Some(next_delim_rel) = find_subslice(&body[body_start..], &delim) else {
            return None;
        };
        let part_body_end = body_start + next_delim_rel;
        // The CRLF that precedes the next delimiter is part of the
        // delimiter sequence per RFC 2046, NOT part of the value. Trim it.
        let trimmed_end = part_body_end.saturating_sub(2);
        let part_body = &body[body_start..trimmed_end];

        // Does any header in this part name "_csrf"?
        // Content-Disposition is case-insensitive per RFC 7230 §3.2.
        if part_disposition_names_csrf(headers_slice) {
            // Form-data values for non-file parts are 7-bit ASCII text. The
            // CSRF alphabet is URL-safe base64.
            let val = std::str::from_utf8(part_body).ok()?.trim();
            if val.is_empty() {
                return None;
            }
            return Some(val.to_string());
        }

        // Move past this part for the next iteration.
        cursor = part_body_end;
    }
    None
}

/// Case-insensitive check that a `\r\n`-separated header block contains a
/// `Content-Disposition: form-data; name="_csrf"`-style line.
fn part_disposition_names_csrf(headers: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(headers) else {
        return false;
    };
    for line in text.split("\r\n") {
        let lower = line.to_ascii_lowercase();
        if !lower.starts_with("content-disposition:") {
            continue;
        }
        // Match both `name="_csrf"` and the (RFC-tolerant) unquoted form.
        if lower.contains(&format!("name=\"{CSRF_FIELD_NAME}\""))
            || lower.contains(&format!("name={CSRF_FIELD_NAME}"))
        {
            return true;
        }
    }
    false
}

/// Pure-byte substring search. The standard library lacks a stable `[u8]::find`
/// for non-trivial needles; this is the textbook two-pointer scan, which is
/// O(n·m) but fine for the small needles (boundary string, "\r\n\r\n") used
/// here. `memchr` would be faster but pulling in a dep for two callers is
/// not worth it.
fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Pull the `boundary=` directive out of a multipart Content-Type header. The
/// boundary may be quoted or unquoted; both forms ship in the wild.
fn parse_multipart_boundary(content_type: &str) -> Option<String> {
    for part in content_type.split(';').map(str::trim) {
        let lower = part.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("boundary=") {
            // Find the corresponding slice in the original (preserves case
            // in the boundary token — boundary chars are mostly punctuation
            // but RFC 2046 allows case).
            let value_start = part.len() - rest.len();
            let mut value = &part[value_start..];
            if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
                value = &value[1..value.len() - 1];
            }
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// The middleware itself. Mount AFTER `classic_session_middleware` so the
/// `ClassicSession` row is in `req.extensions()`.
///
/// On rejection: 403 with an HTML retry-link page (NOT the JSON
/// `AppError::Forbidden` envelope). On success: passes the request through
/// with the buffered body re-attached.
pub async fn classic_csrf_middleware(
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    // Safe methods (GET/HEAD/OPTIONS/TRACE/CONNECT) per RFC 9110 §9.2.1.
    if !matches!(
        req.method(),
        &Method::POST | &Method::PUT | &Method::PATCH | &Method::DELETE
    ) {
        return Ok(next.run(req).await);
    }

    let retry_path = req.uri().path().to_string();

    // The session MUST be in extensions — i.e. classic_session_middleware
    // ran upstream. If not, the request didn't go through the auth stack
    // (programming error OR a stale cookie whose middleware-bounce was
    // swapped out somewhere). Either way, reject — never POST without a
    // verified identity.
    let session_token = req
        .extensions()
        .get::<ClassicSession>()
        .map(|s| s.csrf_token.clone());

    let Some(expected_token) = session_token else {
        // Stale session: classic_session_middleware would have bounced to
        // /classic/login. If we got here it means the session middleware
        // wasn't wired upstream — log loudly and refuse the request so a
        // forgotten layer can't silently bypass CSRF.
        tracing::warn!(
            method = %req.method(),
            path = %retry_path,
            "classic CSRF middleware ran without ClassicSession in extensions — \
             classic_session_middleware likely not wired upstream; refusing request"
        );
        return Ok(render_csrf_error_response(
            "Your session has expired. Sign in again, then resubmit the form.",
            retry_path,
        ));
    };

    // Capture content-type BEFORE we consume the body — peek at headers.
    let content_type = content_type_prefix(req.headers()).unwrap_or("").to_string();
    let content_type_lower = content_type.to_ascii_lowercase();

    // Split into parts so we can re-stream the body to the handler after
    // validation. `to_bytes` caps the buffer at MAX_BODY_BYTES.
    let (parts, body) = req.into_parts();
    let body_bytes: Bytes = match to_bytes(body, MAX_BODY_BYTES).await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(
                error = ?e,
                path = %retry_path,
                "classic CSRF middleware: failed to buffer request body"
            );
            return Ok(render_csrf_error_response(
                "We couldn't read the form data. Reload the page and try again.",
                retry_path,
            ));
        }
    };

    let submitted = if content_type_lower.starts_with("application/x-www-form-urlencoded") {
        extract_form_urlencoded_csrf(&body_bytes)
    } else if content_type_lower.starts_with("multipart/form-data") {
        match parse_multipart_boundary(&content_type) {
            Some(b) => extract_multipart_csrf(&body_bytes, &b),
            None => {
                return Ok(render_csrf_error_response(
                    "Form submission was malformed (missing multipart boundary).",
                    retry_path,
                ));
            }
        }
    } else {
        // Any other content type on a state-changing method is illegal for
        // the Classic UI. No JSON API hits `/classic/`.
        return Ok(render_csrf_error_response(
            "This page only accepts standard form submissions.",
            retry_path,
        ));
    };

    let Some(submitted) = submitted else {
        return Ok(render_csrf_error_response(
            "Form submission was missing its security token. Reload the page and try again.",
            retry_path,
        ));
    };

    if !validate_csrf_token(&submitted, &expected_token) {
        // Don't echo either token into the log — they're per-session
        // secrets equivalent in sensitivity to a session cookie.
        tracing::warn!(
            method = %parts.method,
            path = %retry_path,
            "classic CSRF middleware: token mismatch — possible CSRF attempt \
             or stale browser cache"
        );
        return Ok(render_csrf_error_response(
            "The security token didn't match. Reload the page and try again.",
            retry_path,
        ));
    }

    // Re-attach the buffered body and continue down the stack.
    let req = Request::from_parts(parts, Body::from(body_bytes));
    Ok(next.run(req).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ----- constant_time_eq + validate_csrf_token -----

    #[test]
    fn constant_time_eq_accepts_identical_bytes() {
        assert!(constant_time_eq(b"hello", b"hello"));
    }

    #[test]
    fn constant_time_eq_rejects_different_bytes() {
        assert!(!constant_time_eq(b"hello", b"world"));
    }

    #[test]
    fn constant_time_eq_rejects_length_mismatch() {
        // Length mismatch returns false WITHOUT degenerating into a panic
        // from `zip()` truncating one side.
        assert!(!constant_time_eq(b"abc", b"abcd"));
        assert!(!constant_time_eq(b"abcd", b"abc"));
    }

    #[test]
    fn constant_time_eq_handles_empty() {
        assert!(constant_time_eq(b"", b""));
        assert!(!constant_time_eq(b"", b"x"));
    }

    #[test]
    fn validate_csrf_token_round_trips_a_43_char_base64url_token() {
        // The shape every real token has, from auth::generate_csrf_token().
        let t = "abcd1234EFGH5678ijkl-_mn9012opqrSTUV3456wxyz";
        assert!(validate_csrf_token(t, t));
        // One byte different rejects.
        let mut bad = t.to_string();
        bad.replace_range(0..1, "Z");
        assert!(!validate_csrf_token(&bad, t));
    }

    // ----- extract_form_urlencoded_csrf -----

    #[test]
    fn url_encoded_finds_csrf_at_start_of_body() {
        let body = b"_csrf=tokenvalue&email=x%40y.com&password=hunter2";
        assert_eq!(
            extract_form_urlencoded_csrf(body).as_deref(),
            Some("tokenvalue")
        );
    }

    #[test]
    fn url_encoded_finds_csrf_in_middle_of_body() {
        let body = b"email=foo%40bar.com&_csrf=tokenvalue&password=hunter2";
        assert_eq!(
            extract_form_urlencoded_csrf(body).as_deref(),
            Some("tokenvalue")
        );
    }

    #[test]
    fn url_encoded_finds_csrf_at_end_of_body() {
        let body = b"email=x%40y.com&password=hunter2&_csrf=tokenvalue";
        assert_eq!(
            extract_form_urlencoded_csrf(body).as_deref(),
            Some("tokenvalue")
        );
    }

    #[test]
    fn url_encoded_returns_none_when_csrf_absent() {
        let body = b"email=x%40y.com&password=hunter2";
        assert_eq!(extract_form_urlencoded_csrf(body), None);
    }

    #[test]
    fn url_encoded_returns_none_for_empty_csrf_value() {
        // A blank `_csrf=` field is treated as missing (not as a valid empty
        // token) so an attacker can't strip the value and still pass length-1
        // matching.
        let body = b"_csrf=&email=x%40y.com";
        assert_eq!(extract_form_urlencoded_csrf(body), None);
    }

    #[test]
    fn url_encoded_handles_percent_encoded_token() {
        // The base64url alphabet doesn't include `+` or `/`, but a hostile
        // client COULD percent-encode chars — we still need to decode for
        // a faithful constant-time compare upstream.
        // %2D = `-`, %5F = `_`
        let body = b"_csrf=tok%2Den%5Fvalue&email=x%40y.com";
        assert_eq!(
            extract_form_urlencoded_csrf(body).as_deref(),
            Some("tok-en_value")
        );
    }

    #[test]
    fn url_encoded_returns_none_on_non_utf8_body() {
        // A binary body on a form-urlencoded content type is malformed.
        // Rejecting at the parser keeps the middleware from mis-validating.
        let body = &[0xFF, 0xFE, 0xFD];
        assert_eq!(extract_form_urlencoded_csrf(body), None);
    }

    // ----- parse_multipart_boundary -----

    #[test]
    fn boundary_parsed_from_typical_content_type() {
        let ct = "multipart/form-data; boundary=----WebKitFormBoundaryABC123";
        assert_eq!(
            parse_multipart_boundary(ct).as_deref(),
            Some("----WebKitFormBoundaryABC123")
        );
    }

    #[test]
    fn boundary_parsed_from_quoted_form() {
        let ct = "multipart/form-data; boundary=\"my boundary\"";
        assert_eq!(parse_multipart_boundary(ct).as_deref(), Some("my boundary"));
    }

    #[test]
    fn boundary_missing_returns_none() {
        let ct = "multipart/form-data";
        assert_eq!(parse_multipart_boundary(ct), None);
    }

    // ----- extract_multipart_csrf -----

    fn build_multipart(boundary: &str, parts: &[(&str, &str)]) -> Vec<u8> {
        let mut body = Vec::new();
        for (name, value) in parts {
            body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            body.extend_from_slice(
                format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
            );
            body.extend_from_slice(value.as_bytes());
            body.extend_from_slice(b"\r\n");
        }
        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        body
    }

    #[test]
    fn multipart_finds_csrf_as_first_part() {
        let body = build_multipart("BOUNDARY", &[("_csrf", "tokenvalue"), ("subject", "hi")]);
        assert_eq!(
            extract_multipart_csrf(&body, "BOUNDARY").as_deref(),
            Some("tokenvalue")
        );
    }

    #[test]
    fn multipart_finds_csrf_after_other_parts() {
        let body = build_multipart(
            "BOUNDARY",
            &[("subject", "hi"), ("_csrf", "tokenvalue"), ("body", "hello")],
        );
        assert_eq!(
            extract_multipart_csrf(&body, "BOUNDARY").as_deref(),
            Some("tokenvalue")
        );
    }

    #[test]
    fn multipart_returns_none_when_csrf_absent() {
        let body = build_multipart("BOUNDARY", &[("subject", "hi"), ("body", "hello")]);
        assert_eq!(extract_multipart_csrf(&body, "BOUNDARY"), None);
    }

    #[test]
    fn multipart_returns_none_on_empty_csrf_value() {
        // Same defence as form-urlencoded: empty value is treated as missing.
        let body = build_multipart("BOUNDARY", &[("_csrf", ""), ("subject", "hi")]);
        assert_eq!(extract_multipart_csrf(&body, "BOUNDARY"), None);
    }

    #[test]
    fn multipart_finds_csrf_with_case_insensitive_content_disposition() {
        // Some clients normalise the header name to lowercase.
        let mut body = Vec::new();
        body.extend_from_slice(b"--B\r\n");
        body.extend_from_slice(b"content-disposition: form-data; name=\"_csrf\"\r\n\r\n");
        body.extend_from_slice(b"tokenvalue\r\n");
        body.extend_from_slice(b"--B--\r\n");
        assert_eq!(
            extract_multipart_csrf(&body, "B").as_deref(),
            Some("tokenvalue")
        );
    }

    // ----- find_subslice + part_disposition_names_csrf -----

    #[test]
    fn find_subslice_returns_first_match() {
        assert_eq!(find_subslice(b"abcdefabc", b"abc"), Some(0));
        assert_eq!(find_subslice(b"xxabcxx", b"abc"), Some(2));
        assert_eq!(find_subslice(b"xxxx", b"abc"), None);
        assert_eq!(find_subslice(b"", b"abc"), None);
        assert_eq!(find_subslice(b"abc", b""), None);
    }

    #[test]
    fn disposition_matches_quoted_and_unquoted_names() {
        let quoted = b"Content-Disposition: form-data; name=\"_csrf\"";
        let unquoted = b"Content-Disposition: form-data; name=_csrf";
        let other = b"Content-Disposition: form-data; name=\"subject\"";
        assert!(part_disposition_names_csrf(quoted));
        assert!(part_disposition_names_csrf(unquoted));
        assert!(!part_disposition_names_csrf(other));
    }
}
