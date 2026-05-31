// Added (TMAIL-373): GET /classic/search — search form + paginated results
// for the no-JS Classic UI surface.
//
// What this renders
// -----------------
//   * A search form with the query input (echoed back from `?q=`) and an
//     optional folder selector. The same form lives in the base.html nav
//     so it's reachable from every page; this view re-renders it as the
//     primary action so a user can refine without scrolling up.
//   * When `q` is missing or whitespace-only, an empty state ("Type a
//     search term above") — no IMAP work is done.
//   * When `q` is present, the matching messages are listed in the same
//     semantic `<table>` shape as the folder view (Folder | Sender |
//     Subject | Date). Each row links to the read view at
//     `/classic/folders/{folder}/messages/{uid}`.
//   * Pagination across the up-to-100 results that `ImapService::search_messages`
//     returns. Page size is 25 to mirror the folder view; the slice is
//     done in memory because IMAP SEARCH doesn't paginate at the protocol
//     level.
//
// Why not call `/api/search` over HTTP
// ------------------------------------
// The classic UI shares a binary with the JSON API, so going through HTTP
// would mean re-authing (the JSON API uses Bearer JWT, the Classic UI uses
// a cookie session) and re-fetching the same IMAP work. Calling
// `ImapService::search_messages` directly is the same code path the JSON
// handler uses without the round-trip overhead.
//
// AI / NLP search note
// --------------------
// The issue spec mentions falling back to `/api/search/nlp` when an AI
// config is set. The NLP handler today PARSES the query into search
// params but does NOT execute an IMAP search yet (the handler returns
// `results: Vec::new()` — see `handlers::nlp_search::nlp_search`). Until
// the NLP execution path is wired, we render a small "AI search is
// configured — use the Settings panel to query naturally" hint when an
// active AI config is present, and execute the plain IMAP SEARCH for the
// actual results. That keeps the form working today and the upgrade path
// open for a follow-up.

use askama::Template;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::AppError;
use crate::services::auth_service::Claims;
use crate::services::imap_service::{ImapService, MessageEnvelope};
use crate::state::AppState;
use crate::validation;

use super::CspNonce;

/// Page size for the Classic UI search results table. Matches the folder
/// view's 25-per-page setting so the user gets a consistent rhythm
/// between the two surfaces. The upstream search caps the result set at
/// 100, so `total_pages` will never exceed 4 today.
const PAGE_SIZE: u32 = 25;

/// Default folder searched when `?folder=` is absent. IMAP SEARCH is
/// folder-scoped so a hard pick is required — INBOX matches what the
/// rest of the Classic UI defaults to for newly-arrived mail.
const DEFAULT_FOLDER: &str = "INBOX";

/// Query string for `GET /classic/search`.
///
/// All fields default cleanly so a bare `/classic/search` URL renders the
/// empty-state landing page rather than 400-ing the user.
#[derive(Debug, Deserialize, Default)]
pub struct SearchQuery {
    /// The user's search text. Trimmed before use. Empty / whitespace-only
    /// triggers the empty-state branch — no IMAP call is issued.
    #[serde(default)]
    pub q: Option<String>,
    /// IMAP folder to search inside. Defaults to `INBOX` when missing /
    /// empty. Validated against the standard folder-name rules so a
    /// malformed value 400s rather than reaching IMAP.
    #[serde(default)]
    pub folder: Option<String>,
    /// 0-based page index into the in-memory result slice. Anything
    /// negative / non-numeric serde decodes as `None` and we default to 0.
    /// A page beyond the result count still renders (empty rows) so a
    /// stale bookmark doesn't 500.
    #[serde(default)]
    pub page: Option<u32>,
}

impl SearchQuery {
    fn page(&self) -> u32 {
        self.page.unwrap_or(0)
    }

    /// Resolve the requested folder, falling back to INBOX. Returns the
    /// trimmed value so a malformed `?folder= ` doesn't reach IMAP.
    fn folder_or_default(&self) -> String {
        self.folder
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_FOLDER)
            .to_string()
    }

    /// Return the trimmed query string, or `None` when the field is
    /// missing / whitespace-only.
    fn trimmed_query(&self) -> Option<String> {
        self.q
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    }
}

/// A single result row in the search-results table. Carries the source
/// folder alongside the subject/date so the user can see WHERE the match
/// lives (cross-folder context in a single list).
pub struct SearchResultRow {
    pub uid: u32,
    pub folder: String,
    pub from: String,
    pub subject: String,
    pub date: String,
    pub is_read: bool,
    /// Pre-built href to `/classic/folders/{folder_href}/messages/{uid}`.
    pub message_href: String,
}

