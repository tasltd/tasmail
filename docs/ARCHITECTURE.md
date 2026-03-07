# Architecture Document
# RustMail — Self-Hosted Email Service

**Version:** 1.0
**Date:** 2026-03-07

---

## 1. Architecture Overview

RustMail follows a **layered proxy architecture** where the Rust backend acts as an intelligent middleware between the React frontend and the Linux mail infrastructure (Postfix + Dovecot). The backend never stores email data — it proxies all mail operations through IMAP/SMTP protocols to Dovecot and Postfix respectively.

```
┌─────────────────────────────────────────────────────────────────┐
│                        PRESENTATION LAYER                        │
│  React 19 SPA (Vite + TypeScript)                               │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────────┐  │
│  │FolderTree│ │MsgList   │ │MsgView   │ │Composer (TipTap)  │  │
│  └──────────┘ └──────────┘ └──────────┘ └──────────────────┘  │
│  State: Zustand | Data: TanStack Query | Transport: fetch+WS   │
└────────────────────────────┬────────────────────────────────────┘
                             │ HTTPS (REST + WebSocket)
┌────────────────────────────▼────────────────────────────────────┐
│                        API GATEWAY LAYER                         │
│  Nginx — TLS termination, static file serving, reverse proxy    │
└────────────────────────────┬────────────────────────────────────┘
                             │ HTTP (localhost:3000)
┌────────────────────────────▼────────────────────────────────────┐
│                        APPLICATION LAYER                         │
│  Axum (Rust) — REST handlers, WebSocket hub, business logic     │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ Tower Middleware Stack:                                    │  │
│  │ [CORS] → [RateLimit] → [Auth/JWT] → [Logging] → [Gzip]  │  │
│  └──────────────────────────────────────────────────────────┘  │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────────┐  │
│  │Auth Svc  │ │Mail Svc  │ │Admin Svc │ │WebSocket Hub     │  │
│  │(JWT/Argon│ │(IMAP/SMTP│ │(Domain/  │ │(IDLE→Push)       │  │
│  │  2id)    │ │  proxy)  │ │User mgmt)│ │                  │  │
│  └──────────┘ └──────────┘ └──────────┘ └──────────────────┘  │
└──────┬──────────────┬──────────────┬───────────────────────────┘
       │              │              │
┌──────▼──────┐ ┌─────▼─────┐ ┌─────▼─────┐
│ PostgreSQL   │ │  Dovecot   │ │  Postfix   │
│ (accounts,   │ │  (IMAP,    │ │  (SMTP,    │
│  sessions,   │ │   LMTP,    │ │   relay,   │
│  settings)   │ │   SASL,    │ │   DKIM)    │
│              │ │   Sieve,   │ │            │
│              │ │   FTS)     │ │            │
└──────────────┘ └─────┬─────┘ └────────────┘
                       │
                 ┌─────▼─────┐
                 │  Maildir   │
                 │ /var/vmail │
                 └───────────┘
```

---

## 2. Component Architecture

### 2.1 React Frontend Architecture

