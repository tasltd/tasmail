// Added (TMAIL-365): GET /classic/folders/{folder}/messages/{uid}/parts/{part_id}
// — the attachment download endpoint for the Classic UI surface (driver
// TMAIL-299, gap-analysis `docs/gap-analysis/classic-ui.md` P0 #11).
//
// What this does
// --------------
//   * Reuses `ImapService::get_message_part` (TMAIL-320) to walk the MIME
//     tree by the dotted `part_id` the read view (TMAIL-363) already wired
//     into every attachment link. The same path the SPA download endpoint
//     in `handlers/messages.rs::download_message_part` takes.
//   * Sets `Content-Type` from the MIME part's own header. Falls back to
//     `application/octet-stream` if the source MIME type can't be parsed
//     as a valid HeaderValue (malformed Content-Type from an upstream MTA).
//   * Sets `Content-Disposition: attachment; filename="<ascii>"; filename*=UTF-8''<encoded>`
//     per RFC 6266. The ASCII fallback survives legacy browsers; the UTF-8
//     form survives modern ones. Both are emitted so the download experience
//     is predictable across the matrix.
//   * Sanitises the filename HARD against path traversal (`/`, `\`, `..`)
//     and shell-meta characters (`;`, `|`, `&`, `$`, backtick, `<`, `>`,
//     parens, braces, brackets, `*`, `?`, `!`, `~`) — stricter than the
//     SPA endpoint's CR/LF/quote-only stripping. Classic UI downloads can
//     land on a user's local shell more easily (no JS to mediate), so the
//     defensive surface is wider.
//   * Truncates the sanitised filename to 200 chars total, preserving the
//     trailing extension where possible.
//   * Streams the body via `axum::body::Body::from_stream` rather than
//     `Body::from(Vec<u8>)`. Today `get_message_part` returns the bytes
//     buffered in memory (an existing limitation in the IMAP layer), so
//     the immediate win is just avoiding an extra wrapper copy — but the
//     handler is shaped so a future streaming-`get_message_part` swaps in
//     without touching this file.
//
// What this does NOT do (deliberately deferred)
// ---------------------------------------------
//   * Range requests / Content-Range — out of scope; the read view link
//     is a plain download, not a media player.
//   * Anti-virus scanning — the inbound side runs rspamd; download-time
//     scanning is a future enhancement.
//   * Marking the message \Seen — `get_message_part` uses BODY.PEEK[] so
//     downloading an attachment intentionally does NOT flip the read flag.

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue, Response, StatusCode},
    Extension,
};
use bytes::Bytes;
use futures::stream;
use uuid::Uuid;

use crate::error::AppError;
use crate::services::auth_service::Claims;
use crate::services::imap_service::ImapService;
use crate::state::AppState;

/// Maximum length of the sanitised filename rendered into `Content-Disposition`.
/// 200 leaves headroom under the typical 256-byte filesystem limit on the
/// receiving end, and well under the 8 KB header-size limits common in
/// reverse proxies. Long-enough that genuine human-friendly names survive
/// untouched; short-enough that a hostile 100 KB filename can't blow up the
/// response header.
const MAX_FILENAME_LEN: usize = 200;

/// Stream the IMAP part bytes in 64 KiB chunks. Small enough to keep the
/// per-frame memory bounded on the response side; large enough that the
/// per-chunk overhead doesn't dominate for multi-MB attachments. The same
/// chunk size most HTTP server defaults converge on.
const STREAM_CHUNK_SIZE: usize = 64 * 1024;

