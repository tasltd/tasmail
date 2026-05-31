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
    middleware as axum_middleware,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Router,
};

// PURPOSE: explicit MethodRouter import lets the catch-all route accept any
// method by calling `.fallback(...)` on it. Currently we only wire GET on
// `/classic/{*rest}`; non-GET methods will return 405 from axum which is the
// right behaviour until child tasks need POST/PUT routes under unknown paths.

use crate::error::AppError;
use crate::state::AppState;

// Added (TMAIL-356): Per-request CSP nonce for the inline `<style>` block in
// the Classic UI base layout. Public so child handlers (login, folder, message,
// compose) can call `CspNonce::new()` when building their own template structs.
pub mod nonce;
pub use nonce::CspNonce;

// Added (TMAIL-357): Skeleton auth helpers — generate_csrf_token,
// create_session_and_cookie, destroy_session_and_cookie. The actual route
// handlers for /classic/login + /classic/logout land in TMAIL-359 / TMAIL-360
// and will register themselves in the router via this module.
pub mod auth;

// Added (TMAIL-358): CSRF rejection page template + render helpers shared
// by `middleware::classic_csrf` and any handler that needs a one-off CSRF
// rejection (e.g. login pre-session). The `_csrf` form-field constant lives
// here too so every Classic UI template imports one source of truth.
pub mod csrf;
pub use csrf::{render_csrf_error_response, CsrfErrorTemplate, CSRF_FIELD_NAME};

// Added (TMAIL-359): GET + POST /classic/login handlers and the login form
// template struct. Uses the pre-session double-submit-cookie CSRF pattern
// since the canonical CSRF middleware needs a ClassicSession that doesn't
// exist before login completes.
pub mod login;

// Added (TMAIL-360): POST /classic/logout handler. Mounts on the
// authenticated sub-router below (so it inherits classic_session_middleware
// + classic_csrf_middleware automatically) — never as a GET route, since
// that would let an attacker sign the user out via an `<img src=...>` tag
// in a hostile email or pre-fetched link.
pub mod logout;

// NAME: Session cookie name shared with the Classic UI auth handler that
// lands in TMAIL-357 (P0 #3). The scaffold only needs to *detect* presence;
// the cookie value is opaque here and validated later by the dedicated
// classic_session middleware. Centralising the name here keeps the
// child-task implementation pointed at one source of truth.
pub const CLASSIC_SESSION_COOKIE: &str = "tasmail_classic_sid";

/// 404 template for any path under `/classic/*` that doesn't match a route.
/// Kept in this module (not a standalone `errors` module) so the scaffold
/// stays self-contained — error pages get split out in P2 #45.
///
/// `csp_nonce` carries the per-request value injected into the inline
/// `<style nonce="…">` block on base.html. Askama's template inheritance
/// resolves fields against the child struct, so every template that
/// extends base.html must carry this field (TMAIL-356).
#[derive(Template)]
#[template(path = "classic/not_found.html")]
struct NotFoundTemplate {
    path: String,
    csp_nonce: String,
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
///
/// Two sub-routers are merged here:
///   * Public routes (login GET/POST, index redirect, 404 catch-all) — no
///     middleware, since the user has no session yet.
///   * Authenticated routes (logout, future inbox/compose/settings...) —
///     wrapped in `authenticated_router(state)` which stacks
///     `classic_session_middleware` (cookie → ClassicSession) followed by
///     `classic_csrf_middleware` (validates _csrf form field against the
///     row's csrf_token). Both layers MUST be wired together — the CSRF
///     middleware depends on the session middleware having injected the
///     `ClassicSession` into request extensions.
pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/classic", get(index_redirect))
        .route("/classic/", get(index_redirect))
        // Added (TMAIL-359): Public login routes — must sit ABOVE the
        // catch-all and BELOW any authenticated routes (none yet). These
        // are intentionally NOT behind classic_session_middleware (the
        // user has no session yet) nor classic_csrf_middleware (it needs
        // a ClassicSession); the POST handler does its own double-submit-
        // cookie CSRF check.
        .route("/classic/login", get(login::get_login).post(login::post_login))
        // Added (TMAIL-360): authenticated sub-router for state-changing
        // POST endpoints that need a verified session AND CSRF protection.
        // Logout is the first inhabitant; inbox/compose/settings join in
        // follow-up tasks under driver TMAIL-299.
        .merge(authenticated_router(state))
        // PURPOSE: explicit catch-all so the 404 page only fires for paths
        // under `/classic/...`. The `{*rest}` wildcard captures any remaining
        // path segments — child tasks (login, folder, message, compose) will
        // add specific routes ABOVE this one and axum's most-specific-match
        // semantics route correctly.
        .route("/classic/{*rest}", get(not_found))
}

