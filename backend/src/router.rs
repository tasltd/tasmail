use axum::{
    middleware as axum_middleware,
    routing::{delete, get, post, put},
    Router,
};
use tower_http::compression::CompressionLayer;
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
    // NOTE: WebSocket route is public — auth is handled via token query param during handshake
    let public_routes = Router::new()
        .route("/api/health", get(handlers::health::health_check))
        .route("/api/auth/login", post(handlers::auth::login))
        .route("/api/auth/refresh", post(handlers::auth::refresh))
        .route("/ws", get(handlers::websocket::ws_handler));

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
        // Added: EML export (download) and import (upload) for TMAIL-68
        .route(
            "/api/folders/{folder}/messages/{uid}/eml",
            get(handlers::eml::export_eml),
        )
        .route(
            "/api/folders/{folder}/import-eml",
            post(handlers::eml::import_eml),
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
        // Scheduled / undo-send
        .route("/api/messages/schedule", post(handlers::scheduled::schedule_send))
        .route("/api/messages/scheduled", get(handlers::scheduled::list_scheduled))
        .route(
            "/api/messages/cancel/{cancel_token}",
            post(handlers::scheduled::cancel_scheduled),
        )
        // Auto-reply / vacation responder
        .route(
            "/api/auto-reply",
            get(handlers::auto_reply::get_auto_reply).put(handlers::auto_reply::set_auto_reply),
        )
        // Two-factor authentication
        .route("/api/2fa/enroll", post(handlers::two_factor::enroll))
        .route("/api/2fa/verify", post(handlers::two_factor::verify))
        .route("/api/2fa/status", get(handlers::two_factor::status))
        .route("/api/2fa", delete(handlers::two_factor::disable))
        // SMS OTP
        .route("/api/sms-otp/enroll", post(handlers::sms_otp::enroll))
        .route("/api/sms-otp/verify", post(handlers::sms_otp::verify))
        .route("/api/sms-otp/status", get(handlers::sms_otp::status))
        .route("/api/sms-otp/resend", post(handlers::sms_otp::resend))
        .route("/api/sms-otp", delete(handlers::sms_otp::disable))
        // Quota
        .route("/api/quota", get(handlers::quota::get_quota))
        .route("/api/quota/sync", post(handlers::quota::sync_quota))
        // Distribution groups
        .route(
            "/api/groups",
            get(handlers::groups::list_groups).post(handlers::groups::create_group),
        )
        .route(
            "/api/groups/{id}",
            get(handlers::groups::get_group)
                .put(handlers::groups::update_group)
                .delete(handlers::groups::delete_group),
        )
        .route(
            "/api/groups/{id}/members",
            get(handlers::groups::list_members).post(handlers::groups::add_member),
        )
        .route(
            "/api/groups/{id}/members/{address}",
            delete(handlers::groups::remove_member),
        )
        // Migration (IMAP/MBOX import)
        .route("/api/migration", get(handlers::migration::list_migrations))
        .route("/api/migration/imap", post(handlers::migration::start_imap_migration))
        .route("/api/migration/mbox", post(handlers::migration::start_mbox_import))
        .route("/api/migration/{id}", get(handlers::migration::get_migration))
        .route("/api/migration/{id}/cancel", post(handlers::migration::cancel_migration))
        // Shared mailboxes
        .route(
            "/api/shared-mailboxes",
            get(handlers::shared::list_accessible),
        )
        .route(
            "/api/shared-mailboxes/{mailbox_id}/acl",
            get(handlers::shared::list_acl).post(handlers::shared::grant_access),
        )
        .route(
            "/api/shared-mailboxes/{mailbox_id}/acl/{user_id}",
            delete(handlers::shared::revoke_access),
        )
        // Added: Email delegation
        .route(
            "/api/delegation",
            get(handlers::delegation::list_delegations).post(handlers::delegation::grant_delegation),
        )
        .route(
            "/api/delegation/granted",
            get(handlers::delegation::list_granted),
        )
        .route(
            "/api/delegation/{id}",
            delete(handlers::delegation::revoke_delegation),
        )
        // Added: Email snooze
        .route("/api/messages/snooze", post(handlers::snooze::snooze_message))
        .route("/api/messages/snoozed", get(handlers::snooze::list_snoozed))
        .route(
            "/api/messages/snooze/{id}",
            delete(handlers::snooze::cancel_snooze),
        )
        // Added: Email templates
        .route(
            "/api/templates",
            get(handlers::templates::list_templates).post(handlers::templates::create_template),
        )
        .route(
            "/api/templates/{id}",
            put(handlers::templates::update_template).delete(handlers::templates::delete_template),
        )
        .route(
            "/api/templates/{id}/render",
            post(handlers::templates::render_template),
        )
        // Added: Sieve filter rules
        .route(
            "/api/filters",
            get(handlers::sieve::list_rules).post(handlers::sieve::create_rule),
        )
        .route(
            "/api/filters/{id}",
            put(handlers::sieve::update_rule).delete(handlers::sieve::delete_rule),
        )
        .route("/api/filters/reorder", post(handlers::sieve::reorder_rules))
        // Added: WebAuthn/FIDO2 passkey routes for TMAIL-83
        .route("/api/webauthn/register/begin", post(handlers::webauthn::register_begin))
        .route("/api/webauthn/register/complete", post(handlers::webauthn::register_complete))
        .route("/api/webauthn/authenticate/begin", post(handlers::webauthn::authenticate_begin))
        .route("/api/webauthn/authenticate/complete", post(handlers::webauthn::authenticate_complete))
        .route("/api/webauthn/credentials", get(handlers::webauthn::list_credentials))
        .route("/api/webauthn/credentials/{id}", delete(handlers::webauthn::delete_credential))
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

    // Added: Gzip compression for low-bandwidth optimization
    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(CompressionLayer::new())
        .layer(api_version_layer)
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}
