# System/Software Requirements Specification (SRS)
# RustMail — Self-Hosted Email Service

**Version:** 1.0
**Date:** 2026-03-07
**Standard:** Based on IEEE 830-1998 / ISO/IEC/IEEE 29148:2018
**Status:** Draft

---

## 1. Introduction

### 1.1 Purpose

This Software Requirements Specification defines the functional and non-functional requirements for RustMail, a self-hosted email service consisting of a React frontend, Rust backend (Axum), and integration with Postfix (SMTP) and Dovecot (IMAP/LMTP). This document serves as the authoritative reference for all development, testing, and deployment activities.

### 1.2 Scope

RustMail provides:
- A webmail interface for reading, composing, and managing email
- A REST/WebSocket API backend that proxies IMAP and SMTP operations
- An admin interface for managing domains, users, aliases, and quotas
- Integration with standard Linux mail infrastructure (Postfix + Dovecot)
- Real-time email notifications via IMAP IDLE → WebSocket bridge

RustMail does **not** replace Postfix or Dovecot — it acts as an intelligent middleware layer between the browser and these established mail engines.

### 1.3 Definitions, Acronyms, and Abbreviations

| Term | Definition |
|------|-----------|
| MTA | Mail Transfer Agent — Postfix; handles SMTP send/receive |
| MDA | Mail Delivery Agent — Dovecot; delivers mail to mailboxes |
| LMTP | Local Mail Transfer Protocol — Postfix→Dovecot delivery |
| SASL | Simple Authentication and Security Layer — auth mechanism |
| IMAP | Internet Message Access Protocol — mail reading |
| SMTP | Simple Mail Transfer Protocol — mail sending |
| JWT | JSON Web Token — API authentication |
| SPA | Single-Page Application — React frontend |
| FTS | Full-Text Search — Dovecot search indexing |
| DKIM | DomainKeys Identified Mail — email signing |
| SPF | Sender Policy Framework — DNS-based sender validation |
| DMARC | Domain-based Message Authentication, Reporting & Conformance |
| MIME | Multipurpose Internet Mail Extensions — email format |
| Maildir | One-file-per-message mail storage format |

### 1.4 References

| Document | Description |
|----------|-------------|
| PRD v1.0 | Product Requirements Document |
| RFC 5321 | SMTP Protocol |
| RFC 9051 | IMAP4rev2 |
| RFC 6376 | DKIM Signatures |
| RFC 7489 | DMARC |
| RFC 5228 | Sieve Email Filtering |
| OWASP Top 10 | Web Application Security Risks |

---

## 2. Overall Description

### 2.1 Product Perspective

RustMail is a new product that integrates with existing open-source mail infrastructure. It is not a modification of an existing system. The system boundary is:

```
External Systems (Existing)         RustMail (New)              External Systems (Existing)
┌─────────────────────┐      ┌───────────────────────┐      ┌─────────────────────┐
│ Remote SMTP Servers  │◄────►│ Postfix (MTA)          │      │ DNS Servers          │
│ (Gmail, Outlook...)  │      │                        │      │ (MX, SPF, DKIM)      │
└─────────────────────┘      └───────┬────────────────┘      └─────────────────────┘
                                     │ LMTP / SASL
                              ┌──────▼────────────────┐
                              │ Dovecot (MDA/IMAP)     │
                              └───────┬────────────────┘
                                      │ IMAP / SMTP
                              ┌───────▼────────────────┐
                              │ Axum Backend (NEW)      │◄───► PostgreSQL
                              └───────┬────────────────┘
                                      │ REST / WebSocket
                              ┌───────▼────────────────┐
                              │ React SPA (NEW)         │◄───► User Browser
                              └────────────────────────┘
```

### 2.2 Product Functions

1. **Email Operations** — Read, compose, reply, forward, delete, search, organize
2. **Real-Time Sync** — Push notifications for new emails
3. **Admin Management** — Domains, users, aliases, quotas
4. **Authentication** — JWT-based user auth against PostgreSQL/Dovecot
5. **Security** — TLS, DKIM signing, HTML sanitization, rate limiting

### 2.3 User Characteristics

| User Class | Technical Level | Access |
|------------|----------------|--------|
| Email User | Low-Medium | Webmail UI only |
| Domain Admin | Medium-High | Webmail + Admin panel |
| System Admin | High | Full server access + CLI |

