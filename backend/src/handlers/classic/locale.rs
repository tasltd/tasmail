// Added (TMAIL-387): POST handler for the footer language picker.
//
// Lives on the PUBLIC sub-router (not the authenticated one) so the picker
// in `base.html` works on every page — including the sign-in form, the
// signup wizard, the CSRF rejection page, and the 404 page. Anonymous
// users get to pick their language too, and the cookie carries forward
// into their first authenticated session.
//
// CSRF strategy
// -------------
// Locale switching has no security impact — the worst an attacker can do
// via a forged POST is annoy the user by changing their UI language. We
// deliberately do NOT require a CSRF token here because:
//
//   1. The endpoint is reachable from anonymous pages (login, signup,
//      CSRF rejection) where no session-scoped token exists.
//   2. Adding a pre-session double-submit cookie just for this purpose
//      (the way `/classic/login` does) would be five times more code
//      than the handler itself.
//   3. The locale cookie's `SameSite=Lax` attribute already blocks
//      cross-origin form submissions in modern browsers.
//
// Form payload
// ------------
// The footer renders a `<form method="post" action="/classic/locale">`
// containing:
//   * `<select name="locale">` with one `<option value="<code>">`
//     per supported locale.
//   * Optional `<input type="hidden" name="return_to" value="…">` that
//     the handler validates and uses for the 303 redirect target. When
//     absent / unsafe we redirect to `/classic/folders/INBOX` (the
//     authenticated default landing) which itself bounces to
//     `/classic/login` for anonymous users.
//
// `return_to` whitelist
// ---------------------
// We accept ONLY same-origin paths starting with `/classic/` and free of
// `\` / `\r` / `\n` / `%0a` / scheme prefixes. Anything else falls back
// to the safe default. This prevents the picker from being weaponised as
// an open redirect.

