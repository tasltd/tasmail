# Security Documentation
# RustMail — Self-Hosted Email Service

**Version:** 1.0
**Date:** 2026-03-07

---

## 1. Security Architecture Overview

RustMail implements defense-in-depth across six layers:

```
Layer 1: Network     — Firewall (UFW/nftables), Fail2ban, rate limiting
Layer 2: Transport   — TLS 1.2+ everywhere (HTTPS, IMAPS, SMTPS, STARTTLS)
Layer 3: Application — JWT auth, CORS, CSP, input validation, rate limiting
Layer 4: Data        — Argon2id password hashing, parameterized SQL queries
Layer 5: Email Auth  — SPF, DKIM (2048-bit RSA), DMARC, DANE
Layer 6: Client      — DOMPurify HTML sanitization, CSP headers, SameSite cookies
```

---

## 2. Authentication Security

### 2.1 Password Hashing — Argon2id

All passwords are hashed with Argon2id (winner of the Password Hashing Competition):

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| Memory Cost | 64 MB (65536 KiB) | Resists GPU attacks |
| Time Cost | 3 iterations | Balances security/performance |
| Parallelism | 4 threads | Matches typical VPS CPU |
| Salt | 16 random bytes | Unique per hash |
| Output | 32 bytes | Sufficient for key derivation |

```rust
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use argon2::password_hash::{SaltString, rand_core::OsRng};

// Hash password
let salt = SaltString::generate(&mut OsRng);
let argon2 = Argon2::new(
    argon2::Algorithm::Argon2id,
    argon2::Version::V0x13,
    argon2::Params::new(65536, 3, 4, Some(32)).unwrap(),
);
let hash = argon2.hash_password(password.as_bytes(), &salt).unwrap().to_string();

// Verify password
let parsed_hash = PasswordHash::new(&stored_hash).unwrap();
argon2.verify_password(password.as_bytes(), &parsed_hash).is_ok()
```

### 2.2 JWT Token Security

| Property | Value |
|----------|-------|
| Algorithm | RS256 (RSA-SHA256) |
| Key Size | 2048-bit RSA |
| Access Token Expiry | 15 minutes |
| Refresh Token Expiry | 7 days |
| Refresh Token Storage | SHA-256 hash in PostgreSQL sessions table |
| Refresh Token Delivery | HttpOnly, Secure, SameSite=Strict cookie |
| Token Rotation | Refresh tokens are rotated on every use |
| Revocation | Logout deletes session; token refresh validates against DB |

**JWT Claims:**
```json
{
  "sub": "user-uuid",
  "username": "user@example.com",
  "role": "user",
  "iat": 1709827200,
  "exp": 1709828100
}
```

### 2.3 Brute Force Protection

| Control | Threshold | Action |
|---------|-----------|--------|
| API Rate Limit (login) | 10/minute per IP | 429 response |
| API Rate Limit (general) | 100/minute per user | 429 response |
| Fail2ban (Postfix) | 5 failures in 10 min | 1-hour IP ban |
| Fail2ban (Dovecot) | 5 failures in 10 min | 1-hour IP ban |
| Account lockout | After 20 failures | Manual unlock required |

---

## 3. Transport Security (TLS)

### 3.1 HTTPS (Nginx → Client)

```nginx
ssl_protocols TLSv1.2 TLSv1.3;
ssl_ciphers ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384;
ssl_prefer_server_ciphers on;
ssl_session_timeout 1d;
ssl_session_tickets off;
ssl_stapling on;
ssl_stapling_verify on;
```

### 3.2 SMTP TLS (Postfix)

**Inbound (receiving from other servers):**
```ini
smtpd_tls_security_level = may          # Opportunistic TLS
smtpd_tls_protocols = !SSLv2, !SSLv3, !TLSv1, !TLSv1.1
smtpd_tls_mandatory_protocols = !SSLv2, !SSLv3, !TLSv1, !TLSv1.1
smtpd_tls_ciphers = high
```

**Outbound (sending to other servers):**
```ini
smtp_tls_security_level = dane          # DANE with DNSSEC
smtp_dns_support_level = dnssec
smtp_tls_CAfile = /etc/ssl/certs/ca-certificates.crt
```

**Submission (user → Postfix, port 587):**
```ini
smtpd_tls_security_level = encrypt      # Mandatory TLS
```

### 3.3 IMAP TLS (Dovecot)

