// Added (TMAIL-366): GET + POST /classic/compose — compose form for the
// no-JS Classic UI surface (driver TMAIL-299, gap-analysis
// `docs/gap-analysis/classic-ui.md` P0 #12).
//
// What this owns
// --------------
//   * GET /classic/compose                            → blank compose form
//   * GET /classic/compose?reply_to=<uid>&folder=<f>  → prefilled reply
//   * GET /classic/compose?reply_all=<uid>&folder=<f> → prefilled reply-all
//   * GET /classic/compose?forward=<uid>&folder=<f>   → prefilled forward
//   * POST /classic/compose (multipart/form-data)     → send via BYOK SMTP
//
// The form posts as `multipart/form-data` so attachments ride alongside the
// text fields. Plain-text body only — rich-text composition is N/A on a
// no-JS surface, and downstream mail clients render plain text consistently.
//
// On a successful POST we 303-redirect to the source folder
// (`/classic/folders/{folder}?sent=1`) so a browser refresh of the landing
// page never re-submits the form (POST-Redirect-Get).
//
// On a failed POST we re-render the form with the validation error AND the
// user's prior input preserved — except the attachment bytes, which the
// browser doesn't round-trip on a no-JS form. Filenames are echoed so the
// user can see which files they had attached and re-attach them; that's
// the accepted no-JS UX cost.
//
// Total body+attachments size cap
// -------------------------------
// Spec is 25 MB total (gap-analysis P0 #12). We enforce it in `post_compose`
// after the multipart parse. The CSRF middleware buffers up to 32 MB so a
// just-over-25-MB body still reaches the handler and gets a friendly error
// rather than a generic 413; anything bigger 403s with "form data couldn't
// be read" via the CSRF middleware, which is also acceptable.
//
// Per-file size cap is deliberately out of scope here — it's P1 #28
// (TMAIL-381) along with the rich Content-Type allow/block list.

use askama::Template;
use axum::{
    extract::{Multipart, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::AppError;
use crate::services::auth_service::Claims;
use crate::services::imap_service::ImapService;
use crate::services::smtp_service::{OutgoingAttachment, SendRequest, SmtpService};
use crate::state::AppState;
use crate::validation::{self, MAX_MESSAGE_BODY_LEN};

use super::CspNonce;

/// Hard cap on `body` + sum-of-attachment-sizes. Per gap-analysis P0 #12.
/// Surfaces as a friendly "Total upload too large" form-level error rather
/// than the generic 413 a body-limit layer would emit.
pub const MAX_TOTAL_BYTES: usize = 25 * 1024 * 1024;

/// Default name shown in the attachment-row echo when a browser sends a
/// file with no filename header (rare — most browsers include it). Kept
/// distinct from the read-view's `(unnamed attachment)` placeholder so
/// future log-greps can tell them apart.
const DEFAULT_FILENAME: &str = "attachment";

/// Maximum length we'll echo back into the To / Cc / Bcc / Subject fields
/// on a re-render after failure. Matches what `validation::MAX_BODY_BYTES`
/// allows the body to be (~25 MB) — but the address / subject inputs
/// realistically never approach this; this is just a defensive cap so a
/// hostile client can't make the re-render OOM the renderer.
const MAX_PREFILL_HEADER_LEN: usize = 32 * 1024;

/// Query string for `GET /classic/compose`.
///
/// All four fields are optional and mutually orthogonal. A request with
/// more than one of `reply_to` / `reply_all` / `forward` set takes them in
/// that precedence order so a malformed bookmark still renders something.
#[derive(Debug, Deserialize, Default)]
pub struct ComposeQuery {
    #[serde(default)]
    pub reply_to: Option<u32>,
    #[serde(default)]
    pub reply_all: Option<u32>,
    #[serde(default)]
    pub forward: Option<u32>,
    /// Source folder for the prefill fetch AND for the post-send redirect.
    /// Defaults to INBOX when missing.
    #[serde(default)]
    pub folder: Option<String>,
}

impl ComposeQuery {
    fn folder_or_default(&self) -> String {
        self.folder
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "INBOX".to_string())
    }

    /// Decode the precedence: reply_to > reply_all > forward. Returns the
    /// resolved `(uid, mode)` for the prefill fetch — `None` for a blank
    /// compose. Pulled out so the unit tests don't have to construct a
    /// ComposeQuery just to check the precedence.
    fn prefill_target(&self) -> Option<(u32, PrefillMode)> {
        if let Some(uid) = self.reply_to {
            return Some((uid, PrefillMode::Reply));
        }
        if let Some(uid) = self.reply_all {
            return Some((uid, PrefillMode::ReplyAll));
        }
        if let Some(uid) = self.forward {
            return Some((uid, PrefillMode::Forward));
        }
        None
    }
}

/// Three prefill flavours the compose form supports. Drives both header
/// shape (Reply / Reply-All / Forward) AND the quoted-body prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefillMode {
    Reply,
    ReplyAll,
    Forward,
}

/// Askama template struct backing `templates/classic/compose.html`.
///
/// Field names match the template `{{ var }}` placeholders exactly —
/// Askama validates this at compile time so a rename here without the
/// matching template edit fails `cargo build`.
#[derive(Template)]
#[template(path = "classic/compose.html")]
pub struct ComposeTemplate {
    /// To/Cc/Bcc as comma-separated strings (display form). The POST
    /// handler splits on `,` so round-tripping the raw user input is the
    /// right echo behaviour.
    pub to: String,
    pub cc: String,
    pub bcc: String,
    /// Subject — prefilled with `Re: ` / `Fwd: ` on reply/forward.
    pub subject: String,
    /// Plain-text body — prefilled with the quoted original on reply/
    /// forward, empty on a blank compose.
    pub body: String,
    /// Threading headers stamped on the outbound message; rendered as
    /// hidden inputs so they round-trip across a re-render on failure.
    /// Empty when this is a fresh compose (no thread).
    pub in_reply_to: String,
    /// Whitespace-separated chain of `<message-id>` tokens. Hidden input.
    pub references: String,
    /// Folder we navigated FROM. Used by the success redirect (to land
    /// the user back on the inbox they started from) and by the cancel
    /// link. Always non-empty (defaults to `INBOX`).
    pub source_folder: String,
    /// URL-encoded variant for use inside hrefs.
    pub source_folder_href: String,
    /// On a re-render after failure, the original filenames the user
    /// attached. Echoed for transparency — the bytes can't be round-
    /// tripped on a no-JS form, so the user has to re-attach if they
    /// resubmit. Empty list on a fresh GET / successful POST.
    pub prior_attachment_names: Vec<String>,
    /// Form-level error message rendered above the form. `None` on a
    /// successful GET.
    pub error: Option<String>,
    /// Session CSRF token. Required on every Classic UI form per the
    /// canonical CSRF middleware.
    pub csrf_token: String,
    /// Per-request CSP nonce. Required by base.html (TMAIL-356).
    pub csp_nonce: String,
}

