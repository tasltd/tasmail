# Deployment Guide
# TASMail — Self-Hosted Email Service

**Version:** 1.0
**Date:** 2026-03-07
**Target OS:** Ubuntu 22.04 LTS / 24.04 LTS

---

## 1. Pre-Deployment Checklist

| # | Item | Status |
|---|------|--------|
| 1 | VPS with static public IPv4 address | [ ] |
| 2 | Domain name with DNS control | [ ] |
| 3 | Reverse DNS (PTR) configured for server IP → mail.yourdomain.com | [ ] |
| 4 | Port 25, 80, 143, 443, 465, 587, 993 open in firewall | [ ] |
| 5 | Server hostname set to `mail.yourdomain.com` | [ ] |
| 6 | Not on any email blacklists (check mxtoolbox.com) | [ ] |

---

## 2. DNS Configuration

### 2.1 Required Records

```dns
; A record — points to your server
mail.yourdomain.com.        IN  A      203.0.113.5

; AAAA record (if IPv6 available)
mail.yourdomain.com.        IN  AAAA   2001:db8::5

; MX record — directs email to your server
yourdomain.com.             IN  MX  10 mail.yourdomain.com.

; SPF — authorizes your server to send mail
yourdomain.com.             IN  TXT    "v=spf1 ip4:203.0.113.5 -all"

; DKIM — public key for email signing (generated in step 5)
default._domainkey.yourdomain.com. IN TXT "v=DKIM1; k=rsa; p=<public_key>"

; DMARC — email authentication policy
_dmarc.yourdomain.com.      IN  TXT    "v=DMARC1; p=none; rua=mailto:dmarc-reports@yourdomain.com"

; Autoconfig (for email clients)
autoconfig.yourdomain.com.  IN  CNAME  mail.yourdomain.com.
autodiscover.yourdomain.com. IN CNAME  mail.yourdomain.com.

; MTA-STS (optional but recommended)
_mta-sts.yourdomain.com.    IN  TXT    "v=STSv1; id=20260307"
mta-sts.yourdomain.com.     IN  CNAME  mail.yourdomain.com.
```

### 2.2 Reverse DNS (PTR Record)

Set via your VPS provider's control panel:
```
203.0.113.5 → mail.yourdomain.com
```

### 2.3 Verify DNS

```bash
# Check MX
dig MX yourdomain.com +short

# Check SPF
dig TXT yourdomain.com +short

# Check DKIM (after generating)
dig TXT default._domainkey.yourdomain.com +short

# Check DMARC
dig TXT _dmarc.yourdomain.com +short

# Check PTR
dig -x 203.0.113.5 +short
```

---

## 3. Server Setup

### 3.1 System Preparation

```bash
# Set hostname
sudo hostnamectl set-hostname mail.yourdomain.com

# Update /etc/hosts
echo "203.0.113.5 mail.yourdomain.com mail" | sudo tee -a /etc/hosts

# Update system
sudo apt update && sudo apt upgrade -y

# Install base packages
sudo apt install -y curl wget gnupg2 software-properties-common \
    ufw fail2ban unattended-upgrades
```

### 3.2 Firewall Configuration

```bash
# Enable UFW
sudo ufw default deny incoming
sudo ufw default allow outgoing

# Allow required ports
sudo ufw allow 22/tcp      # SSH
sudo ufw allow 25/tcp      # SMTP
sudo ufw allow 80/tcp      # HTTP (Let's Encrypt + redirect)
sudo ufw allow 143/tcp     # IMAP (optional, for local clients)
sudo ufw allow 443/tcp     # HTTPS
sudo ufw allow 465/tcp     # SMTPS
sudo ufw allow 587/tcp     # Submission
sudo ufw allow 993/tcp     # IMAPS

sudo ufw enable
sudo ufw status verbose
```

### 3.3 Fail2ban Configuration

```bash
sudo tee /etc/fail2ban/jail.local << 'EOF'
[DEFAULT]
bantime = 3600
findtime = 600
maxretry = 5

[sshd]
enabled = true
port = ssh

[postfix]
enabled = true
port = smtp,465,587
logpath = /var/log/mail.log
maxretry = 5

[dovecot]
enabled = true
port = imap,imaps,pop3,pop3s,sieve
logpath = /var/log/mail.log
maxretry = 5
EOF

sudo systemctl restart fail2ban
```

---

## 4. Install Components

### 4.1 PostgreSQL

```bash
sudo apt install -y postgresql-16 postgresql-client-16

# Create database
sudo -u postgres psql << 'SQL'
CREATE USER tasmail WITH PASSWORD 'CHANGE_THIS_PRODUCTION_PASSWORD';
CREATE DATABASE tasmail OWNER tasmail;
\c tasmail
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";
SQL

# Run schema migrations
psql -U tasmail -d tasmail -h localhost < /opt/tasmail/migrations/001_initial_schema.sql
```

