// Added (TMAIL-308): Multi-origin CORS parsing with wildcard support.
//
// CORS_ORIGIN env var is now parsed as a comma-separated list. Each entry is
// either an exact origin (e.g. `https://mail.techatscale.io`) or a wildcard
// pattern (e.g. `*.tenants.tasmail.io` or `https://*.tenants.tasmail.io`).
// Wildcard patterns only support a single `*` as the left-most subdomain — this
// covers the per-tenant-subdomain use case without opening up unbounded matching.
//
// Build behaviour:
//   - No wildcards (one or many entries) → `AllowOrigin::list`     (filter + reflect)
//   - Any wildcard entry                  → `AllowOrigin::predicate` (per-request match)
//
// NOTE: Even when only one exact entry is configured, we use `AllowOrigin::list`
// (not `AllowOrigin::exact`). `AllowOrigin::exact` *always* reflects the configured
// value regardless of the request's `Origin` — browsers still enforce SOP, but
// `Vary: Origin` caching is less clean. `list` filters: the header is set only
// when the request Origin actually matches, which is what we want everywhere.

use axum::http::HeaderValue;
use tower_http::cors::AllowOrigin;

/// One parsed CORS_ORIGIN entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorsOriginRule {
    /// Exact-match origin, e.g. `https://mail.techatscale.io`.
    Exact(String),
    /// Subdomain wildcard. `scheme` is `Some("https")` when the pattern includes
    /// a scheme, or `None` to match any scheme. `suffix` includes the leading
    /// dot, e.g. `.tenants.tasmail.io` — so origin `acme.tenants.tasmail.io`
    /// matches because it ends with `.tenants.tasmail.io` and has at least one
    /// label before it.
    Wildcard {
        scheme: Option<String>,
        suffix: String,
    },
}

impl CorsOriginRule {
    /// Return true when this rule matches the given `Origin` header value.
    pub fn matches(&self, origin: &str) -> bool {
        match self {
            CorsOriginRule::Exact(s) => s == origin,
            CorsOriginRule::Wildcard { scheme, suffix } => {
                let (origin_scheme, origin_rest) = match origin.split_once("://") {
                    Some(t) => t,
                    None => return false,
                };
                if let Some(s) = scheme.as_deref() {
                    if !s.eq_ignore_ascii_case(origin_scheme) {
                        return false;
                    }
                }
                // Origin host is everything up to the first ':' (port) or '/' (path).
                let origin_host = origin_rest
                    .split(|c| c == ':' || c == '/')
                    .next()
                    .unwrap_or("");
                origin_host.len() > suffix.len() && origin_host.ends_with(suffix.as_str())
            }
        }
    }
}

/// Parse the raw CORS_ORIGIN value into a list of rules.
///
/// Empty entries (from stray commas / whitespace) are dropped silently.
/// Malformed wildcard patterns (no `*.` prefix on the host) are also dropped
/// rather than panicked — operators get a startup log line from `build_allow_origin`.
pub fn parse_cors_origins(raw: &str) -> Vec<CorsOriginRule> {
    raw.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .filter_map(parse_entry)
        .collect()
}

fn parse_entry(s: &str) -> Option<CorsOriginRule> {
    if s.contains('*') {
        parse_wildcard(s)
    } else {
        Some(CorsOriginRule::Exact(s.to_string()))
    }
}

fn parse_wildcard(s: &str) -> Option<CorsOriginRule> {
    let (scheme, host_part) = match s.split_once("://") {
        Some((sch, host)) => (Some(sch.to_string()), host),
        None => (None, s),
    };
    let suffix = host_part.strip_prefix("*.")?;
    if suffix.is_empty() || suffix.contains('*') {
        // Only a single left-most `*.` is supported — reject `*.*.foo` etc.
        return None;
    }
    Some(CorsOriginRule::Wildcard {
        scheme,
        suffix: format!(".{}", suffix),
    })
}