/// Authenticated `/classic/*` sub-router. Every route mounted here inherits:
///
///   1. `classic_session_middleware` — validates the `tasmail_classic_sid`
///      cookie's HMAC signature, resolves it to a live `classic_sessions`
///      row, loads the owning `Mailbox`, and injects both the row and a
///      `Claims` struct into request extensions. Bounces to `/classic/login`
///      on any failure.
///   2. `classic_csrf_middleware` — for state-changing methods (POST / PUT /
///      PATCH / DELETE), pulls the `_csrf` form field out of the body and
///      constant-time-compares it against the row's `csrf_token`. Renders
///      the 403 HTML retry page on mismatch.
///
/// Layer order matters: tower layers run bottom-up on the request, so we
/// add the CSRF middleware FIRST (inner) and the session middleware SECOND
/// (outer/runs first). That guarantees the session is in extensions before
/// the CSRF middleware needs to read it — the same pattern `auth_middleware`
/// → `rls_context_middleware` uses on the `/api` side.
fn authenticated_router(state: AppState) -> Router<AppState> {
    Router::new()
        // Added (TMAIL-360): logout is intentionally POST-only — see the
        // module-level comment on `handlers::classic::logout` for the CSRF
        // rationale. The route 405s on GET, which is the right behaviour
        // (no GET handler means an `<img src="/classic/logout">` exploit
        // can't even kick off the chain).
        .route("/classic/logout", post(logout::post_logout))
        // Inner layer: CSRF check on state-changing methods. Runs AFTER
        // the session middleware on the request side, so the session row
        // (carrying the expected token) is in extensions when it executes.
        .layer(axum_middleware::from_fn(
            crate::middleware::classic_csrf::classic_csrf_middleware,
        ))
        // Outer layer: cookie → session resolution. Bounces to login on
        // any failure so downstream layers + handlers never have to
        // worry about an unauthenticated request.
        .layer(axum_middleware::from_fn_with_state(
            state,
            crate::middleware::classic_session::classic_session_middleware,
        ))
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
    let tpl = NotFoundTemplate {
        path,
        // Added (TMAIL-356): Fresh CSP nonce per response so the inline
        // <style> on base.html survives the `style-src 'self' 'nonce-XXX'`
        // header that TMAIL-368 will ship. Today the header isn't set yet;
        // the nonce attribute is harmless in its absence and ready when it
        // lands.
        csp_nonce: CspNonce::new().into_string(),
    };
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

    /// Tiny constructor used by every layout test below — keeps the field
    /// list in one place so adding a new base-layout field (e.g. csrf_token
    /// in TMAIL-357) only needs an edit here, not in 8 separate tests.
    fn render_404(path: &str) -> String {
        let tpl = NotFoundTemplate {
            path: path.to_string(),
            csp_nonce: "test-nonce-fixed-for-assertions".to_string(),
        };
        tpl.render().expect("template should render")
    }

    #[test]
    fn not_found_template_renders() {
        let body = render_404("/classic/does-not-exist");
        assert!(body.contains("Page not found"));
        assert!(body.contains("/classic/does-not-exist"));
        assert!(body.contains("<a href=\"/classic/\""));
    }

    #[test]
    fn not_found_template_escapes_path() {
        // PURPOSE: defence-in-depth — Askama escapes by default for the .html
        // extension, but lock the behaviour in with a test so a future config
        // change can't silently turn it off.
        let body = render_404("/classic/<script>alert(1)</script>");
        assert!(!body.contains("<script>alert(1)</script>"));
        assert!(body.contains("&#60;script&#62;") || body.contains("&lt;script&gt;"));
    }

    // ----- TMAIL-356: base.html layout assertions -----
    //
    // These tests are colocated with the handler module (rather than the
    // base.html-only test file) so they catch regressions whenever ANY child
    // template extends base.html. They render through NotFoundTemplate because
    // that's the only Classic-UI template that exists in the tree today; once
    // login/folder/message templates land they'll exercise the same base layout
    // and don't need to duplicate these assertions.

    #[test]
    fn base_layout_declares_html5_lang_and_viewport() {
        // WCAG 3.1.1 (Language of Page) + responsive baseline.
        let body = render_404("/classic/x");
        assert!(body.starts_with("<!DOCTYPE html>"), "missing HTML5 doctype");
        assert!(body.contains("<html lang=\"en\">"), "missing <html lang>");
        assert!(
            body.contains("<meta name=\"viewport\""),
            "missing viewport meta — mobile rendering will be broken"
        );
        assert!(
            body.contains("width=device-width") && body.contains("initial-scale=1"),
            "viewport meta must declare width=device-width, initial-scale=1"
        );
    }

    #[test]
    fn base_layout_has_semantic_landmarks() {
        // WCAG 1.3.1 (Info and Relationships) + 2.4.1 (Bypass Blocks).
        let body = render_404("/classic/x");
        assert!(body.contains("<header"), "missing <header> landmark");
        assert!(body.contains("<nav"), "missing <nav> landmark");
        assert!(
            body.contains("<main id=\"main\""),
            "missing <main id=\"main\"> — skip-link target"
        );
        assert!(body.contains("<footer"), "missing <footer> landmark");
    }

    #[test]
    fn skip_link_is_first_focusable_element_inside_body() {
        // WCAG 2.4.1 — skip-to-main MUST be the very first focusable element
        // in body so a keyboard user lands on it on the first Tab press. Any
        // earlier interactive element (<a>, <button>, <input>, <select>,
        // <textarea>) would steal that first Tab.
        let body = render_404("/classic/x");
        let body_start = body
            .find("<body>")
            .expect("rendered HTML must contain <body>");
        let after_body = &body[body_start + "<body>".len()..];

        // Locate the skip-link by its distinctive class — not by `href="#main"`
        // which sits AFTER `<a ` in the same opening tag.
        let skip_class_at = after_body
            .find("class=\"skip-link\"")
            .expect("skip-link element not found inside <body>");
        let skip_link_tag_at = after_body[..skip_class_at]
            .rfind("<a ")
            .expect("class=\"skip-link\" must sit on an <a> tag");

        // Anything focusable before the skip-link tag fails WCAG 2.4.1.
        let before_skip = &after_body[..skip_link_tag_at];
        for needle in ["<a ", "<button", "<input", "<select", "<textarea"] {
            assert!(
                !before_skip.contains(needle),
                "found interactive element `{needle}` BEFORE the skip-link — \
                 skip-link must be the first focusable element per WCAG 2.4.1.\n\
                 Body prefix:\n{before_skip}"
            );
        }
    }

    #[test]
    fn inline_style_carries_csp_nonce() {
        // The reason for this whole task — without the nonce attribute on
        // <style>, the CSP planned in TMAIL-368 (`style-src 'self' 'nonce-XXX'`)
        // would block every CSS rule and the page would render unstyled.
        let body = render_404("/classic/x");
        assert!(
            body.contains("<style nonce=\"test-nonce-fixed-for-assertions\">"),
            "inline <style> must carry the per-request CSP nonce"
        );
    }

    #[test]
    fn base_layout_has_zero_script_tags() {
        // Hard rule per the gap analysis: the Classic UI is a no-JS surface.
        // A stray <script> tag would defeat the point and (with TMAIL-368's
        // `script-src 'none'`) would be CSP-blocked anyway. Lock it down.
        let body = render_404("/classic/x");
        assert!(
            !body.contains("<script"),
            "Classic UI base layout must contain ZERO <script> tags; found one in:\n{body}"
        );
    }

    #[test]
    fn base_layout_uses_brand_palette_tokens() {
        // BRAND.md is the source-of-truth palette. Lock the four primary
        // tokens into the rendered CSS so a future drive-by edit can't
        // quietly walk the palette off-brand.
        let body = render_404("/classic/x");
        for token in [
            "--tm-blue-600",
            "--tm-teal-400",
            "--tm-charcoal-900",
            "--tm-charcoal-700",
        ] {
            assert!(
                body.contains(token),
                "expected BRAND.md palette token {token} in rendered CSS"
            );
        }
        // Also: the @-glyph in the wordmark MUST be the teal token per
        // BRAND.md ("never use the teal for text") — used as decorative
        // colour only.
        assert!(
            body.contains("class=\"brand-at\""),
            "wordmark @-glyph must carry the brand-at class so it picks up \
             the teal token from the inline stylesheet"
        );
    }

    // ----- TMAIL-360: logout_form block contract -----
    //
    // base.html declares `{% block logout_form %}{% endblock %}` so
    // authenticated child templates (inbox, compose, settings...) can fill
    // it without touching base.html. The block sits inside the primary nav
    // so the logout button travels every page that extends base.html.
    // These tests lock down two invariants:
    //   1. Unauthenticated pages (login, csrf_error, 404) MUST NOT render
    //      a logout form — they don't override the block, so it stays empty.
    //   2. The nav structure includes the slot at the right position so a
    //      future override actually places the form inside the nav, not
    //      adrift in the page.

    #[test]
    fn base_layout_renders_no_logout_form_when_block_not_overridden() {
        // NotFoundTemplate (and login + csrf_error) don't override
        // logout_form. The rendered output MUST therefore contain no
        // POST form pointing at /classic/logout. A regression here would
        // mean an unauthenticated user sees a Sign-out button on the
        // login page, which is at best confusing and at worst a way to
        // spam invalid logout submissions.
        let body = render_404("/classic/x");
        assert!(
            !body.contains("action=\"/classic/logout\""),
            "unauthenticated template MUST NOT render a logout form: {body}"
        );
        assert!(
            !body.contains(">Sign out<"),
            "unauthenticated template MUST NOT render a Sign-out button: {body}"
        );
    }

    #[test]
    fn base_layout_nav_carries_a_logout_form_slot() {
        // The slot itself is invisible in the rendered HTML (Askama
        // template blocks compile to nothing when not overridden), but
        // the CSS class `site-nav-end` we attached to the surrounding
        // <li> MUST be present so the future override lands inside the
        // nav at the right position. If a refactor accidentally drops
        // the <li class="site-nav-end">, child templates will start
        // rendering the form outside the nav landmark.
        let body = render_404("/classic/x");
        assert!(
            body.contains("class=\"site-nav-end\""),
            "logout-form slot wrapper <li class=\"site-nav-end\"> missing \
             from base.html nav: {body}"
        );
        // Pin the slot inside the nav element, not floating outside it.
        let nav_open = body.find("<nav").expect("nav landmark present");
        let nav_close = body[nav_open..]
            .find("</nav>")
            .map(|rel| nav_open + rel)
            .expect("nav closing tag present");
        let slot_at = body
            .find("class=\"site-nav-end\"")
            .expect("slot wrapper present");
        assert!(
            slot_at > nav_open && slot_at < nav_close,
            "logout-form slot must sit INSIDE the <nav> element, not outside it"
        );
    }

    #[test]
    fn base_layout_styles_include_logout_button_rules() {
        // The Sign-out button uses `.site-nav-end button` to match the
        // visual weight of nav links (not a primary action). If the
        // styles get dropped, the rendered button looks like a primary
        // CTA on every page, which over-weights a destructive action.
        let body = render_404("/classic/x");
        assert!(
            body.contains(".site-nav-end button"),
            "base layout must declare the .site-nav-end button styling so \
             the logout form blends with surrounding nav links: {body}"
        );
    }

    #[test]
    fn base_layout_has_print_stylesheet() {
        // Print-friendly per the spec. Asserting the @media print block is
        // present is the minimum signal; future visual regression tests can
        // do PDF-rendering comparisons.
        let body = render_404("/classic/x");
        assert!(
            body.contains("@media print"),
            "base layout must declare a print stylesheet (hide chrome, monochrome)"
        );
    }

    #[test]
    fn nonce_attribute_value_is_html_escaped() {
        // Defence-in-depth: if a future refactor accidentally lets a nonce
        // containing `"` reach the template, Askama's auto-escaping must
        // catch it. The base64 alphabet doesn't include `"`, so this is a
        // safety net rather than a real risk — but a test costs nothing.
        let tpl = NotFoundTemplate {
            path: "/x".to_string(),
            csp_nonce: "not-real-but-has\"quote".to_string(),
        };
        let body = tpl.render().expect("template should render");
        assert!(
            !body.contains("not-real-but-has\"quote"),
            "raw nonce with unescaped `\"` leaked into output — auto-escape \
             must be on for the .html extension"
        );
        assert!(
            body.contains("not-real-but-has&quot;quote")
                || body.contains("not-real-but-has&#34;quote"),
            "nonce should be HTML-attribute-escaped"
        );
    }
}
