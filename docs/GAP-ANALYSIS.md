# TASMail Gap Analysis — Mobile App & Backend

**Date:** 2026-03-22
**Scope:** Mobile app strategy + backend missing subsystems

---

Excellent. I now have all the source material. Let me conduct a comprehensive gap analysis across both mobile and backend categories. I'll analyze the documented plans against the requirements you've specified.

## COMPREHENSIVE GAP ANALYSIS: TASMAIL (TASMail)

---

### EXECUTIVE SUMMARY

Based on a thorough review of the PRD, API Specification, Architecture, and Ghana Business Validation documents, **TASMail currently has NO mobile strategy** (only React PWA mentioned), and the backend is missing approximately **24 critical subsystems** spanning queue management, email features, infrastructure components, and operational tools. This analysis identifies **47+ gaps** organized by priority and engineering category.

The product is positioned for v1.0 as a **webmail SPA only**, with mobile and many backend subsystems deferred to v2.0+. This gap analysis provides a comprehensive roadmap to close those gaps.

---

## SECTION 1: MOBILE APP GAPS

**Current State:** React SPA with PWA support mentioned (G5 mentions "PWA" but no formal PWA implementation plan exists)

### 1.1 NATIVE MOBILE STRATEGY — MISSING

**GAP-M-001: No Mobile Platform Decision Framework (P0 — Business Critical)**
- **Category:** Product Strategy / Architecture
- **Current State:** Non-goal NG2 states "Mobile native apps — v2.0+"
- **Missing:** 
  - Comparison analysis: Flutter vs React Native vs PWA for Ghana market
  - Low-bandwidth performance metrics (2G/3G thresholds)
  - Device compatibility matrix (Android 8+, iOS 13+)
  - Offline-first sync requirements
- **Why P0:** Ghana market has 42M mobile connections, 75-80% internet penetration, but predominantly mobile-first. PWA alone may not cover offline access needs.
- **Deliverable:** Mobile Strategy Document with platform selection rationale for Ghana market

---

### 1.2 MOBILE PUSH NOTIFICATIONS — MISSING

**GAP-M-002: No FCM/APNs Integration (P0)**
- **Category:** Backend Service / Infrastructure
- **Current State:** Real-time notifications via WebSocket only (F11, WS /ws/notifications)
- **Missing:**
  - Firebase Cloud Messaging (FCM) service integration
  - Apple Push Notification service (APNs) enrollment
  - Device token registration API (`POST /api/mobile/register-device`)
  - Push notification delivery service + retry logic
  - Device token lifecycle management (rotation, revocation)
  - Notification preferences API (quiet hours, notification grouping)
- **Why P0:** Mobile users expect background notifications; WebSocket won't work when app is backgrounded
- **Deliverables:**
  - `POST /api/mobile/devices` — Register device token
  - `DELETE /api/mobile/devices/:token_id` — Unregister
  - Push notification queue service (bullmq or similar)
  - FCM + APNs credential storage in config

---

### 1.3 OFFLINE-FIRST SYNC PROTOCOL — MISSING

**GAP-M-003: No Offline Mode or Sync State Management (P0)**
- **Category:** Backend / Data Synchronization
- **Current State:** Real-time WebSocket push only; no offline queuing
- **Missing:**
  - Offline message composition queue (IndexedDB)
  - Sync state tracking (pending, synced, failed)
  - Conflict resolution for flag changes during offline mode
  - Background sync API for mobile browsers (Service Worker)
  - Delayed send queue (compose now, send when online)
  - Server-side sync version tracking (last_sync_at per device)
  - Bidirectional sync protocol (client → server delta, server → client delta)
- **Why P0:** Ghana has intermittent connectivity (2G fallback, power cuts). Offline draft composition is critical.
- **Deliverables:**
  - Sync API endpoints (`POST /api/sync/state`, `POST /api/sync/apply`)
  - Service Worker for background sync
  - IndexedDB schema design for offline drafts
  - Sync version table in PostgreSQL

---

### 1.4 MOBILE API ENDPOINTS — MISSING

**GAP-M-004: No Mobile-Specific API Surface (P0)**
- **Category:** API / Backend
- **Current State:** REST API designed for desktop SPA; no mobile-specific optimizations
- **Missing:**
  - Compressed response format (differential sync, binary encoding)
  - Delta sync endpoint (`POST /api/mobile/sync` with last_sync_version)
  - Abbreviated message summaries for list view
  - Attachment streaming (chunked download, pause/resume)
  - Data quota management (`GET /api/mobile/usage`)
  - Low-bandwidth mode flag (prefer text-only, smaller previews)
- **Why P0:** Mobile networks are slow; bandwidth efficiency is critical
- **Deliverables:**
  - `POST /api/mobile/sync?version=X` — Delta sync endpoint
  - Response compression middleware
  - Attachment resume support (Range headers)

---

### 1.5 MOBILE BIOMETRIC AUTHENTICATION — MISSING

**GAP-M-005: No Biometric Auth or Certificate Pinning (P1)**
- **Category:** Security / Authentication
- **Current State:** JWT auth only; no device-level security
- **Missing:**
  - Biometric prompt before accessing email
  - Certificate pinning for API calls (prevent MITM on public WiFi)
  - Device unlock requirement before app use
  - Secure storage of refresh token (Keychain/Keystore, not localStorage)
  - Trusted device concept (remember this device for 30 days)
  - Device-level encryption for local drafts
- **Why P1:** Mobile devices may be lost/stolen. Biometric auth + cert pinning are security best practices for financial/healthcare email.
- **Deliverables:**
  - Biometric auth handler in React Native/Flutter
  - Certificate pinning middleware (android-network-security-config.xml, iOS ATS)
  - Device trust token in JWT claims

---

### 1.6 MOBILE UX PATTERNS — MISSING

**GAP-M-006: No Mobile-First Email Interaction Patterns (P1)**
- **Category:** Frontend / UX
- **Current State:** Desktop-first React SPA (folder tree, detailed message view)
- **Missing:**
  - Swipe gestures (swipe-right to archive, swipe-left to delete)
  - Pull-to-refresh for folder sync
  - Bottom navigation bar (Inbox, Compose, Drafts, More)
  - Floating action button (FAB) for compose
  - Thread view (Gmail-style conversation grouping)
  - Inline reply/forward (no full message reload)
  - Full-screen compose modal
  - Attachment preview gallery (swipe through images)
  - Pinch-to-zoom for HTML email bodies
  - Voice input for compose (speech-to-text)
- **Why P1:** Desktop mouse interactions don't translate to touch; mobile UX requires different patterns
- **Deliverables:**
  - Mobile UI component library (swipe, gesture handlers)
  - Message threading algorithm
  - Touch-optimized layout (larger tap targets, 48px minimum)

---

### 1.7 SHARE SHEET & INTENT INTEGRATION — MISSING

**GAP-M-007: No Native OS Integration (P1)**
- **Category:** Frontend / Mobile Platform
- **Current State:** None
- **Missing:**
  - Share sheet integration (share link/text to email)
  - Camera access for photo attachments
  - Photo gallery picker
  - Calendar integration (show availability when inviting)
  - Contact picker from system address book
  - Deep linking (mailto: links open in TASMail)
  - Document picker (Files app on iOS, SAF on Android)
  - Widgets (quick compose, unread badge)
- **Why P1:** Users expect native mobile app integrations; lack of these feels "non-native"
- **Deliverables:**
  - React Native intent handlers
  - Camera/photo library permissions + UI
  - Deep link configuration (AndroidManifest.xml, Info.plist)

---

### 1.8 APP STORE DISTRIBUTION — MISSING

**GAP-M-008: No App Store Submission Strategy (P1)**
- **Category:** DevOps / Distribution
- **Current State:** None
- **Missing:**
  - Google Play Store submission workflow + screenshots
  - Apple App Store submission workflow + screenshots
  - Huawei AppGallery submission (critical for Ghana market without Google Play)
  - Samsung Galaxy Store submission
  - APK signing strategy (debug vs production keys)
  - Build pipeline for app stores (fastlane automation)
  - Versioning strategy (separate from backend)
  - Beta testing via TestFlight (iOS) / Google Play Beta (Android)
  - App update OTA mechanism (not forced, graceful degradation)
  - Analytics integration (Firebase Analytics, Matomo)
