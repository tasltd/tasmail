// Added: Attachment storage and ClamAV scanning service for TMAIL-59
pub mod attachment_service;
pub mod auth_service;
pub mod email_scheduler;
pub mod imap_service;
// Added (TMAIL-302): IMAP IDLE bridge — per-folder persistent IMAP connections
// that push new-mail / unread-update events into the WebSocket event channel.
pub mod imap_idle_bridge;
// Added: Phishing scanner heuristic service for TMAIL-124
pub mod phishing_scanner;
// Added: Queue processor background service for TMAIL-58
pub mod queue_processor;
// Added (TMAIL-310): Shared liveness heartbeat for the queue processor —
// read by the /api/health readiness probe.
pub mod queue_heartbeat;
pub mod sms_service;
pub mod smtp_service;
pub mod totp_service;
// Added: Webhook dispatcher service for outbound notifications (TMAIL-131)
pub mod webhook_dispatcher;
// Added: PST file processor service for Outlook migration (TMAIL-115)
pub mod pst_processor;
// Added: CSV parser and validator for bulk user import (TMAIL-136)
pub mod csv_processor;
// Added: Chat notifier service for team chat webhook integrations (TMAIL-129)
pub mod chat_notifier;
// Added: ICS calendar file generator for meeting scheduling (TMAIL-127)
pub mod ics_generator;
// Added (TMAIL-127): Inbound iMIP parser for accepting VEVENT invitations.
pub mod imip_parser;
// Added: AI API client abstraction for BYOK AI integration (TMAIL-105)
pub mod ai_client;
// Added: Embedding generation and similarity search service for TMAIL-106
pub mod embedding_service;
// Added: DLP content scanner service for Data Loss Prevention (TMAIL-108)
pub mod dlp_scanner;
// Added: NLP query parser service for AI-powered natural language search (TMAIL-135)
pub mod nlp_parser;
// Added: DANE TLSA record lookup and verification service for TMAIL-125
pub mod dane_service;
// Added: SMTP connection tester service for BYO-SMTP (TMAIL-48)
pub mod smtp_tester;
// Added: Plugin executor service for hook-based plugin execution (TMAIL-132)
pub mod plugin_executor;
// Added: vCard import/export service for contact management (TMAIL-119)
pub mod vcard_service;
// Added: Ollama local LLM client service for health, models, pull, delete (TMAIL-102)
pub mod ollama_client;
// Added: Rspamd HTTP API client for spam checking, learning, and statistics (TMAIL-15)
pub mod rspamd_client;
// Added: Payment service for Paystack and MTN MoMo integration (TMAIL-46)
pub mod payment_service;
// Added: Email deliverability checking service for DNS, blacklist, TLS verification (TMAIL-39)
pub mod deliverability_service;
// Added: Push notification sender service for FCM/APNs/Web Push (TMAIL-50)
pub mod push_service;
// Added: Redis cache service for session/branding/rate-limit/token-blacklist caching
pub mod cache_service;
// Added: Centralised AES-256-GCM encryption service used by DB-stored credentials.
pub mod encryption;
// Added: Per-request DB connection helpers that prime RLS session vars (TMAIL-161).
pub mod db_session;
// Added (TMAIL-165): Cached runtime feature flags.
pub mod feature_flags;
// Added (TMAIL-176/177/180): usage-based billing math + rollup loop.
pub mod billing_math;
pub mod billing_rollup;
// Added (TMAIL-127): Meeting-slot suggester — pure interval math over busy
// intervals, used by /api/calendar/suggest-slots.
pub mod slot_suggester;
// Added (TMAIL-266 / TMAIL-127): CalDAV REPORT / free-busy-query client used
// by GET /api/calendar/free-busy to merge external calendars into the local
// busy view.
pub mod caldav_freebusy;
// Added (TMAIL-100): Real LDAP/AD bind+search and apply_sync. Replaces the
// stub that previously lived inline in handlers/ldap.rs.
pub mod ldap_service;
// Added (TMAIL-307): Centralised admin-action audit helper. Every state-changing
// admin endpoint should call audit::audit_admin_action so the audit_log table
// accumulates a complete compliance trail.
pub mod audit;
