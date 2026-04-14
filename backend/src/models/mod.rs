// Added: Attachment storage model for TMAIL-59
pub mod attachment;
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
pub mod quota;
pub mod scheduled_email;
pub mod session;
pub mod sieve_rule;
pub mod snoozed_email;
pub mod shared_mailbox;
pub mod signature;
// Added: WebAuthn credential model for TMAIL-83
pub mod webauthn_credential;
