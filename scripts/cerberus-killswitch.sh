#!/bin/bash
# Cerberus Kill Switch
# Emergency shutdown and data destruction
#
# Usage: sudo ./cerberus-killswitch.sh [mode]
#
# Modes:
#   soft   - Stop services, preserve data (default)
#   hard   - Stop services, secure delete volatile data
#   panic  - Full destruction + kernel panic (CAUTION!)

set -e

MODE="${1:-soft}"
VAULT_PATH="/mnt/cerberus_vault"
LOG_FILE="/var/log/cerberus-killswitch.log"

log() {
    echo "[$(date)] $1" | tee -a "$LOG_FILE"
}

log "=========================================="
log "CERBERUS KILL SWITCH ACTIVATED"
log "Mode: $MODE"
log "=========================================="

# --- Stop All Services ---
stop_services() {
    log "Stopping Cerberus services..."
    
    systemctl stop cerberus-fortify 2>/dev/null || true
    systemctl stop nginx 2>/dev/null || true
    systemctl stop haproxy 2>/dev/null || true
    systemctl stop redis 2>/dev/null || true
    systemctl stop tor 2>/dev/null || true
    
    # Remove XDP/eBPF programs
    IFACE=$(ip route get 8.8.8.8 2>/dev/null | awk '{for(i=1;i<=NF;i++) if($i=="dev") print $(i+1); exit}')
    if [ -n "$IFACE" ]; then
        ip link set dev "$IFACE" xdp off 2>/dev/null || true
        tc qdisc del dev "$IFACE" clsact 2>/dev/null || true
    fi
    
    # Remove pinned BPF maps
    rm -rf /sys/fs/bpf/cerberus_* 2>/dev/null || true
    
    log "✅ All services stopped"
}

# --- Secure Delete Volatile Vault ---
destroy_vault() {
    log "Destroying Volatile Vault..."
    
    if [ -d "$VAULT_PATH" ]; then
        # Overwrite with random data before deletion
        find "$VAULT_PATH" -type f -exec shred -vfz -n 3 {} \; 2>/dev/null || true
        
        # Unmount and destroy tmpfs
        umount -f "$VAULT_PATH" 2>/dev/null || true
        rm -rf "$VAULT_PATH" 2>/dev/null || true
        
        log "✅ Volatile Vault destroyed"
    else
        log "⚠️  Vault not found at $VAULT_PATH"
    fi
}

# --- Delete Sensitive Data ---
delete_sensitive() {
    log "Deleting sensitive data..."
    
    # Tor hidden service keys
    shred -vfz -n 3 /var/lib/tor/hidden_service/hs_ed25519_secret_key 2>/dev/null || true
    shred -vfz -n 3 /var/lib/tor/hidden_service/hs_ed25519_public_key 2>/dev/null || true
    
    # WireGuard keys
    shred -vfz -n 3 /etc/wireguard/*.key 2>/dev/null || true
    
    # Redis dump
    shred -vfz -n 3 /var/lib/redis/dump.rdb 2>/dev/null || true
    
    # Fortify config (may contain secrets)
    shred -vfz -n 3 /etc/cerberus/fortify.toml 2>/dev/null || true
    
    log "✅ Sensitive data destroyed"
}

# --- Execute Based on Mode ---
case "$MODE" in
    soft)
        log "SOFT SHUTDOWN - Preserving data"
        stop_services
        log "✅ Soft shutdown complete"
        ;;
    
    hard)
        log "HARD SHUTDOWN - Destroying volatile data"
        stop_services
        destroy_vault
        delete_sensitive
        log "✅ Hard shutdown complete"
        ;;
    
    panic)
        log "!!! PANIC MODE - FULL DESTRUCTION !!!"
        stop_services
        destroy_vault
        delete_sensitive
        
        # Clear page cache
        sync
        echo 3 > /proc/sys/vm/drop_caches
        
        log "Triggering kernel panic in 5 seconds..."
        log "THIS WILL CRASH THE SYSTEM"
        sleep 5
        
        # Force kernel panic (immediate reboot)
        echo c > /proc/sysrq-trigger
        ;;
    
    *)
        echo "Usage: $0 [soft|hard|panic]"
        exit 1
        ;;
esac