/// Askama template struct backing `templates/classic/search.html`.
#[derive(Template)]
#[template(path = "classic/search.html")]
pub struct SearchTemplate {
    /// Echoed back into the form's input value so the user can refine
    /// without retyping. Empty string when no query was submitted.
    pub query: String,
    /// Folder being searched (display + form value). Always non-empty
    /// (defaulted to INBOX when missing).
    pub folder: String,
    /// True when the user submitted a query. Drives the results /
    /// empty-state branch in the template.
    pub has_query: bool,
    /// True when an AI config is active for this user — drives a small
    /// hint pointing at the NLP search settings. See module-level docs
    /// for the rationale on why this is a hint, not a redirect.
    pub ai_configured: bool,
    /// Total number of matching messages (across all pages, capped at
    /// the 100-result IMAP SEARCH window).
    pub total_results: u32,
    /// 0-based page being rendered.
    pub current_page: u32,
    /// Total page count, minimum 1 so the "Page 1 of 1" footer renders
    /// on empty results without divide-by-zero.
    pub total_pages: u32,
    /// 1-based index of the first row on this page (`0` when the page
    /// is empty).
    pub first_index: u32,
    /// 1-based index of the last row on this page.
    pub last_index: u32,
    pub has_prev: bool,
    pub has_next: bool,
    /// Pre-built `?q=…&folder=…&page=N` href for the prev link.
    pub prev_href: String,
    /// Same shape for the next link.
    pub next_href: String,
    /// Flattened rows for this page. Empty vec when there are no matches
    /// OR the page index is past the last result page.
    pub results: Vec<SearchResultRow>,
    /// Session CSRF token, threaded through the logout form partial in
    /// the base.html nav.
    pub csrf_token: String,
    /// Per-request CSP nonce. Required by base.html (TMAIL-356).
    pub csp_nonce: String,
    /// Added (TMAIL-384): Footer quota indicator ("Using X of Y · NN%").
    /// `None` when the loader couldn't reach the cache + DB; the partial
    /// renders nothing in that branch so the rest of the page still loads.
    pub quota_indicator: Option<super::QuotaIndicator>,
}

/// URL-encode a folder name for path-segment use. Same helper shape as
/// `handlers::classic::folder::encode_folder_segment` — copied locally
/// rather than re-exported so the search handler doesn't develop a
/// load-bearing dependency on the folder module's internals.
fn encode_folder_segment(name: &str) -> String {
    urlencoding::encode(name).into_owned()
}

/// URL-encode a value for query-string use. Spaces become `%20` (the
/// path-safe form) so the same encoder works for both pieces of the
/// "/classic/search?q=…&folder=…&page=N" hrefs.
fn encode_query_value(value: &str) -> String {
    urlencoding::encode(value).into_owned()
}

/// Compute `(total_pages, first_index, last_index)` from the result count
/// and current page index. All indices are 1-based for direct rendering.
/// Pulled out so unit tests can pin the maths without touching IMAP.
fn pagination_window(total: u32, page: u32, rows_on_page: u32) -> (u32, u32, u32) {
    let total_pages = if total == 0 {
        1
    } else {
        total.div_ceil(PAGE_SIZE)
    };
    if rows_on_page == 0 {
        return (total_pages, 0, 0);
    }
    let first = page * PAGE_SIZE + 1;
    let last = (first + rows_on_page - 1).min(total);
    (total_pages, first, last)
}

/// Test the IMAP `\Seen` flag the same way `folder.rs` does. Centralised
/// here so a future bug fix on the flag-spelling check fans out via the
/// same import path.
fn message_is_read(env: &MessageEnvelope) -> bool {
    env.flags
        .iter()
        .any(|f| f.eq_ignore_ascii_case("Seen") || f.eq_ignore_ascii_case("\\Seen"))
}

/// Convert a single IMAP envelope into a row the template can render.
/// `folder` is the source folder (the user-visible string); both the
/// display and the href segment derive from it.
fn envelope_to_row(env: MessageEnvelope, folder: &str) -> SearchResultRow {
    let folder_href = encode_folder_segment(folder);
    SearchResultRow {
        message_href: format!("/classic/folders/{}/messages/{}", folder_href, env.uid),
        is_read: message_is_read(&env),
        uid: env.uid,
        folder: folder.to_string(),
        from: env.from.unwrap_or_default(),
        subject: env
            .subject
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "(no subject)".to_string()),
        date: env.date.unwrap_or_default(),
    }
}

