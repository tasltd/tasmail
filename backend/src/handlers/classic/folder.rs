// Added (TMAIL-362): GET /classic/folders/{folder} — the inbox / folder view
// for the no-JS Classic UI surface.
//
// What this renders
// -----------------
//   * A sidebar `<ul>` of every IMAP folder on the user's BYOK server
//     (read-only nav; per-folder CRUD is the P2 #45 task). The currently
//     viewed folder gets `aria-current="page"` on its link.
//   * A semantic `<table>` of `Sender | Subject | Date` for the page
//     (default page size 25, newest first — matches what `list_messages`
//     already returns). Each row carries:
//       - a checkbox `<input type=checkbox name=uid value=N>` placeholder
//         that the P1 #29 bulk-action bar will hook up; today it's wrapped
//         in a `<form>` that POSTs nowhere (the action bar's stub button
//         renders below the table and submits nothing).
//       - a subject `<a>` link to `/classic/folders/{folder}/messages/{uid}`
//         (read view ships in P0 #9 — clicking before that lands on the
//         catch-all 404, which is expected behaviour).
//     A row counts as **unread** when its IMAP flag set does NOT contain
//     `\\Seen` — rendered as `<strong>` so it pops at a glance even in
//     monochrome / lynx.
//   * Prev / next page links wired off the `?page=` query param. Page 0
//     is the newest; page N+1 is older. A page outside the range still
//     renders (empty rows) so a stale bookmark doesn't 500 — it just shows
//     "no messages on this page".
//
// What this does NOT render (deliberately deferred to follow-up tasks)
// --------------------------------------------------------------------
//   * Bulk action wiring (Mark read / Move to / Delete) — P1 #29.
//   * Compose, search, settings nav highlights — already in the base nav.
//   * Folder CRUD from the sidebar — P2 #45 (sidebar is read-only).
//   * The per-message read view — P0 #9. The subject link points at the
//     future URL today so a click ends on the 404 page until that task
//     lands.
//
// Why both list_folders AND list_messages on the same request
// -----------------------------------------------------------
// The Classic UI is no-JS: every interaction is a fresh server render,
// so a sidebar that lazy-loads via fetch() is impossible. Stuffing both
// IMAP calls into the same handler means one round trip from the browser
// per page view. `list_folders` does a LIST + STATUS per mailbox, which
// on a 30-folder Gmail account costs ~15-30 commands — acceptable on
// modern servers, and the alternative (a separate `/classic/folders` call
// the browser makes via a frame) would either need JS or duplicate the
// auth handshake. The follow-up P2 caching task may add a session-scoped
// LRU; until then the simplicity wins.
//
// Lockout-aware? No — by this point the request already cleared
// `classic_session_middleware`, which only lets through *valid* sessions
// for *active* mailboxes. There's no enumeration / lockout signal to
// surface on a paged inbox.

use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::AppError;
use crate::services::auth_service::Claims;
use crate::services::imap_service::{Folder, ImapService, MessageEnvelope};
use crate::state::AppState;

use super::CspNonce;

/// Page size for the Classic UI inbox table. 25 is the gap-analysis
/// recommendation (P0 #8) — enough to be useful, small enough to keep
/// the HTML response under ~30 KB even on a folder with long subjects,
/// and small enough that prev / next links are usually one click of work.
const PAGE_SIZE: u32 = 25;

/// Built-in folders the sidebar pins to the top in a fixed order. Any
/// folder NOT in this list lands in the "Folders" section below in the
/// order the IMAP server returned it (preserves server-side hierarchy).
/// Case-insensitive match.
const PINNED_FOLDERS: &[&str] = &[
    "INBOX",
    "Drafts",
    "Sent",
    "Sent Items",
    "Trash",
    "Deleted Items",
    "Junk",
    "Spam",
    "Archive",
];

/// Marker IMAP flag for read messages — anything in `MessageEnvelope::flags`
/// that ends with `Seen` (the async-imap Flag enum's `Debug` shape is
/// `Seen` for the system flag) counts as read. Matched case-insensitively
/// so a server returning lower-case `seen` still resolves correctly.
fn message_is_read(env: &MessageEnvelope) -> bool {
    env.flags
        .iter()
        .any(|f| f.eq_ignore_ascii_case("Seen") || f.eq_ignore_ascii_case("\\Seen"))
}

/// Query string for `GET /classic/folders/{folder}?page=N`.
///
/// `page` is **0-based** — page 0 is the newest 25 messages. Anything
/// negative / non-numeric falls back to 0 silently so a malformed bookmark
/// can't 400 the user out of their inbox. Pages beyond `total / PAGE_SIZE`
/// still render (with empty rows) so stale links don't break.
///
/// Added (TMAIL-366): `sent=1` is appended by the compose POST handler on
/// a successful send and surfaces as a green "Message sent" banner on
/// this view. Any other value (or absence) means no banner.
///
/// Added (TMAIL-367): `deleted=1` is appended by the delete POST handler
/// after a successful move-to-Trash OR permanent expunge from Trash, and
/// surfaces as a green "Message deleted" banner. Same truthy-value
/// matching as `sent` so a future `?deleted=true` link still works.
#[derive(Debug, Deserialize, Default)]
pub struct FolderQuery {
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default)]
    pub sent: Option<String>,
    #[serde(default)]
    pub deleted: Option<String>,
}