```ini
ssl = required
ssl_min_protocol = TLSv1.2
ssl_prefer_server_ciphers = yes
```

---

## 4. Email Authentication (Anti-Spoofing)

### 4.1 SPF (Sender Policy Framework)

Authorizes which IP addresses can send email for your domain:

```dns
yourdomain.com. IN TXT "v=spf1 ip4:203.0.113.5 -all"
```

- `ip4:` — only this server can send
- `-all` — hard fail for all other sources (reject)

### 4.2 DKIM (DomainKeys Identified Mail)

Cryptographically signs every outbound email:

- Key size: 2048-bit RSA (minimum for 2025+)
- Selector: `default`
- Headers signed: From, To, Subject, Date, Message-ID, MIME-Version, Content-Type
- Algorithm: rsa-sha256

### 4.3 DMARC (Domain-based Message Authentication)

Policy that tells receiving servers what to do with unauthenticated email:

**Rollout Schedule:**
```
Month 1:  p=none         (monitor, collect reports)
Month 2:  p=quarantine   (move to spam)
Month 3+: p=reject       (block delivery)
```

### 4.4 DANE / MTA-STS

Additional transport security:

- **DANE:** Pins TLS certificate in DNS via TLSA records (requires DNSSEC)
- **MTA-STS:** HTTP-based TLS enforcement policy (fallback when DNSSEC unavailable)

---

## 5. Application Security

### 5.1 HTML Email Sanitization

All HTML email bodies are sanitized before delivery to the frontend:

```typescript
import DOMPurify from 'dompurify';

const ALLOWED_TAGS = [
  'a', 'b', 'br', 'blockquote', 'code', 'div', 'em', 'h1', 'h2', 'h3',
  'h4', 'h5', 'h6', 'hr', 'i', 'img', 'li', 'ol', 'p', 'pre', 'span',
  'strong', 'table', 'tbody', 'td', 'th', 'thead', 'tr', 'u', 'ul',
  'font', 'center', 'small', 'sub', 'sup',
];

const ALLOWED_ATTR = [
  'href', 'src', 'alt', 'title', 'width', 'height', 'style',
  'class', 'id', 'colspan', 'rowspan', 'align', 'valign',
  'color', 'size', 'face', 'bgcolor', 'border', 'cellpadding', 'cellspacing',
];

const sanitized = DOMPurify.sanitize(rawHtml, {
  ALLOWED_TAGS,
  ALLOWED_ATTR,
  ALLOW_DATA_ATTR: false,
  FORBID_TAGS: ['script', 'style', 'iframe', 'object', 'embed', 'form', 'input', 'button'],
  FORBID_ATTR: ['onerror', 'onload', 'onclick', 'onmouseover', 'onfocus'],
});
```

**Blocked:**
- All `<script>` tags
- All JavaScript event handlers (`on*` attributes)
- `<iframe>`, `<object>`, `<embed>` (embed attacks)
- `<form>`, `<input>`, `<button>` (phishing forms)
- `<style>` tags (CSS injection; inline styles are filtered)
- `javascript:` URLs
- `data:` URLs (except for images)

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
  form-action 'self';
```

### 5.3 CORS Configuration

```rust
let cors = CorsLayer::new()
    .allow_origin("https://mail.yourdomain.com".parse::<HeaderValue>().unwrap())
    .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
    .allow_headers([AUTHORIZATION, CONTENT_TYPE])
    .allow_credentials(true);
```

### 5.4 SQL Injection Prevention

All database queries use parameterized queries via sqlx:

```rust
// SAFE: parameterized query
let user = sqlx::query_as!(
    Mailbox,
    "SELECT * FROM mailboxes WHERE username = $1 AND active = TRUE",
    username
)
.fetch_optional(&pool)
.await?;

// NEVER: string concatenation
// let query = format!("SELECT * FROM mailboxes WHERE username = '{}'", username);
```

### 5.5 Input Validation

| Input | Validation |
|-------|-----------|
| Email address | RFC 5322 format validation via `email_address` crate |
| Password | Minimum 8 characters; checked against common passwords list |
| Folder name | Alphanumeric + `_` + `-` + `/`; max 255 chars |
| Search query | Max 500 chars; IMAP-safe character filtering |
| Attachment filename | Strip path separators; sanitize special characters |
| Message body | Max 25 MB total (including attachments) |

---

## 6. Infrastructure Security

### 6.1 Service Hardening (systemd)

```ini
# RustMail service hardening
NoNewPrivileges=true          # Cannot gain privileges
ProtectSystem=strict          # Read-only filesystem (except specified)
ProtectHome=true              # No access to /home
PrivateTmp=true               # Isolated /tmp
ReadWritePaths=/var/log/rustmail
CapabilityBoundingSet=        # No capabilities
SystemCallFilter=@system-service  # Restricted syscalls
```

### 6.2 PostgreSQL Hardening

```ini
# pg_hba.conf — local connections only
local   rustmail    rustmail    md5
host    rustmail    rustmail    127.0.0.1/32    md5

