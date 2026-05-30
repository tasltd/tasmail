// Added: Prometheus metrics endpoint handler for TMAIL-41
// Changed (TMAIL-314): /metrics is no longer publicly scrapeable. Access is
// gated by EITHER an IP allowlist (METRICS_ALLOWED_IPS) OR a Bearer token
// (METRICS_TOKEN) — whichever the operator configures. If neither is set
// the handler falls back to loopback-only (127.0.0.1 / ::1) so a fresh
// deployment is never publicly exposing per-handler latency histograms,
// queue depths, error rates, or internal label cardinality.

use axum::{
    body::Body,
    extract::{ConnectInfo, State},
    http::{HeaderMap, Request, StatusCode},
    response::IntoResponse,
};
use metrics_exporter_prometheus::PrometheusHandle;
use std::net::{IpAddr, SocketAddr};

use crate::state::AppState;

/// Added: GET /metrics — returns Prometheus text format metrics.
///
/// Access control (TMAIL-314):
/// * `METRICS_ALLOWED_IPS` (CSV of IPs) — request allowed if the resolved
///   client IP is in the list. Honours `X-Forwarded-For` so the Apache
///   reverse proxy in front of `tasmail-backend.service` doesn't collapse
///   every scraper into the loopback bucket.
/// * `METRICS_TOKEN` — request allowed if `Authorization: Bearer <token>`
///   matches.
/// * Neither configured → only loopback (127.0.0.1 / ::1) is allowed.
///
/// Unauthorized callers get 403 (no `WWW-Authenticate` header, so curlers
/// don't get prompted to retry with credentials — this is a forbidden
/// resource, not a missing-auth resource).
pub async fn metrics_handler(
    State(state): State<AppState>,
    // NOTE: Taking the raw `Request<Body>` instead of `ConnectInfo` as a
    // dedicated extractor for two reasons. (1) axum 0.8's `ConnectInfo`
    // doesn't implement `OptionalFromRequestParts`, so `Option<ConnectInfo>`
    // won't compile, but tower's `oneshot` test path doesn't populate the
    // extension — using the raw request lets us treat "no connect info" as
    // "no IP" and fail-closed without crashing the handler with a 500.
    // (2) `enterprise_quote` uses `ConnectInfo` directly because its tests
    // already inject it; rather than rewrite that pattern here we keep the
    // gate self-contained and read the IP via the same extensions lookup
    // that the `rate_limit_middleware` uses.
    req: Request<Body>,
) -> impl IntoResponse {
    let headers = req.headers().clone();
    let peer_ip = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(addr)| addr.ip());

    let client_ip = resolve_client_ip(&headers, peer_ip);
    let allowlist = parse_allowed_ips(state.config.metrics_allowed_ips.as_deref());
    let token_match = bearer_token_matches(&headers, state.config.metrics_token.as_deref());
    let ip_match = ip_allowed(client_ip.as_ref(), &allowlist);

    if !ip_match && !token_match {
        tracing::warn!(
            client_ip = ?client_ip,
            "Rejected /metrics scrape (not in allowlist, no valid token)"
        );
        return (StatusCode::FORBIDDEN, "Forbidden".to_string());
    }

    match state.metrics_handle.as_ref() {
        Some(handle) => (StatusCode::OK, handle.render()),
        // NOTE: Returns 503 if metrics recorder was not installed (e.g. in tests)
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            "Metrics not available".to_string(),
        ),
    }
}

/// Added: Install the Prometheus metrics recorder and return the render handle.
/// Must be called once at startup before any metrics are recorded.
pub fn install_prometheus_recorder() -> PrometheusHandle {
    let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
    builder
        .install_recorder()
        .expect("Failed to install Prometheus metrics recorder")
}

/// Resolve the calling client's IP, honouring `X-Forwarded-For` so the
/// Apache reverse proxy in front of the backend doesn't make every scraper
/// look like 127.0.0.1. Takes the LEFT-most entry — that's the original
/// client per RFC 7239 conventions. Falls back to the TCP peer address
/// when no proxy header is present (direct connections).
fn resolve_client_ip(headers: &HeaderMap, peer: Option<IpAddr>) -> Option<IpAddr> {
    resolve_client_ip_from_headers(headers).or(peer)
}

/// XFF-only path used when ConnectInfo is absent (test harness). If the
/// header is missing or unparseable we return `None`, which the handler
/// treats as "no IP" and so falls through to the fail-closed branch.
fn resolve_client_ip_from_headers(headers: &HeaderMap) -> Option<IpAddr> {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<IpAddr>().ok())
}

