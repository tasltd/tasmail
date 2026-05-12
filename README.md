# TASMail — Webmail UI for any IMAP/SMTP server

TASMail is a fast, modern webmail client (BYOK) that connects to **any** IMAP/SMTP
server you already use — Gmail, Outlook, Zoho, FastMail, your corporate Exchange,
ProtonMail Bridge, an existing Dovecot. We never store your email; only the
encrypted credentials needed to fetch it.

Live at **<https://mail.techatscale.io>** (future home: `tasmail.com`).

## Why TASMail?

- **One UI, every account** — bring your own IMAP/SMTP and get a single polished webmail across desktop and mobile
- **Modern stack** — React 19 SPA with real-time push, rich text composer, PWA + native iOS/Android apps
- **Privacy by design** — your provider, your storage. We hold encrypted credentials, never the message body
- **High performance** — Rust/Axum backend uses < 100 MB RAM; sub-200 ms API responses
- **Optional self-host** — operators can run their own Postfix/Dovecot alongside TASMail (see `docs/SELF-HOST-MAIL-SERVERS.md`); not required for the BYOK product

## Pricing

| Plan | Price | What you get |
|------|-------|--------------|
| **TASMail BYOK** | **GHS 1.00 / GB · month** (≈ $0.07 USD), GHS 5 monthly minimum | Connect your own IMAP/SMTP server, unlimited devices, encrypted credentials at rest, email + chat support |
| **Enterprise** | Custom quote | Single-tenant deployment on your cloud or ours, SAML/OIDC SSO, on-premise option, white-glove onboarding + SLA, compliance reporting |

All invoices settled in Ghana cedis via Paystack, Mastercard MPGS, Cybersource invoicing, or bank transfer (the same providers PayPro uses). Visitors outside Ghana see an indicative USD line next to every price. Live calculator + FAQ at <https://mail.techatscale.io/pricing>.

## Architecture

```
React SPA (Vite + TypeScript)
     |
     | REST + WebSocket (HTTPS)
     v
Axum Backend (Rust)
     |              |
async-imap       lettre
     |              |
Dovecot IMAPS   Postfix SMTP
   :993            :587
     |
  Maildir Storage
```

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Frontend | React 19, Vite, TypeScript, TanStack Query, Zustand, TipTap |
| Backend | Rust, Axum 0.7+, Tokio, sqlx, async-imap, lettre |
| Database | PostgreSQL 16+ |
| Mail Transfer | Postfix 3.8+ |
| Mail Delivery | Dovecot 2.3+ |
| Spam Filter | Rspamd 3.x |
| Reverse Proxy | Nginx |
| TLS | Let's Encrypt (certbot) |

## Documentation

| Document | Description |
|----------|-------------|
| [PRD](docs/PRD.md) | Product Requirements Document |
| [SRS](docs/SSR.md) | System Requirements Specification |
| [Architecture](docs/ARCHITECTURE.md) | System architecture and component design |
| [API Spec](docs/API-SPECIFICATION.md) | REST and WebSocket API reference |
| [Dev Setup](docs/DEVELOPMENT-SETUP.md) | Development environment setup guide |
| [Deployment](docs/DEPLOYMENT-GUIDE.md) | Production deployment guide |
| [Security](docs/SECURITY.md) | Security architecture and hardening |
| [PM Plan](docs/PROJECT-MANAGEMENT-PLAN.md) | PMBOK 7 project management plan |
| [Business Validation](docs/BUSINESS-VALIDATION-GHANA.md) | Ghana market business validation |

## Quick Start

```bash
# Backend
cd backend/
cargo build && cargo run

# Frontend
cd frontend/
npm install && npm run dev
```

See [Development Setup](docs/DEVELOPMENT-SETUP.md) for full instructions.

## License

MIT
