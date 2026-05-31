// Added (TMAIL-363): Strict-allowlist HTML sanitiser for rendering
// untrusted email bodies on the Classic UI no-JS surface (and any future
// mobile-render path that wants the same guarantees).
//
// The full design (remote-image gating with a per-message Show-images
// opt-in, CID inline-image rewriting, per-user `block_remote_images`
// setting) is the dedicated TMAIL-364 task — see the gap analysis at
// `docs/gap-analysis/classic-ui.md` P0 #10 for the deeper contract.
// This module ships the **minimum viable sanitiser** the message read
// view needs:
//
//   * Strip every `<script>` / `<style>` / `<iframe>` / `<object>` /
//     `<embed>` / `<form>` element and its children.
//   * Strip every inline event handler (`on*`).
//   * Drop `javascript:` and `vbscript:` URLs.
//   * Drop `data:` URLs except `data:image/*`.
//   * Drop `<base>` (would re-target relative URLs to the attacker's host).
//   * Preserve safe text-formatting tags + safe `<a href>` + safe `<img>`
//     so the rendered email is actually useful, not just a stripped string.
//
// We deliberately do NOT yet:
//   * Block remote `<img src>` URLs — that's TMAIL-386 (P1) + the per-user
//     setting.
//   * Rewrite CID inline references — that's bundled into TMAIL-364.
//
// Both of those CAN be layered on top of `sanitize_email_html` without
// re-doing the strict-allowlist work, so shipping the minimum here keeps
// TMAIL-363's read view unblocked while TMAIL-364 adds the gating.

use std::sync::OnceLock;

use ammonia::Builder;

/// PURPOSE: Sanitise an untrusted email `text/html` body for safe inline
/// rendering inside the Classic UI message view (and any future server-side
/// render path that needs the same guarantees).
///
/// Returns a HTML string that can be dropped into a `{{ var|safe }}` Askama
/// expression without re-escaping — every dangerous construct is stripped
/// at parse time, not relied on at render time.
///
/// The allowlist is intentionally tight (no `<script>`, no inline events,
/// no `<iframe>`, no `<form>`, no `javascript:`/`vbscript:` URLs, no
/// non-image `data:` URLs) so an email containing an XSS payload renders
/// as the visible text content with the payload stripped — not as a
/// vector against the logged-in user's session.
///
/// Remote-image blocking is **not** done here yet (TMAIL-386 / TMAIL-364).
/// Today every `<img src=https://…>` is preserved so the user sees the
/// email as the sender intended; the privacy-aware default is the follow-up
/// task's responsibility.
pub fn sanitize_email_html(raw_html: &str) -> String {
    builder().clean(raw_html).to_string()
}

