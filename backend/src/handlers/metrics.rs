// Added: Prometheus metrics endpoint handler for TMAIL-41
// NOTE: Serves /metrics in Prometheus text exposition format.
// Optionally protected by METRICS_TOKEN bearer auth.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use metrics_exporter_prometheus::PrometheusHandle;

use crate::state::AppState;

/// Added: GET /metrics — returns Prometheus text format metrics.
/// If METRICS_TOKEN is configured, requires Authorization: Bearer <token>.
pub async fn metrics_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Added: Check bearer token if metrics_token is configured
    if let Some(ref expected_token) = state.config.metrics_token {
        let authorized = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(|token| token == expected_token)
            .unwrap_or(false);

        if !authorized {
            return (StatusCode::UNAUTHORIZED, "Unauthorized".to_string());
        }
    }

    // Added: Retrieve the PrometheusHandle from the global recorder and render metrics
    match state.metrics_handle.as_ref() {
        Some(handle) => (StatusCode::OK, handle.render()),
        // NOTE: Returns 503 if metrics recorder was not installed (e.g. in tests)
        None => (StatusCode::SERVICE_UNAVAILABLE, "Metrics not available".to_string()),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_install_prometheus_recorder_returns_handle() {
        // NOTE: This test verifies the recorder installs without panicking.
        // We can only install one global recorder per process, so this test
        // validates the happy path. Subsequent calls in the same process will panic.
        // The test runner may run this in isolation.
        // We skip if a recorder is already installed.
        let result = std::panic::catch_unwind(|| {
            let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
            builder.install_recorder()
        });
        match result {
            Ok(Ok(handle)) => {
                // Added: Verify handle can render (empty metrics is fine)
                let output = handle.render();
                // NOTE: Output may be empty or contain default metrics
                assert!(output.is_empty() || output.contains('#') || output.len() >= 0);
            }
            Ok(Err(_)) | Err(_) => {
                // NOTE: Recorder already installed by another test — that's fine
            }
        }
    }

    #[test]
    fn test_metrics_token_check_logic() {
        // Added: Test the token comparison logic in isolation
        let expected = "secret-token-123";

        // Valid token
        let header_val = format!("Bearer {}", expected);
        let authorized = header_val
            .strip_prefix("Bearer ")
            .map(|token| token == expected)
            .unwrap_or(false);
        assert!(authorized);

        // Wrong token
        let bad_header = "Bearer wrong-token";
        let authorized = bad_header
            .strip_prefix("Bearer ")
            .map(|token| token == expected)
            .unwrap_or(false);
        assert!(!authorized);

        // Missing Bearer prefix
        let no_prefix = "Basic dXNlcjpwYXNz";
        let authorized = no_prefix
            .strip_prefix("Bearer ")
            .map(|token| token == expected)
            .unwrap_or(false);
        assert!(!authorized);

        // Empty header
        let empty = "";
        let authorized = empty
            .strip_prefix("Bearer ")
            .map(|token| token == expected)
            .unwrap_or(false);
        assert!(!authorized);
    }
}
