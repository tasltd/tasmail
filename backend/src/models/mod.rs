// Added: Attachment storage model for TMAIL-59
pub mod attachment;
// Added: Branding model for white-label customization (TMAIL-111)
pub mod branding;
// Added: Email comment model for TMAIL-128
pub mod email_comment;
pub mod audit_log;
pub mod auto_reply;
pub mod contact;
// Added: Email queue model for TMAIL-58
pub mod email_queue;
pub mod distribution_group;
pub mod domain;
pub mod email_delegation;
pub mod email_template;
pub mod mailbox;
pub mod migration_job;
// Added: Phishing report model for TMAIL-124
pub mod phishing_report;
pub mod quota;
pub mod scheduled_email;
pub mod session;
pub mod sieve_rule;
pub mod snoozed_email;
pub mod shared_mailbox;
pub mod signature;
// Added: Email task model for TMAIL-126
pub mod email_task;
// Added: WebAuthn credential model for TMAIL-83
pub mod webauthn_credential;
// Added: Retention policy and legal hold models for TMAIL-109
pub mod retention_policy;
// Added: Webhook model for outbound notifications (TMAIL-131)
pub mod webhook;
// Added: Custom hostname model for per-tenant SNI configuration (TMAIL-112)
pub mod custom_hostname;
// Added: PST import model for Outlook migration (TMAIL-115)
pub mod pst_import;
// Added: Shared file model for large file sharing via download links (TMAIL-138)
pub mod shared_file;
// Added: Bulk user import model for CSV provisioning (TMAIL-136)
pub mod bulk_import;
// Added: Chat integration model for team chat webhooks (TMAIL-129)
pub mod chat_integration;
// Added: Calendar event and attendee models for meeting scheduling (TMAIL-127)
pub mod calendar_event;
// Added: LDAP/AD configuration and sync log models for TMAIL-100
pub mod ldap_config;
// Added: AI configuration model for BYOK AI integration (TMAIL-105)
pub mod ai_config;
// Added: AI summary cache to avoid re-generating identical summaries (TMAIL-103)
pub mod email_summary_cache;
// Added: SAML 2.0 SSO configuration model for enterprise IdP integration (TMAIL-101)
pub mod saml_config;
// Added: OIDC provider model for Sign in with Google/Microsoft (TMAIL-99)
pub mod oidc_provider;
// Added: Email embedding model for semantic search with pgvector (TMAIL-106)
pub mod email_embedding;
// Added: eDiscovery search model for compliance investigations (TMAIL-137)
pub mod ediscovery;
// Added: DLP rule and violation models for Data Loss Prevention scanning (TMAIL-108)
pub mod dlp_rule;
// Added: NLP search history model for AI-powered natural language search (TMAIL-135)
pub mod nlp_search;
// Added: DANE policy and verification models for TMAIL-125
pub mod dane;
// Added: SMTP configuration model for BYO-SMTP (TMAIL-48)
pub mod smtp_config;
// Added: IMAP configuration model for BYO-IMAP (BYOK webmail pivot)
pub mod imap_config;
// Added: Plugin model for extensible plugin/extension architecture (TMAIL-132)
pub mod plugin;
// Added: Contact group model for organizing contacts into labeled groups (TMAIL-119)
pub mod contact_group;
// Added: POP3 configuration model for Dovecot POP3 access (TMAIL-133)
pub mod pop3_config;
// Added: Email archive model for Piler archiving integration (TMAIL-107)
pub mod archive;
// Added: ActiveSync device and policy models for TMAIL-130
pub mod activesync;
// Added: Ollama local LLM configuration and model cache models (TMAIL-102)
pub mod ollama_config;
// Added: CalDAV/CardDAV configuration model for TMAIL-117
pub mod dav_config;
// Added: Rspamd spam filter settings, quarantine, and stats models (TMAIL-15)
pub mod spam;
// Added: Billing plan, subscription, and payment models for Paystack/MoMo (TMAIL-46)
pub mod billing;
// Added: Email deliverability check models for scored diagnostic reports (TMAIL-39)
pub mod deliverability;
// Added: Mobile-optimized response models for lightweight API payloads (TMAIL-52)
pub mod mobile;
// Added: Push notification device and log models for TMAIL-50
pub mod push_notification;
// Added: Sync checkpoint models for offline-first delta sync protocol (TMAIL-51)
pub mod sync;
// Added: IP warm-up schedule models for TMAIL-17
pub mod warmup;
// Added: PaymentProviderConfig — DB-backed credential storage mirroring PayPro's PaymentProviderConfig.
pub mod payment_provider_config;
// Added (TMAIL-165): Runtime feature flags surfaced in the admin dashboard.
pub mod feature_flag;
// Added (TMAIL-357): Server-side session rows for the /classic no-JS surface.
pub mod classic_session;
// Added (TMAIL-361): One-shot 2FA-challenge rows gating /classic logins for
// users with TOTP enrolled. See `migrations/081_pending_2fa_tokens.sql`.
pub mod pending_2fa_token;
// Added (TMAIL-374): In-progress state for the 3-step /classic signup
// wizard. See `migrations/082_classic_signup_drafts.sql`.
pub mod classic_signup_draft;
// Added (TMAIL-375): One-shot password-reset tokens for the /classic
// no-JS surface. See `migrations/083_password_reset_tokens.sql`.
pub mod password_reset_token;
// Added (TMAIL-386): Per-user / per-sender allowlist for remote `<img>`
// rendering in the Classic UI message view. See
// `migrations/084_remote_image_allowlist.sql`.
pub mod remote_image_allowlist;
