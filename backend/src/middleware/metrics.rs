// Added: Prometheus request instrumentation middleware for TMAIL-41
// NOTE: Records http_requests_total (counter) and http_request_duration_seconds (histogram)
// per request, using the matched route pattern for low-cardinality labels.

use axum::{
    extract::{MatchedPath, Request},
    middleware::Next,
    response::Response,
};
use std::time::Instant;

/// Added: Middleware that instruments every HTTP request with Prometheus metrics.
/// Records request count and duration with method, path pattern, and status labels.
pub async fn metrics_middleware(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    // NOTE: Extract the matched route pattern (e.g. "/api/folders/{folder}/messages")
    // instead of the actual path to avoid high-cardinality label explosion
    let path = request
        .extensions()
        .get::<MatchedPath>()
        .map(|mp| mp.as_str().to_owned())
        .unwrap_or_else(|| "unknown".to_string());

    let start = Instant::now();

    let response = next.run(request).await;

    let duration = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();
    let method_str = method.to_string();

    // Added: Increment request counter with method, path, and status labels
    metrics::counter!(
        "http_requests_total",
        "method" => method_str.clone(),
        "path" => path.clone(),
        "status" => status
    )
    .increment(1);

    // Added: Record request duration histogram with method and path labels
    metrics::histogram!(
        "http_request_duration_seconds",
        "method" => method_str,
        "path" => path
    )
    .record(duration);

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        middleware as axum_middleware,
        routing::get,
        Router,
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    // Added: Helper handler for testing
    async fn test_handler() -> &'static str {
        "ok"
    }

    // Added: Helper handler that returns 404
    async fn not_found_handler() -> (axum::http::StatusCode, &'static str) {
        (axum::http::StatusCode::NOT_FOUND, "not found")
    }

    #[tokio::test]
    async fn test_metrics_middleware_passes_through() {
        // NOTE: Verify the middleware doesn't alter the response
        let app = Router::new()
            .route("/test", get(test_handler))
            .layer(axum_middleware::from_fn(metrics_middleware));

        let request = Request::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"ok");
    }

    #[tokio::test]
    async fn test_metrics_middleware_preserves_error_status() {
        // NOTE: Verify the middleware preserves non-200 status codes
        let app = Router::new()
            .route("/missing", get(not_found_handler))
            .layer(axum_middleware::from_fn(metrics_middleware));

        let request = Request::builder()
            .uri("/missing")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_metrics_middleware_handles_unmatched_path() {
        // NOTE: When no route matches, MatchedPath extension is absent;
        // middleware should still work without panicking
        let app = Router::new()
            .route("/exists", get(test_handler))
            .layer(axum_middleware::from_fn(metrics_middleware));

        let request = Request::builder()
            .uri("/nonexistent")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        // NOTE: Axum returns 404 for unmatched routes
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
    }
}