impl ComposeTemplate {
    /// Blank-state factory used by GET /classic/compose on a fresh load.
    ///
    /// Changed (TMAIL-368): takes the per-request CSP nonce by argument so
    /// the inline `<style nonce="…">` on base.html matches the strict
    /// `/classic/*` response CSP header. Callers pull the nonce from
    /// `req.extensions().get::<CspNonce>()` via the `axum::Extension<CspNonce>`
    /// extractor.
    pub fn fresh(
        source_folder: impl Into<String>,
        csrf_token: impl Into<String>,
        csp_nonce: impl Into<String>,
    ) -> Self {
        let folder = source_folder.into();
        let href = urlencoding::encode(&folder).into_owned();
        Self {
            to: String::new(),
            cc: String::new(),
            bcc: String::new(),
            subject: String::new(),
            body: String::new(),
            in_reply_to: String::new(),
            references: String::new(),
            source_folder: folder,
            source_folder_href: href,
            prior_attachment_names: vec![],
            error: None,
            csrf_token: csrf_token.into(),
            csp_nonce: csp_nonce.into(),
        }
    }
}

/// Build the `Re: ` / `Fwd: ` subject for a reply/forward. Idempotent —
/// double-replying doesn't compound the prefix (matches every mail client
/// in the wild).
pub fn build_reply_subject(original: &str, mode: PrefillMode) -> String {
    let trimmed = original.trim();
    let prefix = match mode {
        PrefillMode::Reply | PrefillMode::ReplyAll => "Re: ",
        PrefillMode::Forward => "Fwd: ",
    };
    let lower = trimmed.to_ascii_lowercase();
    // Already prefixed? Don't compound. Match both the literal prefix and
    // the common variant ("re:" without space, "RE:") — case-insensitive.
    let already = match mode {
        PrefillMode::Reply | PrefillMode::ReplyAll => {
            lower.starts_with("re:") || lower.starts_with("re :")
        }
        PrefillMode::Forward => {
            lower.starts_with("fwd:")
                || lower.starts_with("fw:")
                || lower.starts_with("fwd :")
                || lower.starts_with("fw :")
        }
    };
    if already {
        trimmed.to_string()
    } else if trimmed.is_empty() {
        // (no subject) is the read-view's stand-in; preserve it as a
        // suffix so a re-thread that originated with a blank subject is
        // still recognisable.
        format!("{prefix}(no subject)")
    } else {
        format!("{prefix}{trimmed}")
    }
}

/// Build the quoted-body prefix for a reply/forward. Format mirrors what
/// every other mail client does — `> ` line prefix for replies, a
/// `---------- Forwarded message ----------` banner for forwards. Pure
/// function so unit tests can exercise the wrapping without IMAP.
pub fn build_quoted_body(
    from: Option<&str>,
    date: Option<&str>,
    text_body: Option<&str>,
    mode: PrefillMode,
) -> String {
    let body = text_body.unwrap_or("").trim_end();
    if body.is_empty() {
        return String::new();
    }
    match mode {
        PrefillMode::Reply | PrefillMode::ReplyAll => {
            let attribution = match (from, date) {
                (Some(f), Some(d)) => format!("On {}, {} wrote:\n", d, f),
                (Some(f), None) => format!("{} wrote:\n", f),
                (None, Some(d)) => format!("On {} someone wrote:\n", d),
                (None, None) => String::new(),
            };
            let quoted = body
                .lines()
                .map(|l| format!("> {l}"))
                .collect::<Vec<_>>()
                .join("\n");
            format!("\n\n{attribution}{quoted}\n")
        }
        PrefillMode::Forward => {
            let mut buf = String::from("\n\n---------- Forwarded message ----------\n");
            if let Some(f) = from {
                buf.push_str(&format!("From: {f}\n"));
            }
            if let Some(d) = date {
                buf.push_str(&format!("Date: {d}\n"));
            }
            buf.push_str("\n");
            buf.push_str(body);
            buf.push('\n');
            buf
        }
    }
}

/// Build the `References:` chain for a reply. Per RFC 5322 §3.6.4 the new
/// header is the EXISTING References list (or just the original
/// `In-Reply-To` if no chain existed) with the original message-id
/// appended. Whitespace-separated.
///
/// Pulled out into a pure function so the unit tests don't have to spin up
/// IMAP to verify the chain shape.
pub fn build_references_chain(
    original_message_id: Option<&str>,
    original_references: &[String],
    original_in_reply_to: Option<&str>,
) -> String {
    // Build candidate chain: existing References ++ original Message-ID.
    // Fall back to `In-Reply-To` when the original had no References (matches
    // Thunderbird / Gmail).
    let mut chain: Vec<String> = original_references
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if chain.is_empty() {
        if let Some(irt) = original_in_reply_to {
            let t = irt.trim();
            if !t.is_empty() {
                chain.push(t.to_string());
            }
        }
    }
    if let Some(mid) = original_message_id {
        let t = mid.trim();
        if !t.is_empty() && !chain.iter().any(|x| x == t) {
            chain.push(t.to_string());
        }
    }
    chain.join(" ")
}

/// Compute the To / Cc lists for a Reply-All. Per RFC 5322 the reply
/// addresses are the original `From` (as the new To) plus the original
/// `To` + `Cc` (as the new Cc), MINUS the current user's own address so we
/// don't email ourselves. Pulled out for testability — the IMAP fetch
/// gives us the raw envelope and this stays a pure function.
pub fn build_reply_all_recipients(
    original_from: Option<&str>,
    original_to: &[String],
    original_cc: &[String],
    self_email: &str,
) -> (Vec<String>, Vec<String>) {
    let self_lower = self_email.trim().to_ascii_lowercase();
    let is_self = |addr: &str| address_email(addr).eq_ignore_ascii_case(&self_lower);

    let to: Vec<String> = original_from
        .map(|f| vec![f.to_string()])
        .unwrap_or_default()
        .into_iter()
        .filter(|a| !is_self(a))
        .collect();

    let cc: Vec<String> = original_to
        .iter()
        .chain(original_cc.iter())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .filter(|a| !is_self(a))
        .collect();

    (to, cc)
}

/// Extract just the email part from a `"Display Name <addr@host>"` style
/// header value, so the self-address filter doesn't get confused by the
/// display name. Falls back to the trimmed input when no `<…>` is present.
fn address_email(addr: &str) -> String {
    let trimmed = addr.trim();
    if let Some(lt) = trimmed.rfind('<') {
        if let Some(gt) = trimmed[lt + 1..].find('>') {
            return trimmed[lt + 1..lt + 1 + gt].trim().to_string();
        }
    }
    trimmed.to_string()
}

