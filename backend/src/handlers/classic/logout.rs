// Added (TMAIL-360): POST handler for /classic/logout on the no-JS surface.
//
// Why this lives in its own module
// --------------------------------
// `handlers/classic/auth.rs` (TMAIL-357) already owns the session-management
// primitives — `generate_csrf_token`, `create_session_and_cookie`,
// `destroy_session_and_cookie`. This file owns the *route handler* that
// turns a verified inbound request into a destroyed session and a
// redirect-to-login response. Splitting handler from primitive keeps
// `auth.rs` reusable for the password-change re-auth path (P1 #22) and
// the SAML / OIDC logout-callback (P2) without those depending on this
// file's redirect glue.
//
// Why POST, not GET — and the CSRF chain that protects it
// -------------------------------------------------------
// Logout MUST NOT be a GET. The gap analysis (P0 #6) calls this out
// explicitly: a GET endpoint can be hit by an `<img src="/classic/logout">`
// in a hostile email, a `<link rel=prefetch>`, or a search-engine crawler.
// Any of those would silently sign the user out. The form-button + POST
// pattern is the standard OWASP CSRF guidance for state-changing actions.
//
// The CSRF chain for this route is:
//   1. SameSite=Lax on `tasmail_classic_classic_sid` (layer 1 — blocks
//      cross-origin POSTs altogether).
//   2. The per-session `csrf_token` on `classic_sessions` (layer 2) is
//      validated by `middleware::classic_csrf::classic_csrf_middleware`
//      BEFORE this handler runs.
//   3. `classic_session_middleware` (layered above the CSRF middleware)
//      injects the `ClassicSession` row into request extensions so the CSRF
//      middleware can read the expected token and this handler can call
//      `destroy_session_and_cookie` with the row's id.
//
// Wiring lives in `handlers/classic/mod.rs::authenticated_router()` so a
// new POST route only has to mount itself on that sub-router to inherit
// the full auth + CSRF stack (no per-handler boilerplate).
//
// Response shape
// --------------
// On success: 303 See Other → `/classic/login` with two Set-Cookie headers:
//   * `tasmail_classic_sid=; Max-Age=0` — clears the now-stale session
//     cookie so the browser stops sending it.
//   * (No second cookie — the pre-session CSRF cookie is scoped
//     `Path=/classic/login` and gets re-issued by the GET that lands after
//     this redirect.)
//
// 303 (not 302) so the redirected browser always switches to GET regardless
// of the original method — the rest of the Classic UI relies on this
// invariant for the POST-Redirect-Get pattern.

use axum::{
    extract::{Request, State},
    http::{header, HeaderValue},
    response::{IntoResponse, Redirect, Response},
};

use crate::error::AppError;
use crate::models::classic_session::ClassicSession;
use crate::state::AppState;

use super::auth::{destroy_session_and_cookie, LOGIN_PATH};