```
src/
├── main.tsx                    # Entry point, router setup
├── App.tsx                     # Root component, auth provider
├── api/
│   ├── client.ts               # Axios/fetch wrapper with JWT interceptor
│   ├── auth.ts                 # Login, refresh, logout
│   ├── folders.ts              # Folder CRUD
│   ├── messages.ts             # Message CRUD + search
│   └── admin.ts                # Admin API calls
├── components/
│   ├── layout/
│   │   ├── AppShell.tsx        # Main layout: sidebar + content
│   │   ├── Sidebar.tsx         # Folder tree + compose button
│   │   └── TopBar.tsx          # Search + user menu
│   ├── mail/
│   │   ├── FolderTree.tsx      # Recursive folder list
│   │   ├── MessageList.tsx     # Virtualized message list
│   │   ├── MessageRow.tsx      # Single row in list
│   │   ├── MessageView.tsx     # Full message display
│   │   ├── HtmlRenderer.tsx    # Sanitized HTML email body
│   │   ├── AttachmentList.tsx  # Attachment download links
│   │   └── Composer.tsx        # TipTap rich text editor
│   ├── admin/
│   │   ├── DomainManager.tsx
│   │   ├── UserManager.tsx
│   │   ├── AliasManager.tsx
│   │   └── Dashboard.tsx
│   └── shared/
│       ├── LoadingSkeleton.tsx
│       ├── ErrorBoundary.tsx
│       └── ConfirmDialog.tsx
├── hooks/
│   ├── useAuth.ts              # Auth context + token management
│   ├── useMailbox.ts           # TanStack Query for mail data
│   ├── useNotifications.ts     # WebSocket connection + events
│   └── useKeyboardShortcuts.ts # Gmail-like shortcuts
├── stores/
│   ├── mailStore.ts            # Zustand: selected folder, uid, view mode
│   └── uiStore.ts             # Zustand: theme, sidebar state
├── types/
│   ├── mail.ts                 # Message, Folder, Attachment types
│   ├── auth.ts                 # User, Session types
│   └── admin.ts                # Domain, Alias types
└── utils/
    ├── sanitize.ts             # DOMPurify wrapper
    ├── date.ts                 # date-fns formatters
    └── constants.ts            # API URLs, default config
```

### 2.2 Rust Backend Architecture

```
src/
├── main.rs                     # Entry: config load, router build, server start
├── config.rs                   # TOML config structs (serde)
├── router.rs                   # Axum router definition
├── state.rs                    # AppState: DB pool, IMAP pool, config
├── error.rs                    # AppError enum, IntoResponse impl
├── middleware/
│   ├── mod.rs
│   ├── auth.rs                 # JWT extraction + validation
│   ├── rate_limit.rs           # Tower rate limiting
│   └── logging.rs              # Request/response tracing
├── handlers/
│   ├── mod.rs
│   ├── auth.rs                 # Login, refresh, logout handlers
│   ├── folders.rs              # Folder CRUD handlers
│   ├── messages.rs             # Message CRUD + search handlers
│   ├── attachments.rs          # Attachment download handler
│   ├── websocket.rs            # WebSocket upgrade + IMAP IDLE bridge
│   └── admin/
│       ├── mod.rs
│       ├── domains.rs          # Domain management handlers
│       ├── users.rs            # User management handlers
│       ├── aliases.rs          # Alias management handlers
│       └── dashboard.rs        # System stats handler
├── services/
│   ├── mod.rs
│   ├── auth_service.rs         # JWT create/verify, Argon2 hash/verify
│   ├── imap_service.rs         # IMAP connection pool + operations
│   ├── smtp_service.rs         # lettre SMTP sending
│   ├── mime_service.rs         # MIME parsing (mailparse)
│   └── admin_service.rs        # Domain/user/alias CRUD
├── models/
│   ├── mod.rs
│   ├── domain.rs               # Domain struct + sqlx queries
│   ├── mailbox.rs              # Mailbox struct + sqlx queries
│   ├── alias.rs                # Alias struct + sqlx queries
│   ├── session.rs              # Session struct + sqlx queries
│   └── setting.rs              # Setting struct + sqlx queries
├── imap/
│   ├── mod.rs
│   ├── pool.rs                 # Per-user IMAP connection pool
│   ├── idle.rs                 # IMAP IDLE session manager
│   └── parser.rs               # IMAP response → domain types
└── migrations/
    ├── 001_initial_schema.sql
    ├── 002_add_dkim_keys.sql
    └── 003_add_settings.sql
```

### 2.3 Connection Flow Diagrams

#### 2.3.1 Inbound Email (Internet → User Mailbox)

```
Remote MTA                Postfix              Rspamd          Dovecot          Maildir
   │                        │                    │               │                │
   │──SMTP EHLO/MAIL/RCPT─→│                    │               │                │
   │                        │──milter check─────→│               │                │
   │                        │                    │──score/reject─│                │
   │                        │←─accept/reject─────│               │                │
   │                        │                    │               │                │
   │                        │──LMTP delivery────────────────────→│                │
   │                        │  (unix socket)     │               │──Sieve filter─→│
   │                        │                    │               │   write Maildir │
   │                        │                    │               │←───────────────│
   │←──250 OK───────────────│                    │               │                │
```

