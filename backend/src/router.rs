use axum::{
    middleware as axum_middleware,
    routing::{delete, get, post, put},
    Router,
};
use std::sync::Arc;
use tower_http::compression::CompressionLayer;
// Changed: Replaced `Any` origin with explicit allowed origins for CORS hardening (TMAIL-37)
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

use crate::handlers;
use crate::middleware::auth::auth_middleware;
// Added: Prometheus request instrumentation middleware import (TMAIL-41)
use crate::middleware::metrics::metrics_middleware;
// Added: Per-IP rate-limiter applied to anonymous auth endpoints (TMAIL-37).
// Uses an in-memory sliding-window counter; tunable via env vars below.
use crate::middleware::rate_limit::{rate_limit_middleware, RateLimiter};
// Added: Security headers middleware import (TMAIL-37)
use crate::middleware::security_headers::security_headers_middleware;
use crate::state::AppState;

pub fn create_router(state: AppState) -> Router {
    // Changed: Restrict CORS to same-origin by default; configurable via CORS_ORIGIN env var (TMAIL-37)
    // NOTE: In production, set CORS_ORIGIN to the exact frontend URL (e.g. "https://mail.example.com")
    let allowed_origin = std::env::var("CORS_ORIGIN")
        .unwrap_or_else(|_| "http://localhost:5173".to_string());

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::exact(
            allowed_origin.parse().unwrap_or_else(|_| {
                "http://localhost:5173".parse().unwrap()
            }),
        ))
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

    // Added: TMAIL-37 — strict per-IP rate limit on anonymous auth endpoints.
    // Defaults: 10 requests / 60s / IP. Override via env (AUTH_RATE_LIMIT_MAX /
    // AUTH_RATE_LIMIT_WINDOW). Memory-safe: the cleanup task purges expired
    // entries every 60s so the HashMap can't grow without bound.
    let auth_rl_max: u32 = std::env::var("AUTH_RATE_LIMIT_MAX")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    let auth_rl_window: u64 = std::env::var("AUTH_RATE_LIMIT_WINDOW")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);
    let auth_rate_limiter = RateLimiter::new(auth_rl_max, auth_rl_window);
    Arc::new(auth_rate_limiter.clone()).start_cleanup();

    // Auth routes — rate-limited per IP so brute-force login and signup flooding
    // are blocked before they reach the password-hashing path (which is intentionally
    // CPU-expensive and would otherwise be a DoS vector).
    let auth_routes = Router::new()
        .route("/api/auth/login", post(handlers::auth::login))
        // Added: BYOK signup endpoint — public, returns JWT pair on success.
        .route("/api/auth/signup", post(handlers::auth::signup))
        .route("/api/auth/refresh", post(handlers::auth::refresh))
        .layer(axum_middleware::from_fn(rate_limit_middleware))
        .layer(axum::Extension(auth_rate_limiter));

    // Public routes (no auth required)
    // NOTE: WebSocket route is public — auth is handled via token query param during handshake
    let public_routes = Router::new()
        .route("/api/health", get(handlers::health::health_check))
        .route("/ws", get(handlers::websocket::ws_handler))
        // Added: Public branding endpoint — frontend needs it before login (TMAIL-111)
        .route("/api/branding", get(handlers::branding::get_branding))
        // TMAIL-165: public subset of feature flags (no auth) so the SPA can decide
        // which signup/onboarding paths to show before the user is logged in.
        .route("/api/feature-flags", get(handlers::admin::feature_flags::list_public_flags))
        // TMAIL-182: public enterprise quote-request endpoint (rate-limited per IP).
        .route("/api/enterprise/quote-request", post(handlers::enterprise_quote::submit_quote_request))
        // Added: Public download endpoint for shared files (TMAIL-138)
        .route("/api/dl/{token}", get(handlers::shared_files::download_by_token))
        // Added: Public SAML SSO login and callback routes for TMAIL-101
        .route("/api/auth/saml/{id}/login", get(handlers::saml::saml_login))
        .route("/api/auth/saml/callback", post(handlers::saml::saml_callback))
        // Added: Public OIDC login routes for Sign in with Google/Microsoft (TMAIL-99)
        .route("/api/auth/oidc/providers", get(handlers::oidc::list_login_providers))
        .route("/api/auth/oidc/{id}/authorize", get(handlers::oidc::get_authorize_url))
        .route("/api/auth/oidc/callback", post(handlers::oidc::oidc_callback))
        // Added: Prometheus metrics endpoint (TMAIL-41)
        .route("/metrics", get(handlers::metrics::metrics_handler))
        // Added: Public billing routes — plan listing and payment webhooks (TMAIL-46)
        .route("/api/billing/plans", get(handlers::billing::list_plans))
        .route("/api/billing/webhook/paystack", post(handlers::billing::webhook_paystack))
        // Changed: MoMo webhook removed — TASMail mirrors PayPro (Paystack/Mastercard/Cybersource/Bank). Mastercard webhook replaces it.
        .route("/api/billing/webhook/mastercard", post(handlers::billing::webhook_mastercard));

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
        // Added: PayPro-style payment_provider_config CRUD (TMAIL-46 follow-up).
        .route(
            "/api/admin/payment-providers",
            get(handlers::admin::payment_providers::list_providers)
                .post(handlers::admin::payment_providers::create_provider),
        )
        .route(
            "/api/admin/payment-providers/{id}",
            delete(handlers::admin::payment_providers::archive_provider),
        )
        // TMAIL-165: admin CRUD for runtime feature flags.
        .route(
            "/api/admin/feature-flags",
            get(handlers::admin::feature_flags::list_flags),
        )
        .route(
            "/api/admin/feature-flags/{key}",
            axum::routing::patch(handlers::admin::feature_flags::update_flag),
        )
        // TMAIL-178/179: usage-based billing dashboard endpoints.
        .route("/api/billing/usage", get(handlers::usage_billing::get_usage))
        .route("/api/billing/invoices", get(handlers::usage_billing::list_invoices))
        // TMAIL-183: admin endpoints for the enterprise quote-request inbox.
        .route(
            "/api/admin/quote-requests",
            get(handlers::admin::quote_requests::list_quote_requests),
        )
        .route(
            "/api/admin/quote-requests/stats",
            get(handlers::admin::quote_requests::quote_request_stats),
        )
        .route(
            "/api/admin/quote-requests/{id}",
            get(handlers::admin::quote_requests::get_quote_request)
                .patch(handlers::admin::quote_requests::update_quote_request),
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
        // Added: Admin global queue stats for TMAIL-58
        .route(
            "/api/admin/queue-stats",
            get(handlers::admin::queue::admin_queue_stats),
        )
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
        // Added: User CSV export endpoint (TMAIL-136 — companion to bulk-import)
        .route(
            "/api/admin/users/export",
            get(handlers::bulk_import::export_users_csv),
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
        // Added: BYO-IMAP configuration management routes (BYOK webmail pivot).
        .route(
            "/api/imap-configs",
            get(handlers::imap_config::list_imap_configs)
                .post(handlers::imap_config::create_imap_config),
        )
        .route(
            "/api/imap-configs/{id}",
            delete(handlers::imap_config::delete_imap_config),
        )
        .route(
            "/api/imap-configs/test",
            post(handlers::imap_config::test_imap),
        )
        .route(
            "/api/imap-configs/presets",
            get(handlers::imap_config::list_provider_presets),
        )
        // TMAIL-167: managed-mailbox provisioning (DNS-MX onboarding).
        .route(
            "/api/mailbox/provision",
            post(handlers::mailbox_provision::provision_managed_mailbox),
        )
        // Added: BYO-SMTP configuration management routes for TMAIL-48
        .route(
            "/api/smtp-configs",
            get(handlers::smtp_config::list_smtp_configs)
                .post(handlers::smtp_config::create_smtp_config),
        )
        .route(
            "/api/smtp-configs/{id}",
            get(handlers::smtp_config::get_smtp_config)
                .put(handlers::smtp_config::update_smtp_config)
                .delete(handlers::smtp_config::delete_smtp_config),
        )
        .route(
            "/api/smtp-configs/{id}/test",
            post(handlers::smtp_config::test_smtp_config),
        )
        .route(
            "/api/smtp-configs/{id}/default",
            post(handlers::smtp_config::set_default_smtp),
        )
        // Added: Plugin management routes for extensible plugin architecture (TMAIL-132)
        .route(
            "/api/plugins",
            get(handlers::plugins::list_plugins).post(handlers::plugins::create_plugin),
        )
        .route(
            "/api/plugins/{id}",
            get(handlers::plugins::get_plugin)
                .put(handlers::plugins::update_plugin)
                .delete(handlers::plugins::delete_plugin),
        )
        .route(
            "/api/plugins/{id}/executions",
            get(handlers::plugins::list_executions),
        )
        .route(
            "/api/plugins/{id}/test",
            post(handlers::plugins::test_plugin),
        )
        // Added: Contact group management routes for TMAIL-119
        .route(
            "/api/contact-groups",
            get(handlers::contact_groups::list_groups)
                .post(handlers::contact_groups::create_group),
        )
        .route(
            "/api/contact-groups/{id}",
            put(handlers::contact_groups::update_group)
                .delete(handlers::contact_groups::delete_group),
        )
        .route(
            "/api/contact-groups/{id}/members",
            post(handlers::contact_groups::add_member),
        )
        .route(
            "/api/contact-groups/{id}/members/{contact_id}",
            delete(handlers::contact_groups::remove_member),
        )
        .route(
            "/api/contact-groups/{id}/contacts",
            get(handlers::contact_groups::list_group_contacts),
        )
        // Added: vCard import/export and contact merge routes for TMAIL-119
        .route(
            "/api/contacts/import-vcard",
            post(handlers::contact_groups::import_vcard),
        )
        .route(
            "/api/contacts/export-vcard",
            get(handlers::contact_groups::export_vcard),
        )
        .route(
            "/api/contacts/merge",
            post(handlers::contact_groups::merge_contacts),
        )
        // Added: POP3 configuration management routes for Dovecot POP3 access (TMAIL-133)
        .route(
            "/api/pop3/config",
            get(handlers::pop3_config::get_pop3_config)
                .put(handlers::pop3_config::update_pop3_config)
                .delete(handlers::pop3_config::delete_pop3_config),
        )
        .route(
            "/api/pop3/status",
            get(handlers::pop3_config::get_pop3_status),
        )
        // Added: Email archive policy and config management routes for Piler integration (TMAIL-107)
        .route(
            "/api/admin/archive/policies",
            get(handlers::archive::list_policies).post(handlers::archive::create_policy),
        )
        .route(
            "/api/admin/archive/policies/{id}",
            put(handlers::archive::update_policy).delete(handlers::archive::delete_policy),
        )
        .route(
            "/api/admin/archive/config",
            get(handlers::archive::get_config).put(handlers::archive::update_config),
        )
        // Added: User archive search and history routes (TMAIL-107)
        .route(
            "/api/archive/search",
            post(handlers::archive::search_archive),
        )
        .route(
            "/api/archive/search/history",
            get(handlers::archive::search_history),
        )
        // Added: ActiveSync device management routes for TMAIL-130
        .route(
            "/api/activesync/devices",
            get(handlers::activesync::list_devices)
                .post(handlers::activesync::register_device),
        )
        .route(
            "/api/activesync/devices/{id}",
            delete(handlers::activesync::delete_device),
        )
        .route(
            "/api/activesync/devices/{id}/block",
            post(handlers::activesync::block_device),
        )
        .route(
            "/api/activesync/devices/{id}/allow",
            post(handlers::activesync::allow_device),
        )
        .route(
            "/api/activesync/devices/{id}/wipe",
            post(handlers::activesync::wipe_device),
        )
        // Added: ActiveSync policy management routes (admin) for TMAIL-130
        .route(
            "/api/admin/activesync/policies",
            get(handlers::activesync::list_policies)
                .post(handlers::activesync::create_policy),
        )
        .route(
            "/api/admin/activesync/policies/{id}",
            put(handlers::activesync::update_policy)
                .delete(handlers::activesync::delete_policy),
        )
        // Added: Ollama local LLM management routes for TMAIL-102
        .route(
            "/api/admin/ollama/config",
            get(handlers::ollama::get_config).put(handlers::ollama::update_config),
        )
        .route(
            "/api/admin/ollama/status",
            get(handlers::ollama::get_status),
        )
        .route(
            "/api/admin/ollama/models/pull",
            post(handlers::ollama::pull_model),
        )
        .route(
            "/api/admin/ollama/models/{name}",
            delete(handlers::ollama::delete_model),
        )
        .route(
            "/api/admin/ollama/models",
            get(handlers::ollama::list_cached_models),
        )
        // Added: CalDAV/CardDAV configuration management routes for TMAIL-117
        .route(
            "/api/dav/configs",
            get(handlers::dav_config::list_dav_configs)
                .post(handlers::dav_config::create_dav_config),
        )
        .route(
            "/api/dav/configs/{id}",
            get(handlers::dav_config::get_dav_config)
                .put(handlers::dav_config::update_dav_config)
                .delete(handlers::dav_config::delete_dav_config),
        )
        .route(
            "/api/dav/configs/{id}/sync",
            post(handlers::dav_config::sync_dav_config),
        )
        .route(
            "/api/dav/configs/{id}/test",
            post(handlers::dav_config::test_dav_config),
        )
        // Added: Rspamd spam filter management routes for TMAIL-15
        .route(
            "/api/spam/settings",
            get(handlers::spam::get_settings).put(handlers::spam::update_settings),
        )
        .route(
            "/api/spam/quarantine",
            get(handlers::spam::list_quarantine),
        )
        .route(
            "/api/spam/quarantine/{id}/release",
            post(handlers::spam::release_quarantine),
        )
        .route(
            "/api/spam/quarantine/{id}",
            delete(handlers::spam::delete_quarantine),
        )
        .route(
            "/api/spam/learn",
            post(handlers::spam::learn_message),
        )
        .route(
            "/api/spam/stats",
            get(handlers::spam::get_stats),
        )
        // Added: Protected billing routes for subscription and payment management (TMAIL-46)
        .route(
            "/api/billing/subscription",
            get(handlers::billing::get_subscription),
        )
        .route(
            "/api/billing/subscribe",
            post(handlers::billing::subscribe),
        )
        .route(
            "/api/billing/payments",
            get(handlers::billing::list_payments),
        )
        // Added: Email deliverability check route for TMAIL-39
        .route(
            "/api/admin/deliverability/check",
            get(handlers::deliverability::check_deliverability),
        )
        // Added: Mobile-optimized endpoints for lower bandwidth and smaller payloads (TMAIL-52)
        .route("/api/mobile/inbox", get(handlers::mobile::mobile_inbox))
        .route(
            "/api/mobile/message/{folder}/{uid}",
            get(handlers::mobile::mobile_message),
        )
        .route("/api/mobile/folders", get(handlers::mobile::mobile_folders))
        .route(
            "/api/mobile/unread-count",
            get(handlers::mobile::mobile_unread_count),
        )
        .route("/api/mobile/batch", post(handlers::mobile::mobile_batch))
        .route(
            "/api/mobile/sync",
            get(handlers::mobile::mobile_sync).post(handlers::mobile::mobile_sync_post),
        )
        // Added: Lightweight data quota for mobile dashboards (TMAIL-52)
        .route("/api/mobile/usage", get(handlers::mobile::mobile_usage))
        // Added: Push notification device management routes for TMAIL-50
        .route("/api/push/register", post(handlers::push::register_device))
        .route("/api/push/devices", get(handlers::push::list_devices))
        .route(
            "/api/push/devices/{id}",
            delete(handlers::push::unregister_device),
        )
        .route("/api/push/test", post(handlers::push::test_notification))
        // Added: TMAIL-50 — quiet hours + badge count sync for push devices
        .route(
            "/api/push/devices/{id}/quiet-hours",
            put(handlers::push::update_quiet_hours),
        )
        .route(
            "/api/push/devices/{id}/badge",
            put(handlers::push::update_badge_count),
        )
        // Added: Sync checkpoint routes for offline-first delta sync (TMAIL-51)
        .route(
            "/api/sync/checkpoints",
            get(handlers::sync::list_checkpoints),
        )
        .route(
            "/api/sync/checkpoint/{folder}",
            get(handlers::sync::get_checkpoint).post(handlers::sync::update_checkpoint),
        )
        .route(
            "/api/sync/resolve-conflict",
            post(handlers::sync::resolve_conflict),
        )
        // Added: IP warm-up schedule management routes for TMAIL-17
        .route(
            "/api/admin/warmup/status",
            get(handlers::warmup::get_warmup_status),
        )
        .route(
            "/api/admin/warmup/schedule",
            get(handlers::warmup::get_warmup_schedule),
        )
        .route(
            "/api/admin/warmup/start",
            post(handlers::warmup::start_warmup),
        )
        // Added: Cache management routes for Redis admin operations
        .route(
            "/api/admin/cache/status",
            get(handlers::cache::get_cache_status),
        )
        .route(
            "/api/admin/cache/flush",
            post(handlers::cache::flush_cache),
        )
        .route(
            "/api/admin/cache/stats",
            get(handlers::cache::get_cache_stats),
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

    // Added: Multi-algorithm compression for low-bandwidth mobile clients (TMAIL-52).
    // CompressionLayer::new() negotiates gzip, brotli, and deflate based on the
    // client's Accept-Encoding header. Brotli wins for text payloads (JSON, HTML);
    // gzip stays the safe fallback for older clients.
    Router::new()
        .merge(auth_routes)
        .merge(public_routes)
        .merge(protected_routes)
        .layer(
            CompressionLayer::new()
                .gzip(true)
                .br(true)
                .deflate(true),
        )
        .layer(api_version_layer)
        // Added: Prometheus request instrumentation on all routes (TMAIL-41)
        .layer(axum_middleware::from_fn(metrics_middleware))
        // Added: Security headers (CSP, HSTS, X-Frame-Options, etc.) on all responses (TMAIL-37)
        .layer(axum_middleware::from_fn(security_headers_middleware))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}