/// Split a comma-separated recipient field into individual addresses.
/// Whitespace is trimmed, empties dropped. Used for both prefill and
/// the POST parse so the user's input always round-trips identically.
pub fn split_recipient_field(field: &str) -> Vec<String> {
    field
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Truncate `value` to `MAX_PREFILL_HEADER_LEN` chars so a hostile or
/// runaway echo doesn't make the re-render OOM. Returns the original
/// unchanged when under the cap.
fn cap_header_for_echo(value: String) -> String {
    if value.len() <= MAX_PREFILL_HEADER_LEN {
        value
    } else {
        value.chars().take(MAX_PREFILL_HEADER_LEN).collect()
    }
}

/// GET /classic/compose — render the compose form, optionally prefilled
/// from a source message (reply / reply-all / forward).
///
/// A prefill IMAP fetch failure does NOT abort the request — we render the
/// blank form with a soft error banner so the user can still compose a
/// message. The alternative ("can't compose because Reply prefill 500'd")
/// is a strictly worse UX.
pub async fn get_compose(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Extension(session): Extension<crate::models::classic_session::ClassicSession>,
    // Added (TMAIL-368): per-request CSP nonce from `security_headers_middleware`.
    Extension(csp_nonce): Extension<CspNonce>,
    Query(query): Query<ComposeQuery>,
) -> Result<Response, AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID in classic claims")))?;

    let folder = query.folder_or_default();
    let mut template =
        ComposeTemplate::fresh(&folder, &session.csrf_token, csp_nonce.as_str());

    if let Some((uid, mode)) = query.prefill_target() {
        match fetch_prefill(&state, mailbox_id, &folder, uid, mode).await {
            Ok(prefill) => {
                template.to = prefill.to.join(", ");
                template.cc = prefill.cc.join(", ");
                template.subject = prefill.subject;
                template.body = prefill.body;
                template.in_reply_to = prefill.in_reply_to;
                template.references = prefill.references;
            }
            Err(e) => {
                tracing::warn!(
                    error = ?e,
                    uid = %uid,
                    folder = %folder,
                    mode = ?mode,
                    "classic compose prefill failed — rendering blank form with notice"
                );
                template.error = Some(
                    "Couldn't load the original message for prefill. The compose form is blank — \
                     fill it in and send normally."
                        .to_string(),
                );
            }
        }
    }

    let html = template
        .render()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("classic compose template render: {e}")))?;
    Ok((
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/html; charset=utf-8",
        )],
        html,
    )
        .into_response())
}

/// Intermediate shape for the prefill fetch result. Kept private to this
/// module — the public surface is the populated `ComposeTemplate`.
struct Prefill {
    to: Vec<String>,
    cc: Vec<String>,
    subject: String,
    body: String,
    in_reply_to: String,
    references: String,
}

/// Fetch the original message and build the prefill shape for a reply /
/// reply-all / forward. Splits the network call from the rendering so
/// the GET handler can swallow fetch failures into a soft banner.
async fn fetch_prefill(
    state: &AppState,
    mailbox_id: Uuid,
    folder: &str,
    uid: u32,
    mode: PrefillMode,
) -> Result<Prefill, AppError> {
    let imap = ImapService::for_user(state, mailbox_id).await?;
    let (username, password) = imap
        .user_creds()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("BYOK creds missing on ImapService")))?;
    let username = username.to_string();
    let password = password.to_string();
    let original = imap.get_message(&username, &password, folder, uid).await?;

    // We need the user's own primary email to filter themselves out of a
    // reply-all. The IMAP SMTP login username IS that email in every BYOK
    // path we ship (SMTP from_address falls back to username — see
    // handlers::messages::send_message), so it's the right thing to use.
    let self_email = username.clone();

    let subject = build_reply_subject(
        original.subject.as_deref().unwrap_or(""),
        mode,
    );

    let (to, cc) = match mode {
        PrefillMode::Reply => {
            let to = original
                .from
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .map(|f| vec![f.to_string()])
                .unwrap_or_default();
            (to, vec![])
        }
        PrefillMode::ReplyAll => build_reply_all_recipients(
            original.from.as_deref(),
            &original.to,
            &original.cc,
            &self_email,
        ),
        PrefillMode::Forward => (vec![], vec![]),
    };

    let body = build_quoted_body(
        original.from.as_deref(),
        original.date.as_deref(),
        original.text_body.as_deref(),
        mode,
    );

    // Threading headers — only on Reply / Reply-All. Forward starts a new
    // thread (matches Gmail / Thunderbird semantics).
    let (in_reply_to, references) = match mode {
        PrefillMode::Reply | PrefillMode::ReplyAll => (
            original.message_id.clone().unwrap_or_default(),
            build_references_chain(
                original.message_id.as_deref(),
                &original.references,
                original.in_reply_to.as_deref(),
            ),
        ),
        PrefillMode::Forward => (String::new(), String::new()),
    };

    Ok(Prefill {
        to,
        cc,
        subject,
        body,
        in_reply_to,
        references,
    })
}

/// Parsed multipart form fields from POST /classic/compose. Built by
/// `parse_compose_multipart` so the actual handler stays focused on
/// validation + send rather than wire-format parsing.
struct ComposeSubmission {
    to_raw: String,
    cc_raw: String,
    bcc_raw: String,
    subject: String,
    body: String,
    in_reply_to: String,
    references: String,
    source_folder: String,
    attachments: Vec<OutgoingAttachment>,
    /// Total byte count of `body` + each attachment's data — used by the
    /// 25 MB cap.
    total_bytes: usize,
}

