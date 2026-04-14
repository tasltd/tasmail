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
pub mod websocket;