/// Build the prev / next hrefs given the current query, folder, and page.
/// Pulled out because the same encoding logic runs twice and an inline
/// `format!` got hairy enough to be worth a helper.
fn build_pagination_href(query: &str, folder: &str, page: u32) -> String {
    format!(
        "/classic/search?q={}&folder={}&page={}",
        encode_query_value(query),
        encode_query_value(folder),
        page
    )
}

/// GET /classic/search?q=…&folder=…&page=N — render the search view.
///
/// Three rendering branches:
///   * No `q` → empty state, IMAP not touched.
///   * `q` present, no matches → empty state under the form ("No messages
///     match …").
///   * `q` present, matches → paginated results table.
///
/// A failing IMAP search returns `AppError::Imap` which renders as a 500
/// via the global error layer — same surface area as the folder view.
pub async fn get_search(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Extension(session): Extension<crate::models::classic_session::ClassicSession>,
    Extension(csp_nonce): Extension<CspNonce>,
    Query(query): Query<SearchQuery>,
) -> Result<Response, AppError> {
    let mailbox_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID in classic claims")))?;

    let folder = query.folder_or_default();
    // Validate the folder name before reaching IMAP — same rule the JSON
    // /api/search uses. A malformed value (e.g. CRLF injection) 400s
    // rather than corrupting an IMAP command.
    validation::validate_folder_name(&folder)?;

    let page = query.page();
    let trimmed = query.trimmed_query();

    // Check for an active AI config — drives the small "AI search is
    // available" hint in the template. A DB error here shouldn't break
    // the search page (the user can still search the normal way), so a
    // failure logs + treats it as "not configured".
    let ai_configured = match crate::models::ai_config::AiConfiguration::find_active(
        &state.db,
        mailbox_id,
    )
    .await
    {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(e) => {
            tracing::warn!(error = ?e, "classic search AI config check failed");
            false
        }
    };

    // Added (TMAIL-384): hydrate the footer quota indicator once per
    // request. Cache-first via `state.cache.get_quota`; on miss it runs
    // the same DB queries `/api/quota` uses. Returns `None` on any error
    // so the partial silently omits the footer line rather than 500-ing
    // the search page.
    let quota_indicator = super::load_quota_indicator(&state, mailbox_id).await;

    // Empty-state path: no q means no IMAP work.
    let Some(trimmed_query) = trimmed else {
        let template = SearchTemplate {
            query: String::new(),
            folder: folder.clone(),
            has_query: false,
            ai_configured,
            total_results: 0,
            current_page: 0,
            total_pages: 1,
            first_index: 0,
            last_index: 0,
            has_prev: false,
            has_next: false,
            prev_href: String::new(),
            next_href: String::new(),
            results: Vec::new(),
            csrf_token: session.csrf_token.clone(),
            csp_nonce: csp_nonce.into_string(),
            quota_indicator: quota_indicator.clone(),
        };
        let body = template.render().map_err(|e| {
            AppError::Internal(anyhow::anyhow!(
                "classic search empty-state render failed: {e}"
            ))
        })?;
        return Ok(html_response(body));
    };

    // Query present — validate it (length, IMAP injection guards) before
    // reaching the IMAP SEARCH command.
    validation::validate_search_query(&trimmed_query)?;

    // Build the BYOK IMAP service from the user's saved imap_configurations
    // row. Same pattern as folder.rs::get_folder.
    let imap_service = ImapService::for_user(&state, mailbox_id).await?;
    let (username, password) = imap_service
        .user_creds()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("BYOK creds missing on ImapService")))?;
    let username = username.to_string();
    let password = password.to_string();

    let envelopes = imap_service
        .search_messages(&username, &password, &folder, &trimmed_query)
        .await?;

    // In-memory pagination over the up-to-100 results IMAP SEARCH
    // returned. Slicing here keeps the protocol layer simple (it already
    // returns newest-first) and matches the gap-analysis spec.
    let total_results = envelopes.len() as u32;
    let start = (page as usize).saturating_mul(PAGE_SIZE as usize);
    let end = start.saturating_add(PAGE_SIZE as usize).min(envelopes.len());
    let page_slice: Vec<MessageEnvelope> = if start < envelopes.len() {
        envelopes[start..end].to_vec()
    } else {
        Vec::new()
    };
    let rows_on_page = page_slice.len() as u32;
    let (total_pages, first_index, last_index) =
        pagination_window(total_results, page, rows_on_page);
    let results: Vec<SearchResultRow> = page_slice
        .into_iter()
        .map(|e| envelope_to_row(e, &folder))
        .collect();

    let has_prev = page > 0;
    let has_next = total_pages > 0 && page + 1 < total_pages;
    let prev_href = if has_prev {
        build_pagination_href(&trimmed_query, &folder, page - 1)
    } else {
        String::new()
    };
    let next_href = if has_next {
        build_pagination_href(&trimmed_query, &folder, page + 1)
    } else {
        String::new()
    };

    let template = SearchTemplate {
        query: trimmed_query,
        folder,
        has_query: true,
        ai_configured,
        total_results,
        current_page: page,
        total_pages,
        first_index,
        last_index,
        has_prev,
        has_next,
        prev_href,
        next_href,
        results,
        csrf_token: session.csrf_token.clone(),
        csp_nonce: csp_nonce.into_string(),
        quota_indicator,
    };

    let body = template.render().map_err(|e| {
        AppError::Internal(anyhow::anyhow!("classic search template render failed: {e}"))
    })?;
    Ok(html_response(body))
}

