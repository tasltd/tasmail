# Development Setup Guide
# RustMail — Self-Hosted Email Service

**Version:** 1.0
**Date:** 2026-03-07

---

## 1. Prerequisites

### 1.1 Required Software

| Software | Version | Purpose |
|----------|---------|---------|
| Rust | 1.78+ | Backend compilation |
| Node.js | 20 LTS | Frontend build |
| PostgreSQL | 16+ | Database |
| Postfix | 3.5+ | SMTP MTA |
| Dovecot | 2.3+ | IMAP MDA |
| Nginx | 1.24+ | Reverse proxy |
| certbot | latest | TLS certificates |

### 1.2 Installation (Ubuntu 22.04/24.04)

```bash
# System updates
sudo apt update && sudo apt upgrade -y

# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
rustup default stable
rustc --version  # Should be 1.78+

# Node.js 20 LTS
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo bash -
sudo apt install -y nodejs
node --version   # Should be 20.x
npm --version    # Should be 10.x

# PostgreSQL 16
sudo apt install -y postgresql-16 postgresql-client-16

# Postfix
sudo apt install -y postfix postfix-pgsql
# During install: select "Internet Site", enter your domain

# Dovecot
sudo apt install -y dovecot-core dovecot-imapd dovecot-lmtpd dovecot-pgsql

# Nginx
sudo apt install -y nginx

# certbot (Let's Encrypt)
sudo apt install -y certbot python3-certbot-nginx

# Rspamd (spam filter — optional for dev, recommended for prod)
sudo apt install -y rspamd redis-server

# OpenDKIM
sudo apt install -y opendkim opendkim-tools

# Development tools
sudo apt install -y build-essential pkg-config libssl-dev
```

---

## 2. Project Structure

```
project-email-service/
├── docs/                       # Documentation (you are here)
│   ├── PRD.md
│   ├── SSR.md
│   ├── ARCHITECTURE.md
│   ├── API-SPECIFICATION.md
│   ├── DEVELOPMENT-SETUP.md    # This file
│   ├── DEPLOYMENT-GUIDE.md
│   └── SECURITY.md
├── backend/                    # Rust Axum backend
│   ├── Cargo.toml
│   ├── Cargo.lock
│   ├── src/
│   │   ├── main.rs
│   │   ├── config.rs
│   │   ├── router.rs
│   │   ├── state.rs
│   │   ├── error.rs
│   │   ├── middleware/
│   │   ├── handlers/
│   │   ├── services/
│   │   ├── models/
│   │   └── imap/
│   ├── migrations/
│   │   └── 001_initial_schema.sql
│   └── tests/
├── frontend/                   # React SPA
│   ├── package.json
│   ├── vite.config.ts
│   ├── tsconfig.json
│   ├── src/
│   │   ├── main.tsx
│   │   ├── App.tsx
│   │   ├── api/
│   │   ├── components/
│   │   ├── hooks/
│   │   ├── stores/
│   │   ├── types/
│   │   └── utils/
│   └── public/
├── config/                     # Configuration templates
│   ├── rustmail.toml.example
│   ├── postfix/
│   │   ├── main.cf.example
│   │   └── master.cf.example
│   ├── dovecot/
│   │   ├── dovecot.conf.example
│   │   ├── 10-auth.conf.example
│   │   ├── 10-mail.conf.example
│   │   └── 10-master.conf.example
│   └── nginx/
│       └── rustmail.conf.example
├── scripts/                    # Utility scripts
│   ├── setup-dev.sh            # Dev environment setup
│   ├── setup-db.sh             # Database initialization
│   ├── generate-dkim.sh        # DKIM key generation
│   └── deploy.sh               # Production deployment
├── e2e/                        # End-to-end tests
│   ├── playwright.config.ts
│   └── specs/
└── CLAUDE.md                   # Claude Code instructions
```

---

## 3. Database Setup

### 3.1 Create Database and User

```bash
# Switch to postgres user
sudo -u postgres psql

# Create database and user
CREATE USER rustmail WITH PASSWORD 'dev_password_change_me';
CREATE DATABASE rustmail OWNER rustmail;
\c rustmail
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";
\q
```

### 3.2 Run Migrations