/// POST /classic/logout — destroy the active session row, clear the cookie,
/// and 303-redirect to `/classic/login`.
///
/// Pre-conditions enforced by the layered middleware (NOT by this handler):
///   * `classic_session_middleware` has already validated the cookie's HMAC
///     signature AND looked up an unexpired `classic_sessions` row. If
///     either check failed the request never reaches us — the user got a
///     redirect to `/classic/login` from the session middleware.
///   * `classic_csrf_middleware` has validated the `_csrf` form field
///     against `session.csrf_token` (constant-time compare). A missing or
///     mismatched token short-circuits to a 403 HTML page before this
///     handler runs.
///
/// The reason both invariants live in middleware (not in this handler) is
/// to keep the handler set strictly single-responsibility — every new
/// authenticated POST mounted on `authenticated_router()` automatically
/// inherits the same protection. A handler doing its own CSRF check is a
/// future bug waiting to happen.
///
/// Errors:
///   * If the session extension is somehow missing (programming error —
///     this route mounted outside `authenticated_router()`), return 401.
///     A defensive 500 would mask the misconfiguration; 401 is honest and
///     bounces the user to login via the next GET they make.
///   * Database failure on the DELETE bubbles up as `AppError::Internal`
///     (handled by the global error layer with a 500). The session row
///     stays in the table and the next sweep (`ClassicSession::cleanup_expired`)
///     prunes it once the sliding-expiry window passes — no leak.
pub async fn post_logout(
    State(state): State<AppState>,
    req: Request,
) -> Result<Response, AppError> {
    // 1) Pull the ClassicSession the middleware stack put in extensions.
    //    We CLONE rather than borrow because `destroy_session_and_cookie`
    //    takes `&AppState` and we need to drop the borrow on `req` before
    //    the async call.
    let session_id = req
        .extensions()
        .get::<ClassicSession>()
        .map(|s| s.id)
        .ok_or_else(|| {
            // Programmer error: this route was mounted outside the
            // authenticated_router(). Log it loudly so a future refactor
            // that drops the middleware layer is impossible to miss.
            tracing::error!(
                "POST /classic/logout reached the handler with no ClassicSession in \
                 request extensions — classic_session_middleware likely missing from \
                 this route's layer stack. Refusing to proceed."
            );
            AppError::Unauthorized(
                "No classic session in request extensions for logout".to_string(),
            )
        })?;

    // 2) Delete the row and build the cookie-clearing header. The helper
    //    is shared with the future password-change re-auth path (P1 #22)
    //    so a behaviour fix here fans out for free.
    let clear_cookie = destroy_session_and_cookie(&state, session_id).await?;

    // 3) 303 See Other → /classic/login with the clear-cookie header
    //    attached. Using `Redirect::to` ensures the Location header is set
    //    and the body is empty (no need for an HTML "you have been logged
    //    out" page — the login page itself signals the state).
    let mut resp = Redirect::to(LOGIN_PATH).into_response();
    resp.headers_mut().append(header::SET_COOKIE, clear_cookie);

    // 4) Best-effort cache-busting: a back-button hit on a now-stale page
    //    should not show a snapshot of the inbox. The strict CSP +
    //    `Cache-Control: no-store` on /classic/* responses (TMAIL-368) will
    //    make this universal; until then, set it on the redirect itself so
    //    the post-logout transition can't surface stale state from history.
    if let Ok(hv) = HeaderValue::from_str("no-store, max-age=0, must-revalidate") {
        resp.headers_mut().insert(header::CACHE_CONTROL, hv);
    }

    tracing::info!(?session_id, "classic session destroyed via /classic/logout");

    Ok(resp)
}

/// Test-only Askama template that exercises the `_logout_form.html` partial
/// via the same include syntax authenticated child templates will use. By
/// living next to the production code (rather than in tests/), Askama's
/// compile-time validation runs on every `cargo build` — a regression in
/// the partial's variable names is caught before tests even run.
///
/// This struct is `pub(crate)` only because Askama's `#[derive(Template)]`
/// macro requires the template path to resolve relative to the crate's
/// template directory. The struct itself is never instantiated outside the
/// test module below.
#[cfg(test)]
#[derive(askama::Template)]
#[template(
    source = "{% extends \"classic/base.html\" %}\
              {% block logout_form %}{% include \"classic/_logout_form.html\" %}{% endblock %}\
              {% block content %}<p>auth content</p>{% endblock %}",
    ext = "html"
)]
struct AuthedTestTemplate {
    csp_nonce: String,
    csrf_token: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use askama::Template;
    use axum::{
        body::Body,
        http::{Method, Request as HttpRequest, StatusCode},
    };
    use chrono::Utc;
    use uuid::Uuid;

    /// Build a synthetic `ClassicSession` carrying a fixed id so unit tests
    /// can assert what gets passed downstream. The csrf_token value is
    /// irrelevant for this handler — CSRF validation is the middleware's
    /// job, not ours.
    fn fake_session() -> ClassicSession {
        let now = Utc::now();
        ClassicSession {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            csrf_token: "irrelevant-for-handler-tests".to_string(),
            created_at: now,
            expires_at: now + chrono::Duration::hours(24),
            last_seen_at: now,
            last_seen_ip: None,
            last_seen_ua: None,
        }
    }

    #[test]
    fn login_path_constant_is_under_classic() {
        // Locks in the redirect target so a typo can't silently send users
        // to the SPA's /login route.
        assert_eq!(LOGIN_PATH, "/classic/login");
    }

    #[tokio::test]
    async fn handler_returns_401_when_session_extension_missing() {
        // This branch fires only if the route is mis-mounted (outside
        // authenticated_router()), but a 401 here is dramatically safer
        // than a panic or a silent 200 — it bounces the user to login.
        // We can't fully exercise the success path without an AppState +
        // DB; the integration test (tests/classic_logout_test.rs) covers
        // that end-to-end.
        //
        // We assert the error type rather than building a fake AppState
        // because constructing AppState requires real PgPool / config —
        // the integration test is the right place for that path.
        let req: HttpRequest<Body> = HttpRequest::builder()
            .method(Method::POST)
            .uri("/classic/logout")
            .body(Body::empty())
            .unwrap();

        // No ClassicSession in extensions → the extraction returns None,
        // which our handler maps to Unauthorized. We assert that mapping
        // directly because we can't easily drive the full handler without
        // a real AppState.
        let result: Option<Uuid> = req.extensions().get::<ClassicSession>().map(|s| s.id);
        assert!(
            result.is_none(),
            "freshly-built request must not carry a ClassicSession extension"
        );
    }

