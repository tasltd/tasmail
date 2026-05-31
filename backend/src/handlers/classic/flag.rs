// Added (TMAIL-370): POST /classic/folders/{folder}/messages/{uid}/flag and
// POST /classic/folders/{folder}/bulk — mark read / unread for the no-JS
// Classic UI surface (driver TMAIL-299, gap-analysis
// `docs/gap-analysis/classic-ui.md` P1 #16 + P1 #29 partial).
//
// Extended (TMAIL-371 / P1 #17): the same two endpoints now also toggle
// the IMAP `\Flagged` flag (star / unstar). The single endpoint accepts
// `mark=star` and `mark=unstar` alongside the existing read/unread values;
// the bulk endpoint accepts `action=star` and `action=unstar` alongside
// `mark_read` / `mark_unread`. A single MarkAction enum carries both the
// IMAP flag name (\Seen vs \Flagged) and the add/strip direction so each
// handler's IMAP call site stays one-line.
//
// What this owns
// --------------
// Two endpoints, each toggling either the `\Seen` (read/unread) or
// `\Flagged` (star/unstar) IMAP flag:
//
//   1. POST /classic/folders/{folder}/messages/{uid}/flag
//      Form: `_csrf`, `mark` = "read" | "unread" | "star" | "unstar".
//      Backs the "Mark unread" + "Star/Unstar" buttons rendered in the
//      message read view (TMAIL-363 / TMAIL-371). Redirects to the folder
//      view with `?marked=read&count=1`, `?marked=unread&count=1`,
//      `?marked=starred&count=1` or `?marked=unstarred&count=1` so the
//      folder banner flashes a one-time confirmation.
//
//   2. POST /classic/folders/{folder}/bulk
//      Form: `_csrf`, `action` = "mark_read" | "mark_unread" | "star" |
//      "unstar", plus `uid` (repeats once per checked row).
//      Backs the bulk-action bar on the folder view (TMAIL-362 /
//      TMAIL-371). Redirects back to the folder with the same `?marked=…`
//      shape so the same banner code covers both flows.
//
// Why two endpoints (and not one)
// -------------------------------
// The single endpoint sits on the message-scoped path so the read view's
// Delete button + the Star button + Mark-unread button all share one URL
// shape (`…/messages/{uid}/{action}`). The bulk endpoint lives at the
// folder level because the rows on a single page may target different
// uids — embedding the uid in the path would be a lie.
//
// CSRF protection
// ---------------
// Both routes mount on the authenticated sub-router (mod.rs), which means
// `classic_csrf_middleware` validates the `_csrf` field upstream before the
// handler runs. This file only worries about the routing and the IMAP call.
//
// What this does NOT do (deferred)
// --------------------------------
//   * The other buttons on the bulk-action bar (Move…, Delete) — those are
//     P1 #18 (TMAIL-380) and P1 #29 follow-ups, both of which will mount
//     more `action` branches on this same `post_bulk` handler.
//   * Per-row inline star toggle on the folder view — the bulk action bar
//     covers that case (tick the row, click Star/Unstar). A future polish
//     task may add a per-row form button for one-click toggling, but the
//     star indicator already renders today (TMAIL-371 row template).
//
// Why redirect to the folder view (not the read view) on success
// --------------------------------------------------------------
// "Mark unread" / "Star" from a message you just opened is almost always
// a triage gesture — the user is signalling that the message needs
// follow-up and wants to leave it visibly flagged in the inbox. Dropping
// them back on the folder view confirms the row is now bold / starred,
// which is exactly the signal they're after. If they wanted to keep
// reading, they wouldn't have hit the button.

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
use crate::services::auth_service::Claims;
use crate::services::imap_service::ImapService;
use crate::state::AppState;

