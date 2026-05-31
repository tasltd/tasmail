// Added (TMAIL-355): Scaffold for the `/classic` no-JS server-rendered webmail
// surface. This module owns:
//   * The sub-router mounted at `/classic` from `crate::router`.
//   * The index redirect: GET `/classic/` → `/classic/login` when no session
//     cookie is present, otherwise → `/classic/folders/INBOX`.
//   * The catch-all 404 handler that renders `templates/classic/not_found.html`.
//
// The surface itself (login form, message list, compose, etc.) ships in
// follow-up child tasks under driver TMAIL-299. The base.html laid down here
// is intentionally minimal — TMAIL-356 (P0 #2) is the dedicated task for
// the full accessible, CSP-nonced layout.
//
// Templates resolve from `<crate-root>/templates/`, which is `backend/templates/`
// at build time. Compile-time validation means a typo in a `{{ field }}` name
// fails `cargo build` rather than at runtime on a customer page.

use askama::Template;
use axum::{
    extract::Request,
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
    Router,
};

// PURPOSE: explicit MethodRouter import lets the catch-all route accept any
// method by calling `.fallback(...)` on it. Currently we only wire GET on
// `/classic/{*rest}`; non-GET methods will return 405 from axum which is the
// right behaviour until child tasks need POST/PUT routes under unknown paths.

use crate::error::AppError;
use crate::state::AppState;

// NAME: Session cookie name shared with the Classic UI auth handler that
// lands in TMAIL-357 (P0 #3). The scaffold only needs to *detect* presence;
// the cookie value is opaque here and validated later by the dedicated
// classic_session middleware. Centralising the name here keeps the
// child-task implementation pointed at one source of truth.
pub const CLASSIC_SESSION_COOKIE: &str = "tasmail_classic_sid";

/// 404 template for any path under `/classic/*` that doesn't match a route.
/// Kept in this module (not a standalone `errors` module) so the scaffold
/// stays self-contained — error pages get split out in P2 #45.
#[derive(Template)]
#[template(path = "classic/not_found.html")]
struct NotFoundTemplate {
    path: String,
}

/// Build the `/classic/*` sub-router. Merged from `router.rs`.
///
/// Routes are written with their full `/classic/...` paths (rather than
/// nested under `/classic`) to match the rest of the codebase's pattern
/// and to keep the fallback handler scoped via an explicit `/classic/{*rest}`
/// catch-all instead of `Router::fallback`. A `Router::fallback` here would
/// be promoted to the merged router's fallback and start hijacking misses
/// for `/api/*` and `/`.
///
/// The router is intentionally NOT layered with `auth_middleware` —
/// the Classic surface uses its own cookie-based session middleware (added
/// in TMAIL-357), since JWT-in-Authorization-header auth is useless without
/// JavaScript to attach the header.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/classic", get(index_redirect))
        .route("/classic/", get(index_redirect))
        // PURPOSE: explicit catch-all so the 404 page only fires for paths
        // under `/classic/...`. The `{*rest}` wildcard captures any remaining
        // path segments — child tasks (login, folder, message, compose) will
        // add specific routes ABOVE this one and axum's most-specific-match
        // semantics route correctly.
        .route("/classic/{*rest}", get(not_found))
}

/// GET `/classic/` — redirect based on session presence.
///
/// * No `tasmail_classic_sid` cookie → 303 to `/classic/login`.
/// * Cookie present (any value) → 303 to `/classic/folders/INBOX`.
///
/// The scaffold deliberately does NOT validate the cookie's session id
/// against the database — that's the classic_session middleware's job
/// (TMAIL-357). A stale/invalid cookie will land on the inbox route, the
/// middleware will reject it, and the user gets bounced back to login.
/// That round-trip is acceptable for a sub-1% case and keeps this handler
/// free of DB access.
///
/// Uses `303 See Other` (not 302) so that a future POST-Redirect-Get flow
/// always lands as GET on the target.
async fn index_redirect(headers: HeaderMap) -> Redirect {
    if has_classic_session(&headers) {
        Redirect::to("/classic/folders/INBOX")
    } else {
        Redirect::to("/classic/login")
    }
}