#### 2.3.2 Outbound Email (User → Internet)

```
React SPA        Axum Backend       Postfix          OpenDKIM        Remote MTA
   │                 │                 │                 │               │
   │──POST /api/     │                 │                 │               │
   │  messages──────→│                 │                 │               │
   │                 │──lettre SMTP───→│                 │               │
   │                 │  (port 587,     │──milter sign───→│               │
   │                 │   SASL auth)    │                 │──DKIM header─→│
   │                 │                 │←────────────────│               │
   │                 │                 │──SMTP relay────────────────────→│
   │                 │──IMAP APPEND──→ │                 │               │
   │                 │  (Sent folder)  │                 │               │
   │←──201 Created───│                 │                 │               │
```

#### 2.3.3 WebSocket Real-Time Push

```
React SPA        Axum Backend         Dovecot
   │                 │                   │
   │──WS connect────→│                   │
   │  (JWT auth)     │──IMAP LOGIN──────→│
   │                 │──IMAP SELECT─────→│
   │                 │──IMAP IDLE───────→│
   │                 │                   │  (waiting for new mail...)
   │                 │                   │
   │                 │  ← * EXISTS 343 ──│  (new message delivered)
   │                 │──DONE────────────→│
   │                 │──FETCH envelope──→│
   │                 │←─envelope data────│
   │←─WS: new_mail──│                   │
   │                 │──IMAP IDLE───────→│  (restart IDLE)
```

---

## 3. IMAP Connection Pool Design

The backend maintains a pool of IMAP connections to Dovecot, managed per-user:

```
┌─────────────────────────────────────────────────┐
│              IMAP Connection Pool                 │
│                                                   │
│  user@example.com:                                │
│    ├── Connection 1: INBOX (IDLE — push)         │
│    ├── Connection 2: Available (operations)       │
│    └── Connection 3: Available (operations)       │
│                                                   │
│  admin@example.com:                               │
│    ├── Connection 1: INBOX (IDLE — push)         │
│    └── Connection 2: Available (operations)       │
│                                                   │
│  Max per user: 3                                  │
│  Total max: 200                                   │
│  Idle timeout: 5 minutes (non-IDLE connections)   │
│  IDLE refresh: 29 minutes (per RFC recommendation)│
└─────────────────────────────────────────────────┘
```

**Key Design Decisions:**
1. One dedicated IDLE connection per active user (for push notifications)
2. 1-2 additional connections for on-demand operations (fetch, search, flag changes)
3. Connections are lazily created on first request
4. Idle non-IDLE connections are closed after 5 minutes
5. IDLE connections are refreshed every 29 minutes (RFC 2177 recommends < 30 min)

---

## 4. Authentication Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                     JWT Authentication Flow                    │
│                                                                │
│  Login:                                                        │
│  Client ──POST /api/auth/login──→ Axum                        │
│         { username, password }     │                           │
│                                    │──query mailboxes──→ PG   │
│                                    │←─password_hash────        │
│                                    │──argon2id::verify()       │
│                                    │──generate JWT (RS256)     │
│                                    │──store session in PG      │
│         ←── { access_token } ──────│                           │
│         ←── Set-Cookie: refresh ───│                           │
│                                                                │
│  Authenticated Request:                                        │
│  Client ──GET /api/folders──→ AuthMiddleware                  │
│  Header: Authorization: Bearer <jwt>                          │
│                                  │──verify RS256 signature     │
│                                  │──check expiry               │
│                                  │──extract user_id, role      │
│                                  │──inject Claims into request │
│                              ──→ Handler (has Claims)          │
│                                                                │
│  Token Refresh:                                                │
│  Client ──POST /api/auth/refresh──→ Axum                      │
│  Cookie: refresh=<token>            │                          │
│                                     │──hash token              │
│                                     │──lookup in sessions      │
│                                     │──verify not expired      │
│                                     │──issue new access+refresh│
│                                     │──rotate refresh in PG    │
│         ←── { access_token } ───────│                          │
│         ←── Set-Cookie: refresh ────│                          │
└──────────────────────────────────────────────────────────────┘
```

---

## 5. Security Architecture

### 5.1 Defense-in-Depth Layers

| Layer | Control | Technology |
|-------|---------|------------|
| Network | Firewall rules, fail2ban | iptables/nftables, fail2ban |
| Transport | TLS 1.2+ everywhere | Let's Encrypt, Nginx, Postfix TLS, Dovecot SSL |
| Application | JWT auth, rate limiting, input validation | Axum middleware, Tower |
| Data | Argon2id passwords, parameterized queries | Rust argon2 crate, sqlx |
| Email | SPF, DKIM, DMARC, DANE | Postfix + OpenDKIM, DNS records |
| Client | DOMPurify, CSP headers, SameSite cookies | React, Nginx headers |

### 5.2 Content Security Policy

```
Content-Security-Policy:
  default-src 'self';
  script-src 'self';
  style-src 'self' 'unsafe-inline';
  img-src 'self' data: https:;
  connect-src 'self' wss:;
  font-src 'self';
  frame-src 'none';
  object-src 'none';
  base-uri 'self';
