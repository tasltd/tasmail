// Added (TMAIL-363): GET /classic/folders/{folder}/messages/{uid} — the
// message read view for the no-JS Classic UI surface (driver TMAIL-299,
// gap-analysis `docs/gap-analysis/classic-ui.md` P0 #9).
//
// What this renders
// -----------------
//   * Semantic `<dl>` header block: From / To / Cc / Subject / Date.
//     Cc is omitted entirely when the envelope's Cc list is empty so we
//     don't render an "empty" label that screen readers announce.
//   * Body: when the message has a `text/html` part, it goes through
//     `services::html_sanitizer::sanitize_email_html` (strict allowlist —
//     no <script>, no inline event handlers, no javascript:/vbscript:
//     URLs, no <iframe>/<form>/<base>) and is dropped into the page with
//     `{{ html_body|safe }}`. When the message has only `text/plain`, it
//     renders inside a `<pre>` so newlines + monospaced spacing survive.
//     When the message has neither (rare — usually only attachments), the
//     body block renders an empty-state notice.
//   * Attachment list: `<ul>` of `<li>` rows, each with a download `<a>`
//     pointing at `/classic/folders/{folder}/messages/{uid}/parts/{part_id}`.
//     The endpoint itself is the next P0 task (#11 / TMAIL-365); the link
//     is wired now so the read view doesn't need a retrofit later — a
//     click before TMAIL-365 lands hits the catch-all 404, which is the
//     expected behaviour during the build-out.
//   * Action button row, each its OWN `<form method=post>` so each is a
//     CSRF-protected submission rather than a single multi-button form:
//       - Reply / Reply-All / Forward — GET links into the compose
//         endpoint (compose lands in TMAIL-366 / P0 #12) carrying
//         `?reply_to=…` / `?reply_all=…` / `?forward=…` query params.
//         Modelled as links rather than POSTs because they're idempotent
//         navigation, not state changes.
//       - Delete — POST to /classic/folders/{folder}/messages/{uid}/delete
//         (the endpoint is TMAIL-367 / P0 #13). A `_csrf` hidden input
//         carries the session token so the canonical CSRF middleware can
//         validate the action when TMAIL-367 lands.
//       - Move-to-folder dropdown — P1 #18. We render the placeholder
//         disabled `<select>` today so the layout is right; TMAIL-380
//         will hydrate the option list from `list_folders`.
//       - Star — P1 #17. Disabled placeholder button.
//       - Mark unread — P1 #16. Disabled placeholder button.
//
// What this does NOT do (deliberately deferred)
// ---------------------------------------------
//   * Block remote `<img src=…>` URLs — that's TMAIL-386 (P1) + the
//     per-user `block_remote_images` setting.
//   * Render any of the deferred action buttons. They render as disabled
//     placeholders so the layout doesn't shift when the follow-up tasks
//     drop in.
//   * Mark the message as read explicitly here — `ImapService::get_message`
//     already does that internally via `UID STORE +FLAGS (\Seen)` after a
//     successful fetch.

use askama::Template;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension,
};
use uuid::Uuid;

use crate::error::AppError;
use crate::services::auth_service::Claims;
use crate::services::html_sanitizer::sanitize_email_html;
use crate::services::imap_service::{Attachment, FullMessage, ImapService};
use crate::state::AppState;

use super::CspNonce;