impl FolderQuery {
    fn page(&self) -> u32 {
        self.page.unwrap_or(0)
    }

    /// True when `?sent=1` (or any other truthy value the compose handler
    /// might emit) is present. Used to flip the success banner on.
    fn sent_banner(&self) -> bool {
        match self.sent.as_deref() {
            Some(v) => matches!(v, "1" | "true" | "yes"),
            None => false,
        }
    }

    /// Added (TMAIL-367): True when `?deleted=1` is present, set by the
    /// delete POST handler on a successful move-to-Trash OR permanent
    /// expunge from Trash. Drives the one-time confirmation banner.
    fn deleted_banner(&self) -> bool {
        match self.deleted.as_deref() {
            Some(v) => matches!(v, "1" | "true" | "yes"),
            None => false,
        }
    }
}

/// A single sidebar entry. Distinguishes "currently viewed" from the rest
/// so the template can attach `aria-current="page"` and the styled
/// indicator without leaking ARIA logic into the template.
pub struct SidebarFolder {
    /// Display name (the folder's raw IMAP name — same string we round-trip
    /// through the URL).
    pub name: String,
    /// URL-encoded form of `name` so a folder like `[Gmail]/Sent Mail`
    /// survives the trip through the link href without breaking routing.
    pub href_segment: String,
    /// True when this folder is the one currently being rendered.
    pub is_current: bool,
    /// IMAP-reported unread count. `None` when the server didn't return a
    /// STATUS UNSEEN value (some self-hosted servers omit it for special
    /// mailboxes); rendered as an empty cell rather than `0` to avoid
    /// misleading users.
    pub unseen: Option<u32>,
}

/// A single row in the message table — flattened from `MessageEnvelope`
/// so the template doesn't have to reason about `Option<String>` chains
/// or flag parsing.
pub struct MessageRow {
    pub uid: u32,
    pub from: String,
    pub subject: String,
    pub date: String,
    pub is_read: bool,
    /// Pre-built `href` for the row link — the per-folder + per-uid path
    /// the read view (P0 #9) will own.
    pub message_href: String,
}

/// Askama template struct backing `templates/classic/folder.html`.
///
/// Field names match the template `{{ var }}` placeholders exactly —
/// Askama validates this at compile time so a rename here without the
/// matching template edit fails `cargo build`.
#[derive(Template)]
#[template(path = "classic/folder.html")]
pub struct FolderTemplate {
    /// Display name of the currently viewed folder (e.g. `INBOX`).
    pub current_folder: String,
    /// Pre-encoded path segment for the current folder, used by the
    /// pagination links so they round-trip the folder name correctly.
    pub current_folder_href: String,
    /// Total messages in the current folder per IMAP `EXISTS`.
    pub total_messages: u32,
    /// 0-based page number being rendered. `0` = newest page.
    pub current_page: u32,
    /// Total number of pages (= `ceil(total / PAGE_SIZE)`, minimum 1
    /// so the "page 1 of 1" footer still renders on an empty folder).
    pub total_pages: u32,
    /// 1-based index of the first message displayed on this page (`0` when
    /// the page is empty). Used for the "Showing N–M of T" footer.
    pub first_index: u32,
    /// 1-based index of the last message displayed on this page (`0` when
    /// the page is empty).
    pub last_index: u32,
    /// True when `current_page > 0` — drives the prev link visibility.
    pub has_prev: bool,
    /// True when `current_page + 1 < total_pages` — drives the next link.
    pub has_next: bool,
    /// `?page=N` query string for the prev link (empty when no prev).
    pub prev_href: String,
    /// `?page=N` query string for the next link (empty when no next).
    pub next_href: String,
    /// Sidebar entries, pinned built-ins first, then the rest in server order.
    pub sidebar: Vec<SidebarFolder>,
    /// Pre-flattened message rows for the current page.
    pub messages: Vec<MessageRow>,
    /// Session CSRF token, threaded through the logout form partial and
    /// the bulk-action stub form. Both forms POST to handlers that the
    /// canonical CSRF middleware validates.
    pub csrf_token: String,
    /// Added (TMAIL-366): true when the user landed here via the compose
    /// POST-Redirect-Get with `?sent=1`. Drives a one-time green banner
    /// at the top of the message list so the user gets confirmation that
    /// the message they just composed was actually accepted by SMTP.
    pub sent_banner: bool,
    /// Added (TMAIL-367): true when the user landed here via the delete
    /// POST-Redirect-Get with `?deleted=1`. Drives a one-time green
    /// banner at the top of the message list so the user gets confirmation
    /// that their delete action actually completed (either moved to Trash
    /// or permanently expunged when already in Trash).
    pub deleted_banner: bool,
    /// Per-request CSP nonce. Required by base.html (TMAIL-356).
    pub csp_nonce: String,
}

