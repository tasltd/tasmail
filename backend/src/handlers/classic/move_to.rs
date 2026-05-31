// Added (TMAIL-372): POST /classic/folders/{folder}/messages/{uid}/move and
// the move branch on POST /classic/folders/{folder}/bulk — move-to-folder
// action for the no-JS Classic UI surface (driver TMAIL-299, gap-analysis
// `docs/gap-analysis/classic-ui.md` P1 #18).
//
// What this owns
// --------------
// Two flows that share a forbidden-target policy + a target-resolution path:
//
//   1. POST /classic/folders/{folder}/messages/{uid}/move
//      Form: `_csrf`, `target` (the destination folder name, as it appears
//      on the server's LIST output). Renders the friendly 400 page (shared
//      with the flag module's bulk action) when `target` is missing,
//      identical to the source folder, doesn't exist on the user's IMAP
//      server, or names a system folder whose IMAP semantics forbid moves
//      (e.g. `[Gmail]/All Mail`, `[Gmail]/Important` — these are virtual
//      labels on Gmail; copying messages there has no useful meaning and
//      typically silently fails). On success, 303-redirects back to the
//      SOURCE folder with `?moved=1&target=<encoded>&count=1` so the
//      folder view flashes the "Moved 1 message to <target>" banner.
//
//   2. POST /classic/folders/{folder}/bulk with `action=move&target=<dest>`
//      Same `target` validation as the single endpoint plus the standard
//      bulk-action UID set extraction (capped at `MAX_BULK_UIDS`). On
//      success, 303-redirects to the source folder with
//      `?moved=1&target=<encoded>&count=N`.
//
// Why share the bulk endpoint
// ---------------------------
// The bulk-action bar already POSTs every checked UID to one place
// (`/classic/folders/{folder}/bulk`) with a single button submission. Adding
// a sibling endpoint for move would force the bar to split into two forms,
// which would either need JavaScript to coordinate or land the user on a
// different submission shape depending on which button they clicked. The
// existing `post_bulk` already dispatches on the `action` field — we just
// add a new branch here.
//
// CSRF protection
// ---------------
// Both routes mount on the authenticated sub-router (`mod.rs`), so the CSRF
// `_csrf` field is validated upstream by `classic_csrf_middleware` before
// the handler runs. This file only worries about target validation and the
// IMAP COPY/STORE/EXPUNGE call.
//
// What this does NOT do (deferred)
// --------------------------------
//   * Detect `\Noselect` / special-use attribute from the LIST response —
//     `imap_service::Folder` doesn't surface attributes today. The forbidden
//     list below is a conservative name-pattern allowlist that covers Gmail's
//     virtual labels (the cited example in the gap-analysis spec) and the
//     common "All Mail" / "Important" / "Starred" virtual mailboxes other
//     servers ship. A polish task can extend `Folder` with the attribute set
//     and replace the name-pattern check.
//   * Per-row inline Move button on the folder view — the bulk action bar
//     handles the "select rows, pick target, click Move" flow.

use askama::Template;
use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Extension, Form,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::classic_session::ClassicSession;
use crate::services::auth_service::Claims;
use crate::services::imap_service::ImapService;
use crate::state::AppState;

use super::flag::{extract_uids, FlagErrorTemplate};
use super::CspNonce;

/// Form body for the single-message move endpoint.
///
/// `target` is the destination folder name as the IMAP server returned it
/// from LIST — the dropdown options on the message-read view template are
/// populated from the same source so a round-trip stays exact. Missing /
/// empty values are rejected via [`validate_target`] rather than a serde
/// `Option<String>` so the user lands on the friendly error page with a
/// specific reason instead of a 400-with-empty-body.
#[derive(Debug, Deserialize)]
pub struct MoveForm {
    pub target: String,
}

