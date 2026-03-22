# TASMail Competitive Gap Implementation Research

**Date:** 2026-03-22
**Purpose:** Research implementation approaches for features where TASMail falls short of Gmail, Outlook, Zoho Mail, and Thunderbird.

---

## 1. Calendar & Contacts Integration

### CalDAV Server Options

| Server | Pros | Cons |
|--------|------|------|
| **Radicale** | Pure Python, no DB, file-based, simple config | No web admin, limited scalability |
| **Baikal** | Web admin, MySQL/SQLite, PHP-based | Requires PHP stack |
| **Stalwart** | All-in-one Rust mail+CalDAV+CardDAV+JMAP | Would replace Postfix/Dovecot |
| **DAViCal** | Full-featured, PostgreSQL backend | Complex setup, PHP |

**Recommendation:** Radicale standalone behind reverse proxy, sharing Dovecot auth via `radicale-dovecot-auth` unix socket. Axum backend acts as CalDAV/CardDAV client using `libdav` to proxy to React frontend.

### Rust Crates
- **libdav** — CalDAV + CardDAV client with service discovery
- **kitchen-fridge** — CalDAV client + local cache
- **ical-rs / ical** — iCalendar (RFC 5545) + vCard (RFC 6350) parser
- **calcard** (Stalwart) — iCalendar/JSCalendar + vCard/JSContact parsing
- **icalendar** — iCalendar builder + parser

### React Calendar UI
- **FullCalendar** — 1M+ weekly downloads, plugin architecture, drag-drop (recommended)
- **react-big-calendar** — 500K+ downloads, lighter weight, Google Calendar look

**Complexity:** Complex (3-4 weeks MVP)
**Priority:** Medium — expected by business users but email core comes first

### Sources
- https://petermolnar.net/article/replacing-baikal-with-radicale-for-carrdav-and-caldav/
- https://rfrancocantero.medium.com/building-a-self-hosted-caldav-server-the-technical-reality-behind-calendar-sharing-9a930af28ff0
- https://wiki.archlinux.org/title/Radicale
- https://sabre.io/baikal/
- https://crates.io/crates/libdav
- https://docs.rs/kitchen-fridge
- https://crates.io/crates/minicaldav
- https://github.com/stalwartlabs/calcard
- https://fullcalendar.io/docs/react
- https://github.com/jquense/react-big-calendar
- https://www.builder.io/blog/best-react-calendar-component-ai
- https://stalw.art/

---

## 2. 2FA/MFA Implementation

### TOTP (RFC 6238)
**Crate:** `totp-rs` — RFC-compliant, QR code generation, `otpauth://` URL parsing

### WebAuthn/FIDO2 Passkeys
**Crate:** `webauthn-rs` (security audited by SUSE) — tutorial exists for Axum specifically
**Alternative:** `passkey-rs` (1Password) + `oauth2-passkey-axum` for pre-built handlers

### SMS OTP for Ghana Market
| Provider | Ghana Delivery | Pricing |
|----------|---------------|---------|
| **Hubtel** | Native MTN/Telecel/AirtelTigo | ~GHS 0.04-0.06/msg |
| **Arkesel** | MTN-focused, 550-900ms | ~GHS 0.03-0.06/msg |
| **Africa's Talking** | Pan-African (30+ countries) | Varies by volume |
| **Sendexa** | Direct carrier, 180-350ms | Competitive |

**Recommendation:** Hubtel primary (local, mobile money), Africa's Talking fallback

### 2FA + JWT Flow
1. Username/password → partial JWT with `mfa_required: true`
2. Present 2FA screen → TOTP/passkey
3. Validate → full JWT with `mfa_verified: true`
4. Rate-limit: 5 attempts, 15-min lockout

### Backup Codes
- Generate 10 single-use codes at enrollment
- Store hashed (Argon2), display only once
- Require password + existing MFA to regenerate

**Complexity:** Medium (TOTP: 1 week, WebAuthn: 2 weeks, SMS: 1 week)
**Priority:** High — critical for business email security

### Sources
- https://crates.io/crates/totp-rs
- https://github.com/KaneGreen/totp_rfc6238
- https://crates.io/crates/webauthn-rs/0.4.2
- https://ktaka.blog.ccmp.jp/2025/01/implementing-passkeys-authentication-in-rust-axum.html
- https://github.com/1Password/passkey-rs
- https://crates.io/crates/oauth2-passkey-axum
- https://www.sendexa.co/blog/best-sms-api-ghana-comparison-sendexa-vs-hubtel-arkesel
- https://celcomafrica.com/blog/bulk-sms-in-ghana-a-comprehensive-guide/
- https://africastalking.com/pricing

