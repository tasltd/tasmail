// Added: Library crate entry point for integration test access to internal modules
pub mod config;
pub mod error;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod router;
pub mod services;
pub mod state;
// Added: Centralized input validation module for security hardening (TMAIL-37)
pub mod validation;
