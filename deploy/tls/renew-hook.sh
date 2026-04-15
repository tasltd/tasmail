#!/usr/bin/env bash
# Added: Let's Encrypt post-renewal hook for TASMail (TMAIL-16)
# Reloads mail services after certificate renewal and outputs updated DANE hash.
#
# Installation: Copy to /etc/letsencrypt/renewal-hooks/post/tasmail-reload.sh
# Or use setup-tls.sh which installs this automatically.
#
# This script is called by certbot after any certificate renewal.

set -euo pipefail

# Added: Logging helper
log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] TASMail renewal hook: $1"
}

log "Certificate renewed. Reloading services..."

# Added: Reload Postfix (SMTP server)
if systemctl is-active --quiet postfix; then
    systemctl reload postfix
    log "Postfix reloaded successfully"
else
    log "Postfix is not running, skipping reload"
fi

# Added: Reload Dovecot (IMAP server)
if systemctl is-active --quiet dovecot; then
    systemctl reload dovecot
    log "Dovecot reloaded successfully"
else
    log "Dovecot is not running, skipping reload"
fi

# Added: Reload Nginx (reverse proxy / webmail)
if systemctl is-active --quiet nginx; then
    systemctl reload nginx
    log "Nginx reloaded successfully"
else
    log "Nginx is not running, skipping reload"
fi

# Added: Generate updated DANE TLSA hash for DNS
# The RENEWED_LINEAGE env var is set by certbot during renewal
if [[ -n "${RENEWED_LINEAGE:-}" ]] && [[ -f "${RENEWED_LINEAGE}/cert.pem" ]]; then
    TLSA_HASH=$(openssl x509 -in "${RENEWED_LINEAGE}/cert.pem" \
        -noout -pubkey 2>/dev/null | \
        openssl pkey -pubin -outform DER 2>/dev/null | \
        openssl dgst -sha256 -binary | \
        xxd -p -c 64)

    if [[ -n "$TLSA_HASH" ]]; then
        # Added: Extract hostname from certificate path
        CERT_NAME=$(basename "$RENEWED_LINEAGE")
        log "Updated DANE TLSA hash for ${CERT_NAME}:"
        log "  _25._tcp.${CERT_NAME}.  IN  TLSA  3 1 1 ${TLSA_HASH}"
        log "ACTION REQUIRED: Update your DNS TLSA record with the hash above"
    else
        log "WARNING: Could not generate TLSA hash from renewed certificate"
    fi
else
    log "RENEWED_LINEAGE not set or cert not found; skipping TLSA hash generation"
fi

log "Renewal hook complete"