/// Catch-all 404 handler rendered as HTML (not JSON), since this surface is
/// browsed directly by humans. Falls back to a plain-text response if the
/// template itself fails to render — the user always gets *something*
/// rather than the JSON error envelope `AppError` would emit.
///
/// Wired to `/classic/{*rest}` so it only catches unknown paths under
/// `/classic/` — never API misses. Echoes the full request path so the
/// user can spot a typo without having to reconstruct it from the URL bar.
async fn not_found(req: Request) -> Response {
    let path = req.uri().path().to_string();
    let tpl = NotFoundTemplate { path };
    match tpl.render() {
        Ok(body) => (StatusCode::NOT_FOUND, Html(body)).into_response(),
        Err(e) => {
            tracing::error!(error = ?e, "classic 404 template render failed");
            (
                StatusCode::NOT_FOUND,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "404 Not Found",
            )
                .into_response()
        }
    }
}

/// Look for the `tasmail_classic_sid` cookie in the request headers.
///
/// Hand-parsed instead of pulling in `axum-extra::extract::CookieJar` here —
/// the dependency is already in the workspace but the scaffold only needs a
/// name-presence check, not signed-cookie support. The real session
/// middleware (TMAIL-357) will use `CookieJar` for full parsing + value
/// extraction + Set-Cookie response wiring.
fn has_classic_session(headers: &HeaderMap) -> bool {
    let Some(cookie_header) = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    cookie_header
        .split(';')
        .map(str::trim)
        .any(|pair| {
            pair.split_once('=')
                .map(|(name, value)| name == CLASSIC_SESSION_COOKIE && !value.is_empty())
                .unwrap_or(false)
        })
}

/// Helper used by future child tasks to render an Askama template into an
/// axum `Html` response, mapping render errors to `AppError::Internal`.
/// Kept in the scaffold so all child handlers share the same render path
/// (consistent error logging, single integration point if we ever swap
/// engines).
#[allow(dead_code)] // Will be used by child tasks (login, folder, message, compose, ...)
pub(crate) fn render_html<T: Template>(template: &T) -> Result<Html<String>, AppError> {
    template
        .render()
        .map(Html)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("classic template render failed: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn no_cookie_header_means_no_session() {
        let headers = HeaderMap::new();
        assert!(!has_classic_session(&headers));
    }

    #[test]
    fn unrelated_cookie_means_no_session() {
        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, HeaderValue::from_static("other=abc"));
        assert!(!has_classic_session(&headers));
    }

    #[test]
    fn empty_value_means_no_session() {
        // A cleared session cookie (Max-Age=0) commonly leaves the name behind
        // with an empty value on the next request. Treat that as no session.
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("tasmail_classic_sid="),
        );
        assert!(!has_classic_session(&headers));
    }

    #[test]
    fn cookie_present_means_session() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("tasmail_classic_sid=abc123"),
        );
        assert!(has_classic_session(&headers));
    }

    #[test]
    fn cookie_among_others_is_detected() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("foo=bar; tasmail_classic_sid=xyz; baz=qux"),
        );
        assert!(has_classic_session(&headers));
    }

    #[test]
    fn not_found_template_renders() {
        let tpl = NotFoundTemplate {
            path: "/classic/does-not-exist".to_string(),
        };
        let body = tpl.render().expect("template should render");
        assert!(body.contains("Page not found"));
        assert!(body.contains("/classic/does-not-exist"));
        assert!(body.contains("<a href=\"/classic/\""));
    }

    #[test]
    fn not_found_template_escapes_path() {
        // PURPOSE: defence-in-depth — Askama escapes by default for the .html
        // extension, but lock the behaviour in with a test so a future config
        // change can't silently turn it off.
        let tpl = NotFoundTemplate {
            path: "/classic/<script>alert(1)</script>".to_string(),
        };
        let body = tpl.render().expect("template should render");
        assert!(!body.contains("<script>alert(1)</script>"));
        assert!(body.contains("&#60;script&#62;") || body.contains("&lt;script&gt;"));
    }
}