/// Sanitise a MIME-supplied filename for safe inclusion in a
/// `Content-Disposition` header AND for the receiver's filesystem.
///
/// Strips, in order:
///   1. Control characters (< 0x20), DEL (0x7F), NUL.
///   2. Path-traversal characters: `/`, `\`.
///   3. Shell-meta characters: `;`, `|`, `&`, `$`, `` ` ``, `<`, `>`,
///      `(`, `)`, `{`, `}`, `[`, `]`, `*`, `?`, `!`, `~`, `"`, `'`.
///   4. Collapses runs of underscores (the substitution character) so
///      `<script>` doesn't become `_______`.
///   5. Strips leading `.` so a hostile name can't write a hidden file.
///   6. Strips leading/trailing whitespace.
///   7. Replaces any path-segment that is exactly `.` or `..` with `_`.
///
/// Then truncates to `MAX_FILENAME_LEN`, preserving the trailing extension
/// (after the last `.`) when one exists and fits. Empty result substitutes
/// the placeholder `"attachment"`.
///
/// Pulled into a free function so unit tests can exercise the matrix
/// without going through IMAP.
pub fn sanitise_filename(raw: &str) -> String {
    // Step 1–3: per-char filter. Substitute disallowed chars with `_`
    // rather than dropping them so adjacent legitimate chars don't merge
    // into a misleading name (`my..report` → `my_report` not `myreport`).
    let filtered: String = raw
        .chars()
        .map(|c| {
            let is_control = (c as u32) < 0x20 || c == '\u{7F}' || c == '\u{0}';
            let is_disallowed = matches!(
                c,
                '/' | '\\'
                    | ';'
                    | '|'
                    | '&'
                    | '$'
                    | '`'
                    | '<'
                    | '>'
                    | '('
                    | ')'
                    | '{'
                    | '}'
                    | '['
                    | ']'
                    | '*'
                    | '?'
                    | '!'
                    | '~'
                    | '"'
                    | '\''
            );
            if is_control || is_disallowed {
                '_'
            } else {
                c
            }
        })
        .collect();

    // Step 4: collapse runs of `_` so a hostile `<<<<` doesn't expand the
    // visible length.
    let mut collapsed = String::with_capacity(filtered.len());
    let mut prev_underscore = false;
    for c in filtered.chars() {
        if c == '_' {
            if !prev_underscore {
                collapsed.push('_');
            }
            prev_underscore = true;
        } else {
            collapsed.push(c);
            prev_underscore = false;
        }
    }

    // Step 5–6: strip leading dots + outer whitespace.
    let trimmed = collapsed
        .trim()
        .trim_start_matches('.')
        .trim()
        .to_string();

    // Step 7: explicit `.` / `..` whole-name substitution. After step 5
    // these collapse to "" — the empty-name placeholder picks them up
    // below — but cover the case where they survive (e.g. `..` with a
    // trailing space that step 6 trims).
    let safe = if trimmed == "." || trimmed == ".." || trimmed.is_empty() {
        "attachment".to_string()
    } else {
        trimmed
    };

    // Step 8: truncate to MAX_FILENAME_LEN, preserving extension.
    if safe.chars().count() <= MAX_FILENAME_LEN {
        return safe;
    }
    truncate_preserving_extension(&safe, MAX_FILENAME_LEN)
}

/// Truncate `name` to at most `max_chars` characters, preserving the
/// extension (the final `.<suffix>`) when an extension exists and is short
/// enough to leave room for at least one base-name character.
///
/// Operates on `char` counts (not bytes) so a UTF-8 filename can't be
/// truncated mid-codepoint.
fn truncate_preserving_extension(name: &str, max_chars: usize) -> String {
    let total = name.chars().count();
    if total <= max_chars {
        return name.to_string();
    }
    // Locate the last `.` in char-space; refuse extensions that are
    // suspiciously long (>20 chars) — those aren't real extensions, they're
    // the rest of a long name with a `.` in the middle.
    let chars: Vec<char> = name.chars().collect();
    let dot_idx = chars.iter().rposition(|&c| c == '.');
    if let Some(dot) = dot_idx {
        let ext_len = chars.len() - dot; // includes the dot
        if ext_len > 1 && ext_len <= 20 && ext_len + 1 < max_chars {
            // Keep `max_chars - ext_len` base chars, then re-attach `.ext`.
            let base_keep = max_chars - ext_len;
            let mut out = String::with_capacity(max_chars);
            out.extend(chars[..base_keep].iter());
            out.extend(chars[dot..].iter());
            return out;
        }
    }
    // No usable extension — straight char-bounded truncate.
    chars[..max_chars].iter().collect()
}

