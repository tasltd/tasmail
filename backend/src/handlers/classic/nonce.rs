// Added (TMAIL-356): Per-request CSP nonce generator for the /classic surface.
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
// 16 random bytes, base64-standard encoded → 22 visible chars plus 2 `=` pads
// (CSP nonce values are base64; the spec allows pad chars). 16 bytes ≈ 128
// bits of entropy, which matches the W3C CSP nonce guidance and OWASP's
// "at least 128 bits" rule. We use `rand::thread_rng` which is the
// `ChaCha`-backed CSPRNG seeded from the OS entropy pool — fine for nonces.
//
// HOW IT FLOWS
// ------------
// 1. The handler calls `CspNonce::new()` once per request.
// 2. The nonce string is set as a field on the Askama template struct
//    (`csp_nonce: String`) and rendered into the `<style nonce="{{...}}">`
//    attribute on base.html.
// 3. TMAIL-368 will wire the same value into the response header by storing
//    `CspNonce` in a request extension on the way in and reading it in the
//    security_headers middleware on the way out. That refactor lands in the
//    follow-up; this module only exposes the constructor today.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rand::RngCore;

/// 128-bit cryptographic nonce, base64-encoded, suitable for use in a CSP
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
    /// Generate a fresh 128-bit nonce, base64-encoded.
    pub fn new() -> Self {
        // rand 0.9 deprecated `thread_rng()` → `rng()`. `rng()` returns the
        // same thread-local ChaCha-backed CSPRNG, seeded from the OS entropy
        // pool on first use.
        let mut bytes = [0u8; 16];
        rand::rng().fill_bytes(&mut bytes);
        Self(BASE64.encode(bytes))
    }

    /// Borrow the encoded value (for templates that take `&str` / for hand-built
    /// CSP header strings in TMAIL-368).
    ///
    /// Marked `dead_code`-allowed because it has no caller until TMAIL-368
    /// wires the CSP middleware. Keeping it part of the public API now means
    /// that follow-up doesn't need to touch this module just to expose a
    /// borrowing accessor.
    #[allow(dead_code)]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume the wrapper and return the owned `String` — used when stuffing
    /// the value into an Askama template struct's `csp_nonce: String` field.
    pub fn into_string(self) -> String {
        self.0
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
    fn nonce_is_22_to_24_chars_of_base64() {
        // 16 raw bytes → base64 STANDARD encoding produces exactly 24 chars
        // (22 sig + 2 `=` pad). Lock the length in so a switch to base64-URL
        // / unpadded engine doesn't slip the format past us silently.
        let n = CspNonce::new();
        assert_eq!(
            n.as_str().len(),
            24,
            "expected 24-char (22+2 padding) base64 string, got {:?}",
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
        // With 128 bits of entropy, the chance of two collisions in 1000
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
}