/// Walk a `Multipart` body and pull out every known compose field. Unknown
/// fields are ignored (forward compatible — if a future revision adds an
/// `urgency` flag, an older client posting without it shouldn't 400).
async fn parse_compose_multipart(
    mut multipart: Multipart,
) -> Result<ComposeSubmission, AppError> {
    let mut submission = ComposeSubmission {
        to_raw: String::new(),
        cc_raw: String::new(),
        bcc_raw: String::new(),
        subject: String::new(),
        body: String::new(),
        in_reply_to: String::new(),
        references: String::new(),
        source_folder: String::new(),
        attachments: Vec::new(),
        total_bytes: 0,
    };

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid multipart data: {e}")))?
    {
        let field_name = field.name().unwrap_or("").to_string();
        // Files surface with `field.file_name() = Some(...)` regardless of
        // the `name=` attribute; the spec's form field is `attachments[]`
        // but we accept any field that arrives with a filename so a future
        // template change (e.g. `attachment` singular) doesn't need a
        // matching handler edit.
        let file_name_attr = field.file_name().map(String::from);
        let content_type_attr = field
            .content_type()
            .map(String::from)
            .unwrap_or_else(|| "application/octet-stream".to_string());

        if let Some(fname) = file_name_attr {
            // File part. Empty filename means the user didn't actually
            // attach anything in that slot — skip rather than recording a
            // zero-byte attachment.
            let bytes = field
                .bytes()
                .await
                .map_err(|e| AppError::BadRequest(format!("Failed to read attachment: {e}")))?;
            if bytes.is_empty() {
                continue;
            }
            let filename = if fname.trim().is_empty() {
                DEFAULT_FILENAME.to_string()
            } else {
                fname
            };
            submission.total_bytes = submission.total_bytes.saturating_add(bytes.len());
            // Early-exit on size — keeps us from buffering more than the
            // user is allowed to send. The handler also re-checks AFTER
            // the loop in case the cap is hit by `body` text.
            if submission.total_bytes > MAX_TOTAL_BYTES {
                return Err(AppError::BadRequest(format!(
                    "Total upload exceeds the {} MB limit.",
                    MAX_TOTAL_BYTES / (1024 * 1024)
                )));
            }
            submission.attachments.push(OutgoingAttachment {
                filename,
                content_type: content_type_attr,
                data: bytes.to_vec(),
            });
            continue;
        }

        // Text fields — read as UTF-8. Reject non-UTF-8 (browsers always
        // post forms as UTF-8 with the page's charset; binary on a text
        // field is malformed).
        let text = field
            .text()
            .await
            .map_err(|e| AppError::BadRequest(format!("Form field '{field_name}' is malformed: {e}")))?;

        match field_name.as_str() {
            "_csrf" => { /* validated by middleware upstream */ }
            "to" => submission.to_raw = text,
            "cc" => submission.cc_raw = text,
            "bcc" => submission.bcc_raw = text,
            "subject" => submission.subject = text,
            "body" => {
                submission.total_bytes = submission.total_bytes.saturating_add(text.len());
                submission.body = text;
                if submission.total_bytes > MAX_TOTAL_BYTES {
                    return Err(AppError::BadRequest(format!(
                        "Total upload exceeds the {} MB limit.",
                        MAX_TOTAL_BYTES / (1024 * 1024)
                    )));
                }
            }
            "in_reply_to" => submission.in_reply_to = text,
            "references" => submission.references = text,
            "folder" => submission.source_folder = text,
            _ => {
                // Unknown text field — ignore for forward compatibility.
            }
        }
    }

    if submission.source_folder.trim().is_empty() {
        submission.source_folder = "INBOX".to_string();
    }

    Ok(submission)
}

/// Convert a `ComposeSubmission` + the BYOK SMTP `from` address into a
/// validated `SendRequest`. Pulled out so the unit tests can exercise the
/// happy + error paths without standing up IMAP/SMTP.
pub fn build_send_request(
    submission_to: &str,
    submission_cc: &str,
    submission_bcc: &str,
    subject: &str,
    body: &str,
    in_reply_to: &str,
    references: &str,
    attachments: Vec<OutgoingAttachment>,
) -> Result<SendRequest, AppError> {
    validation::validate_subject(subject)?;
    if body.len() > MAX_MESSAGE_BODY_LEN {
        return Err(AppError::BadRequest(format!(
            "Message body exceeds the {} MB limit.",
            MAX_MESSAGE_BODY_LEN / (1024 * 1024)
        )));
    }
    let to_list = split_recipient_field(submission_to);
    if to_list.is_empty() {
        return Err(AppError::BadRequest(
            "At least one To recipient is required.".to_string(),
        ));
    }
    validation::validate_recipient_list("To", &to_list)?;

    let cc_list = split_recipient_field(submission_cc);
    let bcc_list = split_recipient_field(submission_bcc);
    if !cc_list.is_empty() {
        validation::validate_recipient_list("Cc", &cc_list)?;
    }
    if !bcc_list.is_empty() {
        validation::validate_recipient_list("Bcc", &bcc_list)?;
    }

    let in_reply_to_opt = {
        let t = in_reply_to.trim();
        if t.is_empty() { None } else { Some(t.to_string()) }
    };
    let references_opt = {
        let t = references.trim();
        if t.is_empty() {
            None
        } else {
            Some(
                t.split_whitespace()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>(),
            )
        }
    };

    Ok(SendRequest {
        to: to_list,
        cc: if cc_list.is_empty() { None } else { Some(cc_list) },
        bcc: if bcc_list.is_empty() { None } else { Some(bcc_list) },
        subject: subject.to_string(),
        text_body: Some(body.to_string()),
        html_body: None,
        in_reply_to: in_reply_to_opt,
        references: references_opt,
        attachments,
    })
}

