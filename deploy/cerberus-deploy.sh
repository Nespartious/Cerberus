#!/bin/bash
# ============================================================================
# Cerberus Deployment Script
# ============================================================================
# Deploys a Tor hidden service with HAProxy and Nginx, using pre-generated
# vanity .onion keys.
#
# Usage:
#   ./cerberus-deploy.sh [OPTIONS]
#
# Options:
#   --keys-dir DIR      Path to vanity keys (default: ./keys/sigil-mirror)
#   --backend URL       Backend target URL (IP:PORT or onion:PORT)
#   --dry-run           Print actions without executing
#   --uninstall         Remove Cerberus components
#
# Requirements:
#   - Debian/Ubuntu or Alpine Linux
#   - Root privileges
#   - Pre-generated vanity keys (from vanity-onion tool)
# ============================================================================

set -euo pipefail
IFS=$'\n\t'

# --- Defaults ---
KEYS_DIR="${KEYS_DIR:-./keys/sigil-mirror}"
BACKEND="${BACKEND:-127.0.0.1:8080}"
FORTIFY_PORT="${FORTIFY_PORT:-8888}"
DRY_RUN=false
UNINSTALL=false

# Cerberus install paths
CERBERUS_ROOT="/etc/cerberus"
TOR_HS_DIR="/var/lib/tor/cerberus"
HAPROXY_CFG="/etc/haproxy/haproxy.cfg"
NGINX_SITE="/etc/nginx/sites-available/cerberus"
FORTIFY_BIN="/usr/local/bin/fortify"
SYSTEMD_DIR="/etc/systemd/system"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# --- Parse Arguments ---
parse_args() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            --keys-dir)
                KEYS_DIR="$2"
                shift 2
                ;;
            --backend)
                BACKEND="$2"
                shift 2
                ;;
            --dry-run)
                DRY_RUN=true
                shift
                ;;
            --uninstall)
                UNINSTALL=true
                shift
                ;;
            -h|--help)
                echo "Usage: $0 [--keys-dir DIR] [--backend URL] [--dry-run] [--uninstall]"
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                exit 1
                ;;
        esac
    done
}

# --- Check Prerequisites ---
check_prereqs() {
    log_info "Checking prerequisites..."

    if [[ $EUID -ne 0 ]]; then
        log_error "This script must be run as root"
        exit 1
    fi

    # Detect OS
    if [[ -f /etc/debian_version ]]; then
        OS="debian"
        PKG_MANAGER="apt-get"
    elif [[ -f /etc/alpine-release ]]; then
        OS="alpine"
        PKG_MANAGER="apk"
    else
        log_error "Unsupported OS. Use Debian/Ubuntu or Alpine."
        exit 1
    fi

    log_success "Detected OS: $OS"

    # Check keys exist
    if [[ ! -d "$KEYS_DIR" ]]; then
        log_error "Keys directory not found: $KEYS_DIR"
        log_info "Generate keys with: vanity-onion --prefix <prefix> --output $KEYS_DIR"
        exit 1
    fi

    if [[ ! -f "$KEYS_DIR/hostname" ]]; then
        log_error "Missing hostname file in $KEYS_DIR"
        exit 1
    fi

    ONION_ADDR=$(cat "$KEYS_DIR/hostname")
    log_success "Vanity address: $ONION_ADDR"
}

# --- Install Dependencies ---
install_deps() {
    log_info "Installing dependencies..."

    if [[ "$DRY_RUN" == true ]]; then
        log_warn "[DRY-RUN] Would install: tor haproxy nginx redis-server"
        return
    fi

    if [[ "$OS" == "debian" ]]; then
        apt-get update -qq
        apt-get install -y -qq tor haproxy nginx redis-server socat curl

        # Install vanguards for anti-sybil protection
        apt-get install -y -qq python3-pip
        pip3 install vanguards --quiet
    elif [[ "$OS" == "alpine" ]]; then
        apk update
        apk add tor haproxy nginx redis socat curl py3-pip
        pip3 install vanguards
    fi

    log_success "Dependencies installed"
}