### 4.2 Postfix

```bash
sudo apt install -y postfix postfix-pgsql

# Configure (see config/ templates)
sudo cp config/postfix/main.cf /etc/postfix/main.cf
sudo cp config/postfix/master.cf /etc/postfix/master.cf
sudo cp config/postfix/pgsql-*.cf /etc/postfix/

# Set correct permissions
sudo chmod 640 /etc/postfix/pgsql-*.cf
sudo chown root:postfix /etc/postfix/pgsql-*.cf

# Restart
sudo systemctl restart postfix
sudo systemctl enable postfix
```

### 4.3 Dovecot

```bash
sudo apt install -y dovecot-core dovecot-imapd dovecot-lmtpd dovecot-pgsql dovecot-sieve

# Create vmail user
sudo groupadd -g 5000 vmail
sudo useradd -g vmail -u 5000 vmail -d /var/vmail -s /sbin/nologin
sudo mkdir -p /var/vmail
sudo chown -R vmail:vmail /var/vmail

# Configure (see config/ templates)
sudo cp config/dovecot/*.conf /etc/dovecot/
sudo cp config/dovecot/conf.d/* /etc/dovecot/conf.d/
sudo cp config/dovecot/dovecot-sql.conf.ext /etc/dovecot/

# Restart
sudo systemctl restart dovecot
sudo systemctl enable dovecot
```

### 4.4 Rspamd

```bash
sudo apt install -y rspamd redis-server

# Configure Postfix to use Rspamd as milter
echo "smtpd_milters = inet:127.0.0.1:11332" | sudo tee -a /etc/postfix/main.cf
echo "non_smtpd_milters = inet:127.0.0.1:11332" | sudo tee -a /etc/postfix/main.cf
echo "milter_default_action = accept" | sudo tee -a /etc/postfix/main.cf

sudo systemctl restart postfix rspamd redis-server
sudo systemctl enable rspamd redis-server
```

---

## 5. DKIM Setup

```bash
# Generate DKIM key
sudo mkdir -p /etc/opendkim/keys/yourdomain.com
sudo opendkim-genkey -b 2048 -d yourdomain.com -D /etc/opendkim/keys/yourdomain.com -s default -v

# Set permissions
sudo chown -R opendkim:opendkim /etc/opendkim
sudo chmod 600 /etc/opendkim/keys/yourdomain.com/default.private

# Display public key (add this to DNS)
sudo cat /etc/opendkim/keys/yourdomain.com/default.txt

# Configure OpenDKIM
sudo tee /etc/opendkim.conf << 'EOF'
Syslog          yes
UMask           007
Mode            sv
PidFile         /run/opendkim/opendkim.pid
Socket          inet:8891@localhost
Canonicalization relaxed/simple
Domain          yourdomain.com
Selector        default
KeyFile         /etc/opendkim/keys/yourdomain.com/default.private
SignatureAlgorithm rsa-sha256
EOF

# Add OpenDKIM milter to Postfix
echo "milter_protocol = 6" | sudo tee -a /etc/postfix/main.cf
echo "smtpd_milters = inet:127.0.0.1:11332, inet:localhost:8891" | sudo tee -a /etc/postfix/main.cf

sudo systemctl restart opendkim postfix
sudo systemctl enable opendkim
```

---

## 6. TLS Certificates

```bash
# Obtain Let's Encrypt certificate
sudo certbot certonly --nginx -d mail.yourdomain.com

# Certificate files will be at:
# /etc/letsencrypt/live/mail.yourdomain.com/fullchain.pem
# /etc/letsencrypt/live/mail.yourdomain.com/privkey.pem

# Configure auto-renewal
sudo systemctl enable certbot.timer

# Test renewal
sudo certbot renew --dry-run
```

---

## 7. Deploy TASMail Backend

### 7.1 Build Release Binary

```bash
# On build machine (or CI)
cd backend/
cargo build --release

# Binary at: target/release/tasmail
# Copy to server
scp target/release/tasmail user@server:/opt/tasmail/bin/
```

### 7.2 Create Config

```bash
sudo mkdir -p /etc/tasmail /var/log/tasmail /opt/tasmail/bin

# Generate JWT keys
openssl genpkey -algorithm RSA -out /etc/tasmail/jwt-private.pem -pkeyopt rsa_keygen_bits:2048
openssl rsa -pubout -in /etc/tasmail/jwt-private.pem -out /etc/tasmail/jwt-public.pem
sudo chmod 600 /etc/tasmail/jwt-private.pem

# Create config file
sudo tee /etc/tasmail/config.toml << 'EOF'
[server]
host = "127.0.0.1"
port = 3000
workers = 4

[database]
url = "postgresql://tasmail:PRODUCTION_PASSWORD@localhost/tasmail"
max_connections = 20

[imap]
host = "127.0.0.1"
port = 993
tls = true

[smtp]
host = "127.0.0.1"
port = 587
tls = "starttls"

[auth]
jwt_private_key_path = "/etc/tasmail/jwt-private.pem"
jwt_public_key_path = "/etc/tasmail/jwt-public.pem"
access_token_expiry_minutes = 15
refresh_token_expiry_days = 7

[rate_limit]
requests_per_minute = 100
login_attempts_per_minute = 10

[logging]
level = "info"
format = "json"
EOF
```