/// Wrap an Askama-rendered body in the standard `text/html; charset=utf-8`
/// response shape every Classic-UI handler emits.
fn html_response(body: String) -> Response {
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
        body,
    )
        .into_response()
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

    fn empty_template() -> SearchTemplate {
        SearchTemplate {
            query: String::new(),
            folder: "INBOX".to_string(),
            has_query: false,
            ai_configured: false,
            total_results: 0,
            current_page: 0,
            total_pages: 1,
            first_index: 0,
            last_index: 0,
            has_prev: false,
            has_next: false,
            prev_href: String::new(),
            next_href: String::new(),
            results: Vec::new(),
            csrf_token: "test-csrf-token".to_string(),
            csp_nonce: "test-nonce-fixed".to_string(),
            quota_indicator: None,
        }
    }

    // ----- SearchQuery -----

    #[test]
    fn search_query_defaults_folder_to_inbox() {
        let q = SearchQuery::default();
        assert_eq!(q.folder_or_default(), "INBOX");
    }

    #[test]
    fn search_query_trims_folder_whitespace_to_inbox() {
        let q = SearchQuery {
            folder: Some("   ".to_string()),
            ..Default::default()
        };
        assert_eq!(q.folder_or_default(), "INBOX");
    }

    #[test]
    fn search_query_preserves_real_folder_name() {
        let q = SearchQuery {
            folder: Some("Drafts".to_string()),
            ..Default::default()
        };
        assert_eq!(q.folder_or_default(), "Drafts");
    }

    #[test]
    fn search_query_trimmed_query_drops_whitespace_only_value() {
        let q = SearchQuery {
            q: Some("   ".to_string()),
            ..Default::default()
        };
        assert!(q.trimmed_query().is_none());
    }

    #[test]
    fn search_query_trimmed_query_strips_padding() {
        let q = SearchQuery {
            q: Some("  invoice  ".to_string()),
            ..Default::default()
        };
        assert_eq!(q.trimmed_query().as_deref(), Some("invoice"));
    }

    #[test]
    fn search_query_page_defaults_to_zero() {
        let q = SearchQuery::default();
        assert_eq!(q.page(), 0);
    }

    // ----- pagination_window -----

    #[test]
    fn pagination_window_empty_renders_one_page() {
        // Empty results still need a "page 1 of 1" footer rather than
        // "page 1 of 0".
        let (pages, first, last) = pagination_window(0, 0, 0);
        assert_eq!((pages, first, last), (1, 0, 0));
    }

    #[test]
    fn pagination_window_exact_page_size_is_one_page() {
        let (pages, first, last) = pagination_window(25, 0, 25);
        assert_eq!(pages, 1);
        assert_eq!(first, 1);
        assert_eq!(last, 25);
    }

    #[test]
    fn pagination_window_thirty_results_two_pages() {
        // 30 hits → page 0 (1..25), page 1 (26..30).
        let (pages, first, last) = pagination_window(30, 0, 25);
        assert_eq!(pages, 2);
        assert_eq!(first, 1);
        assert_eq!(last, 25);

        let (_, first, last) = pagination_window(30, 1, 5);
        assert_eq!(first, 26);
        assert_eq!(last, 30);
    }

    #[test]
    fn pagination_window_beyond_last_page_zeroes_indices() {
        // Stale `?page=99` on a 5-hit search → 0 rows, indices zero so
        // the footer renders "No results" cleanly.
        let (pages, first, last) = pagination_window(5, 99, 0);
        assert_eq!(pages, 1);
        assert_eq!((first, last), (0, 0));
    }

    #[test]
    fn pagination_window_caps_last_at_total() {
        // Defensive: even if rows_on_page were over-reported, the last
        // index can't exceed total.
        let (_, _, last) = pagination_window(10, 0, 10);
        assert_eq!(last, 10);
    }

    // ----- encode_folder_segment / encode_query_value -----

    #[test]
    fn encode_folder_segment_percent_encodes_slash_and_brackets() {
        // Gmail-style folder name must survive a path-segment round trip.
        let encoded = encode_folder_segment("[Gmail]/Sent Mail");
        assert!(!encoded.contains('/'));
        assert!(!encoded.contains('['));
        assert!(!encoded.contains(']'));
        assert!(!encoded.contains(' '));
    }

    #[test]
    fn encode_query_value_uses_percent20_not_plus_for_spaces() {
        // `+` is form-style and would round-trip back as `+` if the
        // browser doesn't decode it — `%20` is the safer choice for the
        // query string we re-decode on the next request.
        let encoded = encode_query_value("hello world");
        assert!(encoded.contains("%20"));
        assert!(!encoded.contains('+'));
    }

    // ----- envelope_to_row -----

    #[test]
    fn envelope_to_row_falls_back_to_no_subject_on_missing_value() {
        // Empty subject must render as "(no subject)" so the link is
        // always clickable — an empty <a> would be invisible to lynx
        // and would fail screen-reader landmark scans.
        let envelope = env(42, None, Some("alice@example.com"), &[]);
        let row = envelope_to_row(envelope, "INBOX");
        assert_eq!(row.subject, "(no subject)");
        assert_eq!(row.from, "alice@example.com");
        assert_eq!(row.uid, 42);
        assert_eq!(row.message_href, "/classic/folders/INBOX/messages/42");
    }

    #[test]
    fn envelope_to_row_marks_seen_flag_as_read() {
        let envelope = env(1, Some("Hi"), Some("a@b"), &["\\Seen"]);
        let row = envelope_to_row(envelope, "INBOX");
        assert!(row.is_read);
    }

    #[test]
    fn envelope_to_row_treats_missing_seen_as_unread() {
        let envelope = env(1, Some("Hi"), Some("a@b"), &[]);
        let row = envelope_to_row(envelope, "INBOX");
        assert!(!row.is_read);
    }

    #[test]
    fn envelope_to_row_percent_encodes_folder_in_href() {
        // `[Gmail]/All Mail` must survive into the read-view href.
        let envelope = env(7, Some("x"), Some("y"), &[]);
        let row = envelope_to_row(envelope, "[Gmail]/All Mail");
        assert!(row.message_href.starts_with("/classic/folders/"));
        assert!(row.message_href.ends_with("/messages/7"));
        assert!(!row.message_href.contains(' '));
        // Source folder stays human-readable for display.
        assert_eq!(row.folder, "[Gmail]/All Mail");
    }

    // ----- build_pagination_href -----

    #[test]
    fn build_pagination_href_encodes_query_and_folder() {
        let href = build_pagination_href("hello world", "[Gmail]/Sent Mail", 2);
        assert!(href.starts_with("/classic/search?q="));
        assert!(href.contains("&page=2"));
        // Both encoded — no raw spaces / brackets / slashes left in the URL.
        assert!(!href.contains(' '));
        assert!(!href.contains('['));
    }

    // ----- template renders -----

    #[test]
    fn empty_state_template_renders_form_only() {
        // No `has_query` and an empty results vec → the empty-state copy
        // ("Search your mailbox") sits where the table would normally go.
        let body = empty_template().render().expect("template should render");
        assert!(
            body.contains("Search your mailbox") || body.contains("Type a search term"),
            "empty state should prompt the user to type a query: {body}"
        );
        // No results table when no query was submitted.
        assert!(
            !body.contains("No messages match"),
            "no-match copy must NOT render on the bare landing page"
        );
    }

    #[test]
    fn empty_state_template_renders_search_form_input() {
        let body = empty_template().render().expect("template should render");
        // The form input must always render so the user has somewhere to
        // type — both empty-state and results pages.
        assert!(
            body.contains("name=\"q\""),
            "search form input `name=\"q\"` must render: {body}"
        );
        assert!(
            body.contains("action=\"/classic/search\""),
            "form action must point at /classic/search: {body}"
        );
        assert!(
            body.contains("method=\"get\""),
            "search form must be GET so URLs are bookmarkable: {body}"
        );
    }

    #[test]
    fn results_template_renders_each_match() {
        let mut tpl = empty_template();
        tpl.query = "invoice".to_string();
        tpl.has_query = true;
        tpl.total_results = 2;
        tpl.first_index = 1;
        tpl.last_index = 2;
        tpl.results = vec![
            envelope_to_row(env(10, Some("invoice 2026"), Some("alice@x.com"), &["\\Seen"]), "INBOX"),
            envelope_to_row(env(11, Some("invoice 2025"), Some("bob@x.com"), &[]), "INBOX"),
        ];
        let body = tpl.render().expect("template should render");
        assert!(body.contains("invoice 2026"));
        assert!(body.contains("invoice 2025"));
        assert!(body.contains("alice@x.com"));
        assert!(body.contains("bob@x.com"));
        assert!(body.contains("/classic/folders/INBOX/messages/10"));
        assert!(body.contains("/classic/folders/INBOX/messages/11"));
        // Echo the query back into the input so the user can refine.
        assert!(body.contains("value=\"invoice\""));
    }

    #[test]
    fn results_template_renders_no_matches_message_on_empty_results() {
        // q present but zero matches → render the "No messages match…"
        // copy under the form, NOT the empty-landing copy.
        let mut tpl = empty_template();
        tpl.query = "unmatchable".to_string();
        tpl.has_query = true;
        tpl.total_results = 0;
        tpl.results = Vec::new();
        let body = tpl.render().expect("template should render");
        assert!(
            body.contains("No messages match"),
            "must render no-match copy when q is present but results empty: {body}"
        );
        assert!(body.contains("unmatchable"));
    }

    #[test]
    fn results_template_renders_pagination_links_when_more_pages_available() {
        // The pagination block only renders inside the results branch
        // (a hit table), so the fixture has to provide at least one row
        // — otherwise the empty-state copy fires above and the
        // pagination nav never gets emitted.
        let mut tpl = empty_template();
        tpl.query = "many".to_string();
        tpl.has_query = true;
        tpl.total_results = 60;
        tpl.current_page = 1;
        tpl.total_pages = 3;
        tpl.first_index = 26;
        tpl.last_index = 50;
        tpl.has_prev = true;
        tpl.has_next = true;
        tpl.prev_href = "/classic/search?q=many&folder=INBOX&page=0".to_string();
        tpl.next_href = "/classic/search?q=many&folder=INBOX&page=2".to_string();
        tpl.results = vec![envelope_to_row(
            env(100, Some("hit"), Some("alice@x.com"), &[]),
            "INBOX",
        )];
        let body = tpl.render().expect("template should render");
        assert!(body.contains(r#"rel="prev""#), "rel=prev missing: {body}");
        assert!(body.contains(r#"rel="next""#), "rel=next missing: {body}");
        assert!(body.contains("page=0"));
        assert!(body.contains("page=2"));
        assert!(body.contains("Page 2 of 3"));
    }

    #[test]
    fn results_template_echoes_query_safely_against_xss() {
        // Auto-escape is on for `.html` extensions; lock it down with a
        // payload that would be a script tag if it leaked through raw.
        let mut tpl = empty_template();
        tpl.query = "<script>alert(1)</script>".to_string();
        tpl.has_query = true;
        let body = tpl.render().expect("template should render");
        assert!(
            !body.contains("<script>alert(1)</script>"),
            "raw <script> payload must not appear in rendered output: {body}"
        );
    }

    #[test]
    fn results_template_renders_ai_hint_when_configured() {
        let mut tpl = empty_template();
        tpl.ai_configured = true;
        let body = tpl.render().expect("template should render");
        // The hint mentions the AI / NLP configuration — exact copy is
        // free to change but the substring "AI" anchors the assertion.
        assert!(
            body.contains("AI") || body.contains("Natural language"),
            "AI hint should render when ai_configured = true: {body}"
        );
    }

    #[test]
    fn results_template_omits_ai_hint_when_not_configured() {
        let tpl = empty_template();
        let body = tpl.render().expect("template should render");
        // Don't dangle a "configure AI search" CTA on the page when the
        // user has not opted in — that's settings-page work, not search.
        assert!(
            !body.contains("AI search is configured"),
            "AI hint must NOT render when ai_configured = false: {body}"
        );
    }
}