/// POST /classic/compose — accept multipart, send via BYOK SMTP, redirect.
///
/// Failure path re-renders the compose form with the validation error AND
/// the user's prior input preserved (file bytes excepted — see the
/// module-level comment).
pub async fn post_compose(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Extension(session): Extension<crate::models::classic_session::ClassicSession>,
    // Added (TMAIL-368): per-request CSP nonce for the re-render-on-failure
    // path. The success path is a 303 redirect with no body so it carries
    // no inline `<style>` — only the failure branch needs the nonce.
    Extension(csp_nonce): Extension<CspNonce>,
    multipart: Multipart,
) -> Result<Response, AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID in classic claims")))?;

    // Parse multipart. A parse failure short-circuits to AppError → 400.
    let submission = match parse_compose_multipart(multipart).await {
        Ok(s) => s,
        Err(e) => {
            // Render the compose form with the error so the user can
            // retry without losing context. The submission body is gone
            // (parse failed before we could record it), so the form
            // comes back blank — that's the price of a malformed POST.
            return render_error_form(
                &session.csrf_token,
                &error_message(&e),
                ComposeSubmission {
                    to_raw: String::new(),
                    cc_raw: String::new(),
                    bcc_raw: String::new(),
                    subject: String::new(),
                    body: String::new(),
                    in_reply_to: String::new(),
                    references: String::new(),
                    source_folder: "INBOX".to_string(),
                    attachments: vec![],
                    total_bytes: 0,
                },
                vec![],
                csp_nonce.as_str(),
            );
        }
    };

    // Extract attachment filenames BEFORE the send-request build moves the
    // attachments — we need them for the failure re-render.
    let prior_filenames: Vec<String> =
        submission.attachments.iter().map(|a| a.filename.clone()).collect();
    let prior_total_bytes = submission.total_bytes;
    // Clone the text fields too — they're needed by the failure-render
    // path AND by the send request which moves them.
    let to_raw = submission.to_raw.clone();
    let cc_raw = submission.cc_raw.clone();
    let bcc_raw = submission.bcc_raw.clone();
    let subject = submission.subject.clone();
    let body = submission.body.clone();
    let in_reply_to = submission.in_reply_to.clone();
    let references = submission.references.clone();
    let source_folder = submission.source_folder.clone();

    let send_req = match build_send_request(
        &to_raw,
        &cc_raw,
        &bcc_raw,
        &subject,
        &body,
        &in_reply_to,
        &references,
        submission.attachments,
    ) {
        Ok(req) => req,
        Err(e) => {
            return render_error_form(
                &session.csrf_token,
                &error_message(&e),
                ComposeSubmission {
                    to_raw,
                    cc_raw,
                    bcc_raw,
                    subject,
                    body,
                    in_reply_to,
                    references,
                    source_folder,
                    attachments: vec![],
                    total_bytes: prior_total_bytes,
                },
                prior_filenames,
                csp_nonce.as_str(),
            );
        }
    };

    // Load the user's BYOK SMTP config and send. Mirrors the BYOK send
    // path from handlers::messages::send_message — DB row → decrypt
    // password → SmtpService::new → send. Cache lookup keeps the hot
    // path fast.
    let cache_key = mailbox_id.to_string();
    let smtp_cfg: crate::models::smtp_config::SmtpConfiguration = match state
        .cache
        .get_user_smtp_config::<crate::models::smtp_config::SmtpConfiguration>(&cache_key)
        .await
    {
        Some(hit) => hit,
        None => {
            let row = crate::models::smtp_config::SmtpConfiguration::find_default(
                &state.db,
                mailbox_id,
            )
            .await?
            .ok_or_else(|| {
                AppError::ServiceUnavailable(
                    "No SMTP server configured. Complete the onboarding wizard at /onboarding."
                        .into(),
                )
            })?;
            let _ = state.cache.set_user_smtp_config(&cache_key, &row).await;
            row
        }
    };
    let enc_key = crate::models::ai_config::derive_encryption_key(&state.config.jwt.secret);
    let smtp_password = smtp_cfg.decrypted_password(&enc_key).map_err(|e| {
        AppError::Internal(anyhow::anyhow!("Failed to decrypt SMTP password: {}", e))
    })?;
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

    match smtp_service.send(&smtp_from, &smtp_password, &send_req).await {
        Ok(()) => {
            let folder_href = urlencoding::encode(&source_folder).into_owned();
            let target = format!("/classic/folders/{folder_href}?sent=1");
            // 303 See Other — POST-Redirect-Get so a browser reload of
            // the landing page doesn't re-submit the form.
            Ok((
                StatusCode::SEE_OTHER,
                [(axum::http::header::LOCATION, target)],
            )
                .into_response())
        }
        Err(e) => render_error_form(
            &session.csrf_token,
            &error_message(&e),
            ComposeSubmission {
                to_raw,
                cc_raw,
                bcc_raw,
                subject,
                body,
                in_reply_to,
                references,
                source_folder,
                attachments: vec![],
                total_bytes: prior_total_bytes,
            },
            prior_filenames,
            csp_nonce.as_str(),
        ),
    }
}

/// Build a friendly error message from an `AppError`. Hides the inner
/// stack from the user — the original is already in the tracing log via
/// the error layer downstream — and surfaces just the `BadRequest` /
/// `ServiceUnavailable` text so validation errors land verbatim.
fn error_message(err: &AppError) -> String {
    match err {
        AppError::BadRequest(msg) | AppError::ServiceUnavailable(msg) => msg.clone(),
        AppError::NotFound(msg) => msg.clone(),
        AppError::Smtp(msg) => format!("Couldn't send the message: {msg}"),
        AppError::Imap(msg) => format!("Mail server error: {msg}"),
        _ => "We couldn't send the message. Please try again.".to_string(),
    }
}

