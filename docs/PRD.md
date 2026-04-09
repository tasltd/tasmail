# Product Requirements Document (PRD)
# TASMail — Self-Hosted Email Service

**Version:** 1.0
**Date:** 2026-03-07
**Author:** TAS Engineering
**Status:** Draft

---

## 1. Executive Summary

TASMail is a self-hosted email service that provides a modern webmail experience through a React single-page application (SPA) frontend connected to a high-performance Rust backend (Axum). It leverages proven open-source Linux mail engines — Postfix for SMTP mail transfer and Dovecot for IMAP mail delivery/access — to handle the core email protocol responsibilities. The Rust backend acts as an intelligent API proxy between the browser-based UI and the mail infrastructure, providing authentication, session management, real-time notifications, and a clean REST/WebSocket API.

### 1.1 Problem Statement

Existing self-hosted email solutions fall into two categories:

1. **Legacy webmail clients** (Roundcube, SquirrelMail) — PHP-based, sluggish UIs, poor mobile experience, no real-time push notifications.
2. **Monolithic Docker stacks** (Mailcow, Mail-in-a-Box) — opinionated deployment models that bundle dozens of containers, making customization and debugging difficult.

There is no modern, lightweight solution that:
- Provides a fast, responsive React-based webmail UI
- Uses a performant, memory-safe backend (Rust)
- Integrates cleanly with standard Postfix/Dovecot installations
- Supports real-time email push via IMAP IDLE → WebSocket
- Remains simple enough to deploy on a single VPS

### 1.2 Vision

A fast, secure, and extensible self-hosted email service that feels like a modern SaaS product but runs entirely on your own infrastructure.

---

## 2. Goals and Non-Goals

### 2.1 Goals

| # | Goal | Success Metric |
|---|------|----------------|
| G1 | Full webmail experience (read, compose, reply, forward, search, folders) | Feature parity with Roundcube core features |
| G2 | Sub-200ms UI interactions | Lighthouse performance score > 90 |
| G3 | Real-time email notifications | New emails appear in < 3 seconds via IMAP IDLE |
| G4 | Multi-domain support | Manage 10+ domains from a single installation |
| G5 | Secure by default | TLS everywhere, DKIM/SPF/DMARC, JWT auth, XSS-safe HTML rendering |
| G6 | Easy deployment | Single binary + config files, systemd service, < 30 min setup |
| G7 | Low resource usage | < 100 MB RAM for the Rust backend under normal load |

### 2.2 Non-Goals (v1.0)

| # | Non-Goal | Rationale |
|---|----------|-----------|
| NG1 | CalDAV/CardDAV (calendar/contacts) | Separate concern; users can run Radicale/Nextcloud alongside |
| NG2 | Mobile native apps | React PWA covers mobile; native apps are v2.0+ |
| NG3 | Built-in spam filter | Rspamd runs as a separate milter; not embedded in the Rust backend |
| NG4 | POP3 support | IMAP is the modern standard; POP3 is legacy |
| NG5 | Replace Postfix/Dovecot | We proxy to them, not replace them |
| NG6 | Multi-tenant SaaS mode | Single-installation multi-domain; not a shared hosting platform |

---

## 3. User Personas

### 3.1 Self-Hosting Enthusiast (Primary)

- **Name:** Alex
- **Profile:** Linux sysadmin who runs their own VPS, owns 2-3 domains
- **Needs:** Full control over email infrastructure, privacy, no vendor lock-in
- **Pain Points:** Tired of Roundcube's dated UI; Mailcow is too heavy for a small VPS
- **Tech Comfort:** High — comfortable with CLI, DNS records, systemd

### 3.2 Small Business IT Admin (Secondary)

- **Name:** Jordan
- **Profile:** Manages email for a 10-50 person company
- **Needs:** Multi-domain management, user provisioning, quota management
- **Pain Points:** Google Workspace costs are rising; wants self-hosted alternative
- **Tech Comfort:** Medium — can follow documentation, prefers web-based admin

### 3.3 Privacy-Conscious User (Tertiary)

- **Name:** Sam
- **Profile:** End user who wants email privacy but doesn't manage servers
- **Needs:** Clean, fast webmail UI; encrypted connections; no third-party tracking
- **Pain Points:** Doesn't trust Gmail/Outlook with personal data
- **Tech Comfort:** Low — needs the admin (Alex/Jordan) to set it up

---

## 4. Feature Requirements

### 4.1 Core Email Features (P0 — Must Have)

