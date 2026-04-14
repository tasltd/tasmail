// Added: Attachment storage and ClamAV scanning service for TMAIL-59
pub mod attachment_service;
pub mod auth_service;
pub mod email_scheduler;
pub mod imap_service;
// Added: Phishing scanner heuristic service for TMAIL-124
pub mod phishing_scanner;
// Added: Queue processor background service for TMAIL-58
pub mod queue_processor;
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