/// Outcome of [`validate_target`] — a closed enum so the handler can map
/// each failure mode to a precise user-facing message.
#[derive(Debug, PartialEq, Eq)]
pub enum TargetValidationError {
    /// The submitted `target` field was empty (or whitespace only).
    Missing,
    /// The user tried to move into the folder they're already in. IMAP
    /// would silently no-op the COPY+EXPUNGE here, but surfacing it as
    /// an error confirms that the user's intent was understood and the
    /// system declined for a specific reason.
    SameAsSource,
    /// The target wasn't in the user's resolved folder list. Most often a
    /// stale browser cache holding onto a deleted-folder option; rarely
    /// a forged form submission.
    NotFound,
    /// The target matches a known virtual / system folder whose IMAP COPY
    /// semantics are undefined or destructive. See [`is_forbidden_target`].
    Forbidden,
}

impl TargetValidationError {
    /// User-facing copy for the friendly error page. Wording is action-
    /// oriented ("Pick a different…") rather than judgemental ("Invalid…")
    /// so the user knows how to recover.
    pub fn user_message(&self, target: &str, source: &str) -> String {
        match self {
            Self::Missing => {
                "Pick a destination folder before clicking Move.".to_string()
            }
            Self::SameAsSource => format!(
                "You're already in {source:?}. Pick a different folder to move \
                 these messages into."
            ),
            Self::NotFound => format!(
                "The folder {target:?} isn't on your IMAP server. Pick one from \
                 the list and try again."
            ),
            Self::Forbidden => format!(
                "{target:?} is a virtual folder that can't accept moved messages. \
                 Pick a real folder (Inbox, Archive, a label you created)."
            ),
        }
    }
}

/// Conservative allowlist of name patterns we refuse to move INTO. Matched
/// case-insensitively against the trimmed target name.
///
/// Why these
/// ---------
///   * `[Gmail]/All Mail` — Gmail's virtual mailbox that contains every
///     message regardless of folder. COPY-ing to it doesn't move; it just
///     adds the "all mail" label which is already implicit. Cited in the
///     gap-analysis spec as the canonical example.
///   * `[Gmail]/Important`, `[Gmail]/Starred` — same family of virtual
///     labels; COPY semantics are undefined / silently no-op on Gmail.
///   * `INBOX/All Mail`, `[Gmail]` (the parent label itself) — defensive
///     against trailing-slash variants some clients emit.
///
/// A future enhancement can replace this name-pattern check by surfacing
/// LIST attributes (`\Noselect`, `\All`) from `imap_service::Folder` and
/// filtering on those — the attribute set is the IMAP-spec-defined signal.
/// Until then, this list covers the cited gap-analysis example plus the
/// most common virtual labels across the BYOK provider matrix.
const FORBIDDEN_TARGET_PATTERNS: &[&str] = &[
    "[Gmail]/All Mail",
    "[Gmail]/Important",
    "[Gmail]/Starred",
    "[Google Mail]/All Mail",
    "[Google Mail]/Important",
    "[Google Mail]/Starred",
    "INBOX/All Mail",
    "All Mail",
];

/// True when `target` matches a known virtual / system folder we refuse
/// to move INTO. Case-insensitive whole-name match; whitespace is trimmed
/// from the input so a forged `"All Mail "` still resolves.
pub fn is_forbidden_target(target: &str) -> bool {
    let trimmed = target.trim();
    FORBIDDEN_TARGET_PATTERNS
        .iter()
        .any(|p| p.eq_ignore_ascii_case(trimmed))
}

/// Validate that `target` is acceptable as a move destination given the
/// current `source` folder and the user's resolved IMAP folder list.
///
/// `available_folders` is the list of folder names from
/// `imap_service::list_folders` — passed in as a slice of `&str` so the
/// caller can supply borrows without an extra clone. Case-insensitive
/// match (IMAP folder names are case-sensitive per spec but every server
/// we target — Gmail, Outlook, Yahoo, Zoho, FastMail, iCloud, ProtonMail
/// Bridge, Dovecot, Stalwart — treats them case-insensitively for INBOX
/// + system folders; user-created folders preserve case but no two will
/// differ only by case in practice).
pub fn validate_target(
    target: &str,
    source: &str,
    available_folders: &[&str],
) -> Result<(), TargetValidationError> {
    let trimmed = target.trim();
    if trimmed.is_empty() {
        return Err(TargetValidationError::Missing);
    }
    if trimmed.eq_ignore_ascii_case(source.trim()) {
        return Err(TargetValidationError::SameAsSource);
    }
    if is_forbidden_target(trimmed) {
        return Err(TargetValidationError::Forbidden);
    }
    let exists = available_folders
        .iter()
        .any(|name| name.eq_ignore_ascii_case(trimmed));
    if !exists {
        return Err(TargetValidationError::NotFound);
    }
    Ok(())
}