/// URL-encode an IMAP folder name for use inside a path segment. Names like
/// `[Gmail]/Sent Mail` contain `/`, `[`, `]`, and spaces — every one of
/// those breaks routing unless percent-escaped. `urlencoding::encode`
/// uses RFC 3986 unreserved chars only, which is the conservative encode
/// set browsers tolerate everywhere — spaces become `%20`, not `+`, so
/// path-segment use is safe (the form-style `+` encoding would otherwise
/// confuse the IMAP folder back on the next request).
fn encode_folder_segment(name: &str) -> String {
    urlencoding::encode(name).into_owned()
}

/// Compute `(total_pages, first_index, last_index)` from the response
/// counters. Pulled into its own function so the unit tests can exercise
/// the pagination maths without going through Askama / IMAP.
///
/// All indices returned are **1-based**, suitable for direct rendering
/// into the "Showing N–M of T" footer.
fn pagination_window(total: u32, page: u32, rows_on_page: u32) -> (u32, u32, u32) {
    let total_pages = if total == 0 {
        1
    } else {
        // ceil_div without pulling in num_integer
        total.div_ceil(PAGE_SIZE)
    };
    if rows_on_page == 0 {
        return (total_pages, 0, 0);
    }
    // Newest-first: page 0 row 0 → message `total`, page 0 row 24 →
    // message `total - 24` (i.e. message # 1 if total <= 25). The "first
    // index" displayed at the top of the table is therefore the NEWEST
    // message on this page = `total - page * PAGE_SIZE`.
    let first = total.saturating_sub(page * PAGE_SIZE);
    let last = first.saturating_sub(rows_on_page - 1).max(1);
    (total_pages, first, last)
}

/// Build the sidebar entry list with the pinned built-ins ordered first
/// (in the order they appear in `PINNED_FOLDERS`) and the rest in server
/// order. Used by the GET handler and exercised directly by tests.
fn build_sidebar(folders: Vec<Folder>, current_folder: &str) -> Vec<SidebarFolder> {
    let mut pinned: Vec<SidebarFolder> = Vec::new();
    let mut rest: Vec<SidebarFolder> = Vec::new();

    // Build a lookup of original folders by case-insensitive name so the
    // PINNED_FOLDERS list can pull them out in fixed order. We keep the
    // ORIGINAL spelling (whatever the server returned) on the entry so a
    // server that uses "Inbox" instead of "INBOX" displays as it appears
    // on the wire.
    for folder in folders {
        let is_pinned = PINNED_FOLDERS
            .iter()
            .any(|p| p.eq_ignore_ascii_case(&folder.name));
        let entry = SidebarFolder {
            is_current: folder.name.eq_ignore_ascii_case(current_folder),
            href_segment: encode_folder_segment(&folder.name),
            unseen: folder.unseen,
            name: folder.name,
        };
        if is_pinned {
            pinned.push(entry);
        } else {
            rest.push(entry);
        }
    }

    // Sort the pinned bucket by the index of the matching PINNED_FOLDERS
    // entry so INBOX always renders first, Drafts second, etc.
    pinned.sort_by_key(|f| {
        PINNED_FOLDERS
            .iter()
            .position(|p| p.eq_ignore_ascii_case(&f.name))
            .unwrap_or(usize::MAX)
    });

    pinned.extend(rest);
    pinned
}

/// Convert a `MessageEnvelope` from the IMAP layer into a `MessageRow`
/// shaped for the template. Centralising the `Option<String>` → empty
/// string normalisation here keeps the template free of `if let Some`
/// chains and ensures every cell renders *something* even on partial
/// fetches.
fn envelope_to_row(env: MessageEnvelope, folder_href_segment: &str) -> MessageRow {
    MessageRow {
        message_href: format!(
            "/classic/folders/{}/messages/{}",
            folder_href_segment, env.uid
        ),
        is_read: message_is_read(&env),
        uid: env.uid,
        from: env.from.unwrap_or_default(),
        subject: env
            .subject
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "(no subject)".to_string()),
        date: env.date.unwrap_or_default(),
    }
}