- **Why P1:** Distribution channels are critical for market reach; Huawei essential for Ghana
- **Deliverables:**
  - App Store asset pack (screenshots, app icons, privacy policy)
  - fastlane configuration for build + upload
  - App signing + certificate management

---

### 1.9 MOBILE-SPECIFIC QUOTA & STORAGE — MISSING

**GAP-M-009: No Mobile-Specific Cache/Storage Management (P2)**
- **Category:** Backend / Data Management
- **Current State:** No per-device storage tracking
- **Missing:**
  - Per-device local cache quota (e.g., keep last 30 days locally)
  - Remote storage usage API (`GET /api/mobile/storage-usage`)
  - Cache expiration policy (older messages deleted when device fills)
  - Attachment caching strategy (keep thumb + preview, remove full)
  - Sync bandwidth limits (don't sync large attachments on cellular)
- **Why P2:** Mobile device storage is limited (32GB is common in Ghana); intelligent caching required
- **Deliverables:**
  - Cache management service (IndexedDB + SQLite for React Native)
  - Storage quota API endpoint

---

### 1.10 MOBILE LOCALIZATION — MISSING

**GAP-M-010: No Mobile Localization for Ghana (P2)**
- **Category:** Frontend / Localization
- **Current State:** English UI only assumed
- **Missing:**
  - Twi localization (Ghana's most spoken language)
  - Ga, Fante, Ewe, Hausa localization options
  - Date/time formatting for Ghana timezone (GMT)
  - Currency display (GHS) in pricing tiers
  - Phone number format validation for Ghana (+233)
  - Mobile money payment methods (MTN MoMo, Vodafone Cash)
- **Why P2:** Nice-to-have for v1, but critical for Ghana market penetration
- **Deliverables:**
  - i18n framework (react-i18next)
  - Translation files for major Ghanaian languages

---

## SECTION 2: BACKEND GAPS

### 2.1 EMAIL QUEUE & RETRY LOGIC — MISSING

**GAP-B-001: No Email Queue Management System (P0 — Critical)**
- **Category:** Backend Service / Message Processing
- **Current State:** Outbound email sent directly via lettre SMTP; no queue
- **Missing:**
  - Message queue (Bun, bullmq, or native Rust async queue)
  - Retry logic with exponential backoff (3 retries: 5s, 30s, 5m)
  - Dead-letter queue (failed messages after 3 retries)
  - Queue persistence (Redis or database-backed)
  - Send rate limiting (e.g., max 30 msgs/min per user)
  - Bounce handling (NDR — Non-Delivery Report)
  - Delivery status tracking (queued, sent, failed, bounced)
  - Priority queue (urgent messages sent first)
  - Scheduled sending (send at specific time)
- **Why P0:** Email deliverability is core to product; queue ensures reliability
- **Deliverables:**
  - Queue service with Redis/database backend
  - `POST /api/messages?scheduled_at=2026-03-25T14:00:00Z` API
  - NDR webhook handler
  - Queue stats API (`GET /api/admin/queue-stats`)

---

### 2.2 ATTACHMENT STORAGE STRATEGY — MISSING

**GAP-B-002: No Attachment Storage & Streaming (P0)**
- **Category:** Backend / Infrastructure
- **Current State:** API spec allows attachments (F7, 25 MB max), but storage location undefined
- **Missing:**
  - Attachment storage backend decision:
    - Option A: Dovecot Maildir (in-mail storage) — pro: automatic backup with mail; con: no deduplication
    - Option B: S3-compatible object storage (MinIO, Wasabi) — pro: scalable, CDN-friendly; con: sync complexity
    - Option C: NFS mount on VPS — pro: simple; con: not scalable
  - Attachment deduplication (content-hash based)
  - Virolus scanning integration (ClamAV)
  - Attachment preview generation (thumbnails, PDF preview)
  - Streaming download with Range header support
  - Attachment encryption at rest
  - Expiration policy (delete old attachments after 1 year?)
  - Quota enforcement (attachments count toward mailbox quota)
- **Why P0:** Attachments are essential for business email; must be reliable & scalable
- **Deliverables:**
  - Attachment storage abstraction layer
  - ClamAV integration for malware scanning
  - S3 API implementation (if object storage chosen)
  - Thumbnail generation pipeline

---

### 2.3 EMAIL THREADING & CONVERSATION GROUPING — MISSING

**GAP-B-003: No Email Threading Algorithm (P0)**
- **Category:** Backend / Data Processing
- **Current State:** API returns message_id, in_reply_to, references headers (3.2), but no thread grouping
- **Missing:**
  - Thread identification algorithm (RFC 5256: In-Reply-To + References matching)
  - Thread ID assignment (virtual group identifier)
  - Conversation subject normalization (remove "Re:", "Fwd:")
  - Thread metadata calculation (thread_count, last_sender, last_date)
  - Pagination within threads (show parent + related messages)
  - Thread expansion/collapse in UI
  - Unread status per thread (or per message?)
  - Archive-entire-thread action
  - Search within thread
- **Why P0:** Gmail-style threading is expected; improves UX significantly
- **Deliverables:**
  - Thread grouping service (Rust)
  - Thread ID column in message cache table
  - Thread metadata API endpoints

---

### 2.4 CONTACT MANAGEMENT & ADDRESS BOOK — MISSING

**GAP-B-004: No Contact/Address Book System (P0)**
- **Category:** Backend / Data Management
- **Current State:** F20 mentions "Contact Autocomplete" but no storage
- **Missing:**
  - Contact model (name, email, phone, organization, notes)
  - Contact autocomplete service (`GET /api/contacts/autocomplete?q=john`)
  - Contact creation/update (`POST /api/contacts`)
  - Contact deduplication (merge duplicates)
  - Contact import from CSV
  - Contact export to vCard (.vcf)
  - Contact photo storage
  - Contact groups/distribution lists
  - Blacklist/whitelist management
  - LDAP sync option (for corporate deployments)
  - CardDAV read support (RFC 6352)
- **Why P0:** Businesses use address books extensively; essential for professional email
- **Deliverables:**
  - Contact table + CRUD endpoints
  - Autocomplete service with trie-based indexing
  - vCard import/export
  - CardDAV adapter (read-only for v1)

---

### 2.5 CALENDAR INTEGRATION & CALDAV — MISSING

**GAP-B-005: No Calendar Integration (P1 — Deferred but Important)**
- **Category:** Backend / Integration
- **Current State:** NG1 explicitly defers to Radicale/Nextcloud
- **Missing:**
  - CalDAV protocol support (RFC 4791)
  - Calendar endpoint configuration
  - Calendar sync from Radicale/Nextcloud
  - Meeting invitation parsing (iCalendar attachment detection)
  - Accept/Decline/Tentative RSVP
  - Calendar availability display in UI (when composing to attendee)
  - Timezone handling in iCalendar objects
  - Recurring event support
  - Calendar event notification settings
- **Why P1:** Many businesses expect calendar integration; could drive adoption
- **Deliverables:**
  - CalDAV client library (caldav-rs or similar)
  - Calendar sync service
  - Meeting RSVP handler
  - Calendar availability API

---

### 2.6 FILTER & RULES ENGINE — MISSING

**GAP-B-006: No Email Rules/Filters UI (P0)**
- **Category:** Backend / Email Processing
- **Current State:** F22 mentions "Sieve Filter Rules" UI, but implementation missing
- **Missing:**
  - Rule editor UI (condition + action builder)
  - Sieve script generation from rules
  - Rule templates (auto-archive newsletters, flag from boss)
  - Rule execution testing (show which messages match)
  - Rule priority/ordering
  - Rule enable/disable
  - Rule import/export
  - Server-side Sieve compilation & storage
  - Sieve validation + error reporting
  - Complex conditions: AND/OR/NOT logic
  - Actions: move, delete, flag, forward, reply, reject
- **Why P0:** Rules are essential for email management; Sieve support exists but UI missing
- **Deliverables:**
  - Rule builder UI (React components)
  - Sieve script compiler service
  - Rule CRUD API endpoints
  - Rule testing engine

---

### 2.7 AUTO-REPLY & VACATION RESPONDER — MISSING

**GAP-B-007: No Auto-Reply / Out-of-Office (P1)**
- **Category:** Backend / Email Processing
- **Current State:** None
- **Missing:**
  - Auto-reply message composition
  - Date range for auto-reply (enable from/until)
  - Sieve-based auto-reply rule
  - Exclude internal domain (don't reply to self-domain)
  - Exclude specific contacts (don't reply to known senders)
  - One-reply-per-sender rule (don't spam with multiple replies)
  - HTML vs text mode
  - Timezone-aware scheduling (auto-reply at specific hours only)
  - Auto-reply logs (show who was notified)
- **Why P1:** Common business feature; easy to implement with Sieve
- **Deliverables:**
  - Auto-reply settings API
  - Sieve rule generation
  - Auto-reply logging

---

### 2.8 EMAIL SIGNATURE MANAGEMENT — MISSING

**GAP-B-008: No Email Signature System (P0)**
- **Category:** Backend / Email Composition
- **Current State:** F21 mentions "Per-account configurable signatures" but not implemented
- **Missing:**
  - Signature editor (HTML + plain text)
  - Multiple signatures per account (default + role-based)
  - Automatic signature insertion on compose
  - Signature variables (name, title, company, phone)
  - Rich text editor for signatures (TipTap)
  - Signature in replies vs new messages (configurable)
  - Signature MIME placement (text/plain, text/html, multipart/mixed)
  - Signature templates for organizations
  - Admin-enforced signature for company accounts
- **Why P0:** Professional email requires signatures; expected feature
- **Deliverables:**
  - Signature model + CRUD endpoints
  - Signature insertion logic in compose handler
  - Signature variables resolver

---

### 2.9 QUOTA MANAGEMENT & ENFORCEMENT — MISSING

**GAP-B-009: No Quota Tracking & Enforcement (P0)**
- **Category:** Backend / Administration
- **Current State:** F14 mentions "Quota Management," but implementation details missing
- **Missing:**
  - Per-user quota enforcement (refuse to accept email if over quota)
  - Quota calculation (include attachments, inline images)
  - Grace period (warn at 90%, error at 100%)
  - Quota reset scheduling (annual, monthly)
  - Quota increase requests (admin approval workflow)
  - Quota audit logging (track usage growth)
  - Per-folder quota (limit Trash, Drafts)
  - Shared quota across distribution list members
  - Quota API endpoint (`GET /api/admin/users/:id/quota-usage`)
  - Email rejection when quota exceeded
  - Soft delete for over-quota recovery (30-day hold)
- **Why P0:** Without quota enforcement, disk space exhaustion risk
- **Deliverables:**
  - Quota tracking service (daily calculation)
  - Quota enforcement in LMTP delivery
  - Quota API endpoints
  - Grace period + warning notifications

---

### 2.10 MULTI-DEVICE SYNC STATE — MISSING

**GAP-B-010: No Cross-Device Sync Tracking (P1)**
- **Category:** Backend / Data Synchronization
- **Current State:** No per-device state tracking
- **Missing:**
  - Device registration (associate sessions with devices)
  - Per-device read/unread state (message might be read on phone but unread on desktop)
  - Device preference sync (theme, language, sidebar state)
  - Device-level notification silencing
  - Message sync state per device (seen on device X, not on Y)
  - Conflict resolution (which device's flag state wins if changed simultaneously?)
  - Last-sync timestamp per device
  - Device list in user account settings (revoke old sessions)
- **Why P1:** Users switch between mobile/desktop; state must sync correctly
- **Deliverables:**
  - Device table in PostgreSQL
  - Sync state tracking service
  - Multi-device notification logic

---

### 2.11 OAUTH2 / OIDC PROVIDER — MISSING

**GAP-B-011: No Third-Party Integration via OAuth2 (P1)**
- **Category:** Backend / Integration
- **Current State:** None
- **Missing:**
  - OAuth2 authorization server implementation (RFC 6749)
  - OIDC provider for enterprise integrations
  - Third-party app scopes (email:read, email:send, contacts:read)
  - Token management (access + refresh tokens)
  - Redirect URI whitelist
  - Consent screen UI
  - Token revocation endpoint
  - Integration marketplace (list authorized apps)
- **Why P1:** Enables third-party integrations (CRM, analytics, scheduling tools)
- **Deliverables:**
  - OAuth2 authorization endpoint
  - Token endpoint
  - Userinfo endpoint
  - Token revocation + introspection
  - Consent flow UI

---

### 2.12 API VERSIONING STRATEGY — MISSING

**GAP-B-012: No API Versioning Plan (P0 — Architectural)**
- **Category:** Backend / API Design
- **Current State:** API-SPECIFICATION.md doesn't mention versioning
- **Missing:**
  - Versioning strategy decision:
    - Option A: URL-based (/api/v1/, /api/v2/)
    - Option B: Header-based (Accept-Version: 1.0)
    - Option C: Subdomain-based (v1.api.mail.example.com)
  - Deprecation policy (e.g., 12-month support window per version)
  - Breaking change communication plan
  - Backwards compatibility strategy
  - Changelog + release notes
  - API documentation versioning (OpenAPI 3.x)
  - SDK generation for client libraries
- **Why P0:** Without versioning, breaking changes break clients; essential from v1
- **Deliverables:**
  - Versioning policy document
  - Axum router with version support
  - Deprecation headers (Sunset, Deprecation)

---

### 2.13 DATABASE MIGRATION STRATEGY — MISSING

**GAP-B-013: No Database Migration Tool Integration (P0 — DevOps)**
- **Category:** Backend / DevOps
- **Current State:** Migrations folder exists (2.2 Rust Backend), but no tooling specified
- **Missing:**
  - Migration tool decision:
    - Option A: sqlx prepare (compile-time checked)
    - Option B: Flyway (Java-based, industry standard)
    - Option C: Diesel migrations (Rust ORM)
    - Option D: Custom bash scripts
  - Migration naming convention (001_initial_schema.sql)
  - Rollback capability
  - Migration dry-run (test without committing)
  - Zero-downtime migration support (no locks on large tables)
  - Migration history tracking
  - CI/CD integration (auto-migrate on deploy)
  - Data seeding for test/dev
- **Why P0:** Without migrations, schema updates are error-prone; critical for scaling
- **Deliverables:**
  - Migration tool setup (sqlx or Flyway)
  - Migration template generator
  - CI/CD integration

---

### 2.14 LOGGING & AUDIT TRAIL — MISSING

**GAP-B-014: No Comprehensive Audit Logging (P0 — Security)**
- **Category:** Backend / Security / Observability
- **Current State:** Basic JSON logging mentioned (9.1), but no audit trail
- **Missing:**
  - Audit table (who did what when)
  - Login/logout logging
  - Admin action logging (user creation, quota change, domain deletion)
  - Email action logging (send, delete, forward with recipient)
  - Settings change logging (who changed what setting when)
  - API access logging (all REST calls)
  - Failed attempt logging (failed logins, permission denials)
  - Retention policy (keep audit logs for 1 year)
  - Audit log querying API (`GET /api/admin/audit-logs`)
  - Data subject access request support (GDPR/CCPA)
  - Log export for compliance
- **Why P0:** Audit trail is critical for compliance (DPC registration, regulatory requirements in Ghana)
- **Deliverables:**
  - Audit table + logging middleware
  - Audit querying API
  - Log retention policy enforcement
  - Export to CSV/JSON

---

### 2.15 WEBHOOK SYSTEM FOR INTEGRATIONS — MISSING

**GAP-B-015: No Webhook / Event Streaming (P1)**
- **Category:** Backend / Integration
- **Current State:** WebSocket push only (one-to-one user notifications)
- **Missing:**
  - Webhook endpoint registration API
  - Event types (message.received, message.sent, folder.created, user.created)
  - Payload schema (JSON with event metadata)
  - Webhook retry logic (exponential backoff, dead-letter)
  - Webhook signature verification (HMAC-SHA256)
  - Webhook testing UI (trigger test events)
  - Event log (show sent webhooks + responses)
  - Webhook filtering (subscribe to specific events)
  - Batch webhook delivery (coalesce multiple events)
  - Circuit breaker for failing endpoints
- **Why P1:** Enables CRM integration, analytics platforms, business logic automation
- **Deliverables:**
  - Webhook registration API
  - Event dispatcher service
  - Webhook retry queue
  - Webhook signature generation + verification

---

### 2.16 EMAIL IMPORT/EXPORT — MISSING

**GAP-B-016: No Email Import/Export Tools (P1)**
- **Category:** Backend / Data Migration
- **Current State:** None
- **Missing:**
  - MBOX import (batch import from other mail providers)
  - EML import (single message)
  - MBOX export (export user's entire mailbox)
  - CSV export (messages as spreadsheet)
  - Import progress tracking + UI
  - Import error handling (skip bad messages, report issues)
  - Import scheduling (run at off-peak hours)
  - Duplicate detection during import
  - IMAP source migration (connect to other IMAP server, copy messages)
  - Folder mapping during import
  - Attachment handling (skip/include)
- **Why P1:** Users switching from Gmail/Outlook need import tools; eases migration
- **Deliverables:**
  - MBOX/EML parser (mailparse crate)
  - Import API endpoints
  - Import job queue + progress tracking
  - Export API endpoints

---

### 2.17 ACCOUNT MIGRATION TOOLS — MISSING

**GAP-B-017: No User/Domain Migration Utilities (P1)**
- **Category:** Backend / Administration / DevOps
- **Current State:** None
- **Missing:**
  - User migration between domains
  - User migration between TASMail instances (export/import)
  - Domain migration (move all users to new domain)
  - Mailbox backup/restore utilities
  - User reset (clear all data, reinitialize)
  - Bulk import users from CSV
  - LDAP sync for enterprise (sync users from AD)
  - Soft delete with recovery (retain data for 30 days)
  - Hard delete with secure shredding (3-pass overwrite)
  - Migration rollback capability
- **Why P1:** Simplifies user onboarding and admin tasks
- **Deliverables:**
  - Migration CLI tools (Rust)
  - User import CSV parser
  - Backup/restore scripts

---

### 2.18 RATE LIMITING & THROTTLING DETAILS — MISSING

**GAP-B-018: No Detailed Rate Limit Implementation (P0)**
- **Category:** Backend / Infrastructure
- **Current State:** Section 8 mentions rate limits but no implementation details
- **Missing:**
  - Rate limit algorithm decision:
    - Option A: Token bucket (smooth out bursts)
    - Option B: Sliding window (more precise)
    - Option C: Fixed window (simple, less precise)
  - Per-endpoint configuration (different limits for different routes)
  - Per-user configuration (premium users get higher limits)
  - Per-IP configuration (login endpoint limits by IP, not user)
  - Redis backend for distributed rate limiting
  - Rate limit headers in responses (X-RateLimit-*)
  - Burst allowance (allow spike but enforce average)
  - Whitelist management (bypass limits for admin)
  - Rate limit metrics/dashboards
  - DDoS protection integration (CloudFlare, Akamai)
- **Why P0:** Essential for protecting backend from abuse; mail endpoints need protection
- **Deliverables:**
  - Tower Rate Limit middleware configuration
  - Redis-backed rate limit store
  - Per-endpoint rate limit rules
  - Admin rate limit override API

---

### 2.19 DOMAIN & MAIL REPUTATION TRACKING — MISSING

**GAP-B-019: No Sender Reputation System (P1)**
- **Category:** Backend / Deliverability
- **Current State:** None
- **Missing:**
  - Bounce rate tracking per domain
  - Complaint rate tracking (spam reports)
  - Unsubscribe tracking (if list-unsubscribe header used)
  - SMTP error logging (5xx rejections, deferral reasons)
  - Blacklist monitoring (check if domain on DNSBL)
  - Whitelist management (allow user-trusted domains)
  - Warm-up protocol (gradual send increase for new domains)
  - Reputation scoring (0-100 health score)
  - Reputation alerts (warn if dropping)
  - Reputation recovery recommendations
- **Why P1:** Email deliverability depends on domain reputation; monitoring is critical
- **Deliverables:**
  - Reputation tracking service
  - DNSBL checker (Spamhaus, Barracuda, etc.)
  - Reputation dashboard API
  - Reputation metrics export

---

### 2.20 INBOUND EMAIL RULES & FILTERING UI — MISSING

**GAP-B-020: No Inbound Filter Rule Management (P1)**
- **Category:** Backend / Email Processing
- **Current State:** Rspamd runs as external dependency (NG3); no UI for rule customization
- **Missing:**
  - Whitelist management (trust these senders)
  - Blacklist management (always mark as spam/delete)
  - SPF/DKIM/DMARC policy enforcement (reject fail? warn?)
  - Custom rule builder (complex conditions for spam)
  - Bayesian spam training (mark message as spam/ham, retrain filter)
  - Phrase-based filtering (block messages with certain words)
  - Attachment type filtering (block .exe, .zip)
  - Language filtering (block non-English if preferred)
  - Reputation-based filtering (reject from known spam sources)
  - Rate limiting by sender (if one sender sends 100 msgs/hour, drop)
  - Rspamd integration API
- **Why P1:** Fine-tune spam filtering for user preferences
- **Deliverables:**
  - Whitelist/blacklist API
  - Rspamd learner integration
  - Filter rule UI

---

### 2.21 ADMIN ANALYTICS & REPORTING — MISSING

**GAP-B-021: No Advanced Analytics & Reporting (P1)**
- **Category:** Backend / Administration
- **Current State:** `/api/admin/stats` exists but limited (domain/user counts, storage)
- **Missing:**
  - Email traffic analytics (sent/received per day/week/month)
  - User activity analytics (logins, folders accessed)
  - Mailbox size trends (growth over time)
  - Attachment analytics (most common types, total size)
  - Domain reputation trends
  - Bounce/complaint rates
  - Spam filtering effectiveness (messages blocked, % of total)
  - Delivery failure analysis (why emails failed)
  - API usage analytics (which endpoints most called)
  - Custom report builder
  - Scheduled report delivery (weekly digest to admin)
  - Export to CSV/PDF/Google Sheets
  - Visualization dashboard (charts, graphs)
- **Why P1:** Admins need insights into system health and user behavior
- **Deliverables:**
  - Analytics service (aggregate stats)
  - Dashboard API endpoints
  - Report generation service
  - Scheduled report delivery

---

### 2.22 ENCRYPTION AT REST & IN TRANSIT — MISSING

**GAP-B-022: No Full Encryption Implementation (P0 — Security)**
- **Category:** Backend / Security
- **Current State:** TLS mentioned but data encryption at rest not addressed
- **Missing:**
  - Encryption at rest decision:
    - Option A: Full-disk encryption (Linux dm-crypt)
    - Option B: Column-level encryption (encrypt sensitive DB columns)
    - Option C: Application-level encryption (encrypt before storing)
  - Master key management (KMS vs in-app key storage)
  - Key rotation schedule (annual minimum)
  - Encryption algorithm (AES-256)
  - Password reset: what happens to encrypted data?
  - Backup encryption (encrypted backups)
  - Decryption performance (cache vs re-decrypt)
  - User-managed keys option (user provides passphrase)
  - Forward secrecy in TLS (PFS cipher suites)
  - TLS certificate pinning (already mentioned for mobile)
  - DNSSEC for DNS lookups
- **Why P0:** Business email in Ghana may contain sensitive data; encryption is compliance requirement
- **Deliverables:**
  - Full-disk encryption setup (systemd instructions)
  - Column-level encryption for passwords (already Argon2'd but good to double-layer)
  - Key management policy document

---

### 2.23 BACKUP & DISASTER RECOVERY — MISSING

**GAP-B-023: No Backup/Recovery Strategy (P0 — Critical)**
- **Category:** Backend / DevOps / Operations
- **Current State:** None mentioned
- **Missing:**
  - Backup frequency (daily minimum, hourly recommended)
  - Backup storage location (off-site, encrypted)
  - Backup types:
    - Full backup (entire Maildir + DB)
    - Incremental backup (changes only)
    - Snapshot-based backup (VM snapshot at Equinix)
  - Backup retention (30 days minimum, 1 year archived)
  - Backup verification (restore test monthly)
  - Recovery time objective (RTO) — can we restore in 1 hour?
  - Recovery point objective (RPO) — max 1 hour data loss acceptable?
  - Restore procedures (documented, tested)
  - Point-in-time recovery (restore user mailbox to yesterday)
  - Ransomware protection (immutable backups, air-gapped)
  - Backup encryption
  - Monitoring + alerts for backup failures
  - Backup automation (no manual steps)
- **Why P0:** Data loss is catastrophic for email; backups are non-negotiable
- **Deliverables:**
  - Backup script (rsync or bacula)
  - PostgreSQL backup automation (pg_dump)
  - Maildir backup strategy
  - Restore playbook (disaster recovery runbook)
  - Backup monitoring alerts

---

### 2.24 SYSTEM HEALTH & ALERTING — MISSING

**GAP-B-024: No Proactive Monitoring & Alerting (P0)**
- **Category:** Backend / Observability / DevOps
- **Current State:** Health checks mentioned (9.2) but no alerting system
- **Missing:**
  - Metrics collection (Prometheus format)
  - Alert rules:
    - Disk space > 85%
    - Memory usage > 80%
    - DB connection pool exhausted
    - IMAP connection pool errors
    - Email queue delayed > 1 hour
    - API response time p95 > 500ms
    - Failed login rate spike
    - Mail delivery rate drops
    - WebSocket connection errors
  - Alert delivery (email, SMS, Slack, PagerDuty)
  - Alert escalation (if not ack'd in 15 min, escalate)
  - Silence/suppress alerts (maintenance window)
  - Alert history + trends
  - Health dashboard (public status page option)
  - Heartbeat monitoring (service must respond every 5 min)
  - Synthetic monitoring (send test email every hour, verify delivery)
- **Why P0:** Proactive monitoring prevents user-facing outages
- **Deliverables:**
  - Prometheus exporter for metrics
  - Alert rules file (Prometheus AlertManager)
  - Alerting service (webhook receiver)
  - Health dashboard UI

---

## SECTION 3: FEATURE-LEVEL GAPS

### 3.1 ADVANCED SEARCH & FULL-TEXT SEARCH — PARTIAL

**GAP-B-025: FTS Implementation Missing (P0)**
- **Category:** Backend / Search
- **Current State:** F10 and API spec mention "IMAP SEARCH or Dovecot FTS" but no UI details
- **Missing:**
  - Dovecot FTS (Xapian/Flatcurve) configuration
  - Index maintenance (ensure indexes up to date)
  - Search UI components (advanced search form with filters)
  - Saved searches (bookmark frequent searches)
  - Search history (remember past searches)
  - Search suggestions (auto-complete search terms)
  - Boolean operators documentation (AND, OR, NOT, phrase search)
  - Search scoping (search in attachments? sender only? date range?)
  - Search result ranking (most relevant first)
  - Partial word search (suffix matching)
  - Accent-insensitive search (Café = cafe)
  - Search performance optimization (indexes)
  - Search analytics (track popular searches)
- **Why P0:** Search is critical for users with large mailboxes
- **Deliverables:**
  - Dovecot FTS configuration (Xapian plugin)
  - Advanced search UI component
  - Full-text search implementation in backend

---

### 3.2 KEYBOARD SHORTCUTS — PARTIAL

**GAP-B-026: Keyboard Shortcuts UI & Help (P2)**
- **Category:** Frontend / UX
- **Current State:** F18 mentions "Keyboard Shortcuts" but not implemented
- **Missing:**
  - Shortcut definitions (Gmail-compatible if possible):
    - j/k: next/prev message
    - g then i: go to Inbox
    - c: compose
    - /: search
    - r: reply
    - a: reply all
    - f: forward
    - #: delete
    - e: archive
    - Shift+i: mark as unread
    - Ctrl+Enter: send message
  - Shortcut help modal (? key)
  - Customizable shortcuts
  - Platform differences (Ctrl on Windows/Linux, Cmd on Mac)
  - Conflict detection (warn if shortcut overrides browser shortcut)
  - Accessibility: ensure navigation works via Tab also
- **Why P2:** Power users expect keyboard navigation
- **Deliverables:**
  - Keyboard event handler
  - Shortcuts help modal
  - Customizable shortcuts API

---

### 3.3 DRAG & DROP FOR MESSAGE ORGANIZATION — PARTIAL

**GAP-B-027: Drag & Drop Implementation (P2)**
- **Category:** Frontend / UX
- **Current State:** F19 mentions "Drag & Drop" but not implemented
- **Missing:**
  - Drag message to folder
  - Drag folder to reorder
  - Multi-select drag (drag 5 messages at once)
  - Drag destination hover states
  - Drag preview thumbnail
  - Undo if drop fails
  - Accessibility: provide non-drag alternative (move button)
- **Why P2:** Nice-to-have; improves UX
- **Deliverables:**
  - React DnD library integration
  - Move message endpoint (already exists)

---

### 3.4 RICH TEXT EDITOR ENHANCEMENTS — PARTIAL

**GAP-B-028: TipTap Editor Features (P1)**
- **Category:** Frontend / Composition
- **Current State:** TipTap mentioned but feature set unclear
- **Missing:**
  - Formatting: Bold, Italic, Underline, Strikethrough
  - Lists: Ordered, Unordered, Checkbox
  - Indentation: Increase/Decrease
  - Links: Insert, edit, remove
  - Images: Insert from URL, upload
  - Tables: Insert, edit rows/cols
  - Code blocks: Syntax highlighting
  - Blockquote
  - Horizontal rule
  - Undo/Redo
  - Format painter (copy format from one section to another)
  - Paste special (paste as plain text, preserve formatting)
  - Markdown support (auto-detect and convert)
  - Emoji picker
  - Font selection (for display, not for sending; email should be cross-client compatible)
- **Why P1:** Professional email composition needs rich text
- **Deliverables:**
  - TipTap configuration with extensions
  - Toolbar with formatting buttons
  - Keyboard shortcuts for formatting

---

### 3.5 SPAM TRAINING & LEARNING — MISSING

**GAP-B-029: User Spam Training Interface (P1)**
- **Category:** Frontend / Backend
- **Current State:** Rspamd runs but no UI for user feedback
- **Missing:**
  - "Not Spam" button for false positives
  - "Spam" button to mark ham as spam (user training Bayesian filter)
  - Bulk spam marking (select 10, mark all as spam)
  - Rspamd learner integration (send feedback to Rspamd API)
  - Spam score display (show why email was filtered)
  - Spam rule explanations (which rules triggered?)
  - Whitelist from marked "not spam"
  - Blacklist from marked "spam"
  - Training feedback logging
  - Filter statistics (show improvement over time)
- **Why P1:** Improves spam filter accuracy over time
- **Deliverables:**
  - Spam/Not-Spam buttons in UI
  - Rspamd API integration for training
  - Feedback logging service

---

### 3.6 DRAFTS AUTO-SAVE & RECOVERY — PARTIAL

**GAP-B-030: Draft Management (P1)**
- **Category:** Frontend / Backend
- **Current State:** F5 mentions "Compose Email" but save strategy unclear
- **Missing:**
  - Auto-save draft every 10 seconds
  - Save to browser localStorage + server
  - Draft recovery on page reload (populate form with draft)
  - Multiple draft versions (show history, revert to older version?)
  - Draft expiration (delete if not sent in 30 days)
  - Draft API endpoints (`POST /api/drafts`, `PATCH /api/drafts/:id`)
  - Draft folder in IMAP (use Drafts folder)
  - Conflict resolution (user opens draft on 2 devices, edits simultaneously)
  - Draft notification (remind user of unsaved draft)
- **Why P1:** User frustration if draft lost on page crash
- **Deliverables:**
  - Draft auto-save service (front + backend)
  - Draft CRUD API
  - Draft recovery on mount

---

### 3.7 LABEL/TAG SYSTEM — MISSING

**GAP-B-031: Email Labels/Tags (P2)**
- **Category:** Backend / Frontend
- **Current State:** None
- **Missing:**
  - Label/tag model (per-user, not IMAP flag)
  - Create/edit/delete labels
  - Assign label to message
  - Bulk label assignment
  - Label colors
  - Label searching (show all messages with "urgent" label)
  - Label-folder distinction (labels are orthogonal to folders)
  - Default labels (e.g., Urgent, Follow-up, Later)
  - Label nesting (Projects > ClientA > Proposal)
  - Smart labels (auto-applied based on rules)
  - Label suggestions (based on message content)
- **Why P2:** Gmail-style labeling improves organization
- **Deliverables:**
  - Label table in DB
  - Label CRUD API
  - Label UI component

---

### 3.8 EMAIL ENCRYPTION (S/MIME, PGP) — MISSING

**GAP-B-032: Email Encryption Support (P2 — Deferred to v2)**
- **Category:** Backend / Security
- **Current State:** Q1 defers to v2
- **Missing:**
  - S/MIME support (RFC 5751):
    - Certificate management (import, export)
    - Sign outgoing emails
    - Verify signatures
    - Encrypt messages
    - Decrypt messages
  - PGP support (RFC 4880):
    - Key management UI
    - Keyserver integration (search keys)
    - Key trust levels
    - Key revocation
  - Encrypted message display (show encryption status, cert details)
  - Key distribution (attach public key to signature)
  - Encrypted search (if searchable encryption used)
- **Why P2:** Deferred but important for privacy-conscious users
- **Deliverables:**
  - S/MIME library integration (mailparse)
  - PGP library integration (sequoia-pgp)
  - Encryption UI

---

### 3.9 CONVERSATION SNIPPETS & PREVIEWS — PARTIAL

**GAP-B-033: Message Previews (P1)**
- **Category:** Frontend / Backend
- **Current State:** API spec shows "preview" field in list (3.1) but generation logic missing
- **Missing:**
  - Preview generation (first 100 characters of body)
  - Preview formatting (remove HTML tags)
  - Preview truncation (don't show huge quote chains)
  - Preview of attachments ("Has PDF, image, spreadsheet...")
  - Unread preview styling (highlight unread previews)
  - Starred/flag indicators in preview
  - Sender avatar in preview
  - Time formatting in preview (2 hours ago, Yesterday, etc.)
  - Hover preview (expand preview on hover)
- **Why P1:** Helps users scan message list quickly
- **Deliverables:**
  - Preview generation service
  - Preview formatting function
  - Preview UI component

---

## SECTION 4: INFRASTRUCTURE & OPERATIONS GAPS

### 4.1 MULTI-INSTANCE DEPLOYMENT — MISSING

**GAP-B-034: No Multi-Instance / High-Availability Setup (P1)**
- **Category:** DevOps / Infrastructure
- **Current State:** Section 7.1 describes single-server deployment only
- **Missing:**
  - Load balancer configuration (Nginx upstream, HAProxy)
  - Session sharing between instances (Redis vs sticky sessions)
  - Shared IMAP pool (prevent connection storms to Dovecot)
  - Shared database connections (PostgreSQL connection pooling, pgbouncer)
  - Database replication (PostgreSQL streaming replication)
  - Database failover (patroni, pg_failover_slot)
  - Cache layer for multi-instance (Redis for sessions, rate limits)
  - Static asset CDN (serve frontend from edge)
  - File storage sync (Maildir must be on shared NFS)
  - Health check configuration
  - Graceful shutdown (drain connections before stopping)
- **Why P1:** Single server is SPOF (single point of failure); needed for reliability
- **Deliverables:**
  - Load balancer configuration
  - Database replication setup
  - Redis/Cache configuration
  - NFS mount for shared Maildir

---

### 4.2 CONTAINER / DOCKER SUPPORT — MISSING

**GAP-B-035: No Docker/Container Support (P1 — Deployment)**
- **Category:** DevOps / Deployment
- **Current State:** Binary deployment mentioned; no Docker
- **Missing:**
  - Dockerfile for Rust backend
  - Dockerfile for React frontend (nginx + static)
  - docker-compose.yml with all services (Postgres, Dovecot, Postfix, Redis, etc.)
  - Image layering optimization (small images)
  - Health checks in Dockerfile
  - Resource limits (CPU, RAM)
  - Log drivers (journald or syslog)
  - Volume configuration (persistent data, config)
  - Environment variable configuration
  - Network definitions (isolated networks for security)
  - Multi-stage builds (keep images small)
- **Why P1:** Docker makes deployment reproducible and scalable
- **Deliverables:**
  - Dockerfile(s)
  - docker-compose.yml
  - Docker build pipeline

---

### 4.3 KUBERNETES DEPLOYMENT — MISSING

**GAP-B-036: No Kubernetes Support (P2 — Future)**
- **Category:** DevOps / Scalability
- **Current State:** None
- **Missing:**
  - Kubernetes manifests (Deployment, Service, ConfigMap)
  - StatefulSet for stateful services (PostgreSQL, Redis)
  - Persistent volumes for mail storage
  - Ingress configuration (TLS, routing)
  - Horizontal pod autoscaling (HPA)
  - Resource requests/limits per pod
  - Network policies (pod-to-pod communication)
  - RBAC (role-based access control)
  - Service mesh integration (Istio/Linkerd for observability)
  - Helm chart for templating
- **Why P2:** Kubernetes enables cloud-native deployment; future-proofing
- **Deliverables:**
  - Kubernetes manifests
  - Helm chart

---

### 4.4 MONITORING & OBSERVABILITY GAPS — MISSING

**GAP-B-037: No Metrics/Tracing/Profiling (P1)**
- **Category:** DevOps / Observability
- **Current State:** Health checks (9.2) and logging (9.1) mentioned; no metrics/traces
- **Missing:**
  - Metrics collection:
    - API request latency (histogram)
    - API request count (counter)
    - Active WebSocket connections (gauge)
    - IMAP connection pool utilization (gauge)
    - Message queue depth (gauge)
    - Error rates (counter)
    - Custom metrics (email send success rate)
  - Metrics scraping (Prometheus exporter)
  - Distributed tracing (OpenTelemetry, Jaeger)
  - Trace correlation (trace user action across services)
  - Flame graphs for profiling (CPU, memory)
  - Slow query logging (PostgreSQL)
  - Request context propagation (trace ID in logs)
  - Metrics dashboards (Grafana)
  - SLA/SLO definition (99.9% uptime, p99 latency < 200ms)
- **Why P1:** Observability essential for debugging issues in production
- **Deliverables:**
  - Prometheus exporter integration
  - Tracing middleware (OpenTelemetry)
  - Grafana dashboards
  - SLO definition document

---

### 4.5 TESTING STRATEGY — PARTIAL

**GAP-B-038: Test Coverage & E2E Tests (P0)**
- **Category:** Engineering / Quality Assurance
- **Current State:** "cargo test" mentioned; no test plan
- **Missing:**
  - Unit test coverage targets (80% minimum)
  - Integration test coverage (database + services)
  - E2E test scenarios (Playwright, Selenium):
    - Login → read email → reply
    - Compose → send → verify in Sent folder
    - Search
    - Folder operations
    - Admin panel operations
  - Load testing (simulate 100 concurrent users)
  - Security testing (OWASP Top 10)
  - Accessibility testing (WCAG 2.1 AA compliance)
  - Email deliverability testing (SpamAssassin score)
  - Browser/device compatibility testing
  - Performance regression testing
  - Flaky test detection
  - Test environment setup (test DB, test mail server)
- **Why P0:** Quality assurance prevents regressions
- **Deliverables:**
  - Test strategy document
  - Unit test suite (Rust)
  - E2E test suite (Playwright)
  - Test environment setup
  - CI/CD test integration

---

### 4.6 CI/CD PIPELINE — MISSING

**GAP-B-039: No Continuous Integration (P0 — DevOps)**
- **Category:** DevOps / CI/CD
- **Current State:** None mentioned
- **Missing:**
  - CI trigger (on git push)
  - Linting (cargo clippy, eslint)
  - Testing (cargo test, npm test)
  - Build artifacts (Docker image, binary, static bundle)
  - Security scanning (dependency vulnerabilities, SAST)
  - Container registry (Docker Hub, GitHub Container Registry)
  - CD: automated deployment to staging/prod
  - Deployment strategy (blue-green, canary)
  - Rollback capability
  - Deployment notifications
  - Artifact retention policies
- **Why P0:** CI/CD enables fast iteration and safe deployments
- **Deliverables:**
  - GitHub Actions workflow file (.github/workflows/)
  - Linting + test + build jobs
  - Container image building
  - Deployment job

---

### 4.7 INSTALLATION & DEPLOYMENT DOCUMENTATION — PARTIAL

**GAP-B-040: Deployment Runbook Missing (P0)**
- **Category:** Documentation / DevOps
- **Current State:** Deployment steps listed (10.2) but not detailed
- **Missing:**
  - Step-by-step installation guide (copy-paste friendly)
  - DNS configuration (MX, SPF, DKIM, DMARC records)
  - Postfix configuration (main.cf, master.cf, virtual maps)
  - Dovecot configuration (SQL auth setup, Sieve, FTS)
  - PostgreSQL setup (user, database, permissions)
  - TLS certificate setup (Let's Encrypt certbot)
  - Redis setup (if using queue)
  - Backend binary deployment
  - Frontend static deployment
  - Systemd service setup
  - Log rotation setup
  - SELinux/AppArmor policies
  - Firewall rules (iptables/ufw)
  - Upgrade procedures (from v1.0 → v1.1)
  - Rollback procedures
  - Troubleshooting guide
  - Performance tuning (for 100+ users)
- **Why P0:** Self-hosted users need detailed instructions
- **Deliverables:**
  - Installation guide (markdown)
  - Deployment checklist
  - Troubleshooting guide
  - Configuration template files

---

### 4.8 SECURITY TESTING & PENETRATION TEST — MISSING

**GAP-B-041: No Security Audit / Pen Test (P0)**
- **Category:** Security / Testing
- **Current State:** Security mentioned (6.2) but no testing plan
- **Missing:**
  - Penetration testing (hire third-party tester)
  - Vulnerability scanning (OWASP dependency check, container scanning)
  - Static analysis (SonarQube, Clippy)
  - Dynamic analysis (DAST on running instance)
  - Secret scanning (no hardcoded API keys)
  - HTTPS/TLS configuration test (SSL Labs grade)
  - Password policy testing
  - Session management testing
  - XSS/CSRF testing
  - SQL injection testing
  - Rate limit bypass testing
  - Privilege escalation testing
  - Data exposure testing
- **Why P0:** Security is critical, especially for business email in regulated market (Ghana DPC)
- **Deliverables:**
  - Penetration test report
  - Security fixes + re-test
  - Security posture scorecard

---

## SECTION 5: BUSINESS/GTM GAPS

### 5.1 PRICING & BILLING SYSTEM — MISSING

**GAP-B-042: No Billing/Payment System (P0 — Business Critical)**
- **Category:** Product / Business
- **Current State:** Ghana Business Validation mentions pricing (GHS 20-110) but no implementation
- **Missing:**
  - Billing model (per-user, per-domain, usage-based)
  - Payment processing:
    - Stripe integration (credit card)
    - Paystack integration (Ghana's largest payment processor)
    - MTN MoMo mobile money
    - Vodafone Cash
    - Bank transfer
  - Invoice generation
  - Invoice delivery (email, PDF)
  - Subscription management (upgrade, downgrade, cancel)
  - Renewal reminders
  - Failed payment handling (retry, grace period, suspension)
  - Refund/credit policies
  - Tax/VAT handling (Ghanaian VAT)
  - Dunning management (collect failed payments)
  - Metering/usage tracking (for usage-based billing)
  - Billing portal (user-facing)
  - Billing history API
  - Free trial management
  - Promotional codes/discounts
- **Why P0:** Revenue generation depends on billing
- **Deliverables:**
  - Stripe + Paystack integration
  - Billing service (calculate charges, generate invoices)
  - Billing portal UI
  - Subscription management API

---

### 5.2 USER ONBOARDING FLOW — MISSING

**GAP-B-043: No Onboarding / Setup Wizard (P1)**
- **Category:** Product / UX
- **Current State:** None mentioned
- **Missing:**
  - Welcome flow for new users
  - Email verification (confirm email ownership)
  - Password setup guidance (strength meter, password manager suggestion)
  - Quick-start tutorial (short, optional)
  - First-email demo (show how to read/compose)
  - Settings tour (where to configure)
  - Mobile app prompt (suggest mobile app if available)
  - Phone verification (2FA setup)
  - Recovery options setup (backup email, phone number)
  - Signature creation
  - Address book import (suggest Outlook/Gmail import)
  - Sync other email accounts (BYO-SMTP onboarding)
- **Why P1:** Good onboarding reduces churn
- **Deliverables:**
  - Onboarding UI flow
  - Tutorial components
  - First-time-user detection

---

### 5.3 ADMIN SETUP WIZARD — MISSING

**GAP-B-044: No Admin Deployment Wizard (P1)**
- **Category:** Product / DevOps
- **Current State:** Installation guide missing (see GAP-B-040)
- **Missing:**
  - Web-based setup wizard (instead of manual config)
  - Step 1: System requirements check
  - Step 2: Database setup
  - Step 3: Admin account creation
  - Step 4: Domain configuration
  - Step 5: DNS records to create (copy-paste ready)
  - Step 6: TLS certificate setup
  - Step 7: Postfix/Dovecot configuration
  - Step 8: Backend binary configuration
  - Step 9: Email routing test
  - Step 10: User creation
  - Live configuration testing (test IMAP, SMTP connectivity)
  - Configuration validation
  - Error recovery (rollback if setup fails)
- **Why P1:** Reduces deployment friction for non-technical admins
- **Deliverables:**
  - Setup wizard UI
  - Configuration validator service
  - DNS record generator

---

### 5.4 SUPPORT & DOCUMENTATION — MISSING

**GAP-B-045: No Support Infrastructure (P1)**
- **Category:** Support / Documentation
- **Current State:** None mentioned
- **Missing:**
  - User documentation (getting started guides, FAQ)
  - Admin documentation (deployment, management, troubleshooting)
  - API documentation (OpenAPI/Swagger, code examples)
  - Video tutorials (installation, usage)
  - Support channels:
    - Email support
    - Community forum (Discourse?)
    - Live chat (Intercom?)
    - GitHub issues
  - Knowledge base (searchable help articles)
  - Bug report process
  - Feature request process
  - SLA definition (response time, resolution time)
  - Community moderation
- **Why P1:** Users need support to succeed
- **Deliverables:**
  - User guide (markdown/PDF)
  - Admin guide (markdown/PDF)
  - API docs (OpenAPI YAML)
  - FAQ page
  - Knowledge base platform

---

### 5.5 ANALYTICS & USAGE TRACKING — MISSING

**GAP-B-046: No User Analytics (P1)**
- **Category:** Product / Analytics
- **Current State:** None
- **Missing:**
  - Feature usage analytics (which features used most?)
  - User journey analytics (onboarding completion rate)
  - Retention analytics (day 1, 7, 30 retention)
  - Churn analytics (why users leave?)
  - Performance analytics (page load times, error rates)
  - Privacy-compliant analytics (no email content tracking)
  - Heatmaps (UI interaction patterns)
  - Funnel analysis (conversion rates)
  - Cohort analysis (compare user groups)
  - Revenue analytics (LTV, CAC)
  - Analytics dashboard (for product team)
  - Privacy policy compliance (disclose analytics)
  - Opt-out mechanism
- **Why P1:** Data drives product decisions
- **Deliverables:**
  - Analytics service integration (Plausible, Matomo)
  - Analytics events tracking
  - Analytics dashboard

---

### 5.6 COMPLIANCE & PRIVACY — MISSING

**GAP-B-047: No Compliance Documentation (P0 — Critical for Ghana)**
- **Category:** Compliance / Legal
- **Current State:** DPC registration mentioned; no implementation
- **Missing:**
  - Privacy policy (GDPR, CCPA, Ghana DPC Act 843 compliant)
  - Terms of service
  - Data processing agreement (DPA)
  - DPC registration application + evidence
  - GDPR compliance (if EU users):
    - Data subject access request process
    - Data deletion capability
    - Data portability (export)
    - Breach notification procedure
  - Ghana cybersecurity compliance (CSA Act 1038)
  - Data retention policy
  - Cookie consent (if applicable)
  - DPIA (Data Protection Impact Assessment)
  - Sub-processor list (Dovecot, Postfix, PostgreSQL owners)
  - Incident response plan (breach notification)
  - Audit logging for compliance
  - Security certifications (ISO 27001 aspirational)
- **Why P0:** Legal/regulatory compliance is mandatory; critical for Ghana market
- **Deliverables:**
  - Privacy policy document
  - Terms of service
  - DPA template
  - Compliance checklist

---

## SUMMARY TABLE: ALL GAPS

| Gap ID | Gap Name | Priority | Category | Effort | Estimated Impact |
|--------|----------|----------|----------|--------|------------------|
| GAP-M-001 | Mobile Platform Decision | P0 | Strategy | High | Blocks mobile roadmap |
| GAP-M-002 | FCM/APNs Push Notifications | P0 | Backend | High | Essential for mobile |
| GAP-M-003 | Offline Sync Protocol | P0 | Backend | High | Critical for Ghana connectivity |
| GAP-M-004 | Mobile API Endpoints | P0 | Backend | Medium | Required for mobile |
| GAP-M-005 | Biometric Auth + Cert Pinning | P1 | Security | Medium | Security best practice |
| GAP-M-006 | Mobile UX Patterns | P1 | Frontend | High | Touch interactions needed |
| GAP-M-007 | OS Integration (Share, Camera) | P1 | Frontend | High | Expected mobile features |
| GAP-M-008 | App Store Distribution | P1 | DevOps | Medium | Critical for reach |
| GAP-M-009 | Mobile Cache/Storage Mgmt | P2 | Backend | Medium | Nice-to-have |
| GAP-M-010 | Mobile Localization (Twi, Ga) | P2 | Frontend | Medium | Ghana market |
| GAP-B-001 | Email Queue & Retry Logic | P0 | Backend | High | Core reliability |
| GAP-B-002 | Attachment Storage & Streaming | P0 | Backend | High | Essential feature |
| GAP-B-003 | Email Threading | P0 | Backend | High | UX expectation |
| GAP-B-004 | Contact/Address Book | P0 | Backend | High | Core feature |
| GAP-B-005 | Calendar Integration (CalDAV) | P1 | Backend | Medium | Business feature |
| GAP-B-006 | Email Rules/Filters UI | P0 | Backend | Medium | Core feature |
| GAP-B-007 | Auto-Reply/Vacation | P1 | Backend | Low | Common feature |
| GAP-B-008 | Email Signatures | P0 | Backend | Low | Professional feature |
| GAP-B-009 | Quota Management | P0 | Backend | Medium | Admin requirement |
| GAP-B-010 | Multi-Device Sync State | P1 | Backend | Medium | Cross-device UX |
| GAP-B-011 | OAuth2/OIDC Provider | P1 | Backend | Medium | Integration capability |
| GAP-B-012 | API Versioning Strategy | P0 | Architecture | Low | Future-proofing |
| GAP-B-013 | Database Migration Tool | P0 | DevOps | Low | Schema management |
| GAP-B-014 | Audit Logging | P0 | Security | Medium | Compliance (DPC) |
| GAP-B-015 | Webhook System | P1 | Backend | Medium | Integration platform |
| GAP-B-016 | Email Import/Export | P1 | Backend | Medium | Data migration |
| GAP-B-017 | Account Migration Tools | P1 | Backend | Low | Admin UX |
| GAP-B-018 | Rate Limiting Details | P0 | Backend | Low | Already partially done |
| GAP-B-019 | Domain Reputation Tracking | P1 | Backend | Medium | Deliverability |
| GAP-B-020 | Inbound Filter Rule UI | P1 | Frontend | Medium | Spam management |
| GAP-B-021 | Admin Analytics | P1 | Backend | Medium | Insights |
| GAP-B-022 | Encryption at Rest | P0 | Security | Medium | Ghana compliance |
| GAP-B-023 | Backup/Disaster Recovery | P0 | DevOps | High | Data protection |
| GAP-B-024 | Monitoring & Alerting | P0 | DevOps | High | Operational visibility |
| GAP-B-025 | Full-Text Search (FTS) | P0 | Backend | Medium | Core feature |
| GAP-B-026 | Keyboard Shortcuts | P2 | Frontend | Low | Power user feature |
| GAP-B-027 | Drag & Drop | P2 | Frontend | Low | UX polish |
| GAP-B-028 | Rich Text Editor | P1 | Frontend | Medium | Composition UX |
| GAP-B-029 | Spam Training | P1 | Backend | Low | Filter improvement |
| GAP-B-030 | Draft Auto-Save | P1 | Frontend | Low | UX reliability |
| GAP-B-031 | Labels/Tags | P2 | Backend | Medium | Organization |
| GAP-B-032 | Email Encryption (S/MIME/PGP) | P2 | Backend | High | Deferred to v2 |
| GAP-B-033 | Message Previews | P1 | Frontend | Low | UX improvement |
| GAP-B-034 | Multi-Instance HA | P1 | DevOps | High | Reliability |
| GAP-B-035 | Docker Support | P1 | DevOps | Medium | Deployment |
| GAP-B-036 | Kubernetes Support | P2 | DevOps | High | Cloud-native future |
| GAP-B-037 | Metrics/Tracing/Profiling | P1 | DevOps | High | Observability |
| GAP-B-038 | Test Coverage & E2E | P0 | QA | High | Quality assurance |
| GAP-B-039 | CI/CD Pipeline | P0 | DevOps | High | Deployment automation |
| GAP-B-040 | Deployment Runbook | P0 | Documentation | Medium | Self-hosting support |
| GAP-B-041 | Security Testing & Pen Test | P0 | Security | High | Compliance |
| GAP-B-042 | Billing/Payment System | P0 | Business | High | Revenue |
| GAP-B-043 | User Onboarding Flow | P1 | Product | Medium | Retention |
| GAP-B-044 | Admin Setup Wizard | P1 | Product | Medium | Deployment UX |
| GAP-B-045 | Support Infrastructure | P1 | Support | Medium | Customer success |
| GAP-B-046 | User Analytics | P1 | Analytics | Low | Product insights |
| GAP-B-047 | Compliance & Privacy (DPC) | P0 | Legal | High | Ghana market entry |

---

## PRIORITIZED ROADMAP: CLOSING CRITICAL GAPS

### V1.0 Launch Prerequisites (P0 — Must Have Before Any Release)

**Phase 1: Core Functionality (Weeks 1-4)**
- GAP-B-001: Email queue (reliability)
- GAP-B-002: Attachment storage (features)
- GAP-B-003: Email threading (UX)
- GAP-B-004: Contact autocomplete (UX)
- GAP-B-006: Rules/filters UI (features)
- GAP-B-008: Email signatures (professional)
- GAP-B-009: Quota management (admin)

**Phase 2: Infrastructure & Testing (Weeks 5-8)**
- GAP-B-013: Database migrations (schema safety)
- GAP-B-018: Rate limiting (backend protection)
- GAP-B-025: Full-text search (discoverability)
- GAP-B-038: Test suite (quality)
- GAP-B-040: Deployment guide (self-hosting)

**Phase 3: Security & Compliance (Weeks 9-12)**
- GAP-B-012: API versioning (future-proofing)
- GAP-B-014: Audit logging (Ghana DPC requirement)
- GAP-B-022: Encryption at rest (Ghana compliance)
- GAP-B-023: Backup/DR (data protection)
- GAP-B-041: Security testing (compliance)
- GAP-B-047: Privacy/compliance docs (legal)

**Phase 4: DevOps & Operations (Weeks 13-16)**
- GAP-B-024: Monitoring & alerting (visibility)
- GAP-B-039: CI/CD pipeline (safe deployments)

### V1.1 Post-Launch (P1 — Add in First 3 Months)

- GAP-B-005: CalDAV (business feature)
- GAP-B-007: Auto-reply (common feature)
- GAP-B-010: Multi-device sync (cross-device UX)
- GAP-B-011: OAuth2 (integrations)
- GAP-B-015: Webhooks (automation)
- GAP-B-016: Import/export (migration)
- GAP-B-021: Admin analytics (insights)
- GAP-B-034: HA/multi-instance (reliability)
- GAP-B-035: Docker (deployment)
- GAP-B-037: Metrics/tracing (observability)
- GAP-B-043: Onboarding flow (retention)
- GAP-B-042: Billing system (revenue)
- GAP-B-044: Admin wizard (deployment UX)
- GAP-B-045: Support (customer success)

### V2.0 (P2 — Future Roadmap)

- **Mobile Apps** (all GAP-M-*)
- GAP-B-019: Domain reputation (email deliverability)
- GAP-B-020: Inbound filter UI (spam management)
- GAP-B-026: Keyboard shortcuts (power users)
- GAP-B-027: Drag & drop (UX polish)
- GAP-B-028: Rich text enhancements (composition)
- GAP-B-031: Labels/tags (organization)
- GAP-B-032: Email encryption (privacy)
- GAP-B-036: Kubernetes (cloud-native)

---

## RECOMMENDATIONS FOR ISSUE CREATION

Based on this gap analysis, I recommend creating **50+ GitHub issues** organized by:

1. **Milestone: V1.0 Core (P0)** — 16 issues
2. **Milestone: V1.1 Release (P1)** — 24 issues
3. **Milestone: V2.0 Future (P2)** — 10 issues

Each issue should include:
- **Title:** Clear, actionable (e.g., "Add email queue service with retry logic")
- **Description:** Context, acceptance criteria, design notes
- **Labels:** Priority, Category (Backend/Frontend/DevOps/Security), Effort (S/M/L)
- **Assignee:** Rust backend, React frontend, DevOps, etc.
- **Links:** Dependencies (issue #5 must be done before #12)

Would you like me to create a prioritized GitHub issues list, design specifications for any specific gap, or elaborate on implementation strategies for critical components?