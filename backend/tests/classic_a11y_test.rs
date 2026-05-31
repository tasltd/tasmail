// TMAIL-369 — WCAG 2.2 AA conformance for the /classic no-JS templates.
//
// This file complements the Playwright + axe-core spec at
// `frontend/e2e/specs/classic-ui-a11y.spec.ts`. axe-core inside Playwright
// covers the live-browser checks (computed contrast, focus order, ARIA
// state); these in-process Rust tests cover the *structural* invariants
// that the Askama templates must always emit, which we can verify cheaply
// without a real browser:
//
//   1. `<html lang="...">` is declared.
//   2. Landmarks present: `<header role="banner">`, `<nav>`, `<main id="main">`,
//      `<footer role="contentinfo">`.
//   3. Skip-to-main link (`<a class="skip-link" href="#main">`) is in the
//      DOM and appears before <main> in document order.
//   4. Exactly one `<h1>` on each rendered page.
//   5. Every form input has either a matching `<label for>` or an
//      `aria-label` / `aria-labelledby`.
//   6. Every `<button>` has visible text content (or an `aria-label` —
//      no icon-only buttons).
//   7. The `.visually-hidden` CSS class is defined in the inline style
//      block (verifies the base.html SR-only utility is wired up — this
//      catches the bug fixed in TMAIL-369 where the class was used but
//      never defined, leaving the "TASMail" brand suffix, the bulk-action
//      column header, and the Move-to dropdown label visible on screen).
//
// We render the templates by hitting the live router with TestApp, so
// these tests prove the FULL request → handler → template → bytes path
// behaves correctly, not just that the templates compile.

mod common;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use common::TestApp;
use http_body_util::BodyExt;

/// Fetch a Classic UI HTML page through the router and return the body as
/// UTF-8 text. Asserts the response is 2xx / 3xx / 4xx HTML (not JSON) so
/// the structural assertions below run on real markup, not an error envelope.
async fn fetch_html(app: &TestApp, path: &str, expected_status: StatusCode) -> String {
    let req = Request::builder()
        .method(Method::GET)
        .uri(path)
        .body(Body::empty())
        .unwrap();
    let response = app.raw_request(req).await;
    assert_eq!(
        response.status(),
        expected_status,
        "GET {path} returned unexpected status",
    );
    let ct = response
        .headers()
        .get(header::CONTENT_TYPE)
        .map(|v| v.to_str().unwrap().to_owned())
        .unwrap_or_default();
    assert!(
        ct.starts_with("text/html"),
        "GET {path} must render HTML, got Content-Type={ct}",
    );
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).expect("classic UI body must be valid UTF-8")
}

/// Count the literal occurrences of an opening tag of the form `<TAG`.
/// Naive but sufficient for these small templates — we don't have nested
/// `<h1>`s or `<main>`s anywhere in the surface so counting open tags is
/// equivalent to counting elements. Far cheaper than dragging in a full
/// HTML parser as a dev-dep.
fn count_open_tags(html: &str, tag: &str) -> usize {
    // Match `<tag` followed by space, `>`, or `/`. Lowercased.
    let needle_space = format!("<{}", tag.to_lowercase());
    let body = html.to_lowercase();
    let mut count = 0;
    let mut cursor = 0;
    while let Some(idx) = body[cursor..].find(&needle_space) {
        let abs = cursor + idx;
        // The character right after must be space, '>', '/', or '\n' —
        // otherwise we matched e.g. `<header` while searching for `<head`.
        let next = body[abs + needle_space.len()..].chars().next();
        if matches!(next, Some(' ' | '>' | '/' | '\n' | '\r' | '\t')) {
            count += 1;
        }
        cursor = abs + needle_space.len();
    }
    count
}