use axum::{
    extract::Form,
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;

use super::i18n::{build_set_locale_cookie, Locale};
use crate::error::AppError;

/// Default destination after a successful locale change when the form
/// doesn't carry a (safe) `return_to`. INBOX is the universal "home"
/// for authenticated users; anonymous users get bounced to /classic/login
/// by the index redirect, so the same fallback works for both.
const DEFAULT_RETURN_TO: &str = "/classic/folders/INBOX";

#[derive(Debug, Deserialize)]
pub struct LocaleForm {
    pub locale: String,
    #[serde(default)]
    pub return_to: Option<String>,
}

/// POST /classic/locale — write the locale cookie and 303 back to the
/// page the user was on.
///
/// Returns 400 on a missing/unknown locale (so an obvious bug surfaces
/// loudly in dev), 303 on success.
pub async fn post_locale(Form(form): Form<LocaleForm>) -> Result<Response, AppError> {
    let Some(locale) = Locale::from_code(form.locale.trim()) else {
        return Ok((StatusCode::BAD_REQUEST, "Unknown locale.").into_response());
    };
    let target = sanitise_return_to(form.return_to.as_deref());
    let cookie = build_set_locale_cookie(locale);

    let mut resp = (
        StatusCode::SEE_OTHER,
        [(header::LOCATION, target.as_str())],
    )
        .into_response();
    if let Ok(hv) = HeaderValue::from_str(&cookie) {
        resp.headers_mut().append(header::SET_COOKIE, hv);
    }
    Ok(resp)
}

/// Validate the `return_to` form field and pick a safe redirect target.
///
/// Accept rules (must all hold):
///   * Starts with `/classic/`
///   * No backslashes, CR, LF, or `%0` sequences that could smuggle
///     CRLF injection into the Location header.
///   * No scheme prefix (`http:`, `javascript:`, `//evil.com`).
///   * Length ≤ 512 (defensive — typical paths are 30-80 chars).
///
/// Any failure returns `DEFAULT_RETURN_TO`. The whitelist keeps the
/// picker from being weaponised as an open-redirect / response-splitting
/// gadget.
fn sanitise_return_to(raw: Option<&str>) -> String {
    let candidate = match raw {
        Some(s) => s.trim(),
        None => return DEFAULT_RETURN_TO.to_string(),
    };
    if candidate.is_empty() || candidate.len() > 512 {
        return DEFAULT_RETURN_TO.to_string();
    }
    if !candidate.starts_with("/classic/") {
        return DEFAULT_RETURN_TO.to_string();
    }
    // Protocol-relative URLs (`//evil.com`) bypass the leading-slash
    // check unless we reject explicitly. The starts_with above handles
    // the simple `//` case, but be belt-and-braces.
    if candidate.starts_with("//") {
        return DEFAULT_RETURN_TO.to_string();
    }
    // CRLF / null / backslash / scheme prefix anywhere in the value
    // would let an attacker inject a Location header continuation,
    // smuggle a path-traversal segment, or break out into a foreign
    // origin. Reject any of those.
    let forbidden = ['\r', '\n', '\0', '\\'];
    if candidate.chars().any(|c| forbidden.contains(&c)) {
        return DEFAULT_RETURN_TO.to_string();
    }
    // Percent-encoded CRLF would also escape header parsing. Cheap
    // case-insensitive check for the known prefixes.
    let lower = candidate.to_ascii_lowercase();
    if lower.contains("%0a") || lower.contains("%0d") || lower.contains("%00") {
        return DEFAULT_RETURN_TO.to_string();
    }
    // Scheme prefix check — any `:` BEFORE the first `/` past the leading
    // slash would be a scheme. The leading character is already `/`, so
    // a colon would necessarily land somewhere later in the path. Reject
    // any colon in the value: real classic paths don't contain colons.
    if candidate.contains(':') {
        return DEFAULT_RETURN_TO.to_string();
    }
    candidate.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitise_return_to_accepts_safe_classic_path() {
        assert_eq!(
            sanitise_return_to(Some("/classic/folders/INBOX")),
            "/classic/folders/INBOX"
        );
        assert_eq!(
            sanitise_return_to(Some("/classic/settings/signature")),
            "/classic/settings/signature"
        );
        assert_eq!(
            sanitise_return_to(Some("/classic/login")),
            "/classic/login"
        );
    }

    #[test]
    fn sanitise_return_to_rejects_non_classic_path() {
        assert_eq!(
            sanitise_return_to(Some("/api/auth/login")),
            DEFAULT_RETURN_TO
        );
        assert_eq!(sanitise_return_to(Some("/")), DEFAULT_RETURN_TO);
        // Single-slash prefix that isn't `/classic/` — the rule is strict.
        assert_eq!(sanitise_return_to(Some("/classic")), DEFAULT_RETURN_TO);
    }

    #[test]
    fn sanitise_return_to_rejects_open_redirect_patterns() {
        // Protocol-relative URL targeting another origin.
        assert_eq!(
            sanitise_return_to(Some("//evil.example.com/classic/")),
            DEFAULT_RETURN_TO
        );
        // Scheme prefix anywhere in the value.
        assert_eq!(
            sanitise_return_to(Some("/classic/x?next=http://evil.example.com")),
            DEFAULT_RETURN_TO
        );
        assert_eq!(
            sanitise_return_to(Some("/classic/javascript:alert(1)")),
            DEFAULT_RETURN_TO
        );
    }

    #[test]
    fn sanitise_return_to_rejects_crlf_injection() {
        // Direct CR / LF / null bytes.
        assert_eq!(
            sanitise_return_to(Some("/classic/x\r\nSet-Cookie: evil=1")),
            DEFAULT_RETURN_TO
        );
        assert_eq!(
            sanitise_return_to(Some("/classic/x\nfoo")),
            DEFAULT_RETURN_TO
        );
        // Percent-encoded CRLF.
        assert_eq!(
            sanitise_return_to(Some("/classic/x%0aSet-Cookie:evil")),
            DEFAULT_RETURN_TO
        );
        assert_eq!(
            sanitise_return_to(Some("/classic/x%0DSet-Cookie:evil")),
            DEFAULT_RETURN_TO
        );
    }

    #[test]
    fn sanitise_return_to_rejects_backslash() {
        // Backslash → some clients (Windows-y libraries) treat as path
        // separator and a downstream consumer might mis-parse.
        assert_eq!(
            sanitise_return_to(Some("/classic/foo\\bar")),
            DEFAULT_RETURN_TO
        );
    }

    #[test]
    fn sanitise_return_to_rejects_overlong_input() {
        let long = "/classic/".to_string() + &"a".repeat(600);
        assert_eq!(sanitise_return_to(Some(&long)), DEFAULT_RETURN_TO);
    }

    #[test]
    fn sanitise_return_to_falls_back_when_missing() {
        assert_eq!(sanitise_return_to(None), DEFAULT_RETURN_TO);
        assert_eq!(sanitise_return_to(Some("")), DEFAULT_RETURN_TO);
        assert_eq!(sanitise_return_to(Some("   ")), DEFAULT_RETURN_TO);
    }

    #[tokio::test]
    async fn post_locale_sets_cookie_and_redirects_on_known_code() {
        let form = LocaleForm {
            locale: "tw".to_string(),
            return_to: Some("/classic/folders/INBOX".to_string()),
        };
        let resp = post_locale(Form(form)).await.expect("handler succeeds");
        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
        let location = resp
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .expect("Location header present");
        assert_eq!(location, "/classic/folders/INBOX");

        let cookies: Vec<_> = resp
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .collect();
        assert!(
            cookies
                .iter()
                .any(|c| c.starts_with("tasmail_classic_locale=tw")),
            "expected tasmail_classic_locale=tw cookie, found {cookies:?}"
        );
    }

    #[tokio::test]
    async fn post_locale_falls_back_to_default_return_to_when_unsafe() {
        let form = LocaleForm {
            locale: "ha".to_string(),
            return_to: Some("https://evil.example.com/".to_string()),
        };
        let resp = post_locale(Form(form)).await.expect("handler succeeds");
        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
        let location = resp
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .expect("Location header present");
        assert_eq!(location, DEFAULT_RETURN_TO);
    }

    #[tokio::test]
    async fn post_locale_rejects_unknown_locale_with_400() {
        let form = LocaleForm {
            locale: "zh".to_string(),
            return_to: None,
        };
        let resp = post_locale(Form(form)).await.expect("handler succeeds");
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn post_locale_accepts_uppercase_code() {
        // Defensive — the parser is case-insensitive; lock that down so
        // future tightening doesn't accidentally reject "EN".
        let form = LocaleForm {
            locale: "EN".to_string(),
            return_to: None,
        };
        let resp = post_locale(Form(form)).await.expect("handler succeeds");
        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    }

    #[tokio::test]
    async fn post_locale_handles_bcp47_subtag() {
        let form = LocaleForm {
            locale: "tw-GH".to_string(),
            return_to: None,
        };
        let resp = post_locale(Form(form)).await.expect("handler succeeds");
        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
        let cookies: Vec<_> = resp
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .collect();
        assert!(
            cookies
                .iter()
                .any(|c| c.starts_with("tasmail_classic_locale=tw")),
            "BCP47 subtag must collapse to bare code in cookie, found {cookies:?}"
        );
    }
}