| ID | Feature | Description |
|----|---------|-------------|
| F1 | Login/Logout | JWT-based authentication against Dovecot user database |
| F2 | Folder Navigation | Display IMAP folder hierarchy (INBOX, Sent, Drafts, Trash, custom) |
| F3 | Message List | Paginated, sortable list with sender, subject, date, flags |
| F4 | Read Email | Render plain text and sanitized HTML email bodies |
| F5 | Compose Email | Rich text editor (TipTap) with To/Cc/Bcc, subject, body |
| F6 | Reply/Forward | Quote original message, maintain threading |
| F7 | Attachments | Upload/download file attachments (up to 25 MB) |
| F8 | Delete/Archive | Move to Trash or Archive folder via IMAP flags |
| F9 | Mark Read/Unread/Star | Toggle IMAP flags (\Seen, \Flagged) |
| F10 | Search | Full-text search using IMAP SEARCH or Dovecot FTS |
| F11 | Real-time Updates | WebSocket push from IMAP IDLE for new messages |

### 4.2 Admin Features (P1 — Should Have)

| ID | Feature | Description |
|----|---------|-------------|
| F12 | Domain Management | Add/remove/activate email domains |
| F13 | User Management | Create/delete/disable mailbox accounts |
| F14 | Quota Management | Set per-user storage quotas |
| F15 | Alias Management | Create email aliases and forwards |
| F16 | Admin Dashboard | Overview of system health, storage usage, active sessions |

### 4.3 Nice-to-Have (P2)

| ID | Feature | Description |
|----|---------|-------------|
| F17 | Theme Support | Light/dark mode toggle |
| F18 | Keyboard Shortcuts | Gmail-like keyboard navigation |
| F19 | Drag & Drop | Move messages between folders |
| F20 | Contact Autocomplete | Suggest recipients from previous correspondence |
| F21 | Email Signatures | Per-account configurable signatures |
| F22 | Sieve Filter Rules | UI for creating Dovecot Sieve mail filter rules |

---

## 5. System Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                    Internet / DNS                        │
│  MX → mail.example.com    SPF/DKIM/DMARC records       │
└──────────────┬──────────────────────────────┬───────────┘
               │                              │
        Port 25 (SMTP)                 Port 443 (HTTPS)
               │                              │
┌──────────────▼──────────┐    ┌──────────────▼──────────┐
│        Postfix           │    │    Nginx / Reverse Proxy │
│  (MTA — Mail Transfer)   │    │    TLS Termination       │
│  • Receives inbound mail │    └──────────────┬──────────┘
│  • Sends outbound mail   │                   │
│  • Rate limiting         │            Port 3000 (HTTP)
│  • Rspamd milter         │                   │
└──────┬──────────┬────────┘    ┌──────────────▼──────────┐
       │          │             │   Axum Rust Backend       │
  LMTP │    SASL  │             │   • REST API              │
(delivery) (auth) │             │   • WebSocket server      │
       │          │             │   • JWT authentication    │
┌──────▼──────────▼────────┐    │   • IMAP proxy (async)    │
│        Dovecot            │◄──│   • SMTP proxy (lettre)   │
│  (MDA — Mail Delivery)    │    │   • Admin API             │
│  • IMAP access            │    └──────────────┬──────────┘
│  • LMTP delivery          │                   │
│  • Sieve filtering        │            ┌──────▼──────┐
│  • SASL auth for Postfix  │            │ PostgreSQL   │
│  • FTS (Xapian/Flatcurve) │            │ • Users      │
│  • Quota enforcement      │            │ • Domains    │
└───────────────────────────┘            │ • Aliases    │
                                         │ • Sessions   │
         ┌──────────────────┐            │ • Settings   │
         │   React SPA       │            └─────────────┘
         │   (Vite + TS)     │
         │   • Webmail UI    │
         │   • Admin panel   │
         │   • PWA support   │
         └──────────────────┘
