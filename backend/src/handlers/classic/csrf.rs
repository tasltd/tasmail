// Added (TMAIL-358): CSRF rejection page template + render helpers.
//
// PURPOSE
// -------
// When `middleware::classic_csrf::classic_csrf_middleware` rejects a state-
// changing request (POST/PUT/PATCH/DELETE) because the `_csrf` form field
// is missing, mismatched, or the session is gone, it needs to render a 403
// HTML page — NOT the JSON `AppError` envelope used by `/api/*`. This
// module owns that template struct and the render helper.
//
// The helpers are public-in-crate so `middleware::classic_csrf` (which
// can't itself depend on the template path) and any future handler that
// wants to do a one-off CSRF rejection (e.g. login form before a session
// row exists) share one rendering path.
//
// The field name `_csrf` is canonical and centralised here so every Classic
// UI form template imports the same constant — a rename in one place fans
// out without a search-and-replace.

use askama::Template;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};

/// Form field name used by every Classic UI `<form>` to carry the
/// per-session CSRF token. Centralised so a future rename touches one
/// constant, not eight templates.
pub const CSRF_FIELD_NAME: &str = "_csrf";

/// Askama template struct backing `templates/classic/csrf_error.html`.
///
/// Field names match the template variables exactly — Askama validates this
/// at compile time so a rename here without a matching template edit fails
/// `cargo build` rather than at runtime on a customer page.
#[derive(Template)]
#[template(path = "classic/csrf_error.html")]
pub struct CsrfErrorTemplate {
    /// Short human-readable reason for the rejection. Rendered inside the
    /// `.alert-error` block so testers / curl operators can self-diagnose.
    /// MUST be safe text — Askama auto-escapes for the `.html` extension,
    /// so HTML chars submitted by a hostile client become entities.
    pub reason: String,
    /// The original request URI path so the page can offer a "Reload <path>"
    /// link. Passed through untrusted from `req.uri().path()`; Askama
    /// escaping keeps it inert in the rendered output.
    pub retry_path: String,
    /// Per-request CSP nonce, same shape as every other Classic UI template.
    /// Required by `base.html` (TMAIL-356) — without it the inline `<style>`
    /// gets blocked by the strict CSP planned in TMAIL-368 and the page
    /// renders unstyled.
    pub csp_nonce: String,
}

impl CsrfErrorTemplate {
    /// Build the struct with an explicitly-supplied CSP nonce.
    ///
    /// Changed (TMAIL-368): takes the nonce as a parameter rather than
    /// generating one internally so the value matches the per-request nonce
    /// `security_headers_middleware` baked into the response CSP header.
    /// Callers pull it from `req.extensions().get::<CspNonce>()` and pass
    /// the encoded string in.
    pub fn new(
        reason: impl Into<String>,
        retry_path: impl Into<String>,
        csp_nonce: impl Into<String>,
    ) -> Self {
        Self {
            reason: reason.into(),
            retry_path: retry_path.into(),
            csp_nonce: csp_nonce.into(),
        }
    }
}