### 2.4 Constraints

| Constraint | Description |
|------------|-------------|
| C1 | Must integrate with standard Postfix 3.5+ and Dovecot 2.3+ |
| C2 | Must run on Linux (Ubuntu/Debian); no Windows/macOS server support |
| C3 | Must not require Docker for deployment |
| C4 | Backend must compile to a single binary (Rust) |
| C5 | Frontend must be a static SPA (no SSR required) |
| C6 | Must support PostgreSQL only (no MySQL/SQLite) |

### 2.5 Assumptions and Dependencies

| ID | Assumption |
|----|-----------|
| A1 | Server has a static public IP with proper rDNS configured |
| A2 | DNS records (MX, A, AAAA) are correctly configured |
| A3 | TLS certificates are provisioned (Let's Encrypt or manual) |
| A4 | Postfix and Dovecot are installed and minimally configured |
| A5 | PostgreSQL 16+ is available |
| A6 | Rspamd is deployed for spam filtering (optional but recommended) |

---

## 3. Functional Requirements

### 3.1 Authentication and Authorization

#### FR-AUTH-001: User Login
- **Input:** Username (email address), password
- **Process:** Validate credentials against PostgreSQL `mailboxes` table; password verified using Argon2id
- **Output:** JWT access token (15-min expiry) + refresh token (7-day expiry, HttpOnly cookie)
- **Error:** 401 Unauthorized with generic "Invalid credentials" message (no user enumeration)

#### FR-AUTH-002: Token Refresh
- **Input:** Valid refresh token (from HttpOnly cookie)
- **Process:** Validate refresh token against `sessions` table; issue new access + refresh tokens; rotate refresh token
- **Output:** New JWT access token + rotated refresh token
- **Error:** 401 if refresh token is expired or revoked

#### FR-AUTH-003: Logout
- **Input:** Valid access token
- **Process:** Delete session record from `sessions` table; clear HttpOnly cookie
- **Output:** 204 No Content

#### FR-AUTH-004: Role-Based Access
- **Roles:** `user`, `domain_admin`, `super_admin`
- **Enforcement:** Middleware checks JWT claims before routing to admin endpoints
- **Rule:** Users can only access their own mailbox; domain admins can manage users within their domains; super admins have full access

### 3.2 Folder Operations

#### FR-FOLDER-001: List Folders
- **Input:** Authenticated user
- **Process:** Connect to Dovecot via IMAP; execute `LIST "" "*"` command
- **Output:** JSON array of folder objects:
  ```json
  [
    { "name": "INBOX", "delimiter": "/", "flags": ["\\HasNoChildren"], "unseen": 12, "total": 342 },
    { "name": "Sent", "delimiter": "/", "flags": ["\\HasNoChildren", "\\Sent"], "unseen": 0, "total": 156 }
  ]
  ```
- **Cache:** Folder list cached for 60 seconds; invalidated on folder changes

#### FR-FOLDER-002: Create Folder
- **Input:** Folder name (must not contain delimiter `/` or special characters)
- **Process:** IMAP `CREATE` command; IMAP `SUBSCRIBE` command
- **Output:** 201 Created with folder object

#### FR-FOLDER-003: Delete Folder
- **Input:** Folder name (cannot delete INBOX, Sent, Drafts, Trash)
- **Process:** IMAP `DELETE` command
- **Output:** 204 No Content

#### FR-FOLDER-004: Rename Folder
- **Input:** Current name, new name
- **Process:** IMAP `RENAME` command
- **Output:** 200 OK with updated folder object

### 3.3 Message Operations

#### FR-MSG-001: List Messages
- **Input:** Folder name, page (default 1), limit (default 50, max 200), sort (date/from/subject, default date desc)
- **Process:** IMAP `SELECT` folder; `FETCH` range with `(FLAGS ENVELOPE RFC822.SIZE)`
- **Output:**
  ```json
  {
    "messages": [
      {
        "uid": 12345,
        "from": { "name": "John Doe", "email": "john@example.com" },
        "to": [{ "name": "Me", "email": "me@example.com" }],
        "subject": "Meeting Tomorrow",
        "date": "2026-03-07T14:30:00Z",
        "flags": ["\\Seen"],
        "size": 4523,
        "has_attachments": true
      }
    ],
    "total": 342,
    "page": 1,
    "pages": 7
  }
  ```

#### FR-MSG-002: Get Message
- **Input:** Message UID
- **Process:** IMAP `UID FETCH` with `RFC822`; parse MIME with `mailparse`
- **Output:**
  ```json
  {
    "uid": 12345,
    "from": { "name": "John Doe", "email": "john@example.com" },
    "to": [{ "name": "Me", "email": "me@example.com" }],
    "cc": [],
    "bcc": [],
    "subject": "Meeting Tomorrow",
    "date": "2026-03-07T14:30:00Z",
    "flags": ["\\Seen"],
    "body_text": "Plain text version...",
    "body_html": "<p>Sanitized HTML version...</p>",
    "attachments": [
      { "id": "att1", "filename": "agenda.pdf", "mime_type": "application/pdf", "size": 102400 }
    ],
    "headers": {
      "message_id": "<abc123@example.com>",
      "in_reply_to": null,
      "references": []
    }
  }
  ```
- **Side Effect:** Marks message as `\Seen` if not already

#### FR-MSG-003: Send Message
- **Input:** To (required), Cc, Bcc, Subject, Body (HTML), Attachments (multipart)
- **Process:**
  1. Build MIME message using `lettre::Message`
  2. Send via SMTP to Postfix port 587 with SASL authentication
  3. Append copy to IMAP "Sent" folder via `APPEND`
- **Output:** 201 Created with `{ "message_id": "<generated@example.com>" }`
- **Validation:**
  - At least one recipient required
  - Total message size < 25 MB
  - Attachment filenames sanitized

#### FR-MSG-004: Reply to Message
- **Input:** Original message UID, Reply body, Reply-All flag
- **Process:**
  1. Fetch original message headers (In-Reply-To, References, From, To, Cc)
  2. Set `In-Reply-To` to original `Message-ID`
  3. Append original `Message-ID` to `References` header
  4. If Reply-All: include original To and Cc (excluding self)
  5. Quote original body with `>` prefix
  6. Send via SMTP; append to Sent
- **Output:** 201 Created

#### FR-MSG-005: Forward Message
- **Input:** Original message UID, new recipients, optional additional body
- **Process:**
  1. Fetch original message with all attachments
  2. Build new message with original as attachment (RFC 2046) or inline
  3. Send via SMTP; append to Sent
- **Output:** 201 Created

#### FR-MSG-006: Update Message Flags
- **Input:** Message UID, flags to add/remove
- **Process:** IMAP `UID STORE` with `+FLAGS` or `-FLAGS`
- **Supported Flags:** `\Seen` (read), `\Flagged` (starred), `\Deleted`, `\Answered`
- **Output:** 200 OK

#### FR-MSG-007: Move Message
- **Input:** Message UID, destination folder
- **Process:** IMAP `UID MOVE` (or `UID COPY` + `UID STORE \Deleted` + `EXPUNGE` for older servers)
- **Output:** 200 OK

#### FR-MSG-008: Delete Message
- **Input:** Message UID
- **Process:** Move to Trash folder (not permanent delete). If already in Trash, mark `\Deleted` and `EXPUNGE`.
- **Output:** 204 No Content

#### FR-MSG-009: Download Attachment
- **Input:** Message UID, attachment ID
- **Process:** Fetch specific MIME part via IMAP `BODY[part_number]`
- **Output:** Binary file with correct `Content-Type` and `Content-Disposition` headers

### 3.4 Search

#### FR-SEARCH-001: Search Messages
- **Input:** Query string, folder (optional, default all), filters (from, to, date range, has attachment)
- **Process:** IMAP `SEARCH` command with criteria; or Dovecot FTS if configured
- **Output:** Paginated message list (same format as FR-MSG-001)
- **Search Criteria Mapping:**
  - `from:john` → IMAP `FROM "john"`
  - `subject:meeting` → IMAP `SUBJECT "meeting"`
  - `has:attachment` → IMAP `HEADER Content-Type multipart/mixed`
  - `after:2026-01-01` → IMAP `SINCE 01-Jan-2026`
  - Free text → IMAP `TEXT "query"` or FTS

### 3.5 Real-Time Notifications

#### FR-RT-001: WebSocket Connection
- **Input:** Valid JWT token (via query parameter or first message)
- **Process:**
  1. Authenticate JWT
  2. Open IMAP connection to user's INBOX
  3. Start IMAP IDLE command
  4. When IDLE signals `EXISTS` (new message), break IDLE
  5. Fetch new message envelope
  6. Push JSON event to WebSocket
  7. Restart IDLE
- **Output:** Server-sent JSON events:
  ```json
  { "type": "new_mail", "folder": "INBOX", "uid": 12345, "from": "john@example.com", "subject": "Hello" }
  { "type": "flags_changed", "folder": "INBOX", "uid": 12345, "flags": ["\\Seen"] }
  { "type": "expunge", "folder": "INBOX", "uid": 12345 }
  ```
- **Heartbeat:** Server sends `{ "type": "ping" }` every 60 seconds
- **Reconnection:** Client auto-reconnects with exponential backoff (1s, 2s, 4s, 8s, max 30s)

### 3.6 Admin Operations

#### FR-ADMIN-001: Manage Domains
- **List:** GET `/api/admin/domains` → `[{ "id": 1, "domain": "example.com", "active": true, "user_count": 5, "created_at": "..." }]`
- **Create:** POST with `{ "domain": "newdomain.com" }` → validates DNS MX record exists
- **Update:** PATCH with `{ "active": false }` → disables all mailboxes in domain
- **Delete:** DELETE → soft-delete (deactivate); requires confirmation

#### FR-ADMIN-002: Manage Users
- **List:** GET `/api/admin/users?domain=example.com` → paginated user list
- **Create:** POST with `{ "username": "user@example.com", "password": "...", "quota": 1073741824 }`
  - Validates domain exists and is active
  - Hashes password with Argon2id
  - Creates Maildir on disk (or Dovecot auto-creates on first delivery)
- **Update:** PATCH with `{ "quota": ..., "active": ..., "password": "..." }`
- **Delete:** Deactivate account (mail preserved); optional permanent purge

#### FR-ADMIN-003: Manage Aliases
- **List:** GET `/api/admin/aliases?domain=example.com`
- **Create:** POST with `{ "source": "info@example.com", "destination": "user@example.com" }`
- **Delete:** DELETE by alias ID

#### FR-ADMIN-004: System Dashboard
- **Metrics:** Active users, total mailboxes, total storage used, messages today, queue size
- **Sources:** PostgreSQL queries + Postfix `mailq` + Dovecot `doveadm quota get`

---

## 4. Non-Functional Requirements

### 4.1 Performance Requirements

| ID | Requirement | Metric |
|----|-------------|--------|
| NFR-PERF-001 | API response time for message list | p95 < 150 ms |
| NFR-PERF-002 | Full message fetch + parse | p95 < 300 ms |
| NFR-PERF-003 | Send message | p95 < 500 ms |
| NFR-PERF-004 | Search (IMAP SEARCH) | p95 < 1000 ms |
| NFR-PERF-005 | WebSocket push latency | < 3 seconds from LMTP delivery |
| NFR-PERF-006 | SPA initial load (cached) | < 2 seconds |
| NFR-PERF-007 | SPA bundle size | < 500 KB gzipped |

### 4.2 Scalability Requirements

| ID | Requirement |
|----|-------------|
| NFR-SCALE-001 | Support 100 concurrent users per single server instance |
| NFR-SCALE-002 | Support 50 simultaneous IMAP IDLE connections |
| NFR-SCALE-003 | Support 10,000 messages per folder without degradation |
| NFR-SCALE-004 | Support 100 domains per installation |
| NFR-SCALE-005 | Support 1,000 mailboxes per installation |

### 4.3 Security Requirements

| ID | Requirement | Implementation |
|----|-------------|----------------|
| NFR-SEC-001 | All external connections encrypted with TLS 1.2+ | Nginx TLS termination; STARTTLS for SMTP |
| NFR-SEC-002 | Passwords stored as Argon2id hashes | Rust `argon2` crate with recommended parameters |
| NFR-SEC-003 | JWT tokens signed with RS256 | RSA-2048 key pair; tokens contain user ID, role, expiry |
| NFR-SEC-004 | HTML email bodies sanitized before rendering | DOMPurify with strict allowlist; no `<script>`, no event handlers |
| NFR-SEC-005 | API rate limiting | 100 requests/minute per user; 10 login attempts/minute per IP |
| NFR-SEC-006 | CSRF protection | SameSite=Strict cookies; X-CSRF-Token for state-changing requests |
| NFR-SEC-007 | SQL injection prevention | Parameterized queries via sqlx; no raw SQL string concatenation |
| NFR-SEC-008 | IMAP credentials never sent to browser | Backend holds IMAP sessions; browser only sees JWT |
| NFR-SEC-009 | Outbound email signed with DKIM | 2048-bit RSA key; signing handled by Postfix/OpenDKIM or Rust mail-auth |
| NFR-SEC-010 | Fail2ban integration | Ban IPs after 5 failed login attempts for 1 hour |

### 4.4 Reliability Requirements

| ID | Requirement |
|----|-------------|
| NFR-REL-001 | Backend process auto-restarts on crash (systemd `Restart=always`) |
| NFR-REL-002 | IMAP connection pool auto-reconnects on connection loss |
| NFR-REL-003 | WebSocket clients auto-reconnect with exponential backoff |
| NFR-REL-004 | Database migrations are idempotent and reversible |
| NFR-REL-005 | No data loss on backend restart (all state in PostgreSQL/IMAP) |

### 4.5 Maintainability Requirements

| ID | Requirement |
|----|-------------|
| NFR-MAINT-001 | Backend compiles to a single static binary |
| NFR-MAINT-002 | Configuration via TOML file (not environment variables only) |
| NFR-MAINT-003 | Structured JSON logging (compatible with journald) |
| NFR-MAINT-004 | Database schema managed via versioned migration files |
| NFR-MAINT-005 | Frontend builds to static files (served by Nginx, no Node.js runtime) |

### 4.6 Usability Requirements

| ID | Requirement |
|----|-------------|
| NFR-USE-001 | Responsive design: desktop (1024px+), tablet (768px+), mobile (375px+) |
| NFR-USE-002 | Keyboard navigation: j/k (prev/next), r (reply), c (compose), / (search) |
| NFR-USE-003 | Loading states for all async operations (skeleton screens, not spinners) |
| NFR-USE-004 | Error messages are user-friendly (not raw HTTP status codes) |
| NFR-USE-005 | Dark mode support (system preference detection + manual toggle) |

---

## 5. Interface Requirements

### 5.1 User Interface

The React SPA provides the following views:

| View | Route | Description |
|------|-------|-------------|
| Login | `/login` | Email + password form |
| Inbox | `/mail/INBOX` | Default after login; message list |
| Folder View | `/mail/:folder` | Any IMAP folder |
| Message View | `/mail/:folder/:uid` | Full message display |
| Compose | `/compose` | New email composer |
| Reply | `/compose?reply=:uid` | Reply to message |
| Search Results | `/search?q=...` | Search results |
| Settings | `/settings` | User preferences |
| Admin: Domains | `/admin/domains` | Domain management |
| Admin: Users | `/admin/users` | User management |
| Admin: Aliases | `/admin/aliases` | Alias management |
| Admin: Dashboard | `/admin` | System overview |

### 5.2 Hardware Interfaces

None — RustMail is a software-only system.

### 5.3 Software Interfaces

| External System | Interface | Protocol | Port |
|-----------------|-----------|----------|------|
| Postfix | SMTP submission | SMTP/STARTTLS | 587 |
| Dovecot | Mail reading | IMAPS | 993 |
| Dovecot | LMTP (Postfix→Dovecot) | LMTP (Unix socket) | N/A |
| Dovecot | SASL (Postfix auth) | SASL (Unix socket) | N/A |
| PostgreSQL | Data storage | PostgreSQL wire protocol | 5432 |
| Let's Encrypt | TLS certificates | ACME | 80/443 |
| Rspamd | Spam filtering | Milter | 11332 |

### 5.4 Communication Interfaces

| Interface | Protocol | Format |
|-----------|----------|--------|
| REST API | HTTPS | JSON |
| WebSocket | WSS | JSON events |
| IMAP Proxy | TCP/TLS | IMAP4rev1/rev2 |
| SMTP Proxy | TCP/TLS | SMTP (RFC 5321) |

---

## 6. Database Schema

### 6.1 Entity-Relationship Diagram

```
┌──────────────┐     ┌──────────────────┐     ┌──────────────┐
│   domains     │     │    mailboxes      │     │   aliases     │
├──────────────┤     ├──────────────────┤     ├──────────────┤
│ id (PK)      │←───┐│ id (PK)          │  ┌─→│ id (PK)      │
│ domain       │    ││ username (UNIQUE) │  │  │ source       │
│ active       │    ││ password_hash    │  │  │ destination  │
│ created_at   │    ││ domain_id (FK)───│──┘  │ domain_id(FK)│
│ updated_at   │    ││ display_name     │     │ active       │
└──────────────┘    ││ quota            │     │ created_at   │
                    ││ role             │     └──────────────┘
                    ││ active           │
                    ││ created_at       │     ┌──────────────┐
                    ││ updated_at       │     │  sessions     │
                    │└──────────────────┘     ├──────────────┤
                    │                    ┌───→│ id (PK)      │
                    │                    │    │ user_id (FK) │
                    │                    │    │ refresh_hash │
                    └────────────────────┘    │ ip_address   │
                                              │ user_agent   │
                                              │ expires_at   │
                                              │ created_at   │
                                              └──────────────┘

┌──────────────┐     ┌──────────────────┐
│  dkim_keys    │     │   settings        │
├──────────────┤     ├──────────────────┤
│ id (PK)      │     │ id (PK)          │
│ domain_id(FK)│     │ user_id (FK)     │
│ selector     │     │ key              │
│ private_key  │     │ value (JSONB)    │
│ public_key   │     │ updated_at       │
│ active       │     └──────────────────┘
│ created_at   │
└──────────────┘
```

### 6.2 Table Definitions

```sql
-- Domains
CREATE TABLE domains (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain      VARCHAR(255) NOT NULL UNIQUE,
    active      BOOLEAN NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Mailboxes
CREATE TABLE mailboxes (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username      VARCHAR(255) NOT NULL UNIQUE,  -- full email: user@domain.com
    password_hash VARCHAR(255) NOT NULL,          -- Argon2id hash
    domain_id     UUID NOT NULL REFERENCES domains(id),
    display_name  VARCHAR(255),
    quota         BIGINT NOT NULL DEFAULT 1073741824,  -- 1 GB in bytes
    role          VARCHAR(20) NOT NULL DEFAULT 'user'
                  CHECK (role IN ('user', 'domain_admin', 'super_admin')),
    active        BOOLEAN NOT NULL DEFAULT TRUE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Aliases
CREATE TABLE aliases (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source      VARCHAR(255) NOT NULL,
    destination VARCHAR(255) NOT NULL,
    domain_id   UUID NOT NULL REFERENCES domains(id),
    active      BOOLEAN NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(source, destination)
);

-- Sessions (refresh tokens)
CREATE TABLE sessions (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id       UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    refresh_hash  VARCHAR(255) NOT NULL,  -- SHA-256 of refresh token
    ip_address    INET,
    user_agent    TEXT,
    expires_at    TIMESTAMPTZ NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- DKIM Keys
CREATE TABLE dkim_keys (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id   UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    selector    VARCHAR(63) NOT NULL DEFAULT 'default',
    private_key TEXT NOT NULL,
    public_key  TEXT NOT NULL,
    active      BOOLEAN NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(domain_id, selector)
);

-- User Settings
CREATE TABLE settings (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    key        VARCHAR(100) NOT NULL,
    value      JSONB NOT NULL DEFAULT '{}',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, key)
);

-- Indexes
CREATE INDEX idx_mailboxes_domain ON mailboxes(domain_id);
CREATE INDEX idx_mailboxes_active ON mailboxes(active) WHERE active = TRUE;
CREATE INDEX idx_aliases_domain ON aliases(domain_id);
CREATE INDEX idx_aliases_source ON aliases(source);
CREATE INDEX idx_sessions_user ON sessions(user_id);
CREATE INDEX idx_sessions_expires ON sessions(expires_at);
CREATE INDEX idx_settings_user ON settings(user_id);
```

---

## 7. Configuration Requirements

### 7.1 Backend Configuration (TOML)

```toml
[server]
host = "127.0.0.1"
port = 3000
workers = 4  # tokio runtime threads

[database]
url = "postgresql://rustmail:password@localhost/rustmail"
max_connections = 20
min_connections = 5

[imap]
host = "127.0.0.1"
port = 993
tls = true
max_connections_per_user = 3
idle_timeout_seconds = 1740  # 29 minutes (RFC recommends < 30)

[smtp]
host = "127.0.0.1"
port = 587
tls = "starttls"

[auth]
jwt_private_key_path = "/etc/rustmail/jwt-private.pem"
jwt_public_key_path = "/etc/rustmail/jwt-public.pem"
access_token_expiry_minutes = 15
refresh_token_expiry_days = 7
argon2_memory_cost = 65536
argon2_time_cost = 3
argon2_parallelism = 4

[rate_limit]
requests_per_minute = 100
login_attempts_per_minute = 10

[logging]
level = "info"  # trace, debug, info, warn, error
format = "json"
```

---

## 8. Testing Requirements

### 8.1 Unit Tests

| Module | Coverage Target | Framework |
|--------|----------------|-----------|
| MIME parsing | > 90% | Rust built-in (`#[cfg(test)]`) |
| Auth (JWT, Argon2) | > 95% | Rust built-in |
| API handlers | > 80% | Axum test utilities |
| React components | > 80% | Vitest + React Testing Library |

### 8.2 Integration Tests

| Test | Description |
|------|-------------|
| IMAP flow | Login → SELECT → FETCH → STORE → LOGOUT against real Dovecot |
| SMTP flow | Build message → send via lettre → verify delivery in Dovecot |
| Auth flow | Register → login → refresh → logout against PostgreSQL |
| Admin flow | Create domain → create user → send email → verify delivery |

### 8.3 End-to-End Tests

| Test | Description | Framework |
|------|-------------|-----------|
| Login flow | Open browser → login → verify inbox loads | Playwright |
| Send flow | Login → compose → send → verify in recipient inbox | Playwright |
| Search flow | Login → search → verify results | Playwright |
| Admin flow | Login as admin → manage domains/users | Playwright |

### 8.4 Security Tests

| Test | Description |
|------|-------------|
| XSS in HTML email | Send crafted HTML email → verify scripts stripped |
| SQL injection | Attempt injection via search/login → verify parameterized queries |
| JWT tampering | Modify JWT payload → verify rejection |
| Rate limiting | Exceed rate limit → verify 429 response |
| CSRF | Attempt cross-origin POST → verify rejection |

---

## 9. Deployment Requirements

### 9.1 System Services

| Service | Type | Description |
|---------|------|-------------|
| `rustmail-backend` | systemd | Axum API server |
| `postfix` | systemd | SMTP MTA |
| `dovecot` | systemd | IMAP/LMTP MDA |
| `postgresql` | systemd | Database |
| `nginx` | systemd | Reverse proxy / TLS |
| `rspamd` | systemd | Spam filter (optional) |

### 9.2 File System Layout

```
/etc/rustmail/
  config.toml              # Backend configuration
  jwt-private.pem          # JWT signing key
  jwt-public.pem           # JWT verification key

/opt/rustmail/
  bin/rustmail             # Backend binary
  frontend/                # React SPA static build
    index.html
    assets/

/var/vmail/                # Dovecot Maildir storage
  example.com/
    user/
      Maildir/

/var/log/rustmail/         # Application logs
  backend.log
```

---

## 10. Traceability Matrix

| PRD Feature | SRS Requirement(s) | Priority |
|-------------|---------------------|----------|
| F1 Login | FR-AUTH-001, FR-AUTH-002 | P0 |
| F2 Folders | FR-FOLDER-001..004 | P0 |
| F3 Message List | FR-MSG-001 | P0 |
| F4 Read Email | FR-MSG-002 | P0 |
| F5 Compose | FR-MSG-003 | P0 |
| F6 Reply/Forward | FR-MSG-004, FR-MSG-005 | P0 |
| F7 Attachments | FR-MSG-003, FR-MSG-009 | P0 |
| F8 Delete/Archive | FR-MSG-007, FR-MSG-008 | P0 |
| F9 Flags | FR-MSG-006 | P0 |
| F10 Search | FR-SEARCH-001 | P0 |
| F11 Real-Time | FR-RT-001 | P0 |
| F12 Domains | FR-ADMIN-001 | P1 |
| F13 Users | FR-ADMIN-002 | P1 |
| F14 Quotas | FR-ADMIN-002 (quota field) | P1 |
| F15 Aliases | FR-ADMIN-003 | P1 |
| F16 Dashboard | FR-ADMIN-004 | P1 |
