// Added (TMAIL-356): Per-request CSP nonce generator for the /classic surface.
// Changed (TMAIL-368): bumped entropy from 16 → 32 bytes per the P0 #14 spec
// ("Fresh nonce per request (32 random bytes, base64)"), and added the
// `from_extensions_or_new` helper so middleware-injected nonces flow through
// `Request`-taking handlers without each call site re-rolling its own.
//
// PURPOSE
// -------
// The Classic UI base layout (templates/classic/base.html) carries a single
// inline `<style nonce="...">` block. The strict CSP planned for /classic/*
// (`style-src 'self' 'nonce-XXX'`, see TMAIL-368) requires the same nonce on
// both the response header and every inline `<style>` / `<script>` it admits.
// This module owns the nonce-value side of that contract.
//
// FORMAT
// ------
// 32 random bytes, base64-standard encoded → 43 visible chars plus 1 `=` pad
// (CSP nonce values are base64; the spec allows pad chars). 32 bytes ≈ 256
// bits of entropy, well above the W3C CSP nonce guidance and OWASP's
// "at least 128 bits" rule. We use the rand 0.9 thread-local `ChaCha`-backed
// CSPRNG seeded from the OS entropy pool — fine for nonces.
//
// HOW IT FLOWS
// ------------
// 1. `security_headers_middleware` calls `CspNonce::new()` once per
//    /classic/* request and inserts it into `request.extensions_mut()` before
//    delegating to the handler.
// 2. Handlers extract it via the `axum::Extension<CspNonce>` extractor (or
//    `req.extensions().get::<CspNonce>()` for handlers that take `Request`
//    directly) and pass the string into their Askama template struct's
//    `csp_nonce: String` field.
// 3. The middleware then assembles the `Content-Security-Policy` response
//    header with the same nonce baked into the `style-src` source list, so
//    the inline `<style nonce="…">` block on base.html is admitted by the
//    browser while every other inline style is rejected.

use axum::http::Extensions;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rand::RngCore;

/// 256-bit cryptographic nonce, base64-encoded, suitable for use in a CSP
/// `'nonce-XXX'` source expression and the matching `<style nonce="XXX">`
/// attribute on inline styles in the Classic UI base layout.
///
/// Wrapping the string in a newtype keeps "this string is a fresh CSP nonce"
/// distinct from "this string is something else". Future code that wants to
/// receive a nonce can take `&CspNonce` and the type system will refuse
/// arbitrary `String`s — which is exactly what you want around CSP, where
/// reusing a stale or attacker-controlled value is the failure mode.
#[derive(Debug, Clone)]
pub struct CspNonce(String);

impl CspNonce {
    /// Generate a fresh 256-bit nonce, base64-encoded.
    pub fn new() -> Self {
        // rand 0.9 deprecated `thread_rng()` → `rng()`. `rng()` returns the
        // same thread-local ChaCha-backed CSPRNG, seeded from the OS entropy
        // pool on first use.
        let mut bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut bytes);
        Self(BASE64.encode(bytes))
    }

    /// Borrow the encoded value (used by the `Content-Security-Policy` header
    /// builder in `security_headers_middleware`).
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume the wrapper and return the owned `String` — used when stuffing
    /// the value into an Askama template struct's `csp_nonce: String` field.
    pub fn into_string(self) -> String {
        self.0
    }

    /// Added (TMAIL-368): pull a request-scoped nonce out of `Extensions` (the
    /// one that `security_headers_middleware` inserted on the way in). If the
    /// extension is missing — which in production should never happen for
    /// `/classic/*` paths, but DOES happen in unit tests that exercise a
    /// handler without the middleware stack — we fall back to a fresh nonce
    /// so the page still renders. The fallback CANNOT silently corrupt CSP in
    /// production because the middleware always inserts before delegating, so
    /// the only path that hits the fallback is the test harness.
    pub fn from_extensions_or_new(ext: &Extensions) -> Self {
        ext.get::<CspNonce>().cloned().unwrap_or_else(CspNonce::new)
    }
}

impl Default for CspNonce {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonce_is_44_chars_of_base64() {
        // 32 raw bytes → base64 STANDARD encoding produces exactly 44 chars
        // (43 sig + 1 `=` pad). Lock the length in so a switch to base64-URL
        // / unpadded engine doesn't slip the format past us silently, and so
        // a future regression of the byte count (back to 16 or up to 64)
        // surfaces here rather than in the CSP header at runtime.
        let n = CspNonce::new();
        assert_eq!(
            n.as_str().len(),
            44,
            "expected 44-char (43+1 padding) base64 string, got {:?}",
            n.as_str()
        );
    }

    #[test]
    fn nonce_chars_are_csp_compatible() {
        // CSP nonce values are base64-encoded per the spec; the standard
        // alphabet (A-Z, a-z, 0-9, +, /, =) is admissible inside the
        // `'nonce-...'` source expression with no further escaping.
        let n = CspNonce::new();
        for ch in n.as_str().chars() {
            assert!(
                ch.is_ascii_alphanumeric() || ch == '+' || ch == '/' || ch == '=',
                "nonce contained non-base64 character {:?} in {:?}",
                ch,
                n.as_str()
            );
        }
    }

    #[test]
    fn nonces_are_unique_per_call() {
        // Defence in depth: a nonce that repeats across requests defeats
        // its purpose. Generate a batch and assert they're all distinct.
        // With 256 bits of entropy, the chance of two collisions in 1000
        // draws is astronomically low — a collision here means the RNG is
        // broken or someone replaced `thread_rng` with a constant.
        let mut seen = std::collections::HashSet::new();
        for _ in 0..1_000 {
            assert!(
                seen.insert(CspNonce::new().into_string()),
                "duplicate nonce generated within the same batch"
            );
        }
    }

    #[test]
    fn into_string_and_as_str_round_trip() {
        let n = CspNonce::new();
        let borrowed = n.as_str().to_string();
        let owned = n.into_string();
        assert_eq!(borrowed, owned);
    }

    #[test]
    fn from_extensions_returns_inserted_nonce() {
        // Added (TMAIL-368): the middleware inserts a CspNonce into the
        // request extensions; handlers that read it via this helper MUST
        // get the SAME value back — if they got a fresh nonce, the header
        // and the inline `<style nonce="…">` would diverge and the browser
        // would block every CSS rule on the page.
        let mut ext = Extensions::new();
        let injected = CspNonce::new();
        let injected_str = injected.as_str().to_string();
        ext.insert(injected);

        let pulled = CspNonce::from_extensions_or_new(&ext);
        assert_eq!(
            pulled.as_str(),
            injected_str,
            "from_extensions_or_new must return the previously-inserted nonce"
        );
    }

    #[test]
    fn from_extensions_falls_back_to_fresh_when_missing() {
        // Test-harness fallback: handlers exercised without the middleware
        // still need a renderable page. The fallback is fine here because
        // there's no CSP header pinning the value; in production the
        // middleware always pre-inserts so this branch is unreachable.
        let ext = Extensions::new();
        let n = CspNonce::from_extensions_or_new(&ext);
        assert_eq!(n.as_str().len(), 44, "fallback nonce must be 44 chars");
    }
}