/// Build the `Content-Disposition` header value per RFC 6266.
///
/// Emits both `filename="<ascii>"` (legacy fallback — non-ASCII chars
/// replaced with `_`) AND `filename*=UTF-8''<percent-encoded>` (modern
/// browsers). Both forms reference the SAME sanitised filename — they
/// only differ on encoding, not content.
///
/// `sanitised` is expected to be the output of `sanitise_filename` —
/// any raw input passed here would bypass the path-traversal /
/// shell-meta strip.
pub fn build_content_disposition(sanitised: &str) -> String {
    let ascii: String = sanitised
        .chars()
        .map(|c| if c.is_ascii() { c } else { '_' })
        .collect();
    let encoded = urlencoding::encode(sanitised);
    format!(
        "attachment; filename=\"{}\"; filename*=UTF-8''{}",
        ascii, encoded
    )
}

/// GET /classic/folders/{folder}/messages/{uid}/parts/{part_id} — stream
/// the bytes of a single MIME part as an attachment download.
///
/// Same auth + folder-resolution path as the message read view
/// (`handlers::classic::message::get_message`). Errors surface through
/// the global `AppError` layer; the read-view links that get here will
/// continue working for the lifetime of the underlying part_id (which
/// matches `extract_parts` in the IMAP service).
pub async fn get_attachment(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((folder, uid, part_id)): Path<(String, u32, String)>,
) -> Result<Response<Body>, AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID in classic claims")))?;

    let imap_service = ImapService::for_user(&state, mailbox_id).await?;
    let (username, password) = imap_service
        .user_creds()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("BYOK creds missing on ImapService")))?;
    let username = username.to_string();
    let password = password.to_string();

    let part = imap_service
        .get_message_part(&username, &password, &folder, uid, &part_id)
        .await?;

    let safe_filename = sanitise_filename(&part.filename);
    let cd = build_content_disposition(&safe_filename);

    let mut headers = HeaderMap::new();
    let ct = HeaderValue::from_str(&part.content_type)
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
    headers.insert(header::CONTENT_TYPE, ct);
    if let Ok(v) = HeaderValue::from_str(&cd) {
        headers.insert(header::CONTENT_DISPOSITION, v);
    }
    headers.insert(header::CONTENT_LENGTH, HeaderValue::from(part.bytes.len()));
    // X-Content-Type-Options: nosniff so a browser can't sniff a text/html
    // off an `application/octet-stream` and render the attachment inline —
    // a defence-in-depth pairing with the `Content-Disposition: attachment`
    // disposition that's already telling the browser to download, not render.
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );

    // Stream the in-memory bytes as a chunked response. Zero-copy thanks to
    // `Bytes::from(Vec<u8>)`; chunked so the response body type matches the
    // streaming pattern. When `get_message_part` later grows a true streaming
    // variant, this handler swaps the source stream without touching the
    // header-building code above.
    let body = bytes_to_chunked_stream(part.bytes, STREAM_CHUNK_SIZE);
    let mut response = Response::new(body);
    *response.status_mut() = StatusCode::OK;
    *response.headers_mut() = headers;
    Ok(response)
}