/// Hard cap on the number of UIDs accepted by `post_bulk` in a single
/// submission. The folder view page size is 25 (`folder::PAGE_SIZE`), so
/// users selecting "every row on this page" tops out well under this.
/// The cap exists to keep a hostile / scripted client from sending a
/// 10,000-element UID set that would lock the IMAP session for minutes.
pub const MAX_BULK_UIDS: usize = 200;

/// IMAP flag for "read" on every server we target. Centralised constant so
/// a future flag change (or escape-handling tweak) only touches one line.
const SEEN_FLAG: &str = "\\Seen";

/// Added (TMAIL-371): IMAP flag for "starred" (a.k.a. "flagged" in the RFC).
/// Every webmail UI surfaces this as a star icon, even though the IMAP
/// flag is `\Flagged`. Same centralisation rationale as `SEEN_FLAG` —
/// one source of truth so a future server-specific tweak (e.g. Gmail's
/// `$Starred` keyword aliasing) only touches this constant.
const FLAGGED_FLAG: &str = "\\Flagged";

/// Form body for the single-message endpoint.
///
/// `mark` is required and validated against the [`MarkAction`] enum below.
/// An unknown value falls through to `AppError::BadRequest` rather than
/// silently applying neither operation — failing loudly here keeps a
/// renamed button from quietly becoming a no-op.
#[derive(Debug, Deserialize)]
pub struct SingleFlagForm {
    pub mark: String,
}

/// Form body for the bulk endpoint.
///
/// `action` is required; `uid` is harvested out of the raw form pairs by
/// `post_bulk` because `serde_urlencoded` doesn't preserve duplicate keys
/// when deserialising into a typed struct. We parse the body twice — once
/// as `BulkActionForm` to get the action and once as `Vec<(String, String)>`
/// to harvest the repeated `uid` fields. Both passes are cheap (the body is
/// already in memory after the CSRF middleware buffered it).
#[derive(Debug, Deserialize)]
pub struct BulkActionForm {
    pub action: String,
}

/// Resolved mark direction. Centralised so the template, the form parser,
/// and the redirect builder all agree on the same set of valid values.
///
/// Extended (TMAIL-371): the `Star` and `Unstar` variants toggle the IMAP
/// `\Flagged` flag, mirroring the existing Read/Unread pair for `\Seen`.
/// `imap_flag()` returns the right flag string per variant so the handler
/// can stay agnostic of which underlying flag is being toggled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkAction {
    Read,
    Unread,
    Star,
    Unstar,
}

impl MarkAction {
    /// Parse the `mark` form field on the single endpoint. Accepts the
    /// canonical lowercase values; anything else is a 400.
    pub fn from_single(raw: &str) -> Result<Self, AppError> {
        match raw {
            "read" => Ok(Self::Read),
            "unread" => Ok(Self::Unread),
            "star" => Ok(Self::Star),
            "unstar" => Ok(Self::Unstar),
            other => Err(AppError::BadRequest(format!(
                "Unknown mark direction: {other:?} (expected 'read', 'unread', 'star' or 'unstar')"
            ))),
        }
    }

    /// Parse the `action` form field on the bulk endpoint. Returns `None`
    /// for non-mark actions (move / delete) so the bulk handler can hand
    /// off to a future sibling routine without a noisy error path. Returns
    /// `Err` only when the value is recognised-but-malformed.
    ///
    /// Star / Unstar use the bare verbs (`star` / `unstar`) on the bulk
    /// endpoint rather than the `mark_…` prefix the read/unread pair uses,
    /// because "Star" reads better as a button label than "Mark starred"
    /// and the URL needs to round-trip the label directly via the
    /// `<button value="…">` attribute.
    pub fn from_bulk(raw: &str) -> Option<Self> {
        match raw {
            "mark_read" => Some(Self::Read),
            "mark_unread" => Some(Self::Unread),
            "star" => Some(Self::Star),
            "unstar" => Some(Self::Unstar),
            _ => None,
        }
    }

