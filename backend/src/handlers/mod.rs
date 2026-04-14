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
pub mod websocket;