/// Render the compose form for the failure path: 400 + the user's prior
/// input echoed back AND a visible error banner. The browser doesn't
/// round-trip file bytes on no-JS forms, so the attachment list is the
/// filenames only — the user has to re-attach if they resubmit.
///
/// Pulls the source folder out of `submission.source_folder` (defaulting
/// to INBOX when blank) so callers don't have to pass it twice.
fn render_error_form(
    csrf_token: &str,
    error: &str,
    submission: ComposeSubmission,
    prior_attachment_names: Vec<String>,
    // Added (TMAIL-368): explicit CSP nonce — must be the per-request value
    // the security_headers middleware already baked into the response CSP
    // header. Threaded from `post_compose`.
    csp_nonce: &str,
) -> Result<Response, AppError> {
    let folder = if submission.source_folder.trim().is_empty() {
        "INBOX".to_string()
    } else {
        submission.source_folder.clone()
    };
    let folder_href = urlencoding::encode(&folder).into_owned();

    let template = ComposeTemplate {
        to: cap_header_for_echo(submission.to_raw),
        cc: cap_header_for_echo(submission.cc_raw),
        bcc: cap_header_for_echo(submission.bcc_raw),
        subject: cap_header_for_echo(submission.subject),
        body: submission.body,
        in_reply_to: cap_header_for_echo(submission.in_reply_to),
        references: cap_header_for_echo(submission.references),
        source_folder: folder,
        source_folder_href: folder_href,
        prior_attachment_names,
        error: Some(error.to_string()),
        csrf_token: csrf_token.to_string(),
        csp_nonce: csp_nonce.to_string(),
    };
    let html = template
        .render()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("classic compose template render: {e}")))?;
    Ok((
        // Use 400 on validation errors so a caller (curl, monitoring,
        // log filters) can distinguish a re-render from a fresh GET.
        StatusCode::BAD_REQUEST,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/html; charset=utf-8",
        )],
        html,
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lt(s: &str) -> String {
        // Helper: collapse repeated "<" / ">" escape variants for assertion
        // readability. Askama escapes them to `&#60;` / `&lt;` depending on
        // version — tests should accept either.
        s.replace("&lt;", "<").replace("&gt;", ">")
            .replace("&#60;", "<").replace("&#62;", ">")
    }

    fn fresh_template() -> ComposeTemplate {
        // Changed (TMAIL-368): `fresh` now takes the CSP nonce as a third
        // argument so the template-vs-CSP-header binding is explicit. Tests
        // can pass any fixed value; production code threads the per-request
        // value from request extensions.
        ComposeTemplate::fresh("INBOX", "test-csrf-token", "test-nonce-fixed-for-tests")
    }

    // ----- ComposeQuery -----

    #[test]
    fn compose_query_defaults_to_inbox_when_folder_missing() {
        let q = ComposeQuery::default();
        assert_eq!(q.folder_or_default(), "INBOX");
    }

    #[test]
    fn compose_query_defaults_to_inbox_when_folder_blank() {
        let q = ComposeQuery {
            folder: Some("   ".to_string()),
            ..Default::default()
        };
        assert_eq!(q.folder_or_default(), "INBOX");
    }

    #[test]
    fn compose_query_passes_through_explicit_folder() {
        let q = ComposeQuery {
            folder: Some("Sent".to_string()),
            ..Default::default()
        };
        assert_eq!(q.folder_or_default(), "Sent");
    }

    #[test]
    fn compose_query_prefill_precedence_reply_to_wins() {
        let q = ComposeQuery {
            reply_to: Some(1),
            reply_all: Some(2),
            forward: Some(3),
            folder: None,
        };
        assert_eq!(q.prefill_target(), Some((1, PrefillMode::Reply)));
    }

    #[test]
    fn compose_query_prefill_precedence_reply_all_over_forward() {
        let q = ComposeQuery {
            reply_to: None,
            reply_all: Some(2),
            forward: Some(3),
            folder: None,
        };
        assert_eq!(q.prefill_target(), Some((2, PrefillMode::ReplyAll)));
    }

    #[test]
    fn compose_query_prefill_forward_alone() {
        let q = ComposeQuery {
            forward: Some(7),
            ..Default::default()
        };
        assert_eq!(q.prefill_target(), Some((7, PrefillMode::Forward)));
    }

    #[test]
    fn compose_query_prefill_none_on_blank() {
        let q = ComposeQuery::default();
        assert_eq!(q.prefill_target(), None);
    }

    // ----- build_reply_subject -----

    #[test]
    fn reply_subject_adds_re_prefix() {
        assert_eq!(
            build_reply_subject("Hello there", PrefillMode::Reply),
            "Re: Hello there"
        );
        assert_eq!(
            build_reply_subject("Hello there", PrefillMode::ReplyAll),
            "Re: Hello there"
        );
    }

    #[test]
    fn reply_subject_idempotent_on_existing_re() {
        assert_eq!(
            build_reply_subject("Re: thread", PrefillMode::Reply),
            "Re: thread"
        );
        assert_eq!(
            build_reply_subject("RE: shouting", PrefillMode::Reply),
            "RE: shouting"
        );
        assert_eq!(
            build_reply_subject("re: lowercase", PrefillMode::Reply),
            "re: lowercase"
        );
    }

    #[test]
    fn forward_subject_adds_fwd_prefix() {
        assert_eq!(
            build_reply_subject("doc.pdf", PrefillMode::Forward),
            "Fwd: doc.pdf"
        );
    }

    #[test]
    fn forward_subject_idempotent_on_fwd_and_fw() {
        assert_eq!(
            build_reply_subject("Fwd: thread", PrefillMode::Forward),
            "Fwd: thread"
        );
        assert_eq!(
            build_reply_subject("Fw: short form", PrefillMode::Forward),
            "Fw: short form"
        );
        assert_eq!(
            build_reply_subject("FWD: SHOUTING", PrefillMode::Forward),
            "FWD: SHOUTING"
        );
    }

    #[test]
    fn reply_subject_handles_blank_source() {
        // Read-view substitutes `(no subject)` for an empty subject; the
        // compose prefix preserves that so the reply still has context.
        assert_eq!(
            build_reply_subject("", PrefillMode::Reply),
            "Re: (no subject)"
        );
        assert_eq!(
            build_reply_subject("   ", PrefillMode::Forward),
            "Fwd: (no subject)"
        );
    }

    // ----- build_quoted_body -----

    #[test]
    fn quoted_body_prefixes_reply_lines_with_gt() {
        let q = build_quoted_body(
            Some("Alice <a@x.com>"),
            Some("Mon, 1 Jan 2026 09:00:00 +0000"),
            Some("Line one\nLine two"),
            PrefillMode::Reply,
        );
        assert!(
            q.contains("On Mon, 1 Jan 2026 09:00:00 +0000, Alice <a@x.com> wrote:"),
            "missing attribution: {q}"
        );
        assert!(q.contains("> Line one"));
        assert!(q.contains("> Line two"));
        // Leading blank lines so the cursor lands above the quoted block.
        assert!(q.starts_with("\n\n"), "expected leading blank lines: {q:?}");
    }

    #[test]
    fn quoted_body_falls_back_when_from_missing() {
        let q = build_quoted_body(
            None,
            Some("Mon, 1 Jan 2026 09:00:00 +0000"),
            Some("Hi"),
            PrefillMode::Reply,
        );
        assert!(q.contains("On Mon, 1 Jan 2026 09:00:00 +0000 someone wrote:"));
    }

    #[test]
    fn quoted_body_skips_attribution_when_both_missing() {
        let q = build_quoted_body(None, None, Some("Hi"), PrefillMode::Reply);
        assert!(!q.contains("wrote:"));
        assert!(q.contains("> Hi"));
    }

    #[test]
    fn quoted_body_returns_empty_when_no_body() {
        assert_eq!(build_quoted_body(Some("x"), Some("y"), None, PrefillMode::Reply), "");
        assert_eq!(build_quoted_body(Some("x"), Some("y"), Some(""), PrefillMode::Forward), "");
        assert_eq!(build_quoted_body(Some("x"), Some("y"), Some("\n\n"), PrefillMode::Forward), "");
    }

    #[test]
    fn forward_body_uses_banner_not_gt_prefix() {
        let q = build_quoted_body(
            Some("Alice <a@x.com>"),
            Some("Mon, 1 Jan 2026"),
            Some("Original body"),
            PrefillMode::Forward,
        );
        assert!(q.contains("---------- Forwarded message ----------"));
        assert!(q.contains("From: Alice <a@x.com>"));
        assert!(q.contains("Date: Mon, 1 Jan 2026"));
        assert!(q.contains("Original body"));
        // Forward should NOT use the `> ` quote prefix — the banner is enough.
        assert!(!q.contains("> Original body"));
    }

    // ----- build_references_chain -----

    #[test]
    fn references_chain_appends_message_id_to_existing_chain() {
        let chain = build_references_chain(
            Some("<msg-3@x>"),
            &["<msg-1@x>".to_string(), "<msg-2@x>".to_string()],
            None,
        );
        assert_eq!(chain, "<msg-1@x> <msg-2@x> <msg-3@x>");
    }

    #[test]
    fn references_chain_falls_back_to_in_reply_to_when_no_references() {
        let chain = build_references_chain(Some("<msg-2@x>"), &[], Some("<msg-1@x>"));
        assert_eq!(chain, "<msg-1@x> <msg-2@x>");
    }

    #[test]
    fn references_chain_returns_just_message_id_when_no_chain_exists() {
        let chain = build_references_chain(Some("<msg-1@x>"), &[], None);
        assert_eq!(chain, "<msg-1@x>");
    }

    #[test]
    fn references_chain_returns_empty_when_nothing_to_say() {
        let chain = build_references_chain(None, &[], None);
        assert!(chain.is_empty());
    }

    #[test]
    fn references_chain_dedupes_when_message_id_already_in_chain() {
        let chain = build_references_chain(
            Some("<msg-2@x>"),
            &["<msg-1@x>".to_string(), "<msg-2@x>".to_string()],
            None,
        );
        assert_eq!(chain, "<msg-1@x> <msg-2@x>");
    }

    // ----- build_reply_all_recipients -----

    #[test]
    fn reply_all_puts_from_in_to_and_others_in_cc() {
        let (to, cc) = build_reply_all_recipients(
            Some("Alice <a@x.com>"),
            &["Bob <b@x.com>".to_string()],
            &["Carol <c@x.com>".to_string()],
            "self@x.com",
        );
        assert_eq!(to, vec!["Alice <a@x.com>"]);
        assert_eq!(cc, vec!["Bob <b@x.com>", "Carol <c@x.com>"]);
    }

    #[test]
    fn reply_all_filters_self_out_of_to_and_cc() {
        // Self in From → reply-all sends to nobody in To. Self in To/Cc
        // → drops from Cc.
        let (to, cc) = build_reply_all_recipients(
            Some("self@x.com"),
            &["self@x.com".to_string(), "Bob <b@x.com>".to_string()],
            &["Self Display <self@x.com>".to_string()],
            "self@x.com",
        );
        assert!(to.is_empty(), "From=self should drop from To");
        assert_eq!(cc, vec!["Bob <b@x.com>"]);
    }

    #[test]
    fn reply_all_self_filter_handles_display_name_form() {
        // The `From` header is `"Self Name" <self@x.com>` — the filter
        // must compare the email portion, not the whole display string.
        let (to, _cc) = build_reply_all_recipients(
            Some("Self Name <self@x.com>"),
            &[],
            &[],
            "self@x.com",
        );
        assert!(to.is_empty());
    }

    // ----- split_recipient_field -----

    #[test]
    fn split_recipient_field_splits_on_commas_and_trims() {
        let parts = split_recipient_field("a@x.com, b@x.com ,c@x.com");
        assert_eq!(parts, vec!["a@x.com", "b@x.com", "c@x.com"]);
    }

    #[test]
    fn split_recipient_field_drops_empties() {
        let parts = split_recipient_field(",,a@x.com,,b@x.com,");
        assert_eq!(parts, vec!["a@x.com", "b@x.com"]);
    }

    #[test]
    fn split_recipient_field_empty_input_yields_empty_vec() {
        assert!(split_recipient_field("").is_empty());
        assert!(split_recipient_field("   ").is_empty());
        assert!(split_recipient_field(",,,").is_empty());
    }

    #[test]
    fn split_recipient_field_preserves_display_names() {
        let parts = split_recipient_field("Alice <a@x.com>, Bob <b@x.com>");
        assert_eq!(parts, vec!["Alice <a@x.com>", "Bob <b@x.com>"]);
    }

    // ----- address_email -----

    #[test]
    fn address_email_extracts_email_from_display_form() {
        assert_eq!(address_email("Alice <a@x.com>"), "a@x.com");
        assert_eq!(address_email("\"Doe, Jane\" <jd@x.com>"), "jd@x.com");
    }

    #[test]
    fn address_email_returns_input_when_no_brackets() {
        assert_eq!(address_email("plain@x.com"), "plain@x.com");
        assert_eq!(address_email("  plain@x.com  "), "plain@x.com");
    }

    // ----- build_send_request -----

    #[test]
    fn send_request_built_from_simple_compose() {
        let req = build_send_request(
            "to@x.com",
            "",
            "",
            "Hello",
            "World",
            "",
            "",
            vec![],
        )
        .expect("valid send request");
        assert_eq!(req.to, vec!["to@x.com"]);
        assert!(req.cc.is_none());
        assert!(req.bcc.is_none());
        assert_eq!(req.subject, "Hello");
        assert_eq!(req.text_body.as_deref(), Some("World"));
        assert!(req.html_body.is_none());
        assert!(req.in_reply_to.is_none());
        assert!(req.references.is_none());
        assert!(req.attachments.is_empty());
    }

    #[test]
    fn send_request_rejects_empty_to() {
        let err = build_send_request(
            "",
            "",
            "",
            "Hello",
            "World",
            "",
            "",
            vec![],
        )
        .expect_err("empty to should reject");
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn send_request_splits_comma_separated_to() {
        let req = build_send_request(
            "a@x.com, b@x.com, c@x.com",
            "",
            "",
            "Hello",
            "World",
            "",
            "",
            vec![],
        )
        .expect("valid send request");
        assert_eq!(req.to, vec!["a@x.com", "b@x.com", "c@x.com"]);
    }

    #[test]
    fn send_request_carries_cc_and_bcc() {
        let req = build_send_request(
            "a@x.com",
            "b@x.com, c@x.com",
            "d@x.com",
            "Hello",
            "World",
            "",
            "",
            vec![],
        )
        .expect("valid send request");
        assert_eq!(req.cc.as_deref(), Some(&["b@x.com".to_string(), "c@x.com".to_string()][..]));
        assert_eq!(req.bcc.as_deref(), Some(&["d@x.com".to_string()][..]));
    }

    #[test]
    fn send_request_carries_threading_headers() {
        let req = build_send_request(
            "to@x.com",
            "",
            "",
            "Re: Hi",
            "Body",
            "<msg-1@x>",
            "<msg-1@x> <msg-2@x>",
            vec![],
        )
        .expect("valid send request");
        assert_eq!(req.in_reply_to.as_deref(), Some("<msg-1@x>"));
        assert_eq!(
            req.references.as_deref(),
            Some(&["<msg-1@x>".to_string(), "<msg-2@x>".to_string()][..])
        );
    }

    #[test]
    fn send_request_drops_empty_threading_headers() {
        let req = build_send_request(
            "to@x.com", "", "", "Hi", "Body", "   ", "   ", vec![],
        )
        .expect("valid send request");
        assert!(req.in_reply_to.is_none());
        assert!(req.references.is_none());
    }

    #[test]
    fn send_request_attaches_files() {
        let att = OutgoingAttachment {
            filename: "doc.pdf".to_string(),
            content_type: "application/pdf".to_string(),
            data: vec![1, 2, 3, 4],
        };
        let req = build_send_request(
            "to@x.com", "", "", "Hi", "Body", "", "", vec![att],
        )
        .expect("valid send request");
        assert_eq!(req.attachments.len(), 1);
        assert_eq!(req.attachments[0].filename, "doc.pdf");
        assert_eq!(req.attachments[0].data, vec![1, 2, 3, 4]);
    }

    #[test]
    fn send_request_rejects_malformed_recipient() {
        // The validator rejects addresses without an `@`. Pre-flight should
        // surface this as a BadRequest, not let it bubble to lettre.
        let err = build_send_request(
            "not-an-email", "", "", "Hi", "Body", "", "", vec![],
        )
        .expect_err("malformed recipient should reject");
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn send_request_rejects_subject_with_crlf_injection() {
        // CRLF in subject is a classic header-injection vector. The
        // validator must catch it before lettre sees it.
        let err = build_send_request(
            "to@x.com", "", "", "Subject\r\nBcc: evil@x.com", "Body", "", "", vec![],
        )
        .expect_err("CRLF in subject should reject");
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    // ----- cap_header_for_echo -----

    #[test]
    fn cap_header_passes_through_short_values() {
        let s = "short string".to_string();
        assert_eq!(cap_header_for_echo(s.clone()), s);
    }

    #[test]
    fn cap_header_truncates_oversized_values() {
        let s = "x".repeat(MAX_PREFILL_HEADER_LEN + 100);
        let capped = cap_header_for_echo(s);
        assert_eq!(capped.len(), MAX_PREFILL_HEADER_LEN);
    }

    // ----- Template rendering -----

    #[test]
    fn compose_template_extends_base_layout() {
        let body = fresh_template().render().expect("template renders");
        assert!(body.starts_with("<!DOCTYPE html>"));
        assert!(body.contains("class=\"skip-link\""));
        assert!(body.contains("<main id=\"main\""));
        assert!(body.contains("<style nonce=\"test-nonce-fixed-for-assertions\">") || body.contains("<style nonce=\""));
    }

    #[test]
    fn compose_template_has_zero_script_tags() {
        let body = fresh_template().render().expect("template renders");
        assert!(
            !body.contains("<script"),
            "compose template must contain ZERO <script> tags: {body}"
        );
    }

    #[test]
    fn compose_template_renders_logout_form_with_csrf_token() {
        let body = fresh_template().render().expect("template renders");
        assert!(body.contains("action=\"/classic/logout\""));
        assert!(body.contains("value=\"test-csrf-token\""));
    }

    #[test]
    fn compose_template_form_posts_multipart_to_compose() {
        let body = fresh_template().render().expect("template renders");
        assert!(
            body.contains("action=\"/classic/compose\""),
            "form action missing: {body}"
        );
        assert!(
            body.contains("method=\"post\""),
            "form method missing: {body}"
        );
        assert!(
            body.contains("enctype=\"multipart/form-data\""),
            "multipart enctype missing — attachments wouldn't upload: {body}"
        );
    }

    #[test]
    fn compose_template_renders_csrf_input_in_compose_form() {
        let body = fresh_template().render().expect("template renders");
        // The CSRF input appears twice — once in the logout form, once in
        // the compose form itself. Match by surrounding attributes.
        let compose_csrf_count = body.matches("name=\"_csrf\" value=\"test-csrf-token\"").count();
        assert!(
            compose_csrf_count >= 2,
            "expected CSRF input in BOTH compose and logout forms: {body}"
        );
    }

    #[test]
    fn compose_template_renders_required_to_subject_body_inputs() {
        let body = fresh_template().render().expect("template renders");
        assert!(body.contains("name=\"to\""));
        assert!(body.contains("name=\"cc\""));
        assert!(body.contains("name=\"bcc\""));
        assert!(body.contains("name=\"subject\""));
        assert!(body.contains("name=\"body\""));
        // Attachments file input — multiple selection so the user can attach
        // several files in one go.
        assert!(body.contains("name=\"attachments\""));
        assert!(body.contains("type=\"file\""));
        assert!(body.contains("multiple"));
    }

    #[test]
    fn compose_template_renders_hidden_threading_and_folder_inputs() {
        let mut t = fresh_template();
        t.in_reply_to = "<msg-1@x>".to_string();
        t.references = "<msg-1@x> <msg-2@x>".to_string();
        let body = t.render().expect("template renders");
        assert!(body.contains("name=\"in_reply_to\""));
        assert!(body.contains("name=\"references\""));
        assert!(body.contains("name=\"folder\""));
        // Hidden input echo values land verbatim.
        assert!(lt(&body).contains(r#"value="<msg-1@x>""#));
    }

    #[test]
    fn compose_template_echoes_prefilled_to_subject_body() {
        let mut t = fresh_template();
        t.to = "Bob <bob@x.com>".to_string();
        t.subject = "Re: hi".to_string();
        t.body = "Quoted body\n\n> original".to_string();
        let body = t.render().expect("template renders");
        let decoded = lt(&body);
        // To input value echoes the prefill.
        assert!(decoded.contains(r#"value="Bob <bob@x.com>""#) || decoded.contains("Bob &lt;bob"));
        // Subject value echoes.
        assert!(decoded.contains(r#"value="Re: hi""#));
        // Body textarea echoes (between <textarea>...</textarea>).
        assert!(decoded.contains("Quoted body"));
        assert!(decoded.contains("> original"));
    }

    #[test]
    fn compose_template_renders_error_banner_when_error_present() {
        let mut t = fresh_template();
        t.error = Some("Invalid email address.".to_string());
        let body = t.render().expect("template renders");
        assert!(
            body.contains("alert-error") || body.contains("role=\"alert\""),
            "error banner missing: {body}"
        );
        assert!(body.contains("Invalid email address."));
    }

    #[test]
    fn compose_template_omits_error_banner_when_no_error() {
        // The base.html stylesheet defines `.alert-error` CSS rules so the
        // string appears in the rendered CSS even when no error block is
        // emitted. Assert against the actual rendered element instead.
        let body = fresh_template().render().expect("template renders");
        assert!(
            !body.contains("class=\"alert alert-error\""),
            "no error block should render when error is None: {body}"
        );
        assert!(
            !body.contains("role=\"alert\""),
            "no role=\"alert\" element should render when error is None: {body}"
        );
    }

    #[test]
    fn compose_template_echoes_prior_attachment_filenames_on_failure() {
        let mut t = fresh_template();
        t.prior_attachment_names = vec!["doc.pdf".to_string(), "image.png".to_string()];
        let body = t.render().expect("template renders");
        // The list renders so the user knows what they had attached.
        assert!(body.contains("doc.pdf"));
        assert!(body.contains("image.png"));
        // The text should make it clear they have to re-attach.
        assert!(
            body.contains("re-attach") || body.contains("attach again"),
            "missing re-attach notice on failure echo: {body}"
        );
    }

    #[test]
    fn compose_template_renders_cancel_link_back_to_source_folder() {
        let t = ComposeTemplate::fresh("Sent", "test-csrf-token", "test-nonce-fixed-for-tests");
        let body = t.render().expect("template renders");
        assert!(
            body.contains("href=\"/classic/folders/Sent\""),
            "cancel link to source folder missing: {body}"
        );
    }

    #[test]
    fn compose_template_html_escapes_hostile_values() {
        let mut t = fresh_template();
        t.subject = "<script>alert(1)</script>".to_string();
        t.to = "<script>".to_string();
        t.error = Some("<img src=x onerror=evil>".to_string());
        let body = t.render().expect("template renders");
        assert!(!body.contains("<script>alert(1)</script>"));
        assert!(!body.contains("<img src=x onerror=evil>"));
    }
}