---

## 3. End-to-End Encryption

### Recommended: OpenPGP.js (ProtonMail model)
1. Generate OpenPGP keypair in browser on account creation
2. Private key encrypted with AES-256, key derived from password
3. Encrypted private key stored server-side (zero-access)
4. On login, password decrypts private key locally
5. Messages encrypted with recipient's public key before leaving browser

**OpenPGP.js** — maintained by Proton Mail, RFC 9580, Web Crypto API

### Rust Crates (server-side)
- **sequoia-openpgp** — full RFC 9580 + RFC 4880, multiple crypto backends
- **pgp** — alternative OpenPGP implementation

### S/MIME
- Requires X.509 certificates from CA (Actalis, ACME Email for free)
- Management at scale is complex; prioritize OpenPGP over S/MIME

**Complexity:** Complex (4-6 weeks basic E2EE)
**Priority:** Low for Ghana SMB — TLS + at-rest encryption sufficient for v1

### Sources
- https://proton.me/security/end-to-end-encryption
- https://proton.me/support/proton-mail-encryption-explained
- https://openpgpjs.org/
- https://github.com/openpgpjs/openpgpjs
- https://sequoia-pgp.org/
- https://crates.io/crates/sequoia-openpgp

---

## 4. Schedule Send, Snooze, Email Recall

### Scheduled Sending
Application-level: store in DB with `send_at` timestamp, background Tokio task checks every 30s, submits to Postfix when due.

### Snooze
Move to virtual "snoozed" folder (IMAP flag or DB record), background job checks `snooze_until`, restores to inbox when due.

### Email Recall
- **Within TASMail org:** Delete from recipient's mailbox via IMAP admin
- **External:** Impossible (SMTP has no recall)
- **Undo Send (recommended):** Delay SMTP submission 5-30s, configurable

### Rust Crates
- **tokio-cron-scheduler** — cron expressions, async, PostgreSQL persistence
- **apalis** — full job queue with PostgreSQL/Redis backends

### Database Schema
```sql
CREATE TABLE scheduled_messages (
    id UUID PRIMARY KEY, user_id UUID, send_at TIMESTAMPTZ,
    from_address VARCHAR(255), to_addresses JSONB, subject TEXT,
    body_html TEXT, attachments JSONB,
    status VARCHAR(20) DEFAULT 'scheduled' -- scheduled|sent|cancelled|failed
);
CREATE TABLE snoozed_messages (
    id UUID PRIMARY KEY, user_id UUID, message_uid BIGINT,
    mailbox VARCHAR(255), snooze_until TIMESTAMPTZ,
    status VARCHAR(20) DEFAULT 'snoozed' -- snoozed|restored|cancelled
);
```

**Complexity:** Medium (schedule: 1 week, snooze: 1 week, undo: 2 days)
**Priority:** Medium-High — business users expect from Gmail/Zoho

### Sources
- https://www.postfix.org/SCHEDULER_README.html
- https://www.postfix.org/postsuper.1.html
- https://www.zoho.com/mail/help/email-snooze.html
- https://www.zoho.com/mail/help/unsend-email.html
- https://crates.io/crates/tokio-cron-scheduler

---

## 5. Offline Access & PWA

### How Gmail Offline Works
- IndexedDB v3 for message bodies, headers, labels
- AES-256 encryption tied to OS credentials
- Web Workers for background indexing
- Service Worker intercepts fetch, serves cached content

### Recommended Architecture
| Resource Type | Caching Strategy |
|--------------|-----------------|
| App shell (HTML/CSS/JS) | Cache-first (Workbox precache) |
| API responses (email list) | Stale-while-revalidate |
| Individual emails | Cache-first, background refresh |
| Attachments | Network-only |

### IndexedDB Schema
- `emails`: uid, subject, from, to, date, body_preview, body_html, labels, read_status
- `drafts`: local drafts composed offline
- `sync_queue`: offline actions to replay on reconnect

### Key Libraries
- **Workbox** (Google) — service worker toolkit
- **idb** / **Dexie.js** — IndexedDB wrappers

**Complexity:** Medium-Complex (3-4 weeks)
**Priority:** Very High for Ghana — intermittent connectivity

### Sources
- https://developer.mozilla.org/en-US/docs/Web/Progressive_web_apps/Guides/Offline_and_background_operation
- https://web.dev/learn/pwa/offline-data
- https://web.dev/learn/pwa/workbox
- https://developer.chrome.com/docs/workbox/modules/workbox-background-sync

---

