// Added (TMAIL-363) / Expanded (TMAIL-364): Strict-allowlist HTML sanitiser
// for rendering untrusted email bodies on the Classic UI no-JS surface and
// any future mobile-render path that wants the same guarantees.
//
// This module is the *single* sanitiser used by every server-side render
// path. Strict allowlist:
//
//   * Drop `<script>` / `<style>` / `<iframe>` / `<object>` / `<embed>` /
//     `<form>` / `<base>` / `<meta>` elements and their children
//     (ammonia's default tag allowlist already does this — we lock it in
//     with explicit tests below).
//   * Drop every inline event handler (`on*`) — ammonia's default attribute
//     allowlist already does this.
//   * Drop `javascript:` and `vbscript:` URLs — the scheme allowlist does
//     this.
//   * Drop `data:` URLs except `data:image/*` on `<img src>`, and explicitly
//     drop `data:image/svg+xml` because inline SVG can carry `<script>`.
//   * Drop inline `style=` that contains `expression(...)`,
//     `javascript:`, or `vbscript:` (defence-in-depth — ammonia drops
//     `style=` by default, but the filter survives if any future tag
//     allowlists `style`).
//   * Block remote `<img src>` (http / https) by default and rewrite the
//     URL to a 1×1 transparent-GIF data URL placeholder so the original
//     remote URL never leaves the server. This kills the most common
//     tracking-pixel beacon vector. Senders that want their images shown
//     get the per-message "Show images" opt-in (TMAIL-32, P1) which calls
//     `sanitize_email_html_with_options(html, SanitizeOptions {
//     allow_remote_images: true })`.
//   * Preserve safe text-formatting tags + safe `<a href>` (with
//     `rel="noopener noreferrer nofollow"` injected) + safe `<img>` with a
//     local / `cid:` / `data:image/*` (non-svg) source, so the rendered
//     email is actually useful, not just a stripped string.
//
// See `docs/gap-analysis/classic-ui.md` P0 #10 (this task) and P1 #32
// (Show-images UX that consumes `SanitizeOptions::allow_remote_images`).

use std::borrow::Cow;
use std::sync::OnceLock;

use ammonia::Builder;

/// 1×1 transparent GIF served inline as a data URL. We rewrite remote
/// `<img src>` to this when remote images are blocked, so the original
/// URL never leaks (no tracking-pixel beacon) and the `<img>` element
/// still renders without a "broken image" chrome glyph.
///
/// The Show-images UX (TMAIL-32) decides whether to re-render with
/// `allow_remote_images = true` and surface the real URL again.
const BLOCKED_IMAGE_PLACEHOLDER: &str =
    "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7";

/// Render-time options for the email sanitiser.
///
/// Today the only knob is `allow_remote_images`. Keep this a struct (not a
/// bare `bool` argument) so the P1 Show-images UX (#32) and any future
/// per-user pref (`block_remote_images` from the gap analysis) can extend
/// it without re-touching every call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SanitizeOptions {
    /// When `true`, remote `http`/`https` `<img src>` URLs survive the
    /// sanitiser unchanged. When `false` (the default), they are rewritten
    /// to `BLOCKED_IMAGE_PLACEHOLDER`. CID inline images and
    /// `data:image/*` (non-svg) images are unaffected by this flag —
    /// they are always allowed because they live in the same email.
    pub allow_remote_images: bool,
}

impl Default for SanitizeOptions {
    fn default() -> Self {
        // Privacy-aware default: block remote images. Matches the gap
        // analysis recommendation ("default `true` for unknown senders")
        // and is the right default for any caller that hasn't explicitly
        // opted the recipient in.
        Self {
            allow_remote_images: false,
        }
    }
}

