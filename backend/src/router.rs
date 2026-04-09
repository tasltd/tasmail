use axum::{
    middleware as axum_middleware,
    routing::{delete, get, post},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::handlers;
use crate::middleware::auth::auth_middleware;
use crate::state::AppState;

pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Public routes (no auth required)
    let public_routes = Router::new()
        .route("/api/health", get(handlers::health::health_check))
        .route("/api/auth/login", post(handlers::auth::login))
        .route("/api/auth/refresh", post(handlers::auth::refresh));

    // Protected routes (auth required)
    let protected_routes = Router::new()
        .route("/api/auth/logout", post(handlers::auth::logout))
        .route("/api/folders", get(handlers::folders::list_folders))
        .route(
            "/api/folders/{folder}/messages",
            get(handlers::messages::list_messages),
        )
        .route(
            "/api/folders/{folder}/messages/{uid}",
            get(handlers::messages::get_message),
        )
        .route("/api/messages/send", post(handlers::messages::send_message))
        // Admin routes
        .route(
            "/api/admin/domains",
            get(handlers::admin::domains::list_domains).post(handlers::admin::domains::create_domain),
        )
        .route(
            "/api/admin/domains/{id}",
            delete(handlers::admin::domains::delete_domain),
        )
        .route(
            "/api/admin/users",
            get(handlers::admin::users::list_users).post(handlers::admin::users::create_user),
        )
        .route(
            "/api/admin/users/{id}",
            delete(handlers::admin::users::delete_user),
        )
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}
