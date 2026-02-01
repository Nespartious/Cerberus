#!/bin/bash
# Cerberus XDP/eBPF Initialization Script
# Robust loader with adaptive fallback strategy
#
# Usage: sudo ./cerberus-init.sh [interface]
# 
# Exit Codes:
#   0 - Success (XDP Native, Generic, TC, or Nftables loaded)
#   1 - Critical failure (no defense active)

set -u  # Exit on undefined variables

LOG_FILE="/var/log/cerberus-xdp.log"
XDP_DIR="${XDP_DIR:-/opt/cerberus/xdp}"

# Ensure log directory exists
mkdir -p "$(dirname "$LOG_FILE")"
exec > >(tee -a "${LOG_FILE}") 2>&1

echo "[$(date)] =========================================="
echo "[$(date)] Starting Cerberus XDP Init..."

# --- 1. Dependency Check ---
echo "[$(date)] Checking dependencies..."
MISSING_DEPS=""
for cmd in ip ethtool bpftool tc nft; do
    if ! command -v "$cmd" &> /dev/null; then
        MISSING_DEPS="$MISSING_DEPS $cmd"
    fi
done

if [ -n "$MISSING_DEPS" ]; then
    echo "❌ Critical Error: Missing commands:$MISSING_DEPS"
    echo "   Install: apt install iproute2 ethtool bpf-tools nftables"
    exit 1
fi
echo "✅ All dependencies present"

# --- 2. Interface Detection ---
if [ $# -ge 1 ]; then
    IFACE="$1"
else
    IFACE=$(ip route get 8.8.8.8 2>/dev/null | awk '{for(i=1;i<=NF;i++) if($i=="dev") print $(i+1); exit}')
fi

if [ -z "$IFACE" ]; then
    echo "❌ Error: Could not detect default interface."
    echo "   Usage: $0 <interface>"
    exit 1
fi

# Get driver info
DRIVER=$(ethtool -i "$IFACE" 2>/dev/null | grep driver | awk '{print $2}' || echo "unknown")
echo "ℹ️  Detected Interface: $IFACE (Driver: $DRIVER)"

# --- 3. Cleanup Previous State ---
echo "[$(date)] Cleaning up previous XDP/TC state..."
ip link set dev "$IFACE" xdp off 2>/dev/null || true
tc qdisc del dev "$IFACE" clsact 2>/dev/null || true
rm -f /sys/fs/bpf/cerberus_* 2>/dev/null || true

# --- 4. XDP Loading Functions ---

load_xdp_native() {
    echo "🔄 Plan A: Attempting XDP Native mode..."
    if ip link set dev "$IFACE" xdp obj "${XDP_DIR}/cerberus_xdp.o" sec xdp 2>/dev/null; then
        echo "✅ Success: XDP Native mode active"
        echo "🚀 Cerberus L2 Defense: HARDWARE MODE (Best Performance)"
        return 0
    fi
    echo "⚠️  XDP Native failed (driver may not support it)"
    return 1
}

load_xdp_generic() {
    echo "🔄 Plan B: Attempting XDP Generic (SKB) mode..."
    if ip link set dev "$IFACE" xdpgeneric obj "${XDP_DIR}/cerberus_xdp.o" sec xdp 2>/dev/null; then
        echo "✅ Success: XDP Generic mode active"
        echo "🛡️ Cerberus L2 Defense: GENERIC MODE (Good Compatibility)"
        return 0
    fi
    echo "⚠️  XDP Generic failed"
    return 1
}

load_tc_bpf() {
    echo "🔄 Plan C: Attempting TC eBPF Ingress mode..."
    
    # Add clsact qdisc (supports ingress + egress)
    if ! tc qdisc add dev "$IFACE" clsact 2>/dev/null; then
        echo "⚠️  Failed to add clsact qdisc"
        return 1
    fi
    
    if tc filter add dev "$IFACE" ingress bpf da obj "${XDP_DIR}/cerberus_tc.o" sec ingress_firewall 2>/dev/null; then
        echo "✅ Success: TC eBPF mode active"
        echo "🛡️ Cerberus L3 Defense: TC MODE (Traffic Control)"
        return 0
    fi
    
    echo "⚠️  TC eBPF failed"
    tc qdisc del dev "$IFACE" clsact 2>/dev/null || true
    return 1
}

load_nftables() {
    echo "🔄 Plan D: Falling back to Nftables Raw..."
    
    # Remove existing cerberus table if present
    nft delete table ip cerberus_raw 2>/dev/null || true
    
    # Create basic rate limiting rules
    cat <<EOF | nft -f -
table ip cerberus_raw {
    set ratelimit {
        type ipv4_addr
        flags dynamic,timeout
        timeout 60s
    }
    
    chain prerouting {
        type filter hook prerouting priority -300; policy accept;
        
        # Track and limit per-IP
        ip protocol tcp update @ratelimit { ip saddr limit rate over 5000/second } drop
        
        # Allow WireGuard
        udp dport 51820 accept
    }
}
EOF
    
    if [ $? -eq 0 ]; then
        echo "✅ Success: Nftables mode active"
        echo "🛡️ Cerberus L3 Defense: NFTABLES MODE (Basic Filtering)"
        return 0
    fi
    
    echo "❌ Nftables failed"
    return 1
}

# --- 5. Execution Strategy: Adaptive Loading ---

# Check if XDP objects exist
if [ ! -f "${XDP_DIR}/cerberus_xdp.o" ]; then
    echo "⚠️  XDP objects not found at ${XDP_DIR}/"
    echo "   Attempting to build from source..."
    
    if [ -f "xdp/Makefile" ]; then
        make -C xdp && make -C xdp install
    else
        echo "⚠️  No XDP source found, skipping to Plan C/D"
    fi
fi

# Try each plan in order
if [ -f "${XDP_DIR}/cerberus_xdp.o" ]; then
    load_xdp_native && exit 0
    load_xdp_generic && exit 0
fi

if [ -f "${XDP_DIR}/cerberus_tc.o" ]; then
    load_tc_bpf && exit 0
fi

load_nftables && exit 0

# --- 6. Complete Failure ---
echo "❌ CRITICAL: All defense mechanisms failed!"
echo "   The system is UNPROTECTED against volumetric attacks."
echo "   Manual intervention required."
exit 1