## 6. SSO/SAML/LDAP Integration

### SAML 2.0
**Crate:** `samael` (v0.0.20) — SP-initiated SSO, encrypted assertions, XML signature verification

### OpenID Connect (OIDC)
**Crate:** `axum-oidc` (v0.6.0) — Axum middleware with `OidcClaims`, `OidcAccessToken` extractors
**Alternative:** `openidconnect` — core OIDC client library

### LDAP/Active Directory
**Crate:** `ldap3` — pure-Rust async on Tokio, Kerberos/GSSAPI, NTLM, TLS
**Dovecot integration:** `dovecot-ldap.conf.ext` with `pass_filter` for authentication

### Implementation Order
1. OIDC (easiest, Google/Azure/Okta support)
2. LDAP (enterprise must-have, many Ghana enterprises use AD)
3. SAML 2.0 (enterprise compliance)

**Complexity:** Complex (OIDC: 2 weeks, LDAP: 2 weeks, SAML: 3 weeks)
**Priority:** Medium-High

### Sources
- https://crates.io/crates/samael
- https://crates.io/crates/axum-oidc
- https://crates.io/crates/openidconnect
- https://github.com/inejge/ldap3
- https://doc.dovecot.org/main/core/config/auth/databases/ldap.html
- https://doc.dovecot.org/main/howto/active_directory.html
- https://www.zoho.com/mail/help/adminconsole/saml-authentication.html

---

## 7. Collaboration Features

### Shared Mailboxes
Use Dovecot's built-in shared namespace + ACL plugin (`acl = yes`, `imap_acl = yes`, `vfile` driver).

### Email Delegation
- **Postfix level:** `sender_login_maps` for send-as authorization
- **Send-On-Behalf:** `Sender:` header injection via milter
- **Application level:** delegation grants in PostgreSQL

### Distribution Groups
- **v1.0:** Postfix `virtual_alias_maps` → PostgreSQL (simple, handles 90% of SMB cases)
- **v2.0+:** Mailman 3 for moderation, archives, digests

**Complexity:** Medium
**Priority:** High — shared mailboxes essential for SMBs

### Sources
- https://doc.dovecot.org/main/core/config/shared_mailboxes.html
- https://doc.dovecot.org/main/core/plugins/acl.html
- https://www.postfix.org/VIRTUAL_README.html
- https://shami.blog/2016/04/adding-on-behalf-of-to-outgoing-emails-with-postfix/

---

## 8. AI Features

### Self-Hosted LLM
**Recommended:** Ollama (bundles llama.cpp, OpenAI-compatible API)
**Rust crate:** `ollama-rs`
**Models:** Llama 3.1 8B (summarization), Phi-3.5 Mini (smart replies), nomic-embed-text (embeddings)

### Semantic Search
**pgvector** extension in PostgreSQL + Ollama embedding model

### BYOK (Bring Your Own Key)
Three tiers:
1. Built-in: Local Ollama with Phi-3.5 Mini (free)
2. BYOK: Customer's OpenAI/Anthropic/Gemini key via `llm` crate
3. Premium: TASMail-managed vLLM cluster

### React AI UI
**Assistant-UI** — composable primitives, Ollama/OpenAI/Anthropic support, streaming

**Complexity:** Medium (Ollama) / Complex (vLLM production)
**Priority:** Medium — strong differentiator

### Sources
- https://blog.premai.io/self-hosted-llm-guide-setup-tools-cost-comparison-2026/
- https://docs.rs/crate/ollama-rs/0.1.9
- https://github.com/pgvector/pgvector
- https://www.zoho.com/mail/help/adminconsole/byok-for-ai-integration.html
- https://github.com/assistant-ui/assistant-ui
- https://www.dabafinance.com/en/news/zoho-zia-llm-africa-launch

---

## 9. Enterprise Governance

### Email Archiving
**Piler** (open source) — Postfix `always_bcc` integration, deduplication, full-text indexing
**Alternative:** OpenArchiver (newer, S3 storage, API-friendly)

### DLP
Postfix milter in Rust using `indymilter` crate — keyword/regex scanning, attachment filtering

### Retention Policies
Scheduled Tokio job, per-tenant rules in PostgreSQL, legal hold flag blocks all deletion

**Complexity:** Medium (archiving) / Complex (DLP, legal hold)
**Priority:** Medium for compliance-conscious businesses

### Sources
- https://www.mailpiler.org/
- https://github.com/LogicLabs-OU/OpenArchiver
- http://www.postfix.org/MILTER_README.html
- https://lib.rs/crates/indymilter

---

## 10. Email Templates & Drafts