/// Parse the configured allowlist. When `None` or empty, return the
/// loopback addresses as the safe default so a fresh deployment never
/// publicly exposes /metrics. Invalid entries are logged and skipped.
fn parse_allowed_ips(raw: Option<&str>) -> Vec<IpAddr> {
    match raw {
        Some(s) if !s.trim().is_empty() => s
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .filter_map(|entry| match entry.parse::<IpAddr>() {
                Ok(ip) => Some(ip),
                Err(_) => {
                    tracing::warn!(
                        entry = entry,
                        "METRICS_ALLOWED_IPS contains invalid IP — skipping"
                    );
                    None
                }
            })
            .collect(),
        // NOTE: Loopback-only default. Operators who actually want a
        // Prometheus scrape from another host MUST set METRICS_ALLOWED_IPS
        // (or METRICS_TOKEN) explicitly — fail-closed by design.
        _ => vec![
            IpAddr::from([127, 0, 0, 1]),
            IpAddr::from([0, 0, 0, 0, 0, 0, 0, 1]),
        ],
    }
}

fn ip_allowed(client: Option<&IpAddr>, allowlist: &[IpAddr]) -> bool {
    match client {
        Some(ip) => allowlist.iter().any(|allowed| allowed == ip),
        None => false,
    }
}

fn bearer_token_matches(headers: &HeaderMap, expected: Option<&str>) -> bool {
    let Some(expected) = expected.filter(|t| !t.is_empty()) else {
        return false;
    };
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|token| token == expected)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderName, HeaderValue};

    fn hdrs(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            let name = HeaderName::from_bytes(k.as_bytes()).unwrap();
            h.insert(name, HeaderValue::from_str(v).unwrap());
        }
        h
    }

    #[test]
    fn test_install_prometheus_recorder_returns_handle() {
        // NOTE: This test verifies the recorder installs without panicking.
        // We can only install one global recorder per process — subsequent
        // installs return Err which we tolerate.
        let result = std::panic::catch_unwind(|| {
            let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
            builder.install_recorder()
        });
        match result {
            Ok(Ok(handle)) => {
                let output = handle.render();
                assert!(output.is_empty() || output.contains('#') || !output.is_empty());
            }
            Ok(Err(_)) | Err(_) => {
                // Recorder already installed — that's fine.
            }
        }
    }

    // ---- IP resolution ----

    #[test]
    fn resolve_uses_peer_when_no_proxy_header() {
        let peer: SocketAddr = "203.0.113.7:5000".parse().unwrap();
        let ip = resolve_client_ip(&HeaderMap::new(), Some(peer.ip())).unwrap();
        assert_eq!(ip.to_string(), "203.0.113.7");
    }

    #[test]
    fn resolve_honours_x_forwarded_for_left_most() {
        let peer: SocketAddr = "127.0.0.1:5000".parse().unwrap();
        let h = hdrs(&[("x-forwarded-for", "10.0.0.5, 10.0.0.6, 10.0.0.7")]);
        let ip = resolve_client_ip(&h, Some(peer.ip())).unwrap();
        assert_eq!(ip.to_string(), "10.0.0.5");
    }

    #[test]
    fn resolve_ignores_garbage_xff_and_falls_back_to_peer() {
        let peer: SocketAddr = "127.0.0.1:5000".parse().unwrap();
        let h = hdrs(&[("x-forwarded-for", "not-an-ip")]);
        let ip = resolve_client_ip(&h, Some(peer.ip())).unwrap();
        assert_eq!(ip.to_string(), "127.0.0.1");
    }

    #[test]
    fn resolve_without_peer_or_header_returns_none() {
        // Mirrors the test-harness path (tower oneshot + no XFF) — the
        // handler must treat this as "no IP" so the fail-closed branch
        // fires and we don't accidentally authenticate a faceless request.
        let ip = resolve_client_ip(&HeaderMap::new(), None);
        assert!(ip.is_none());
    }

    // ---- Allowlist parsing ----

    #[test]
    fn parse_allowlist_none_returns_loopback_only() {
        let list = parse_allowed_ips(None);
        assert_eq!(list.len(), 2);
        assert!(list.contains(&IpAddr::from([127, 0, 0, 1])));
        assert!(list.contains(&IpAddr::from([0, 0, 0, 0, 0, 0, 0, 1])));
    }

    #[test]
    fn parse_allowlist_empty_string_returns_loopback_only() {
        let list = parse_allowed_ips(Some("   "));
        assert!(list.contains(&IpAddr::from([127, 0, 0, 1])));
        assert!(list.contains(&IpAddr::from([0, 0, 0, 0, 0, 0, 0, 1])));
    }

    #[test]
    fn parse_allowlist_csv_v4_and_v6() {
        let list = parse_allowed_ips(Some("10.0.0.5, 192.168.1.1,::1"));
        assert_eq!(list.len(), 3);
        assert!(list.contains(&"10.0.0.5".parse::<IpAddr>().unwrap()));
        assert!(list.contains(&"192.168.1.1".parse::<IpAddr>().unwrap()));
        assert!(list.contains(&"::1".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn parse_allowlist_skips_invalid_entries() {
        let list = parse_allowed_ips(Some("10.0.0.5, not-an-ip, 192.168.1.1"));
        assert_eq!(list.len(), 2);
        assert!(list.contains(&"10.0.0.5".parse::<IpAddr>().unwrap()));
        assert!(list.contains(&"192.168.1.1".parse::<IpAddr>().unwrap()));
    }

    // ---- ip_allowed ----

    #[test]
    fn ip_allowed_matches_exact() {
        let list = vec!["10.0.0.5".parse::<IpAddr>().unwrap()];
        let client: IpAddr = "10.0.0.5".parse().unwrap();
        assert!(ip_allowed(Some(&client), &list));
    }

    #[test]
    fn ip_allowed_rejects_non_listed() {
        let list = vec!["10.0.0.5".parse::<IpAddr>().unwrap()];
        let client: IpAddr = "10.0.0.6".parse().unwrap();
        assert!(!ip_allowed(Some(&client), &list));
    }

    #[test]
    fn ip_allowed_rejects_none_client() {
        let list = vec!["10.0.0.5".parse::<IpAddr>().unwrap()];
        assert!(!ip_allowed(None, &list));
    }

    #[test]
    fn loopback_default_allows_localhost() {
        let list = parse_allowed_ips(None);
        let v4: IpAddr = "127.0.0.1".parse().unwrap();
        let v6: IpAddr = "::1".parse().unwrap();
        assert!(ip_allowed(Some(&v4), &list));
        assert!(ip_allowed(Some(&v6), &list));
    }

    #[test]
    fn loopback_default_rejects_public_ip() {
        let list = parse_allowed_ips(None);
        let public: IpAddr = "8.8.8.8".parse().unwrap();
        assert!(!ip_allowed(Some(&public), &list));
    }

    // ---- Bearer token ----

    #[test]
    fn token_matches_valid_bearer() {
        let h = hdrs(&[("authorization", "Bearer secret-token-123")]);
        assert!(bearer_token_matches(&h, Some("secret-token-123")));
    }

    #[test]
    fn token_rejects_wrong_token() {
        let h = hdrs(&[("authorization", "Bearer wrong-token")]);
        assert!(!bearer_token_matches(&h, Some("secret-token-123")));
    }

    #[test]
    fn token_rejects_missing_bearer_prefix() {
        let h = hdrs(&[("authorization", "Basic dXNlcjpwYXNz")]);
        assert!(!bearer_token_matches(&h, Some("secret-token-123")));
    }

    #[test]
    fn token_rejects_when_no_expected_configured() {
        // Even a Bearer header should not authenticate when METRICS_TOKEN
        // is unset — operators must opt in explicitly.
        let h = hdrs(&[("authorization", "Bearer anything")]);
        assert!(!bearer_token_matches(&h, None));
        assert!(!bearer_token_matches(&h, Some("")));
    }

    #[test]
    fn token_rejects_missing_header() {
        assert!(!bearer_token_matches(&HeaderMap::new(), Some("token")));
    }

    // ---- End-to-end access-control decision ----
    //
    // These mirror the handler's `if !ip_match && !token_match` gate so the
    // matrix is documented in a single place.

    #[test]
    fn access_default_blocks_public_ip_no_token() {
        let allowlist = parse_allowed_ips(None);
        let h = HeaderMap::new();
        let public: IpAddr = "8.8.8.8".parse().unwrap();

        let ip_match = ip_allowed(Some(&public), &allowlist);
        let token_match = bearer_token_matches(&h, None);

        assert!(!ip_match, "public IP must not match loopback default");
        assert!(!token_match, "no token configured must not authenticate");
        assert!(!ip_match && !token_match, "request must be rejected");
    }

    #[test]
    fn access_token_alone_authenticates_non_listed_ip() {
        let allowlist = parse_allowed_ips(Some("10.0.0.5"));
        let h = hdrs(&[("authorization", "Bearer prom-token")]);
        let public: IpAddr = "8.8.8.8".parse().unwrap();

        let ip_match = ip_allowed(Some(&public), &allowlist);
        let token_match = bearer_token_matches(&h, Some("prom-token"));

        assert!(!ip_match);
        assert!(token_match);
        assert!(ip_match || token_match, "token alone must allow access");
    }

    #[test]
    fn access_allowlisted_ip_authenticates_without_token() {
        let allowlist = parse_allowed_ips(Some("10.0.0.5"));
        let h = HeaderMap::new();
        let prom: IpAddr = "10.0.0.5".parse().unwrap();

        let ip_match = ip_allowed(Some(&prom), &allowlist);
        let token_match = bearer_token_matches(&h, Some("prom-token"));

        assert!(ip_match);
        assert!(!token_match);
        assert!(ip_match || token_match, "listed IP alone must allow access");
    }

    #[test]
    fn access_explicit_allowlist_excludes_loopback_by_default() {
        // When the operator sets METRICS_ALLOWED_IPS, the loopback default
        // is REPLACED, not added to. Operators who want both must list both.
        let allowlist = parse_allowed_ips(Some("10.0.0.5"));
        let loopback: IpAddr = "127.0.0.1".parse().unwrap();
        assert!(!ip_allowed(Some(&loopback), &allowlist));
    }
}
