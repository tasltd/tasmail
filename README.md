# TASMail — Self-Hosted Email Service

A modern, self-hosted email service with a React frontend, Rust (Axum) backend, and Postfix/Dovecot mail infrastructure. Privacy-first, cost-effective alternative to Google Workspace and Microsoft 365.

## Why TASMail?

- **Modern Webmail UI** — React 19 SPA with real-time push notifications, rich text composer, and responsive design
- **High Performance** — Rust backend uses < 100 MB RAM; sub-200ms API responses
- **Full Privacy** — Your server, your data. No third-party tracking or data mining
- **Cost Effective** — Runs on a $10-20/month VPS for unlimited users vs $6-22/user/month for Google Workspace
- **Web Access Anywhere** — Replace desktop email clients with browser-based access from any device
- **Battle-Tested Infrastructure** — Built on Postfix (SMTP) and Dovecot (IMAP), powering millions of mail servers worldwide

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
