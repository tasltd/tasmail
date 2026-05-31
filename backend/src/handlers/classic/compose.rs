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
// Per-file size cap + DLP / phishing pre-flight (TMAIL-382)
// ---------------------------------------------------------
// Changed (TMAIL-382, P1 #28):
//   * Per-file 10 MB cap on top of the 25 MB total. Enforced inside the
//     multipart loop via streaming `field.chunk()` reads so a single 50 MB
//     file is aborted before its full bytes hit RAM.
//   * Every part is run through `dlp_scanner::scan_content` (subject + body)
//     and `dlp_scanner::scan_attachments` (filenames vs DEFAULT_BLOCKED_EXTENSIONS)
//     before send. On a `Block` or `Quarantine` action the send is rejected
//     inline and the user's text fields are preserved in the re-render.
//   * Attachment filenames are also passed through
//     `phishing_scanner::scan_attachments` which catches the dangerous-
//     executable list (Outlook Safe Attachments parity) PLUS the double-
//     extension trick (`invoice.pdf.exe`) — that's what the spec means by
//     "existing virus/phishing-attachment hooks".
//   * The no-JS form fundamentally cannot round-trip attachment BYTES across
//     a failure re-render (the browser only echoes filenames). The error
//     copy in the template and the friendly error messages emitted here both
//     call out the re-attach step.

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
use crate::models::dlp_rule::{DlpAction, DlpRule};
use crate::models::signature::Signature;
use crate::services::auth_service::Claims;
use crate::services::dlp_scanner;
use crate::services::imap_service::ImapService;
use crate::services::phishing_scanner::{self, AttachmentMeta};
use crate::services::smtp_service::{OutgoingAttachment, SendRequest, SmtpService};
use crate::state::AppState;
use crate::validation::{self, MAX_MESSAGE_BODY_LEN};

use super::CspNonce;

// Added (TMAIL-378): RFC 3676 §4.3 signature separator. Exactly "-- "
// (dash, dash, space) followed by a line-feed — kept as a constant so the
// `append_signature` helper and its unit tests share one definition.
// `\n\n` precedes the separator so the user's body and the signature
// always sit in distinct paragraphs even when the body doesn't end in a
// newline.
pub const SIGNATURE_SEPARATOR: &str = "\n\n-- \n";

/// Hard cap on `body` + sum-of-attachment-sizes. Per gap-analysis P0 #12.
/// Surfaces as a friendly "Total upload too large" form-level error rather
/// than the generic 413 a body-limit layer would emit.
pub const MAX_TOTAL_BYTES: usize = 25 * 1024 * 1024;

/// Added (TMAIL-382): hard cap on the size of a SINGLE attachment file. Per
/// gap-analysis P1 #28 the default is 10 MB / file; the total stays at the
/// 25 MB `MAX_TOTAL_BYTES` cap above.
///
/// Enforced INSIDE the multipart `field.chunk()` streaming loop so a 50 MB
/// single file is rejected before its full bytes hit RAM. The same constant
/// is surfaced in the user-facing error message (no magic number duplication
/// between the check and the copy).
///
/// Sized below the total cap so a user can still attach multiple smaller
/// files up to the total — e.g. two 10 MB files would hit the total cap
/// after the second one, which is the friendlier failure mode.
pub const MAX_PER_FILE_BYTES: usize = 10 * 1024 * 1024;

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
    /// Added (TMAIL-384): Footer "Using X of Y · NN%" indicator. Hydrated
    /// by `super::load_quota_indicator`; `None` when the loader couldn't
    /// reach the cache + DB so the partial renders nothing in that branch.
    pub quota_indicator: Option<super::QuotaIndicator>,
}

