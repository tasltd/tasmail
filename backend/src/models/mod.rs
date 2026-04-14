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