/// Pretty-printed file size for the attachment list. Email attachments are
/// almost always <100 MB, so a 3-tier (`B → KB → MB`) format covers every
/// realistic case without dragging in a humansize crate.
fn format_size(bytes: usize) -> String {
    const KB: usize = 1024;
    const MB: usize = 1024 * 1024;
    if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Body shape the template branches on. Keeping the three states as a
/// closed enum (rather than two `Option<String>` template variables) means
/// the template doesn't have to express the negative case ("no html AND
/// no plain"); it just `match`-equivalents on the variant.
#[derive(Debug, PartialEq, Eq)]
pub enum BodyRendering {
    /// `text/html` body that's already been through
    /// `sanitize_email_html`. Rendered with `{{ body|safe }}`.
    Html(String),
    /// `text/plain` body; rendered inside `<pre>` so newlines and
    /// monospaced layout are preserved.
    Text(String),
    /// Neither part present — rare; usually only attachments. Renders the
    /// empty-state notice.
    Empty,
}

impl BodyRendering {
    pub fn is_html(&self) -> bool {
        matches!(self, BodyRendering::Html(_))
    }
    pub fn is_text(&self) -> bool {
        matches!(self, BodyRendering::Text(_))
    }
    pub fn is_empty(&self) -> bool {
        matches!(self, BodyRendering::Empty)
    }
    pub fn html(&self) -> &str {
        if let BodyRendering::Html(s) = self {
            s
        } else {
            ""
        }
    }
    pub fn text(&self) -> &str {
        if let BodyRendering::Text(s) = self {
            s
        } else {
            ""
        }
    }
}

/// Per-attachment view-model for the template list. Pre-computes the
/// download href so the template doesn't have to concat strings; pre-
/// formats the size string for the same reason.
pub struct AttachmentRow {
    pub filename: String,
    pub content_type: String,
    pub size_display: String,
    pub download_href: String,
}

/// Pick the right body branch from a `FullMessage`. Pulled into a free
/// function so the unit tests can exercise the sanitisation + fallback
/// path without going through IMAP.
///
/// Precedence:
///   1. `html_body` if present and non-empty → sanitise and render as HTML
///   2. `text_body` if present and non-empty → render as plain text
///   3. `Empty` — no usable body
pub fn build_body(text_body: Option<&str>, html_body: Option<&str>) -> BodyRendering {
    if let Some(html) = html_body.filter(|s| !s.trim().is_empty()) {
        return BodyRendering::Html(sanitize_email_html(html));
    }
    if let Some(text) = text_body.filter(|s| !s.trim().is_empty()) {
        return BodyRendering::Text(text.to_string());
    }
    BodyRendering::Empty
}

/// Build the per-attachment view-model list from the IMAP fetch result.
/// `folder_href` is the URL-encoded folder name already in the page URL.
pub fn build_attachment_rows(
    attachments: &[Attachment],
    folder_href: &str,
    uid: u32,
) -> Vec<AttachmentRow> {
    attachments
        .iter()
        .map(|a| AttachmentRow {
            download_href: format!(
                "/classic/folders/{}/messages/{}/parts/{}",
                folder_href,
                uid,
                urlencoding::encode(&a.part_id)
            ),
            filename: if a.filename.trim().is_empty() {
                "(unnamed attachment)".to_string()
            } else {
                a.filename.clone()
            },
            content_type: a.content_type.clone(),
            size_display: format_size(a.size),
        })
        .collect()
}

/// Askama template struct backing `templates/classic/message.html`.
///
/// Field names match the template `{{ var }}` placeholders exactly —
/// Askama validates this at compile time so a rename here without the
/// matching template edit fails `cargo build`.
#[derive(Template)]
#[template(path = "classic/message.html")]
pub struct MessageTemplate {
    /// Display name of the folder this message lives in (e.g. INBOX).
    pub current_folder: String,
    /// URL-encoded path segment for the folder, used by the back-to-folder
    /// link, the action-button form actions, and the attachment links.
    pub current_folder_href: String,
    /// IMAP UID of the message — surfaces in the page title and forms.
    pub uid: u32,
    /// Subject (already trimmed; `(no subject)` substituted when empty).
    pub subject: String,
    /// From header — single display string ("Name <addr@host>").
    pub from: String,
    /// To header list — pre-joined with ", " so the template doesn't
    /// have to deal with `Vec` iteration ergonomics.
    pub to: String,
    /// Cc header list — same shape as `to`. Empty string when the
    /// envelope's Cc is empty; the template uses this to omit the entire
    /// Cc row rather than rendering a label with no value.
    pub cc: String,
    /// Display-formatted Date header. Empty string when missing — the
    /// template hides the row in that case.
    pub date: String,
    /// Resolved body rendering (HTML/text/empty). The template
    /// `match`-equivalents on the three variants.
    pub body: BodyRendering,
    /// Pre-built attachment rows for the `<ul>` list. Empty when the
    /// message has no attachments.
    pub attachments: Vec<AttachmentRow>,
    /// Compose link for "Reply" — points at the compose endpoint with
    /// `?reply_to={uid}`. The endpoint itself ships in TMAIL-366; the
    /// link works today as a navigation that ends on the catch-all 404.
    pub reply_href: String,
    /// Compose link for "Reply all" — same shape, `?reply_all={uid}`.
    pub reply_all_href: String,
    /// Compose link for "Forward" — same shape, `?forward={uid}`.
    pub forward_href: String,
    /// Form action for the Delete button (POST). Endpoint lands in
    /// TMAIL-367; today a POST returns the catch-all 404, which is the
    /// expected during-build behaviour.
    pub delete_action: String,
    /// Session CSRF token threaded into every action form (Delete today,
    /// Star / Mark-unread / Move when those flip from placeholder to
    /// real). Also into the logout form partial.
    pub csrf_token: String,
    /// Per-request CSP nonce. Required by base.html (TMAIL-356).
    pub csp_nonce: String,
}

/// GET /classic/folders/{folder}/messages/{uid} — render the message
/// read view.
///
/// `folder` is URL-decoded by axum's `Path` extractor. We pass it verbatim
/// to IMAP — a missing folder bubbles up as `AppError::Imap`, and a
/// missing UID as `AppError::NotFound`. Both render through the global
/// error layer; mapping IMAP "no such mailbox / no such uid" errors to
/// a friendlier Classic-UI page is the P2 polish task.
pub async fn get_message(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Extension(session): Extension<crate::models::classic_session::ClassicSession>,
    // Added (TMAIL-368): per-request CSP nonce so the inline `<style nonce="…">`
    // on base.html matches the strict /classic/* CSP response header.
    Extension(csp_nonce): Extension<CspNonce>,
    Path((folder, uid)): Path<(String, u32)>,
) -> Result<Response, AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID in classic claims")))?;

    // 1) Build the BYOK IMAP service from the user's saved imap_configurations
    //    row. Decrypts the password with the JWT-derived AES key. Same shape
    //    as the folder handler.
    let imap_service = ImapService::for_user(&state, mailbox_id).await?;
    let (username, password) = imap_service
        .user_creds()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("BYOK creds missing on ImapService")))?;
    // Match folder.rs: convert to owned `String` so the borrows on
    // `imap_service.user_credentials` don't fight with the `&self` borrow
    // `get_message` needs across the `.await`.
    let username = username.to_string();
    let password = password.to_string();

    // 2) Fetch the full message (FLAGS + BODY[] + ENVELOPE). `get_message`
    //    also marks the message \Seen as a side-effect of opening it, which
    //    matches the behaviour of every other webmail "open this message"
    //    code path on the project.
    let full: FullMessage = imap_service
        .get_message(&username, &password, &folder, uid)
        .await?;

    // 3) Build template view-model.
    let folder_href = urlencoding::encode(&folder).into_owned();
    let subject = full
        .subject
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "(no subject)".to_string());
    let from = full.from.unwrap_or_default();
    let to = full.to.join(", ");
    let cc = full.cc.join(", ");
    let date = full.date.unwrap_or_default();
    let body = build_body(full.text_body.as_deref(), full.html_body.as_deref());
    let attachments = build_attachment_rows(&full.attachments, &folder_href, full.uid);

    // Action endpoints — base path shared across every form action /
    // compose link so a future renamer (e.g. /classic/mail/...) only
    // edits one format string.
    let base = format!("/classic/folders/{}/messages/{}", folder_href, full.uid);
    let reply_href = format!("/classic/compose?reply_to={}&folder={}", full.uid, folder_href);
    let reply_all_href = format!("/classic/compose?reply_all={}&folder={}", full.uid, folder_href);
    let forward_href = format!("/classic/compose?forward={}&folder={}", full.uid, folder_href);
    let delete_action = format!("{}/delete", base);

    let template = MessageTemplate {
        current_folder: folder,
        current_folder_href: folder_href,
        uid: full.uid,
        subject,
        from,
        to,
        cc,
        date,
        body,
        attachments,
        reply_href,
        reply_all_href,
        forward_href,
        delete_action,
        csrf_token: session.csrf_token.clone(),
        csp_nonce: csp_nonce.into_string(),
    };

    let html = template.render().map_err(|e| {
        AppError::Internal(anyhow::anyhow!(
            "classic message template render failed: {e}"
        ))
    })?;

    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attachment(filename: &str, content_type: &str, size: usize, part_id: &str) -> Attachment {
        Attachment {
            filename: filename.to_string(),
            content_type: content_type.to_string(),
            size,
            part_id: part_id.to_string(),
        }
    }

    fn fresh_template() -> MessageTemplate {
        MessageTemplate {
            current_folder: "INBOX".to_string(),
            current_folder_href: "INBOX".to_string(),
            uid: 42,
            subject: "Test subject".to_string(),
            from: "Alice <alice@example.com>".to_string(),
            to: "Bob <bob@example.com>".to_string(),
            cc: String::new(),
            date: "Mon, 1 Jan 2026 09:00:00 +0000".to_string(),
            body: BodyRendering::Text("hello world".to_string()),
            attachments: vec![],
            reply_href: "/classic/compose?reply_to=42&folder=INBOX".to_string(),
            reply_all_href: "/classic/compose?reply_all=42&folder=INBOX".to_string(),
            forward_href: "/classic/compose?forward=42&folder=INBOX".to_string(),
            delete_action: "/classic/folders/INBOX/messages/42/delete".to_string(),
            csrf_token: "test-csrf-token".to_string(),
            csp_nonce: "test-nonce-fixed".to_string(),
        }
    }

    // ----- format_size -----

    #[test]
    fn format_size_renders_bytes_for_under_1_kb() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1023), "1023 B");
    }

    #[test]
    fn format_size_renders_kb_under_1_mb() {
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(1024 * 1024 - 1), "1024.0 KB");
    }

    #[test]
    fn format_size_renders_mb_at_or_above_1_mb() {
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_size(5 * 1024 * 1024 + 1024 * 512), "5.5 MB");
    }

    // ----- build_body -----

    #[test]
    fn build_body_prefers_html_when_present() {
        let body = build_body(Some("plain alt"), Some("<p>HTML</p>"));
        assert!(body.is_html());
        assert!(body.html().contains("<p>HTML</p>"));
    }

    #[test]
    fn build_body_falls_back_to_text_when_html_missing() {
        let body = build_body(Some("plain only"), None);
        assert!(body.is_text());
        assert_eq!(body.text(), "plain only");
    }

    #[test]
    fn build_body_falls_back_to_text_when_html_empty() {
        let body = build_body(Some("plain"), Some("   "));
        assert!(body.is_text());
    }

    #[test]
    fn build_body_returns_empty_when_neither_present() {
        let body = build_body(None, None);
        assert!(body.is_empty());
    }

    #[test]
    fn build_body_sanitises_html_strips_script() {
        let body = build_body(None, Some("<p>ok</p><script>alert('xss')</script>"));
        assert!(body.is_html());
        let html = body.html();
        assert!(!html.contains("<script"));
        assert!(!html.contains("alert"));
        assert!(html.contains("<p>ok</p>"));
    }

    #[test]
    fn build_body_sanitises_html_strips_onload() {
        let body = build_body(None, Some(r#"<body onload="evil()">x</body>"#));
        assert!(body.is_html());
        assert!(!body.html().contains("onload"));
    }

    // ----- build_attachment_rows -----

    #[test]
    fn build_attachment_rows_constructs_download_href_per_attachment() {
        let atts = vec![
            attachment("doc.pdf", "application/pdf", 2048, "1.2"),
            attachment("image.png", "image/png", 4096, "1.3"),
        ];
        let rows = build_attachment_rows(&atts, "INBOX", 42);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].filename, "doc.pdf");
        assert_eq!(rows[0].size_display, "2.0 KB");
        assert_eq!(rows[0].content_type, "application/pdf");
        assert_eq!(
            rows[0].download_href,
            "/classic/folders/INBOX/messages/42/parts/1.2"
        );
        assert_eq!(rows[1].download_href, "/classic/folders/INBOX/messages/42/parts/1.3");
    }

    #[test]
    fn build_attachment_rows_url_encodes_folder_and_part_id() {
        // Folder href segment is already URL-encoded by the caller, so we
        // don't double-encode. The part_id, however, MUST be encoded — a
        // semicolon or slash in a future hypothetical part path would
        // break routing.
        let atts = vec![attachment("a.bin", "application/octet-stream", 100, "1;2/3")];
        let rows = build_attachment_rows(&atts, "%5BGmail%5D%2FAll%20Mail", 5);
        assert_eq!(
            rows[0].download_href,
            "/classic/folders/%5BGmail%5D%2FAll%20Mail/messages/5/parts/1%3B2%2F3"
        );
    }

    #[test]
    fn build_attachment_rows_substitutes_placeholder_for_empty_filename() {
        let atts = vec![attachment("", "application/octet-stream", 10, "2")];
        let rows = build_attachment_rows(&atts, "INBOX", 1);
        assert_eq!(rows[0].filename, "(unnamed attachment)");
    }

    #[test]
    fn build_attachment_rows_returns_empty_vec_for_empty_input() {
        let rows = build_attachment_rows(&[], "INBOX", 1);
        assert!(rows.is_empty());
    }

    // ----- Template rendering -----

    #[test]
    fn message_template_extends_base_layout() {
        let body = fresh_template().render().expect("template renders");
        assert!(body.contains("<!DOCTYPE html>"), "missing HTML5 doctype");
        assert!(body.contains("class=\"skip-link\""), "skip-link missing");
        assert!(body.contains("<main id=\"main\""), "<main> landmark missing");
        assert!(
            body.contains("<style nonce=\"test-nonce-fixed\">"),
            "inline <style> must carry the per-request CSP nonce: {body}"
        );
    }

    #[test]
    fn message_template_has_zero_script_tags() {
        // Classic UI is no-JS — hard rule.
        let body = fresh_template().render().expect("template renders");
        assert!(
            !body.contains("<script"),
            "message template must contain ZERO <script> tags: {body}"
        );
    }

    #[test]
    fn message_template_renders_logout_form_with_csrf_token() {
        // Read view is an authenticated page → MUST override the
        // logout_form block from base.html.
        let body = fresh_template().render().expect("template renders");
        assert!(
            body.contains("action=\"/classic/logout\""),
            "logout form must render on authenticated read view: {body}"
        );
        assert!(
            body.contains("value=\"test-csrf-token\""),
            "logout form must carry the session csrf_token: {body}"
        );
    }

    #[test]
    fn message_template_renders_semantic_header_dl() {
        let body = fresh_template().render().expect("template renders");
        // <dl> with <dt> labels + <dd> values — the spec calls for a
        // semantic <dl> or <table>, and <dl> is the better fit because
        // each header is a single-value definition, not tabular data.
        assert!(body.contains("<dl"), "header block must be a <dl>: {body}");
        assert!(body.contains("<dt"), "header dt labels missing");
        assert!(body.contains("<dd"), "header dd values missing");
        assert!(body.contains(">From<"), "From label missing");
        assert!(body.contains(">To<"), "To label missing");
        assert!(body.contains(">Subject<"), "Subject label missing");
        assert!(body.contains(">Date<"), "Date label missing");
        // Header values rendered.
        assert!(body.contains("Alice &#60;alice@example.com&#62;") || body.contains("Alice &lt;alice@example.com&gt;"));
        assert!(body.contains("Test subject"));
    }

    #[test]
    fn message_template_omits_cc_row_when_cc_empty() {
        // No empty "Cc:" label — screen readers shouldn't announce a label
        // with no value. The fresh_template fixture has `cc = ""`.
        let body = fresh_template().render().expect("template renders");
        assert!(
            !body.contains(">Cc<"),
            "Cc label must NOT render when Cc list is empty: {body}"
        );
    }

    #[test]
    fn message_template_renders_cc_row_when_cc_present() {
        let mut t = fresh_template();
        t.cc = "Carol <carol@example.com>".to_string();
        let body = t.render().expect("template renders");
        assert!(body.contains(">Cc<"));
        assert!(
            body.contains("Carol &#60;carol@example.com&#62;")
                || body.contains("Carol &lt;carol@example.com&gt;")
        );
    }

    #[test]
    fn message_template_renders_plaintext_body_in_pre() {
        let mut t = fresh_template();
        t.body = BodyRendering::Text("line one\nline two".to_string());
        let body = t.render().expect("template renders");
        assert!(body.contains("<pre"), "<pre> wrapper missing on plaintext body");
        // The actual text content is escaped by Askama auto-escape.
        assert!(body.contains("line one"));
        assert!(body.contains("line two"));
    }

    #[test]
    fn message_template_renders_html_body_with_safe_marker() {
        // The HTML branch uses `{{ body.html()|safe }}` — verify the
        // pre-sanitised HTML lands verbatim in the output.
        let mut t = fresh_template();
        t.body = BodyRendering::Html("<p>hello <strong>world</strong></p>".to_string());
        let body = t.render().expect("template renders");
        assert!(body.contains("<p>hello <strong>world</strong></p>"));
        // And the plaintext <pre> wrapper is NOT rendered for the HTML branch.
        assert!(!body.contains("<pre class=\"msg-text\""));
    }

    #[test]
    fn message_template_renders_empty_state_when_no_body() {
        let mut t = fresh_template();
        t.body = BodyRendering::Empty;
        let body = t.render().expect("template renders");
        assert!(
            body.contains("(This message has no readable body."),
            "empty-state copy missing: {body}"
        );
    }

    #[test]
    fn message_template_html_escapes_hostile_subject_and_sender() {
        // Defence in depth — Askama auto-escapes for .html, but lock the
        // behaviour in with a test so a config drift can't silently turn
        // it off and let a phisher's <script> escape onto the page.
        let mut t = fresh_template();
        t.subject = "<script>alert('subj')</script>".to_string();
        t.from = "\"><script>alert('from')</script>".to_string();
        let body = t.render().expect("template renders");
        assert!(!body.contains("<script>alert('subj')</script>"));
        assert!(!body.contains("<script>alert('from')</script>"));
    }

    #[test]
    fn message_template_renders_attachment_list_with_download_links() {
        let mut t = fresh_template();
        t.attachments = vec![
            AttachmentRow {
                filename: "doc.pdf".to_string(),
                content_type: "application/pdf".to_string(),
                size_display: "2.0 KB".to_string(),
                download_href: "/classic/folders/INBOX/messages/42/parts/1.2".to_string(),
            },
            AttachmentRow {
                filename: "image.png".to_string(),
                content_type: "image/png".to_string(),
                size_display: "4.0 KB".to_string(),
                download_href: "/classic/folders/INBOX/messages/42/parts/1.3".to_string(),
            },
        ];
        let body = t.render().expect("template renders");
        // The list is rendered as a <ul> per the spec.
        assert!(body.contains("<ul"), "attachment list must be a <ul>");
        let li_count = body.matches("<li class=\"attachment-row\"").count();
        assert_eq!(li_count, 2, "expected 2 <li> rows: {body}");
        assert!(body.contains("href=\"/classic/folders/INBOX/messages/42/parts/1.2\""));
        assert!(body.contains("href=\"/classic/folders/INBOX/messages/42/parts/1.3\""));
        assert!(body.contains("doc.pdf"));
        assert!(body.contains("image.png"));
        assert!(body.contains("application/pdf"));
        assert!(body.contains("2.0 KB"));
    }

    #[test]
    fn message_template_omits_attachment_section_when_none() {
        let body = fresh_template().render().expect("template renders");
        assert!(
            !body.contains("class=\"attachments\""),
            "attachment section should NOT render when there are no attachments: {body}"
        );
    }

    #[test]
    fn message_template_renders_reply_reply_all_forward_links() {
        let body = fresh_template().render().expect("template renders");
        // Reply / Reply-All / Forward render as <a> links — they're
        // idempotent navigation, so they don't need to be POSTs. Askama's
        // auto-escape may render the query-string `&` as either `&amp;`
        // or the numeric `&#38;`; both are valid HTML so accept either.
        for (label, prefix) in [
            ("Reply", "reply_to"),
            ("Reply-All", "reply_all"),
            ("Forward", "forward"),
        ] {
            let amp = format!("href=\"/classic/compose?{prefix}=42&amp;folder=INBOX\"");
            let numeric = format!("href=\"/classic/compose?{prefix}=42&#38;folder=INBOX\"");
            assert!(
                body.contains(&amp) || body.contains(&numeric),
                "{label} link missing (neither &amp; nor &#38; form found): {body}"
            );
        }
        // And the labels are user-facing.
        assert!(body.contains(">Reply<"));
        assert!(body.contains(">Reply all<"));
        assert!(body.contains(">Forward<"));
    }

    #[test]
    fn message_template_renders_delete_as_its_own_form_with_csrf() {
        let body = fresh_template().render().expect("template renders");
        // Delete is its OWN <form method=post> — per the spec, every
        // action button is a separate CSRF-protected form (not a single
        // multi-button form). That makes the CSRF check on each action
        // independent and prevents accidental cross-action submission
        // by an old browser.
        assert!(
            body.contains("action=\"/classic/folders/INBOX/messages/42/delete\""),
            "Delete form action missing: {body}"
        );
        assert!(
            body.contains("name=\"_csrf\" value=\"test-csrf-token\""),
            "Delete form must carry the session csrf_token: {body}"
        );
        assert!(body.contains(">Delete<"));
    }

    #[test]
    fn message_template_renders_disabled_placeholders_for_deferred_actions() {
        // P1 placeholders — Move-to-folder dropdown, Star, Mark unread.
        // They render as disabled controls so the layout is right NOW;
        // the follow-up tasks just flip the `disabled` attribute and
        // wire the handler.
        let body = fresh_template().render().expect("template renders");
        assert!(
            body.contains(">Star<"),
            "Star button placeholder missing: {body}"
        );
        assert!(
            body.contains(">Mark unread<"),
            "Mark-unread button placeholder missing: {body}"
        );
        assert!(
            body.contains("<select"),
            "Move-to-folder <select> missing: {body}"
        );
        // Disabled — TMAIL-380 / TMAIL-377 / TMAIL-376 will hydrate them.
        let disabled_count = body.matches("disabled").count();
        assert!(
            disabled_count >= 3,
            "expected at least 3 disabled controls (Star, Mark unread, Move): {body}"
        );
    }

    #[test]
    fn message_template_renders_back_to_folder_link() {
        let body = fresh_template().render().expect("template renders");
        // Back-to-folder link so the user can navigate without using the
        // browser back button (which would re-POST any form they came from).
        assert!(
            body.contains("href=\"/classic/folders/INBOX\""),
            "back-to-folder link missing: {body}"
        );
    }

    #[test]
    fn message_template_renders_page_title_with_subject() {
        let body = fresh_template().render().expect("template renders");
        assert!(
            body.contains("<title>Test subject"),
            "<title> must lead with the subject: {body}"
        );
    }
}
