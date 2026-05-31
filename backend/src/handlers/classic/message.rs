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
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension, Form,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::remote_image_allowlist;
use crate::services::auth_service::Claims;
use crate::services::html_sanitizer::{
    html_has_remote_images, sanitize_email_html_with_options, SanitizeOptions,
};
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
///
/// `allow_remote_images` is forwarded straight to the sanitiser
/// (`SanitizeOptions::allow_remote_images`). Defaults to `false` everywhere
/// except the two TMAIL-386 opt-in paths:
///   * `?show_images=1` query on the GET handler (one-shot per render).
///   * Per-sender row in `remote_image_allowlist` ("Always show images
///     from this sender").
pub fn build_body(
    text_body: Option<&str>,
    html_body: Option<&str>,
    allow_remote_images: bool,
) -> BodyRendering {
    if let Some(html) = html_body.filter(|s| !s.trim().is_empty()) {
        let cleaned = sanitize_email_html_with_options(
            html,
            SanitizeOptions {
                allow_remote_images,
            },
        );
        return BodyRendering::Html(cleaned);
    }
    if let Some(text) = text_body.filter(|s| !s.trim().is_empty()) {
        return BodyRendering::Text(text.to_string());
    }
    BodyRendering::Empty
}

/// PURPOSE (TMAIL-386): pull the bare email address out of a From header
/// display string like `"Alice <alice@example.com>"` or `alice@example.com`
/// so it can be used as the lookup key for the per-sender allowlist.
///
/// Returns `None` when the input doesn't contain a recognisable `@`-bearing
/// token — defensive, so a hostile sender that ships a malformed `From`
/// header can't accidentally land an empty row in `remote_image_allowlist`
/// or short-circuit the lookup against `WHERE sender_address = ''`.
///
/// Heuristic — good enough for an opt-in keyed on what the user sees in the
/// From row, without dragging in a full RFC 5322 address parser:
///   * If the string contains `<...>`, return whatever's between the angle
///     brackets when that span contains an `@`.
///   * Otherwise, scan word-by-word and return the first whitespace-
///     separated token that contains `@`.
///   * Trim ASCII whitespace and surrounding `"` / `'` / `,` / `;` from the
///     result, then enforce `local@domain` shape.
pub fn parse_sender_email(from_header: &str) -> Option<String> {
    let candidate = if let (Some(lt), Some(gt)) = (from_header.find('<'), from_header.rfind('>')) {
        if gt > lt {
            let inside = &from_header[lt + 1..gt];
            if inside.contains('@') {
                inside.to_string()
            } else {
                from_header.to_string()
            }
        } else {
            from_header.to_string()
        }
    } else {
        from_header.to_string()
    };
    // If there were no angle brackets, scan space-separated tokens for the
    // first `@`-bearing one. (Display name first, address last is the most
    // common shape: `Alice alice@example.com`.)
    let token = if candidate.contains('<') {
        // The candidate is still the full string because angle parsing
        // didn't yield an @-bearing inner span. Fall through to token scan.
        candidate
            .split_whitespace()
            .find(|t| t.contains('@'))
            .unwrap_or("")
            .to_string()
    } else if candidate.contains('@') && candidate.split_whitespace().count() <= 1 {
        candidate
    } else {
        candidate
            .split_whitespace()
            .find(|t| t.contains('@'))
            .unwrap_or("")
            .to_string()
    };
    let trimmed = token
        .trim()
        .trim_matches(|c: char| c == '"' || c == '\'' || c == ',' || c == ';' || c == '<' || c == '>')
        .trim();
    if trimmed.is_empty() {
        return None;
    }
    // Enforce shape: exactly one `@`, non-empty local + domain, and the
    // domain has at least one `.`.
    let (local, domain) = trimmed.split_once('@')?;
    if local.is_empty() || domain.is_empty() || !domain.contains('.') {
        return None;
    }
    Some(trimmed.to_string())
}

