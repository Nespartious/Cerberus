#!/bin/bash
# Cerberus Dynamic Kernel Tuning
# Calculates optimal TCP/network settings based on available resources
#
# Usage: sudo ./cerberus-sysctl.sh
#
# Generates: /etc/sysctl.d/99-cerberus.conf

set -e

echo "🔧 Cerberus Kernel Tuning Script"
echo "================================"

# --- Detect System Resources ---
MEM_KB=$(grep MemTotal /proc/meminfo | awk '{print $2}')
MEM_MB=$((MEM_KB / 1024))
MEM_GB=$((MEM_MB / 1024))
CPU_CORES=$(nproc)

echo "Detected: ${MEM_MB}MB RAM (${MEM_GB}GB), ${CPU_CORES} CPU Cores"

# --- Calculate Optimal Limits ---

# Max Connections (somaxconn)
# Base: 4096. Add 1024 per 256MB RAM. Cap at 262144.
MAX_CONN=$(( 4096 + (MEM_MB / 256) * 1024 ))
if [ $MAX_CONN -gt 262144 ]; then MAX_CONN=262144; fi

# SYN Backlog
# Base: 1024. Add 512 per 128MB RAM. Cap at 65535.
SYN_BACKLOG=$(( 1024 + (MEM_MB / 128) * 512 ))
if [ $SYN_BACKLOG -gt 65535 ]; then SYN_BACKLOG=65535; fi

# File Descriptors
FILE_MAX=$(( MAX_CONN * 4 ))

# Network Buffer Sizes (based on available RAM)
if [ $MEM_GB -ge 16 ]; then
    RMEM_MAX=67108864    # 64MB
    WMEM_MAX=67108864
    NETDEV_BUDGET=600
elif [ $MEM_GB -ge 8 ]; then
    RMEM_MAX=33554432    # 32MB
    WMEM_MAX=33554432
    NETDEV_BUDGET=450
elif [ $MEM_GB -ge 4 ]; then
    RMEM_MAX=16777216    # 16MB
    WMEM_MAX=16777216
    NETDEV_BUDGET=300
else
    RMEM_MAX=8388608     # 8MB
    WMEM_MAX=8388608
    NETDEV_BUDGET=200
fi

echo ""
echo "Calculated Optimization:"
echo "  Max Connections: $MAX_CONN"
echo "  SYN Backlog:     $SYN_BACKLOG"
echo "  File Max:        $FILE_MAX"
echo "  Buffer Size:     $((RMEM_MAX / 1024 / 1024))MB"
echo "  Netdev Budget:   $NETDEV_BUDGET"

# --- Write Configuration ---
CONFIG_FILE="/etc/sysctl.d/99-cerberus.conf"

cat <<EOF > "$CONFIG_FILE"
# Cerberus Dynamic Kernel Tuning
# Generated on $(date) for ${MEM_MB}MB / ${CPU_CORES} Core system
# DO NOT EDIT - Regenerate with: cerberus-sysctl.sh

# =============================================================================
# MEMORY PROTECTION
# =============================================================================
# Reduce swappiness - keep working set in RAM
vm.swappiness = 10

# Allow overcommit (Redis requirement)
vm.overcommit_memory = 1

# =============================================================================
# NETWORK HARDENING (Anti-DoS)
# =============================================================================
# Enable SYN cookies (protects against SYN flood)
net.ipv4.tcp_syncookies = 1

# Reduce SYN-ACK retries (faster timeout for half-open connections)
net.ipv4.tcp_synack_retries = 2

# SYN backlog queue size
net.ipv4.tcp_max_syn_backlog = ${SYN_BACKLOG}

# Listen backlog (accept queue)
net.core.somaxconn = ${MAX_CONN}

# Netdev backlog (NIC → kernel queue)
net.core.netdev_max_backlog = ${MAX_CONN}

# Enable reverse path filtering (anti-spoofing)
net.ipv4.conf.all.rp_filter = 1
net.ipv4.conf.default.rp_filter = 1

# Ignore ICMP redirects (prevent MITM)
net.ipv4.conf.all.accept_redirects = 0
net.ipv4.conf.default.accept_redirects = 0

# Don't send ICMP redirects
net.ipv4.conf.all.send_redirects = 0
net.ipv4.conf.default.send_redirects = 0

# =============================================================================
# CONNECTION RECYCLING (High Throughput)
# =============================================================================
# Faster FIN timeout (default: 60s)
net.ipv4.tcp_fin_timeout = 15

# Faster keepalive detection
net.ipv4.tcp_keepalive_time = 60
net.ipv4.tcp_keepalive_probes = 3
net.ipv4.tcp_keepalive_intvl = 10

# Allow TIME-WAIT socket reuse
net.ipv4.tcp_tw_reuse = 1

# TIME-WAIT bucket limit
net.ipv4.tcp_max_tw_buckets = $(( MAX_CONN * 2 ))

# =============================================================================
# BUFFER SIZES (Performance)
# =============================================================================
# Socket receive buffer
net.core.rmem_default = 262144
net.core.rmem_max = ${RMEM_MAX}

# Socket send buffer
net.core.wmem_default = 262144
net.core.wmem_max = ${WMEM_MAX}

# TCP buffer auto-tuning (min, default, max)
net.ipv4.tcp_rmem = 4096 87380 ${RMEM_MAX}
net.ipv4.tcp_wmem = 4096 65536 ${WMEM_MAX}

# Network device budget (packets per softirq)
net.core.netdev_budget = ${NETDEV_BUDGET}
net.core.netdev_budget_usecs = 8000

# =============================================================================
# FILE DESCRIPTOR LIMITS
# =============================================================================
fs.file-max = ${FILE_MAX}

# Expand local port range
net.ipv4.ip_local_port_range = 1024 65535
EOF

echo ""
echo "✅ Configuration written to: $CONFIG_FILE"

# --- Apply Configuration ---
echo ""
echo "Applying sysctl settings..."
sysctl --system > /dev/null 2>&1

echo "✅ Kernel optimization applied successfully!"
echo ""
echo "📊 Verify with: sysctl -a | grep -E 'somaxconn|tcp_max_syn|file-max'"
