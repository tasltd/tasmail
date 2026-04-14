// Added: Attachment storage model for TMAIL-59
pub mod attachment;
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
// Added: Webhook model for outbound notifications (TMAIL-131)
pub mod webhook;