### 7.3 Create System User and Service

```bash
# Create service user
sudo useradd -r -s /sbin/nologin tasmail

# Set permissions
sudo chown -R tasmail:tasmail /var/log/tasmail
sudo chown tasmail:tasmail /opt/tasmail/bin/tasmail

# Install systemd service
sudo tee /etc/systemd/system/tasmail.service << 'EOF'
[Unit]
Description=TASMail Email Service Backend
After=postgresql.service dovecot.service postfix.service
Wants=postgresql.service dovecot.service postfix.service

[Service]
Type=simple
User=tasmail
Group=tasmail
ExecStart=/opt/tasmail/bin/tasmail --config /etc/tasmail/config.toml
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal

NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/log/tasmail
PrivateTmp=true

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl start tasmail
sudo systemctl enable tasmail
```

---

## 8. Deploy React Frontend

```bash
# Build on dev machine
cd frontend/
npm run build

# Copy dist/ to server
scp -r dist/* user@server:/opt/tasmail/frontend/
```

---

## 9. Configure Nginx

```bash
sudo tee /etc/nginx/sites-available/tasmail << 'EOF'
server {
    listen 80;
    server_name mail.yourdomain.com;
    return 301 https://$host$request_uri;
}

server {
    listen 443 ssl http2;
    server_name mail.yourdomain.com;

    ssl_certificate     /etc/letsencrypt/live/mail.yourdomain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/mail.yourdomain.com/privkey.pem;
    ssl_protocols       TLSv1.2 TLSv1.3;
    ssl_ciphers         HIGH:!aNULL:!MD5;
    ssl_prefer_server_ciphers on;

    # Security headers
    add_header X-Frame-Options DENY always;
    add_header X-Content-Type-Options nosniff always;
    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;
    add_header Referrer-Policy "strict-origin-when-cross-origin" always;

    # React SPA
    root /opt/tasmail/frontend;
    index index.html;

    location / {
        try_files $uri $uri/ /index.html;
    }

    # API
    location /api/ {
        proxy_pass http://127.0.0.1:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        client_max_body_size 25m;
    }

    # WebSocket
    location /ws/ {
        proxy_pass http://127.0.0.1:3000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "Upgrade";
        proxy_set_header Host $host;
        proxy_read_timeout 3600s;
    }
}
EOF

sudo ln -s /etc/nginx/sites-available/tasmail /etc/nginx/sites-enabled/
sudo nginx -t
sudo systemctl restart nginx
sudo systemctl enable nginx
```

---

## 10. Post-Deployment Verification

```bash
# 1. Check all services
sudo systemctl status postgresql postfix dovecot tasmail nginx rspamd

# 2. Test SMTP
swaks --to test@yourdomain.com --from test@external.com --server localhost

# 3. Test IMAP
openssl s_client -connect localhost:993

# 4. Test API
curl -s https://mail.yourdomain.com/api/health | jq

# 5. Test webmail
# Open https://mail.yourdomain.com in browser

# 6. Test email deliverability
# Send email to check-auth@verifier.port25.com
# Send email to your Gmail and check headers for SPF/DKIM/DMARC pass

# 7. Online verification tools
# - https://mxtoolbox.com/SuperTool.aspx
# - https://www.mail-tester.com/
# - https://internet.nl/
```

---

## 11. Backup Strategy

```bash
# Database backup (daily cron)
pg_dump -U tasmail tasmail | gzip > /backups/tasmail-db-$(date +%Y%m%d).sql.gz

# Maildir backup (daily rsync)
rsync -avz /var/vmail/ /backups/vmail/

# Config backup
tar czf /backups/tasmail-config-$(date +%Y%m%d).tar.gz \
  /etc/tasmail/ /etc/postfix/ /etc/dovecot/ /etc/nginx/sites-available/tasmail

# DKIM keys backup (keep secure!)
sudo cp -r /etc/opendkim/keys/ /backups/dkim-keys/
```

---

## 12. Updating TASMail

```bash
# 1. Build new release
cd backend/ && cargo build --release

# 2. Deploy with zero downtime
sudo systemctl stop tasmail
sudo cp target/release/tasmail /opt/tasmail/bin/tasmail
sudo systemctl start tasmail

# 3. Update frontend
cd frontend/ && npm run build
sudo rsync -avz --delete dist/ /opt/tasmail/frontend/

# 4. Verify
curl -s https://mail.yourdomain.com/api/health
```
