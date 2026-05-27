pub mod audit;
pub mod domains;
pub mod users;
// Added: Admin CRUD for payment_provider_config (PayPro-style DB-backed credentials).
pub mod payment_providers;
// Added (TMAIL-165): Admin CRUD for runtime feature flags.
pub mod feature_flags;
// Added (TMAIL-183): Admin endpoints for the enterprise_quote_requests inbox.
pub mod quote_requests;
// Added (TMAIL-58): Admin endpoint for global email queue statistics.
pub mod queue;
