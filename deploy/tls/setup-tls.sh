#!/usr/bin/env bash
# Added: Let's Encrypt TLS setup script for TASMail (TMAIL-16)
# Obtains certificates, configures auto-renewal, generates DANE hash,
# and outputs Postfix/Dovecot TLS configuration snippets.
#
# Usage: ./setup-tls.sh <mail-hostname> [--webroot /var/www/html] [--email admin@example.com]
#   mail-hostname  — FQDN for the mail server (e.g., mail.example.com)
#   --webroot      — use webroot method instead of standalone (for running web servers)
#   --email        — email for Let's Encrypt notifications (default: postmaster@<domain>)
#
# Dependencies: certbot, openssl
# Must be run as root (or with sudo)

set -euo pipefail

# Added: Color output helpers
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
BOLD='\033[1m'
RESET='\033[0m'

info() { echo -e "${GREEN}[INFO]${RESET} $1"; }
error() { echo -e "${RED}[ERROR]${RESET} $1"; }
warn() { echo -e "${YELLOW}[WARN]${RESET} $1"; }

# Added: Parse arguments
MAIL_HOST=""
WEBROOT=""
EMAIL=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --webroot)
            WEBROOT="$2"
            shift 2
            ;;
        --email)
            EMAIL="$2"
            shift 2
            ;;
        -h|--help)
            echo "Usage: $0 <mail-hostname> [--webroot /var/www/html] [--email admin@example.com]"
            echo ""
            echo "Options:"
            echo "  --webroot PATH   Use webroot authentication (for servers already running nginx/apache)"
            echo "  --email ADDR     Email for Let's Encrypt expiry notifications"
            echo "  -h, --help       Show this help"
            exit 0
            ;;
        *)
            if [[ -z "$MAIL_HOST" ]]; then
                MAIL_HOST="$1"
            else
                error "Unknown argument: $1"
                exit 1
            fi
            shift
            ;;
    esac
done

if [[ -z "$MAIL_HOST" ]]; then
    error "Mail hostname is required."
    echo "Usage: $0 <mail-hostname> [--webroot /var/www/html] [--email admin@example.com]"
    exit 1
fi

# Added: Extract domain from mail hostname (mail.example.com -> example.com)
DOMAIN="${MAIL_HOST#*.}"
EMAIL="${EMAIL:-postmaster@${DOMAIN}}"

# Added: Check root privileges
if [[ $EUID -ne 0 ]]; then
    error "This script must be run as root (or with sudo)."
    exit 1
fi

echo -e "${BOLD}TASMail TLS Setup${RESET}"
echo "Mail hostname: ${MAIL_HOST}"
echo "Contact email: ${EMAIL}"
echo ""

# --------------------------------------------------------------------------
# Step 1: Install certbot if not present
# --------------------------------------------------------------------------
info "Checking for certbot..."
if command -v certbot &>/dev/null; then
    info "certbot found: $(certbot --version 2>&1)"
else
    info "Installing certbot..."
    if command -v apt-get &>/dev/null; then
        apt-get update -qq
        apt-get install -y -qq certbot
    elif command -v dnf &>/dev/null; then
        dnf install -y certbot
    elif command -v yum &>/dev/null; then
        yum install -y certbot
    elif command -v pacman &>/dev/null; then
        pacman -S --noconfirm certbot
    else
        error "Cannot detect package manager. Install certbot manually."
        exit 1
    fi
    info "certbot installed successfully."
fi

# --------------------------------------------------------------------------
# Step 2: Obtain certificate
# --------------------------------------------------------------------------
CERT_DIR="/etc/letsencrypt/live/${MAIL_HOST}"

if [[ -d "$CERT_DIR" ]]; then
    warn "Certificate already exists at ${CERT_DIR}. Skipping issuance."
    warn "To force renewal: certbot renew --force-renewal --cert-name ${MAIL_HOST}"
else
    info "Obtaining Let's Encrypt certificate for ${MAIL_HOST}..."

    if [[ -n "$WEBROOT" ]]; then
        # Added: Webroot method — use when nginx/apache is already running
        info "Using webroot method (webroot: ${WEBROOT})"
        certbot certonly \
            --webroot \
            --webroot-path "$WEBROOT" \
            --domain "$MAIL_HOST" \
            --email "$EMAIL" \
            --agree-tos \
            --non-interactive
    else
        # Added: Standalone method — certbot runs its own temporary HTTP server
        # Requires port 80 to be free
        info "Using standalone method (ensure port 80 is free)"
        certbot certonly \
            --standalone \
            --domain "$MAIL_HOST" \
            --email "$EMAIL" \
            --agree-tos \
            --non-interactive
    fi

    info "Certificate obtained successfully."
fi

# --------------------------------------------------------------------------
# Step 3: Verify certificate files exist
# --------------------------------------------------------------------------
info "Verifying certificate files..."
for FILE in cert.pem chain.pem fullchain.pem privkey.pem; do
    if [[ -f "${CERT_DIR}/${FILE}" ]]; then
        info "  Found: ${CERT_DIR}/${FILE}"
    else
        error "  Missing: ${CERT_DIR}/${FILE}"
        exit 1
    fi
done

# --------------------------------------------------------------------------
# Step 4: Set up auto-renewal hook
# --------------------------------------------------------------------------
HOOK_DIR="/etc/letsencrypt/renewal-hooks/post"
HOOK_SCRIPT="${HOOK_DIR}/tasmail-reload.sh"