### Auto-Save Drafts
- Debounced PATCH to `/api/v1/drafts/{id}` (2-3s delay)
- Optimistic mutations via TanStack Query
- Version counter for conflict detection
- Sync to Dovecot Drafts folder via IMAP APPEND

### Email Templates
- TipTap editor for template creation
- `{{variable_name}}` Handlebars-style merge fields
- Rust `handlebars` crate for server-side rendering
- Categories: sales, support, billing

**Complexity:** Medium
**Priority:** High — core email features

### Sources
- https://medium.com/@darius-marlowe/smarter-forms-in-react-building-a-useautosave-hook-with-debounce-and-react-query-d4d7f9bb052e
- https://github.com/noobships/email-template-builder
- https://tiptap.dev/docs/editor/getting-started/overview

---

## 11. Migration & Import Tools

### IMAP-to-IMAP
**imapsync** — Perl, actively maintained, incremental sync, 32+ server implementations

### File Import
- **MBOX:** `mail-parser` crate (Stalwart Labs) → LMTP delivery
- **EML:** `mail-parser` or `eml-parser` crate
- **PST:** `outlook-pst` crate (Microsoft official, pure Rust, read-only)

**Complexity:** Medium (MBOX/EML) / Complex (PST)
**Priority:** High — critical for customer onboarding

### Sources
- https://imapsync.lamiral.info/
- https://crates.io/crates/mail-parser
- https://crates.io/crates/outlook-pst
- https://github.com/microsoft/outlook-pst-rs
- https://www.zoho.com/mail/help/adminconsole/zoho-mail-migration-wizard.html

---

## 12. White Labeling & Multi-Tenancy

### Multi-Tenancy
PostgreSQL Row-Level Security (RLS) with `tenant_id` on all tables — industry consensus for 2025-2026.

### Custom SMTP/IMAP Hostnames
SNI in Postfix (`tls_server_sni_maps`) and Dovecot (`local_name`), automated Let's Encrypt via ACME.

### White-Label Branding
- Tenant branding config in PostgreSQL (logo, colors, CSS, domain)
- React CSS custom properties for theming
- Reseller pricing: $0.50-$2.00/mailbox wholesale, $5-$10 resale

**Complexity:** Medium
**Priority:** High — essential for MSPs reselling to businesses

### Sources
- https://www.simplyblock.io/blog/underated-postgres-multi-tenancy-with-row-level-security/
- https://www.crunchydata.com/blog/row-level-security-for-tenants-in-postgres
- https://www.postfix.org/TLS_README.html
- https://doc.dovecot.org/main/core/config/ssl.html
- https://www.infraforge.ai/blog/white-label-email-hosting

---

## Priority Matrix Summary

| Feature | Complexity | Ghana Priority | Phase |
|---------|-----------|---------------|-------|
| Offline/PWA | Medium-Complex | Very High | v1.0 |
| 2FA/MFA (TOTP + SMS) | Medium | High | v1.0 |
| Auto-save drafts | Medium | High | v1.0 |
| Email templates | Medium | High | v1.0 |
| Shared mailboxes | Medium | High | v1.0 |
| Distribution groups | Simple | High | v1.0 |
| Multi-tenancy (RLS) | Medium | High | v1.0 |
| Migration tools (IMAP/MBOX) | Medium | High | v1.0 |
| White-label branding | Medium | High | v1.0 |
| Schedule send / Undo send | Medium | Medium-High | v1.0 |
| SSO (OIDC + LDAP) | Complex | Medium-High | v1.5 |
| Snooze | Simple-Medium | Medium | v1.5 |
| Email delegation | Medium | Medium | v1.5 |
| BYOK AI | Simple | High | v1.5 |
| AI summarization (Ollama) | Medium | Medium | v1.5 |
| Calendar/Contacts (CalDAV) | Complex | Medium | v2.0 |
| PST import | Complex | Medium | v1.5 |
| Custom SMTP/IMAP per tenant | Complex | Medium | v1.5 |
| WebAuthn/Passkeys | Medium | Medium | v2.0 |
| Semantic search (pgvector) | Medium | Medium | v2.0 |
| Email archiving (Piler) | Medium | Medium | v2.0 |
| Retention policies | Medium | Medium | v2.0 |
| SAML 2.0 | Complex | Low-Medium | v2.0 |
| DLP (milter-based) | Complex | Low | v2.0 |
| E2E Encryption (OpenPGP) | Complex | Low | v2.0+ |
| Legal hold + eDiscovery | Complex | Low | v2.0+ |
| S/MIME | Complex | Low | v3.0 |