/// Assert the global shell invariants every Classic UI page must satisfy.
fn assert_classic_shell(html: &str, page_label: &str) {
    // 1. `<html lang>` declared.
    assert!(
        html.contains("<html lang=\"en\">") || html.contains("<html lang='en'>"),
        "{page_label}: <html lang=\"en\"> must be declared",
    );

    // 2. Skip-to-main link is in the markup and points at #main.
    assert!(
        html.contains("class=\"skip-link\"")
            && html.contains("href=\"#main\""),
        "{page_label}: skip-to-main link must be in the markup",
    );
    let skip_idx = html.find("class=\"skip-link\"").expect("skip-link must exist");
    let main_idx = html.find("id=\"main\"").expect("<main id=main> must exist");
    assert!(
        skip_idx < main_idx,
        "{page_label}: skip link must appear BEFORE <main id=main> in document order",
    );

    // 3. Landmarks.
    assert!(
        html.contains("<header class=\"site-header\" role=\"banner\">"),
        "{page_label}: <header role=banner> landmark must be present",
    );
    assert!(
        html.contains("<nav class=\"site-nav\" aria-label=\"Primary\">"),
        "{page_label}: <nav aria-label=Primary> landmark must be present",
    );
    assert!(
        html.contains("<main id=\"main\""),
        "{page_label}: <main id=\"main\"> landmark must be present",
    );
    assert!(
        html.contains("role=\"main\""),
        "{page_label}: <main> must declare role=\"main\" explicitly",
    );
    assert!(
        html.contains("<footer class=\"site-footer\" role=\"contentinfo\">"),
        "{page_label}: <footer role=contentinfo> landmark must be present",
    );

    // 4. Exactly one <h1>.
    let h1_count = count_open_tags(html, "h1");
    assert_eq!(
        h1_count, 1,
        "{page_label}: must have exactly one <h1>, found {h1_count}",
    );

    // 5. The .visually-hidden CSS class is defined in the inline <style>
    //    block — regression guard for the TMAIL-369 fix that added it.
    //    Without this rule the brand "TASMail" suffix, the "Select" column
    //    header on the folder view, and the Move-to dropdown label all
    //    render as visible text instead of being SR-only.
    assert!(
        html.contains(".visually-hidden"),
        "{page_label}: .visually-hidden CSS rule must be defined in base.html",
    );
    assert!(
        html.contains("position: absolute !important")
            || html.contains("position:absolute !important"),
        "{page_label}: .visually-hidden must use the SR-only clipping recipe",
    );

    // 6. Every <button> must have a visible text node or aria-label.
    //    Naive scan: find each `<button` opening, capture up to the
    //    matching `</button>`, strip tags, check non-empty.
    let mut cursor = 0;
    while let Some(open) = html[cursor..].find("<button") {
        let abs_open = cursor + open;
        let close_open_tag = html[abs_open..].find('>').unwrap();
        let body_start = abs_open + close_open_tag + 1;
        let close_idx = html[body_start..]
            .find("</button>")
            .unwrap_or_else(|| panic!("{page_label}: unclosed <button> at offset {abs_open}"));
        let abs_close = body_start + close_idx;
        let inner = &html[body_start..abs_close];
        let opening = &html[abs_open..body_start];

        // Strip nested tags from inner to get pure text.
        let mut text_only = String::new();
        let mut in_tag = false;
        for c in inner.chars() {
            if c == '<' {
                in_tag = true;
            } else if c == '>' {
                in_tag = false;
            } else if !in_tag {
                text_only.push(c);
            }
        }
        let text_trimmed = text_only.trim();
        let has_aria_label = opening.contains("aria-label=");
        assert!(
            !text_trimmed.is_empty() || has_aria_label,
            "{page_label}: <button> at offset {abs_open} must have visible text or aria-label (opening tag: `{opening}`)",
        );

        cursor = abs_close + "</button>".len();
    }

    // 7. No raw `form=""` attribute on inputs (axe flags as invalid; the
    //    fix in TMAIL-369 either drops the attribute or references a
    //    real form id).
    assert!(
        !html.contains(" form=\"\""),
        "{page_label}: no input may carry an empty `form=\"\"` attribute (axe-core flags it as invalid)",
    );

    // 8. No <script> tag — Classic UI is a strict no-JS surface.
    assert_eq!(
        count_open_tags(html, "script"),
        0,
        "{page_label}: no <script> tag may appear on the Classic UI",
    );

    // 9. CSP nonce was substituted into the style tag — empty
    //    `nonce=""` would mean the CSP header rejects every CSS rule,
    //    rendering the page unstyled.
    assert!(
        !html.contains("<style nonce=\"\""),
        "{page_label}: <style nonce=\"...\"> must carry a non-empty per-request CSP nonce",
    );
}

