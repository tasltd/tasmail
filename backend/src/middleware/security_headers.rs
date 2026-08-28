// Added: Security headers middleware for OWASP top-10 compliance (TMAIL-37)
// PURPOSE: Adds CSP, X-Frame-Options, X-Content-Type-Options, HSTS, and other
// security headers to all API responses.
//
// Changed (TMAIL-368): the middleware is now PATH-AWARE.
//   * `/classic/*` paths get a strict CSP locked down to the no-JS server-
//     rendered surface — `script-src 'none'`, `object-src 'none'`, nonce-only
//     inline styles, `frame-ancestors 'none'`, etc. The middleware generates a
//     fresh 256-bit nonce per request, parks it in `request.extensions_mut()`
//     so the classic handlers + templates render the matching
//     `<style nonce="…">` attribute, then bakes the same value into the
//     `style-src` source list on the response header. Other classic-specific
//     headers (`Referrer-Policy: same-origin`, `Permissions-Policy:
//     interest-cohort=()`) land on the same branch.
//   * Every other path keeps the existing SPA CSP from TMAIL-37 unchanged so
//     the React SPA / mobile API surface aren't tightened in a way that would
//     break legitimate inline boot scripts, image hosts, etc.

use axum::{
    extract::Request,
    http::{header::HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};

use crate::handlers::classic::CspNonce;

/// Path prefix the strict-CSP branch matches on. Centralised so the routing
/// layer and the middleware never disagree about which surface gets the
/// locked-down policy. The slash IS NOT included so both `/classic` (root
/// redirect) and `/classic/foo` (sub-routes) match.
const CLASSIC_PATH_PREFIX: &str = "/classic";

/// Axum middleware that appends security headers to every response.
///
/// On the request side, for `/classic/*` paths only, generates a fresh
/// `CspNonce` and inserts it into the request extensions so handlers can
/// extract it via the `axum::Extension<CspNonce>` extractor (or
/// `req.extensions().get::<CspNonce>()` for `Request`-taking handlers).
///
/// On the response side, sets the appropriate Content-Security-Policy
/// (strict for `/classic/*`, the existing SPA policy elsewhere) plus the
/// shared OWASP-recommended headers (X-Frame-Options, X-Content-Type-Options,
/// HSTS, X-XSS-Protection, Permissions-Policy, Referrer-Policy).
pub async fn security_headers_middleware(
    mut request: Request,
    next: Next,
) -> Response {
    // Decide branch BEFORE handing the request to `next.run` — once the
    // request moves we can't peek at the URI any more. The path-prefix check
    // is the same one the router uses to mount the classic sub-router, so
    // the two stay in lockstep.
    let is_classic = request.uri().path().starts_with(CLASSIC_PATH_PREFIX);

    // For /classic/*: pre-insert a per-request CspNonce so handlers + sub-
    // templates render the matching `<style nonce="…">` attribute. We keep
    // a borrowed copy of the encoded value for the header-building step
    // below — once the request moves into `next.run`, the inner extension
    // is no longer reachable from this scope.
    let classic_nonce: Option<String> = if is_classic {
        let nonce = CspNonce::new();
        let encoded = nonce.as_str().to_string();
        request.extensions_mut().insert(nonce);
        Some(encoded)
    } else {
        None
    };

    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    // Added: Prevent clickjacking — deny all framing. Same on both branches.
    headers.insert(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    );

    // Added: Prevent MIME-type sniffing attacks. Same on both branches.
    headers.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );

    if let Some(nonce_str) = classic_nonce {
        // Strict /classic/* CSP. See TMAIL-368 (driver: TMAIL-299, P0 #14).
        //
        // Why each directive:
        //   default-src 'self'          — block everything not explicitly allowed
        //   style-src 'self' 'nonce-X'  — only own CSS + the one nonced inline block
        //   img-src 'self' data: blob:  — own imgs + data/blob: for inline avatars,
        //                                 email previews; rich email HTML is
        //                                 sanitised elsewhere
        //   form-action 'self'          — forms can only POST back to our origin
        //   script-src 'none'           — no JS, period. The whole point of the
        //                                 classic surface
        //   object-src 'none'           — no <object>/<embed>/<applet>
        //   frame-ancestors 'none'      — overlaps X-Frame-Options: DENY for
        //                                 modern browsers; both ship per defence-
        //                                 in-depth
        //   base-uri 'self'             — block <base> tag rewrites of relative
        //                                 URLs to an attacker-controlled host
        let csp = format!(
            "default-src 'self'; \
             style-src 'self' 'nonce-{nonce}'; \
             img-src 'self' data: blob:; \
             form-action 'self'; \
             script-src 'none'; \
             object-src 'none'; \
             frame-ancestors 'none'; \
             base-uri 'self'",
            nonce = nonce_str
        );
        // HeaderValue::from_str can only fail on non-ASCII / control chars.
        // The nonce is base64 (ASCII alphanumeric + `+/=`) and every other
        // byte is a fixed ASCII literal, so this is unwrappable in practice.
        // Defensive fallback: drop the CSP if parsing somehow fails rather
        // than panicking on a customer page.
        if let Ok(value) = HeaderValue::from_str(&csp) {
            headers.insert(
                HeaderName::from_static("content-security-policy"),
                value,
            );
        }

        // Referrer-Policy: same-origin — the classic surface is no-JS so
        // outbound links go through `<a href>` only. Sending the path/query
        // to third-party origins would leak the user's mailbox state to ad
        // networks via the Referer header.
        headers.insert(
            HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("same-origin"),
        );

        // Permissions-Policy: interest-cohort=() — opt out of FLoC / Topics
        // API. Belt-and-braces because we already serve no-JS, but the
        // header is the documented way to signal opt-out to crawlers and
        // policy enforcers.
        headers.insert(
            HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static("interest-cohort=()"),
        );
    } else {
        // Existing SPA CSP — unchanged from TMAIL-37. Inline styles allowed
        // (TipTap / React-mounted style attrs), self-only script, etc.
        headers.insert(
            HeaderName::from_static("content-security-policy"),
            HeaderValue::from_static(
                // Changed: widened for Google Tag Manager (GTM-KLBBC27C).
                //
                // script-src keeps 'self' and deliberately does NOT gain
                // 'unsafe-inline' — that would re-open the whole SPA to any
                // injected inline script. Instead the ONE inline GTM loader in
                // index.html is allowlisted by the sha256 of its exact body:
                //
                //   sha256-SXc2wpeV9E0mm/lmfMA8bUtOWkrkYb6zlCTQ/JtrjX0=
                //
                // That hash is over the bytes BETWEEN <script> and </script>.
                // Re-edit the snippet — even one byte of whitespace — and the
                // hash no longer matches and GTM silently stops loading.
                // Recompute with:
                //   python3 -c "import hashlib,base64,sys;print(base64.b64encode(hashlib.sha256(open(sys.argv[1],'rb').read()).digest()).decode())" snippet.txt
                //
                // frame-src allows the <noscript> ns.html iframe fallback.
                // Note the /classic/* branch above is untouched: it keeps
                // script-src 'none' because it is the deliberate no-JS surface.
                "default-src 'self'; \
                 script-src 'self' 'sha256-SXc2wpeV9E0mm/lmfMA8bUtOWkrkYb6zlCTQ/JtrjX0=' https://www.googletagmanager.com; \
                 style-src 'self' 'unsafe-inline'; \
                 img-src 'self' https: data:; \
                 font-src 'self'; \
                 frame-src https://www.googletagmanager.com; \
                 connect-src 'self' https://www.google-analytics.com https://analytics.google.com https://*.analytics.google.com https://*.google-analytics.com https://www.googletagmanager.com; \
                 frame-ancestors 'none'; base-uri 'self'; form-action 'self'"
            ),
        );

        // Existing SPA Referrer-Policy — strict-origin-when-cross-origin
        // strikes a middle ground (full URL on same-origin, origin-only
        // cross-origin, nothing on protocol downgrade).
        headers.insert(
            HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        );

        // Existing SPA Permissions-Policy — disable features the app doesn't
        // need. Stays distinct from the classic branch's `interest-cohort=()`
        // payload because the SPA already locks down camera/mic/geolocation.
        headers.insert(
            HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static("camera=(), microphone=(), geolocation=(), payment=()"),
        );
    }

    // Added: HSTS — enforce HTTPS for 1 year with subdomains. Same on both
    // branches; the classic surface is reverse-proxied via the same Apache
    // vhost as the SPA so it inherits the TLS termination either way.
    headers.insert(
        HeaderName::from_static("strict-transport-security"),
        HeaderValue::from_static("max-age=31536000; includeSubDomains"),
    );

    // Added: XSS protection for legacy browsers. Modern browsers ignore this
    // header in favour of CSP, but the audit checklist still flags its
    // absence — kept on both branches.
    headers.insert(
        HeaderName::from_static("x-xss-protection"),
        HeaderValue::from_static("1; mode=block"),
    );

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        extract::Extension,
        http::StatusCode,
        middleware,
        routing::get,
        Router,
    };
    use tower::ServiceExt;

    async fn ok_handler() -> &'static str {
        "ok"
    }

    /// Classic-path handler that pulls the per-request CspNonce out of
    /// request extensions and echoes its base64 value in the body so the
    /// header-vs-body match test can assert the two agree.
    async fn classic_nonce_echo(Extension(nonce): Extension<CspNonce>) -> String {
        nonce.into_string()
    }

    fn test_app() -> Router {
        Router::new()
            .route("/test", get(ok_handler))
            .route("/classic/echo", get(classic_nonce_echo))
            .route("/classic/sub/path", get(ok_handler))
            .layer(middleware::from_fn(security_headers_middleware))
    }

    // ----- Shared headers (both branches) -----

    #[tokio::test]
    async fn test_x_frame_options_header() {
        let app = test_app();
        let req = axum::http::Request::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers().get("x-frame-options").unwrap(),
            "DENY"
        );
    }

    #[tokio::test]
    async fn test_x_content_type_options_header() {
        let app = test_app();
        let req = axum::http::Request::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(
            resp.headers().get("x-content-type-options").unwrap(),
            "nosniff"
        );
    }

    #[tokio::test]
    async fn test_hsts_header_present() {
        let app = test_app();
        let req = axum::http::Request::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let hsts = resp
            .headers()
            .get("strict-transport-security")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(hsts.contains("max-age=31536000"));
        assert!(hsts.contains("includeSubDomains"));
    }

    #[tokio::test]
    async fn test_xss_protection_header() {
        let app = test_app();
        let req = axum::http::Request::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(
            resp.headers().get("x-xss-protection").unwrap(),
            "1; mode=block"
        );
    }

    // ----- SPA-branch CSP (non-classic paths) -----

    #[tokio::test]
    async fn test_spa_csp_header_present() {
        let app = test_app();
        let req = axum::http::Request::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let csp = resp
            .headers()
            .get("content-security-policy")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(csp.contains("default-src 'self'"));
        assert!(csp.contains("frame-ancestors 'none'"));
        // SPA branch keeps inline styles (React/TipTap) — the strict
        // /classic/* branch is the only one that removes 'unsafe-inline'.
        assert!(
            csp.contains("style-src 'self' 'unsafe-inline'"),
            "SPA CSP must keep 'unsafe-inline' for React inline styles: {csp}"
        );
    }

    #[tokio::test]
    async fn test_spa_referrer_policy_header() {
        let app = test_app();
        let req = axum::http::Request::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(
            resp.headers().get("referrer-policy").unwrap(),
            "strict-origin-when-cross-origin"
        );
    }

    #[tokio::test]
    async fn test_spa_permissions_policy_header() {
        let app = test_app();
        let req = axum::http::Request::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let pp = resp
            .headers()
            .get("permissions-policy")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(pp.contains("camera=()"));
        assert!(pp.contains("microphone=()"));
    }

    // ----- Classic-branch strict CSP (TMAIL-368) -----

    #[tokio::test]
    async fn test_classic_csp_has_strict_directives() {
        // Sampled response on /classic/sub/path — the strict CSP must
        // include every directive called out in the TMAIL-368 spec:
        //   default-src 'self'
        //   style-src 'self' 'nonce-XXX'
        //   img-src 'self' data: blob:
        //   form-action 'self'
        //   script-src 'none'
        //   object-src 'none'
        //   frame-ancestors 'none'
        //   base-uri 'self'
        let app = test_app();
        let req = axum::http::Request::builder()
            .uri("/classic/sub/path")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let csp = resp
            .headers()
            .get("content-security-policy")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(csp.contains("default-src 'self'"), "missing default-src: {csp}");
        assert!(
            csp.contains("img-src 'self' data: blob:"),
            "missing img-src: {csp}"
        );
        assert!(csp.contains("form-action 'self'"), "missing form-action: {csp}");
        assert!(csp.contains("script-src 'none'"), "missing script-src 'none': {csp}");
        assert!(csp.contains("object-src 'none'"), "missing object-src 'none': {csp}");
        assert!(
            csp.contains("frame-ancestors 'none'"),
            "missing frame-ancestors 'none': {csp}"
        );
        assert!(csp.contains("base-uri 'self'"), "missing base-uri 'self': {csp}");
    }

    #[tokio::test]
    async fn test_classic_csp_has_nonce_in_style_src() {
        // The CSP must carry a 'nonce-XXX' source on style-src so the
        // inline <style nonce="…"> block on base.html is admitted.
        let app = test_app();
        let req = axum::http::Request::builder()
            .uri("/classic/sub/path")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let csp = resp
            .headers()
            .get("content-security-policy")
            .unwrap()
            .to_str()
            .unwrap();
        // Looks like:
        //   style-src 'self' 'nonce-Ab12+/=='
        assert!(
            csp.contains("style-src 'self' 'nonce-"),
            "style-src must carry a 'nonce-…' source: {csp}"
        );
    }

    #[tokio::test]
    async fn test_classic_csp_does_not_allow_unsafe_inline_styles() {
        // The whole point of the nonce branch is to forbid blanket
        // 'unsafe-inline'. A regression that re-added it would silently
        // re-enable inline-style XSS on the classic surface.
        let app = test_app();
        let req = axum::http::Request::builder()
            .uri("/classic/sub/path")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let csp = resp
            .headers()
            .get("content-security-policy")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(
            !csp.contains("'unsafe-inline'"),
            "/classic/* CSP must NOT permit 'unsafe-inline': {csp}"
        );
        assert!(
            !csp.contains("'unsafe-eval'"),
            "/classic/* CSP must NOT permit 'unsafe-eval': {csp}"
        );
    }

    #[tokio::test]
    async fn test_classic_csp_nonce_matches_request_extension() {
        // End-to-end: the nonce baked into the response CSP header MUST be
        // the same value the handler saw in request extensions. If these
        // diverge, the inline <style nonce="…"> on every page renders unstyled.
        let app = test_app();
        let req = axum::http::Request::builder()
            .uri("/classic/echo")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let csp = resp
            .headers()
            .get("content-security-policy")
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let body_bytes = axum::body::to_bytes(resp.into_body(), 8 * 1024)
            .await
            .unwrap();
        let nonce_seen_by_handler = String::from_utf8(body_bytes.to_vec()).unwrap();
        assert!(
            csp.contains(&format!("'nonce-{}'", nonce_seen_by_handler)),
            "CSP header nonce ({}) doesn't match the handler-side nonce ({}): {csp}",
            csp,
            nonce_seen_by_handler
        );
        // Also confirm the nonce on the wire really is 44 base64 chars,
        // i.e. the 32-byte spec value — guards against an accidental
        // truncation in the format!() string.
        assert_eq!(
            nonce_seen_by_handler.len(),
            44,
            "nonce should be 44 chars (32 bytes base64): {nonce_seen_by_handler}"
        );
    }

    #[tokio::test]
    async fn test_classic_referrer_policy_is_same_origin() {
        // Distinct from the SPA branch's strict-origin-when-cross-origin —
        // the classic surface clamps it tighter to same-origin so outbound
        // links never leak path/query to third parties.
        let app = test_app();
        let req = axum::http::Request::builder()
            .uri("/classic/sub/path")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(
            resp.headers().get("referrer-policy").unwrap(),
            "same-origin",
            "/classic/* must use Referrer-Policy: same-origin per TMAIL-368"
        );
    }

    #[tokio::test]
    async fn test_classic_permissions_policy_blocks_interest_cohort() {
        let app = test_app();
        let req = axum::http::Request::builder()
            .uri("/classic/sub/path")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(
            resp.headers().get("permissions-policy").unwrap(),
            "interest-cohort=()",
            "/classic/* must opt out of FLoC/Topics via Permissions-Policy"
        );
    }

    #[tokio::test]
    async fn test_classic_x_frame_options_deny() {
        // X-Frame-Options: DENY is shared, but the spec calls it out
        // explicitly for /classic/* so guard the value on this branch too.
        let app = test_app();
        let req = axum::http::Request::builder()
            .uri("/classic/sub/path")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(
            resp.headers().get("x-frame-options").unwrap(),
            "DENY",
            "/classic/* must ship X-Frame-Options: DENY"
        );
    }

    #[tokio::test]
    async fn test_classic_nosniff() {
        let app = test_app();
        let req = axum::http::Request::builder()
            .uri("/classic/sub/path")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(
            resp.headers().get("x-content-type-options").unwrap(),
            "nosniff",
            "/classic/* must ship X-Content-Type-Options: nosniff"
        );
    }

    #[tokio::test]
    async fn test_classic_root_path_also_gets_strict_csp() {
        // `/classic` (no trailing slash) is the index redirect — confirm
        // the prefix match catches it too, not just `/classic/...`.
        let app = test_app();
        let req = axum::http::Request::builder()
            .uri("/classic/echo")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let csp = resp
            .headers()
            .get("content-security-policy")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(
            csp.contains("script-src 'none'"),
            "/classic prefix must trigger strict CSP: {csp}"
        );
    }
}