```

---

## 6. Technology Stack Summary

| Layer | Technology | Version | Purpose |
|-------|-----------|---------|---------|
| Frontend Framework | React | 19.x | UI rendering |
| Frontend Build | Vite | 6.x | Dev server + production build |
| Frontend Language | TypeScript | 5.x | Type safety |
| Server State | TanStack Query | 5.x | Async data fetching + cache |
| Client State | Zustand | 5.x | UI state management |
| Rich Text | TipTap | 2.x | Email composer |
| HTML Sanitize | DOMPurify | 3.x | XSS prevention |
| Virtual List | @tanstack/virtual | 3.x | Performance for large mailboxes |
| Backend Framework | Axum | 0.7+ | HTTP + WebSocket server |
| Async Runtime | Tokio | 1.x | Async I/O |
| SMTP Client | lettre | 0.11.x | Email sending |
| IMAP Client | async-imap | 0.11.x | Email reading |
| MIME Parser | mailparse | 0.16.x | Email body parsing |
| DKIM/SPF | mail-auth | 0.7.x | Email authentication |
| Database Driver | sqlx | 0.8.x | PostgreSQL async queries |
| Auth (JWT) | jsonwebtoken | 9.x | Token creation/verification |
| Auth (Password) | argon2 | 0.5.x | Password hashing |
| Serialization | serde + serde_json | 1.x | JSON handling |
| Logging | tracing + tracing-subscriber | 0.1/0.3 | Structured logging |
| HTTP Middleware | tower + tower-http | 0.4/0.5 | CORS, compression, tracing |
| Database | PostgreSQL | 16+ | User/session/config storage |
| MTA | Postfix | 3.8+ | SMTP mail transfer |
| MDA | Dovecot | 2.3+ | IMAP access + LMTP delivery |
| Spam Filter | Rspamd | 3.x | Milter-based filtering |
| Reverse Proxy | Nginx | 1.24+ | TLS + static serving |
| TLS Certs | Let's Encrypt / certbot | latest | Automated TLS provisioning |

---

## 7. Deployment Architecture

### 7.1 Single-Server Deployment (Recommended for v1)

```
┌──────────────────── VPS (Ubuntu 24.04) ────────────────────┐
│                                                              │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌────────────┐ │
│  │ Nginx    │  │ Axum     │  │ Postfix  │  │ Dovecot    │ │
│  │ :443/:80 │──│ :3000    │  │ :25/:587 │──│ :993       │ │
│  └──────────┘  └──────────┘  └──────────┘  └────────────┘ │
│                      │            │              │          │
│                ┌─────▼──────┐    │              │          │
│                │ PostgreSQL  │    └──────────────┘          │
│                │ :5432       │     LMTP + SASL (sockets)   │
│                └─────────────┘                              │
│                                                              │
│  ┌──────────┐  ┌──────────┐                                │
│  │ Rspamd   │  │ Redis    │                                │
│  │ :11332   │  │ :6379    │                                │
│  └──────────┘  └──────────┘                                │
│                                                              │
│  Storage: /var/vmail/ (Maildir)                             │
│  Config:  /etc/rustmail/config.toml                         │
│  Logs:    /var/log/rustmail/                                │
│  Binary:  /opt/rustmail/bin/rustmail                        │
│  Frontend:/opt/rustmail/frontend/                           │
└──────────────────────────────────────────────────────────────┘
```

### 7.2 Nginx Configuration

```nginx
# /etc/nginx/sites-enabled/rustmail.conf