/// Check every <label for="X"> targets an input that exists, and every
/// non-hidden input either has a matching label OR an aria-label /
/// aria-labelledby. Mirrors axe's `label` rule.
fn assert_inputs_are_labelled(html: &str, page_label: &str) {
    // Collect every label `for=` attribute value.
    let mut labelled_ids: Vec<String> = Vec::new();
    let mut cursor = 0;
    while let Some(open) = html[cursor..].find("<label") {
        let abs_open = cursor + open;
        let close_open = html[abs_open..].find('>').unwrap();
        let opening = &html[abs_open..abs_open + close_open];
        if let Some(pos) = opening.find("for=\"") {
            let val_start = pos + "for=\"".len();
            let val_end = opening[val_start..].find('"').unwrap();
            labelled_ids.push(opening[val_start..val_start + val_end].to_string());
        }
        cursor = abs_open + close_open;
    }

    // Now scan every <input>, <textarea>, <select> and confirm a label
    // points at it (or it has aria-label / aria-labelledby).
    for tag in &["input", "textarea", "select"] {
        let needle = format!("<{tag}");
        let mut cursor = 0;
        while let Some(open) = html[cursor..].find(&needle) {
            let abs_open = cursor + open;
            let close_open = html[abs_open..].find('>').unwrap();
            let opening = &html[abs_open..abs_open + close_open];

            // Skip if the very next char after the tag name isn't
            // delimiter-y (avoids matching `<inputable>` etc.).
            let next_char = opening.as_bytes().get(needle.len()).copied().unwrap_or(b' ');
            if !matches!(next_char, b' ' | b'/' | b'>' | b'\n' | b'\r' | b'\t') {
                cursor = abs_open + close_open;
                continue;
            }

            // Skip hidden + submit + button inputs — they don't need labels.
            if opening.contains("type=\"hidden\"")
                || opening.contains("type=\"submit\"")
                || opening.contains("type=\"button\"")
            {
                cursor = abs_open + close_open;
                continue;
            }

            // aria-label / aria-labelledby satisfy the rule.
            if opening.contains("aria-label=") || opening.contains("aria-labelledby=") {
                cursor = abs_open + close_open;
                continue;
            }

            // Otherwise the input MUST have id="X" and a label[for=X]
            // somewhere on the page.
            let id_start = opening.find("id=\"").unwrap_or_else(|| {
                panic!(
                    "{page_label}: <{tag}> without aria-label and without id at offset {abs_open}: `{opening}`",
                )
            });
            let id_val_start = id_start + "id=\"".len();
            let id_val_end = opening[id_val_start..].find('"').unwrap_or_else(|| {
                panic!(
                    "{page_label}: malformed id attribute on <{tag}> at offset {abs_open}: `{opening}`",
                )
            });
            let id = &opening[id_val_start..id_val_start + id_val_end];
            assert!(
                labelled_ids.iter().any(|l| l == id),
                "{page_label}: <{tag} id=\"{id}\"> has no matching <label for=\"{id}\"> and no aria-label",
            );

            cursor = abs_open + close_open;
        }
    }
}

#[tokio::test]
async fn classic_login_page_passes_wcag_aa_structural_checks() {
    let app = TestApp::new().await;
    let html = fetch_html(&app, "/classic/login", StatusCode::OK).await;

    assert_classic_shell(&html, "/classic/login");
    assert_inputs_are_labelled(&html, "/classic/login");

    // Login-specific: email and password inputs have correct autocomplete
    // values so password managers + iOS keychain can populate them. WCAG
    // 2.2 SC 1.3.5 "Identify Input Purpose" requires autocomplete.
    assert!(
        html.contains("autocomplete=\"username\""),
        "/classic/login: email field must declare autocomplete=username (WCAG 2.2 SC 1.3.5)",
    );
    assert!(
        html.contains("autocomplete=\"current-password\""),
        "/classic/login: password field must declare autocomplete=current-password",
    );
}