/// Added (TMAIL-371): Detect whether the message's IMAP flag set carries
/// `\Flagged` (a.k.a. the "starred" flag). Tolerates the `Flagged` vs
/// `\Flagged` Debug-shape variance plus Gmail's `$Starred` keyword so the
/// read view's Star button toggles correctly across the providers our
/// BYOK target list covers (Gmail, Outlook, Yahoo, Zoho, FastMail,
/// iCloud, ProtonMail Bridge, self-hosted Dovecot, Stalwart).
pub fn message_is_starred(flags: &[String]) -> bool {
    flags.iter().any(|f| {
        f.eq_ignore_ascii_case("Flagged")
            || f.eq_ignore_ascii_case("\\Flagged")
            || f.eq_ignore_ascii_case("$Starred")
    })
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
    /// Added (TMAIL-370): POST action for the Mark unread button. Same
    /// endpoint as the future Mark read (P1 #16 is single-toggle from the
    /// read view), but the read view always renders "Mark unread" since
    /// opening the message already marks it `\Seen` as a side-effect.
    pub flag_action: String,
    /// Added (TMAIL-371): true when the IMAP `\Flagged` flag is set on
    /// this message. Drives the read view's Star button — when starred,
    /// the button reads "Unstar" and POSTs `mark=unstar`; when unstarred,
    /// it reads "Star" and POSTs `mark=star`. Both forms hit the same
    /// `flag_action` endpoint above.
    pub is_starred: bool,
    /// Added (TMAIL-372): POST action for the Move-to-folder form. Same
    /// `/messages/{uid}/move` endpoint the bulk handler dispatches to via
    /// `flag::post_bulk`. The template renders the dropdown only when
    /// `move_targets` is non-empty.
    pub move_action: String,
    /// Added (TMAIL-372): pre-filtered list of folder names the user can
    /// move THIS message into. Excludes the source folder and any
    /// forbidden virtual mailboxes (see `move_to::FORBIDDEN_TARGET_PATTERNS`)
    /// so the dropdown can't surface an invalid option in the first place.
    /// Empty = render a disabled placeholder (e.g. fresh account with no
    /// extra folders yet).
    pub move_targets: Vec<String>,
    /// Session CSRF token threaded into every action form (Delete today,
    /// Star / Mark-unread / Move when those flip from placeholder to
    /// real). Also into the logout form partial.
    pub csrf_token: String,
    /// Per-request CSP nonce. Required by base.html (TMAIL-356).
    pub csp_nonce: String,
    /// Added (TMAIL-384): Footer "Using X of Y · NN%" quota indicator.
    /// Hydrated by `super::load_quota_indicator`; `None` means the loader
    /// couldn't reach the cache + DB, in which case the partial just
    /// renders nothing so the rest of the read view still loads.
    pub quota_indicator: Option<super::QuotaIndicator>,
    /// Added (TMAIL-386): `true` when the *raw* (pre-sanitiser) HTML body
    /// contained at least one remote `<img src="http(s)://...">` element.
    /// Drives the "[Remote images blocked]" banner above the body — the
    /// banner is only useful when there were blocked images to opt back in
    /// to seeing.
    pub remote_images_present: bool,
    /// Added (TMAIL-386): `true` when this render is currently surfacing
    /// the real remote URLs (because the user clicked "Show images" OR the
    /// sender is on `remote_image_allowlist`). Drives the *second* banner
    /// state — when images ARE shown, we tell the user why so they don't
    /// think the privacy default broke.
    pub remote_images_shown: bool,
    /// Added (TMAIL-386): `true` when the sender is on
    /// `remote_image_allowlist`. Drives a small "Always-allowed sender"
    /// note in the shown-images banner so the user knows the persistent
    /// allowlist row is what's surfacing the images (vs the one-shot
    /// `?show_images=1` query).
    pub remote_images_from_allowlisted_sender: bool,
    /// Added (TMAIL-386): URL-encoded sender address (or empty when the
    /// sender header didn't parse to a valid `local@domain.tld`). Empty
    /// means the "Always show images from this sender" button MUST NOT
    /// render — there's nothing valid to persist. Embedded in the form
    /// action so the POST handler doesn't have to re-fetch the message
    /// just to learn who sent it.
    pub allow_sender_address: String,
    /// Added (TMAIL-386): POST action for the one-shot "Show images" button
    /// (the form posts to the same message URL with `?show_images=1` so the
    /// 303-redirect lands on the same GET handler with the query primed).
    pub show_images_once_action: String,
    /// Added (TMAIL-386): POST action for the persistent "Always show
    /// images from this sender" button.
    pub show_images_always_action: String,
}

/// GET /classic/folders/{folder}/messages/{uid} — render the message
/// read view.
///
/// `folder` is URL-decoded by axum's `Path` extractor. We pass it verbatim
/// to IMAP — a missing folder bubbles up as `AppError::Imap`, and a
/// missing UID as `AppError::NotFound`. Both render through the global
/// error layer; mapping IMAP "no such mailbox / no such uid" errors to
/// a friendlier Classic-UI page is the P2 polish task.
/// Added (TMAIL-386): query-string carrier for the one-shot "Show images"
/// opt-in. Set by the POST `show-images-once` handler's 303 redirect, then
/// consumed on the follow-up GET. `serde(default)` so the bare
/// `/classic/folders/INBOX/messages/42` URL (the common case) still
/// deserialises without 400'ing.
#[derive(Debug, Default, Deserialize)]
pub struct MessageQuery {
    #[serde(default)]
    pub show_images: Option<String>,
}

impl MessageQuery {
    /// True when the query carries `?show_images=1` (or `=true` / `=yes` —
    /// same liberal accept the Delete form's `is_confirmed()` uses, so the
    /// flag is impossible to mis-set when typed by hand).
    pub fn show_images(&self) -> bool {
        matches!(self.show_images.as_deref(), Some("1" | "true" | "yes"))
    }
}

