#!/bin/bash
# ============================================================================
# Cerberus One-Click Deployment Script
# Fresh Ubuntu 22.04/24.04 LTS → Production Tor Hidden Service
# 
# Run as root:
#   curl -sSL https://raw.githubusercontent.com/YOUR_USER/Cerberus/main/deploy/cerberus-one-click.sh | sudo bash
# 
# Or clone and run:
#   git clone https://github.com/YOUR_USER/Cerberus.git
#   cd Cerberus
#   sudo bash deploy/cerberus-one-click.sh
# ============================================================================

set -e

# =============================================================================
# HARDCODED TEST CONFIGURATION
# =============================================================================
# Backend: The actual Tor hidden service we're protecting
BACKEND_ONION="sigilahzwq5u34gdh2bl3ymokyc7kobika55kyhztsucdoub73hz7qid.onion"

# Vanity: Will be GENERATED during deployment (first 5 chars match)
VANITY_PREFIX="sigil"

# Fortify listen port
FORTIFY_PORT="8888"

# GitHub Repository (public, no auth needed)
GITHUB_REPO="https://github.com/Nespartious/Cerberus.git"

# =============================================================================
# PATHS
# =============================================================================
CERBERUS_ROOT="/etc/cerberus"
TOR_HS_DIR="/var/lib/tor/cerberus_hs"
HAPROXY_CFG="/etc/haproxy/haproxy.cfg"
NGINX_SITE="/etc/nginx/sites-available/cerberus"
FORTIFY_BIN="/usr/local/bin/fortify"
SYSTEMD_DIR="/etc/systemd/system"
INSTALL_DIR="/opt/cerberus"
RUST_USER="cerberus"
DASHBOARD_PORT="9999"
HAS_DISPLAY=false

