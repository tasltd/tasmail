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
        .route("/ws", get(handlers::websocket::ws_handler))
        // Added: Public branding endpoint — frontend needs it before login (TMAIL-111)
        .route("/api/branding", get(handlers::branding::get_branding))
        // Added: Public download endpoint for shared files (TMAIL-138)
        .route("/api/dl/{token}", get(handlers::shared_files::download_by_token))
        // Added: Public SAML SSO login and callback routes for TMAIL-101
        .route("/api/auth/saml/{id}/login", get(handlers::saml::saml_login))
        .route("/api/auth/saml/callback", post(handlers::saml::saml_callback))
        // Added: Public OIDC login routes for Sign in with Google/Microsoft (TMAIL-99)
        .route("/api/auth/oidc/providers", get(handlers::oidc::list_login_providers))
        .route("/api/auth/oidc/{id}/authorize", get(handlers::oidc::get_authorize_url))
        .route("/api/auth/oidc/callback", post(handlers::oidc::oidc_callback));

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
        // Added: Email comments on messages for TMAIL-128
        .route(
            "/api/folders/{folder}/messages/{uid}/comments",
            get(handlers::comments::list_comments).post(handlers::comments::create_comment),
        )
        .route(
            "/api/comments/{id}",
            put(handlers::comments::update_comment).delete(handlers::comments::delete_comment),
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
        // Added: PST import routes for Outlook migration (TMAIL-115)
        .route("/api/migration/pst/upload", post(handlers::pst_import::upload_pst))
        .route("/api/migration/pst", get(handlers::pst_import::list_pst_imports))
        .route(
            "/api/migration/pst/{id}",
            get(handlers::pst_import::get_pst_import).delete(handlers::pst_import::delete_pst_import),
        )
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
        // Added: Attachment storage routes for TMAIL-59
        .route(
            "/api/attachments",
            get(handlers::attachments::list_attachments).post(handlers::attachments::upload_attachment),
        )
        .route(
            "/api/attachments/stats",
            get(handlers::attachments::attachment_stats),
        )
        .route(
            "/api/attachments/{id}/download",
            get(handlers::attachments::download_attachment),
        )
        .route(
            "/api/attachments/{id}",
            delete(handlers::attachments::delete_attachment),
        )
        // Added: Phishing scan and report routes for TMAIL-124
        .route(
            "/api/folders/{folder}/messages/{uid}/phishing",
            get(handlers::phishing::get_phishing_report),
        )
        .route(
            "/api/folders/{folder}/messages/{uid}/phishing/scan",
            post(handlers::phishing::scan_message),
        )
        .route(
            "/api/phishing/{id}/action",
            put(handlers::phishing::update_action),
        )
        // Added: Email queue management routes for TMAIL-58
        .route("/api/queue", get(handlers::queue::list_queue))
        .route("/api/queue/stats", get(handlers::queue::queue_stats))
        .route("/api/queue/{id}", delete(handlers::queue::cancel_queued))
        .route("/api/queue/{id}/retry", post(handlers::queue::retry_queued))
        // Added: Email tasks/to-do routes for TMAIL-126
        .route(
            "/api/tasks",
            get(handlers::tasks::list_tasks).post(handlers::tasks::create_task),
        )
        .route(
            "/api/tasks/{id}",
            get(handlers::tasks::get_task)
                .put(handlers::tasks::update_task)
                .delete(handlers::tasks::delete_task),
        )
        // Added: Outbound webhook management routes for TMAIL-131
        .route(
            "/api/webhooks",
            get(handlers::webhooks::list_webhooks).post(handlers::webhooks::create_webhook),
        )
        .route(
            "/api/webhooks/{id}",
            get(handlers::webhooks::get_webhook)
                .put(handlers::webhooks::update_webhook)
                .delete(handlers::webhooks::delete_webhook),
        )
        .route(
            "/api/webhooks/{id}/deliveries",
            get(handlers::webhooks::list_deliveries),
        )
        // Added: Admin branding management routes (TMAIL-111)
        .route("/api/admin/branding", put(handlers::branding::update_branding))
        .route("/api/admin/branding/reset", post(handlers::branding::reset_branding))
        // Added: Retention policy management routes for TMAIL-109
        .route(
            "/api/admin/retention",
            get(handlers::retention::list_retention_policies)
                .post(handlers::retention::create_retention_policy),
        )
        .route(
            "/api/admin/retention/{id}",
            put(handlers::retention::update_retention_policy)
                .delete(handlers::retention::delete_retention_policy),
        )
        // Added: Legal hold management routes for TMAIL-109
        .route(
            "/api/admin/legal-holds",
            get(handlers::retention::list_legal_holds)
                .post(handlers::retention::create_legal_hold),
        )
        .route(
            "/api/admin/legal-holds/{id}/release",
            put(handlers::retention::release_legal_hold),
        )
        // Added: Custom hostname management routes for per-tenant SNI (TMAIL-112)
        .route(
            "/api/admin/hostnames",
            get(handlers::custom_hostnames::list_hostnames)
                .post(handlers::custom_hostnames::create_hostname),
        )
        .route(
            "/api/admin/hostnames/{id}",
            get(handlers::custom_hostnames::get_hostname)
                .put(handlers::custom_hostnames::update_hostname)
                .delete(handlers::custom_hostnames::delete_hostname),
        )
        .route(
            "/api/admin/hostnames/{id}/verify",
            post(handlers::custom_hostnames::verify_hostname),
        )
        // Added: Shared file management routes for large file sharing (TMAIL-138)
        .route(
            "/api/shared-files/upload",
            post(handlers::shared_files::upload_shared_file),
        )
        .route(
            "/api/shared-files",
            get(handlers::shared_files::list_shared_files),
        )
        .route(
            "/api/shared-files/{id}",
            get(handlers::shared_files::get_shared_file)
                .delete(handlers::shared_files::delete_shared_file),
        )
        // Added: Bulk user import routes for CSV provisioning (TMAIL-136)
        .route(
            "/api/admin/users/bulk-import",
            post(handlers::bulk_import::upload_bulk_csv),
        )
        .route(
            "/api/admin/users/bulk-imports",
            get(handlers::bulk_import::list_bulk_imports),
        )
        .route(
            "/api/admin/users/bulk-imports/{id}",
            get(handlers::bulk_import::get_bulk_import),
        )
        .route(
            "/api/admin/users/bulk-import/template",
            get(handlers::bulk_import::download_template),
        )
        // Added: Chat integration management routes for TMAIL-129
        .route(
            "/api/chat-integrations",
            get(handlers::chat_integrations::list_chat_integrations)
                .post(handlers::chat_integrations::create_chat_integration),
        )
        .route(
            "/api/chat-integrations/{id}",
            get(handlers::chat_integrations::get_chat_integration)
                .put(handlers::chat_integrations::update_chat_integration)
                .delete(handlers::chat_integrations::delete_chat_integration),
        )
        .route(
            "/api/chat-integrations/{id}/test",
            post(handlers::chat_integrations::test_chat_integration),
        )
        // Added: Calendar event management routes for meeting scheduling (TMAIL-127)
        .route(
            "/api/calendar/events",
            get(handlers::calendar::list_events).post(handlers::calendar::create_event),
        )
        .route(
            "/api/calendar/events/{id}",
            get(handlers::calendar::get_event)
                .put(handlers::calendar::update_event)
                .delete(handlers::calendar::cancel_event),
        )
        .route(
            "/api/calendar/events/{id}/rsvp",
            post(handlers::calendar::rsvp_event),
        )
        .route(
            "/api/calendar/events/{id}/ics",
            get(handlers::calendar::download_ics),
        )
        // Added: LDAP/AD configuration management routes for TMAIL-100
        .route(
            "/api/admin/ldap",
            get(handlers::ldap::list_ldap_configs).post(handlers::ldap::create_ldap_config),
        )
        .route(
            "/api/admin/ldap/{id}",
            put(handlers::ldap::update_ldap_config).delete(handlers::ldap::delete_ldap_config),
        )
        .route(
            "/api/admin/ldap/{id}/sync",
            post(handlers::ldap::trigger_sync),
        )
        .route(
            "/api/admin/ldap/{id}/logs",
            get(handlers::ldap::list_sync_logs),
        )
        // Added: AI configuration management routes for BYOK AI integration (TMAIL-105)
        .route(
            "/api/ai/config",
            get(handlers::ai_config::list_ai_configs).post(handlers::ai_config::create_ai_config),
        )
        .route(
            "/api/ai/config/{id}",
            put(handlers::ai_config::update_ai_config).delete(handlers::ai_config::delete_ai_config),
        )
        .route(
            "/api/ai/config/{id}/test",
            post(handlers::ai_config::test_ai_config),
        )
        .route(
            "/api/ai/summarize",
            post(handlers::ai_config::summarize_email),
        )
        // Added: Smart reply generation route for TMAIL-104
        .route(
            "/api/ai/smart-reply",
            post(handlers::ai_config::smart_reply),
        )
        // Added: Thread/conversation summarization route for TMAIL-103
        .route(
            "/api/ai/thread-summary",
            post(handlers::ai_config::thread_summary),
        )
        // Added: AI compose (full draft generation) route for TMAIL-134
        .route(
            "/api/ai/compose",
            post(handlers::ai_config::compose_email),
        )
        // Added: SAML 2.0 SSO admin configuration routes for TMAIL-101
        .route(
            "/api/admin/saml",
            get(handlers::saml::list_saml_configs).post(handlers::saml::create_saml_config),
        )
        .route(
            "/api/admin/saml/{id}",
            put(handlers::saml::update_saml_config).delete(handlers::saml::delete_saml_config),
        )
        // Added: Admin OIDC provider management routes for TMAIL-99
        .route(
            "/api/admin/oidc",
            get(handlers::oidc::list_oidc_providers).post(handlers::oidc::create_oidc_provider),
        )
        .route(
            "/api/admin/oidc/{id}",
            put(handlers::oidc::update_oidc_provider).delete(handlers::oidc::delete_oidc_provider),
        )
        // Added: Semantic search routes for pgvector similarity search (TMAIL-106)
        .route(
            "/api/search/semantic",
            post(handlers::semantic_search::semantic_search),
        )
        .route(
            "/api/search/index",
            post(handlers::semantic_search::index_email),
        )
        .route(
            "/api/search/index/stats",
            get(handlers::semantic_search::index_stats),
        )
        // Added: eDiscovery search routes for compliance investigations (TMAIL-137)
        .route(
            "/api/admin/ediscovery",
            get(handlers::ediscovery::list_searches)
                .post(handlers::ediscovery::create_search),
        )
        .route(
            "/api/admin/ediscovery/{id}",
            get(handlers::ediscovery::get_search)
                .delete(handlers::ediscovery::delete_search),
        )
        .route(
            "/api/admin/ediscovery/{id}/execute",
            post(handlers::ediscovery::execute_search),
        )
        .route(
            "/api/admin/ediscovery/{id}/export",
            post(handlers::ediscovery::export_results),
        )
        // Added: NLP search routes for AI-powered natural language email search (TMAIL-135)
        .route(
            "/api/search/nlp",
            post(handlers::nlp_search::nlp_search),
        )
        .route(
            "/api/search/nlp/history",
            get(handlers::nlp_search::list_nlp_history)
                .delete(handlers::nlp_search::clear_nlp_history),
        )
        // Added: DLP rule and violation management routes for TMAIL-108
        .route(
            "/api/admin/dlp/rules",
            get(handlers::dlp::list_rules).post(handlers::dlp::create_rule),
        )
        .route(
            "/api/admin/dlp/rules/{id}",
            put(handlers::dlp::update_rule).delete(handlers::dlp::delete_rule),
        )
        .route(
            "/api/admin/dlp/violations",
            get(handlers::dlp::list_violations),
        )
        .route(
            "/api/admin/dlp/scan",
            post(handlers::dlp::test_scan),
        )
        // Added: DANE policy and verification routes for TMAIL-125
        .route(
            "/api/admin/dane",
            get(handlers::dane::list_policies).post(handlers::dane::create_policy),
        )
        .route(
            "/api/admin/dane/{id}",
            delete(handlers::dane::delete_policy),
        )
        .route(
            "/api/admin/dane/lookup",
            post(handlers::dane::lookup_tlsa),
        )
        .route(
            "/api/dane/verifications",
            get(handlers::dane::list_verifications),
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