```bash
# From project root
cd backend/

# Using sqlx-cli
cargo install sqlx-cli --no-default-features --features postgres
export DATABASE_URL="postgresql://rustmail:dev_password_change_me@localhost/rustmail"
sqlx migrate run

# Or manually
psql -U rustmail -d rustmail -f migrations/001_initial_schema.sql
```

### 3.3 Seed Development Data

```sql
-- Insert a test domain
INSERT INTO domains (domain, active) VALUES ('dev.localhost', TRUE);

-- Insert a test user (password: "test123")
-- Argon2id hash for "test123"
INSERT INTO mailboxes (username, password_hash, domain_id, display_name, role)
SELECT 'test@dev.localhost',
       '$argon2id$v=19$m=65536,t=3,p=4$c29tZXNhbHQ$hash_placeholder',
       id, 'Test User', 'super_admin'
FROM domains WHERE domain = 'dev.localhost';
```

---

## 4. Postfix Configuration (Development)

### 4.1 main.cf

```bash
# /etc/postfix/main.cf — Development configuration
sudo tee /etc/postfix/main.cf << 'EOF'
# Basic settings
myhostname = mail.dev.localhost
mydomain = dev.localhost
myorigin = $mydomain
mydestination = localhost

# Virtual domains (from PostgreSQL)
virtual_mailbox_domains = pgsql:/etc/postfix/pgsql-virtual-domains.cf
virtual_mailbox_maps = pgsql:/etc/postfix/pgsql-virtual-mailbox.cf
virtual_alias_maps = pgsql:/etc/postfix/pgsql-virtual-alias.cf

# Deliver to Dovecot via LMTP
virtual_transport = lmtp:unix:private/dovecot-lmtp

# SASL authentication via Dovecot
smtpd_sasl_type = dovecot
smtpd_sasl_path = private/auth
smtpd_sasl_auth_enable = yes

# Relay restrictions
smtpd_relay_restrictions = permit_mynetworks, permit_sasl_authenticated, reject_unauth_destination

# TLS (skip for local dev, enable for prod)
# smtpd_tls_security_level = may

# Submission port (587)
# Configured in master.cf
EOF
```

### 4.2 PostgreSQL Maps

```bash
# /etc/postfix/pgsql-virtual-domains.cf
sudo tee /etc/postfix/pgsql-virtual-domains.cf << 'EOF'
hosts = localhost
dbname = rustmail
user = rustmail
password = dev_password_change_me
query = SELECT domain FROM domains WHERE domain = '%s' AND active = TRUE
EOF

# /etc/postfix/pgsql-virtual-mailbox.cf
sudo tee /etc/postfix/pgsql-virtual-mailbox.cf << 'EOF'
hosts = localhost
dbname = rustmail
user = rustmail
password = dev_password_change_me
query = SELECT CONCAT(SPLIT_PART(username, '@', 2), '/', SPLIT_PART(username, '@', 1), '/Maildir/') FROM mailboxes WHERE username = '%s' AND active = TRUE
EOF

# /etc/postfix/pgsql-virtual-alias.cf
sudo tee /etc/postfix/pgsql-virtual-alias.cf << 'EOF'
hosts = localhost
dbname = rustmail
user = rustmail
password = dev_password_change_me
query = SELECT destination FROM aliases WHERE source = '%s' AND active = TRUE
EOF
```

### 4.3 master.cf (Submission Port)

```bash
# Add to /etc/postfix/master.cf
submission inet n - n - - smtpd
  -o smtpd_sasl_auth_enable=yes
  -o smtpd_sasl_type=dovecot
  -o smtpd_sasl_path=private/auth
  -o smtpd_client_restrictions=permit_sasl_authenticated,reject
```

---

## 5. Dovecot Configuration (Development)

### 5.1 Main Config

```bash
# /etc/dovecot/dovecot.conf
sudo tee /etc/dovecot/dovecot.conf << 'EOF'
protocols = imap lmtp
listen = *, ::
mail_home = /var/vmail/%d/%n
mail_location = maildir:/var/vmail/%d/%n/Maildir

# Auth
auth_mechanisms = plain login
!include conf.d/10-auth.conf
!include conf.d/10-mail.conf
!include conf.d/10-master.conf
!include conf.d/10-ssl.conf
EOF
```

### 5.2 Auth Config