# =============================================================================
# COLORS
# =============================================================================
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# =============================================================================
# LOGGING
# =============================================================================
log_info()    { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[✓]${NC} $1"; }
log_warn()    { echo -e "${YELLOW}[!]${NC} $1"; }
log_error()   { echo -e "${RED}[✗]${NC} $1"; }

# =============================================================================
# DETECT DISPLAY ENVIRONMENT
# =============================================================================
detect_display() {
    # Check for various display indicators
    if [[ -n "$DISPLAY" ]]; then
        HAS_DISPLAY=true
        log_success "Display detected: X11 (DISPLAY=$DISPLAY)"
    elif [[ -n "$WAYLAND_DISPLAY" ]]; then
        HAS_DISPLAY=true
        log_success "Display detected: Wayland"
    elif command -v xdg-open &> /dev/null && xdpyinfo &> /dev/null 2>&1; then
        HAS_DISPLAY=true
        log_success "Display detected: X11 via xdpyinfo"
    else
        HAS_DISPLAY=false
        log_info "No display detected (headless mode)"
    fi
}

# =============================================================================
# BANNER
# =============================================================================
print_banner() {
    echo ""
    echo -e "${RED}╔═══════════════════════════════════════════════════════════╗${NC}"
    echo -e "${RED}║${NC}        ${YELLOW}CERBERUS${NC} - Multi-Layer Tor Defense System         ${RED}║${NC}"
    echo -e "${RED}║${NC}                  One-Click Deployment                      ${RED}║${NC}"
    echo -e "${RED}╚═══════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "Backend Target:  ${YELLOW}${BACKEND_ONION}${NC}"
    echo -e "Vanity Prefix:   ${GREEN}${VANITY_PREFIX}*${NC} (will be generated)"
    echo ""
}

# =============================================================================
# PRE-FLIGHT CHECKS
# =============================================================================
check_root() {
    if [[ $EUID -ne 0 ]]; then
        log_error "This script must be run as root"
        echo "Try: sudo bash $0"
        exit 1
    fi
    log_success "Running as root"
}

check_os() {
    if [[ -f /etc/os-release ]]; then
        . /etc/os-release
        if [[ "$ID" == "ubuntu" || "$ID" == "debian" ]]; then
            log_success "Detected OS: $PRETTY_NAME"
            return 0
        fi
    fi
    log_error "Unsupported OS. This script requires Ubuntu or Debian."
    exit 1
}

# =============================================================================
# EXTENSIVE PRE-FLIGHT CHECKS
# =============================================================================
preflight_checks() {
    log_info "Running pre-flight checks..."
    
    PREFLIGHT_FAILED=0
    
    # Check internet connectivity
    log_info "  [1/6] Checking internet connectivity..."
    if ping -c 1 google.com &> /dev/null || ping -c 1 1.1.1.1 &> /dev/null; then
        log_success "  Internet connectivity OK"
    else
        log_error "  No internet connectivity!"
        PREFLIGHT_FAILED=1
    fi
    
    # Check disk space (need at least 2GB)
    log_info "  [2/6] Checking disk space..."
    AVAILABLE_GB=$(df / | awk 'NR==2 {print int($4/1024/1024)}')
    if [[ $AVAILABLE_GB -ge 2 ]]; then
        log_success "  Disk space OK (${AVAILABLE_GB}GB available)"
    else
        log_warn "  Low disk space: ${AVAILABLE_GB}GB (recommend 2GB+)"
    fi
    
    # Check memory (need at least 512MB)
    log_info "  [3/6] Checking memory..."
    MEM_MB=$(grep MemTotal /proc/meminfo | awk '{print int($2/1024)}')
    if [[ $MEM_MB -ge 512 ]]; then
        log_success "  Memory OK (${MEM_MB}MB total)"
    else
        log_warn "  Low memory: ${MEM_MB}MB (recommend 512MB+)"
    fi
    
    # Check for existing Tor installation conflicts
    log_info "  [4/6] Checking for Tor conflicts..."
    if [[ -f /etc/tor/torrc ]]; then
        log_warn "  Existing torrc found - will be backed up and replaced"
    fi
    if [[ -d /var/lib/tor/hidden_service ]]; then
        log_warn "  Existing default hidden service found - will not interfere"
    fi
    
    # Check for port conflicts
    log_info "  [5/6] Checking for port conflicts..."
    PORTS_TO_CHECK="10000 10001 8888 9999 8404"
    for port in $PORTS_TO_CHECK; do
        if ss -tlnp 2>/dev/null | grep -q ":$port "; then
            log_warn "  Port $port is already in use!"
            ss -tlnp 2>/dev/null | grep ":$port " | head -1
        fi
    done
    log_success "  Port check complete"
    
    # Check for previous failed installations
    log_info "  [6/6] Checking for previous installations..."
    if systemctl is-active --quiet tor 2>/dev/null; then
        log_warn "  Tor is currently running - will be reconfigured"
        systemctl stop tor 2>/dev/null || true
    fi
    if systemctl is-active --quiet fortify 2>/dev/null; then
        log_warn "  Fortify is currently running - will be reconfigured"
        systemctl stop fortify 2>/dev/null || true
    fi
    if systemctl is-active --quiet haproxy 2>/dev/null; then
        log_warn "  HAProxy is currently running - will be reconfigured"
        systemctl stop haproxy 2>/dev/null || true
    fi
    if systemctl is-active --quiet nginx 2>/dev/null; then
        log_warn "  Nginx is currently running - will be reconfigured"
        systemctl stop nginx 2>/dev/null || true
    fi
    log_success "  Previous installation check complete"
    
    if [[ $PREFLIGHT_FAILED -eq 1 ]]; then
        log_error "Pre-flight checks failed. Please fix the above issues."
        exit 1
    fi
    
    log_success "All pre-flight checks passed"
}

# =============================================================================
# SYSTEM PREPARATION
# =============================================================================
prepare_system() {
    log_info "Updating system packages..."
    apt-get update -qq
    apt-get upgrade -y -qq
    log_success "System updated"
}

install_base_deps() {
    log_info "Installing base dependencies..."
    apt-get install -y -qq \
        curl wget git build-essential \
        pkg-config libssl-dev \
        ca-certificates gnupg lsb-release \
        software-properties-common
    log_success "Base dependencies installed"
}

# =============================================================================
# INSTALL RUST TOOLCHAIN
# =============================================================================
install_rust() {
    log_info "Installing Rust toolchain..."
    
    # Check if Rust is already installed
    if command -v rustc &> /dev/null; then
        RUST_VER=$(rustc --version)
        log_success "Rust already installed: $RUST_VER"
        return 0
    fi
    
    # Install rustup for the current user (root in this case)
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
    
    # Source cargo environment
    source "$HOME/.cargo/env"
    
    # Verify installation
    if command -v rustc &> /dev/null; then
        RUST_VER=$(rustc --version)
        log_success "Rust installed: $RUST_VER"
    else
        log_error "Rust installation failed"
        exit 1
    fi
}

# =============================================================================
# INSTALL SERVICE DEPENDENCIES
# =============================================================================
install_services() {
    log_info "Installing Tor, HAProxy, Nginx, Redis..."
    
    # Add Tor Project official repository for latest version
    log_info "Adding Tor Project repository..."
    apt-get install -y -qq apt-transport-https
    
    # Create sources list for Tor
    DISTRO=$(lsb_release -cs)
    echo "deb [signed-by=/usr/share/keyrings/tor-archive-keyring.gpg] https://deb.torproject.org/torproject.org $DISTRO main" > /etc/apt/sources.list.d/tor.list
    echo "deb-src [signed-by=/usr/share/keyrings/tor-archive-keyring.gpg] https://deb.torproject.org/torproject.org $DISTRO main" >> /etc/apt/sources.list.d/tor.list
    
    # Import Tor signing key
    wget -qO- https://deb.torproject.org/torproject.org/A3C4F0F979CAA22CDBA8F512EE8CBC9E886DDD89.asc | gpg --dearmor | tee /usr/share/keyrings/tor-archive-keyring.gpg >/dev/null
    
    apt-get update -qq
    apt-get install -y -qq tor deb.torproject.org-keyring haproxy nginx redis-server socat
    
    # Install vanguards for anti-sybil
    apt-get install -y -qq python3-pip python3-venv
    pip3 install vanguards --quiet --break-system-packages 2>/dev/null || pip3 install vanguards --quiet
    
    log_success "Services installed"
}

# =============================================================================
# CLONE AND BUILD PROJECT
# =============================================================================
clone_project() {
    log_info "Cloning Cerberus from GitHub..."
    
    # Remove old installation if exists
    rm -rf "$INSTALL_DIR"
    
    # Clone repository
    git clone --depth 1 "$GITHUB_REPO" "$INSTALL_DIR"
    
    if [[ -d "$INSTALL_DIR" ]]; then
        log_success "Repository cloned to $INSTALL_DIR"
    else
        log_error "Failed to clone repository"
        exit 1
    fi
}

build_fortify() {
    log_info "Building Fortify and vanity-onion (this may take 3-7 minutes)..."
    
    cd "$INSTALL_DIR"
    
    # Ensure cargo is available
    source "$HOME/.cargo/env" 2>/dev/null || true
    
    # Build release binaries (both fortify and vanity-onion)
    cargo build --release -p fortify -p vanity-onion 2>&1 | tail -20
    
    if [[ -f "$INSTALL_DIR/target/release/fortify" ]]; then
        cp "$INSTALL_DIR/target/release/fortify" "$FORTIFY_BIN"
        chmod +x "$FORTIFY_BIN"
        log_success "Fortify built and installed to $FORTIFY_BIN"
    else
        log_error "Fortify build failed"
        exit 1
    fi
    
    if [[ -f "$INSTALL_DIR/target/release/vanity-onion" ]]; then
        cp "$INSTALL_DIR/target/release/vanity-onion" /usr/local/bin/vanity-onion
        chmod +x /usr/local/bin/vanity-onion
        log_success "vanity-onion built and installed"
    else
        log_error "vanity-onion build failed"
        exit 1
    fi
}

# =============================================================================
# CREATE DIRECTORY STRUCTURE
# =============================================================================
create_directories() {
    log_info "Creating directory structure..."
    
    mkdir -p "$CERBERUS_ROOT"/{tor,haproxy,nginx,fortify}
    mkdir -p "$TOR_HS_DIR"
    mkdir -p /var/www/cerberus/static
    mkdir -p /var/lib/cerberus/ammo
    mkdir -p /var/log/cerberus
    
    log_success "Directories created"
}

# =============================================================================
# GENERATE VANITY ADDRESS
# =============================================================================
setup_vanity_keys() {
    log_info "Generating vanity address with prefix '$VANITY_PREFIX'..."
    
    # Use test mode for faster generation (3-char prefix)
    # In production, remove --test-mode for full prefix matching
    if /usr/local/bin/vanity-onion --prefix "$VANITY_PREFIX" --output "$TOR_HS_DIR" --test-mode --timeout 120; then
        if [[ -f "$TOR_HS_DIR/hostname" ]]; then
            GENERATED_ONION=$(cat "$TOR_HS_DIR/hostname")
            log_success "Generated vanity address: $GENERATED_ONION"
        else
            log_error "Vanity generation succeeded but no hostname file"
            exit 1
        fi
    else
        EXIT_CODE=$?
        if [[ $EXIT_CODE -eq 2 ]]; then
            log_warn "Vanity generation timed out - using random address"
            log_info "Tor will generate a random address on first start"
            log_info "For production, run vanity-onion manually with more time"
        else
            log_error "Vanity generation failed"
            exit 1
        fi
    fi
    
    # Set strict permissions (Tor requires this)
    chown -R debian-tor:debian-tor "$TOR_HS_DIR" 2>/dev/null || chown -R tor:tor "$TOR_HS_DIR"
    chmod 700 "$TOR_HS_DIR"
    chmod 600 "$TOR_HS_DIR"/* 2>/dev/null || true
    
    log_success "Vanity keys configured with correct permissions"
}

# =============================================================================
# CONFIGURE TOR
# =============================================================================
configure_tor() {
    log_info "Configuring Tor hidden service..."
    
    # IMPORTANT: Do NOT set SocksPort or ControlPort here!
    # The system defaults in /usr/share/tor/tor-service-defaults-torrc already
    # set SocksPort 9050 and we should not override with SocksPort 0 as it conflicts
    # and causes "Problem with User value" errors when combined with systemd service
    
    log_info "  [VERBOSE] Backing up existing torrc if present..."
    if [[ -f /etc/tor/torrc ]]; then
        cp /etc/tor/torrc /etc/tor/torrc.backup.$(date +%Y%m%d_%H%M%S)
        log_info "  [VERBOSE] Backed up existing torrc"
    fi
    
    log_info "  [VERBOSE] Writing new torrc configuration..."
    cat > /etc/tor/torrc << EOF
# ============================================================================
# Cerberus Tor Configuration
# Generated by cerberus-one-click.sh on $(date)
# ============================================================================
# IMPORTANT: SocksPort and ControlPort are managed by system defaults
# Do NOT add SocksPort 0 or ControlPort 0 - it conflicts with systemd service

# Logging - set to info for verbose output
Log info file /var/log/tor/notices.log
Log notice stdout

# Hidden Service Configuration
HiddenServiceDir $TOR_HS_DIR
HiddenServicePort 80 127.0.0.1:10000
HiddenServiceVersion 3

# Export Circuit ID to HAProxy (CRITICAL for tracking)
HiddenServiceExportCircuitID haproxy

# Native Tor PoW Defense (Requires Tor 0.4.8+)
HiddenServicePoWDefensesEnabled 1
HiddenServicePoWQueueRate 50
HiddenServicePoWQueueBurst 100
EOF

    log_info "  [VERBOSE] torrc written successfully"
    
    # Verify hidden service directory permissions
    log_info "  [VERBOSE] Verifying hidden service directory permissions..."
    if [[ ! -d "$TOR_HS_DIR" ]]; then
        log_info "  [VERBOSE] Creating hidden service directory: $TOR_HS_DIR"
        mkdir -p "$TOR_HS_DIR"
    fi
    
    # Set correct ownership (debian-tor for apt-installed Tor)
    log_info "  [VERBOSE] Setting ownership to debian-tor:debian-tor..."
    chown -R debian-tor:debian-tor "$TOR_HS_DIR" 2>/dev/null || {
        log_warn "debian-tor user not found, trying 'tor' user..."
        chown -R tor:tor "$TOR_HS_DIR"
    }
    
    # Set strict permissions (Tor REQUIRES 700 on the directory)
    log_info "  [VERBOSE] Setting directory permissions to 700..."
    chmod 700 "$TOR_HS_DIR"
    
    # Set file permissions
    if [[ -f "$TOR_HS_DIR/hostname" ]]; then
        chmod 600 "$TOR_HS_DIR/hostname"
        chmod 600 "$TOR_HS_DIR/hs_ed25519_secret_key" 2>/dev/null || true
        chmod 600 "$TOR_HS_DIR/hs_ed25519_public_key" 2>/dev/null || true
    fi
    
    log_info "  [VERBOSE] Verifying torrc syntax..."
    # NOTE: We must verify config as debian-tor user because:
    # 1. Tor refuses to run as root with HiddenServiceDir owned by another user
    # 2. The HS directory is owned by debian-tor (required by Tor)
    # 3. Running tor --verify-config as root will FAIL with permission errors
    log_info "  [VERBOSE] Running config verification as debian-tor user..."
    if ! sudo -u debian-tor tor --verify-config -f /etc/tor/torrc 2>&1; then
        log_error "Tor configuration verification FAILED!"
        log_error "This usually means a syntax error in /etc/tor/torrc"
        cat /etc/tor/torrc
        exit 1
    fi
    log_success "  [VERBOSE] Tor config verification passed"
    
    # Create /var/log/tor if it doesn't exist
    mkdir -p /var/log/tor
    chown debian-tor:debian-tor /var/log/tor 2>/dev/null || chown tor:tor /var/log/tor
    touch /var/log/tor/notices.log
    chown debian-tor:debian-tor /var/log/tor/notices.log 2>/dev/null || chown tor:tor /var/log/tor/notices.log
    
    log_success "Tor configured successfully"
}

# =============================================================================
# CONFIGURE HAPROXY
# =============================================================================
configure_haproxy() {
    log_info "Configuring HAProxy..."
    
    cat > "$HAPROXY_CFG" << 'EOF'
# ============================================================================
# Cerberus HAProxy Configuration
# Layer 4: Connection Governor
# ============================================================================

global
    log /dev/log local0
    log /dev/log local1 notice
    user haproxy
    group haproxy
    daemon
    
    # Performance
    maxconn 100000
    nbthread 4
    
    # Runtime API
    stats socket /var/run/haproxy.sock mode 660 level admin expose-fd listeners

defaults
    log     global
    mode    http
    option  httplog
    option  dontlognull
    
    # Aggressive Timeouts
    timeout connect 5s
    timeout client  10s
    timeout server  10s
    timeout http-request 3s

# Circuit Tracking Stick Table
backend be_stick_tables
    stick-table type string len 64 size 1m expire 30m store conn_cur,conn_rate(10s),http_req_rate(10s),gpc0

# Frontend: Tor Entry
frontend ft_tor_public
    bind 127.0.0.1:10000 accept-proxy
    
    http-request set-var(req.circuit_id) fc_pp_unique_id
    http-request track-sc0 var(req.circuit_id) table be_stick_tables
    
    # Ban check (gpc0 == 2)
    http-request deny deny_status 403 if { sc0_get_gpc0(be_stick_tables) eq 2 }
    
    # Rate limiting
    http-request deny deny_status 429 if { sc0_conn_cur(be_stick_tables) gt 10 }
    http-request deny deny_status 429 if { sc0_http_req_rate(be_stick_tables) gt 20 }
    
    # VIP bypass (gpc0 == 1)
    use_backend be_nginx_vip if { sc0_get_gpc0(be_stick_tables) eq 1 }
    
    default_backend be_nginx_public

backend be_nginx_public
    server nginx_local 127.0.0.1:10001 maxconn 5000
    http-request set-header X-Circuit-ID %[var(req.circuit_id)]

backend be_nginx_vip
    server nginx_local 127.0.0.1:10001 maxconn 10000
    http-request set-header X-Circuit-ID %[var(req.circuit_id)]
    http-request set-header X-VIP "1"

listen stats
    bind 127.0.0.1:8404
    mode http
    stats enable
    stats uri /
    stats refresh 5s
    stats admin if TRUE
EOF

    # Create error pages
    mkdir -p /etc/haproxy/errors
    echo -e "HTTP/1.0 429 Too Many Requests\r\nContent-Type: text/html\r\n\r\n<h1>429 - Slow Down</h1>" > /etc/haproxy/errors/429.http
    echo -e "HTTP/1.0 403 Forbidden\r\nContent-Type: text/html\r\n\r\n<h1>403 - Access Denied</h1>" > /etc/haproxy/errors/403.http
    
    log_success "HAProxy configured"
}

# =============================================================================
# CONFIGURE NGINX
# =============================================================================
configure_nginx() {
    log_info "Configuring Nginx..."
    
    cat > /etc/nginx/nginx.conf << 'EOF'
user www-data;
worker_processes auto;
pid /run/nginx.pid;

events {
    worker_connections 2048;
    use epoll;
    multi_accept on;
}

http {
    sendfile on;
    tcp_nopush on;
    tcp_nodelay on;
    keepalive_timeout 5s;
    server_tokens off;
    
    client_body_buffer_size 16k;
    client_header_buffer_size 1k;
    client_max_body_size 1m;
    large_client_header_buffers 2 1k;
    client_body_timeout 5s;
    client_header_timeout 5s;
    
    include /etc/nginx/mime.types;
    default_type application/octet-stream;
    
    access_log /var/log/nginx/access.log;
    error_log /var/log/nginx/error.log;
    
    include /etc/nginx/sites-enabled/*;
}
EOF

    cat > "$NGINX_SITE" << EOF
# Cerberus Nginx Configuration
server {
    listen 127.0.0.1:10001;
    server_name _;
    
    root /var/www/cerberus/static;
    
    # Header Scrubbing
    proxy_set_header User-Agent "Mozilla/5.0 (Windows NT 10.0; rv:115.0) Gecko/20100101 Firefox/115.0";
    proxy_set_header Accept-Language "en-US,en;q=0.5";
    proxy_set_header Via "";
    proxy_set_header X-Forwarded-For "";
    
    # Security Headers
    add_header X-Frame-Options "DENY" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header Referrer-Policy "no-referrer" always;
    
    # CAPTCHA Gate
    location = / {
        proxy_pass http://127.0.0.1:$FORTIFY_PORT;
        proxy_set_header X-Circuit-ID \$http_x_circuit_id;
    }
    
    location = /captcha.html {
        proxy_pass http://127.0.0.1:$FORTIFY_PORT;
    }
    
    location = /challenge {
        proxy_pass http://127.0.0.1:$FORTIFY_PORT;
        proxy_set_header X-Circuit-ID \$http_x_circuit_id;
    }
    
    location = /verify {
        limit_except POST { deny all; }
        proxy_pass http://127.0.0.1:$FORTIFY_PORT;
        proxy_set_header X-Circuit-ID \$http_x_circuit_id;
        proxy_connect_timeout 1s;
        proxy_read_timeout 2s;
    }
    
    # Passport validation for auth_request
    location = /validate {
        internal;
        proxy_pass http://127.0.0.1:$FORTIFY_PORT/validate?token=\$arg_passport_token&circuit_id=\$http_x_circuit_id;
        proxy_pass_request_body off;
        proxy_set_header Content-Length "";
        proxy_set_header X-Original-URI \$request_uri;
    }
    
    # Protected App - requires valid passport token
    # When passport is valid, user sees this content
    location /app/ {
        auth_request /validate;
        
        # Success: Return protected content
        # The backend onion would be accessed here if we had a proper backend proxy
        # For now, return a success message indicating access was granted
        default_type text/html;
        return 200 '<!DOCTYPE html><html><head><title>Cerberus - Protected Area</title></head><body style="background:#1a1a2e;color:#e0e0e0;font-family:sans-serif;display:flex;align-items:center;justify-content:center;height:100vh;margin:0;"><div style="text-align:center;"><h1 style="color:#4ecdc4;">Access Granted</h1><p>Your passport token is valid.</p><p>Backend: $BACKEND_ONION</p></div></body></html>';
    }
    
    location /health {
        proxy_pass http://127.0.0.1:$FORTIFY_PORT;
    }
}
EOF

    ln -sf "$NGINX_SITE" /etc/nginx/sites-enabled/cerberus
    rm -f /etc/nginx/sites-enabled/default 2>/dev/null || true
    
    log_success "Nginx configured"
}

# =============================================================================
# CONFIGURE FORTIFY
# =============================================================================
configure_fortify() {
    log_info "Configuring Fortify..."
    
    mkdir -p "$CERBERUS_ROOT/fortify"
    
    cat > "$CERBERUS_ROOT/fortify/config.toml" << EOF
# Cerberus Fortify Configuration

listen_addr = "127.0.0.1:$FORTIFY_PORT"
redis_url = "redis://127.0.0.1:6379"
node_id = "$(hostname)"
initial_threat_level = 1

[backend]
target = "$BACKEND_ONION"
vanity = "$VANITY_PREFIX"

[captcha]
challenge_ttl_secs = 300
passport_ttl_secs = 300

[rate_limit]
max_failed_attempts = 5
soft_lock_duration_secs = 300
ban_duration_secs = 3600

[ammo_box]
pool_size = 1000
disk_cache_path = "/var/lib/cerberus/ammo"
refill_threshold = 0.5
EOF

    # Create systemd service
    cat > "$SYSTEMD_DIR/fortify.service" << EOF
[Unit]
Description=Cerberus Fortify - L7+ Logic Engine
After=network.target redis.service
Wants=redis.service

[Service]
Type=simple
ExecStart=$FORTIFY_BIN --config $CERBERUS_ROOT/fortify/config.toml
Restart=always
RestartSec=5
User=www-data
Group=www-data
Environment="RUST_LOG=info"

# Security Hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
ReadWritePaths=/var/lib/cerberus /var/log/cerberus

[Install]
WantedBy=multi-user.target
EOF

    # Set permissions
    chown -R www-data:www-data /var/lib/cerberus
    chown -R www-data:www-data /var/log/cerberus
    
    systemctl daemon-reload
    
    log_success "Fortify configured"
}

# =============================================================================
# KERNEL TUNING
# =============================================================================
apply_kernel_tuning() {
    log_info "Applying kernel tuning..."
    
    MEM_KB=$(grep MemTotal /proc/meminfo | awk '{print $2}')
    MEM_MB=$((MEM_KB / 1024))
    
    MAX_CONN=$(( 4096 + (MEM_MB / 256) * 1024 ))
    if [ $MAX_CONN -gt 262144 ]; then MAX_CONN=262144; fi
    
    SYN_BACKLOG=$(( 1024 + (MEM_MB / 128) * 512 ))
    if [ $SYN_BACKLOG -gt 65535 ]; then SYN_BACKLOG=65535; fi

    cat > /etc/sysctl.d/99-cerberus.conf << EOF
# Cerberus Kernel Tuning

vm.swappiness = 10
vm.overcommit_memory = 1

net.ipv4.tcp_syncookies = 1
net.ipv4.tcp_synack_retries = 2
net.ipv4.tcp_max_syn_backlog = ${SYN_BACKLOG}
net.core.somaxconn = ${MAX_CONN}
net.core.netdev_max_backlog = ${MAX_CONN}

net.ipv4.tcp_fin_timeout = 15
net.ipv4.tcp_keepalive_time = 60
net.ipv4.tcp_keepalive_probes = 3
net.ipv4.tcp_keepalive_intvl = 10
net.ipv4.tcp_tw_reuse = 1
net.ipv4.tcp_max_tw_buckets = $(( MAX_CONN * 2 ))

fs.file-max = $(( MAX_CONN * 4 ))
net.ipv4.ip_local_port_range = 1024 65535
EOF

    sysctl --system > /dev/null 2>&1
    log_success "Kernel tuning applied"
}

# =============================================================================
# START ALL SERVICES
# =============================================================================
start_services() {
    log_info "Starting services with verbose logging..."
    
    # Start Redis first
    log_info "  [1/5] Starting Redis..."
    if systemctl enable --now redis-server 2>/dev/null || systemctl enable --now redis 2>/dev/null; then
        log_success "  Redis started successfully"
    else
        log_error "  Redis failed to start!"
        journalctl -u redis-server -n 10 --no-pager 2>/dev/null || journalctl -u redis -n 10 --no-pager
        exit 1
    fi
    
    # Start Tor with extensive error checking
    log_info "  [2/5] Starting Tor service..."
    log_info "  [VERBOSE] Enabling tor service..."
    systemctl enable tor
    
    log_info "  [VERBOSE] Starting tor service..."
    if systemctl start tor; then
        log_success "  Tor started successfully"
    else
        log_error "  Tor FAILED to start!"
        log_error "  [VERBOSE] Dumping tor service status:"
        systemctl status tor --no-pager -l
        log_error "  [VERBOSE] Dumping last 30 lines of tor journal:"
        journalctl -u tor -n 30 --no-pager
        log_error "  [VERBOSE] Checking torrc:"
        cat /etc/tor/torrc
        log_error "  [VERBOSE] Checking hidden service directory:"
        ls -la "$TOR_HS_DIR" 2>/dev/null || echo "Directory does not exist!"
        log_error "  [VERBOSE] Checking tor defaults file:"
        cat /usr/share/tor/tor-service-defaults-torrc 2>/dev/null | head -20
        exit 1
    fi
    
    # Wait a moment for Tor to initialize
    log_info "  [VERBOSE] Waiting for Tor to initialize (5 seconds)..."
    sleep 5
    
    # Check if Tor is actually running
    if ! systemctl is-active --quiet tor; then
        log_error "  Tor stopped unexpectedly after start!"
        journalctl -u tor -n 20 --no-pager
        exit 1
    fi
    log_info "  [VERBOSE] Tor is actively running"
    
    # Start HAProxy
    log_info "  [3/5] Starting HAProxy..."
    if systemctl enable --now haproxy; then
        log_success "  HAProxy started successfully"
    else
        log_error "  HAProxy failed to start!"
        journalctl -u haproxy -n 10 --no-pager
        exit 1
    fi
    
    # Start Nginx
    log_info "  [4/5] Starting Nginx..."
    if systemctl enable --now nginx; then
        log_success "  Nginx started successfully"
    else
        log_error "  Nginx failed to start!"
        journalctl -u nginx -n 10 --no-pager
        nginx -t
        exit 1
    fi
    
    # Start Fortify
    log_info "  [5/5] Starting Fortify..."
    if systemctl enable --now fortify; then
        log_success "  Fortify started successfully"
    else
        log_error "  Fortify failed to start!"
        journalctl -u fortify -n 10 --no-pager
        exit 1
    fi
    
    log_success "All services started successfully"
    
    # CRITICAL: Verify HAProxy is actually listening on ports
    # Sometimes HAProxy starts but doesn't bind - restart it to be sure
    log_info "  [5.5/5] Verifying HAProxy port bindings..."
    sleep 2
    if ! ss -tlnp 2>/dev/null | grep -q ":10000 "; then
        log_warn "  HAProxy not listening on 10000 - restarting..."
        systemctl restart haproxy
        sleep 2
        if ss -tlnp 2>/dev/null | grep -q ":10000 "; then
            log_success "  HAProxy now listening on port 10000"
        else
            log_error "  HAProxy still not listening after restart!"
            systemctl status haproxy --no-pager
            exit 1
        fi
    else
        log_success "  HAProxy verified listening on port 10000"
    fi
    
    # Copy hostname to dashboard-readable location
    log_info "  [VERBOSE] Copying hostname for dashboard access..."
    mkdir -p /etc/cerberus
    sleep 3  # Give Tor time to write hostname
    if [[ -f "$TOR_HS_DIR/hostname" ]]; then
        cp "$TOR_HS_DIR/hostname" /etc/cerberus/onion_hostname
        chmod 644 /etc/cerberus/onion_hostname
        log_success "  Hostname copied to /etc/cerberus/onion_hostname for dashboard"
    else
        log_warn "  Hostname not yet generated, will copy after Tor fully starts"
    fi
}

# =============================================================================
# VERIFY DEPLOYMENT
# =============================================================================
verify_deployment() {
    log_info "Verifying deployment with detailed checks..."
    
    sleep 5
    
    echo ""
    FAILED=0
    
    log_info "  [VERBOSE] Checking service status..."
    for svc in redis-server tor haproxy nginx fortify; do
        if systemctl is-active --quiet "$svc" 2>/dev/null; then
            log_success "$svc is running"
        elif systemctl is-active --quiet "${svc%%-server}" 2>/dev/null; then
            log_success "${svc%%-server} is running"
        else
            log_error "$svc is NOT running"
            log_error "  [VERBOSE] Checking $svc status:"
            systemctl status "$svc" --no-pager -l 2>/dev/null | head -20
            FAILED=1
        fi
    done
    
    # Check ports are listening
    log_info "  [VERBOSE] Checking listening ports..."
    log_info "  Expected: Redis:6379, HAProxy:10000/8404, Nginx:10001, Fortify:8888"
    ss -tlnp 2>/dev/null | grep -E "(6379|10000|10001|8888|8404)" | while read line; do
        log_info "    $line"
    done
    
    # Wait for Tor to publish (it takes time)
    log_info "Waiting for Tor to generate/load hidden service address (15 seconds)..."
    for i in {1..15}; do
        if [[ -f "$TOR_HS_DIR/hostname" ]]; then
            break
        fi
        sleep 1
        echo -n "."
    done
    echo ""
    
    # Final check and copy of hostname
    if [[ -f "$TOR_HS_DIR/hostname" ]]; then
        ONION_ADDR=$(cat "$TOR_HS_DIR/hostname")
        log_success "Onion address ready: $ONION_ADDR"
        
        # Ensure dashboard can read it
        mkdir -p /etc/cerberus
        cp "$TOR_HS_DIR/hostname" /etc/cerberus/onion_hostname
        chmod 644 /etc/cerberus/onion_hostname
        log_success "Hostname exported to /etc/cerberus/onion_hostname"
    else
        log_warn "Onion address not yet generated - Tor may still be starting"
        log_info "  [VERBOSE] Checking Tor logs for issues..."
        journalctl -u tor -n 10 --no-pager
        ONION_ADDR="(pending...)"
    fi
    
    # Display results
    echo ""
    echo "════════════════════════════════════════════════════════════════"
    if [[ $FAILED -eq 0 ]]; then
        echo -e "${GREEN}        CERBERUS DEPLOYMENT SUCCESSFUL!${NC}"
    else
        echo -e "${YELLOW}        CERBERUS DEPLOYMENT COMPLETED (with warnings)${NC}"
    fi
    echo "════════════════════════════════════════════════════════════════"
    echo ""
    
    echo -e "  Your Onion Address: ${GREEN}${ONION_ADDR}${NC}"
    
    echo ""
    echo "  Backend Target:     $BACKEND_ONION"
    echo ""
    echo "════════════════════════════════════════════════════════════════"
    echo "  LOCAL TESTING:"
    echo "    curl http://127.0.0.1:10001/           # CAPTCHA page"
    echo "    curl http://127.0.0.1:10001/health     # Health check"
    echo "    http://127.0.0.1:8404/                 # HAProxy stats"
    echo ""
    echo "  LOGS:"
    echo "    journalctl -u fortify -f              # Fortify logs"
    echo "    journalctl -u tor -f                  # Tor logs"
    echo "    tail -f /var/log/nginx/access.log    # Nginx access"
    echo ""
    echo "  MANAGEMENT:"
    echo "    systemctl status fortify haproxy nginx tor"
    echo "    systemctl restart fortify"
    echo ""
    echo "  DASHBOARD:"
    echo "    python3 $INSTALL_DIR/deploy/dashboard_server.py"
    echo "    Then open: http://127.0.0.1:$DASHBOARD_PORT/"
    echo ""
    echo "  NOTE: Tor needs 1-5 minutes to publish the onion address"
    echo "════════════════════════════════════════════════════════════════"
    echo ""
}

# =============================================================================
# LAUNCH DASHBOARD (IF DISPLAY AVAILABLE)
# =============================================================================
launch_dashboard() {
    if [[ "$HAS_DISPLAY" != true ]]; then
        log_info "No display - skipping dashboard launch"
        log_info "To view dashboard later, run:"
        echo "    python3 $INSTALL_DIR/deploy/dashboard_server.py &"
        echo "    xdg-open http://127.0.0.1:$DASHBOARD_PORT/"
        return
    fi
    
    log_info "Launching deployment dashboard..."
    
    # Start the dashboard server in background
    nohup python3 "$INSTALL_DIR/deploy/dashboard_server.py" > /var/log/cerberus/dashboard.log 2>&1 &
    DASHBOARD_PID=$!
    
    # Wait for server to start
    sleep 2
    
    # Check if server is running
    if kill -0 $DASHBOARD_PID 2>/dev/null; then
        log_success "Dashboard server started (PID: $DASHBOARD_PID)"
        
        # Try to open browser
        DASHBOARD_URL="http://127.0.0.1:$DASHBOARD_PORT/"
        
        if command -v xdg-open &> /dev/null; then
            xdg-open "$DASHBOARD_URL" 2>/dev/null &
            log_success "Opened dashboard in browser: $DASHBOARD_URL"
        elif command -v gnome-open &> /dev/null; then
            gnome-open "$DASHBOARD_URL" 2>/dev/null &
            log_success "Opened dashboard in browser: $DASHBOARD_URL"
        elif command -v firefox &> /dev/null; then
            firefox "$DASHBOARD_URL" 2>/dev/null &
            log_success "Opened dashboard in Firefox: $DASHBOARD_URL"
        elif command -v chromium-browser &> /dev/null; then
            chromium-browser "$DASHBOARD_URL" 2>/dev/null &
            log_success "Opened dashboard in Chromium: $DASHBOARD_URL"
        else
            log_warn "Could not detect browser. Open manually: $DASHBOARD_URL"
        fi
        
        # Create systemd service for dashboard
        cat > "$SYSTEMD_DIR/cerberus-dashboard.service" << EOF
[Unit]
Description=Cerberus Deployment Dashboard
After=network.target

[Service]
Type=simple
ExecStart=/usr/bin/python3 $INSTALL_DIR/deploy/dashboard_server.py
Restart=on-failure
RestartSec=5
User=root
WorkingDirectory=$INSTALL_DIR/deploy

[Install]
WantedBy=multi-user.target
EOF
        systemctl daemon-reload
        log_success "Dashboard service installed (cerberus-dashboard.service)"
    else
        log_warn "Dashboard server failed to start"
    fi
}

# =============================================================================
# MAIN
# =============================================================================
main() {
    print_banner
    
    check_root
    check_os
    preflight_checks
    detect_display
    prepare_system
    install_base_deps
    install_rust
    install_services
    clone_project
    build_fortify
    create_directories
    setup_vanity_keys
    configure_tor
    configure_haproxy
    configure_nginx
    configure_fortify
    apply_kernel_tuning
    start_services
    verify_deployment
    launch_dashboard
    
    log_success "Cerberus deployment complete!"
    log_info "To access from Tor Browser, use your .onion address above"
    log_info "Dashboard running at: http://127.0.0.1:$DASHBOARD_PORT/"
}

main "$@"