/// PURPOSE: Sanitise an untrusted email `text/html` body for safe inline
/// rendering inside the Classic UI message view (and any future server-side
/// render path that needs the same guarantees).
///
/// Returns a HTML string that can be dropped into a `{{ var|safe }}` Askama
/// expression without re-escaping — every dangerous construct is stripped
/// at parse time, not relied on at render time.
///
/// Uses `SanitizeOptions::default()` — i.e. blocks remote images by
/// default. Call `sanitize_email_html_with_options` to override.
pub fn sanitize_email_html(raw_html: &str) -> String {
    sanitize_email_html_with_options(raw_html, SanitizeOptions::default())
}

/// PURPOSE: Same as `sanitize_email_html`, but lets the caller override
/// the default options. Used by the Show-images per-message opt-in
/// (TMAIL-32) which renders the message twice — once with images blocked
/// for the default view, and once with `allow_remote_images = true` for
/// the "Show images" re-render.
pub fn sanitize_email_html_with_options(raw_html: &str, opts: SanitizeOptions) -> String {
    if opts.allow_remote_images {
        builder_allow_remote().clean(raw_html).to_string()
    } else {
        builder_block_remote().clean(raw_html).to_string()
    }
}

/// Shared base config — every knob that doesn't depend on the
/// `allow_remote_images` flag goes here. The two callers wrap it with the
/// appropriate `attribute_filter`.
fn configure_base(b: &mut Builder<'static>) {
    // ammonia's default tag allowlist already drops <script>, <iframe>,
    // <object>, <embed>, <style>, <link>, <meta>, <base>, <form>, etc.
    // We keep the default and only extend with the tags an email needs
    // that ammonia ships disabled by default.
    b.add_tags(["img", "table", "thead", "tbody", "tfoot", "tr", "td", "th"]);
    // `target` on <a> so external links open in a new tab; ammonia's
    // `link_rel` rewriter below adds noopener/noreferrer.
    b.add_tag_attributes("a", ["target"]);
    // Email frequently uses <img width/height/alt/title>; allowlist those.
    b.add_tag_attributes("img", ["width", "height", "alt", "title"]);
    // <table> formatting attributes — old-school table-based email layout
    // (still used by Mailchimp etc.) needs them to render readably.
    b.add_tag_attributes(
        "table",
        ["width", "cellpadding", "cellspacing", "border", "align"],
    );
    b.add_tag_attributes(
        "td",
        ["width", "height", "align", "valign", "colspan", "rowspan"],
    );
    b.add_tag_attributes(
        "th",
        ["width", "height", "align", "valign", "colspan", "rowspan"],
    );
    b.add_tag_attributes("tr", ["align", "valign"]);
    // URL scheme allowlist. We include `data` so inline `data:image/*`
    // attachments survive, and rely on `attribute_filter` (below) to
    // strip every `data:` URL that isn't a safe image. Schemes that
    // never make sense in an email body (ftp, magnet, gopher, irc, …)
    // are dropped from the default set to shrink the attack surface.
    b.url_schemes(
        ["http", "https", "mailto", "tel", "cid", "data"]
            .iter()
            .copied()
            .collect(),
    );
    // Add noopener + noreferrer to every <a> that survives, so a
    // tabnabbing payload can't reach window.opener on the click target.
    b.link_rel(Some("noopener noreferrer nofollow"));
}

fn builder_block_remote() -> &'static Builder<'static> {
    static BUILDER: OnceLock<Builder<'static>> = OnceLock::new();
    BUILDER.get_or_init(|| {
        let mut b = Builder::default();
        configure_base(&mut b);
        b.attribute_filter(|element, attribute, value| {
            attribute_gate(element, attribute, value, false)
        });
        b
    })
}

fn builder_allow_remote() -> &'static Builder<'static> {
    static BUILDER: OnceLock<Builder<'static>> = OnceLock::new();
    BUILDER.get_or_init(|| {
        let mut b = Builder::default();
        configure_base(&mut b);
        b.attribute_filter(|element, attribute, value| {
            attribute_gate(element, attribute, value, true)
        });
        b
    })
}