    /// The IMAP flag string this action toggles. `\Seen` for the read/unread
    /// pair, `\Flagged` for the star/unstar pair. Returned by-value as
    /// `&'static str` so the caller can pass it straight to
    /// `ImapService::set_flag` without an extra allocation.
    pub fn imap_flag(self) -> &'static str {
        match self {
            Self::Read | Self::Unread => SEEN_FLAG,
            Self::Star | Self::Unstar => FLAGGED_FLAG,
        }
    }

    /// Whether this action ADDs the flag (true) or strips it (false).
    /// Read + Star both ADD their respective flag; Unread + Unstar both
    /// strip it. The handler passes this directly into `set_flag(... add)`.
    pub fn adds_flag(self) -> bool {
        matches!(self, Self::Read | Self::Star)
    }

    /// The query-param value the folder view's `?marked=…` banner expects.
    /// The starred/unstarred keys are spelled in their UI-readable form
    /// (not "star"/"unstar") because the folder banner copy renders as
    /// "Starred N messages" / "Unstarred N messages", matching the URL key
    /// makes the banner template branches read naturally.
    pub fn as_banner_key(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Unread => "unread",
            Self::Star => "starred",
            Self::Unstar => "unstarred",
        }
    }
}

/// Template for the rare "no uids selected" + "unknown bulk action" cases.
/// Kept as a flat in-line `<p>` rendered into base.html so the user lands
/// somewhere sensible (with the standard nav + logout slot) rather than on
/// a JSON error envelope.
#[derive(Template)]
#[template(path = "classic/flag_error.html")]
pub struct FlagErrorTemplate {
    pub message: String,
    pub back_href: String,
    pub csrf_token: String,
    pub csp_nonce: String,
}

/// Pull the `uid` field out of a form body that already-deserialised
/// `BulkActionForm` once. Each `uid` parses with `u32::from_str`; bogus
/// values (non-numeric, negative, overflowing) are silently dropped rather
/// than 400'ing the whole submission — that way a stale checkbox holding
/// onto a deleted message's UID doesn't sabotage the user's other 24
/// selections.
///
/// The cap on `MAX_BULK_UIDS` triggers a BadRequest so a malicious /
/// runaway client can't push a 10k-element STORE through. De-duplicates
/// the returned vec because IMAP's `UID STORE` semantics permit duplicates
/// but reporting "Marked 27 messages" when 25 were selected is misleading.
pub fn extract_uids(pairs: &[(String, String)]) -> Result<Vec<u32>, AppError> {
    let mut uids: Vec<u32> = pairs
        .iter()
        .filter(|(k, _)| k == "uid")
        .filter_map(|(_, v)| v.parse::<u32>().ok())
        .collect();

    if uids.len() > MAX_BULK_UIDS {
        return Err(AppError::BadRequest(format!(
            "Too many messages selected ({} > {}).",
            uids.len(),
            MAX_BULK_UIDS
        )));
    }

    uids.sort_unstable();
    uids.dedup();
    Ok(uids)
}

/// Build the 303 redirect target for the folder view's marked banner.
/// Same shape for both the single-message and bulk endpoints so the
/// folder view doesn't need separate `?marked=read` vs `?marked-one=read`
/// handling.
fn marked_redirect(folder_href: &str, action: MarkAction, count: usize) -> String {
    format!(
        "/classic/folders/{folder_href}?marked={key}&count={count}",
        key = action.as_banner_key(),
    )
}

/// POST /classic/folders/{folder}/messages/{uid}/flag — toggle `\Seen` on
/// one message.
///
/// CSRF + auth already enforced upstream. Bad `mark` values 400; everything
/// else flows through `set_flag` and 303s back to the folder.
pub async fn post_message_flag(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((folder, uid)): Path<(String, u32)>,
    Form(form): Form<SingleFlagForm>,
) -> Result<Response, AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID in classic claims")))?;

    let action = MarkAction::from_single(&form.mark)?;

    let imap_service = ImapService::for_user(&state, mailbox_id).await?;
    let (username, password) = imap_service
        .user_creds()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("BYOK creds missing on ImapService")))?;
    let username = username.to_string();
    let password = password.to_string();

    imap_service
        .set_flag(
            &username,
            &password,
            &folder,
            uid,
            action.imap_flag(),
            action.adds_flag(),
        )
        .await?;

    let folder_href = urlencoding::encode(&folder).into_owned();
    let target = marked_redirect(&folder_href, action, 1);
    Ok((StatusCode::SEE_OTHER, [(header::LOCATION, target)]).into_response())
}

