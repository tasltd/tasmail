pub mod mailbox;
pub use mailbox::MailboxExtractor;
// Alias so both the legacy `crate::extractors::Mailbox` / `crate::Mailbox`
// imports (archive.rs, imap_config.rs, groups.rs, migration.rs, signatures.rs)
// and the newer `MailboxExtractor` form resolve to the same extractor.
pub use mailbox::MailboxExtractor as Mailbox;
