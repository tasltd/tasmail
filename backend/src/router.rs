use axum::{
    middleware as axum_middleware,
    routing::{delete, get, post, put},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::set_header::SetResponseHeaderLayer;
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
            get(handlers::messages::get_message).delete(handlers::messages::delete_message),
        )
        .route("/api/messages/send", post(handlers::messages::send_message))
        .route("/api/drafts", post(handlers::messages::save_draft))
        .route("/api/search", get(handlers::messages::search_messages))
        .route(
            "/api/folders/{folder}/messages/{uid}/move",
            post(handlers::messages::move_message),
        )
        .route(
            "/api/folders/{folder}/messages/{uid}/flag",
            post(handlers::messages::flag_message),
        )
        // Signatures
        .route(
            "/api/signatures",
            get(handlers::signatures::list_signatures).post(handlers::signatures::create_signature),
        )
        .route(
            "/api/signatures/{id}",
            put(handlers::signatures::update_signature).delete(handlers::signatures::delete_signature),
        )
        // Contacts
        .route(
            "/api/contacts",
            get(handlers::contacts::list_contacts).post(handlers::contacts::create_contact),
        )
        .route(
            "/api/contacts/{id}",
            put(handlers::contacts::update_contact).delete(handlers::contacts::delete_contact),
        )
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
        // Audit log
        .route(
            "/api/admin/audit-log",
            get(handlers::admin::audit::list_audit_logs),
        )
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Added: API version header on all responses
    let api_version_layer = SetResponseHeaderLayer::overriding(
        axum::http::header::HeaderName::from_static("x-api-version"),
        axum::http::HeaderValue::from_static("1.0"),
    );

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(api_version_layer)
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}