```

### 5.1 Component Responsibilities

| Component | Responsibility | Technology |
|-----------|---------------|------------|
| **React SPA** | User interface — webmail and admin panel | React 19, Vite, TypeScript, TanStack Query, Zustand, TipTap |
| **Axum Backend** | API server, WebSocket hub, IMAP/SMTP proxy | Rust, Axum 0.7+, tokio, async-imap, lettre, sqlx |
| **PostgreSQL** | User accounts, domains, aliases, sessions, settings | PostgreSQL 16+ |
| **Postfix** | Inbound/outbound SMTP, relay, rate limiting | Postfix 3.8+ |
| **Dovecot** | IMAP access, LMTP delivery, SASL auth, Sieve, FTS | Dovecot 2.3+ |
| **Rspamd** | Spam/virus filtering (milter interface to Postfix) | Rspamd 3.x |
| **Nginx** | TLS termination, static file serving, reverse proxy | Nginx 1.24+ |
| **Let's Encrypt** | Automated TLS certificate provisioning | certbot |

---

## 6. Technical Requirements

### 6.1 Performance

| Metric | Target |
|--------|--------|
| API response time (message list) | < 150 ms (p95) |
| Message body fetch + render | < 300 ms (p95) |
| New email push notification | < 3 seconds from delivery |
| Concurrent users per instance | 100+ |
| Backend memory usage (idle) | < 50 MB |
| Backend memory usage (100 users) | < 200 MB |

### 6.2 Security

| Requirement | Implementation |
|-------------|----------------|
| Authentication | JWT with RS256, 15-min access + 7-day refresh tokens |
| Password storage | Argon2id hashing (in PostgreSQL via Rust backend) |
| TLS | Required for all external connections (HTTPS, IMAPS, SMTPS) |
| Email authentication | SPF, DKIM (2048-bit RSA), DMARC |
| HTML rendering | DOMPurify sanitization; no inline scripts or event handlers |
| Rate limiting | API rate limiting via Tower middleware |
| CSRF protection | SameSite cookies + CSRF tokens for state-changing operations |
| Input validation | Server-side validation for all API inputs |

### 6.3 Compatibility

| Requirement | Target |
|-------------|--------|
| OS | Ubuntu 22.04 LTS / 24.04 LTS, Debian 12+ |
| Rust | 1.78+ (2021 edition) |
| Node.js | 20 LTS (for frontend build) |
| PostgreSQL | 16+ |
| Browsers | Chrome 120+, Firefox 120+, Safari 17+, Edge 120+ |

---

## 7. Data Model

### 7.1 Core Entities

```
domains (id, domain, active, created_at)
    ├── mailboxes (id, username, password_hash, domain_id, quota, active, created_at)
    ├── aliases (id, source, destination, domain_id, active)
    └── dkim_keys (id, domain_id, selector, private_key, public_key)

sessions (id, user_id, refresh_token_hash, expires_at, created_at, ip_address)
settings (id, user_id, key, value)  -- per-user preferences
```

### 7.2 Mail Storage

Mail is stored by Dovecot in Maildir format at `/var/vmail/{domain}/{user}/Maildir/`. The Rust backend does **not** directly access mail files — it communicates exclusively through IMAP protocol to Dovecot.

---

## 8. API Overview

### 8.1 REST Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/auth/login` | Authenticate user, return JWT |
| POST | `/api/auth/refresh` | Refresh access token |
| POST | `/api/auth/logout` | Invalidate refresh token |
| GET | `/api/folders` | List IMAP folders |
| GET | `/api/folders/:name/messages` | List messages (paginated) |
| GET | `/api/messages/:uid` | Get full message (headers + body + attachments) |
| POST | `/api/messages` | Send new email |
| POST | `/api/messages/:uid/reply` | Reply to message |
| POST | `/api/messages/:uid/forward` | Forward message |
| PATCH | `/api/messages/:uid/flags` | Update flags (read/star/delete) |
| DELETE | `/api/messages/:uid` | Move to Trash |
| POST | `/api/messages/:uid/move` | Move to folder |
| GET | `/api/search` | Search messages |
| GET | `/api/contacts/autocomplete` | Suggest recipients |

### 8.2 WebSocket

| Endpoint | Description |
|----------|-------------|
| `WS /ws/notifications` | IMAP IDLE relay — pushes new email events to the browser |

