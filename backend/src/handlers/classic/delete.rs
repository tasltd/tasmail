// Added (TMAIL-367): POST /classic/folders/{folder}/messages/{uid}/delete —
// single delete action for the no-JS Classic UI surface (driver TMAIL-299,
// gap-analysis `docs/gap-analysis/classic-ui.md` P0 #13).
//
// What this owns
// --------------
// One POST endpoint with two behaviours decided by the source folder:
//
//   1. `folder != trash_folder()` — the canonical "delete = move to Trash"
//      path. Calls `imap_service::move_message` to copy the UID into the
//      user's configured trash folder and `\Deleted`-then-EXPUNGE the
//      original. 303-redirects to `/classic/folders/{folder}?deleted=1`
//      so the message list page surfaces a one-time green banner.
//
//   2. `folder == trash_folder()` — already in Trash, so a delete is a
//      destructive permanent expunge. The handler renders a confirm page
//      first. The confirm page POSTs to the SAME endpoint with a
//      `confirm=1` hidden field; only then does the handler call
//      `imap_service::delete_message` (which permanent-deletes when
//      `folder == trash`) and 303-redirect to the Trash folder with
//      `?deleted=1`.
//
// CSRF protection
// ---------------
// Both POST hits flow through `classic_csrf_middleware` (wired in
// `handlers::classic::mod::authenticated_router`), so the `_csrf` field on
// both the message-read-view Delete form AND the confirm-page Confirm form
// is validated automatically. This handler only checks `confirm` —
// authentication and CSRF are already enforced upstream.
//
// What this does NOT do (deferred)
// --------------------------------
//   * "Empty Trash" (delete every message in Trash with one click) is a P1
//     follow-up — the data-model + UX for it lives in the bulk-delete task.
//   * Hard-coded "Trash" — the trash folder name is resolved via
//     `ImapService::trash_folder()` which honours the per-user
//     `imap_configurations.trash_folder` (Gmail "[Gmail]/Trash", Stalwart
//     "Deleted Items", iCloud "Deleted Messages", ...). Comparing against a
//     literal "Trash" would break BYOK on every non-Dovecot provider.
//
// Added (TMAIL-383): the bulk-delete branch on POST /classic/folders/{folder}/bulk
// is dispatched from `flag::post_bulk` when `action=delete` is submitted. It
// mirrors the single-message flow exactly: move-to-Trash from any source
// folder, OR (when source == trash) render a confirm page, and only on a
// follow-up POST with `confirm=1` does it permanent-expunge via IMAP. Lives
// in this file (not its own) because the trash-vs-not branching is identical
// to the single-message handler — keeping both pathways co-located avoids
// the trash-resolution + folder-comparison logic drifting apart.

