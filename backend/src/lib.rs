// Added: Library crate entry point for integration test access to internal modules
// Fix: modules must stay `pub` — the dlp_milter binary and the integration tests
// reach them as `tasmail::models`, `tasmail::services`, and so on.
pub mod config;
// Added (TMAIL-308): Multi-origin CORS parser with wildcard support.
pub mod cors;
pub mod error;
pub mod extractors;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod router;
pub mod services;
pub mod state;
// Added: Centralized input validation module for security hardening (TMAIL-37)
pub mod validation;

pub use crate::error::AppError;
pub use crate::extractors::mailbox::MailboxExtractor;
// Alias so legacy `crate::Mailbox` imports (archive.rs, imap_config.rs) resolve.
pub use crate::extractors::mailbox::MailboxExtractor as Mailbox;
pub use crate::state::AppState;