# --- Create Directory Structure ---
create_dirs() {
    log_info "Creating directory structure..."

    if [[ "$DRY_RUN" == true ]]; then
        log_warn "[DRY-RUN] Would create: $CERBERUS_ROOT, $TOR_HS_DIR"
        return
    fi

    mkdir -p "$CERBERUS_ROOT"/{tor,haproxy,nginx,fortify}
    mkdir -p "$TOR_HS_DIR"
    mkdir -p /var/www/cerberus/static
    mkdir -p /var/lib/cerberus/ammo
    mkdir -p /var/log/cerberus

    log_success "Directories created"
}

# --- Configure Tor Hidden Service ---
configure_tor() {
    log_info "Configuring Tor hidden service..."

    if [[ "$DRY_RUN" == true ]]; then
        log_warn "[DRY-RUN] Would configure Tor with vanity keys"
        return
    fi

    # Copy vanity keys
    cp "$KEYS_DIR/hs_ed25519_secret_key" "$TOR_HS_DIR/"
    cp "$KEYS_DIR/hs_ed25519_public_key" "$TOR_HS_DIR/"
    cp "$KEYS_DIR/hostname" "$TOR_HS_DIR/"

    # Set permissions (Tor is very strict about this)
    chown -R debian-tor:debian-tor "$TOR_HS_DIR" 2>/dev/null || \
    chown -R tor:tor "$TOR_HS_DIR"
    chmod 700 "$TOR_HS_DIR"
    chmod 600 "$TOR_HS_DIR"/*

    # Generate torrc
    cat > /etc/tor/torrc << EOF
# Cerberus Tor Configuration
# Generated by cerberus-deploy.sh

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

# Security Hardening
SocksPort 0
ControlPort 0
Log notice file /var/log/tor/notices.log

# Vanguards will manage additional protections
EOF

    log_success "Tor configured with vanity address: $(cat $TOR_HS_DIR/hostname)"
}

# --- Configure HAProxy ---
configure_haproxy() {
    log_info "Configuring HAProxy..."

    if [[ "$DRY_RUN" == true ]]; then
        log_warn "[DRY-RUN] Would configure HAProxy"
        return
    fi

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
    
    # Performance Limits
    maxconn 100000
    nbthread 4
    
    # Runtime API (for Fortify integration)
    stats socket /var/run/haproxy.sock mode 660 level admin expose-fd listeners

defaults
    log     global
    mode    http
    option  httplog
    option  dontlognull
    
    # Aggressive Timeouts (Slowloris Defense)
    timeout connect 5s
    timeout client  10s
    timeout server  10s
    timeout http-request 3s
    
    # Error handling
    errorfile 400 /etc/haproxy/errors/400.http
    errorfile 403 /etc/haproxy/errors/403.http
    errorfile 408 /etc/haproxy/errors/408.http
    errorfile 429 /etc/haproxy/errors/429.http
    errorfile 500 /etc/haproxy/errors/500.http
    errorfile 502 /etc/haproxy/errors/502.http
    errorfile 503 /etc/haproxy/errors/503.http
    errorfile 504 /etc/haproxy/errors/504.http

# --- Stick Table: Circuit Tracking ---
# Tracks Tor Circuit IDs
# gpc0: 0=Normal, 1=VIP, 2=Banned
backend be_stick_tables
    stick-table type string len 64 size 1m expire 30m store conn_cur,conn_rate(10s),http_req_rate(10s),gpc0

# --- Frontend: Tor Entry (Port 10000) ---
frontend ft_tor_public
    bind 127.0.0.1:10000 accept-proxy
    
    # Extract Circuit ID from PROXY v2 header
    http-request set-var(req.circuit_id) fc_pp_unique_id
    
    # Track in Stick Table
    http-request track-sc0 var(req.circuit_id) table be_stick_tables
    
    # Security Checks
    
    # 1. Ban Check (gpc0 == 2)
    http-request deny deny_status 403 if { sc0_get_gpc0(be_stick_tables) eq 2 }
    
    # 2. Rate Limiting (Strict for unverified)
    # Max 10 concurrent conns, Max 20 req/10s
    http-request deny deny_status 429 if { sc0_conn_cur(be_stick_tables) gt 10 }
    http-request deny deny_status 429 if { sc0_http_req_rate(be_stick_tables) gt 20 }
    
    # 3. Routing
    # VIPs bypass strict limits (gpc0 == 1)
    use_backend be_nginx_vip if { sc0_get_gpc0(be_stick_tables) eq 1 }
    
    # Normal users go to standard backend
    default_backend be_nginx_public

# --- Backends ---
backend be_nginx_public
    server nginx_local 127.0.0.1:10001 maxconn 5000
    
    # Pass circuit ID to Nginx/Fortify
    http-request set-header X-Circuit-ID %[var(req.circuit_id)]

backend be_nginx_vip
    server nginx_local 127.0.0.1:10001 maxconn 10000
    
    # Mark as VIP for Fortify
    http-request set-header X-Circuit-ID %[var(req.circuit_id)]
    http-request set-header X-VIP "1"

# --- Stats (Admin Only) ---
listen stats
    bind 127.0.0.1:8404
    mode http
    stats enable
    stats uri /
    stats refresh 5s
    stats admin if TRUE
EOF

    # Create error pages directory
    mkdir -p /etc/haproxy/errors
    echo "HTTP/1.0 429 Too Many Requests\r\nContent-Type: text/html\r\n\r\n<h1>429 - Slow Down</h1>" > /etc/haproxy/errors/429.http
    echo "HTTP/1.0 403 Forbidden\r\nContent-Type: text/html\r\n\r\n<h1>403 - Access Denied</h1>" > /etc/haproxy/errors/403.http

    log_success "HAProxy configured"
}

# --- Configure Nginx ---
configure_nginx() {
    log_info "Configuring Nginx..."

    if [[ "$DRY_RUN" == true ]]; then
        log_warn "[DRY-RUN] Would configure Nginx"
        return
    fi

    # Main nginx.conf
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
    # Basic Settings
    sendfile on;
    tcp_nopush on;
    tcp_nodelay on;
    keepalive_timeout 5s;
    server_tokens off;
    
    # Buffer Hardening (Anti-DoS)
    client_body_buffer_size 16k;
    client_header_buffer_size 1k;
    client_max_body_size 1m;
    large_client_header_buffers 2 1k;
    
    # Timeouts (Kill slowloris)
    client_body_timeout 5s;
    client_header_timeout 5s;
    
    # MIME Types
    include /etc/nginx/mime.types;
    default_type application/octet-stream;
    
    # Logging
    access_log /var/log/nginx/access.log;
    error_log /var/log/nginx/error.log;
    
    include /etc/nginx/sites-enabled/*;
}
EOF

    # Site configuration
    cat > "$NGINX_SITE" << 'EOF'
# Cerberus Nginx Site Configuration
# Layer 7: Gatekeeper

server {
    listen 127.0.0.1:10001;
    server_name _;
    
    root /var/www/cerberus/static;
    
    # --- Header Scrubbing (Privacy) ---
    # Normalize all client headers for anonymity
    proxy_set_header User-Agent "Mozilla/5.0 (Windows NT 10.0; rv:115.0) Gecko/20100101 Firefox/115.0";
    proxy_set_header Accept-Language "en-US,en;q=0.5";
    proxy_set_header Accept-Encoding "gzip, deflate";
    proxy_set_header Via "";
    proxy_set_header X-Forwarded-For "";
    
    # --- Security Headers ---
    add_header X-Frame-Options "DENY" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header Content-Security-Policy "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:;" always;
    add_header Referrer-Policy "no-referrer" always;
    
    # --- Route: Static CAPTCHA Gate ---
    location = / {
        proxy_pass http://127.0.0.1:8888;
        proxy_set_header X-Circuit-ID $http_x_circuit_id;
    }
    
    location = /captcha.html {
        proxy_pass http://127.0.0.1:8888;
    }
    
    # --- Route: CAPTCHA Challenge API ---
    location = /challenge {
        proxy_pass http://127.0.0.1:8888;
        proxy_set_header X-Circuit-ID $http_x_circuit_id;
    }
    
    # --- Route: CAPTCHA Verification ---
    location = /verify {
        limit_except POST { deny all; }
        
        proxy_pass http://127.0.0.1:8888;
        proxy_set_header X-Circuit-ID $http_x_circuit_id;
        
        # Backpressure
        proxy_connect_timeout 1s;
        proxy_read_timeout 2s;
    }
    
    # --- Route: Passport Validation (Internal) ---
    location = /validate {
        internal;
        proxy_pass http://127.0.0.1:8888;
    }
    
    # --- Route: Protected App (Requires Valid Passport) ---
    location /app/ {
        # Validate passport via subrequest
        auth_request /validate;
        auth_request_set $auth_status $upstream_status;
        
        # If valid, proxy to backend
        proxy_pass http://BACKEND_PLACEHOLDER;
        proxy_set_header X-Circuit-ID $http_x_circuit_id;
    }
    
    # --- Health Check ---
    location /health {
        proxy_pass http://127.0.0.1:8888;
    }
}
EOF

    # Replace backend placeholder
    sed -i "s|BACKEND_PLACEHOLDER|$BACKEND|g" "$NGINX_SITE"

    # Enable site
    ln -sf "$NGINX_SITE" /etc/nginx/sites-enabled/cerberus
    rm -f /etc/nginx/sites-enabled/default 2>/dev/null || true

    log_success "Nginx configured"
}

# --- Install Fortify Binary ---
install_fortify() {
    log_info "Installing Fortify binary..."

    if [[ "$DRY_RUN" == true ]]; then
        log_warn "[DRY-RUN] Would install Fortify binary"
        return
    fi

    # Check if binary exists locally
    if [[ -f "./target/release/fortify" ]]; then
        cp ./target/release/fortify "$FORTIFY_BIN"
        chmod +x "$FORTIFY_BIN"
        log_success "Fortify installed from local build"
    else
        log_warn "Fortify binary not found. Build with: cargo build --release -p fortify"
        log_warn "Skipping Fortify installation..."
        return
    fi

    # Create config
    mkdir -p "$CERBERUS_ROOT/fortify"
    cat > "$CERBERUS_ROOT/fortify/config.toml" << EOF
# Cerberus Fortify Configuration

listen_addr = "127.0.0.1:$FORTIFY_PORT"
redis_url = "redis://127.0.0.1:6379"
node_id = "$(hostname)"
initial_threat_level = 1

[backend]
target = "$BACKEND"
vanity = "$(cat $TOR_HS_DIR/hostname | cut -c1-5)"

[captcha]
challenge_ttl_secs = 300
passport_ttl_secs = 300

[rate_limit]
max_failed_attempts = 5
soft_lock_duration_secs = 300
ban_duration_secs = 3600
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

# Security Hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
ReadWritePaths=/var/lib/cerberus

[Install]
WantedBy=multi-user.target
EOF

    systemctl daemon-reload
    log_success "Fortify service installed"
}

# --- Apply Kernel Tuning ---
apply_sysctl() {
    log_info "Applying kernel tuning..."

    if [[ "$DRY_RUN" == true ]]; then
        log_warn "[DRY-RUN] Would apply sysctl settings"
        return
    fi

    MEM_KB=$(grep MemTotal /proc/meminfo | awk '{print $2}')
    MEM_MB=$((MEM_KB / 1024))
    
    MAX_CONN=$(( 4096 + (MEM_MB / 256) * 1024 ))
    if [ $MAX_CONN -gt 262144 ]; then MAX_CONN=262144; fi
    
    SYN_BACKLOG=$(( 1024 + (MEM_MB / 128) * 512 ))
    if [ $SYN_BACKLOG -gt 65535 ]; then SYN_BACKLOG=65535; fi

    cat > /etc/sysctl.d/99-cerberus.conf << EOF
# Cerberus Kernel Tuning
# Generated for ${MEM_MB}MB system

# Memory Protection
vm.swappiness = 10
vm.overcommit_memory = 1

# Network Hardening
net.ipv4.tcp_syncookies = 1
net.ipv4.tcp_synack_retries = 2
net.ipv4.tcp_max_syn_backlog = ${SYN_BACKLOG}
net.core.somaxconn = ${MAX_CONN}
net.core.netdev_max_backlog = ${MAX_CONN}

# Resource Recycling
net.ipv4.tcp_fin_timeout = 15
net.ipv4.tcp_keepalive_time = 60
net.ipv4.tcp_keepalive_probes = 3
net.ipv4.tcp_keepalive_intvl = 10
net.ipv4.tcp_tw_reuse = 1
net.ipv4.tcp_max_tw_buckets = $(( MAX_CONN * 2 ))

# Connection Limits
fs.file-max = $(( MAX_CONN * 4 ))
net.ipv4.ip_local_port_range = 1024 65535
EOF

    sysctl --system > /dev/null 2>&1
    log_success "Kernel tuning applied"
}

# --- Start Services ---
start_services() {
    log_info "Starting services..."

    if [[ "$DRY_RUN" == true ]]; then
        log_warn "[DRY-RUN] Would start: tor, redis, haproxy, nginx, fortify"
        return
    fi

    systemctl enable --now redis-server || systemctl enable --now redis
    systemctl enable --now tor
    systemctl enable --now haproxy
    systemctl enable --now nginx

    if [[ -f "$FORTIFY_BIN" ]]; then
        systemctl enable --now fortify
    fi

    log_success "Services started"
}

# --- Verify Deployment ---
verify() {
    log_info "Verifying deployment..."

    if [[ "$DRY_RUN" == true ]]; then
        log_warn "[DRY-RUN] Would verify deployment"
        return
    fi

    sleep 5

    # Check services
    for svc in tor redis-server haproxy nginx; do
        if systemctl is-active --quiet "$svc" 2>/dev/null || \
           systemctl is-active --quiet "${svc%%-server}" 2>/dev/null; then
            log_success "$svc is running"
        else
            log_error "$svc is NOT running"
        fi
    done

    # Check Fortify if installed
    if [[ -f "$FORTIFY_BIN" ]]; then
        if systemctl is-active --quiet fortify; then
            log_success "fortify is running"
        else
            log_warn "fortify is NOT running (may need Redis)"
        fi
    fi

    # Display onion address
    echo ""
    echo "=============================================="
    echo -e "${GREEN}Cerberus Deployed Successfully!${NC}"
    echo "=============================================="
    echo ""
    echo -e "Onion Address: ${YELLOW}$(cat $TOR_HS_DIR/hostname)${NC}"
    echo ""
    echo "Test locally:  curl http://127.0.0.1:10001/"
    echo "HAProxy stats: http://127.0.0.1:8404/"
    echo ""
    echo "Note: Tor needs 1-5 minutes to publish the onion address."
    echo "=============================================="
}

# --- Uninstall ---
uninstall() {
    log_warn "Uninstalling Cerberus..."

    systemctl stop fortify haproxy nginx tor 2>/dev/null || true
    systemctl disable fortify haproxy nginx tor 2>/dev/null || true

    rm -rf "$CERBERUS_ROOT"
    rm -rf "$TOR_HS_DIR"
    rm -f "$NGINX_SITE"
    rm -f /etc/nginx/sites-enabled/cerberus
    rm -f "$SYSTEMD_DIR/fortify.service"
    rm -f "$FORTIFY_BIN"
    rm -f /etc/sysctl.d/99-cerberus.conf

    systemctl daemon-reload
    sysctl --system > /dev/null 2>&1

    log_success "Cerberus uninstalled"
}

# --- Main ---
main() {
    parse_args "$@"

    echo ""
    echo "=============================================="
    echo "     Cerberus Deployment Script v1.0"
    echo "=============================================="
    echo ""

    if [[ "$UNINSTALL" == true ]]; then
        uninstall
        exit 0
    fi

    check_prereqs
    install_deps
    create_dirs
    configure_tor
    configure_haproxy
    configure_nginx
    install_fortify
    apply_sysctl
    start_services
    verify
}

main "$@"