use askama::Template;
use axum::{
    extract::{Path, State},
    http::StatusCode,
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

use super::CspNonce;

/// Form body shape for both flows. `_csrf` is validated by the CSRF
/// middleware upstream so it isn't surfaced here; `confirm` only matters
/// for the Trash-folder permanent-delete branch.
///
/// `serde(default)` on `confirm` means a missing field deserialises as
/// `None` rather than 400'ing the request — the initial Delete-button
/// submission carries no `confirm` field at all, and we want it to render
/// the confirm page rather than fail.
#[derive(Debug, Default, Deserialize)]
pub struct DeleteForm {
    #[serde(default)]
    pub confirm: Option<String>,
}

impl DeleteForm {
    /// True when the form's `confirm` field is set to a positive value.
    /// Centralised so the handler and the unit tests share one definition.
    pub fn is_confirmed(&self) -> bool {
        matches!(self.confirm.as_deref(), Some("1" | "true" | "yes"))
    }
}

/// Added (TMAIL-383): Askama template for the bulk-delete permanent-expunge
/// confirmation page. Rendered ONLY when the user clicks the bulk Delete
/// button from inside the resolved trash folder. The confirm form POSTs to
/// the same `/classic/folders/{folder}/bulk` endpoint with
/// `action=delete&confirm=1` + the same `uid` set so the handler can branch
/// straight into `imap_service::delete_message_batch`.
///
/// Source-folder != trash never lands here — bulk-delete from anywhere else
/// is a plain batch move-to-trash, mirroring the single-message flow.
#[derive(Template)]
#[template(path = "classic/bulk_delete_confirm.html")]
pub struct BulkDeleteConfirmTemplate {
    /// Display name of the folder (always the user's trash folder when this
    /// template renders).
    pub current_folder: String,
    /// URL-encoded path segment for the form action + cancel/back hrefs.
    pub current_folder_href: String,
    /// Number of UIDs about to be expunged. Rendered into the page title +
    /// alert banner + button label so the user gets a count-aware signal.
    pub count: u32,
    /// Every selected UID; rendered as a `<input type="hidden" name="uid">`
    /// per element so the confirm POST hits the same selection the user
    /// originally made. NOT trusted server-side — `flag::extract_uids` re-
    /// parses on the confirm POST so a forged hidden field can't sneak past.
    pub uids: Vec<u32>,
    /// POST action for the confirm form (the /bulk endpoint).
    pub form_action: String,
    /// Cancel href: back to the folder view (drops the selection — the user
    /// has to re-tick if they want to retry).
    pub cancel_href: String,
    /// Session CSRF token threaded into the confirm form AND the logout
    /// partial. Mandatory; an empty value would 403 the confirm.
    pub csrf_token: String,
    /// Per-request CSP nonce. Required by base.html.
    pub csp_nonce: String,
}

/// Askama template for the Trash-folder permanent-delete confirmation page.
///
/// Shown ONLY when the user POSTs the initial Delete from a message that
/// already lives in Trash. The page renders:
///   * Subject + From of the message about to be permanently deleted (so
///     the user can verify they're targeting the right row).
///   * A POST <form action="…/delete"> with `confirm=1` + the same `_csrf`
///     token threaded in, so submitting it lands back on this same handler
///     and goes through the IMAP delete branch.
///   * A "Cancel" <a> link back to the message read view.
#[derive(Template)]
#[template(path = "classic/delete_confirm.html")]
pub struct DeleteConfirmTemplate {
    /// Display name of the folder (always the user's trash folder when
    /// this template renders — but pass it verbatim for the cancel-link
    /// href so a custom Trash name surfaces correctly).
    pub current_folder: String,
    /// URL-encoded path segment for the folder, used in the form action,
    /// the cancel-link href, and the back-to-folder href.
    pub current_folder_href: String,
    /// IMAP UID of the message about to be deleted.
    pub uid: u32,
    /// Subject line of the targeted message, with the same `(no subject)`
    /// fallback the read view uses so the user sees a consistent label.
    pub subject: String,
    /// From header of the targeted message (already-formatted display
    /// string). Empty when the envelope has no From — the template hides
    /// the row in that case.
    pub from: String,
    /// POST action for the confirm form. Same URL as the original Delete
    /// button — the handler branches on `confirm=1`.
    pub form_action: String,
    /// Cancel href: the message read view the user came from.
    pub cancel_href: String,
    /// Session CSRF token threaded into the confirm form AND the logout
    /// partial. Mandatory; an empty value would 403 the confirm.
    pub csrf_token: String,
    /// Per-request CSP nonce. Required by base.html (TMAIL-356).
    pub csp_nonce: String,
}

/// POST /classic/folders/{folder}/messages/{uid}/delete
///
/// One handler, two branches:
///   * `folder != trash`: move the message to the user's trash folder
///     and 303 back to the source folder.
///   * `folder == trash` AND `confirm != 1`: render the confirm page.
///   * `folder == trash` AND `confirm == 1`: permanent-delete via IMAP
///     EXPUNGE, 303 back to the trash folder.
///
/// Auth + CSRF are enforced upstream by `classic_session_middleware` +
/// `classic_csrf_middleware`, so the handler only worries about the
/// folder/confirm routing.
pub async fn post_delete(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Extension(session): Extension<ClassicSession>,
    // Added (TMAIL-368): per-request CSP nonce for the confirm-page render
    // path. The Move-to-Trash + permanent-delete branches return 303 with
    // no body, so they don't need the nonce — only the confirm render does.
    Extension(csp_nonce): Extension<CspNonce>,
    Path((folder, uid)): Path<(String, u32)>,
    Form(form): Form<DeleteForm>,
) -> Result<Response, AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID in classic claims")))?;

    let imap_service = ImapService::for_user(&state, mailbox_id).await?;
    let (username, password) = imap_service
        .user_creds()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("BYOK creds missing on ImapService")))?;
    // Match the rest of the classic handlers: convert to owned `String` so
    // borrows on `imap_service.user_credentials` don't fight the `&self`
    // borrow the IMAP methods need across `.await`.
    let username = username.to_string();
    let password = password.to_string();

    let trash_folder = imap_service.trash_folder().to_string();
    let in_trash = folder == trash_folder;
    let folder_href = urlencoding::encode(&folder).into_owned();

    if !in_trash {
        // ── Branch 1 — Move to Trash ───────────────────────────────────
        // Per spec: "If folder != Trash: move the message to Trash via
        // imap_service::move, redirect back to source folder." Note that
        // `move_message` already handles the COPY → +FLAGS(\Deleted) →
        // EXPUNGE sequence atomically per UID, so a partial-failure can't
        // leave the source folder with a `\Deleted`-flagged ghost.
        imap_service
            .move_message(&username, &password, &folder, uid, &trash_folder)
            .await?;
        let target = deleted_redirect(&folder_href, 1);
        return Ok((StatusCode::SEE_OTHER, [(axum::http::header::LOCATION, target)]).into_response());
    }

    // ── Branch 2 — In Trash; needs explicit confirmation ───────────────
    if form.is_confirmed() {
        // Per spec: "only on confirm-form POST does imap_service::delete
        // actually run." `delete_message` permanent-deletes when the
        // source folder equals the resolved trash folder, which is exactly
        // the case we're in here. Calling `delete_message` (not
        // `move_message`) also future-proofs us if the IMAP service ever
        // grows extra side-effects (audit log, push notification, etc.).
        imap_service
            .delete_message(&username, &password, &folder, uid)
            .await?;
        let target = deleted_redirect(&folder_href, 1);
        return Ok((StatusCode::SEE_OTHER, [(axum::http::header::LOCATION, target)]).into_response());
    }

    // Need confirmation — fetch the message header to render a
    // friendlier confirm page. If the fetch fails (UID gone, IMAP
    // hiccup), `get_message` returns `AppError::Imap` which bubbles up
    // to the global error layer; the user lands on the generic error
    // page rather than a "click Delete to make it disappear" form
    // without context. That's the right tradeoff — we don't want to
    // delete by UID when we can't even verify the UID exists.
    let full = imap_service
        .get_message(&username, &password, &folder, uid)
        .await?;

    let subject = full
        .subject
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "(no subject)".to_string());
    let from = full.from.unwrap_or_default();

    let form_action = format!("/classic/folders/{folder_href}/messages/{uid}/delete");
    let cancel_href = format!("/classic/folders/{folder_href}/messages/{uid}");

    let template = DeleteConfirmTemplate {
        current_folder: folder,
        current_folder_href: folder_href,
        uid,
        subject,
        from,
        form_action,
        cancel_href,
        csrf_token: session.csrf_token.clone(),
        csp_nonce: csp_nonce.into_string(),
    };

    let html = template
        .render()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("classic delete-confirm template render failed: {e}")))?;

    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
        .into_response())
}

