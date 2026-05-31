pub mod auth;
// Added: Prometheus request instrumentation middleware (TMAIL-41)
pub mod metrics;
pub mod rate_limit;
// Added: TMAIL-309 — per-request RLS context tower layer that primes the
// `RlsConn` extractor with claims + state for lazy connection acquisition.
pub mod rls_context;
// Added: Security headers middleware for OWASP compliance (TMAIL-37)
pub mod security_headers;
// Added (TMAIL-357): Cookie-based session middleware for the /classic no-JS surface.
pub mod classic_session;
// Added (TMAIL-358): Per-session CSRF synchroniser-token validator for the
// /classic no-JS surface. Layers AFTER classic_session_middleware so the
// session row (carrying the expected token) is available in extensions.
pub mod classic_csrf;