pub async fn get_message(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Extension(session): Extension<crate::models::classic_session::ClassicSession>,
    // Added (TMAIL-368): per-request CSP nonce so the inline `<style nonce="…">`
    // on base.html matches the strict /classic/* CSP response header.
    Extension(csp_nonce): Extension<CspNonce>,
    Path((folder, uid)): Path<(String, u32)>,
    // Added (TMAIL-386): `?show_images=1` opt-in for the one-shot Show
    // images form.
    Query(query): Query<MessageQuery>,
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

    // Added (TMAIL-372): also fetch the user's folder list so the read view
    // can render the Move-to-folder dropdown options server-side (no JS).
    // A LIST failure here shouldn't block the message render — fall back to
    // an empty list (the template renders the disabled placeholder in that
    // case) so a transient IMAP hiccup doesn't 500 the read view.
    let folder_names: Vec<String> = imap_service
        .list_folders(&username, &password)
        .await
        .map(|fs| fs.into_iter().map(|f| f.name).collect())
        .unwrap_or_else(|e| {
            tracing::warn!(error = ?e, "list_folders failed during classic message render");
            Vec::new()
        });
    let move_targets = super::move_to::build_target_options(&folder_names, &folder);

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

    // Added (TMAIL-386): decide whether this render surfaces real remote
    // <img src=...> URLs or leaves the sanitiser's privacy default in place.
    // Two independent opt-ins compose:
    //   * One-shot: `?show_images=1` on the GET (set by the POST handler
    //     `post_show_images_once` via a 303 redirect — keeps the opt-in
    //     scoped to a single page view, not persisted).
    //   * Persistent: the parsed sender address sits in the user's
    //     `remote_image_allowlist`. We swallow lookup errors so a transient
    //     DB hiccup degrades to "block images" (the privacy-safe direction)
    //     rather than 500'ing the read view.
    let sender_email = parse_sender_email(&from);
    let from_allowlisted_sender = if let Some(addr) = sender_email.as_deref() {
        match remote_image_allowlist::is_allowed(&state.db, mailbox_id, addr).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    error = ?e,
                    "remote_image_allowlist lookup failed; defaulting to block"
                );
                false
            }
        }
    } else {
        false
    };
    let one_shot_show = query.show_images();
    let allow_remote_images = one_shot_show || from_allowlisted_sender;

    // Pre-scan the raw HTML body for any remote <img src=...> so the
    // template can decide whether to render the "[Remote images blocked]"
    // banner. We check the *raw* HTML (not the sanitised output) so the
    // banner stays accurate even when the sanitiser has already rewritten
    // every remote URL to the placeholder.
    let remote_images_present = full
        .html_body
        .as_deref()
        .map(html_has_remote_images)
        .unwrap_or(false);

    let body = build_body(
        full.text_body.as_deref(),
        full.html_body.as_deref(),
        allow_remote_images,
    );
    let attachments = build_attachment_rows(&full.attachments, &folder_href, full.uid);
    // Added (TMAIL-371): resolve the starred state from the message's IMAP
    // flag set so the read view's Star button renders as "Star" vs
    // "Unstar" with the matching `mark=star|unstar` hidden field.
    let is_starred = message_is_starred(&full.flags);

    // Action endpoints — base path shared across every form action /
    // compose link so a future renamer (e.g. /classic/mail/...) only
    // edits one format string.
    let base = format!("/classic/folders/{}/messages/{}", folder_href, full.uid);
    let reply_href = format!("/classic/compose?reply_to={}&folder={}", full.uid, folder_href);
    let reply_all_href = format!("/classic/compose?reply_all={}&folder={}", full.uid, folder_href);
    let forward_href = format!("/classic/compose?forward={}&folder={}", full.uid, folder_href);
    let delete_action = format!("{}/delete", base);
    // Added (TMAIL-370): flag toggle endpoint. The read view renders a
    // single "Mark unread" button — opening the message marks it `\Seen`
    // server-side, so toggling to "read" from here would always be a
    // no-op. The folder view's bulk-action bar covers the "mark read"
    // direction.
    let flag_action = format!("{}/flag", base);
    // Added (TMAIL-372): per-message move endpoint — sibling of `/delete`
    // and `/flag` so the URL layout stays consistent.
    let move_action = format!("{}/move", base);
    // Added (TMAIL-386): two POST endpoints fed by the read-view banner.
    // `/show-images-once` 303-redirects back to the GET handler with
    // `?show_images=1`; `/show-images-always` writes a row to
    // `remote_image_allowlist` keyed on the parsed sender then 303s back.
    let show_images_once_action = format!("{}/show-images-once", base);
    let show_images_always_action = format!("{}/show-images-always", base);

    // Added (TMAIL-384): hydrate the footer quota indicator. Cache-first
    // (see context::load_quota_indicator) — `None` on Redis + DB outage,
    // which the partial silently omits.
    let quota_indicator = super::load_quota_indicator(&state, mailbox_id).await;

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
        flag_action,
        is_starred,
        move_action,
        move_targets,
        csrf_token: session.csrf_token.clone(),
        csp_nonce: csp_nonce.into_string(),
        quota_indicator,
        remote_images_present,
        remote_images_shown: allow_remote_images && remote_images_present,
        remote_images_from_allowlisted_sender: from_allowlisted_sender && remote_images_present,
        // Empty string when the sender header didn't parse — the template
        // hides the "Always show images from this sender" button in that
        // case so we can't submit a junk row.
        allow_sender_address: sender_email.unwrap_or_default(),
        show_images_once_action,
        show_images_always_action,
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

// =====================================================================
// TMAIL-386 — Remote-image opt-in handlers.
// =====================================================================
//
// Two POSTs land on the message read view's banner. Both rely on the
// session + CSRF middleware layered upstream (see
// `handlers::classic::authenticated_router`) — no auth or CSRF work
// happens in these handlers themselves; both just verify session
// presence via `Claims` and 303-redirect back to the GET handler.

/// Form body shape for `show-images-always`. The sender address is rendered
/// into a hidden field on the read view so the POST handler doesn't have to
/// re-fetch the message just to learn who sent it. `serde(default)` so a
/// missing field surfaces as `None` instead of 400'ing — the handler maps
/// `None` to a 400 with a clear message.
#[derive(Debug, Default, Deserialize)]
pub struct ShowImagesAlwaysForm {
    #[serde(default)]
    pub sender: Option<String>,
}