impl ComposeTemplate {
    /// Blank-state factory used by GET /classic/compose on a fresh load.
    ///
    /// Changed (TMAIL-368): takes the per-request CSP nonce by argument so
    /// the inline `<style nonce="…">` on base.html matches the strict
    /// `/classic/*` response CSP header. Callers pull the nonce from
    /// `req.extensions().get::<CspNonce>()` via the `axum::Extension<CspNonce>`
    /// extractor.
    ///
    /// Changed (TMAIL-384): accepts an optional `quota_indicator` so the
    /// blank-state GET (an empty compose form) carries the same footer
    /// "Using X of Y" indicator every other authenticated page renders.
    /// Pass `None` from tests / contexts that don't need it.
    pub fn fresh(
        source_folder: impl Into<String>,
        csrf_token: impl Into<String>,
        csp_nonce: impl Into<String>,
        quota_indicator: Option<super::QuotaIndicator>,
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
            quota_indicator,
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

/// Added (TMAIL-378): suffix the user's default signature onto the body
/// being prefilled into the compose form, using the RFC 3676 §4.3
/// signature separator (`-- ` followed by a line-feed). Returns the
/// original body unchanged when the signature is empty.
///
/// Idempotent — if the body already ends with `{SIGNATURE_SEPARATOR}{sig}`
/// (modulo trailing whitespace), the input is returned untouched. That
/// matters for the failure-path re-render in `post_compose`: the user's
/// in-progress submission already contains the appended signature, and
/// we don't want a second copy to grow on every validation failure.
///
/// Pure function so unit tests can pin the formatting without spinning
/// up DB or IMAP.
pub fn append_signature(body: &str, signature_text: &str) -> String {
    // Trim trailing newlines AND spaces from the signature — a signature
    // that's whitespace-only must behave the same as an empty one (the
    // caller is paying for the lookup either way, but we shouldn't
    // emit a separator with nothing meaningful after it). The lead is
    // preserved so a `"\n\nBest,"` signature still renders correctly.
    let sig = signature_text.trim_end_matches(|c: char| c.is_whitespace());
    if sig.is_empty() {
        return body.to_string();
    }
    // Idempotency check: does the body already end with our separator +
    // signature? Compare on the trimmed-right version so trailing
    // whitespace doesn't defeat the check.
    let body_rtrim = body.trim_end_matches(|c: char| c == '\n' || c == '\r' || c == ' ');
    let expected_suffix = format!("{SIGNATURE_SEPARATOR}{sig}");
    if body_rtrim.ends_with(&expected_suffix) {
        return body.to_string();
    }
    // Strip trailing newlines from the body so we don't double-pad before
    // the separator (which starts with `\n\n`).
    let body_clean = body.trim_end_matches(|c: char| c == '\n' || c == '\r');
    format!("{body_clean}{SIGNATURE_SEPARATOR}{sig}")
}

/// Look up the user's default signature, if any. Internal-only; called
/// from `get_compose` to populate the auto-append. Logs and swallows DB
/// failures (returning `None`) so a transient DB blip doesn't gate the
/// user out of composing a new mail.
async fn load_default_signature_text(state: &AppState, mailbox_id: Uuid) -> Option<String> {
    match Signature::find_default(&state.db, mailbox_id).await {
        Ok(Some(sig)) => {
            // text_body is the canonical plain-text form persisted by
            // the classic signature handler — use it directly. (html_body
            // exists for rich rendering elsewhere but plain text is the
            // right thing to drop into a no-JS compose textarea.)
            let trimmed = sig.text_body.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(sig.text_body)
            }
        }
        Ok(None) => None,
        Err(e) => {
            tracing::warn!(
                error = ?e,
                user_id = ?mailbox_id,
                "classic compose: default signature lookup failed — composing without auto-append"
            );
            None
        }
    }
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
    // Added (TMAIL-384): hydrate the footer quota indicator once per
    // GET so it stays consistent with the rest of the authenticated
    // surface. The indicator is built BEFORE any IMAP prefill so a slow
    // prefill doesn't block the page render — both are awaited
    // sequentially today; future optimisation may join them.
    let quota_indicator = super::load_quota_indicator(&state, mailbox_id).await;
    let mut template = ComposeTemplate::fresh(
        &folder,
        &session.csrf_token,
        csp_nonce.as_str(),
        quota_indicator,
    );

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

    // Added (TMAIL-378): auto-append the user's default signature (if any)
    // using the RFC 3676 §4.3 separator. Applies uniformly to blank
    // compose, reply, reply-all, and forward — the prefilled body already
    // contains the quoted original (reply/forward) or is empty (blank),
    // and the signature lands below either way. The user can edit or
    // delete it inline before sending.
    if let Some(sig_text) = load_default_signature_text(&state, mailbox_id).await {
        template.body = append_signature(&template.body, &sig_text);
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

    while let Some(mut field) = multipart
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
            // File part. Changed (TMAIL-382): stream chunks via
            // `field.chunk()` rather than buffering the whole field with
            // `field.bytes()`. The per-file and rolling-total caps are
            // enforced INSIDE the loop so a 50 MB single attachment is
            // rejected before its full bytes hit RAM.
            //
            // `filename_for_error` is the user-facing label used in the
            // error copy when the cap is breached. We trim it now so the
            // "Attachment 'X' too large" message reads naturally.
            let filename_for_error = if fname.trim().is_empty() {
                DEFAULT_FILENAME.to_string()
            } else {
                fname.trim().to_string()
            };

            let mut buffer: Vec<u8> = Vec::new();
            loop {
                match field.chunk().await {
                    Ok(Some(chunk)) => {
                        let next_file_size = buffer.len().saturating_add(chunk.len());
                        if next_file_size > MAX_PER_FILE_BYTES {
                            return Err(AppError::BadRequest(format!(
                                "Attachment '{}' exceeds the {} MB per-file limit. \
                                 Total across all attachments must also stay under {} MB.",
                                filename_for_error,
                                MAX_PER_FILE_BYTES / (1024 * 1024),
                                MAX_TOTAL_BYTES / (1024 * 1024)
                            )));
                        }
                        // Project the running total INCLUDING this in-progress
                        // file so we abort the moment the combined size would
                        // cross the cap — not after this attachment is fully
                        // buffered.
                        let projected_total = submission
                            .total_bytes
                            .saturating_add(next_file_size);
                        if projected_total > MAX_TOTAL_BYTES {
                            return Err(AppError::BadRequest(format!(
                                "Total upload exceeds the {} MB limit.",
                                MAX_TOTAL_BYTES / (1024 * 1024)
                            )));
                        }
                        buffer.extend_from_slice(&chunk);
                    }
                    Ok(None) => break,
                    Err(e) => {
                        return Err(AppError::BadRequest(format!(
                            "Failed to read attachment '{}': {e}",
                            filename_for_error
                        )));
                    }
                }
            }
            // Empty filename slot — the user clicked Browse, picked nothing,
            // and the browser sent an empty file part. Skip rather than
            // record a zero-byte attachment.
            if buffer.is_empty() {
                continue;
            }
            submission.total_bytes = submission.total_bytes.saturating_add(buffer.len());
            submission.attachments.push(OutgoingAttachment {
                filename: filename_for_error,
                content_type: content_type_attr,
                data: buffer,
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

/// Added (TMAIL-382): pure check that flags an outgoing attachment list for
/// dangerous filenames. Combines the operator-configurable DLP blocked-
/// extension list with the phishing-scanner heuristic (Outlook Safe
/// Attachments parity + double-extension trick). Returns the FIRST blocker
/// so the user sees a concrete reason for the first bad file rather than a
/// soup of all of them.
///
/// Pulled out as a pure function so the test suite can exercise it without
/// standing up a DB.
pub fn check_attachments_for_blockers(
    attachments: &[OutgoingAttachment],
) -> Result<(), AppError> {
    let filenames: Vec<String> = attachments.iter().map(|a| a.filename.clone()).collect();
    let dlp_attachment_hits = dlp_scanner::scan_attachments(
        &filenames,
        dlp_scanner::DEFAULT_BLOCKED_EXTENSIONS,
    );
    if let Some(hit) = dlp_attachment_hits
        .iter()
        .find(|m| matches!(m.action, DlpAction::Block | DlpAction::Quarantine))
    {
        return Err(AppError::BadRequest(format!(
            "Attachment '{}' is blocked by your organisation's data-loss-prevention policy \
             ({}). Remove it and resend; the rest of your message has been kept.",
            hit.matched_text, hit.rule_name
        )));
    }

    // Phishing heuristic — catches `invoice.pdf.exe` even when only `.exe`
    // is in the DLP list (the DLP scanner only inspects the last extension).
    let phishing_metas: Vec<AttachmentMeta> = attachments
        .iter()
        .map(|a| AttachmentMeta {
            filename: a.filename.clone(),
            content_type: Some(a.content_type.clone()),
        })
        .collect();
    let dangerous = phishing_scanner::scan_attachments(&phishing_metas);
    if let Some(d) = dangerous.first() {
        return Err(AppError::BadRequest(format!(
            "Attachment '{}' was blocked: {}. Remove it and resend; the rest of your \
             message has been kept.",
            d.filename, d.reason
        )));
    }

    Ok(())
}

/// Added (TMAIL-382): pure check that runs DLP content scanning against the
/// subject + body. Caller threads in the active rule set (typically
/// `DlpRule::list_active(&state.db).await?`); built-in CC/SSN/IBAN patterns
/// are scanned by `dlp_scanner::scan_content` regardless of the rule list.
///
/// Returns `Err` on the first `Block`/`Quarantine` match. The error copy
/// names the rule but DOESN'T echo the matched text — that would teach a
/// hostile sender what to obfuscate.
pub fn check_content_for_blockers(
    rules: &[DlpRule],
    subject: &str,
    body: &str,
) -> Result<(), AppError> {
    let content_hits = dlp_scanner::scan_content(rules, Some(subject), Some(body));
    if let Some(hit) = content_hits
        .iter()
        .find(|m| matches!(m.action, DlpAction::Block | DlpAction::Quarantine))
    {
        return Err(AppError::BadRequest(format!(
            "Your message was blocked by your organisation's data-loss-prevention policy \
             ({}). Edit the subject or body and resend; recipients and attachments are kept.",
            hit.rule_name
        )));
    }
    Ok(())
}

/// Added (TMAIL-382): pre-flight scan that gates the outgoing message
/// against the DLP rules + the phishing/virus attachment heuristics. Mirrors
/// what the SPA-side send path will land in the BYOK-send refactor — the
/// classic surface gets it first because no-JS users have no client-side
/// telemetry to fall back on if a bad attachment slips through.
///
/// Returns `Ok(())` on a clean message, `Err(AppError::BadRequest(...))` on
/// the first blocking match. The `BadRequest` path is what the failure
/// re-render in `post_compose` consumes — body + recipients land back on
/// the page intact, but attachments have to be re-picked.
///
/// Scan order (cheapest first so the user gets fast feedback):
///   1. Attachment-name checks (DLP blocked-extension + phishing dangerous)
///   2. DLP subject + body scan against active DB rules + built-in patterns
///
/// DLP rule fetch failure is downgraded to a tracing warning — the user
/// shouldn't be locked out of sending mail just because the rules table is
/// unreachable. The built-in patterns (CC, SSN, IBAN) still run because
/// they don't need a DB query.
async fn scan_outgoing_for_blockers(
    state: &AppState,
    subject: &str,
    body: &str,
    attachments: &[OutgoingAttachment],
) -> Result<(), AppError> {
    check_attachments_for_blockers(attachments)?;

    let rules = match DlpRule::list_active(&state.db).await {
        Ok(rs) => rs,
        Err(e) => {
            tracing::warn!(
                error = ?e,
                "classic compose: DLP rule fetch failed — running built-in patterns only"
            );
            Vec::new()
        }
    };
    check_content_for_blockers(&rules, subject, body)?;
    Ok(())
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

    // Added (TMAIL-384): hydrate the footer quota indicator once at the
    // top of post_compose so every error re-render branch (4 of them)
    // carries the same indicator the GET path would have shown. Cheap —
    // cache-hit path is sub-ms; cache miss runs two indexed Postgres
    // queries. Cloned at each render site rather than re-fetched.
    let quota_indicator = super::load_quota_indicator(&state, mailbox_id).await;

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
                quota_indicator.clone(),
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

    // Added (TMAIL-382): DLP + phishing/virus pre-flight. Runs BEFORE the
    // send-request build so a block hit leaves the multipart payload untouched
    // and the user gets a friendly inline error rather than a generic SMTP
    // bounce after the bytes already left the machine. The text fields land
    // back on the re-render via the same path as a malformed-recipient error;
    // attachment bytes can't round-trip on a no-JS form so the template asks
    // the user to re-attach (and explains why).
    if let Err(e) = scan_outgoing_for_blockers(
        &state,
        &subject,
        &body,
        &submission.attachments,
    )
    .await
    {
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
            quota_indicator.clone(),
        );
    }

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
                quota_indicator.clone(),
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
            quota_indicator,
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
    // Added (TMAIL-384): Optional footer quota indicator. The POST handler
    // hydrates it before calling render_error_form so the error re-render
    // keeps the same indicator the GET path would have shown.
    quota_indicator: Option<super::QuotaIndicator>,
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
        quota_indicator,
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
        // Changed (TMAIL-384): `fresh` now also takes the optional
        // QuotaIndicator — tests pass `None` so the existing assertions
        // still match the rendered HTML exactly; dedicated render tests
        // for the indicator live in `context::tests`.
        ComposeTemplate::fresh(
            "INBOX",
            "test-csrf-token",
            "test-nonce-fixed-for-tests",
            None,
        )
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
        let t = ComposeTemplate::fresh("Sent", "test-csrf-token", "test-nonce-fixed-for-tests", None);
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

    // ----- append_signature (TMAIL-378) -----

    #[test]
    fn signature_separator_is_rfc3676() {
        // RFC 3676 §4.3: "-- " followed by CRLF (or LF on POSIX text
        // bodies). The dash-dash-space marker is what every desktop mail
        // client recognises as the signature delimiter.
        assert_eq!(SIGNATURE_SEPARATOR, "\n\n-- \n");
    }

    #[test]
    fn append_signature_returns_body_unchanged_when_sig_empty() {
        assert_eq!(append_signature("Hello", ""), "Hello");
        assert_eq!(append_signature("Hello", "   "), "Hello");
        assert_eq!(append_signature("Hello", "\n\n"), "Hello");
    }

    #[test]
    fn append_signature_appends_to_blank_body() {
        let result = append_signature("", "Best,\nKwame");
        assert_eq!(result, "\n\n-- \nBest,\nKwame");
    }

    #[test]
    fn append_signature_appends_after_existing_body() {
        let result = append_signature("Body text", "Best,\nKwame");
        assert_eq!(result, "Body text\n\n-- \nBest,\nKwame");
    }

    #[test]
    fn append_signature_collapses_trailing_newlines_before_separator() {
        // The separator already starts with `\n\n`; if the body has a
        // trailing newline we'd otherwise get three blank lines.
        let result = append_signature("Body text\n\n\n", "Sig");
        assert_eq!(result, "Body text\n\n-- \nSig");
    }

    #[test]
    fn append_signature_idempotent_on_already_appended_body() {
        let first = append_signature("Body", "Sig");
        let second = append_signature(&first, "Sig");
        assert_eq!(
            first, second,
            "double-append should be a no-op: first={first:?} second={second:?}"
        );
    }

    #[test]
    fn append_signature_idempotent_when_body_has_trailing_whitespace() {
        let appended = append_signature("Body", "Sig");
        let with_trailing_ws = format!("{appended}   \n\n");
        let again = append_signature(&with_trailing_ws, "Sig");
        // The trailing whitespace on the in-progress body must NOT
        // trigger a second append.
        assert!(
            again.matches("-- \nSig").count() == 1,
            "expected exactly one signature, got: {again:?}"
        );
    }

    #[test]
    fn append_signature_re_appends_when_user_changed_the_signature() {
        // A "Sig v1" body should grow a "Sig v2" suffix when the saved
        // signature changes — idempotency is per-signature, not "any
        // signature".
        let appended = append_signature("Body", "Sig v1");
        let new_signature = append_signature(&appended, "Sig v2");
        assert!(
            new_signature.contains("Sig v1"),
            "old signature should still appear in user's body: {new_signature:?}"
        );
        assert!(
            new_signature.contains("Sig v2"),
            "new signature should be appended: {new_signature:?}"
        );
    }

    #[test]
    fn append_signature_works_on_reply_quoted_body() {
        // The reply body shape is `\n\n{attribution}\n{quoted}\n` —
        // appending the signature should slot in cleanly at the end.
        let reply_body = build_quoted_body(
            Some("Alice <a@x.com>"),
            Some("Mon, 1 Jan 2026"),
            Some("Original line"),
            PrefillMode::Reply,
        );
        let result = append_signature(&reply_body, "Sig");
        // The signature appears AFTER the quoted block.
        assert!(result.ends_with("-- \nSig"), "result: {result}");
        // The quoted body is still intact.
        assert!(result.contains("> Original line"));
    }

    #[test]
    fn append_signature_works_on_forward_banner_body() {
        let fwd_body = build_quoted_body(
            Some("Alice"),
            Some("Date"),
            Some("Original line"),
            PrefillMode::Forward,
        );
        let result = append_signature(&fwd_body, "Sig");
        assert!(result.ends_with("-- \nSig"), "result: {result}");
        assert!(result.contains("---------- Forwarded message ----------"));
    }

    // ----- Per-file + total size caps (TMAIL-382) -----

    #[test]
    fn per_file_cap_is_ten_megabytes() {
        // The gap-analysis P1 #28 spec pins this at 10 MB / file. If this
        // constant is ever bumped the user-facing help text in
        // `templates/classic/compose.html` and the rejection error copy
        // must move with it — those messages embed the megabyte value.
        assert_eq!(MAX_PER_FILE_BYTES, 10 * 1024 * 1024);
    }

    #[test]
    fn per_file_cap_strictly_below_total_cap() {
        // A single attachment at the per-file cap must fit under the total
        // cap with room to spare for the body. Otherwise users couldn't
        // attach the file AND type anything.
        assert!(
            MAX_PER_FILE_BYTES < MAX_TOTAL_BYTES,
            "per-file cap {MAX_PER_FILE_BYTES} must be strictly less than total cap {MAX_TOTAL_BYTES}",
        );
    }

    // ----- check_attachments_for_blockers (TMAIL-382) -----

    fn att(filename: &str) -> OutgoingAttachment {
        OutgoingAttachment {
            filename: filename.to_string(),
            content_type: "application/octet-stream".to_string(),
            data: vec![0, 1, 2, 3],
        }
    }

    #[test]
    fn attachments_clean_list_passes() {
        let attachments = vec![att("report.pdf"), att("photo.jpg"), att("notes.txt")];
        check_attachments_for_blockers(&attachments)
            .expect("clean attachment list should pass the scan");
    }

    #[test]
    fn attachments_blocks_executable_extension_dlp() {
        // .exe is in DEFAULT_BLOCKED_EXTENSIONS — must reject.
        let attachments = vec![att("report.pdf"), att("malware.exe")];
        let err = check_attachments_for_blockers(&attachments)
            .expect_err(".exe must be blocked");
        match err {
            AppError::BadRequest(msg) => {
                assert!(
                    msg.contains("malware.exe"),
                    "error must name the offending filename: {msg}"
                );
                assert!(
                    msg.contains("data-loss-prevention") || msg.contains("blocked"),
                    "error must mention DLP/block reason: {msg}"
                );
                assert!(
                    msg.contains("rest of your message has been kept"),
                    "error must reassure the user the body/recipients survived: {msg}"
                );
            }
            other => panic!("expected BadRequest, got {other:?}"),
        }
    }

    #[test]
    fn attachments_blocks_double_extension_pdf_exe() {
        // `invoice.pdf.exe` — the DLP scanner ALSO catches `.exe` here, but
        // the test pins that the phishing-attachment check would still kick
        // in if the operator removed `.exe` from the DLP list (the dangerous-
        // extension list in phishing_scanner is independent).
        let attachments = vec![att("invoice.pdf.exe")];
        let err = check_attachments_for_blockers(&attachments)
            .expect_err("double-extension exe must be blocked");
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn attachments_blocks_vbs_script() {
        // .vbs is a Visual Basic Script — classic malware vector.
        let attachments = vec![att("clean.txt"), att("update.vbs")];
        let err = check_attachments_for_blockers(&attachments)
            .expect_err(".vbs must be blocked");
        if let AppError::BadRequest(msg) = err {
            assert!(
                msg.contains("update.vbs"),
                "error must name the offending file: {msg}"
            );
        } else {
            panic!("expected BadRequest");
        }
    }

    #[test]
    fn attachments_first_blocker_wins() {
        // When multiple bad attachments are in play, the first one in the
        // list is the one the user is told about. This keeps the error
        // copy concise — they can re-try and see the next one if they hit
        // multiple in one submission.
        let attachments = vec![att("first.exe"), att("second.bat")];
        let err = check_attachments_for_blockers(&attachments)
            .expect_err("first blocker should reject");
        if let AppError::BadRequest(msg) = err {
            assert!(
                msg.contains("first.exe"),
                "first blocker should be named, got: {msg}"
            );
        } else {
            panic!("expected BadRequest");
        }
    }

    #[test]
    fn attachments_case_insensitive_extension_match() {
        // .EXE in uppercase should still be blocked. (DLP scanner uses
        // case-insensitive comparison via `eq_ignore_ascii_case`.)
        let attachments = vec![att("LAUNCHER.EXE")];
        let err = check_attachments_for_blockers(&attachments)
            .expect_err("uppercase .EXE must be blocked");
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    // ----- check_content_for_blockers (TMAIL-382) -----

    #[test]
    fn content_clean_body_passes() {
        // No rules + clean body → no built-in pattern matches → pass.
        let rules: Vec<DlpRule> = vec![];
        check_content_for_blockers(
            &rules,
            "Lunch tomorrow",
            "Hey team, sandwich place at 12:30. — Kwame",
        )
        .expect("clean body should pass the scan");
    }

    #[test]
    fn content_blocks_credit_card_in_body_via_builtin() {
        // The credit-card built-in pattern action is `Block` — even with no
        // DB rules, a Visa-format CC in the body must block.
        let rules: Vec<DlpRule> = vec![];
        let err = check_content_for_blockers(
            &rules,
            "Invoice",
            "Charge my card 4111-1111-1111-1111 thanks",
        )
        .expect_err("credit card must block via built-in");
        if let AppError::BadRequest(msg) = err {
            assert!(
                msg.contains("data-loss-prevention"),
                "error must call out DLP: {msg}"
            );
            // Don't echo the matched CC bytes back — the test guards the
            // operational rule that we don't teach attackers what to obfuscate.
            assert!(
                !msg.contains("4111-1111-1111-1111"),
                "error must NOT echo the matched secret: {msg}"
            );
            assert!(
                msg.contains("recipients and attachments are kept"),
                "error must reassure recipients/attachments survive: {msg}"
            );
        } else {
            panic!("expected BadRequest");
        }
    }

    #[test]
    fn content_blocks_us_ssn_in_body_via_builtin() {
        let rules: Vec<DlpRule> = vec![];
        let err = check_content_for_blockers(
            &rules,
            "Onboarding",
            "My SSN is 123-45-6789 please use this for the form",
        )
        .expect_err("US SSN must block via built-in");
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn content_warn_severity_does_not_block_send() {
        // The IBAN built-in is action=Warn (not Block) — Warns must NOT
        // gate the send. Same for any DB rule whose action is Warn/Log.
        let rules: Vec<DlpRule> = vec![];
        check_content_for_blockers(
            &rules,
            "Wire transfer",
            "Please send to DE89370400440532013000",
        )
        .expect("Warn-severity matches must not gate the send");
    }

    #[test]
    fn content_blocks_when_db_rule_has_block_action() {
        // A custom DB-defined rule with `Block` action must reject even if
        // the body doesn't trip any built-in pattern.
        let rule = DlpRule {
            id: Uuid::new_v4(),
            name: "Project Phoenix".to_string(),
            description: None,
            pattern: "project phoenix".to_string(),
            pattern_type: "keyword".to_string(),
            action: DlpAction::Block,
            severity: crate::models::dlp_rule::DlpSeverity::Critical,
            apply_to_subject: true,
            apply_to_body: true,
            apply_to_attachments: false,
            active: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let err = check_content_for_blockers(
            &[rule],
            "Project Phoenix update",
            "Status memo for the team",
        )
        .expect_err("custom Block-action rule must block");
        if let AppError::BadRequest(msg) = err {
            assert!(
                msg.contains("Project Phoenix"),
                "error must name the matched rule: {msg}"
            );
        } else {
            panic!("expected BadRequest");
        }
    }

    #[test]
    fn content_blocks_when_db_rule_has_quarantine_action() {
        // Quarantine treated the same as Block at the send-time gate — the
        // user has no quarantine UI on no-JS, so the only useful behaviour
        // is to refuse to send and let them edit the message.
        let rule = DlpRule {
            id: Uuid::new_v4(),
            name: "Internal Code Name".to_string(),
            description: None,
            pattern: "operation polaris".to_string(),
            pattern_type: "keyword".to_string(),
            action: DlpAction::Quarantine,
            severity: crate::models::dlp_rule::DlpSeverity::High,
            apply_to_subject: false,
            apply_to_body: true,
            apply_to_attachments: false,
            active: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let err = check_content_for_blockers(
            &[rule],
            "Status update",
            "Notes on operation polaris timeline",
        )
        .expect_err("Quarantine-action rule must also gate the send");
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    // ----- Template + error-render verification (TMAIL-382) -----

    #[test]
    fn compose_template_help_text_mentions_per_file_limit() {
        // The user-facing help text under the file input has to surface the
        // 10 MB / file cap so a user picking a 20 MB video knows in advance
        // it won't go through.
        let body = fresh_template().render().expect("template renders");
        assert!(
            body.contains("10 MB per file"),
            "compose help text must mention the per-file cap: {body}"
        );
        assert!(
            body.contains("25 MB") || body.contains("Total upload"),
            "compose help text must still mention the total cap: {body}"
        );
        // Mention of the DLP / executable rejection so users aren't
        // surprised mid-compose. Stated declaratively, not threateningly.
        assert!(
            body.contains(".exe") || body.contains("Executable"),
            "compose help text must mention executable-file rejection: {body}"
        );
    }

    #[test]
    fn compose_template_reattach_note_explains_no_js_limitation() {
        // When attachments need to be re-picked, the failure notice has to
        // explain WHY (no-JS can't preserve bytes) — otherwise it looks like
        // a bug and a frustrated user will assume the upload itself failed.
        let mut t = fresh_template();
        t.prior_attachment_names = vec!["doc.pdf".to_string()];
        let body = t.render().expect("template renders");
        assert!(
            body.contains("re-attach"),
            "failure note must use the 're-attach' verb: {body}"
        );
        assert!(
            body.contains("JavaScript") || body.contains("can't hold"),
            "failure note must explain the no-JS attachment limitation: {body}"
        );
    }
}