/// Render the CSRF rejection page as a complete 403 HTTP response.
///
/// Always returns 403 Forbidden with `Content-Type: text/html; charset=utf-8`
/// (axum's `Html` newtype sets the header). If the template itself fails to
/// render — which would mean a build-time invariant broke — falls back to a
/// plain-text 403 so the user still sees *something* rather than the JSON
/// `AppError` envelope.
///
/// Changed (TMAIL-368): now takes the per-request `csp_nonce` so the inline
/// `<style nonce="…">` on base.html matches the response CSP header that the
/// security_headers middleware also sets. The classic_csrf_middleware pulls
/// the nonce out of request extensions and threads it in.
pub fn render_csrf_error_response(
    reason: impl Into<String>,
    retry_path: impl Into<String>,
    csp_nonce: impl Into<String>,
) -> Response {
    let tpl = CsrfErrorTemplate::new(reason, retry_path, csp_nonce);
    match tpl.render() {
        Ok(body) => (StatusCode::FORBIDDEN, Html(body)).into_response(),
        Err(e) => {
            tracing::error!(error = ?e, "classic CSRF error template render failed");
            (
                StatusCode::FORBIDDEN,
                [(axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "403 Forbidden — CSRF token rejected. Please reload the form and try again.",
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csrf_field_name_is_underscore_csrf() {
        // Centralised — every template + every test pulls this constant. A
        // rename here without a matching template edit is a contract break.
        assert_eq!(CSRF_FIELD_NAME, "_csrf");
    }

    fn render_with(reason: &str, retry_path: &str) -> String {
        // Build via the explicit struct literal so this helper stays one
        // line away from showing the fixed test nonce — readers don't have
        // to chase the `CsrfErrorTemplate::new` API to understand what's
        // being asserted on.
        CsrfErrorTemplate {
            reason: reason.to_string(),
            retry_path: retry_path.to_string(),
            csp_nonce: "test-nonce-fixed-for-assertions".to_string(),
        }
        .render()
        .expect("template should render")
    }

    #[test]
    fn renders_reason_and_retry_link() {
        let body = render_with("Missing CSRF token", "/classic/compose");
        // The error sits in the alert block so a screen reader announces it
        // first via role="alert".
        assert!(body.contains("role=\"alert\""), "alert role missing: {body}");
        assert!(body.contains("Missing CSRF token"), "reason missing: {body}");
        // Retry link uses the original path, not a hard-coded fallback.
        assert!(
            body.contains("href=\"/classic/compose\""),
            "retry-link to original path missing: {body}"
        );
        // Fallback link back to /classic/ always present so a user with a
        // genuinely broken page still has an out.
        assert!(
            body.contains("href=\"/classic/\""),
            "/classic/ fallback link missing: {body}"
        );
    }

    #[test]
    fn extends_base_layout_with_skip_link_and_nav() {
        // CSRF page must NOT bypass the accessible base layout — a user
        // who lands here through a stale form still needs the skip-link and
        // navigation chrome.
        let body = render_with("x", "/classic/x");
        assert!(body.contains("class=\"skip-link\""), "skip-link missing");
        assert!(body.contains("<nav"), "<nav> landmark missing");
        assert!(body.contains("<main id=\"main\""), "<main> landmark missing");
    }

    #[test]
    fn auto_escapes_hostile_reason() {
        // Defence in depth: a hostile client controls neither `reason` nor
        // `retry_path` directly, but if a future middleware ever surfaces
        // raw header content into the reason, Askama's `.html`-extension
        // auto-escaping must keep `<script>` inert.
        let body = render_with("<script>alert(1)</script>", "/classic/x");
        assert!(
            !body.contains("<script>alert(1)</script>"),
            "raw <script> leaked into output — auto-escape broken"
        );
        assert!(
            body.contains("&#60;script&#62;") || body.contains("&lt;script&gt;"),
            "expected escaped <script> tag in output: {body}"
        );
    }

    #[test]
    fn auto_escapes_hostile_retry_path() {
        let body = render_with("x", "/classic/\"><script>1</script>");
        assert!(
            !body.contains("\"><script>1</script>"),
            "raw retry_path leaked unescaped into href attribute"
        );
    }

    #[test]
    fn new_constructor_stores_passed_in_nonce_verbatim() {
        // Changed (TMAIL-368): the constructor no longer rolls its own
        // nonce — it stores whatever the caller passed in. The middleware
        // is the source of truth for the per-request value now. This test
        // pins the new contract: same input → same nonce, no surprise
        // randomness on the way through.
        let a = CsrfErrorTemplate::new("r", "/x", "nonce-aaa");
        let b = CsrfErrorTemplate::new("r", "/x", "nonce-bbb");
        assert_eq!(a.csp_nonce, "nonce-aaa");
        assert_eq!(b.csp_nonce, "nonce-bbb");
    }
}