/// POST `/classic/folders/{folder}/messages/{uid}/show-images-once`.
///
/// One-shot opt-in. No DB write — the handler just 303-redirects back to
/// the GET endpoint with `?show_images=1` so the same render that just had
/// images blocked now surfaces the real remote URLs. The next click on a
/// different message lands on the privacy-safe default again.
///
/// CSRF + session are enforced upstream by the classic auth + CSRF
/// middleware (the route is mounted on `authenticated_router`).
pub async fn post_show_images_once(
    Path((folder, uid)): Path<(String, u32)>,
) -> Result<Response, AppError> {
    let folder_href = urlencoding::encode(&folder).into_owned();
    let target = format!(
        "/classic/folders/{}/messages/{}?show_images=1",
        folder_href, uid
    );
    Ok((StatusCode::SEE_OTHER, [(axum::http::header::LOCATION, target)]).into_response())
}

/// POST `/classic/folders/{folder}/messages/{uid}/show-images-always`.
///
/// Persistent opt-in. Parses the submitted sender address (sanity-checked
/// against the same `parse_sender_email` rules that built the hidden field
/// in the first place — defence-in-depth, so a tampered form can't sneak a
/// malformed row past `remote_image_allowlist::allow_sender`), then UPSERTs
/// a row in `remote_image_allowlist`. 303-redirects back to the GET
/// handler so the next render of THIS message — and every future message
/// from this sender — surfaces the real remote URLs.
///
/// Returns a 400 if the form doesn't carry a parseable sender (would
/// indicate either a tampered form OR a sender that the original render
/// couldn't parse either — in which case we shouldn't have rendered the
/// "Always allow" button to begin with).
pub async fn post_show_images_always(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((folder, uid)): Path<(String, u32)>,
    Form(form): Form<ShowImagesAlwaysForm>,
) -> Result<Response, AppError> {
    let mailbox_id: Uuid = claims.sub.parse().map_err(|_| {
        AppError::Internal(anyhow::anyhow!("Invalid mailbox ID in classic claims"))
    })?;
    let raw = form.sender.unwrap_or_default();
    let parsed = parse_sender_email(&raw).ok_or_else(|| {
        AppError::BadRequest(
            "Cannot allow images: the message's From header did not parse to a valid \
             email address. Use the one-shot \"Show images\" button instead."
                .to_string(),
        )
    })?;
    // Idempotent upsert — re-clicking the button on a sender that's already
    // allowlisted is a safe no-op. We don't surface a "Sender was already
    // allowed" banner today; the post-303 GET silently shows images either
    // way, which is the right UX.
    remote_image_allowlist::allow_sender(&state.db, mailbox_id, &parsed).await?;

    let folder_href = urlencoding::encode(&folder).into_owned();
    // We don't tack on `?show_images=1` here — the persistent allowlist
    // entry will make the GET render surface real images on its own. (We
    // also redirect without the query so a future "Forget this sender"
    // action lands on the privacy-safe view the moment the row is gone.)
    let target = format!("/classic/folders/{}/messages/{}", folder_href, uid);
    Ok((StatusCode::SEE_OTHER, [(axum::http::header::LOCATION, target)]).into_response())
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
            flag_action: "/classic/folders/INBOX/messages/42/flag".to_string(),
            is_starred: false,
            move_action: "/classic/folders/INBOX/messages/42/move".to_string(),
            // Default fixture surfaces a typical 3-folder dropdown so the
            // template's "render options" branch is the exercised path; the
            // empty-targets branch is locked down by its own dedicated test.
            move_targets: vec![
                "Drafts".to_string(),
                "Sent".to_string(),
                "Archive".to_string(),
            ],
            csrf_token: "test-csrf-token".to_string(),
            csp_nonce: "test-nonce-fixed".to_string(),
            quota_indicator: None,
            // TMAIL-386 defaults — every existing test exercises the
            // "no remote images at all" path so the banner doesn't render
            // and the existing assertions still hold. Banner-specific
            // tests below toggle these flags explicitly.
            remote_images_present: false,
            remote_images_shown: false,
            remote_images_from_allowlisted_sender: false,
            allow_sender_address: String::new(),
            show_images_once_action: "/classic/folders/INBOX/messages/42/show-images-once"
                .to_string(),
            show_images_always_action: "/classic/folders/INBOX/messages/42/show-images-always"
                .to_string(),
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
        let body = build_body(Some("plain alt"), Some("<p>HTML</p>"), false);
        assert!(body.is_html());
        assert!(body.html().contains("<p>HTML</p>"));
    }

    #[test]
    fn build_body_falls_back_to_text_when_html_missing() {
        let body = build_body(Some("plain only"), None, false);
        assert!(body.is_text());
        assert_eq!(body.text(), "plain only");
    }

    #[test]
    fn build_body_falls_back_to_text_when_html_empty() {
        let body = build_body(Some("plain"), Some("   "), false);
        assert!(body.is_text());
    }

    #[test]
    fn build_body_returns_empty_when_neither_present() {
        let body = build_body(None, None, false);
        assert!(body.is_empty());
    }

    #[test]
    fn build_body_sanitises_html_strips_script() {
        let body = build_body(None, Some("<p>ok</p><script>alert('xss')</script>"), false);
        assert!(body.is_html());
        let html = body.html();
        assert!(!html.contains("<script"));
        assert!(!html.contains("alert"));
        assert!(html.contains("<p>ok</p>"));
    }

    #[test]
    fn build_body_sanitises_html_strips_onload() {
        let body = build_body(None, Some(r#"<body onload="evil()">x</body>"#), false);
        assert!(body.is_html());
        assert!(!body.html().contains("onload"));
    }

    // ----- TMAIL-386: build_body honours allow_remote_images -----

    #[test]
    fn build_body_blocks_remote_image_when_allow_is_false() {
        // Default (privacy-aware) path. The sanitiser rewrites the remote
        // src to its 1×1 placeholder; the original URL must not survive.
        let body = build_body(
            None,
            Some(r#"<p>hi</p><img src="https://tracker.example.com/x.gif">"#),
            false,
        );
        assert!(body.is_html());
        assert!(!body.html().contains("tracker.example.com"));
    }

    #[test]
    fn build_body_surfaces_remote_image_when_allow_is_true() {
        // The Show-images opt-in path (TMAIL-386). With allow=true the
        // sanitiser leaves the remote URL in place.
        let body = build_body(
            None,
            Some(r#"<p>hi</p><img src="https://example.com/banner.png">"#),
            true,
        );
        assert!(body.is_html());
        assert!(
            body.html().contains("https://example.com/banner.png"),
            "remote URL should survive when allow_remote_images=true: {}",
            body.html()
        );
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
    fn message_template_no_longer_renders_disabled_action_controls() {
        // Updated for TMAIL-372 — Move-to-folder is now LIVE alongside
        // Star (TMAIL-371) and Mark unread (TMAIL-370). When the fresh
        // template fixture surfaces 3 targets, every action <button> and
        // <select> in the action row must render WITHOUT the `disabled`
        // attribute. The only remaining `disabled` attribute is the
        // `<option value="" selected disabled>Move to…</option>`
        // placeholder INSIDE the live select — that's the standard
        // pattern for a required-select-an-option dropdown, not a
        // deferred-action placeholder.
        let body = fresh_template().render().expect("template renders");
        // Slice out the action row `<div class="msg-actions" …>` — the
        // template's <style> block also references `.msg-actions` selectors
        // that include the `disabled` pseudo-class, so a naive substring
        // find on `"msg-actions"` would land in the CSS and trip the
        // assertion. Look for the opening `<div` tag explicitly.
        let actions_at = body
            .find("<div class=\"msg-actions\"")
            .expect("action row <div> missing");
        let actions_end = body[actions_at..]
            .find("</div>")
            .map(|i| actions_at + i)
            .unwrap_or(body.len());
        let actions = &body[actions_at..actions_end];
        // The select-prompt option counts as one expected `disabled`
        // attribute (it's the "select a value" placeholder pattern, not a
        // deferred-action placeholder). Slice it out before counting.
        // The template source uses the HTML entity `&hellip;`, which is
        // what shows up in the rendered output — not the Unicode `…`.
        let actions_without_prompt = actions.replace(
            "<option value=\"\" selected disabled>Move to&hellip;</option>",
            "",
        );
        assert!(
            !actions_without_prompt.contains("disabled"),
            "no action control in the row should be disabled after TMAIL-372: {actions}"
        );
        // Move <select> must still render so the layout matches the
        // gap-analysis spec.
        assert!(
            actions.contains("id=\"move-target\""),
            "Move-to-folder <select> missing from action row: {actions}"
        );
    }

    // ----- TMAIL-371: Star toggle is now live -----

    #[test]
    fn message_template_renders_live_star_form_for_unstarred_message() {
        // The Star button is no longer a placeholder. For an unstarred
        // message it must render labelled "Star", POST to the flag
        // endpoint, carry `mark=star` + the session CSRF token, and
        // MUST NOT be disabled.
        let mut t = fresh_template();
        t.is_starred = false;
        let body = t.render().expect("template renders");
        assert!(
            body.contains(">Star<"),
            "Star button label must render for an unstarred message: {body}"
        );
        // Locate the Star button and check the surrounding form.
        let star_at = body
            .find(">Star<")
            .expect("Star button must render");
        let form_start = body[..star_at]
            .rfind("<form")
            .expect("Star button must sit inside a <form>");
        let form_segment = &body[form_start..star_at + 20];
        assert!(
            form_segment.contains("action=\"/classic/folders/INBOX/messages/42/flag\""),
            "Star form must POST to the flag endpoint: {form_segment}"
        );
        assert!(
            form_segment.contains("name=\"_csrf\" value=\"test-csrf-token\""),
            "Star form must carry the session csrf_token: {form_segment}"
        );
        assert!(
            form_segment.contains("name=\"mark\" value=\"star\""),
            "Star form must include the mark=star hidden field: {form_segment}"
        );
        assert!(
            !form_segment.contains("disabled"),
            "Star button must NOT be disabled: {form_segment}"
        );
    }

    #[test]
    fn message_template_renders_live_unstar_form_for_starred_message() {
        // For an already-starred message, the same button flips to
        // "Unstar" and posts `mark=unstar` so toggling it back works as
        // expected. The form action stays the same — only the label and
        // hidden field change.
        let mut t = fresh_template();
        t.is_starred = true;
        let body = t.render().expect("template renders");
        assert!(
            body.contains(">Unstar<"),
            "Unstar button label must render for a starred message: {body}"
        );
        // The standalone "Star" word must NOT leak through — `>Star<` would
        // mean the form is rendering the wrong label for a starred message.
        // The closing-tag form `>Star<` is the precise needle the unstarred
        // branch uses, so checking it here catches a swapped if/else block.
        assert!(
            !body.contains(">Star<"),
            "Star label must NOT render on a starred message — only Unstar: {body}"
        );
        let unstar_at = body
            .find(">Unstar<")
            .expect("Unstar button must render");
        let form_start = body[..unstar_at]
            .rfind("<form")
            .expect("Unstar button must sit inside a <form>");
        let form_segment = &body[form_start..unstar_at + 20];
        assert!(
            form_segment.contains("action=\"/classic/folders/INBOX/messages/42/flag\""),
            "Unstar form must POST to the flag endpoint: {form_segment}"
        );
        assert!(
            form_segment.contains("name=\"mark\" value=\"unstar\""),
            "Unstar form must include the mark=unstar hidden field: {form_segment}"
        );
        assert!(
            !form_segment.contains("disabled"),
            "Unstar button must NOT be disabled: {form_segment}"
        );
    }

    // ----- TMAIL-371: message_is_starred helper -----

    #[test]
    fn message_is_starred_recognises_flagged_variants() {
        // Same flag-spelling variance the read-detection helper tolerates:
        // bare "Flagged" (async-imap Debug shape), "\\Flagged" (RFC 3501),
        // and Gmail's `$Starred` keyword.
        assert!(message_is_starred(&["Flagged".to_string()]));
        assert!(message_is_starred(&["\\Flagged".to_string()]));
        assert!(message_is_starred(&["FLAGGED".to_string()]));
        assert!(message_is_starred(&["$Starred".to_string()]));
        assert!(message_is_starred(&[
            "Seen".to_string(),
            "\\Flagged".to_string()
        ]));
    }

    #[test]
    fn message_is_starred_returns_false_when_flag_absent() {
        assert!(!message_is_starred(&[]));
        assert!(!message_is_starred(&["Seen".to_string()]));
        assert!(!message_is_starred(&["Recent".to_string()]));
        assert!(!message_is_starred(&["\\Seen".to_string(), "Recent".to_string()]));
    }

    // ----- TMAIL-370: Mark unread is now live -----

    #[test]
    fn message_template_renders_live_mark_unread_form() {
        // The Mark unread button has flipped from placeholder to live —
        // it POSTs to the flag endpoint with `mark=unread` so the handler
        // strips `\Seen` from the message. The form must carry the
        // session CSRF token, must NOT be disabled, and must point at
        // the flag_action URL the handler builds.
        let body = fresh_template().render().expect("template renders");
        assert!(
            body.contains("action=\"/classic/folders/INBOX/messages/42/flag\""),
            "Mark unread form action must point at the flag endpoint: {body}"
        );
        // Locate the Mark unread button text and verify the form around
        // it is fully wired (csrf + mark=unread hidden + not disabled).
        let mark_at = body
            .find(">Mark unread<")
            .expect("Mark unread button must render");
        // Walk backwards to the opening <form so we can scan the whole
        // form block for hidden fields + disabled attr.
        let form_start = body[..mark_at]
            .rfind("<form")
            .expect("Mark unread button must sit inside a <form>");
        let form_segment = &body[form_start..mark_at + 50];
        assert!(
            form_segment.contains("name=\"_csrf\" value=\"test-csrf-token\""),
            "Mark unread form must carry the session csrf_token: {form_segment}"
        );
        assert!(
            form_segment.contains("name=\"mark\" value=\"unread\""),
            "Mark unread form must include the mark=unread hidden field: {form_segment}"
        );
        assert!(
            !form_segment.contains("disabled"),
            "Mark unread button must NOT be disabled: {form_segment}"
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

    // ----- TMAIL-372: Move-to-folder dropdown -----

    #[test]
    fn message_template_renders_move_dropdown_when_targets_populated() {
        // fresh_template() ships 3 targets (Drafts, Sent, Archive). The
        // read-view action row must render the form with all three as
        // <option> entries AND a live Submit button.
        let body = fresh_template().render().expect("template renders");
        assert!(
            body.contains("id=\"move-target\""),
            "move <select> missing: {body}"
        );
        assert!(
            body.contains("name=\"target\""),
            "move <select> must carry name=\"target\": {body}"
        );
        assert!(
            body.contains("action=\"/classic/folders/INBOX/messages/42/move\""),
            "move form must POST to the /move endpoint: {body}"
        );
        for name in ["Drafts", "Sent", "Archive"] {
            assert!(
                body.contains(&format!("<option value=\"{name}\">{name}</option>")),
                "target {name:?} option missing from dropdown: {body}"
            );
        }
        // The <select> must NOT be disabled when targets exist.
        // Slice out the <select> opening tag and assert.
        let select_at = body.find("id=\"move-target\"").expect("select renders");
        let tag_start = body[..select_at].rfind("<select").expect("opening tag");
        let tag_end = body[tag_start..]
            .find('>')
            .map(|i| tag_start + i + 1)
            .expect("closing >");
        let select_tag = &body[tag_start..tag_end];
        assert!(
            !select_tag.contains("disabled"),
            "<select> must NOT be disabled when targets exist: {select_tag}"
        );
        // The CSRF token must thread through the form so the canonical
        // middleware accepts the submission.
        assert!(
            body.contains("name=\"_csrf\" value=\"test-csrf-token\""),
            "move form must carry the session csrf_token: {body}"
        );
    }

    #[test]
    fn message_template_disables_move_dropdown_when_no_targets() {
        // Edge case — a fresh account on a server with only INBOX. The
        // form renders as disabled placeholder + a hint so the layout
        // doesn't shift when the user lands here for the first time.
        let mut t = fresh_template();
        t.move_targets = vec![];
        let body = t.render().expect("template renders");
        assert!(
            body.contains("No folders to move to"),
            "empty-state copy missing on read-view move dropdown: {body}"
        );
        // The submit button must NOT render when there's nothing to move
        // to — clicking a disabled select isn't intuitive, and rendering
        // an enabled submit on an empty form would just lead to a 400.
        // (The {% else %} branch is the only one that emits <button>.)
        assert!(
            !body.contains(">Move<"),
            "Move submit button must NOT render when no targets: {body}"
        );
    }

    // ===== TMAIL-386: parse_sender_email helper =====

    #[test]
    fn parse_sender_email_extracts_address_from_display_form() {
        assert_eq!(
            parse_sender_email("Alice <alice@example.com>").as_deref(),
            Some("alice@example.com")
        );
        assert_eq!(
            parse_sender_email("\"Bob B.\" <bob@example.com>").as_deref(),
            Some("bob@example.com")
        );
    }

    #[test]
    fn parse_sender_email_handles_bare_address() {
        assert_eq!(
            parse_sender_email("alice@example.com").as_deref(),
            Some("alice@example.com")
        );
        assert_eq!(
            parse_sender_email("  alice@example.com  ").as_deref(),
            Some("alice@example.com")
        );
    }

    #[test]
    fn parse_sender_email_returns_none_for_unparseable_input() {
        assert!(parse_sender_email("").is_none());
        assert!(parse_sender_email("Alice").is_none());
        assert!(parse_sender_email("not-an-email").is_none());
        // Missing domain dot — most providers require at least one,
        // and our writer / read paths use the lookup key as opaque so a
        // junk `alice@localhost` row would never get a real match.
        assert!(parse_sender_email("alice@localhost").is_none());
        // Missing local-part / domain.
        assert!(parse_sender_email("@example.com").is_none());
        assert!(parse_sender_email("alice@").is_none());
    }

    #[test]
    fn parse_sender_email_preserves_plus_addressing() {
        assert_eq!(
            parse_sender_email("Alice <alice+lists@example.com>").as_deref(),
            Some("alice+lists@example.com")
        );
    }

    // ===== TMAIL-386: MessageQuery -> show_images flag =====

    #[test]
    fn message_query_recognises_truthy_values() {
        let mut q = MessageQuery {
            show_images: Some("1".to_string()),
        };
        assert!(q.show_images());
        q.show_images = Some("true".to_string());
        assert!(q.show_images());
        q.show_images = Some("yes".to_string());
        assert!(q.show_images());
    }

    #[test]
    fn message_query_rejects_anything_else() {
        assert!(!MessageQuery::default().show_images());
        assert!(!MessageQuery {
            show_images: Some("0".to_string()),
        }
        .show_images());
        assert!(!MessageQuery {
            show_images: Some("".to_string()),
        }
        .show_images());
        assert!(!MessageQuery {
            show_images: Some("nope".to_string()),
        }
        .show_images());
    }

    // ===== TMAIL-386: banner rendering branches =====

    #[test]
    fn template_renders_no_banner_when_no_remote_images_present() {
        // The fresh fixture has remote_images_present=false; neither
        // banner state should render and the body sits directly under
        // the header block as it did before TMAIL-386. We assert on the
        // rendered label span and the form action (not a freeform
        // substring) so the leak-free CSS comment that mentions both
        // banner copies doesn't trip this test.
        let body = fresh_template().render().expect("template renders");
        assert!(
            !body.contains(">[Remote images blocked]<"),
            "blocked-images banner label must NOT render when no remote images are present: {body}"
        );
        assert!(
            !body.contains(">[Remote images shown]<"),
            "shown-images banner label must NOT render when no remote images are present: {body}"
        );
        assert!(
            !body.contains("show-images-once"),
            "show-images-once form action must NOT render when no remote images are present: {body}"
        );
        assert!(
            !body.contains("show-images-always"),
            "show-images-always form action must NOT render when no remote images are present: {body}"
        );
    }

    #[test]
    fn template_renders_blocked_banner_with_show_once_button_when_images_present_and_blocked() {
        let mut t = fresh_template();
        t.remote_images_present = true;
        t.remote_images_shown = false;
        t.allow_sender_address = "alice@example.com".to_string();
        let body = t.render().expect("template renders");
        assert!(
            body.contains(">[Remote images blocked]<"),
            "blocked-images banner label must render: {body}"
        );
        // The one-shot "Show images" form must POST to the show-images-once
        // endpoint and carry the session CSRF token.
        assert!(
            body.contains(
                "action=\"/classic/folders/INBOX/messages/42/show-images-once\""
            ),
            "show-images-once form action missing: {body}"
        );
        // The button label is the user-facing "Show images" copy.
        assert!(
            body.contains(">Show images<"),
            "Show images button missing: {body}"
        );
        // Locate the show-images-once form and verify it carries the csrf
        // hidden field.
        let action_at = body
            .find("action=\"/classic/folders/INBOX/messages/42/show-images-once\"")
            .expect("show-images-once action present");
        let form_start = body[..action_at]
            .rfind("<form")
            .expect("show-images-once must sit inside a <form>");
        let form_end = body[form_start..]
            .find("</form>")
            .map(|i| form_start + i)
            .expect("show-images-once form closing tag");
        let form_segment = &body[form_start..form_end];
        assert!(
            form_segment.contains("name=\"_csrf\" value=\"test-csrf-token\""),
            "show-images-once form must carry the session csrf_token: {form_segment}"
        );
    }

    #[test]
    fn template_renders_always_allow_button_when_sender_parsed() {
        // When the parsed sender is non-empty, the "Always show images from
        // this sender" form must render too — with the sender as a hidden
        // field so the POST handler doesn't have to re-fetch.
        let mut t = fresh_template();
        t.remote_images_present = true;
        t.remote_images_shown = false;
        t.allow_sender_address = "alice@example.com".to_string();
        let body = t.render().expect("template renders");
        assert!(
            body.contains(
                "action=\"/classic/folders/INBOX/messages/42/show-images-always\""
            ),
            "show-images-always form action missing: {body}"
        );
        assert!(
            body.contains(">Always show images from this sender<"),
            "Always show button missing: {body}"
        );
        assert!(
            body.contains("name=\"sender\" value=\"alice@example.com\""),
            "hidden sender field missing or wrong: {body}"
        );
    }

    #[test]
    fn template_omits_always_allow_button_when_sender_unparsed() {
        // Edge case — a From header that didn't parse to a real address
        // (display-name-only sender, malformed RFC 5322, etc.). The
        // one-shot Show images button still renders so the user can opt in
        // for this view, but the Always-allow button MUST NOT render — we
        // have nothing valid to persist.
        let mut t = fresh_template();
        t.remote_images_present = true;
        t.remote_images_shown = false;
        t.allow_sender_address = String::new();
        let body = t.render().expect("template renders");
        assert!(
            body.contains(">[Remote images blocked]<"),
            "blocked-images banner must still render: {body}"
        );
        assert!(
            body.contains(">Show images<"),
            "one-shot Show images must still render: {body}"
        );
        assert!(
            !body.contains("show-images-always"),
            "Always-allow form must NOT render when sender unparsed: {body}"
        );
        assert!(
            !body.contains(">Always show images from this sender<"),
            "Always-allow button label must NOT render when sender unparsed: {body}"
        );
    }

    #[test]
    fn template_renders_muted_shown_banner_when_images_are_already_shown_one_shot() {
        // remote_images_shown=true with allowlisted_sender=false means the
        // user clicked the one-shot Show images button. The muted banner
        // says "One-time only — this view." so the user always knows why
        // images are surfacing.
        let mut t = fresh_template();
        t.remote_images_present = true;
        t.remote_images_shown = true;
        t.remote_images_from_allowlisted_sender = false;
        let body = t.render().expect("template renders");
        assert!(body.contains(">[Remote images shown]<"), "shown banner label missing: {body}");
        assert!(
            body.contains("One-time only"),
            "one-shot copy missing in shown banner: {body}"
        );
        // The two opt-in forms MUST NOT render — they belong to the
        // blocked-state branch.
        assert!(
            !body.contains("show-images-once"),
            "show-images-once form must NOT render in shown state: {body}"
        );
        assert!(
            !body.contains("show-images-always"),
            "show-images-always form must NOT render in shown state: {body}"
        );
    }

    #[test]
    fn template_renders_shown_banner_with_allowlist_note_when_sender_is_persisted() {
        let mut t = fresh_template();
        t.remote_images_present = true;
        t.remote_images_shown = true;
        t.remote_images_from_allowlisted_sender = true;
        let body = t.render().expect("template renders");
        assert!(body.contains(">[Remote images shown]<"));
        assert!(
            body.contains("always-allow list"),
            "allowlisted-sender copy missing in shown banner: {body}"
        );
        // The one-time-only copy MUST NOT leak into this branch — that
        // would confuse the user about why images are surfacing.
        assert!(
            !body.contains("One-time only"),
            "one-time-only copy must NOT render when sender is allowlisted: {body}"
        );
    }

    #[test]
    fn template_html_escapes_sender_address_in_hidden_field() {
        // Defence in depth — the address comes from `parse_sender_email`
        // which sanity-checks the shape, but a hostile From header could
        // still smuggle weird characters. Lock auto-escape on so a future
        // bug can't leak a raw payload into the hidden field's value.
        let mut t = fresh_template();
        t.remote_images_present = true;
        t.allow_sender_address = "alice@\"x.example.com".to_string();
        let body = t.render().expect("template renders");
        assert!(!body.contains("alice@\"x.example.com"));
        assert!(
            body.contains("alice@&quot;x.example.com")
                || body.contains("alice@&#34;x.example.com"),
            "sender value must be HTML-escaped: {body}"
        );
    }

    // ===== TMAIL-386: ShowImagesAlwaysForm =====

    #[test]
    fn show_images_always_form_default_is_none() {
        // serde(default) means a missing `sender` field deserialises as
        // None rather than 400'ing the request, so the handler can map it
        // to a friendlier 400 with an actionable error message.
        let f = ShowImagesAlwaysForm::default();
        assert!(f.sender.is_none());
    }

    #[test]
    fn message_template_html_escapes_move_target_option() {
        // Defence in depth — IMAP folder names ultimately come from the
        // server. Lock auto-escape on so a hostile name can't escape into
        // raw markup inside the dropdown options.
        let mut t = fresh_template();
        t.move_targets = vec!["<img onerror=alert(1) src=x>".to_string()];
        let body = t.render().expect("template renders");
        assert!(!body.contains("<img onerror=alert(1)"));
        assert!(
            body.contains("&#60;img") || body.contains("&lt;img"),
            "move-target name must be HTML-escaped in <option>: {body}"
        );
    }
}
