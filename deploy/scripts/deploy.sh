#!/usr/bin/env bash
# Added: TASMail production deployment script (TMAIL-40)
# PURPOSE: Builds backend and frontend, deploys artifacts to production paths,
#          reloads Nginx, restarts the backend service, and runs a health check.
# USAGE: sudo ./deploy.sh [--skip-build] [--backend-only] [--frontend-only]

set -euo pipefail

# Added: Configuration variables
REPO_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
BACKEND_DIR="${REPO_DIR}/backend"
FRONTEND_DIR="${REPO_DIR}/frontend"
DEPLOY_DIR="${REPO_DIR}/deploy"

INSTALL_BIN="/usr/local/bin/tasmail"
FRONTEND_DEST="/var/www/tasmail/frontend/dist"
NGINX_CONF_SRC="${DEPLOY_DIR}/nginx/tasmail.conf"
NGINX_CONF_DEST="/etc/nginx/sites-available/tasmail.conf"
NGINX_ENABLED="/etc/nginx/sites-enabled/tasmail.conf"
# Added: Logrotate config for nginx tasmail vhost logs (TMAIL-40)
LOGROTATE_CONF_SRC="${DEPLOY_DIR}/logrotate/tasmail"
LOGROTATE_CONF_DEST="/etc/logrotate.d/tasmail"

HEALTH_URL="http://127.0.0.1:3000/api/health"
HEALTH_TIMEOUT=30

# Added: Color output helpers
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info()  { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Added: Parse command-line flags
SKIP_BUILD=false
BACKEND_ONLY=false
FRONTEND_ONLY=false

for arg in "$@"; do
    case "$arg" in
        --skip-build)    SKIP_BUILD=true ;;
        --backend-only)  BACKEND_ONLY=true ;;
        --frontend-only) FRONTEND_ONLY=true ;;
        *)               log_error "Unknown flag: $arg"; exit 1 ;;
    esac
done

# Added: Verify running as root (needed for systemctl, file copies)
if [[ $EUID -ne 0 ]]; then
    log_error "This script must be run as root (or with sudo)"
    exit 1
fi

# Added: Build backend — compile release binary with optimizations
build_backend() {
    if [[ "$SKIP_BUILD" == "true" ]]; then
        log_warn "Skipping backend build (--skip-build)"
        return
    fi
    log_info "Building backend (cargo build --release)..."
    cd "$BACKEND_DIR"
    cargo build --release
    log_info "Backend build complete"
}

# Added: Build frontend — TypeScript check + Vite production bundle
build_frontend() {
    if [[ "$SKIP_BUILD" == "true" ]]; then
        log_warn "Skipping frontend build (--skip-build)"
        return
    fi
    log_info "Building frontend (npm run build)..."
    cd "$FRONTEND_DIR"
    npm ci --production=false
    npm run build
    log_info "Frontend build complete"
}

# Added: Deploy backend binary to /usr/local/bin
deploy_backend() {
    log_info "Deploying backend binary to ${INSTALL_BIN}..."
    cp "${BACKEND_DIR}/target/release/tasmail" "$INSTALL_BIN"
    chmod 755 "$INSTALL_BIN"
    chown root:root "$INSTALL_BIN"
    log_info "Backend binary deployed"
}

# Added: Deploy frontend static files
deploy_frontend() {
    log_info "Deploying frontend to ${FRONTEND_DEST}..."
    mkdir -p "$FRONTEND_DEST"
    # Added: Remove old dist files before copying new ones
    rm -rf "${FRONTEND_DEST:?}/"*
    cp -r "${FRONTEND_DIR}/dist/"* "$FRONTEND_DEST/"
    chown -R www-data:www-data "$FRONTEND_DEST"
    log_info "Frontend deployed"
}

# Added: Deploy and reload Nginx configuration
deploy_nginx() {
    log_info "Deploying Nginx configuration..."
    cp "$NGINX_CONF_SRC" "$NGINX_CONF_DEST"

    # Added: Create symlink in sites-enabled if not present
    if [[ ! -L "$NGINX_ENABLED" ]]; then
        ln -sf "$NGINX_CONF_DEST" "$NGINX_ENABLED"
    fi

    # Added: Test Nginx config before reloading
    if nginx -t 2>&1; then
        systemctl reload nginx
        log_info "Nginx configuration reloaded"
    else
        log_error "Nginx configuration test failed — not reloading"
        exit 1
    fi
}

# Added: Install logrotate config for nginx tasmail logs (TMAIL-40)
deploy_logrotate() {
    log_info "Installing logrotate configuration..."
    if [[ ! -f "$LOGROTATE_CONF_SRC" ]]; then
        log_warn "Logrotate config not found at ${LOGROTATE_CONF_SRC} — skipping"
        return 0
    fi
    install -m 0644 -o root -g root "$LOGROTATE_CONF_SRC" "$LOGROTATE_CONF_DEST"
    # Added: Dry-run logrotate to catch syntax errors early
    if logrotate -d "$LOGROTATE_CONF_DEST" > /dev/null 2>&1; then
        log_info "Logrotate configuration installed at ${LOGROTATE_CONF_DEST}"
    else
        log_warn "Logrotate dry-run reported issues — inspect with: logrotate -d ${LOGROTATE_CONF_DEST}"
    fi
}

# Added: Restart the backend systemd service
restart_backend() {
    log_info "Restarting tasmail-backend service..."
    systemctl restart tasmail-backend
    log_info "Service restarted"
}

# Added: Health check — wait for backend to respond on /api/health
health_check() {
    log_info "Running health check (timeout: ${HEALTH_TIMEOUT}s)..."
    local elapsed=0
    while [[ $elapsed -lt $HEALTH_TIMEOUT ]]; do
        if curl -sf "$HEALTH_URL" > /dev/null 2>&1; then
            log_info "Health check passed — backend is responding"
            return 0
        fi
        sleep 2
        elapsed=$((elapsed + 2))
    done
    log_error "Health check FAILED after ${HEALTH_TIMEOUT}s — backend not responding on ${HEALTH_URL}"
    log_error "Check logs: journalctl -u tasmail-backend -n 50"
    exit 1
}

# Added: Main deployment flow
log_info "========================================="
log_info "TASMail Deployment — $(date '+%Y-%m-%d %H:%M:%S')"
log_info "========================================="

if [[ "$FRONTEND_ONLY" == "true" ]]; then
    build_frontend
    deploy_frontend
    deploy_nginx
    deploy_logrotate
elif [[ "$BACKEND_ONLY" == "true" ]]; then
    build_backend
    deploy_backend
    restart_backend
    health_check
else
    # Added: Full deployment — both backend and frontend
    build_backend
    build_frontend
    deploy_backend
    deploy_frontend
    deploy_nginx
    deploy_logrotate
    restart_backend
    health_check
fi

log_info "========================================="
log_info "Deployment complete"
log_info "========================================="