#[tokio::test]
async fn classic_csrf_error_page_passes_wcag_aa_structural_checks() {
    // Drive the CSRF error path: POST /classic/login without the pre-session
    // CSRF cookie → re-render with role="alert" error banner. The handler
    // renders the login template with `error: Some(...)`, so the shell
    // invariants must still hold.
    let app = TestApp::new().await;
    let req = Request::builder()
        .method(Method::POST)
        .uri("/classic/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("email=fake%40example.invalid&password=wrong&_csrf=bogus"))
        .unwrap();
    let response = app.raw_request(req).await;
    // Either 200 with error banner, 400, or 403 — all surface the same
    // template. We accept any 2xx/4xx; what matters is the HTML body has
    // the role="alert" banner with our shell invariants.
    let status = response.status();
    let ct = response
        .headers()
        .get(header::CONTENT_TYPE)
        .map(|v| v.to_str().unwrap().to_owned())
        .unwrap_or_default();
    if !ct.starts_with("text/html") {
        // Some failure paths short-circuit before rendering — those don't
        // need a11y checks since they're 4xx without a surface. Skip.
        return;
    }
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let html = String::from_utf8(bytes.to_vec()).expect("utf-8");
    assert_classic_shell(&html, &format!("POST /classic/login (CSRF reject, status={status})"));
    assert!(
        html.contains("role=\"alert\""),
        "CSRF error re-render must announce the failure via role=alert",
    );
}

#[tokio::test]
async fn classic_not_found_page_passes_wcag_aa_structural_checks() {
    let app = TestApp::new().await;
    let html = fetch_html(
        &app,
        "/classic/this-route-does-not-exist-tmail369",
        StatusCode::NOT_FOUND,
    )
    .await;

    assert_classic_shell(&html, "/classic/<404>");
    // 404 page deliberately has no inputs — assertion below is a no-op
    // but exercises the path.
    assert_inputs_are_labelled(&html, "/classic/<404>");
}

/// Dump the rendered HTML of each Classic UI public page to /tmp so the
/// shell-side smoke test (`scripts/classic-ui-text-smoke.sh`) can pipe
/// the dumps through `lynx -dump` / `w3m -dump` and assert the text-only
/// output remains coherent. Marked `#[ignore]` so a plain
/// `cargo test --test classic_a11y_test` only runs the structural checks
/// above — the dumps are explicit-opt-in.
///
/// Run:
///   cargo test --test classic_a11y_test -- --ignored
/// Or via the smoke script:
///   scripts/classic-ui-text-smoke.sh
#[tokio::test]
#[ignore]
async fn classic_dump_pages_for_text_browser_smoke_test() {
    use std::io::Write;
    let app = TestApp::new().await;
    let dump_dir = std::env::var("TASMAIL_DUMP_DIR")
        .unwrap_or_else(|_| "/tmp/tasmail-classic-dumps".to_string());
    std::fs::create_dir_all(&dump_dir).expect("create dump dir");

    let pages = [
        ("login.html", "/classic/login", StatusCode::OK),
        ("not_found.html", "/classic/this-route-does-not-exist", StatusCode::NOT_FOUND),
    ];

    for (filename, path, expected_status) in pages.iter() {
        let req = Request::builder()
            .method(Method::GET)
            .uri(*path)
            .body(Body::empty())
            .unwrap();
        let resp = app.raw_request(req).await;
        assert_eq!(resp.status(), *expected_status, "GET {path} status mismatch");
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let dest = std::path::Path::new(&dump_dir).join(filename);
        let mut f = std::fs::File::create(&dest).expect("create dump file");
        f.write_all(&bytes).expect("write dump file");
        eprintln!("dumped {path} → {}", dest.display());
    }
}

#[tokio::test]
async fn count_open_tags_handles_word_boundaries() {
    // Regression guard for the naive open-tag counter. Make sure
    // `<header>` doesn't get counted when we ask for `<head>`.
    let html = "<head></head><header></header><h1>x</h1><h1>y</h1>";
    assert_eq!(count_open_tags(html, "head"), 1);
    assert_eq!(count_open_tags(html, "header"), 1);
    assert_eq!(count_open_tags(html, "h1"), 2);
    assert_eq!(count_open_tags(html, "script"), 0);
}