```bash
# /etc/dovecot/conf.d/10-auth.conf
sudo tee /etc/dovecot/conf.d/10-auth.conf << 'EOF'
auth_mechanisms = plain login
!include auth-sql.conf.ext
EOF

# /etc/dovecot/conf.d/auth-sql.conf.ext
sudo tee /etc/dovecot/conf.d/auth-sql.conf.ext << 'EOF'
passdb {
  driver = sql
  args = /etc/dovecot/dovecot-sql.conf.ext
}
userdb {
  driver = sql
  args = /etc/dovecot/dovecot-sql.conf.ext
}
EOF

# /etc/dovecot/dovecot-sql.conf.ext
sudo tee /etc/dovecot/dovecot-sql.conf.ext << 'EOF'
driver = pgsql
connect = host=127.0.0.1 dbname=rustmail user=rustmail password=dev_password_change_me

default_pass_scheme = SHA512-CRYPT

password_query = SELECT username, password_hash AS password \
  FROM mailboxes WHERE username = '%u' AND active = TRUE

user_query = SELECT \
  '/var/vmail/%d/%n' AS home, \
  'maildir:/var/vmail/%d/%n/Maildir' AS mail, \
  5000 AS uid, 5000 AS gid, \
  CONCAT('*:bytes=', quota) AS quota_rule \
  FROM mailboxes WHERE username = '%u'
EOF
```

### 5.3 Master Config (Sockets)

```bash
# /etc/dovecot/conf.d/10-master.conf
sudo tee /etc/dovecot/conf.d/10-master.conf << 'EOF'
service lmtp {
  unix_listener /var/spool/postfix/private/dovecot-lmtp {
    group = postfix
    mode = 0660
    user = postfix
  }
}

service auth {
  unix_listener /var/spool/postfix/private/auth {
    group = postfix
    mode = 0660
    user = postfix
  }
  unix_listener auth-userdb {
    mode = 0600
    user = vmail
  }
}

service imap-login {
  inet_listener imap {
    port = 143
  }
  inet_listener imaps {
    port = 993
    ssl = yes
  }
}
EOF
```

### 5.4 Create vmail User

```bash
# Create vmail system user for Maildir ownership
sudo groupadd -g 5000 vmail
sudo useradd -g vmail -u 5000 vmail -d /var/vmail -s /sbin/nologin
sudo mkdir -p /var/vmail
sudo chown -R vmail:vmail /var/vmail
sudo chmod -R 770 /var/vmail
```

---

## 6. Backend Development

### 6.1 Setup

```bash
cd backend/

# Create Cargo.toml
cat > Cargo.toml << 'EOF'
[package]
name = "rustmail"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = { version = "0.7", features = ["ws", "macros", "multipart"] }
tokio = { version = "1", features = ["full"] }
tower = { version = "0.4", features = ["full"] }
tower-http = { version = "0.5", features = ["cors", "trace", "compression-gzip"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
lettre = { version = "0.11", default-features = false, features = [
    "builder", "hostname", "smtp-transport",
    "tokio1", "tokio1-rustls", "rustls-tls",
]}
async-imap = { version = "0.11", features = ["runtime-tokio"] }
async-native-tls = "0.5"
futures = "0.3"
mailparse = "0.16"
mail-auth = "0.7"
jsonwebtoken = "9"
argon2 = "0.5"
uuid = { version = "1", features = ["v4", "serde"] }
sqlx = { version = "0.8", features = [
    "postgres", "runtime-tokio-rustls", "uuid", "time", "macros"
]}
time = { version = "0.3", features = ["serde"] }
toml = "0.8"
thiserror = "1"
EOF

# Build
cargo build
```

### 6.2 Development Server

```bash
# Set environment
export DATABASE_URL="postgresql://rustmail:dev_password_change_me@localhost/rustmail"
export RUST_LOG=debug

# Run with auto-reload (install cargo-watch)
cargo install cargo-watch
cargo watch -x run
```

### 6.3 Testing

```bash
# Unit tests
cargo test

# With logging
RUST_LOG=debug cargo test -- --nocapture

# Specific test
cargo test test_auth_login

# Code coverage (install tarpaulin)
cargo install cargo-tarpaulin
cargo tarpaulin --out Html
```

---

## 7. Frontend Development