# postgresql.conf
listen_addresses = 'localhost'    # No external connections
ssl = on
```

### 6.3 File Permissions

| Path | Owner | Permissions |
|------|-------|-------------|
| `/etc/rustmail/config.toml` | root:rustmail | 640 |
| `/etc/rustmail/jwt-private.pem` | rustmail:rustmail | 600 |
| `/etc/postfix/pgsql-*.cf` | root:postfix | 640 |
| `/etc/dovecot/dovecot-sql.conf.ext` | root:dovecot | 640 |
| `/etc/opendkim/keys/` | opendkim:opendkim | 600 |
| `/var/vmail/` | vmail:vmail | 770 |
| `/opt/rustmail/bin/rustmail` | rustmail:rustmail | 755 |

---

## 7. Security Monitoring

### 7.1 Log Monitoring

| Log | Location | Watch For |
|-----|----------|-----------|
| RustMail API | journald / /var/log/rustmail/ | Failed logins, 4xx/5xx errors |
| Postfix | /var/log/mail.log | Rejected connections, auth failures |
| Dovecot | /var/log/mail.log | Auth failures, connection errors |
| Nginx | /var/log/nginx/ | 4xx/5xx errors, unusual patterns |
| Fail2ban | /var/log/fail2ban.log | Bans and unbans |

### 7.2 Alerting Checklist

| Event | Action |
|-------|--------|
| > 50 failed logins in 1 hour | Investigate IPs; consider geo-blocking |
| TLS certificate expiring in < 14 days | Verify certbot renewal |
| Postfix queue > 100 messages | Check for spam relay compromise |
| Disk usage > 80% | Clean old mail or expand storage |
| Service restart (unexpected) | Check logs for crash cause |

---

## 8. Vulnerability Management

### 8.1 Dependency Updates

```bash
# Rust dependencies
cargo audit              # Check for known vulnerabilities
cargo update            # Update to latest compatible versions

# Node.js dependencies
npm audit               # Check for vulnerabilities
npm update              # Update packages

# System packages
sudo apt update && sudo apt upgrade
```

### 8.2 Security Headers Checklist

| Header | Value | Purpose |
|--------|-------|---------|
| `Strict-Transport-Security` | `max-age=31536000; includeSubDomains` | Force HTTPS |
| `X-Frame-Options` | `DENY` | Prevent clickjacking |
| `X-Content-Type-Options` | `nosniff` | Prevent MIME sniffing |
| `Referrer-Policy` | `strict-origin-when-cross-origin` | Limit referrer info |
| `Content-Security-Policy` | (see 5.2) | Prevent XSS/injection |
| `Permissions-Policy` | `camera=(), microphone=(), geolocation=()` | Restrict browser APIs |

---

## 9. Incident Response

### 9.1 If Server is Compromised

1. **Isolate:** Block all ports except SSH
2. **Assess:** Check auth logs, mail logs, process list
3. **Rotate:** Change all passwords, rotate JWT keys, rotate DKIM keys
4. **Patch:** Apply security updates
5. **Restore:** Restore from known-good backup if needed
6. **Notify:** Inform affected users

### 9.2 If Email Sending is Abused

1. **Block:** Disable compromised account immediately
2. **Queue:** Flush Postfix mail queue (`postsuper -d ALL`)
3. **Investigate:** Check which account was compromised
4. **Report:** Contact blacklist removal if server was listed
5. **Harden:** Strengthen password policy, add 2FA

---

## 10. Compliance Notes

| Standard | Relevance |
|----------|-----------|
| GDPR | User data stored in PostgreSQL; provide data export/deletion |
| DKIM/SPF/DMARC | Required by Google/Microsoft for email delivery |
| TLS 1.2+ | PCI DSS requirement for encrypted transport |
| Password Hashing | OWASP recommends Argon2id |
| HSTS | Required for modern web security standards |