/// Build the `AllowOrigin` from a raw CORS_ORIGIN string. Falls back to the
/// development localhost SPA origin when the input is empty or every entry is
/// malformed.
pub fn build_allow_origin(raw: &str) -> AllowOrigin {
    let rules = parse_cors_origins(raw);
    if rules.is_empty() {
        tracing::warn!(
            "CORS_ORIGIN parsed to zero valid entries — falling back to http://localhost:5173"
        );
        return AllowOrigin::exact("http://localhost:5173".parse().unwrap());
    }

    let has_wildcard = rules
        .iter()
        .any(|r| matches!(r, CorsOriginRule::Wildcard { .. }));

    if !has_wildcard {
        // Exact-only path: collect HeaderValues. Skip any that fail to parse
        // (e.g. contain non-ASCII or control chars) and log so operators notice.
        let header_values: Vec<HeaderValue> = rules
            .iter()
            .filter_map(|r| match r {
                CorsOriginRule::Exact(s) => match HeaderValue::from_str(s) {
                    Ok(hv) => Some(hv),
                    Err(_) => {
                        tracing::warn!("CORS_ORIGIN entry rejected (invalid header value): {}", s);
                        None
                    }
                },
                _ => None,
            })
            .collect();

        if header_values.is_empty() {
            tracing::warn!(
                "CORS_ORIGIN produced no valid HeaderValues — falling back to http://localhost:5173"
            );
            return AllowOrigin::exact("http://localhost:5173".parse().unwrap());
        }
        // Always use `list` (even for single-entry) so the header is only set
        // when the request Origin actually matches — see module-level note above.
        return AllowOrigin::list(header_values);
    }

    // Wildcards present — use a predicate. Move the parsed rules into the
    // closure so per-request matching is allocation-free in the hot path.
    AllowOrigin::predicate(move |origin: &HeaderValue, _parts| {
        let origin_str = match origin.to_str() {
            Ok(s) => s,
            Err(_) => return false,
        };
        rules.iter().any(|r| r.matches(origin_str))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_cors_origins ---

    #[test]
    fn parses_single_origin() {
        let rules = parse_cors_origins("https://mail.example.com");
        assert_eq!(
            rules,
            vec![CorsOriginRule::Exact("https://mail.example.com".to_string())]
        );
    }

    #[test]
    fn parses_comma_separated_origins() {
        let rules = parse_cors_origins("https://a.example.com,https://b.example.com");
        assert_eq!(rules.len(), 2);
        assert_eq!(
            rules[0],
            CorsOriginRule::Exact("https://a.example.com".to_string())
        );
        assert_eq!(
            rules[1],
            CorsOriginRule::Exact("https://b.example.com".to_string())
        );
    }

    #[test]
    fn parses_with_surrounding_whitespace() {
        let rules =
            parse_cors_origins("  https://a.example.com  ,  https://b.example.com  ");
        assert_eq!(rules.len(), 2);
        assert_eq!(
            rules[0],
            CorsOriginRule::Exact("https://a.example.com".to_string())
        );
    }

    #[test]
    fn skips_empty_entries() {
        let rules = parse_cors_origins("https://a.com,,https://b.com, ,");
        assert_eq!(rules.len(), 2);
    }

    #[test]
    fn parses_wildcard_without_scheme() {
        let rules = parse_cors_origins("*.tenants.tasmail.io");
        assert_eq!(
            rules,
            vec![CorsOriginRule::Wildcard {
                scheme: None,
                suffix: ".tenants.tasmail.io".to_string(),
            }]
        );
    }

    #[test]
    fn parses_wildcard_with_scheme() {
        let rules = parse_cors_origins("https://*.tenants.tasmail.io");
        assert_eq!(
            rules,
            vec![CorsOriginRule::Wildcard {
                scheme: Some("https".to_string()),
                suffix: ".tenants.tasmail.io".to_string(),
            }]
        );
    }

    #[test]
    fn parses_mixed_exact_and_wildcard() {
        let rules = parse_cors_origins(
            "https://mail.techatscale.io, https://*.tenants.tasmail.io, http://localhost:5173",
        );
        assert_eq!(rules.len(), 3);
        assert!(matches!(rules[0], CorsOriginRule::Exact(_)));
        assert!(matches!(rules[1], CorsOriginRule::Wildcard { .. }));
        assert!(matches!(rules[2], CorsOriginRule::Exact(_)));
    }

    #[test]
    fn rejects_double_wildcard_pattern() {
        // `*.*.foo` is not supported — left-most label only.
        let rules = parse_cors_origins("https://*.*.tasmail.io");
        assert!(rules.is_empty());
    }

    #[test]
    fn rejects_wildcard_without_dot_prefix() {
        // `*foo.com` has no `*.` — drop it.
        let rules = parse_cors_origins("*foo.com");
        assert!(rules.is_empty());
    }

    // --- CorsOriginRule::matches ---

    #[test]
    fn exact_match_succeeds_on_identical_origin() {
        let rule = CorsOriginRule::Exact("https://mail.example.com".to_string());
        assert!(rule.matches("https://mail.example.com"));
    }

    #[test]
    fn exact_match_fails_on_different_scheme() {
        let rule = CorsOriginRule::Exact("https://mail.example.com".to_string());
        assert!(!rule.matches("http://mail.example.com"));
    }

    #[test]
    fn exact_match_fails_on_different_host() {
        let rule = CorsOriginRule::Exact("https://mail.example.com".to_string());
        assert!(!rule.matches("https://other.example.com"));
    }

    #[test]
    fn wildcard_matches_subdomain() {
        let rule = CorsOriginRule::Wildcard {
            scheme: Some("https".to_string()),
            suffix: ".tenants.tasmail.io".to_string(),
        };
        assert!(rule.matches("https://acme.tenants.tasmail.io"));
        assert!(rule.matches("https://acme.tenants.tasmail.io:8443"));
        assert!(rule.matches("https://a-b-c.tenants.tasmail.io"));
    }

    #[test]
    fn wildcard_rejects_bare_apex() {
        // `tenants.tasmail.io` (the apex itself) must NOT match `*.tenants.tasmail.io`.
        let rule = CorsOriginRule::Wildcard {
            scheme: Some("https".to_string()),
            suffix: ".tenants.tasmail.io".to_string(),
        };
        assert!(!rule.matches("https://tenants.tasmail.io"));
    }

    #[test]
    fn wildcard_rejects_suffix_attack() {
        // `eviltenants.tasmail.io` ends with `tenants.tasmail.io` but the char
        // before is `l`, not `.`, so it must NOT match `*.tenants.tasmail.io`.
        let rule = CorsOriginRule::Wildcard {
            scheme: Some("https".to_string()),
            suffix: ".tenants.tasmail.io".to_string(),
        };
        assert!(!rule.matches("https://eviltenants.tasmail.io"));
    }

    #[test]
    fn wildcard_rejects_wrong_scheme() {
        let rule = CorsOriginRule::Wildcard {
            scheme: Some("https".to_string()),
            suffix: ".tenants.tasmail.io".to_string(),
        };
        assert!(!rule.matches("http://acme.tenants.tasmail.io"));
    }

    #[test]
    fn wildcard_without_scheme_matches_any_scheme() {
        let rule = CorsOriginRule::Wildcard {
            scheme: None,
            suffix: ".tenants.tasmail.io".to_string(),
        };
        assert!(rule.matches("https://acme.tenants.tasmail.io"));
        assert!(rule.matches("http://acme.tenants.tasmail.io"));
    }

    #[test]
    fn wildcard_rejects_origin_without_scheme() {
        // The Origin header always carries a scheme. A scheme-less value is malformed.
        let rule = CorsOriginRule::Wildcard {
            scheme: None,
            suffix: ".tenants.tasmail.io".to_string(),
        };
        assert!(!rule.matches("acme.tenants.tasmail.io"));
    }

    // --- build_allow_origin smoke tests (we can't easily inspect AllowOrigin
    //     internals, but we can confirm the function returns without panicking
    //     across the supported shapes — the integration tests cover behaviour). ---

    #[test]
    fn build_handles_single_origin() {
        let _ = build_allow_origin("https://mail.example.com");
    }

    #[test]
    fn build_handles_comma_separated() {
        let _ = build_allow_origin("https://a.example.com,https://b.example.com");
    }

    #[test]
    fn build_handles_wildcard_mix() {
        let _ = build_allow_origin(
            "https://mail.example.com,https://*.tenants.tasmail.io,http://localhost:5173",
        );
    }

    #[test]
    fn build_falls_back_on_empty_input() {
        let _ = build_allow_origin("");
        let _ = build_allow_origin("   ,  ");
    }
}