/// Build the available-targets list shown in the message-read-view and
/// bulk-action dropdowns. Drops the current folder AND every forbidden
/// virtual mailbox so the user can't even pick an invalid option.
///
/// Order: input order preserved (`list_folders` returns the server's LIST
/// order, which already groups built-ins sensibly on Gmail / Outlook).
pub fn build_target_options(available_folders: &[String], current_folder: &str) -> Vec<String> {
    available_folders
        .iter()
        .filter(|name| !name.eq_ignore_ascii_case(current_folder.trim()))
        .filter(|name| !is_forbidden_target(name))
        .cloned()
        .collect()
}

/// Build the 303 redirect target for the "Moved N message(s) to <target>"
/// folder-view banner. Same URL shape for the single and bulk endpoints so
/// the folder template only has one banner branch.
///
/// `target` is percent-encoded here so a folder named `Sent Mail` survives
/// the round trip through the query string without breaking the
/// `<a href="?moved=…&target=…">` link rendering.
pub fn moved_redirect(folder_href: &str, target: &str, count: usize) -> String {
    let target_encoded = urlencoding::encode(target);
    format!(
        "/classic/folders/{folder_href}?moved=1&target={target_encoded}&count={count}",
    )
}

/// POST /classic/folders/{folder}/messages/{uid}/move — move a single
/// message into the target folder.
///
/// CSRF + auth are enforced upstream. Validation failures render the
/// friendly error page; success 303s back to the source folder.
pub async fn post_message_move(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Extension(session): Extension<ClassicSession>,
    Extension(csp_nonce): Extension<CspNonce>,
    Path((folder, uid)): Path<(String, u32)>,
    Form(form): Form<MoveForm>,
) -> Result<Response, AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID in classic claims")))?;

    let folder_href = urlencoding::encode(&folder).into_owned();

    let imap_service = ImapService::for_user(&state, mailbox_id).await?;
    let (username, password) = imap_service
        .user_creds()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("BYOK creds missing on ImapService")))?;
    let username = username.to_string();
    let password = password.to_string();

    let folders_list = imap_service.list_folders(&username, &password).await?;
    let folder_names: Vec<&str> = folders_list.iter().map(|f| f.name.as_str()).collect();

    if let Err(err) = validate_target(&form.target, &folder, &folder_names) {
        return render_move_error(
            &err.user_message(&form.target, &folder),
            &folder_href,
            &session.csrf_token,
            csp_nonce.into_string(),
        );
    }

    imap_service
        .move_message(&username, &password, &folder, uid, form.target.trim())
        .await?;

    let target = moved_redirect(&folder_href, form.target.trim(), 1);
    Ok((StatusCode::SEE_OTHER, [(header::LOCATION, target)]).into_response())
}