/// GET /classic/folders/{folder}?page=N — render the folder view.
///
/// `folder` is URL-decoded by axum's `Path` extractor. We pass it
/// verbatim to IMAP — the IMAP server validates whether the name exists
/// and a missing folder bubbles up as `AppError::Imap` (rendered as 500
/// by the global error layer; the catch-all 404 doesn't apply because
/// the route DID match). A future polish task can map IMAP "no such
/// mailbox" errors to a friendlier 404 page.
pub async fn get_folder(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Extension(session): Extension<crate::models::classic_session::ClassicSession>,
    Path(folder): Path<String>,
    Query(query): Query<FolderQuery>,
) -> Result<Response, AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID in classic claims")))?;

    let page = query.page();

    // 1) Build the BYOK IMAP service from the user's saved imap_configurations
    //    row. Decrypts the password with the JWT-derived AES key.
    let imap_service = ImapService::for_user(&state, mailbox_id).await?;
    let (username, password) = imap_service
        .user_creds()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("BYOK creds missing on ImapService")))?;
    let username = username.to_string();
    let password = password.to_string();

    // 2) Fetch folders + messages back-to-back. Each call opens its own
    //    IMAP session for now — see the module-level comment for why this
    //    isn't a performance problem at Classic UI scale.
    let folders_list = imap_service
        .list_folders(&username, &password)
        .await
        .unwrap_or_else(|e| {
            // A failed sidebar shouldn't blow up the inbox view —
            // log and render an empty sidebar so the table at least
            // renders. The page is still useful with just the current
            // folder's messages.
            tracing::warn!(error = ?e, "list_folders failed during classic inbox render");
            Vec::new()
        });
    let (envelopes, total) = imap_service
        .list_messages(&username, &password, &folder, page, PAGE_SIZE)
        .await?;

    // 3) Build template fields.
    let folder_href = encode_folder_segment(&folder);
    let sidebar = build_sidebar(folders_list, &folder);
    let rows_on_page = envelopes.len() as u32;
    let (total_pages, first_index, last_index) = pagination_window(total, page, rows_on_page);
    let messages: Vec<MessageRow> = envelopes
        .into_iter()
        .map(|e| envelope_to_row(e, &folder_href))
        .collect();

    let has_prev = page > 0;
    let has_next = total_pages > 0 && page + 1 < total_pages;
    let prev_href = if has_prev {
        format!("/classic/folders/{}?page={}", folder_href, page - 1)
    } else {
        String::new()
    };
    let next_href = if has_next {
        format!("/classic/folders/{}?page={}", folder_href, page + 1)
    } else {
        String::new()
    };

    let template = FolderTemplate {
        current_folder: folder,
        current_folder_href: folder_href,
        total_messages: total,
        current_page: page,
        total_pages,
        first_index,
        last_index,
        has_prev,
        has_next,
        prev_href,
        next_href,
        sidebar,
        messages,
        csrf_token: session.csrf_token.clone(),
        sent_banner: query.sent_banner(),
        deleted_banner: query.deleted_banner(),
        csp_nonce: CspNonce::new().into_string(),
    };

    let body = template.render().map_err(|e| {
        AppError::Internal(anyhow::anyhow!("classic folder template render failed: {e}"))
    })?;

    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
        body,
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(uid: u32, subject: Option<&str>, from: Option<&str>, flags: &[&str]) -> MessageEnvelope {
        MessageEnvelope {
            uid,
            subject: subject.map(String::from),
            from: from.map(String::from),
            date: Some("Mon, 1 Jan 2026 09:00:00 +0000".to_string()),
            flags: flags.iter().map(|s| s.to_string()).collect(),
            size: Some(1024),
            preview: None,
            message_id: None,
            in_reply_to: None,
            references: vec![],
        }
    }

    fn folder(name: &str, unseen: Option<u32>) -> Folder {
        Folder {
            name: name.to_string(),
            delimiter: "/".to_string(),
            messages: Some(0),
            unseen,
        }
    }

    fn fresh_template() -> FolderTemplate {
        FolderTemplate {
            current_folder: "INBOX".to_string(),
            current_folder_href: "INBOX".to_string(),
            total_messages: 0,
            current_page: 0,
            total_pages: 1,
            first_index: 0,
            last_index: 0,
            has_prev: false,
            has_next: false,
            prev_href: String::new(),
            next_href: String::new(),
            sidebar: vec![],
            messages: vec![],
            csrf_token: "test-csrf-token".to_string(),
            sent_banner: false,
            deleted_banner: false,
            csp_nonce: "test-nonce-fixed".to_string(),
        }
    }

    // ----- pagination_window -----

    #[test]
    fn pagination_window_empty_folder_renders_one_page() {
        // Empty folder must still show "page 1 of 1" so the footer
        // doesn't render "page 1 of 0".
        let (pages, first, last) = pagination_window(0, 0, 0);
        assert_eq!((pages, first, last), (1, 0, 0));
    }

    #[test]
    fn pagination_window_exact_page_size_is_one_page() {
        let (pages, first, last) = pagination_window(25, 0, 25);
        assert_eq!(pages, 1);
        assert_eq!(first, 25);
        assert_eq!(last, 1);
    }

    #[test]
    fn pagination_window_one_over_page_size_is_two_pages() {
        // page 0 holds messages 26..2; page 1 holds message 1.
        let (pages, first, last) = pagination_window(26, 0, 25);
        assert_eq!(pages, 2);
        assert_eq!(first, 26);
        assert_eq!(last, 2);

        let (_, first, last) = pagination_window(26, 1, 1);
        assert_eq!(first, 1);
        assert_eq!(last, 1);
    }

    #[test]
    fn pagination_window_beyond_last_page_returns_zero_first_last() {
        // Stale bookmark for ?page=99 on a 5-message folder: 0 rows,
        // 1 total page, indices zeroed so the footer renders sensibly.
        let (pages, first, last) = pagination_window(5, 99, 0);
        assert_eq!(pages, 1);
        assert_eq!((first, last), (0, 0));
    }

    #[test]
    fn pagination_window_large_folder_partial_last_page() {
        // 60 messages, page 0 has 25 (60..36), page 1 has 25 (35..11),
        // page 2 has 10 (10..1).
        let (pages, _, _) = pagination_window(60, 0, 25);
        assert_eq!(pages, 3);
        let (_, first, last) = pagination_window(60, 2, 10);
        assert_eq!(first, 10);
        assert_eq!(last, 1);
    }

    // ----- message_is_read -----

    #[test]
    fn message_is_read_recognises_seen_flag_variants() {
        assert!(message_is_read(&env(1, None, None, &["Seen"])));
        assert!(message_is_read(&env(1, None, None, &["\\Seen"])));
        assert!(message_is_read(&env(1, None, None, &["seen"])));
        assert!(message_is_read(&env(1, None, None, &["Flagged", "Seen"])));
    }

    #[test]
    fn message_is_read_returns_false_when_seen_absent() {
        assert!(!message_is_read(&env(1, None, None, &[])));
        assert!(!message_is_read(&env(1, None, None, &["Flagged"])));
        assert!(!message_is_read(&env(1, None, None, &["Recent"])));
    }

    // ----- envelope_to_row -----

    #[test]
    fn envelope_to_row_substitutes_no_subject_for_empty() {
        let row = envelope_to_row(env(42, Some(""), Some("from@x"), &["Seen"]), "INBOX");
        assert_eq!(row.subject, "(no subject)");
        assert_eq!(row.uid, 42);
        assert_eq!(row.from, "from@x");
        assert!(row.is_read);
        assert_eq!(row.message_href, "/classic/folders/INBOX/messages/42");
    }

    #[test]
    fn envelope_to_row_substitutes_no_subject_for_whitespace() {
        let row = envelope_to_row(env(42, Some("   "), None, &[]), "INBOX");
        assert_eq!(row.subject, "(no subject)");
    }

    #[test]
    fn envelope_to_row_preserves_subject_when_present() {
        let row = envelope_to_row(env(42, Some("Hello"), None, &[]), "INBOX");
        assert_eq!(row.subject, "Hello");
        assert!(!row.is_read);
    }

    #[test]
    fn envelope_to_row_uses_encoded_folder_in_href() {
        let row = envelope_to_row(env(7, Some("S"), None, &[]), "%5BGmail%5D%2FSent%20Mail");
        assert_eq!(
            row.message_href,
            "/classic/folders/%5BGmail%5D%2FSent%20Mail/messages/7"
        );
    }

    // ----- encode_folder_segment -----

    #[test]
    fn encode_folder_segment_escapes_brackets_slash_space() {
        // The four characters that break path routing in IMAP folder names.
        assert_eq!(encode_folder_segment("[Gmail]/Sent Mail"), "%5BGmail%5D%2FSent%20Mail");
        assert_eq!(encode_folder_segment("INBOX"), "INBOX");
        assert_eq!(encode_folder_segment("My Folder"), "My%20Folder");
    }

    // ----- build_sidebar -----

    #[test]
    fn build_sidebar_pins_built_ins_first_in_fixed_order() {
        let folders = vec![
            folder("Custom", None),
            folder("Sent", Some(0)),
            folder("INBOX", Some(3)),
            folder("Trash", Some(0)),
            folder("Drafts", Some(1)),
        ];
        let sidebar = build_sidebar(folders, "INBOX");
        let names: Vec<&str> = sidebar.iter().map(|f| f.name.as_str()).collect();
        // INBOX, Drafts, Sent, Trash come from PINNED_FOLDERS order;
        // Custom is the only non-pinned, so it lands last.
        assert_eq!(names, vec!["INBOX", "Drafts", "Sent", "Trash", "Custom"]);
    }

    #[test]
    fn build_sidebar_marks_current_folder_case_insensitively() {
        let folders = vec![folder("INBOX", None), folder("Sent", None)];
        let sidebar = build_sidebar(folders, "inbox");
        let inbox = sidebar.iter().find(|f| f.name == "INBOX").unwrap();
        let sent = sidebar.iter().find(|f| f.name == "Sent").unwrap();
        assert!(inbox.is_current);
        assert!(!sent.is_current);
    }

    #[test]
    fn build_sidebar_preserves_server_order_for_unpinned() {
        // Hierarchical / custom folders should appear in server order
        // (matching what list_folders returned) so the user sees their
        // existing layout.
        let folders = vec![
            folder("[Gmail]/All Mail", None),
            folder("INBOX", None),
            folder("Personal", None),
            folder("Work", None),
        ];
        let sidebar = build_sidebar(folders, "INBOX");
        let non_pinned: Vec<&str> = sidebar
            .iter()
            .filter(|f| !PINNED_FOLDERS.iter().any(|p| p.eq_ignore_ascii_case(&f.name)))
            .map(|f| f.name.as_str())
            .collect();
        assert_eq!(non_pinned, vec!["[Gmail]/All Mail", "Personal", "Work"]);
    }

    #[test]
    fn build_sidebar_round_trips_href_segment() {
        let folders = vec![folder("[Gmail]/Sent Mail", Some(0))];
        let sidebar = build_sidebar(folders, "INBOX");
        assert_eq!(sidebar[0].href_segment, "%5BGmail%5D%2FSent%20Mail");
    }

    // ----- FolderQuery -----

    #[test]
    fn folder_query_defaults_to_page_zero() {
        let q = FolderQuery { page: None, sent: None, deleted: None };
        assert_eq!(q.page(), 0);
        let q = FolderQuery::default();
        assert_eq!(q.page(), 0);
    }

    #[test]
    fn folder_query_passes_explicit_page() {
        let q = FolderQuery { page: Some(3), sent: None, deleted: None };
        assert_eq!(q.page(), 3);
    }

    // ----- sent_banner (TMAIL-366) -----

    #[test]
    fn folder_query_sent_banner_off_by_default() {
        let q = FolderQuery::default();
        assert!(!q.sent_banner());
    }

    #[test]
    fn folder_query_sent_banner_on_for_truthy_values() {
        for val in &["1", "true", "yes"] {
            let q = FolderQuery {
                page: None,
                sent: Some((*val).to_string()),
                deleted: None,
            };
            assert!(q.sent_banner(), "?sent={val} should turn the banner on");
        }
    }

    #[test]
    fn folder_query_sent_banner_off_for_other_values() {
        // A malformed bookmark with `?sent=banana` shouldn't flash a green
        // success banner — only canonical truthy values flip the flag.
        for val in &["0", "false", "no", "banana", ""] {
            let q = FolderQuery {
                page: None,
                sent: Some((*val).to_string()),
                deleted: None,
            };
            assert!(!q.sent_banner(), "?sent={val} should NOT turn the banner on");
        }
    }

    // ----- deleted_banner (TMAIL-367) -----

    #[test]
    fn folder_query_deleted_banner_off_by_default() {
        let q = FolderQuery::default();
        assert!(!q.deleted_banner());
    }

    #[test]
    fn folder_query_deleted_banner_on_for_truthy_values() {
        for val in &["1", "true", "yes"] {
            let q = FolderQuery {
                page: None,
                sent: None,
                deleted: Some((*val).to_string()),
            };
            assert!(q.deleted_banner(), "?deleted={val} should turn the banner on");
        }
    }

    #[test]
    fn folder_query_deleted_banner_off_for_other_values() {
        // A malformed bookmark with `?deleted=oops` shouldn't flash a
        // green success banner — only canonical truthy values flip it.
        for val in &["0", "false", "no", "oops", ""] {
            let q = FolderQuery {
                page: None,
                sent: None,
                deleted: Some((*val).to_string()),
            };
            assert!(!q.deleted_banner(), "?deleted={val} should NOT turn the banner on");
        }
    }

    // ----- Template rendering -----

    #[test]
    fn folder_template_renders_empty_inbox_with_helpful_message() {
        let body = fresh_template().render().expect("template renders");
        assert!(body.contains("<table"), "must render a table");
        assert!(body.contains("Sender") && body.contains("Subject") && body.contains("Date"));
        assert!(
            body.contains("No messages in this folder."),
            "empty state copy missing: {body}"
        );
    }

    // ----- TMAIL-366: success banner -----

    #[test]
    fn folder_template_renders_sent_success_banner_when_flag_set() {
        let mut t = fresh_template();
        t.sent_banner = true;
        let body = t.render().expect("template renders");
        assert!(
            body.contains("alert-success"),
            "success alert class missing when sent_banner = true: {body}"
        );
        assert!(
            body.contains("Message sent"),
            "success banner copy missing: {body}"
        );
    }

    #[test]
    fn folder_template_omits_sent_success_banner_by_default() {
        // fresh_template() has sent_banner = false. The banner MUST NOT
        // render on a fresh inbox load — a green "Message sent" on a
        // regular folder open would be misleading.
        // NOTE: the base.html stylesheet defines `.alert-success` CSS
        // rules so the bare class name appears in the rendered CSS even
        // when no banner is emitted. Assert against the actual element.
        let body = fresh_template().render().expect("template renders");
        assert!(
            !body.contains("class=\"alert alert-success\""),
            "success alert element must NOT render when sent_banner = false: {body}"
        );
        assert!(
            !body.contains("Message sent"),
            "success copy must NOT render when sent_banner = false: {body}"
        );
    }

    // ----- TMAIL-367: deleted banner -----

    #[test]
    fn folder_template_renders_deleted_success_banner_when_flag_set() {
        let mut t = fresh_template();
        t.deleted_banner = true;
        let body = t.render().expect("template renders");
        assert!(
            body.contains("alert-success"),
            "success alert class missing when deleted_banner = true: {body}"
        );
        assert!(
            body.contains("Message deleted"),
            "deleted banner copy missing: {body}"
        );
    }

    #[test]
    fn folder_template_omits_deleted_success_banner_by_default() {
        // fresh_template() has deleted_banner = false. A green "Message
        // deleted" on a regular folder open would be misleading.
        let body = fresh_template().render().expect("template renders");
        assert!(
            !body.contains("Message deleted"),
            "deleted copy must NOT render when deleted_banner = false: {body}"
        );
    }

    #[test]
    fn folder_template_renders_one_row_per_message() {
        let mut t = fresh_template();
        t.total_messages = 2;
        t.last_index = 1;
        t.first_index = 2;
        t.messages = vec![
            MessageRow {
                uid: 12,
                from: "alice@example.com".to_string(),
                subject: "Hi from Alice".to_string(),
                date: "Mon, 1 Jan 2026".to_string(),
                is_read: false,
                message_href: "/classic/folders/INBOX/messages/12".to_string(),
            },
            MessageRow {
                uid: 13,
                from: "bob@example.com".to_string(),
                subject: "Re: Hi from Alice".to_string(),
                date: "Mon, 1 Jan 2026".to_string(),
                is_read: true,
                message_href: "/classic/folders/INBOX/messages/13".to_string(),
            },
        ];
        let body = t.render().expect("template renders");
        // Two rows
        let row_count = body.matches("<tr class=\"msg-row").count();
        assert_eq!(row_count, 2, "expected 2 message rows: {body}");
        // Subject links wire to the per-uid path
        assert!(body.contains("href=\"/classic/folders/INBOX/messages/12\""));
        assert!(body.contains("href=\"/classic/folders/INBOX/messages/13\""));
        // Checkbox placeholders carry the uid
        assert!(body.contains("name=\"uid\" value=\"12\""));
        assert!(body.contains("name=\"uid\" value=\"13\""));
        // Sender + subject visible
        assert!(body.contains("alice@example.com"));
        assert!(body.contains("Hi from Alice"));
    }

    #[test]
    fn folder_template_marks_unread_rows_visually_distinct() {
        let mut t = fresh_template();
        t.total_messages = 1;
        t.last_index = 1;
        t.first_index = 1;
        t.messages = vec![MessageRow {
            uid: 1,
            from: "x@y".to_string(),
            subject: "unread one".to_string(),
            date: "today".to_string(),
            is_read: false,
            message_href: "/classic/folders/INBOX/messages/1".to_string(),
        }];
        let body = t.render().expect("template renders");
        assert!(
            body.contains("msg-row msg-row-unread"),
            "unread row must carry distinguishing class: {body}"
        );
        assert!(
            body.contains("<strong>unread one</strong>"),
            "unread subject must be wrapped in <strong> for monochrome readers: {body}"
        );
    }

    #[test]
    fn folder_template_does_not_emphasise_read_rows() {
        let mut t = fresh_template();
        t.total_messages = 1;
        t.last_index = 1;
        t.first_index = 1;
        t.messages = vec![MessageRow {
            uid: 1,
            from: "x@y".to_string(),
            subject: "read one".to_string(),
            date: "today".to_string(),
            is_read: true,
            message_href: "/classic/folders/INBOX/messages/1".to_string(),
        }];
        let body = t.render().expect("template renders");
        // The CSS rule for `.msg-row-unread` lives in the inline <style>
        // block, so we can't just `contains("msg-row-unread")` here —
        // assert on the actual <tr class=...> attribute shape instead.
        assert!(
            !body.contains("class=\"msg-row msg-row-unread\""),
            "read row must NOT carry msg-row-unread class on its <tr>: {body}"
        );
        assert!(
            body.contains("class=\"msg-row\""),
            "read row must carry the plain msg-row class on its <tr>: {body}"
        );
        assert!(!body.contains("<strong>read one</strong>"));
    }

    #[test]
    fn folder_template_html_escapes_hostile_subject_and_sender() {
        // Defence in depth — Askama auto-escapes for .html, but lock the
        // behaviour in with a test so a config drift can't silently turn
        // it off and let a phisher's <script> escape onto the page.
        let mut t = fresh_template();
        t.total_messages = 1;
        t.last_index = 1;
        t.first_index = 1;
        t.messages = vec![MessageRow {
            uid: 1,
            from: "\"><script>alert('from')</script>".to_string(),
            subject: "<script>alert('subj')</script>".to_string(),
            date: "today".to_string(),
            is_read: true,
            message_href: "/classic/folders/INBOX/messages/1".to_string(),
        }];
        let body = t.render().expect("template renders");
        assert!(!body.contains("<script>alert('from')</script>"));
        assert!(!body.contains("<script>alert('subj')</script>"));
    }

    #[test]
    fn folder_template_renders_sidebar_with_current_aria() {
        let mut t = fresh_template();
        t.sidebar = vec![
            SidebarFolder {
                name: "INBOX".to_string(),
                href_segment: "INBOX".to_string(),
                is_current: true,
                unseen: Some(3),
            },
            SidebarFolder {
                name: "Drafts".to_string(),
                href_segment: "Drafts".to_string(),
                is_current: false,
                unseen: Some(0),
            },
        ];
        let body = t.render().expect("template renders");
        assert!(
            body.contains("aria-current=\"page\""),
            "current folder must carry aria-current=\"page\": {body}"
        );
        assert!(body.contains("href=\"/classic/folders/INBOX\""));
        assert!(body.contains("href=\"/classic/folders/Drafts\""));
        // Unread count rendered for INBOX (3) but not Drafts (0).
        assert!(body.contains(">3<"), "INBOX unread count missing");
    }

    #[test]
    fn folder_template_omits_zero_unread_badge() {
        let mut t = fresh_template();
        t.sidebar = vec![SidebarFolder {
            name: "Drafts".to_string(),
            href_segment: "Drafts".to_string(),
            is_current: false,
            unseen: Some(0),
        }];
        let body = t.render().expect("template renders");
        // Rendering `0` as a badge is misleading — the cell stays empty.
        assert!(
            !body.contains("class=\"folder-unread\">0<"),
            "zero-unread badge leaked into sidebar: {body}"
        );
    }

    #[test]
    fn folder_template_renders_prev_link_only_when_has_prev() {
        let mut t = fresh_template();
        t.total_messages = 60;
        t.current_page = 0;
        t.total_pages = 3;
        t.has_prev = false;
        t.has_next = true;
        t.next_href = "/classic/folders/INBOX?page=1".to_string();
        let body = t.render().expect("template renders");
        // No prev link visible on page 0
        assert!(!body.contains("rel=\"prev\""));
        assert!(body.contains("rel=\"next\""));
        assert!(body.contains("href=\"/classic/folders/INBOX?page=1\""));
    }

    #[test]
    fn folder_template_renders_next_link_only_when_has_next() {
        let mut t = fresh_template();
        t.total_messages = 60;
        t.current_page = 2;
        t.total_pages = 3;
        t.has_prev = true;
        t.has_next = false;
        t.prev_href = "/classic/folders/INBOX?page=1".to_string();
        let body = t.render().expect("template renders");
        assert!(body.contains("rel=\"prev\""));
        assert!(!body.contains("rel=\"next\""));
        assert!(body.contains("href=\"/classic/folders/INBOX?page=1\""));
    }

    #[test]
    fn folder_template_renders_pagination_footer() {
        let mut t = fresh_template();
        t.total_messages = 60;
        t.current_page = 0;
        t.total_pages = 3;
        t.first_index = 60;
        t.last_index = 36;
        let body = t.render().expect("template renders");
        // "Showing N–M of T" footer present
        assert!(body.contains("Showing"));
        assert!(body.contains("60"));
        assert!(body.contains("36"));
        assert!(body.contains("Page 1 of 3"));
    }

    #[test]
    fn folder_template_renders_logout_form_with_csrf_token() {
        // Folder view is an authenticated page → MUST override the
        // logout_form block from base.html. The token threaded through
        // matches what the session middleware injected.
        let body = fresh_template().render().expect("template renders");
        assert!(
            body.contains("action=\"/classic/logout\""),
            "logout form must render on authenticated folder view: {body}"
        );
        assert!(
            body.contains("value=\"test-csrf-token\""),
            "logout form must carry the session csrf_token: {body}"
        );
    }

    #[test]
    fn folder_template_extends_base_layout() {
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
    fn folder_template_has_zero_script_tags() {
        // Hard rule per the gap analysis: Classic UI is no-JS.
        let body = fresh_template().render().expect("template renders");
        assert!(
            !body.contains("<script"),
            "folder template must contain ZERO <script> tags: {body}"
        );
    }

    #[test]
    fn folder_template_renders_bulk_action_checkbox_placeholders() {
        // The action bar is a stub today (P1 #29 will wire it), but the
        // checkboxes MUST render so we don't have to retrofit them later.
        let mut t = fresh_template();
        t.total_messages = 1;
        t.last_index = 1;
        t.first_index = 1;
        t.messages = vec![MessageRow {
            uid: 99,
            from: "x@y".to_string(),
            subject: "row".to_string(),
            date: "today".to_string(),
            is_read: true,
            message_href: "/classic/folders/INBOX/messages/99".to_string(),
        }];
        let body = t.render().expect("template renders");
        assert!(
            body.contains("type=\"checkbox\""),
            "bulk-action checkbox missing: {body}"
        );
        assert!(body.contains("name=\"uid\" value=\"99\""));
        // The stub action form's _csrf must thread through too so the
        // P1 #29 task only has to add the action buttons + handler.
        assert!(
            body.contains("name=\"_csrf\" value=\"test-csrf-token\""),
            "bulk-action form must carry the session csrf_token: {body}"
        );
    }

    #[test]
    fn folder_template_table_has_semantic_thead_and_tbody() {
        let body = fresh_template().render().expect("template renders");
        // Real <table>, not <div> soup — gap analysis P0 #8 calls this out.
        assert!(body.contains("<table"), "<table> element missing");
        assert!(body.contains("<thead"), "<thead> missing");
        assert!(body.contains("<tbody"), "<tbody> missing");
        // Sender/Subject/Date column headers are real <th scope="col">.
        let th_count = body.matches("<th scope=\"col\"").count();
        assert!(th_count >= 3, "expected at least 3 <th scope=col>: {body}");
    }
}
