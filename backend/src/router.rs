pub mod admin;
pub mod auth;
pub mod features;
pub mod mail;
pub mod public;
pub mod user;

use axum::{
    middleware as axum_middleware,
    Router,
};
use std::sync::Arc;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

use crate::cors::build_allow_origin;
use crate::middleware::auth::auth_middleware;
use crate::middleware::metrics::metrics_middleware;
use crate::middleware::rate_limit::{rate_limit_middleware, RateLimiter};
use crate::middleware::rls_context::rls_context_middleware;
use crate::middleware::security_headers::security_headers_middleware;
use crate::state::AppState;

pub fn create_router(state: AppState) -> Router<AppState> {
    // CORS configuration
    let allowed_origin_raw = std::env::var("CORS_ORIGIN")
        .unwrap_or_else(|_| "http://localhost:5173".to_string());

    let cors = CorsLayer::new()
        .allow_origin(build_allow_origin(&allowed_origin_raw))
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
            axum::http::header::ACCEPT,
        ])
        .allow_credentials(true);

    // Rate limiter for auth endpoints
    let auth_rl_max: u32 = std::env::var("AUTH_RATE_LIMIT_MAX")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    let auth_rl_window: u64 = std::env::var("AUTH_RATE_LIMIT_WINDOW")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);
    let auth_rl_bypass = std::env::var("TASMAIL_E2E_TEST_MODE")
        .ok()
        .map(|v| v == "true")
        .unwrap_or(false);
    let auth_rate_limiter = RateLimiter::new(auth_rl_max, auth_rl_window, auth_rl_bypass);
    Arc::new(auth_rate_limiter.clone()).start_cleanup();

    // Build the complete router with available routes
    let app = Router::new()
        // Public routes (no authentication required)
        .route("/api/health", axum::routing::get(crate::handlers::health::health_check))
        .route("/api/health/live", axum::routing::get(crate::handlers::health::liveness))
        .route("/api/health/ready", axum::routing::get(crate::handlers::health::readiness))
        .route("/api/branding", axum::routing::get(crate::handlers::branding::get_branding))
        // Auth routes (rate-limited)
        .route("/api/auth/login", axum::routing::post(crate::handlers::auth::login))
        .route("/api/auth/signup", axum::routing::post(crate::handlers::auth::signup))
        .route("/api/auth/refresh", axum::routing::post(crate::handlers::auth::refresh))
        // Mail routes (require authentication)
        .route("/api/folders", axum::routing::get(crate::handlers::folders::list_folders))
        .route("/api/folders/{folder}", axum::routing::delete(crate::handlers::folders::delete_folder))
        // Signatures
        .route("/api/signatures", axum::routing::get(crate::handlers::signatures::list_signatures))
        .route("/api/signatures", axum::routing::post(crate::handlers::signatures::create_signature))
        // Contacts
        .route("/api/contacts", axum::routing::get(crate::handlers::contacts::list_contacts))
        // Calendar
        .route("/api/calendar/events", axum::routing::get(crate::handlers::calendar::list_events))
        .route("/api/calendar/free-busy", axum::routing::post(crate::handlers::calendar::get_free_busy))
        // Metrics
        .route("/metrics", axum::routing::get(crate::handlers::metrics::metrics_handler))
        // Quota
        .route("/api/quota", axum::routing::get(crate::handlers::quota::get_quota))
        // Rate limiter middleware
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            rls_context_middleware,
        ))
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        // Global layers
        .layer(
            CompressionLayer::new()
                .gzip(true)
                .br(true)
                .deflate(true),
        )
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::HeaderName::from_static("x-api-version"),
            axum::http::HeaderValue::from_static("1.0"),
        ))
        .layer(axum_middleware::from_fn(metrics_middleware))
        .layer(axum_middleware::from_fn(security_headers_middleware))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        // Auth rate limiter
        .layer(axum_middleware::from_fn(rate_limit_middleware));

    app
}