### 8.3 Admin API

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/admin/domains` | List domains |
| POST | `/api/admin/domains` | Add domain |
| DELETE | `/api/admin/domains/:id` | Remove domain |
| GET | `/api/admin/users` | List users |
| POST | `/api/admin/users` | Create user |
| PATCH | `/api/admin/users/:id` | Update user (quota, active) |
| DELETE | `/api/admin/users/:id` | Deactivate user |
| GET | `/api/admin/aliases` | List aliases |
| POST | `/api/admin/aliases` | Create alias |
| GET | `/api/admin/stats` | System statistics |

---

## 9. User Flows

### 9.1 Read Email Flow

1. User opens TASMail in browser → React SPA loads
2. SPA calls `POST /api/auth/login` with credentials → JWT returned
3. SPA calls `GET /api/folders` → folder tree rendered
4. SPA calls `GET /api/folders/INBOX/messages?page=1` → message list rendered
5. User clicks a message → SPA calls `GET /api/messages/:uid` → message rendered
6. Backend marks message as read via IMAP `STORE \Seen`

### 9.2 Send Email Flow

1. User clicks "Compose" → editor opens with TipTap rich text
2. User fills To, Subject, Body, attaches files
3. SPA calls `POST /api/messages` with multipart form data
4. Backend constructs MIME message using `lettre` Message builder
5. Backend sends via SMTP to Postfix on port 587 (authenticated submission)
6. Postfix signs with DKIM, delivers outbound
7. Copy saved to Sent folder via IMAP APPEND

### 9.3 Real-Time Notification Flow

1. After login, SPA opens WebSocket to `WS /ws/notifications`
2. Backend starts IMAP IDLE session for the user's INBOX
3. When Dovecot signals new mail via IDLE response
4. Backend fetches envelope of new message
5. Backend pushes `{ type: "new_mail", folder: "INBOX", uid: 12345, from: "...", subject: "..." }` over WebSocket
6. SPA shows notification badge and prepends message to list

---

## 10. Deployment Model

### 10.1 Minimum Server Requirements

| Resource | Minimum | Recommended |
|----------|---------|-------------|
| CPU | 1 vCPU | 2 vCPU |
| RAM | 1 GB | 2 GB |
| Storage | 20 GB SSD | 50+ GB SSD |
| OS | Ubuntu 22.04 LTS | Ubuntu 24.04 LTS |
| Public IP | 1 static IPv4 | + IPv6 |

### 10.2 Deployment Steps (High Level)

1. Install Postfix, Dovecot, PostgreSQL, Rspamd, Nginx, certbot
2. Configure DNS: MX, A/AAAA, SPF, DKIM, DMARC, rDNS
3. Configure PostgreSQL schema (Flyway migrations or SQL scripts)
4. Configure Postfix (main.cf, master.cf, virtual maps)
5. Configure Dovecot (SQL auth, LMTP, Sieve, quotas)
6. Deploy Rust backend binary + config file
7. Deploy React SPA (static build served by Nginx)
8. Configure Nginx reverse proxy (API → Axum, static → SPA)
9. Obtain Let's Encrypt TLS certificate
10. Start all services, verify with test emails

---

## 11. Success Criteria

| Milestone | Criteria | Timeline |
|-----------|----------|----------|
| **M1: Foundation** | Backend compiles, connects to Dovecot/Postfix, basic CRUD via API | Week 4 |
| **M2: Core Webmail** | Read, compose, reply, folders, search working in React UI | Week 8 |
| **M3: Real-Time** | WebSocket push notifications from IMAP IDLE | Week 10 |
| **M4: Admin Panel** | Domain/user/alias management via web UI | Week 12 |
| **M5: Security Hardening** | DKIM signing, rate limiting, audit logging, pen test | Week 14 |
| **M6: Beta Release** | Deployed on production VPS, 5+ beta users | Week 16 |

---

## 12. Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Email deliverability (spam classification) | High | Proper DNS (SPF/DKIM/DMARC), warm-up sending reputation, monitor blacklists |
| IMAP protocol complexity | Medium | Use battle-tested `async-imap` crate; test against Dovecot specifically |
| HTML email rendering (XSS) | High | Strict DOMPurify sanitization; sandboxed iframe fallback |
| Concurrent IMAP connections | Medium | Connection pooling per user; IDLE session limits |
| Postfix/Dovecot version drift | Low | Pin versions; test upgrades in staging |

---

## 13. Open Questions

| # | Question | Owner | Status |
|---|----------|-------|--------|
| Q1 | Should we support S/MIME or PGP encryption in v1? | Product | Deferred to v2 |
| Q2 | Should admin UI be a separate SPA or routes within the same app? | Engineering | Decision: Same SPA with role-based routing |
| Q3 | Do we need ActiveSync support for mobile clients? | Product | Deferred to v2 |
| Q4 | Should we build our own spam filter or always require Rspamd? | Engineering | Decision: Require Rspamd as external dependency |

---

## 14. References

- [Postfix Documentation](http://www.postfix.org/documentation.html)
- [Dovecot Documentation](https://doc.dovecot.org/)
- [Stalwart Mail Server (Rust reference)](https://github.com/stalwartlabs/stalwart)
- [Mailcow Architecture](https://docs.mailcow.email/)
- [Axum Framework](https://github.com/tokio-rs/axum)
- [lettre SMTP crate](https://docs.rs/lettre/)
- [async-imap crate](https://docs.rs/async-imap/)