/// Handle the `action=move` branch from `flag::post_bulk`. Pulled out into
/// its own function (called from `flag::post_bulk` after the action dispatch)
/// so the bulk endpoint stays a single route while the per-action logic
/// lives next to its sibling code.
///
/// The caller has already parsed `pairs` once via the bulk-action dispatch
/// and is the one that resolved `pairs`. We re-derive `target` + `uid` here
/// to keep the function self-contained — the cost of one extra `.iter()`
/// pass on an already-in-memory form body is negligible.
pub async fn handle_bulk_move(
    state: AppState,
    claims: Claims,
    session: ClassicSession,
    csp_nonce: CspNonce,
    folder: String,
    pairs: Vec<(String, String)>,
) -> Result<Response, AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID in classic claims")))?;

    let folder_href = urlencoding::encode(&folder).into_owned();

    let target = pairs
        .iter()
        .find(|(k, _)| k == "target")
        .map(|(_, v)| v.clone())
        .unwrap_or_default();

    let uids = extract_uids(&pairs)?;
    if uids.is_empty() {
        return render_move_error(
            "No messages were selected. Tick the checkbox on at least one row \
             before clicking Move.",
            &folder_href,
            &session.csrf_token,
            csp_nonce.into_string(),
        );
    }

    let imap_service = ImapService::for_user(&state, mailbox_id).await?;
    let (username, password) = imap_service
        .user_creds()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("BYOK creds missing on ImapService")))?;
    let username = username.to_string();
    let password = password.to_string();

    let folders_list = imap_service.list_folders(&username, &password).await?;
    let folder_names: Vec<&str> = folders_list.iter().map(|f| f.name.as_str()).collect();

    if let Err(err) = validate_target(&target, &folder, &folder_names) {
        return render_move_error(
            &err.user_message(&target, &folder),
            &folder_href,
            &session.csrf_token,
            csp_nonce.into_string(),
        );
    }

    imap_service
        .move_message_batch(&username, &password, &folder, &uids, target.trim())
        .await?;

    let target_url = moved_redirect(&folder_href, target.trim(), uids.len());
    Ok((StatusCode::SEE_OTHER, [(header::LOCATION, target_url)]).into_response())
}