# Redirect HTTP → HTTPS
server {
    listen 80;
    server_name mail.example.com;
    return 301 https://$host$request_uri;
}

server {
    listen 443 ssl http2;
    server_name mail.example.com;

    ssl_certificate     /etc/letsencrypt/live/mail.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/mail.example.com/privkey.pem;
    ssl_protocols       TLSv1.2 TLSv1.3;

    # Security headers
    add_header X-Frame-Options DENY;
    add_header X-Content-Type-Options nosniff;
    add_header X-XSS-Protection "1; mode=block";
    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;
    add_header Referrer-Policy "strict-origin-when-cross-origin";

    # React SPA — static files
    root /opt/rustmail/frontend;
    index index.html;

    # SPA fallback
    location / {
        try_files $uri $uri/ /index.html;
    }

    # API proxy → Axum
    location /api/ {
        proxy_pass http://127.0.0.1:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    # WebSocket proxy → Axum
    location /ws/ {
        proxy_pass http://127.0.0.1:3000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "Upgrade";
        proxy_set_header Host $host;
        proxy_read_timeout 3600s;
    }

    # Attachment download — increase limits
    location /api/messages/ {
        proxy_pass http://127.0.0.1:3000;
        client_max_body_size 25m;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

### 7.3 Systemd Service

```ini
# /etc/systemd/system/rustmail.service
[Unit]
Description=RustMail Email Service Backend
After=postgresql.service dovecot.service postfix.service
Wants=postgresql.service dovecot.service postfix.service

[Service]
Type=simple
User=rustmail
Group=rustmail
ExecStart=/opt/rustmail/bin/rustmail --config /etc/rustmail/config.toml
Restart=always
RestartSec=5
Environment=RUST_LOG=info
StandardOutput=journal
StandardError=journal

# Hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/log/rustmail
PrivateTmp=true

[Install]
WantedBy=multi-user.target
```

---

## 8. Development Workflow

### 8.1 Prerequisites

```bash
# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default stable  # 1.78+

# Node.js (frontend)
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo bash -
sudo apt install nodejs

# PostgreSQL
sudo apt install postgresql-16

# Dovecot + Postfix (for integration testing)
sudo apt install dovecot-core dovecot-imapd dovecot-lmtpd postfix
```

### 8.2 Development Commands

```bash
# Backend
cd backend/
cargo build                      # Compile
cargo run                        # Run dev server
cargo test                       # Unit tests
cargo clippy                     # Lint
cargo fmt                        # Format

# Frontend
cd frontend/
npm install                      # Install deps
npm run dev                      # Vite dev server (port 5173)
npm run build                    # Production build
npm run test                     # Vitest
npm run lint                     # ESLint + Prettier

# Integration tests
cd e2e/
npx playwright test              # E2E tests
```

---

## 9. Monitoring and Observability

### 9.1 Logging Strategy

| Component | Log Format | Destination |
|-----------|-----------|-------------|
| Axum Backend | JSON (tracing) | journald / /var/log/rustmail/ |
| Postfix | syslog | /var/log/mail.log |
| Dovecot | syslog | /var/log/mail.log |
| Nginx | combined | /var/log/nginx/access.log |

### 9.2 Health Checks

| Endpoint | Check |
|----------|-------|
| `GET /api/health` | Backend alive + DB connected |
| `GET /api/health/imap` | IMAP connection to Dovecot works |
| `GET /api/health/smtp` | SMTP connection to Postfix works |

### 9.3 Key Metrics to Monitor

- API response times (p50, p95, p99)
- Active WebSocket connections
- IMAP connection pool utilization
- PostgreSQL connection pool utilization
- Postfix mail queue size
- Dovecot storage usage per domain
- Failed login attempts per IP
- Email send/receive rate