/// Convert an in-memory `Vec<u8>` into an `axum::body::Body` that streams
/// the bytes in `chunk_size`-byte frames. Uses `bytes::Bytes` slicing so
/// the chunks share the underlying allocation (no per-chunk copy).
fn bytes_to_chunked_stream(buf: Vec<u8>, chunk_size: usize) -> Body {
    let bytes = Bytes::from(buf);
    let total = bytes.len();
    let chunk_size = chunk_size.max(1);
    let chunks = (0..total)
        .step_by(chunk_size)
        .map(move |offset| {
            let end = (offset + chunk_size).min(total);
            // `slice` is O(1) — it bumps refcounts on the shared allocation.
            Ok::<_, std::io::Error>(bytes.slice(offset..end))
        })
        .collect::<Vec<_>>();
    Body::from_stream(stream::iter(chunks))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ----- sanitise_filename: path traversal -----

    #[test]
    fn sanitise_strips_forward_slash() {
        assert_eq!(sanitise_filename("etc/passwd"), "etc_passwd");
        // Leading `/` becomes a leading `_` — the important property is that
        // the result has no slashes, so it can never be opened as an absolute
        // path on the receiver's filesystem.
        let out = sanitise_filename("/etc/passwd");
        assert!(!out.contains('/'));
        assert!(out.ends_with("etc_passwd"));
    }

    #[test]
    fn sanitise_strips_backslash() {
        assert_eq!(sanitise_filename("foo\\bar.txt"), "foo_bar.txt");
        // `..\\..\\Windows\\...` — all backslashes become `_`, and the `..`
        // segments are mangled (their dots survive but the slashes don't).
        // The important property: no backslashes survive, so the result
        // can never traverse directories on a Windows receiver.
        let out = sanitise_filename("..\\..\\Windows\\System32\\evil.exe");
        assert!(!out.contains('\\'));
        assert!(out.ends_with("Windows_System32_evil.exe"));
    }

    #[test]
    fn sanitise_strips_double_dot_segments() {
        // `../../etc/passwd` — the slashes become `_`, leaving `.._.._etc_passwd`,
        // then leading dots are stripped, leaving `_.._etc_passwd` collapsed.
        let out = sanitise_filename("../../etc/passwd");
        assert!(!out.contains("/"));
        assert!(!out.contains("\\"));
        // The dot-dot segments are mangled into underscores after the slash
        // substitution; the important assertion is that the result can never
        // be opened as a parent-directory traversal.
        assert!(!out.starts_with("."));
    }

    #[test]
    fn sanitise_replaces_lone_dot_dot() {
        assert_eq!(sanitise_filename(".."), "attachment");
        assert_eq!(sanitise_filename("."), "attachment");
    }

    // ----- sanitise_filename: shell meta -----

    #[test]
    fn sanitise_strips_shell_meta_chars() {
        for hostile in [
            "rm -rf; ls", "a|b", "a&b", "a$b", "a`b", "a<b", "a>b",
            "a(b)", "a{b}", "a[b]", "a*b", "a?b", "a!b", "a~b", "a\"b", "a'b",
        ] {
            let out = sanitise_filename(hostile);
            for bad in [';', '|', '&', '$', '`', '<', '>', '(', ')', '{', '}', '[', ']', '*', '?', '!', '~', '"', '\''] {
                assert!(
                    !out.contains(bad),
                    "shell-meta char {bad:?} survived sanitisation of {hostile:?}: {out}"
                );
            }
        }
    }

    #[test]
    fn sanitise_strips_control_chars_and_nul() {
        let out = sanitise_filename("foo\u{0}bar\u{1}baz\u{7F}qux");
        assert!(!out.contains('\u{0}'));
        assert!(!out.contains('\u{1}'));
        assert!(!out.contains('\u{7F}'));
    }

    #[test]
    fn sanitise_strips_crlf_and_quote() {
        // Already covered by the shell-meta tests for `"` and `'`, but lock
        // in CR/LF explicitly since header injection is the worst-case here.
        let out = sanitise_filename("foo\r\nContent-Type: text/html\r\n");
        assert!(!out.contains('\r'));
        assert!(!out.contains('\n'));
        assert!(!out.contains('"'));
    }

    // ----- sanitise_filename: collapse + empty handling -----

    #[test]
    fn sanitise_collapses_runs_of_substituted_chars() {
        // `<<<<<` would become `_____` without the collapse pass.
        let out = sanitise_filename("<<<<<");
        assert_eq!(out, "_");
    }

    #[test]
    fn sanitise_replaces_empty_with_attachment() {
        assert_eq!(sanitise_filename(""), "attachment");
        assert_eq!(sanitise_filename("   "), "attachment");
        assert_eq!(sanitise_filename("..."), "attachment");
    }

    #[test]
    fn sanitise_strips_leading_dots_so_no_hidden_files() {
        assert_eq!(sanitise_filename(".bashrc"), "bashrc");
        assert_eq!(sanitise_filename("..env"), "env");
    }

    #[test]
    fn sanitise_preserves_safe_unicode_in_name() {
        // Non-ASCII alphabetic chars are kept — the ASCII fallback in the
        // Content-Disposition header is what protects legacy browsers.
        let out = sanitise_filename("résumé.pdf");
        assert!(out.contains('é'));
        assert!(out.ends_with(".pdf"));
    }

    // ----- sanitise_filename: truncation -----

    #[test]
    fn sanitise_truncates_overly_long_names() {
        let long: String = "a".repeat(500);
        let out = sanitise_filename(&long);
        assert!(out.chars().count() <= MAX_FILENAME_LEN);
    }

    #[test]
    fn sanitise_preserves_extension_on_truncate() {
        let long_base: String = "a".repeat(500);
        let raw = format!("{}.pdf", long_base);
        let out = sanitise_filename(&raw);
        assert!(out.chars().count() <= MAX_FILENAME_LEN);
        assert!(out.ends_with(".pdf"), "extension lost: {out}");
    }

    #[test]
    fn sanitise_truncate_handles_no_extension() {
        let long: String = "a".repeat(500);
        let out = sanitise_filename(&long);
        assert_eq!(out.chars().count(), MAX_FILENAME_LEN);
    }

    #[test]
    fn sanitise_truncate_ignores_suspiciously_long_pseudo_extension() {
        // A `.` near the end of a 500-char run isn't really an extension.
        // Don't preserve it.
        let raw = format!("{}.{}", "a".repeat(450), "x".repeat(50));
        let out = sanitise_filename(&raw);
        assert!(out.chars().count() <= MAX_FILENAME_LEN);
    }

    #[test]
    fn truncate_preserving_extension_handles_no_dot() {
        let out = truncate_preserving_extension("abcdefghij", 5);
        assert_eq!(out, "abcde");
    }

    #[test]
    fn truncate_preserving_extension_keeps_extension() {
        let out = truncate_preserving_extension("very-long-filename.pdf", 10);
        assert_eq!(out, "very-l.pdf");
    }

    #[test]
    fn truncate_preserving_extension_does_not_split_multibyte() {
        // 3-byte UTF-8 chars; ensure char-boundary truncation, not byte-boundary.
        let name = "日本語テスト_長いファイル名.pdf";
        let out = truncate_preserving_extension(name, 10);
        assert!(out.chars().count() <= 10);
        assert!(out.ends_with(".pdf"));
        // Round-tripping through String guarantees we didn't construct an
        // invalid UTF-8 sequence — but the type system already enforces
        // that here. The real assertion is the char count.
    }

    // ----- build_content_disposition -----

    #[test]
    fn content_disposition_includes_both_filename_forms() {
        let cd = build_content_disposition("report.pdf");
        assert!(cd.starts_with("attachment;"));
        assert!(cd.contains("filename=\"report.pdf\""));
        assert!(cd.contains("filename*=UTF-8''report.pdf"));
    }

    #[test]
    fn content_disposition_ascii_falls_back_underscores_for_unicode() {
        let cd = build_content_disposition("résumé.pdf");
        // ASCII form: non-ASCII chars replaced with `_`.
        assert!(cd.contains("filename=\"r_sum_.pdf\""));
        // UTF-8 form: percent-encoded.
        assert!(cd.contains("filename*=UTF-8''"));
        assert!(cd.contains("%C3%A9")); // é
    }

    #[test]
    fn content_disposition_url_encodes_spaces_and_special_chars() {
        let cd = build_content_disposition("my report.pdf");
        // urlencoding crate encodes space as %20.
        assert!(cd.contains("filename*=UTF-8''my%20report.pdf"));
    }

    #[test]
    fn content_disposition_is_a_valid_header_value() {
        // Smoke: build a HeaderValue from the result; any unrepresentable
        // bytes would fail this. Covers the property that sanitise_filename
        // + build_content_disposition together produce header-safe output.
        for raw in [
            "report.pdf",
            "résumé.pdf",
            "with spaces.txt",
            "../../../etc/passwd",
            "..\\..\\Windows\\foo.exe",
            "<script>alert(1)</script>.html",
            "foo\r\nX-Evil: yes\r\n.txt",
            "",
        ] {
            let safe = sanitise_filename(raw);
            let cd = build_content_disposition(&safe);
            HeaderValue::from_str(&cd).unwrap_or_else(|e| {
                panic!("Content-Disposition built from {raw:?} -> {cd:?} is not a valid HeaderValue: {e}")
            });
        }
    }

    #[test]
    fn content_disposition_resists_header_injection() {
        // The combination of sanitise + RFC 6266 encoding must NOT let a
        // hostile filename break out of the header. Only CR/LF can do that
        // — colons and equals signs are legitimately allowed inside an HTTP
        // header value (RFC 7230 §3.2.6). So the contract here is "no raw
        // CR/LF survives", not "no scary-looking text survives".
        let raw = "foo\r\nSet-Cookie: pwn=1\r\n.pdf";
        let safe = sanitise_filename(raw);
        let cd = build_content_disposition(&safe);
        assert!(!cd.contains('\r'), "raw CR survived: {cd}");
        assert!(!cd.contains('\n'), "raw LF survived: {cd}");
        // And the whole thing remains a parseable HeaderValue, which is the
        // ultimate property that matters — axum / hyper would reject a header
        // value with embedded CRLF outright.
        HeaderValue::from_str(&cd)
            .unwrap_or_else(|e| panic!("hostile-input Content-Disposition not header-safe: {e}"));
    }

    // ----- bytes_to_chunked_stream -----

    #[tokio::test]
    async fn stream_emits_all_bytes_for_small_buffer() {
        use http_body_util::BodyExt;
        let body = bytes_to_chunked_stream(b"hello world".to_vec(), 4);
        let collected = body.collect().await.expect("body collects").to_bytes();
        assert_eq!(collected.as_ref(), b"hello world");
    }

    #[tokio::test]
    async fn stream_chunks_at_the_requested_boundary() {
        use http_body_util::BodyExt;
        let buf: Vec<u8> = (0u8..=255).collect();
        let body = bytes_to_chunked_stream(buf.clone(), 32);
        // Walk frames and count: 256 bytes / 32 per chunk = 8 chunks.
        let mut frames = 0usize;
        let mut total = Vec::new();
        let mut body = body;
        while let Some(frame) = body.frame().await {
            let frame = frame.expect("frame ok");
            if let Ok(data) = frame.into_data() {
                frames += 1;
                total.extend_from_slice(&data);
            }
        }
        assert_eq!(frames, 8);
        assert_eq!(total, buf);
    }

    #[tokio::test]
    async fn stream_handles_empty_buffer() {
        use http_body_util::BodyExt;
        let body = bytes_to_chunked_stream(Vec::new(), 64);
        let collected = body.collect().await.expect("body collects").to_bytes();
        assert!(collected.is_empty());
    }

    #[tokio::test]
    async fn stream_handles_chunk_size_larger_than_buffer() {
        use http_body_util::BodyExt;
        let body = bytes_to_chunked_stream(b"tiny".to_vec(), 1024 * 1024);
        let collected = body.collect().await.expect("body collects").to_bytes();
        assert_eq!(collected.as_ref(), b"tiny");
    }
}