info "Setting up post-renewal hook..."
mkdir -p "$HOOK_DIR"

# Added: Copy the renew-hook.sh from the deploy directory if available
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
if [[ -f "${SCRIPT_DIR}/renew-hook.sh" ]]; then
    cp "${SCRIPT_DIR}/renew-hook.sh" "$HOOK_SCRIPT"
    chmod +x "$HOOK_SCRIPT"
    info "Installed renewal hook from ${SCRIPT_DIR}/renew-hook.sh"
else
    # Added: Create a minimal hook inline
    cat > "$HOOK_SCRIPT" << 'HOOK_EOF'
#!/usr/bin/env bash
# Added: TASMail certificate renewal hook
systemctl reload postfix 2>/dev/null || true
systemctl reload dovecot 2>/dev/null || true
systemctl reload nginx 2>/dev/null || true
HOOK_EOF
    chmod +x "$HOOK_SCRIPT"
    info "Created minimal renewal hook at ${HOOK_SCRIPT}"
fi

# Added: Verify certbot timer is active
if systemctl is-active --quiet certbot.timer 2>/dev/null; then
    info "certbot.timer is active (auto-renewal enabled)"
else
    warn "certbot.timer is not active. Enable with: systemctl enable --now certbot.timer"
fi

# --------------------------------------------------------------------------
# Step 5: Generate DANE TLSA hash
# --------------------------------------------------------------------------
info "Generating DANE TLSA hash..."
TLSA_HASH=$(openssl x509 -in "${CERT_DIR}/cert.pem" \
    -noout -pubkey 2>/dev/null | \
    openssl pkey -pubin -outform DER 2>/dev/null | \
    openssl dgst -sha256 -binary | \
    xxd -p -c 64)

if [[ -n "$TLSA_HASH" ]]; then
    info "DANE TLSA hash generated successfully"
    echo ""
    echo -e "${BOLD}DANE TLSA DNS Record:${RESET}"
    echo "_25._tcp.${MAIL_HOST}.  IN  TLSA  3 1 1 ${TLSA_HASH}"
    echo ""
else
    warn "Could not generate TLSA hash. Ensure openssl and xxd are installed."
fi

# --------------------------------------------------------------------------
# Step 6: Output Postfix TLS configuration
# --------------------------------------------------------------------------
echo -e "${BOLD}Postfix TLS Configuration (add to /etc/postfix/main.cf):${RESET}"
cat << EOF
# --- TLS Settings (TLS 1.2 minimum, TLS 1.3 preferred) ---
# Added: TLS certificate paths for Let's Encrypt
smtpd_tls_cert_file = ${CERT_DIR}/fullchain.pem
smtpd_tls_key_file = ${CERT_DIR}/privkey.pem
smtpd_tls_security_level = may
smtpd_tls_auth_only = yes
smtpd_tls_loglevel = 1
smtpd_tls_protocols = !SSLv2, !SSLv3, !TLSv1, !TLSv1.1
smtpd_tls_mandatory_protocols = !SSLv2, !SSLv3, !TLSv1, !TLSv1.1
smtpd_tls_mandatory_ciphers = high
smtpd_tls_ciphers = high
tls_preempt_cipherlist = yes

# Added: Outbound TLS (opportunistic, high-grade ciphers)
smtp_tls_security_level = may
smtp_tls_loglevel = 1
smtp_tls_protocols = !SSLv2, !SSLv3, !TLSv1, !TLSv1.1
smtp_tls_mandatory_protocols = !SSLv2, !SSLv3, !TLSv1, !TLSv1.1
smtp_tls_mandatory_ciphers = high
smtp_tls_CApath = /etc/ssl/certs

# Added: TLS session cache
smtpd_tls_session_cache_database = btree:\${data_directory}/smtpd_scache
smtp_tls_session_cache_database = btree:\${data_directory}/smtp_scache
EOF

echo ""

# --------------------------------------------------------------------------
# Step 7: Output Dovecot TLS configuration
# --------------------------------------------------------------------------
echo -e "${BOLD}Dovecot TLS Configuration (add to /etc/dovecot/conf.d/10-ssl.conf):${RESET}"
cat << EOF
# Added: TLS settings for Dovecot IMAP (TLS 1.2 min, TLS 1.3 preferred)
ssl = required
ssl_cert = <${CERT_DIR}/fullchain.pem
ssl_key = <${CERT_DIR}/privkey.pem
ssl_min_protocol = TLSv1.2
ssl_prefer_server_ciphers = yes
ssl_cipher_list = ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384:ECDHE-ECDSA-CHACHA20-POLY1305:ECDHE-RSA-CHACHA20-POLY1305:ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:!aNULL:!MD5:!DSS
ssl_ciphersuites = TLS_AES_256_GCM_SHA384:TLS_CHACHA20_POLY1305_SHA256:TLS_AES_128_GCM_SHA256
EOF

echo ""
info "TLS setup complete for ${MAIL_HOST}"
info "Next steps:"
echo "  1. Add Postfix TLS config to /etc/postfix/main.cf"
echo "  2. Add Dovecot TLS config to /etc/dovecot/conf.d/10-ssl.conf"
echo "  3. Add DANE TLSA record to your DNS zone"
echo "  4. Reload services: systemctl reload postfix dovecot"
echo "  5. Test with: openssl s_client -connect ${MAIL_HOST}:993 -servername ${MAIL_HOST}"