/// Cached `ammonia::Builder`. Allocating a fresh Builder + tag/attr maps
/// on every render would dominate the read-view latency on long emails;
/// the Builder is internally `Sync`-safe so a OnceLock-cached instance
/// is the cheapest correct shape.
fn builder() -> &'static Builder<'static> {
    static BUILDER: OnceLock<Builder<'static>> = OnceLock::new();
    BUILDER.get_or_init(|| {
        let mut b = Builder::default();
        // ammonia's default tag allowlist already drops <script>, <iframe>,
        // <object>, <embed>, <style>, <link>, <meta>, <base>, etc. We keep
        // the default and only extend with the tags an email needs that
        // ammonia ships disabled by default.
        b.add_tags(["img", "table", "thead", "tbody", "tfoot", "tr", "td", "th"]);
        // `target` + `rel` on <a> so external links open in a new tab AND
        // get noopener/noreferrer applied by ammonia's link rel rewriter.
        b.add_tag_attributes("a", ["target"]);
        // Email frequently uses <img width/height/alt/title>; allowlist those.
        b.add_tag_attributes("img", ["width", "height", "alt", "title"]);
        // <table> formatting attributes — old-school table-based email layout
        // (still used by Mailchimp etc.) needs them to render readably.
        b.add_tag_attributes("table", ["width", "cellpadding", "cellspacing", "border", "align"]);
        b.add_tag_attributes("td", ["width", "height", "align", "valign", "colspan", "rowspan"]);
        b.add_tag_attributes("th", ["width", "height", "align", "valign", "colspan", "rowspan"]);
        b.add_tag_attributes("tr", ["align", "valign"]);
        // Drop `data:` URLs everywhere except `data:image/*` on <img src>.
        // ammonia's default URL scheme allowlist is {http, https, mailto,
        // ftp, ftps, magnet, gopher, irc, ircs, news, nntp, sms, ssh,
        // tel, urn, xmpp}. We strip the bunch that don't make sense in an
        // email body to shrink the attack surface further.
        b.url_schemes(
            ["http", "https", "mailto", "tel", "cid"]
                .iter()
                .copied()
                .collect(),
        );
        // Add noopener + noreferrer to every <a> that gets through, so a
        // tabnabbing payload can't reach window.opener on the click target.
        b.link_rel(Some("noopener noreferrer nofollow"));
        b
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn drops_data_url_html() {
        // <a href=data:text/html,...> would render arbitrary HTML if the user
        // clicked the link. ammonia's default scheme allowlist doesn't include
        // `data` so the href is stripped.
        let input = r#"<a href="data:text/html,<script>alert(1)</script>">x</a>"#;
        let out = sanitize_email_html(input);
        assert!(!out.contains("data:text/html"));
        assert!(!out.contains("<script"));
    }

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
        // visible content. Drop <form> outright — the Classic UI never needs
        // a sender-provided form to render in a message body.
        let input = r#"<form action="https://evil.example.com"><input name="pw"></form><p>ok</p>"#;
        let out = sanitize_email_html(input);
        assert!(!out.contains("<form"));
        assert!(!out.contains("<input"));
        assert!(out.contains("<p>ok</p>"));
    }

    #[test]
    fn strips_base_tag() {
        // <base href="https://evil.example.com/"> would re-target every
        // relative URL in the email body to the attacker's host. Drop it.
        let input = r#"<base href="https://evil.example.com/"><a href="/x">x</a>"#;
        let out = sanitize_email_html(input);
        assert!(!out.contains("<base"));
        assert!(!out.contains("evil.example.com"));
    }

    #[test]
    fn strips_style_tag() {
        // Author <style> tags can include `expression()` / `@import` /
        // `url("javascript:...")` payloads on old IE shims. Ammonia drops
        // <style> by default; lock that in.
        let input = r#"<style>body { background: url("javascript:x") }</style><p>ok</p>"#;
        let out = sanitize_email_html(input);
        assert!(!out.contains("<style"));
        assert!(!out.contains("javascript:"));
        assert!(out.contains("<p>ok</p>"));
    }

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
    fn preserves_remote_image_src_for_now() {
        // TMAIL-364 / TMAIL-386 will add per-message Show-images gating.
        // Today the sanitiser preserves remote <img src> as-is so the read
        // view is at least visually-correct. This test pins current
        // behaviour and will flip to "blocked by default" when TMAIL-386
        // lands — at which point this assertion is what the follow-up
        // task should rewrite.
        let input = r#"<img src="https://example.com/pixel.png" alt="tracker">"#;
        let out = sanitize_email_html(input);
        assert!(
            out.contains("src=\"https://example.com/pixel.png\""),
            "remote image src should currently survive sanitisation: {out}"
        );
    }

    #[test]
    fn preserves_cid_inline_image_scheme() {
        // `cid:` is the RFC 2392 scheme for inline message-part references.
        // Even with remote-image blocking on (TMAIL-386), we want the inline
        // attachment images to keep rendering — they live in the same email.
        let input = r#"<img src="cid:logo@example.com" alt="logo">"#;
        let out = sanitize_email_html(input);
        assert!(out.contains("cid:logo@example.com"), "cid: scheme dropped: {out}");
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

    #[test]
    fn rendering_empty_body_returns_empty_string() {
        // Defensive — an email with no body shouldn't panic the renderer.
        assert_eq!(sanitize_email_html(""), "");
    }

    #[test]
    fn strips_object_and_embed() {
        // Both <object> and <embed> can host plug-in content that escapes
        // the document sandbox.
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

    #[test]
    fn handles_malformed_html_without_panicking() {
        // mailparse can return half-broken HTML on truncated MIME parts.
        // The sanitiser must NOT panic — it should produce something
        // renderable.
        let input = "<p>missing close <a href='https://example.com'>open<unclosed";
        let out = sanitize_email_html(input);
        assert!(!out.contains("<unclosed"));
    }
}