/// Case-insensitive ASCII prefix check. Avoids allocating a lowercased
/// copy of the full value when we only need to inspect the leading
/// characters (data URLs can be many KB).
fn ci_starts_with(s: &str, prefix: &str) -> bool {
    s.get(..prefix.len())
        .map(|head| head.eq_ignore_ascii_case(prefix))
        .unwrap_or(false)
}

/// PURPOSE: Per-attribute gate run by ammonia for every attribute that
/// survives the tag/attr/scheme allowlists. Layered defence on top of
/// ammonia's built-in filtering — gives us the fine-grained rules the
/// allowlist API can't express.
///
/// Returns `None` to drop the attribute entirely, `Some(value)` to keep
/// it (optionally rewritten).
fn attribute_gate<'a>(
    element: &str,
    attribute: &str,
    value: &'a str,
    allow_remote_images: bool,
) -> Option<Cow<'a, str>> {
    // 1. `data:` URLs. The scheme allowlist lets these through; this
    //    filter narrows the surface to `data:image/*` on `<img src>`
    //    only, and excludes `data:image/svg+xml` because inline SVG can
    //    carry `<script>` which executes when navigated to (and in some
    //    legacy renderers when loaded via `<img>`).
    if ci_starts_with(value, "data:") {
        let safe_inline_image = element == "img"
            && attribute == "src"
            && ci_starts_with(value, "data:image/")
            && !ci_starts_with(value, "data:image/svg+xml");
        if !safe_inline_image {
            return None;
        }
        return Some(Cow::Borrowed(value));
    }

    // 2. Inline `style=` defence. ammonia drops `style=` by default
    //    (it's not in any tag's default attribute allowlist), but this
    //    filter survives even if a future revision allowlists it, and
    //    catches `expression(...)` (legacy IE), `javascript:` URLs
    //    inside `background:url(...)`, and `vbscript:` URLs.
    if attribute.eq_ignore_ascii_case("style") {
        let lower = value.to_ascii_lowercase();
        if lower.contains("expression(")
            || lower.contains("javascript:")
            || lower.contains("vbscript:")
        {
            return None;
        }
    }

    // 3. Remote-image gating. http/https `<img src>` is rewritten to a
    //    1×1 transparent placeholder when the caller hasn't opted in to
    //    remote images. This kills the tracking-pixel beacon: the
    //    original URL never leaves the server, so the sender can't tell
    //    the recipient opened the message.
    if element == "img"
        && attribute == "src"
        && !allow_remote_images
        && (ci_starts_with(value, "http://") || ci_starts_with(value, "https://"))
    {
        return Some(Cow::Borrowed(BLOCKED_IMAGE_PLACEHOLDER));
    }

    Some(Cow::Borrowed(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- XSS / payload removal -----------------------------------------

    #[test]
    fn strips_script_tag_and_payload() {
        let input = r#"<p>hello</p><script>alert('xss')</script>"#;
        let out = sanitize_email_html(input);
        assert!(!out.contains("<script"), "raw <script> leaked: {out}");
        assert!(!out.contains("alert"), "script body leaked: {out}");
        assert!(out.contains("<p>hello</p>"), "safe content lost: {out}");
    }

    #[test]
    fn strips_inline_event_handlers() {
        let input = r#"<a href="https://example.com" onclick="steal()">click</a>"#;
        let out = sanitize_email_html(input);
        assert!(!out.contains("onclick"), "onclick survived: {out}");
        assert!(!out.contains("steal()"));
        assert!(out.contains("href=\"https://example.com\""));
    }

    #[test]
    fn strips_onerror_handler_on_image() {
        // A common XSS payload smuggled via an <img onerror=...> attribute.
        let input = r#"<img src="cid:logo" onerror="alert(1)" alt="x">"#;
        let out = sanitize_email_html(input);
        assert!(!out.contains("onerror"), "onerror survived: {out}");
        assert!(!out.contains("alert(1)"));
    }

    // ---- URL scheme stripping ------------------------------------------

    #[test]
    fn drops_javascript_url() {
        let input = r#"<a href="javascript:alert(1)">x</a>"#;
        let out = sanitize_email_html(input);
        assert!(!out.contains("javascript:"));
        assert!(!out.contains("alert(1)"));
    }

    #[test]
    fn drops_vbscript_url() {
        let input = r#"<a href="vbscript:msgbox(1)">x</a>"#;
        let out = sanitize_email_html(input);
        assert!(!out.contains("vbscript:"));
    }

    #[test]
    fn drops_data_text_html_on_anchor() {
        // <a href=data:text/html,...> would render arbitrary HTML if the
        // user clicked the link. `data` is in the scheme allowlist so
        // that `data:image/*` works on <img src>, but `attribute_gate`
        // strips every `data:` URL that isn't a safe image.
        let input = r#"<a href="data:text/html,<script>alert(1)</script>">x</a>"#;
        let out = sanitize_email_html(input);
        assert!(!out.contains("data:text/html"), "data:text/html leaked: {out}");
        assert!(!out.contains("<script"));
    }

    #[test]
    fn drops_data_image_on_anchor_href() {
        // Even `data:image/png` is unsafe as an <a href> — the user
        // clicking the link could be tricked into navigating to a
        // server-controlled image that's used as part of a phishing
        // chain. Only allow `data:image/*` on <img src>.
        let input = r#"<a href="data:image/png;base64,iVBORw0KGgo=">click</a>"#;
        let out = sanitize_email_html(input);
        assert!(!out.contains("data:image/png"), "data:image/* leaked on <a>: {out}");
    }

    // ---- Element stripping ---------------------------------------------

    #[test]
    fn strips_iframe() {
        let input = r#"<iframe src="https://evil.example.com/x"></iframe><p>after</p>"#;
        let out = sanitize_email_html(input);
        assert!(!out.contains("<iframe"));
        assert!(!out.contains("evil.example.com"));
        assert!(out.contains("<p>after</p>"));
    }

    #[test]
    fn strips_form_to_prevent_phishing_overlay() {
        // A phishing email could overlay a fake login form on top of the
        // visible content. Drop <form> outright — the Classic UI never
        // needs a sender-provided form to render in a message body.
        let input = r#"<form action="https://evil.example.com"><input name="pw"></form><p>ok</p>"#;
        let out = sanitize_email_html(input);
        assert!(!out.contains("<form"));
        assert!(!out.contains("<input"));
        assert!(out.contains("<p>ok</p>"));
    }

    #[test]
    fn strips_base_tag() {
        // <base href="https://evil.example.com/"> would re-target every
        // relative URL in the email body to the attacker's host.
        let input = r#"<base href="https://evil.example.com/"><a href="/x">x</a>"#;
        let out = sanitize_email_html(input);
        assert!(!out.contains("<base"));
        assert!(!out.contains("evil.example.com"));
    }

    #[test]
    fn strips_style_tag_and_css_expression_payload() {
        // Author <style> tags can include `expression()` (legacy IE
        // remote-code), `@import`, or `url("javascript:...")` payloads.
        // ammonia drops <style> by default; this test locks that in AND
        // is the canonical "CSS expression stripping" check called out
        // in the TMAIL-364 task description.
        let input = r#"<style>body { background: expression(alert('xss')); }</style><p>ok</p>"#;
        let out = sanitize_email_html(input);
        assert!(!out.contains("<style"), "<style> survived: {out}");
        assert!(!out.contains("expression("), "expression() survived: {out}");
        assert!(!out.contains("alert"));
        assert!(out.contains("<p>ok</p>"));
    }

    #[test]
    fn strips_inline_style_attribute() {
        // ammonia drops `style=` by default (not in any tag's default
        // allowlist). This pins that behaviour — even a CSS expression
        // payload smuggled into an inline `style=` is gone.
        let input = r#"<p style="background:url('javascript:alert(1)')">x</p>"#;
        let out = sanitize_email_html(input);
        assert!(!out.contains("style="), "inline style attr survived: {out}");
        assert!(!out.contains("javascript:"));
        assert!(!out.contains("alert(1)"));
    }

    #[test]
    fn strips_object_and_embed() {
        // Both <object> and <embed> can host plug-in content that
        // escapes the document sandbox.
        let input = r#"<object data="evil.swf"></object><embed src="evil.swf">"#;
        let out = sanitize_email_html(input);
        assert!(!out.contains("<object"));
        assert!(!out.contains("<embed"));
        assert!(!out.contains("evil.swf"));
    }

    #[test]
    fn strips_meta_refresh_redirect() {
        // <meta http-equiv="refresh"> can hijack the page to an attacker URL.
        let input = r#"<meta http-equiv="refresh" content="0;url=https://evil.example.com/"><p>ok</p>"#;
        let out = sanitize_email_html(input);
        assert!(!out.contains("<meta"));
        assert!(!out.contains("evil.example.com"));
        assert!(out.contains("<p>ok</p>"));
    }

    // ---- Remote-image gating (TMAIL-364 core) --------------------------

    #[test]
    fn blocks_remote_http_image_by_default_and_rewrites_to_placeholder() {
        let input = r#"<img src="http://example.com/pixel.png" alt="tracker">"#;
        let out = sanitize_email_html(input);
        assert!(
            !out.contains("example.com"),
            "original remote URL leaked through sanitiser: {out}"
        );
        assert!(
            out.contains(BLOCKED_IMAGE_PLACEHOLDER),
            "remote <img src> not rewritten to placeholder: {out}"
        );
        // The alt text still surfaces so the reader knows what was there.
        assert!(out.contains("alt=\"tracker\""));
    }

    #[test]
    fn blocks_remote_https_image_by_default_and_rewrites_to_placeholder() {
        let input = r#"<img src="https://tracker.example.com/beacon.gif?uid=42">"#;
        let out = sanitize_email_html(input);
        assert!(
            !out.contains("tracker.example.com"),
            "original remote URL leaked: {out}"
        );
        assert!(!out.contains("uid=42"));
        assert!(out.contains(BLOCKED_IMAGE_PLACEHOLDER));
    }

    #[test]
    fn blocks_remote_image_when_scheme_is_uppercase() {
        // Case-insensitive scheme handling — `HTTPS://...` is the same
        // URL to the browser as `https://...`. Our gate must catch both.
        let input = r#"<img src="HTTPS://tracker.example.com/x.gif">"#;
        let out = sanitize_email_html(input);
        assert!(!out.contains("tracker.example.com"));
        assert!(out.contains(BLOCKED_IMAGE_PLACEHOLDER));
    }

    #[test]
    fn allow_remote_images_option_keeps_remote_src_unchanged() {
        // The Show-images opt-in path (P1 #32) calls this entry point.
        // When the user has explicitly clicked "Show images" we must
        // surface the real URL again so the email renders as the sender
        // intended.
        let input = r#"<img src="https://example.com/header.png" alt="banner">"#;
        let out = sanitize_email_html_with_options(
            input,
            SanitizeOptions {
                allow_remote_images: true,
            },
        );
        assert!(
            out.contains("src=\"https://example.com/header.png\""),
            "opted-in remote image was still rewritten: {out}"
        );
        assert!(!out.contains(BLOCKED_IMAGE_PLACEHOLDER));
    }

    #[test]
    fn allows_inline_data_image_png() {
        // Inline `data:image/png` images are part of the message MIME
        // body — no remote fetch happens. Always allow.
        let input = r#"<img src="data:image/png;base64,iVBORw0KGgo=">"#;
        let out = sanitize_email_html(input);
        assert!(
            out.contains("data:image/png;base64,iVBORw0KGgo="),
            "inline data:image/png was stripped: {out}"
        );
    }

    #[test]
    fn blocks_data_image_svg_xml_to_kill_svg_xss() {
        // SVG can carry inline <script>. Some legacy renderers execute
        // SVG script when loaded via <img>. Drop `data:image/svg+xml`
        // even on <img src>.
        let input =
            r#"<img src="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg'><script>alert(1)</script></svg>">"#;
        let out = sanitize_email_html(input);
        assert!(
            !out.contains("data:image/svg+xml"),
            "data:image/svg+xml survived: {out}"
        );
        assert!(!out.contains("<script"));
    }

    #[test]
    fn preserves_cid_inline_image_scheme() {
        // `cid:` is the RFC 2392 scheme for inline message-part
        // references. Inline attachments live in the same email — they
        // are NOT a remote fetch and the gate doesn't touch them.
        let input = r#"<img src="cid:logo@example.com" alt="logo">"#;
        let out = sanitize_email_html(input);
        assert!(
            out.contains("cid:logo@example.com"),
            "cid: scheme dropped: {out}"
        );
    }

    #[test]
    fn drops_image_with_unsupported_scheme() {
        // ftp/file/etc. aren't in the URL scheme allowlist — ammonia
        // strips the attribute. The placeholder is NOT inserted because
        // there's no `src` value for our filter to rewrite.
        let input = r#"<img src="ftp://example.com/x.png" alt="ftp"><img src="file:///etc/passwd">"#;
        let out = sanitize_email_html(input);
        assert!(!out.contains("ftp://"));
        assert!(!out.contains("file://"));
        assert!(!out.contains("/etc/passwd"));
    }

    // ---- Safe content preservation -------------------------------------

    #[test]
    fn preserves_safe_anchor_with_noopener_rel() {
        let input = r#"<a href="https://example.com">click</a>"#;
        let out = sanitize_email_html(input);
        assert!(out.contains("href=\"https://example.com\""));
        // ammonia rewrites every <a> with noopener noreferrer (configured
        // via link_rel) so a tabnabbing payload can't reach window.opener.
        assert!(
            out.contains("noopener"),
            "expected rel=noopener on outbound link: {out}"
        );
    }

    #[test]
    fn preserves_safe_text_formatting() {
        let input = r#"<p>hello <strong>world</strong></p><blockquote><em>quoted</em></blockquote><ul><li>one</li></ul>"#;
        let out = sanitize_email_html(input);
        assert!(out.contains("<strong>world</strong>"));
        assert!(out.contains("<blockquote"));
        assert!(out.contains("<em>quoted</em>"));
        assert!(out.contains("<li>one</li>"));
    }

    #[test]
    fn preserves_safe_table_layout() {
        let input = r#"<table width="600"><tr><td>cell</td></tr></table>"#;
        let out = sanitize_email_html(input);
        assert!(out.contains("<table"));
        assert!(out.contains("<tr"));
        assert!(out.contains("<td"));
        assert!(out.contains("cell"));
    }

    // ---- Defensive ------------------------------------------------------

    #[test]
    fn rendering_empty_body_returns_empty_string() {
        assert_eq!(sanitize_email_html(""), "");
    }

    #[test]
    fn handles_malformed_html_without_panicking() {
        // mailparse can return half-broken HTML on truncated MIME parts.
        // The sanitiser must NOT panic — it should produce something
        // renderable.
        let input = "<p>missing close <a href='https://example.com'>open<unclosed";
        let out = sanitize_email_html(input);
        assert!(!out.contains("<unclosed"));
    }

    #[test]
    fn sanitize_options_default_blocks_remote_images() {
        // The default opt-out is the privacy-aware behaviour. Any new
        // call site that builds SanitizeOptions::default() must inherit
        // remote-image blocking.
        let opts = SanitizeOptions::default();
        assert!(!opts.allow_remote_images);
    }
}