/// Added (TMAIL-383): Build the 303 redirect target for the "Deleted N
/// messages" folder-view banner. Shared between the single-message handler
/// (count=1, fired from `post_delete` after a successful move/expunge) and
/// the bulk handler (count=N). Keeps the folder template down to one banner
/// branch even though two distinct endpoints emit it.
pub fn deleted_redirect(folder_href: &str, count: usize) -> String {
    format!("/classic/folders/{folder_href}?deleted=1&count={count}")
}

/// Added (TMAIL-383): Handle the `action=delete` branch from `flag::post_bulk`.
///
/// Two source-folder branches that mirror the single-message `post_delete`:
///
///   * `folder != trash` — bulk move-to-Trash. Calls
///     `imap_service::move_message_batch` to COPY+EXPUNGE every UID into the
///     resolved trash folder in one round trip. 303-redirects back to the
///     SOURCE folder with `?deleted=1&count=N` so the folder banner reads
///     "Deleted N messages."
///
///   * `folder == trash` AND `confirm != 1` — render the bulk-confirm page
///     (`BulkDeleteConfirmTemplate`) which POSTs back to the same /bulk
///     endpoint with `action=delete&confirm=1` + the same `uid` set as
///     hidden inputs.
///
///   * `folder == trash` AND `confirm == 1` — bulk permanent-delete via
///     `imap_service::delete_message_batch`. 303-redirects to the trash
///     folder with `?deleted=1&count=N`.
///
/// CSRF + auth are enforced upstream by `classic_csrf_middleware` +
/// `classic_session_middleware`, so this handler only worries about target
/// resolution + the IMAP call sequence.
pub async fn handle_bulk_delete(
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

    // Re-parse the UID set off the form pairs every time — same shape as the
    // move handler, same trust boundary. A forged `confirm=1` POST with a
    // different uid set than the original confirm-page render still has to
    // pass `extract_uids` (which caps at MAX_BULK_UIDS) so the user can't
    // smuggle 10k UIDs into the EXPUNGE branch.
    let uids = super::flag::extract_uids(&pairs)?;
    if uids.is_empty() {
        return super::flag::FlagErrorTemplate {
            message: "No messages were selected. Tick the checkbox on at least one row \
                      before clicking Delete."
                .to_string(),
            back_href: format!("/classic/folders/{folder_href}"),
            csrf_token: session.csrf_token.clone(),
            csp_nonce: csp_nonce.into_string(),
        }
        .render()
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "classic flag-error template render failed: {e}"
            ))
        })
        .map(|html| {
            (
                StatusCode::BAD_REQUEST,
                [(
                    axum::http::header::CONTENT_TYPE,
                    "text/html; charset=utf-8",
                )],
                html,
            )
                .into_response()
        });
    }

    let imap_service = ImapService::for_user(&state, mailbox_id).await?;
    let (username, password) = imap_service
        .user_creds()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("BYOK creds missing on ImapService")))?;
    let username = username.to_string();
    let password = password.to_string();

    let trash_folder = imap_service.trash_folder().to_string();
    let in_trash = folder == trash_folder;

    // Confirmation flag — `serde_urlencoded` doesn't help us here since the
    // surrounding handler already buffered the body into `Vec<(String, String)>`
    // for the move dispatcher. Match the same positive-value set the single
    // delete handler accepts so the two endpoints stay in lockstep.
    let confirmed = pairs
        .iter()
        .find(|(k, _)| k == "confirm")
        .map(|(_, v)| matches!(v.as_str(), "1" | "true" | "yes"))
        .unwrap_or(false);

    if !in_trash {
        // ── Branch 1 — Bulk move-to-Trash ─────────────────────────────────
        // Matches the single-message flow exactly: from any source folder
        // other than the resolved trash, "Delete" means "move to Trash".
        // No confirm step — the messages remain recoverable via Trash.
        imap_service
            .move_message_batch(&username, &password, &folder, &uids, &trash_folder)
            .await?;
        let target = deleted_redirect(&folder_href, uids.len());
        return Ok((StatusCode::SEE_OTHER, [(axum::http::header::LOCATION, target)]).into_response());
    }

    // ── Branch 2 — Already in Trash; needs explicit confirmation ──────────
    if confirmed {
        imap_service
            .delete_message_batch(&username, &password, &folder, &uids)
            .await?;
        let target = deleted_redirect(&folder_href, uids.len());
        return Ok((StatusCode::SEE_OTHER, [(axum::http::header::LOCATION, target)]).into_response());
    }

    // Render the confirm page. The user can still cancel out (back to folder
    // drops the selection). Same destructive-action UX as the single-message
    // confirm page — no autofocus / default-submit, explicit button click
    // required to permanent-expunge.
    let form_action = format!("/classic/folders/{folder_href}/bulk");
    let cancel_href = format!("/classic/folders/{folder_href}");

    let template = BulkDeleteConfirmTemplate {
        current_folder: folder.clone(),
        current_folder_href: folder_href,
        count: uids.len() as u32,
        uids,
        form_action,
        cancel_href,
        csrf_token: session.csrf_token.clone(),
        csp_nonce: csp_nonce.into_string(),
    };

    let html = template.render().map_err(|e| {
        AppError::Internal(anyhow::anyhow!(
            "classic bulk-delete-confirm template render failed: {e}"
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

    // ───────────── DeleteForm::is_confirmed ─────────────

    #[test]
    fn is_confirmed_false_when_field_missing() {
        let form = DeleteForm { confirm: None };
        assert!(!form.is_confirmed());
    }

    #[test]
    fn is_confirmed_false_when_field_empty() {
        let form = DeleteForm {
            confirm: Some(String::new()),
        };
        assert!(!form.is_confirmed());
    }

    #[test]
    fn is_confirmed_false_for_negative_values() {
        for value in ["0", "false", "no", "off", "anything-else"] {
            let form = DeleteForm {
                confirm: Some(value.to_string()),
            };
            assert!(!form.is_confirmed(), "confirm={value:?} should not confirm");
        }
    }

    #[test]
    fn is_confirmed_true_for_positive_values() {
        for value in ["1", "true", "yes"] {
            let form = DeleteForm {
                confirm: Some(value.to_string()),
            };
            assert!(form.is_confirmed(), "confirm={value:?} should confirm");
        }
    }

    // ───────────── DeleteConfirmTemplate rendering ─────────────

    fn fresh_template() -> DeleteConfirmTemplate {
        DeleteConfirmTemplate {
            current_folder: "Trash".to_string(),
            current_folder_href: "Trash".to_string(),
            uid: 42,
            subject: "Old test message".to_string(),
            from: "Alice <alice@example.com>".to_string(),
            form_action: "/classic/folders/Trash/messages/42/delete".to_string(),
            cancel_href: "/classic/folders/Trash/messages/42".to_string(),
            csrf_token: "test-csrf-token".to_string(),
            csp_nonce: "test-nonce-fixed".to_string(),
        }
    }

    #[test]
    fn confirm_template_extends_base_layout() {
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
    fn confirm_template_has_zero_script_tags() {
        // Classic UI is no-JS — hard rule.
        let body = fresh_template().render().expect("template renders");
        assert!(
            !body.contains("<script"),
            "confirm template must contain ZERO <script> tags: {body}"
        );
    }

    #[test]
    fn confirm_template_renders_logout_form_with_csrf_token() {
        // Authenticated page → MUST override the logout_form block.
        let body = fresh_template().render().expect("template renders");
        assert!(
            body.contains("action=\"/classic/logout\""),
            "logout form must render on confirm page: {body}"
        );
        assert!(
            body.contains("value=\"test-csrf-token\""),
            "logout form must carry the session csrf_token: {body}"
        );
    }

    #[test]
    fn confirm_template_renders_subject_and_from() {
        let body = fresh_template().render().expect("template renders");
        // Subject is in the body and the <title>. Both serve the user's
        // "am I deleting the right thing?" check.
        assert!(body.contains("Old test message"));
        assert!(
            body.contains("Alice &#60;alice@example.com&#62;")
                || body.contains("Alice &lt;alice@example.com&gt;"),
            "From header must render (HTML-escaped): {body}"
        );
    }

    #[test]
    fn confirm_template_html_escapes_hostile_subject() {
        // Defence in depth — Askama auto-escapes for `.html`, but lock
        // the behaviour in so a config drift can't silently turn it off
        // and let a phisher's `<script>` escape into the confirm page.
        let mut t = fresh_template();
        t.subject = "<script>alert('xss')</script>".to_string();
        let body = t.render().expect("template renders");
        assert!(!body.contains("<script>alert('xss')</script>"));
        assert!(body.contains("&#60;script&#62;") || body.contains("&lt;script&gt;"));
    }

    #[test]
    fn confirm_template_renders_post_form_with_action_and_confirm_hidden_field() {
        let body = fresh_template().render().expect("template renders");
        // POST <form action="…/delete"> with the `confirm=1` hidden
        // field is the contract that drives the handler's confirmed
        // branch. Lock it down so a future template edit can't drop
        // the field and silently turn the confirm page into a "click
        // again to delete" no-op.
        assert!(
            body.contains("method=\"post\""),
            "confirm form must be POST: {body}"
        );
        assert!(
            body.contains("action=\"/classic/folders/Trash/messages/42/delete\""),
            "confirm form action must round-trip to the same delete endpoint: {body}"
        );
        assert!(
            body.contains("name=\"confirm\" value=\"1\""),
            "confirm hidden field must be present so the handler branches into permanent-delete: {body}"
        );
        assert!(
            body.contains("name=\"_csrf\" value=\"test-csrf-token\""),
            "confirm form must carry the session csrf_token: {body}"
        );
    }

    #[test]
    fn confirm_template_renders_cancel_link_back_to_message() {
        let body = fresh_template().render().expect("template renders");
        // Cancel is a plain GET <a> — idempotent navigation, no state
        // change, so it doesn't need to be a form. Lock the href down so
        // a future refactor doesn't accidentally drop the user on the
        // folder root (which loses the message context entirely).
        assert!(
            body.contains("href=\"/classic/folders/Trash/messages/42\""),
            "cancel link must point back at the message read view: {body}"
        );
        assert!(body.contains(">Cancel<"));
    }

    #[test]
    fn confirm_template_renders_destructive_warning_copy() {
        let body = fresh_template().render().expect("template renders");
        // The confirm page exists specifically to make the user pause
        // before a destructive action — lock the warning copy down so a
        // refactor doesn't quietly remove it and turn the page into a
        // one-click delete.
        assert!(
            body.contains("permanent")
                || body.contains("Permanent")
                || body.contains("cannot be undone"),
            "confirm page must surface destructive-action warning copy: {body}"
        );
    }

    #[test]
    fn confirm_template_omits_from_row_when_blank() {
        // Empty From shouldn't render an empty label that screen readers
        // announce. Matches the same pattern message.html uses for Cc.
        let mut t = fresh_template();
        t.from = String::new();
        let body = t.render().expect("template renders");
        assert!(
            !body.contains(">From<"),
            "From label must NOT render when from is empty: {body}"
        );
    }

    #[test]
    fn confirm_template_renders_destructive_button_label() {
        let body = fresh_template().render().expect("template renders");
        // Button copy MUST be the destructive verb ("Delete permanently"),
        // not a generic "OK" / "Confirm" that hides the consequence.
        assert!(
            body.contains("Delete permanently") || body.contains("Delete forever"),
            "confirm button must use a destructive verb so the user can't \
             misread it as a benign confirmation: {body}"
        );
    }

    #[test]
    fn confirm_template_renders_page_title_with_subject() {
        let body = fresh_template().render().expect("template renders");
        // Window title leads with the subject so the browser tab + back
        // history are scannable.
        assert!(
            body.contains("<title>Delete") && body.contains("Old test message"),
            "<title> must indicate destructive action AND surface the subject: {body}"
        );
    }
}