### 7.1 Setup

```bash
cd frontend/

# Create Vite + React + TypeScript project
npm create vite@latest . -- --template react-ts

# Install dependencies
npm install @tanstack/react-query zustand @tiptap/react @tiptap/starter-kit \
            @tiptap/extension-link @tiptap/extension-image @tiptap/extension-placeholder \
            dompurify @tanstack/react-virtual date-fns react-router-dom \
            @types/dompurify

# Development dependencies
npm install -D @types/react @types/react-dom vitest @testing-library/react \
              @testing-library/jest-dom @testing-library/user-event \
              eslint prettier tailwindcss postcss autoprefixer
```

### 7.2 Vite Configuration

```typescript
// vite.config.ts
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      '/api': {
        target: 'http://localhost:3000',
        changeOrigin: true,
      },
      '/ws': {
        target: 'ws://localhost:3000',
        ws: true,
      },
    },
  },
});
```

### 7.3 Development Server

```bash
npm run dev
# Opens at http://localhost:5173
# API calls proxied to Axum at http://localhost:3000
```

### 7.4 Testing

```bash
npm run test              # Vitest
npm run test -- --coverage # With coverage
npm run lint              # ESLint
npm run format            # Prettier
```

---

## 8. Running the Full Stack (Development)

```bash
# Terminal 1: Start PostgreSQL (usually auto-started)
sudo systemctl start postgresql

# Terminal 2: Start Postfix
sudo systemctl start postfix

# Terminal 3: Start Dovecot
sudo systemctl start dovecot

# Terminal 4: Start Rust backend
cd backend/ && cargo watch -x run

# Terminal 5: Start React frontend
cd frontend/ && npm run dev

# Access at: http://localhost:5173
```

### 8.1 Quick Health Check

```bash
# Check PostgreSQL
psql -U rustmail -d rustmail -c "SELECT 1;"

# Check Postfix
sudo postfix status

# Check Dovecot
sudo doveadm service status

# Check API
curl http://localhost:3000/api/health

# Check frontend
curl http://localhost:5173
```

---

## 9. Common Development Tasks

### 9.1 Send Test Email via CLI

```bash
# Using swaks (Swiss Army Knife for SMTP)
sudo apt install swaks

# Send test email through Postfix
swaks --to test@dev.localhost \
      --from sender@external.com \
      --server localhost \
      --port 25 \
      --header "Subject: Test Email" \
      --body "This is a test email"

# Send via authenticated submission (port 587)
swaks --to recipient@external.com \
      --from test@dev.localhost \
      --server localhost \
      --port 587 \
      --auth-user test@dev.localhost \
      --auth-password test123 \
      --tls
```

### 9.2 Check Maildir

```bash
# List delivered mail
ls -la /var/vmail/dev.localhost/test/Maildir/new/

# Read a message
cat /var/vmail/dev.localhost/test/Maildir/new/*.msg
```

### 9.3 IMAP Testing via CLI

```bash
# Using openssl s_client (or nc for unencrypted)
openssl s_client -connect localhost:993

# Then type IMAP commands:
a1 LOGIN test@dev.localhost test123
a2 SELECT INBOX
a3 FETCH 1:* (FLAGS ENVELOPE)
a4 LOGOUT
```

---

## 10. Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | — | PostgreSQL connection string |
| `RUST_LOG` | `info` | Log level (trace/debug/info/warn/error) |
| `RUSTMAIL_CONFIG` | `config.toml` | Path to config file |
| `JWT_PRIVATE_KEY` | — | Path to RS256 private key |
| `JWT_PUBLIC_KEY` | — | Path to RS256 public key |
| `VITE_API_URL` | `/api` | Frontend API base URL |

---

## 11. Generating JWT Keys

```bash
# Generate RSA-2048 key pair for JWT signing
openssl genpkey -algorithm RSA -out jwt-private.pem -pkeyopt rsa_keygen_bits:2048
openssl rsa -pubout -in jwt-private.pem -out jwt-public.pem

# Place in config directory
sudo mkdir -p /etc/rustmail
sudo cp jwt-private.pem jwt-public.pem /etc/rustmail/
sudo chmod 600 /etc/rustmail/jwt-private.pem
sudo chmod 644 /etc/rustmail/jwt-public.pem
```