/// POST /classic/folders/{folder}/bulk — apply a bulk action to a set of
/// UIDs on the folder.
///
/// Today only `action=mark_read` and `action=mark_unread` are wired —
/// every other action value renders the friendly error page and 400s.
/// Future P1 follow-ups (#18 Move, #29 Bulk delete) will land here as
/// new branches on the action match.
///
/// CSRF + auth are enforced upstream by the authenticated sub-router.
pub async fn post_bulk(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Extension(session): Extension<crate::models::classic_session::ClassicSession>,
    Extension(csp_nonce): Extension<super::CspNonce>,
    Path(folder): Path<String>,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Result<Response, AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID in classic claims")))?;

    let folder_href = urlencoding::encode(&folder).into_owned();

    // The `action` field is required. A submission with no `action` (which
    // can happen if a future button is dropped onto the bar without a
    // value attribute) renders the friendly error page rather than 400'ing
    // through the global JSON error envelope.
    let Some(action_raw) = pairs.iter().find(|(k, _)| k == "action").map(|(_, v)| v.clone()) else {
        return render_flag_error(
            "No bulk action was submitted. Pick an action and try again.",
            &folder_href,
            &session.csrf_token,
            csp_nonce.into_string(),
        );
    };

    // Added (TMAIL-372): `action=move` is dispatched out to the move
    // handler so the bulk-action bar's Move… button hits the same /bulk
    // endpoint as Mark read / Mark unread / Star / Unstar. The move handler
    // owns target validation + the IMAP COPY+STORE+EXPUNGE call.
    if action_raw == "move" {
        return super::move_to::handle_bulk_move(
            state,
            claims,
            session,
            csp_nonce,
            folder,
            pairs,
        )
        .await;
    }

    let Some(action) = MarkAction::from_bulk(&action_raw) else {
        // Delete bulk action lives on this same endpoint but ships under a
        // separate task (P1 #29). Today it 400s via the friendly page so
        // the user gets a hint instead of silence.
        return render_flag_error(
            &format!(
                "The bulk action {action_raw:?} isn't wired up yet. Mark read, \
                 Mark unread, Star, Unstar and Move are the only actions \
                 available right now."
            ),
            &folder_href,
            &session.csrf_token,
            csp_nonce.into_string(),
        );
    };

    let uids = extract_uids(&pairs)?;
    if uids.is_empty() {
        return render_flag_error(
            "No messages were selected. Tick the checkbox on at least one row \
             before clicking a bulk action.",
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

    imap_service
        .set_flag_batch(
            &username,
            &password,
            &folder,
            &uids,
            action.imap_flag(),
            action.adds_flag(),
        )
        .await?;

    let target = marked_redirect(&folder_href, action, uids.len());
    Ok((StatusCode::SEE_OTHER, [(header::LOCATION, target)]).into_response())
}

/// Render the friendly 400 page for the bulk endpoint's "no uids" /
/// "unknown action" cases. Kept as a helper so both branches stay tight.
fn render_flag_error(
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
    let html = template
        .render()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("classic flag-error template render failed: {e}")))?;
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

    // ───────────── MarkAction::from_single ─────────────

    #[test]
    fn from_single_accepts_read() {
        assert_eq!(MarkAction::from_single("read").unwrap(), MarkAction::Read);
    }

    #[test]
    fn from_single_accepts_unread() {
        assert_eq!(MarkAction::from_single("unread").unwrap(), MarkAction::Unread);
    }

    // Added (TMAIL-371): Star + Unstar single-endpoint parsing. Same shape
    // as the read/unread pair above so the message-read-view's Star button
    // can POST through the existing `/flag` endpoint with `mark=star` /
    // `mark=unstar` without needing a parallel route.
    #[test]
    fn from_single_accepts_star() {
        assert_eq!(MarkAction::from_single("star").unwrap(), MarkAction::Star);
    }

    #[test]
    fn from_single_accepts_unstar() {
        assert_eq!(MarkAction::from_single("unstar").unwrap(), MarkAction::Unstar);
    }

    #[test]
    fn from_single_rejects_unknown_values() {
        // A renamed button or stale browser cache shouldn't silently turn
        // into a no-op — fail loud with a BadRequest the user can spot.
        // STAR/UNSTAR (uppercase) + the bulk-prefixed `mark_read` shape
        // must also fail here — the single endpoint accepts only the bare
        // lowercase verbs.
        for raw in [
            "", "READ", " read ", "1", "true", "yes", "mark_read", "seen",
            "STAR", "Star", "starred", "mark_star", "flagged",
        ] {
            let err = MarkAction::from_single(raw).expect_err(&format!(
                "from_single({raw:?}) should have failed but returned a value"
            ));
            assert!(
                matches!(err, AppError::BadRequest(_)),
                "expected BadRequest, got {err:?}"
            );
        }
    }

    // ───────────── MarkAction::from_bulk ─────────────

    #[test]
    fn from_bulk_accepts_mark_read_and_mark_unread() {
        assert_eq!(MarkAction::from_bulk("mark_read"), Some(MarkAction::Read));
        assert_eq!(
            MarkAction::from_bulk("mark_unread"),
            Some(MarkAction::Unread)
        );
    }

    // Added (TMAIL-371): Star + Unstar bulk parsing. Uses the bare verb
    // (`star` / `unstar`) rather than the `mark_…` prefix the read/unread
    // pair uses so the button labels round-trip the `value=` attribute
    // directly. `from_bulk` is the only caller that has to know about that
    // inconsistency, which is acceptable.
    #[test]
    fn from_bulk_accepts_star_and_unstar() {
        assert_eq!(MarkAction::from_bulk("star"), Some(MarkAction::Star));
        assert_eq!(MarkAction::from_bulk("unstar"), Some(MarkAction::Unstar));
    }

    #[test]
    fn from_bulk_returns_none_for_other_actions() {
        // The Move and Delete actions ship in follow-up tasks. The bulk
        // handler distinguishes "known-but-unwired" from "known-and-handled"
        // by getting `None` here, then dispatching to the friendly error
        // page rather than 400'ing through the JSON error envelope.
        // STAR/UNSTAR uppercase + the single-endpoint `read`/`unread`
        // shapes must also fall through — the bulk endpoint uses different
        // verbs for the seen/flagged pairs by design.
        for raw in [
            "move", "delete", "archive", "", "MARK_READ", "junk",
            "STAR", "Star", "starred", "mark_star", "read", "unread",
        ] {
            assert_eq!(MarkAction::from_bulk(raw), None, "raw={raw:?}");
        }
    }

    // ───────────── MarkAction helpers ─────────────

    #[test]
    fn adds_flag_is_true_for_read_and_star() {
        // Read + Star both ADD their respective IMAP flag; Unread + Unstar
        // both strip it. Locked-in invariant — flipping this matrix would
        // silently turn every Star into Unstar on production.
        assert!(MarkAction::Read.adds_flag());
        assert!(MarkAction::Star.adds_flag());
        assert!(!MarkAction::Unread.adds_flag());
        assert!(!MarkAction::Unstar.adds_flag());
    }

    // Added (TMAIL-371): Verify each variant maps to the right IMAP flag.
    // A renamed constant or a stray Star→\Seen mapping would silently
    // mark messages read when the user clicks Star — the exact kind of
    // bug a unit test on a closed enum should catch up front.
    #[test]
    fn imap_flag_maps_seen_for_read_pair() {
        assert_eq!(MarkAction::Read.imap_flag(), "\\Seen");
        assert_eq!(MarkAction::Unread.imap_flag(), "\\Seen");
    }

    #[test]
    fn imap_flag_maps_flagged_for_star_pair() {
        assert_eq!(MarkAction::Star.imap_flag(), "\\Flagged");
        assert_eq!(MarkAction::Unstar.imap_flag(), "\\Flagged");
    }

    #[test]
    fn banner_key_round_trips() {
        assert_eq!(MarkAction::Read.as_banner_key(), "read");
        assert_eq!(MarkAction::Unread.as_banner_key(), "unread");
        // Added (TMAIL-371): the star/unstar pair uses the UI-readable
        // form ("starred" / "unstarred") rather than the bare verb so the
        // folder banner copy ("Starred N messages") matches the URL key.
        assert_eq!(MarkAction::Star.as_banner_key(), "starred");
        assert_eq!(MarkAction::Unstar.as_banner_key(), "unstarred");
    }

    // ───────────── extract_uids ─────────────

    fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
        items
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn extract_uids_filters_to_uid_only() {
        let p = pairs(&[
            ("_csrf", "abc"),
            ("action", "mark_read"),
            ("uid", "12"),
            ("uid", "34"),
            ("uid", "56"),
            ("page", "0"),
        ]);
        let uids = extract_uids(&p).expect("happy-path parse");
        assert_eq!(uids, vec![12, 34, 56]);
    }

    #[test]
    fn extract_uids_returns_empty_when_no_uid_fields() {
        let p = pairs(&[("_csrf", "abc"), ("action", "mark_read")]);
        let uids = extract_uids(&p).expect("happy-path parse");
        assert!(uids.is_empty());
    }

    #[test]
    fn extract_uids_drops_non_numeric_values_silently() {
        // A stale checkbox holding onto a non-numeric value shouldn't
        // sabotage the rest of the submission.
        let p = pairs(&[
            ("uid", "12"),
            ("uid", "abc"),
            ("uid", ""),
            ("uid", "-1"),
            ("uid", "99"),
        ]);
        let uids = extract_uids(&p).expect("happy-path parse");
        assert_eq!(uids, vec![12, 99]);
    }

    #[test]
    fn extract_uids_dedupes_and_sorts() {
        // De-dup so the redirect's count matches the unique-row count.
        // Sort so the IMAP STORE wire form is stable across runs (and
        // so the unit test is order-independent).
        let p = pairs(&[
            ("uid", "5"),
            ("uid", "1"),
            ("uid", "5"),
            ("uid", "3"),
            ("uid", "1"),
        ]);
        let uids = extract_uids(&p).expect("happy-path parse");
        assert_eq!(uids, vec![1, 3, 5]);
    }

    #[test]
    fn extract_uids_caps_at_max_bulk_uids() {
        let many: Vec<(String, String)> = (0..=MAX_BULK_UIDS as u32)
            .map(|n| ("uid".to_string(), n.to_string()))
            .collect();
        let err = extract_uids(&many).expect_err("over-cap should reject");
        assert!(
            matches!(err, AppError::BadRequest(_)),
            "expected BadRequest, got {err:?}"
        );
    }

    #[test]
    fn extract_uids_accepts_exactly_max_bulk_uids() {
        // The cap is INCLUSIVE — exactly MAX_BULK_UIDS unique values
        // should still go through.
        let exactly: Vec<(String, String)> = (0..MAX_BULK_UIDS as u32)
            .map(|n| ("uid".to_string(), n.to_string()))
            .collect();
        let uids = extract_uids(&exactly).expect("at-cap should accept");
        assert_eq!(uids.len(), MAX_BULK_UIDS);
    }

    // ───────────── marked_redirect ─────────────

    #[test]
    fn marked_redirect_round_trips_folder_and_action() {
        assert_eq!(
            marked_redirect("INBOX", MarkAction::Read, 3),
            "/classic/folders/INBOX?marked=read&count=3"
        );
        assert_eq!(
            marked_redirect("INBOX", MarkAction::Unread, 1),
            "/classic/folders/INBOX?marked=unread&count=1"
        );
        // Added (TMAIL-371): star/unstar redirects use the same URL shape
        // — the folder view's `marked_banner()` parser dispatches on the
        // value so the same banner template covers all four directions.
        assert_eq!(
            marked_redirect("INBOX", MarkAction::Star, 2),
            "/classic/folders/INBOX?marked=starred&count=2"
        );
        assert_eq!(
            marked_redirect("INBOX", MarkAction::Unstar, 1),
            "/classic/folders/INBOX?marked=unstarred&count=1"
        );
    }

    #[test]
    fn marked_redirect_preserves_url_encoded_folder() {
        // Folder names with brackets / slashes / spaces are already
        // percent-encoded by the caller — the redirect must NOT
        // double-encode them.
        assert_eq!(
            marked_redirect("%5BGmail%5D%2FSent%20Mail", MarkAction::Read, 2),
            "/classic/folders/%5BGmail%5D%2FSent%20Mail?marked=read&count=2"
        );
    }

    // ───────────── FlagErrorTemplate rendering ─────────────

    fn fresh_error_template() -> FlagErrorTemplate {
        FlagErrorTemplate {
            message: "Test error message".to_string(),
            back_href: "/classic/folders/INBOX".to_string(),
            csrf_token: "test-csrf-token".to_string(),
            csp_nonce: "test-nonce-fixed".to_string(),
        }
    }

    #[test]
    fn flag_error_template_extends_base_layout() {
        let body = fresh_error_template().render().expect("renders");
        assert!(body.contains("<!DOCTYPE html>"), "missing HTML5 doctype");
        assert!(body.contains("class=\"skip-link\""), "skip-link missing");
        assert!(body.contains("<main id=\"main\""), "<main> landmark missing");
        assert!(
            body.contains("<style nonce=\"test-nonce-fixed\">"),
            "inline <style> must carry the per-request CSP nonce: {body}"
        );
    }

    #[test]
    fn flag_error_template_renders_message_and_back_link() {
        let body = fresh_error_template().render().expect("renders");
        assert!(
            body.contains("Test error message"),
            "error message must render: {body}"
        );
        assert!(
            body.contains("href=\"/classic/folders/INBOX\""),
            "back-link href must round-trip: {body}"
        );
    }

    #[test]
    fn flag_error_template_renders_logout_form_with_csrf() {
        // Authenticated page → must override the logout_form block.
        let body = fresh_error_template().render().expect("renders");
        assert!(
            body.contains("action=\"/classic/logout\""),
            "logout form must render on flag-error page: {body}"
        );
        assert!(
            body.contains("value=\"test-csrf-token\""),
            "logout form must carry the session csrf_token: {body}"
        );
    }

    #[test]
    fn flag_error_template_has_zero_script_tags() {
        // Classic UI is no-JS — hard rule.
        let body = fresh_error_template().render().expect("renders");
        assert!(
            !body.contains("<script"),
            "flag-error template must contain ZERO <script> tags: {body}"
        );
    }

    #[test]
    fn flag_error_template_escapes_hostile_message() {
        // Defence in depth — the message comes from server-side strings
        // today, but locking auto-escape in keeps a future "include the
        // submitted action value" tweak from XSS'ing the user via a
        // forged form action.
        let mut t = fresh_error_template();
        t.message = "<script>alert('xss')</script>".to_string();
        let body = t.render().expect("renders");
        assert!(!body.contains("<script>alert('xss')</script>"));
        assert!(body.contains("&#60;script&#62;") || body.contains("&lt;script&gt;"));
    }
}