/// Render the friendly 400 page for move validation failures. Reuses the
/// flag module's `FlagErrorTemplate` because the layout + nav are identical
/// — only the message text differs.
fn render_move_error(
    message: &str,
    folder_href: &str,
    csrf_token: &str,
    csp_nonce: String,
) -> Result<Response, AppError> {
    let template = FlagErrorTemplate {
        message: message.to_string(),
        back_href: format!("/classic/folders/{folder_href}"),
        csrf_token: csrf_token.to_string(),
        csp_nonce,
    };
    let html = template.render().map_err(|e| {
        AppError::Internal(anyhow::anyhow!(
            "classic move-error template render failed: {e}"
        ))
    })?;
    Ok((
        StatusCode::BAD_REQUEST,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ───────────── is_forbidden_target ─────────────

    #[test]
    fn is_forbidden_target_rejects_gmail_all_mail() {
        // The canonical example cited in the gap-analysis P1 #18 spec.
        // Gmail surfaces `[Gmail]/All Mail` as a virtual label; COPY-ing
        // messages there is a no-op (they already carry that label) so we
        // refuse to even let the user pick it.
        assert!(is_forbidden_target("[Gmail]/All Mail"));
        // Some clients lowercase or re-cased; match case-insensitively.
        assert!(is_forbidden_target("[gmail]/all mail"));
        // Forged trailing whitespace shouldn't sneak past.
        assert!(is_forbidden_target("  [Gmail]/All Mail  "));
    }

    #[test]
    fn is_forbidden_target_rejects_other_virtual_labels() {
        assert!(is_forbidden_target("[Gmail]/Important"));
        assert!(is_forbidden_target("[Gmail]/Starred"));
        assert!(is_forbidden_target("[Google Mail]/All Mail"));
        assert!(is_forbidden_target("INBOX/All Mail"));
        assert!(is_forbidden_target("All Mail"));
    }

    #[test]
    fn is_forbidden_target_allows_normal_folders() {
        // Common user folders and the four pinned built-ins must pass —
        // a regression here would make the dropdown empty on every load.
        for ok in [
            "INBOX",
            "Drafts",
            "Sent",
            "Sent Items",
            "Trash",
            "Archive",
            "Junk",
            "Spam",
            "Projects/Work",
            "Receipts",
            "[Gmail]/Sent Mail", // Gmail's REAL sent folder is fine
            "[Gmail]/Drafts",
            "[Gmail]/Trash",
        ] {
            assert!(!is_forbidden_target(ok), "{ok:?} should not be forbidden");
        }
    }

    // ───────────── validate_target ─────────────

    fn folder_list() -> Vec<&'static str> {
        vec![
            "INBOX",
            "Drafts",
            "Sent",
            "Trash",
            "Archive",
            "Receipts",
            "Projects/Work",
            "[Gmail]/All Mail", // present in LIST but forbidden as target
        ]
    }

    #[test]
    fn validate_target_rejects_empty_field() {
        let folders = folder_list();
        assert_eq!(
            validate_target("", "INBOX", &folders).unwrap_err(),
            TargetValidationError::Missing
        );
        // Whitespace-only — the user clicked Move with the empty `(Move to…)`
        // placeholder still selected.
        assert_eq!(
            validate_target("   ", "INBOX", &folders).unwrap_err(),
            TargetValidationError::Missing
        );
    }

    #[test]
    fn validate_target_rejects_same_folder() {
        let folders = folder_list();
        assert_eq!(
            validate_target("INBOX", "INBOX", &folders).unwrap_err(),
            TargetValidationError::SameAsSource
        );
        // Case-insensitive — `inbox` from a stale option must still resolve.
        assert_eq!(
            validate_target("inbox", "INBOX", &folders).unwrap_err(),
            TargetValidationError::SameAsSource
        );
    }

    #[test]
    fn validate_target_rejects_missing_folder() {
        let folders = folder_list();
        // A folder name that's not on the server (stale browser cache or
        // forged submission) lands here.
        assert_eq!(
            validate_target("DoesNotExist", "INBOX", &folders).unwrap_err(),
            TargetValidationError::NotFound
        );
        // Subfolder name without its parent is also a miss.
        assert_eq!(
            validate_target("Work", "INBOX", &folders).unwrap_err(),
            TargetValidationError::NotFound
        );
    }

    #[test]
    fn validate_target_rejects_forbidden_virtual_folder() {
        let folders = folder_list();
        // `[Gmail]/All Mail` IS on the server's LIST output (it appears in
        // `folder_list`) — but the policy still refuses it as a move target.
        // Forbidden takes precedence over NotFound — order matters: a
        // forbidden target that ALSO doesn't exist on the server should still
        // surface as Forbidden so the user gets the more useful "pick a real
        // folder" hint rather than "this folder doesn't exist".
        assert_eq!(
            validate_target("[Gmail]/All Mail", "INBOX", &folders).unwrap_err(),
            TargetValidationError::Forbidden
        );
        assert_eq!(
            validate_target("All Mail", "INBOX", &folders).unwrap_err(),
            TargetValidationError::Forbidden
        );
    }

    #[test]
    fn validate_target_accepts_real_target() {
        let folders = folder_list();
        assert!(validate_target("Archive", "INBOX", &folders).is_ok());
        assert!(validate_target("Receipts", "Trash", &folders).is_ok());
        // Case-insensitive match against the folder list — Gmail's
        // `Drafts` vs `drafts` rendering shouldn't trip us.
        assert!(validate_target("drafts", "INBOX", &folders).is_ok());
        // Folder names with `/` (hierarchy) round-trip.
        assert!(validate_target("Projects/Work", "INBOX", &folders).is_ok());
    }

    // ───────────── TargetValidationError::user_message ─────────────

    #[test]
    fn user_message_missing_is_action_oriented() {
        // Wording check — the friendly page tells the user what to do,
        // not just "something was wrong". A regression here would lose
        // the user's ability to recover from the error.
        let msg = TargetValidationError::Missing.user_message("", "INBOX");
        assert!(msg.contains("Pick"), "missing-target copy must guide user: {msg:?}");
    }

    #[test]
    fn user_message_same_source_names_the_source() {
        let msg = TargetValidationError::SameAsSource.user_message("INBOX", "INBOX");
        assert!(msg.contains("INBOX"), "same-source copy must echo folder name: {msg:?}");
    }

    #[test]
    fn user_message_not_found_names_the_target() {
        let msg = TargetValidationError::NotFound.user_message("Phantom", "INBOX");
        assert!(msg.contains("Phantom"), "not-found copy must echo target: {msg:?}");
    }

    #[test]
    fn user_message_forbidden_names_the_target() {
        let msg = TargetValidationError::Forbidden.user_message("[Gmail]/All Mail", "INBOX");
        assert!(
            msg.contains("[Gmail]/All Mail") || msg.contains("Gmail"),
            "forbidden copy must echo target: {msg:?}"
        );
        // Should hint at the recovery path (a real folder) so the user
        // knows what to do next.
        assert!(
            msg.contains("real folder") || msg.contains("Inbox") || msg.contains("label"),
            "forbidden copy must guide recovery: {msg:?}"
        );
    }

    // ───────────── build_target_options ─────────────

    fn folder_strings() -> Vec<String> {
        vec![
            "INBOX".to_string(),
            "Drafts".to_string(),
            "Sent".to_string(),
            "Trash".to_string(),
            "Archive".to_string(),
            "Receipts".to_string(),
            "[Gmail]/All Mail".to_string(),
        ]
    }

    #[test]
    fn build_target_options_drops_current_folder() {
        let opts = build_target_options(&folder_strings(), "INBOX");
        assert!(!opts.iter().any(|n| n.eq_ignore_ascii_case("INBOX")));
        assert!(opts.iter().any(|n| n == "Archive"));
        assert!(opts.iter().any(|n| n == "Trash"));
    }

    #[test]
    fn build_target_options_drops_forbidden_virtual_folders() {
        // `[Gmail]/All Mail` shows up on the LIST output but must NOT
        // appear in the dropdown — otherwise the user could pick it,
        // submit, and land on the friendly error page. Filtering at
        // render time keeps the form free of invalid options.
        let opts = build_target_options(&folder_strings(), "INBOX");
        assert!(
            !opts.iter().any(|n| n.contains("All Mail")),
            "[Gmail]/All Mail must NOT appear in dropdown: {opts:?}"
        );
    }

    #[test]
    fn build_target_options_preserves_input_order() {
        // `list_folders` returns server LIST order which usually places
        // INBOX + system folders first; preserving that order keeps the
        // dropdown visually consistent with the sidebar.
        let opts = build_target_options(&folder_strings(), "INBOX");
        // After dropping INBOX (current) and `[Gmail]/All Mail` (forbidden):
        // Drafts, Sent, Trash, Archive, Receipts in that order.
        assert_eq!(
            opts,
            vec![
                "Drafts".to_string(),
                "Sent".to_string(),
                "Trash".to_string(),
                "Archive".to_string(),
                "Receipts".to_string(),
            ]
        );
    }

    #[test]
    fn build_target_options_empty_when_only_current_and_forbidden() {
        // Edge case: a fresh account with only INBOX + Gmail virtual labels.
        // The dropdown is allowed to be empty — the template will render a
        // disabled placeholder when so.
        let limited = vec![
            "INBOX".to_string(),
            "[Gmail]/All Mail".to_string(),
        ];
        let opts = build_target_options(&limited, "INBOX");
        assert!(opts.is_empty(), "expected no options, got: {opts:?}");
    }

    // ───────────── moved_redirect ─────────────

    #[test]
    fn moved_redirect_round_trips_target_and_count() {
        // Folder name in the path is already encoded by the caller; the
        // target gets encoded here so a name like `Sent Mail` survives the
        // query-string round trip without breaking link rendering.
        assert_eq!(
            moved_redirect("INBOX", "Archive", 1),
            "/classic/folders/INBOX?moved=1&target=Archive&count=1"
        );
        assert_eq!(
            moved_redirect("INBOX", "Sent Mail", 12),
            "/classic/folders/INBOX?moved=1&target=Sent%20Mail&count=12"
        );
    }

    #[test]
    fn moved_redirect_encodes_bracketed_target() {
        // `[Gmail]/Sent Mail` is a REAL (not forbidden) target on Gmail —
        // the brackets and slash must percent-encode in the query string.
        let url = moved_redirect("INBOX", "[Gmail]/Sent Mail", 3);
        assert!(url.contains("target=%5BGmail%5D%2FSent%20Mail"), "got: {url}");
        assert!(url.ends_with("&count=3"));
    }

    #[test]
    fn moved_redirect_preserves_encoded_source_folder() {
        // Source folder names with `/` (Gmail hierarchy) arrive already
        // encoded from the caller — we must NOT double-encode.
        let url = moved_redirect("%5BGmail%5D%2FSent%20Mail", "Archive", 2);
        assert_eq!(
            url,
            "/classic/folders/%5BGmail%5D%2FSent%20Mail?moved=1&target=Archive&count=2"
        );
    }
}