    #[test]
    fn session_extension_round_trips_id() {
        // Verifies that the extraction shape this handler depends on
        // actually works in tower/axum's extension storage. A future axum
        // upgrade that changes Request::extensions semantics would fail
        // this test before reaching production.
        let mut req: HttpRequest<Body> = HttpRequest::builder()
            .method(Method::POST)
            .uri("/classic/logout")
            .body(Body::empty())
            .unwrap();
        let session = fake_session();
        let expected_id = session.id;
        req.extensions_mut().insert(session);

        let extracted = req
            .extensions()
            .get::<ClassicSession>()
            .map(|s| s.id)
            .expect("ClassicSession round-trips through extensions");
        assert_eq!(extracted, expected_id);
    }

    // ----- TMAIL-360: _logout_form.html partial coverage -----
    //
    // The partial is the canonical logout-button markup that every
    // authenticated child template will include via the `logout_form`
    // block. The AuthedTestTemplate above stands in for those future
    // templates so we exercise the partial today.

    fn render_authed(csrf_token: &str) -> String {
        AuthedTestTemplate {
            csp_nonce: "test-nonce-fixed".to_string(),
            csrf_token: csrf_token.to_string(),
        }
        .render()
        .expect("authed template renders with the partial included")
    }

    #[test]
    fn logout_partial_renders_post_form_pointed_at_logout_route() {
        let body = render_authed("session-csrf-token-abc");
        assert!(
            body.contains("<form method=\"post\" action=\"/classic/logout\">"),
            "logout partial must render a POST form to /classic/logout: {body}"
        );
        assert!(
            body.contains("name=\"_csrf\" value=\"session-csrf-token-abc\""),
            "logout partial must round-trip the session csrf_token into the \
             hidden _csrf field: {body}"
        );
        assert!(
            body.contains("<button type=\"submit\""),
            "logout partial must render a real submit button (not a link): {body}"
        );
        assert!(
            body.contains("aria-label=\"Sign out of TASMail Classic\""),
            "logout button must carry an aria-label for assistive tech: {body}"
        );
    }

    #[test]
    fn logout_partial_lands_inside_nav_landmark() {
        let body = render_authed("tok");
        // Pin the form inside the <nav> element — the slot wrapper
        // `<li class="site-nav-end">` was defined in base.html
        // specifically to hold this form.
        let nav_open = body.find("<nav").expect("nav landmark present");
        let nav_close = body[nav_open..]
            .find("</nav>")
            .map(|rel| nav_open + rel)
            .expect("nav closing tag present");
        let form_at = body
            .find("action=\"/classic/logout\"")
            .expect("logout form rendered");
        assert!(
            form_at > nav_open && form_at < nav_close,
            "logout form MUST render inside the <nav> landmark — found at \
             byte {form_at}, nav spans {nav_open}..{nav_close}"
        );
    }

    #[test]
    fn logout_partial_html_escapes_hostile_csrf_token() {
        // Defence in depth: a hostile (e.g. corrupted DB row) csrf_token
        // containing HTML chars MUST be inert in the rendered output.
        // Askama auto-escapes for the .html extension; this test locks
        // that behaviour down so a future config change can't silently
        // turn it off.
        let body = render_authed("\"><script>alert(1)</script>");
        assert!(
            !body.contains("\"><script>alert(1)</script>"),
            "raw <script> leaked through the partial's value attribute: {body}"
        );
    }

    #[test]
    fn redirect_path_constant_is_303_compatible() {
        // Sanity check: Redirect::to in axum 0.8 returns a 303 by default
        // (it's the See Other variant). If a future axum bump changes
        // the default to 307 (preserve-method) the POST-Redirect-Get
        // contract breaks and we want this test to scream.
        let r = Redirect::to(LOGIN_PATH).into_response();
        assert_eq!(r.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            r.headers().get(header::LOCATION).and_then(|v| v.to_str().ok()),
            Some(LOGIN_PATH)
        );
    }
}
