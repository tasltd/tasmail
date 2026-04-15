pub mod admin;
// Added: Attachment upload/download/stats handlers for TMAIL-59
pub mod attachments;
// Added: Branding handlers for white-label customization (TMAIL-111)
pub mod branding;
pub mod auth;
pub mod auto_reply;
// Added: Email comment handlers for TMAIL-128
pub mod comments;
pub mod contacts;
pub mod delegation;
// Added: Email queue management handlers for TMAIL-58
pub mod queue;
// Added: EML import/export handlers for TMAIL-68
pub mod eml;
pub mod folders;
pub mod groups;
pub mod health;
pub mod messages;
// Added: Prometheus metrics endpoint handler (TMAIL-41)
pub mod metrics;
// Added: Phishing scan and report handlers for TMAIL-124
pub mod phishing;
pub mod migration;
pub mod quota;
pub mod scheduled;
pub mod shared;
pub mod sms_otp;
pub mod sieve;
pub mod signatures;
pub mod snooze;
// Added: Email task/to-do handlers for TMAIL-126
pub mod tasks;
pub mod templates;
pub mod two_factor;
// Added: WebAuthn/FIDO2 passkey handlers for TMAIL-83
pub mod webauthn;
// Added: Retention policy and legal hold handlers for TMAIL-109
pub mod retention;
// Added: Webhook management handlers for TMAIL-131
pub mod webhooks;
// Added: Custom hostname management handlers for per-tenant SNI (TMAIL-112)
pub mod custom_hostnames;
// Added: PST import handlers for Outlook migration (TMAIL-115)
pub mod pst_import;
// Added: Shared file upload/download handlers for large file sharing (TMAIL-138)
pub mod shared_files;
// Added: Bulk user import handlers for CSV provisioning (TMAIL-136)
pub mod bulk_import;
// Added: Chat integration management handlers for TMAIL-129
pub mod chat_integrations;
// Added: Calendar event handlers for meeting scheduling (TMAIL-127)
pub mod calendar;
// Added: LDAP/AD configuration management handlers for TMAIL-100
pub mod ldap;
// Added: AI configuration management handlers for BYOK AI integration (TMAIL-105)
pub mod ai_config;
// Added: SAML 2.0 SSO configuration and authentication handlers (TMAIL-101)
pub mod saml;
// Added: OIDC identity provider handlers for Sign in with Google/Microsoft (TMAIL-99)
pub mod oidc;
// Added: Semantic search handlers for pgvector similarity search (TMAIL-106)
pub mod semantic_search;
// Added: eDiscovery search handlers for compliance investigations (TMAIL-137)
pub mod ediscovery;
// Added: DLP rule and violation management handlers for TMAIL-108
pub mod dlp;
// Added: NLP search handlers for AI-powered natural language email search (TMAIL-135)
pub mod nlp_search;
// Added: DANE policy and verification handlers for TMAIL-125
pub mod dane;
pub mod websocket;
// Added: SMTP configuration management handlers for BYO-SMTP (TMAIL-48)
pub mod smtp_config;
// Added: Plugin management handlers for extensible plugin architecture (TMAIL-132)
pub mod plugins;
// Added: Contact group and vCard import/export/merge handlers (TMAIL-119)
pub mod contact_groups;
// Added: POP3 configuration management handlers for Dovecot POP3 access (TMAIL-133)
pub mod pop3_config;
// Added: Email archive handlers for Piler integration (TMAIL-107)
pub mod archive;
// Added: ActiveSync device management handlers for TMAIL-130
pub mod activesync;
// Added: Ollama local LLM management handlers for TMAIL-102
pub mod ollama;
// Added: CalDAV/CardDAV configuration management handlers for TMAIL-117
pub mod dav_config;
// Added: Rspamd spam filter management handlers for TMAIL-15
pub mod spam;
// Added: Email deliverability testing handler for TMAIL-39
pub mod deliverability;
// Added: Billing and payment handlers for Paystack/MoMo integration (TMAIL-46)
pub mod billing;
