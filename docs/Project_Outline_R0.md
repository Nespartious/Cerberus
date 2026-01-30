# Cerberus: The Monster Documentation
> **Single Source of Truth** for the Cerberus High-Assurance Tor Ingress Defense System.
> *Last Updated: Jan 2026*

---

# 1. Project Overview

**Cerberus** is a specialized, defense-in-depth reverse proxy designed exclusively for **Tor Onion Services** operating in hostile environments. It acts as a shield between the Tor network and your backend application, filtering traffic through four distinct layers of defense.

### Core Philosophy: Human-Cost Asymmetry
> **"Make the cost of being wrong trivial for humans and expensive for bots."**

- **Humans:** One easy CAPTCHA, quick solve, mistakes are free.
- **Bots:** Failed attempts trigger escalation → multi-CAPTCHA chains → soft-locks → bans.
- **Impact:** A bot needs **38+ days** to make 10,000 requests. A human needs **seconds**.

### Key Constraints
1.  **Tor-Native:** Optimized for high-latency, anonymous circuits.
2.  **Zero JavaScript:** Fully functional in Tor Browser "Safest" Mode.
3.  **Privacy First:** No IP logging (useless anyway), no persistent fingerprinting.
4.  **Fail-Closed:** If a component fails, traffic stops. No "fail-open" vulnerability.

---

# 2. Master Architecture (The 4 Layers)

Traffic flows through four distinct defense layers before reaching the backend.

| Layer | Component | Role | Key Defense |
|-------|-----------|------|-------------|
| **L2** | **XDP / eBPF** | Kernel Shield | Volumetric flood protection (SYN, UDP) |
| **L3** | **TC eBPF** | Flow Shaper | Relay-aware traffic shaping & penalties |
| **L4** | **HAProxy** | Connection Governor | Circuit tracking, rate limiting, connection slots |
| **L7** | **Nginx** | Gatekeeper | Protocol sanitization, static CAPTCHA delivery |
| **L7+** | **Fortify** | Logic Engine | CAPTCHA verification, Threat Dial, Clustering |

---

# 3. L2: The Kernel Shield (XDP / eBPF)
*Located at the Linux Network Driver level.*

### 3.1 Source Code: `cerberus_xdp_kern.c`
The following eBPF program drops volumetric floods before the OS allocates memory. It implements a Token Bucket algorithm for per-IP rate limiting.

```c
#include <linux/bpf.h>
#include <linux/if_ether.h>
#include <linux/ip.h>
#include <linux/udp.h>
#include <linux/tcp.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_endian.h>

// --- Configuration Constants ---
#define MAX_PPS_PER_IP 5000     // Packets Per Second Allowed
#define BLOCK_DURATION 60000000000ULL // 60 seconds in nanoseconds

// --- BPF Maps ---

// 1. Rate Limit Map (LRU Hash)
// Key: Source IP (u32), Value: struct rate_info
struct rate_info {
    __u64 last_seen;
    __u64 packet_count;
};

struct {
    __uint(type, BPF_MAP_TYPE_LRU_HASH);
    __uint(max_entries, 100000); // Track up to 100k distinct IPs
    __type(key, __u32);
    __type(value, struct rate_info);
} rate_map SEC(".maps");

// 2. Block List (LRU Hash)
// Key: Source IP (u32), Value: Expiry Timestamp (u64)
struct {
    __uint(type, BPF_MAP_TYPE_LRU_HASH);
    __uint(max_entries, 10000);
    __type(key, __u32);
    __type(value, __u64);
} block_map SEC(".maps");

// --- Helper Functions ---

static __always_inline int check_rate_limit(__u32 src_ip) {
    __u64 now = bpf_ktime_get_ns();
    struct rate_info *info = bpf_map_lookup_elem(&rate_map, &src_ip);
    
    if (!info) {
        // New IP: Initialize
        struct rate_info new_info = { .last_seen = now, .packet_count = 1 };
        bpf_map_update_elem(&rate_map, &src_ip, &new_info, BPF_ANY);
        return XDP_PASS;
    }

    // Reset counter every second
    if (now - info->last_seen > 1000000000ULL) {
        info->last_seen = now;
        info->packet_count = 1;
    } else {
        info->packet_count++;
        if (info->packet_count > MAX_PPS_PER_IP) {
            // Threshold exceeded: Add to block map
            __u64 expiry = now + BLOCK_DURATION;
            bpf_map_update_elem(&block_map, &src_ip, &expiry, BPF_ANY);
            return XDP_DROP;
        }
    }
    return XDP_PASS;
}

SEC("xdp")
int cerberus_firewall(struct xdp_md *ctx) {
    void *data_end = (void *)(long)ctx->data_end;
    void *data = (void *)(long)ctx->data;
    struct ethhdr *eth = data;

    // 1. Sanity Check: Is it IP?
    if ((void *)(eth + 1) > data_end) return XDP_PASS;
    if (eth->h_proto != bpf_htons(ETH_P_IP)) return XDP_PASS;

    struct iphdr *ip = (void *)(eth + 1);
    if ((void *)(ip + 1) > data_end) return XDP_PASS;
    
    // 2. Check Block List First (Fast Drop)
    __u32 src_ip = ip->saddr;
    __u64 now = bpf_ktime_get_ns();
    __u64 *expiry = bpf_map_lookup_elem(&block_map, &src_ip);
    
    if (expiry) {
        if (now < *expiry) return XDP_DROP; // Still blocked
        bpf_map_delete_elem(&block_map, &src_ip); // Expired, unblock
    }

    // 3. Protocol Filter
    
    // TCP: Rate Limit + SYN Flood Check
    if (ip->protocol == IPPROTO_TCP) {
        struct tcphdr *tcp = (void *)(ip + 1);
        if ((void *)(tcp + 1) > data_end) return XDP_DROP;
        
        // Rate Limit all TCP traffic per IP
        return check_rate_limit(src_ip);
    }
    
    // UDP: Allow only WireGuard (51820)
    if (ip->protocol == IPPROTO_UDP) {
        struct udphdr *udp = (void *)(ip + 1);
        if ((void *)(udp + 1) > data_end) return XDP_PASS; // Malformed UDP -> Pass to kernel validation
        if (udp->dest != bpf_htons(51820)) return XDP_PASS; // Allow other UDP (DNS/DHCP) for now
        return XDP_PASS;
    }

    // Default Action: PASS (Allow unknown traffic for MVP stability)
    // TODO: Harden to XDP_DROP in future phases once allowlist is exhaustive (SSH, ICMP, DNS)
    return XDP_PASS;
}
char _license[] SEC("license") = "GPL";
```

### 3.2 Build System: `Makefile`
Compiles the eBPF program using clang/llvm.

```makefile
CLANG ?= clang
LLC ?= llc
ARCH := $(shell uname -m | sed 's/x86_64/x86/' | sed 's/aarch64/arm64/')

BPF_CFLAGS ?= -O2 -g -target bpf -D__TARGET_ARCH_$(ARCH)

all: cerberus_xdp.o

cerberus_xdp.o: cerberus_xdp_kern.c
	$(CLANG) $(BPF_CFLAGS) -c $< -o $@

clean:
	rm -f cerberus_xdp.o
```

### 3.3 Userspace Loader: `cerberus_loader.c`
Loads the XDP program, attaches it to the interface, and pins the maps for userspace access.

```c
#include <bpf/libbpf.h>
#include <linux/if_link.h>
#include <net/if.h>

int main(int argc, char **argv) {
    struct bpf_object *obj;
    int prog_fd, map_fd;
    const char *ifname = "eth0"; // Detect dynamically in prod

    // 1. Open and Load BPF Object
    obj = bpf_object__open_file("cerberus_xdp.o", NULL);
    if (libbpf_get_error(obj)) return 1;
    
    if (bpf_object__load(obj)) return 1;

    // 2. Find Program and Attach
    struct bpf_program *prog = bpf_object__find_program_by_name(obj, "cerberus_firewall");
    prog_fd = bpf_program__fd(prog);
    
    int ifindex = if_nametoindex(ifname);
    // Attach in DRIVER (Native) mode, fallback handled by script
    bpf_set_link_xdp_fd(ifindex, prog_fd, XDP_FLAGS_DRV_MODE);

    // 3. Pin Maps for Persistence
    // Allows cerberus-cli to read maps even if loader exits
    struct bpf_map *rate_map = bpf_object__find_map_by_name(obj, "rate_map");
    bpf_map__pin(rate_map, "/sys/fs/bpf/cerberus_rate_map");
    
    return 0;
}
```

### 3.4 Deployment: `cerberus-init.sh` (Robust XDP Loader)
Detects hardware, validates dependencies, and attempts Native loading with a robust Generic fallback.

```bash
#!/bin/bash
set -u # Exit on undefined variables

LOG_FILE="/var/log/cerberus-xdp.log"
exec > >(tee -a ${LOG_FILE}) 2>&1

echo "[$(date)] Starting Cerberus XDP Init..."

# 1. Dependency Check
for cmd in ip ethtool bpftool; do
    if ! command -v $cmd &> /dev/null; then
        echo "❌ Critical Error: $cmd not found. Install iproute2, ethtool, bpf-tools."
        exit 1
    fi
done

# 2. Interface Detection
# Find default interface used for routing
IFACE=$(ip route get 8.8.8.8 | awk '{print $5; exit}')
if [ -z "$IFACE" ]; then
    echo "❌ Error: Could not detect default interface."
    exit 1
fi

DRIVER=$(ethtool -i $IFACE | grep driver | awk '{print $2}')
echo "ℹ️  Detected Interface: $IFACE (Driver: $DRIVER)"

# 3. Cleanup Previous State
ip link set dev $IFACE xdp off 2>/dev/null
rm -f /sys/fs/bpf/cerberus_rate_map 2>/dev/null

# 4. Load Function
load_xdp() {
    local mode=$1
    local flags=""
    
    if [ "$mode" == "native" ]; then
        flags="xdp" # XDP_FLAGS_DRV_MODE
    else
        flags="xdpgeneric" # XDP_FLAGS_SKB_MODE
    fi

    echo "🔄 Attempting loading in $mode mode..."
    
    # Try loading via ip link (basic)
    if ip link set dev $IFACE $flags obj cerberus_xdp.o sec xdp verbose; then
        echo "✅ Success: Loaded XDP in $mode mode."
        return 0
    else
        echo "⚠️  Failed to load in $mode mode."
        return 1
    fi
}

# 5. Execution Strategy
# Try Native (Driver) Mode first for performance
if load_xdp "native"; then
    MODE="native"
else
    # Fallback to Generic (SKB) Mode
    echo "⚠️  Native loading failed. Falling back to Generic mode (slower but compatible)."
    if load_xdp "generic"; then
        MODE="generic"
    else
        echo "❌ Critical Failure: Could not load XDP in Native OR Generic mode."
        echo "   Check kernel version (requires 4.18+) and verifier logs."
        exit 1
    fi
fi

# 6. Verification
IS_LOADED=$(ip link show dev $IFACE | grep "prog/xdp")
if [ -z "$IS_LOADED" ]; then
    echo "❌ Error: Interface reports XDP not attached despite command success."
    exit 1
fi

echo "🚀 Cerberus XDP Active on $IFACE [$MODE]"
exit 0
```

---

# 4. L3/L4: The Flow Shaper & Kernel Tuning
*Optimization of the Linux Network Stack.*

### 4.1 TC eBPF: Traffic Control Policy
We use `tc` (Traffic Control) with a BPF classifier to apply stateful shaping.

**Deployment Script: `cerberus-tc.sh`**
```bash
#!/bin/bash
IFACE="eth0"

# 1. Clear existing qdiscs
tc qdisc del dev $IFACE clsact 2>/dev/null

# 2. Add clsact qdisc (ingress + egress)
tc qdisc add dev $IFACE clsact

# 3. Load BPF Program for Ingress
# This BPF program reads the 'rate_map' populated by XDP 
# and sets skb->mark if the IP is flagged as "Suspicious"
tc filter add dev $IFACE ingress bpf da obj cerberus_tc.o sec ingress_flow_shaper

# 4. Apply Policing based on Marks
# Mark 1: Suspicious (Add 100ms Latency)
tc qdisc add dev $IFACE handle 1: root htb
tc class add dev $IFACE parent 1: classid 1:1 htb rate 1gbit
tc qdisc add dev $IFACE parent 1:1 handle 10: netem delay 100ms

# Redirect marked packets to the netem qdisc
tc filter add dev $IFACE parent 1: protocol ip handle 1 fw flowid 1:1
```

### 4.2 Dynamic Kernel Tuning: `cerberus-sysctl.sh`
Calculates optimal TCP settings based on available RAM and CPU, supporting everything from low-end VPS to high-end dedicated servers.

```bash
#!/bin/bash
# Generate /etc/sysctl.d/99-cerberus.conf dynamically

MEM_KB=$(grep MemTotal /proc/meminfo | awk '{print $2}')
MEM_MB=$((MEM_KB / 1024))
CPU_CORES=$(nproc)

echo "Detected: ${MEM_MB}MB RAM, ${CPU_CORES} Cores"

# --- Calculate Limits ---

# Max Connections (somaxconn)
# Base: 4096. Add 1024 per 256MB RAM. Cap at 262144.
MAX_CONN=$(( 4096 + (MEM_MB / 256) * 1024 ))
if [ $MAX_CONN -gt 262144 ]; then MAX_CONN=262144; fi

# SYN Backlog
# Base: 1024. Add 512 per 128MB RAM.
SYN_BACKLOG=$(( 1024 + (MEM_MB / 128) * 512 ))
if [ $SYN_BACKLOG -gt 65535 ]; then SYN_BACKLOG=65535; fi

# File Descriptors
FILE_MAX=$(( MAX_CONN * 4 ))

echo "Calculated Optimization:"
echo "  Max Connections: $MAX_CONN"
echo "  SYN Backlog:     $SYN_BACKLOG"
echo "  File Max:        $FILE_MAX"

# --- Write Configuration ---
cat <<EOF > /etc/sysctl.d/99-cerberus.conf
# Cerberus Dynamic Tuning
# Generated on $(date) for ${MEM_MB}MB / ${CPU_CORES} Core system

# --- Memory Protection ---
vm.swappiness = 10
vm.overcommit_memory = 1

# --- Network Hardening ---
net.ipv4.tcp_syncookies = 1
net.ipv4.tcp_synack_retries = 2
net.ipv4.tcp_max_syn_backlog = ${SYN_BACKLOG}
net.core.somaxconn = ${MAX_CONN}
net.core.netdev_max_backlog = ${MAX_CONN}

# --- Resource Recycling ---
net.ipv4.tcp_fin_timeout = 15
net.ipv4.tcp_keepalive_time = 60
net.ipv4.tcp_keepalive_probes = 3
net.ipv4.tcp_keepalive_intvl = 10
net.ipv4.tcp_tw_reuse = 1
net.ipv4.tcp_max_tw_buckets = $(( MAX_CONN * 2 ))

# --- Connection Limits ---
fs.file-max = ${FILE_MAX}
net.ipv4.ip_local_port_range = 1024 65535
EOF

# Apply
sysctl --system
echo "✅ Sysctl optimization applied."
```

---

# 5. L4: The Governor (HAProxy)
*Located at userland TCP/HTTP level.*

### Configuration: `/etc/haproxy/haproxy.cfg`
Full configuration implementing the Two-Lane architecture and Circuit ID tracking.

```haproxy
global
    log /dev/log local0
    log /dev/log local1 notice
    user haproxy
    group haproxy
    daemon
    
    # Performance Limits
    maxconn 100000
    nbthread 4
    
    # Runtime API (for Fortify)
    stats socket /var/run/haproxy.sock mode 660 level admin

defaults
    log     global
    mode    http
    option  httplog
    option  dontlognull
    
    # Aggressive Timeouts (Slowloris Defense)
    timeout connect 5s
    timeout client  10s
    timeout server  10s
    timeout http-request 3s  # Kill clients sending headers too slowly

# --- Stick Table Definition ---
# Tracks Tor Circuit IDs.
# gpc0: 0=Normal, 1=VIP, 2=Banned
backend be_stick_tables
    stick-table type string len 64 size 1m expire 30m store conn_cur,conn_rate(10s),http_req_rate(10s),gpc0

# --- Lane A: Public (Port 8080) ---
frontend ft_tor_public
    bind 127.0.0.1:8080 accept-proxy
    
    # 1. Extract Circuit ID (from PROXY v2 header or custom header)
    # Note: Assumes Tor passes ID via PROXY protocol
    http-request set-var(req.circuit_id) fc_pp_unique_id
    
    # 2. Track in Stick Table
    http-request track-sc0 var(req.circuit_id) table be_stick_tables
    
    # 3. Security Checks
    # Ban Check
    http-request deny deny_status 403 if { sc0_get_gpc0(be_stick_tables) eq 2 }
    
    # Rate Limiting (Strict)
    # Max 10 concurrent conns, Max 20 req/10s
    http-request deny deny_status 429 if { sc0_conn_cur(be_stick_tables) gt 10 }
    http-request deny deny_status 429 if { sc0_http_req_rate(be_stick_tables) gt 20 }
    
    # 4. Routing
    # VIPs bypass queue (gpc0 == 1)
    use_backend be_nginx_vip if { sc0_get_gpc0(be_stick_tables) eq 1 }
    
    # Normal users go to standard backend
    default_backend be_nginx_public

# --- Lane B: Passport/VIP (Port 8081) ---
frontend ft_tor_passport
    bind 127.0.0.1:8081 accept-proxy # HTTP (Encrypted by WireGuard/Tor)
    
    # 1. Validation
    # Must have ?passport_token=...
    acl has_token url_param(passport_token) -m found
    http-request deny deny_status 403 unless has_token
    
    # 2. Relaxed Limits
    # Max 50 concurrent conns
    http-request track-sc0 fc_pp_unique_id table be_stick_tables
    http-request deny deny_status 429 if { sc0_conn_cur(be_stick_tables) gt 50 }
    
    default_backend be_nginx_vip

# --- Backends ---
backend be_nginx_public
    server nginx_local 127.0.0.1:10001 maxconn 5000

backend be_nginx_vip
    server nginx_local 127.0.0.1:10001 maxconn 5000
```

### Lua Scripting: `tor_circuit.lua` (Optional)
If Tor doesn't provide the Unique ID in a standard field, use Lua to extract it from the PROXY header.

```lua
core.register_action("extract_circuit", { "http-req" }, function(txn)
    -- Logic to parse custom header if needed
    local cid = txn.f:src() -- Placeholder
    txn:set_var("req.circuit_id", cid)
end)
```

---

# 6. L7: The Gatekeeper (Nginx)
*Located at HTTP level, behind HAProxy.*

### Main Config: `/etc/nginx/nginx.conf`
Optimized for high concurrency and security.

```nginx
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
    keepalive_timeout 5s; # Aggressive timeout
    server_tokens off;    # Hide version
    
    # Buffer Hardening (Anti-DoS)
    client_body_buffer_size 16k;  # Max buffer for POST
    client_header_buffer_size 1k; # Tiny header buffer
    client_max_body_size 1m;      # Absolute max upload
    large_client_header_buffers 2 1k;
    
    # Timeouts (Kill slowloris)
    client_body_timeout 5s;
    client_header_timeout 5s;
    
    include /etc/nginx/sites-enabled/*;
}
```

### Site Config: `/etc/nginx/sites-available/cerberus`
Implements the Static Gate and Header Scrubbing.

```nginx
server {
    listen 127.0.0.1:10001;
    server_name _;
    
    root /var/www/cerberus/static;
    index captcha.html;
    
    # --- header Scrubbing (Privacy) ---
    proxy_set_header User-Agent "Mozilla/5.0 (Windows NT 10.0; rv:115.0) Gecko/20100101 Firefox/115.0";
    proxy_set_header Accept-Language "en-US,en;q=0.5";
    proxy_set_header Accept-Encoding "gzip, deflate";
    proxy_set_header Via "";
    proxy_set_header X-Forwarded-For ""; # Hide real IP path
    
    # --- Security Headers ---
    add_header X-Frame-Options "DENY";
    add_header Content-Security-Policy "default-src 'self'; style-src 'self' 'unsafe-inline';";
    
    # --- 1. Static Gate (Default) ---
    # Serve CAPTCHA from RAM (OS Page Cache)
    location / {
        try_files $uri /captcha.html;
    }
    
    # --- 2. Fortify Interface (Dynamic) ---
    # Only for CAPTCHA verification
    location /verify {
        limit_except POST { deny all; }
        
        # Isolation: Use UNIX Socket
        proxy_pass http://unix:/var/run/fortify.sock;
        
        # Backpressure (Fail Fast)
        proxy_connect_timeout 1s;
        proxy_read_timeout 2s;
    }
    
    # --- 3. Protected Backend (Authenticated) ---
    # Only accessible with valid session cookie
    location /tunnel {
        internal; # Only accessible via X-Accel-Redirect from Fortify
        proxy_pass http://127.0.0.1:8082; # Tor Tunnel
    }
}
```

---

# 7. L7+: Fortify (The Logic Engine)
*Custom Rust binary. The Brain of the operation.*

### 7.1 Core Logic & Entry Point: `main.rs`
Sets up the Tokio runtime, Redis pool, and HTTP server.

```rust
use axum::{routing::{get, post}, Router};
use deadpool_redis::{Config, Runtime};
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize Logging
    tracing_subscriber::fmt::init();
    
    // 2. Setup Redis Pool (Clustered) - Non-Blocking
    let mut cfg = Config::from_url("redis://10.100.0.1:6379");
    let pool = cfg.create_pool(Some(Runtime::Tokio1))
        .unwrap_or_else(|e| {
            error!("⚠️  Redis Init Failed: {}. Starting in DEGRADED MODE (Local Only).", e);
            // Create a "Null" pool or handle in AppState
            create_offline_pool() 
        });
    
    // 3. Initialize Ammo Box (RAM Pool)
    let ammo_box = Arc::new(AmmoBox::new(100_000));
    let ammo_clone = ammo_box.clone();
    
    // 4. Spawn Background Workers
    tokio::spawn(async move {
        maintain_ammo_box(ammo_clone).await;
    });

    // 5. Build Router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/captcha", get(get_captcha))
        .route("/verify", post(verify_solution))
        .with_state(AppState { redis: pool, ammo: ammo_box });

    // 6. Bind to Unix Socket (Isolation)
    let listener = tokio::net::UnixListener::bind("/var/run/fortify.sock")?;
    axum::serve(listener, app).await?;
    
    Ok(())
}
```

### 7.2 The Ammo Box: `captcha.rs`
Implements "Deep Storage" strategy: Tier 1 (RAM Ring Buffer) for speed, Tier 2 (Disk Cache) for sustainment.

```rust
use serde::{Serialize, Deserialize};
use crossbeam_queue::ArrayQueue;
use std::time::{Instant, Duration};
use tokio::fs;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CaptchaChallenge {
    pub id: String,          // Uuid
    #[serde(skip)]           // Don't send bytes in JSON debug
    pub image_png: Vec<u8>,
    pub answer_hash: String, // Sha256(answer + salt)
    pub variant: String,     // "distorted_text", "object_id", etc.
}

pub struct AmmoBox {
    pool: ArrayQueue<CaptchaChallenge>,
    capacity: usize,
    last_dump: Mutex<Instant>,
}

impl AmmoBox {
    pub fn new(capacity: usize) -> Self {
        // On startup: Try loading from disk (ammo.bin)
        // If disk fails, start empty and let background worker fill
        Self {
            pool: ArrayQueue::new(capacity),
            capacity,
            last_dump: Mutex::new(Instant::now()),
        }
    }
}

// Background Worker: The "Reloader"
// Prioritizes loading from disk under load, generating to disk when idle.
async fn maintain_ammo_box(ammo: Arc<AmmoBox>) {
    const MAX_DISK_AMMO: usize = 1_000_000; // Hard cap: 1 Million CAPTCHAs (~500MB on disk)
    const MIN_DISK_FREE_GB: u64 = 10;       // Don't fill disk if low space

    loop {
        let current_load = system::get_cpu_load(); // 0-100
        let pool_len = ammo.pool.len();
        let pool_max = ammo.capacity;
        let disk_count = disk_store::get_count();
        let free_space_gb = disk_store::get_free_space_gb();

        // 1. Critical Low (< 10%): Emergency Action
        if pool_len < pool_max / 10 {
            if current_load > 80 {
                // CPU High: Load from Disk (Cheap I/O)
                load_batch_from_disk(&ammo).await;
            } else {
                // CPU Low: Generate (Expensive)
                let batch = generate_batch(100); 
                push_batch(&ammo, batch);
            }
        }
        // 2. Normal Maintenance (< 80%)
        else if pool_len < (pool_max as f64 * 0.8) as usize {
            if current_load < 50 {
                // Only generate if system is healthy
                let batch = generate_batch(50);
                push_batch(&ammo, batch);
            }
        }
        
        // 3. Surplus Strategy (> 95%): Deep Storage
        // Generate "Ammo Cans" to disk ONLY if:
        // - RAM Pool is full
        // - CPU is idle (< 20%)
        // - Disk usage is below cap (< 1M)
        // - Disk space is healthy (> 10GB free)
        if pool_len > (pool_max as f64 * 0.95) as usize 
           && current_load < 20 
           && disk_count < MAX_DISK_AMMO
           && free_space_gb > MIN_DISK_FREE_GB 
        {
             generate_ammo_can_to_disk("ammo_cache/batch_x.bin", 1000).await;
        }
        
        // 4. Safe Dump Trigger (Persist RAM to Disk)
        // Dump if FULL and hasn't dumped in 10 mins (Recovery Point)
        let mut last = ammo.last_dump.lock().await;
        if ammo.pool.is_full() && last.elapsed() > Duration::from_secs(600) {
             // Save to /var/lib/cerberus/ammo.bin
             save_ammo_to_disk(&ammo.pool, "/var/lib/cerberus/ammo.bin").await;
             *last = Instant::now();
        }
        
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
```

### 7.3 Logic Core & Load Shedding: `handlers.rs`
Decides whether to admit, shed, or bunker based on system health.

```rust
pub async fn handle_request(req: Request) -> Response {
    let load = system::get_cpu_load(); // %
    
    match load {
        // Normal Mode: Admit to CAPTCHA
        0..=80 => serve_captcha_from_pool().await,
        
        // Shed Mode: Redirect to Peer
        81..=95 => {
            let peer = cluster::get_healthy_peer();
            if let Some(p) = peer {
                let passport = crypto::mint_passport(p.id);
                // Redirect to peer with passport token
                return Response::redirect(format!("http://{}/?passport_token={}", p.onion, passport));
            }
            // If no peers, fallback to Bunker
            Response::html(QUEUE_PAGE_HTML)
        },
        
        // Bunker Mode: Queue (Meta-Refresh)
        _ => Response::html(QUEUE_PAGE_HTML), // Static page with <meta refresh=15>
    }
}
```

### 7.4 Error Handling: `error.rs`
Using `thiserror` for precise control.

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FortifyError {
    #[error("Redis connection failed")]
    RedisError(#[from] deadpool_redis::PoolError),
    
    #[error("Ammo box empty")]
    AmmoEmpty,
    
    #[error("Invalid passport signature")]
    InvalidPassport,
    
    #[error("System overloaded")]
    Overload,
}
```

### 7.5 The Passport Protocol (Crypto)
Cryptographic tokens for inter-node trust.

```rust
use ed25519_dalek::{Signer, Verifier, Signature};

// Minting a Passport
fn mint_passport(target_node: String) -> String {
    let expiry = now() + 30; // Valid for 30 seconds
    let payload = format!("{}:{}", target_node, expiry);
    let keypair = get_node_keypair();
    
    let signature = keypair.sign(payload.as_bytes());
    base64::encode(format!("{}:{}:{}", payload, signature, MY_NODE_ID))
}

// Validating (on the receiver node)
fn validate_passport(token: String) -> bool {
    let parts: Vec<&str> = token.split(':').collect();
    if parts.len() != 4 { return false; }
    
    let target = parts[0];
    let expiry = parts[1].parse::<u64>().unwrap_or(0);
    let sig_bytes = base64::decode(parts[2]).unwrap_or_default();
    let sender_id = parts[3];
    
    // 1. Check Expiry
    if expiry < now() { return false; }
    
    // 2. Check Target (Is it for me?)
    if target != MY_NODE_ID { return false; }
    
    // 3. Verify Signature
    let sender_pubkey = cluster::get_pubkey(sender_id);
    let payload = format!("{}:{}", target, expiry);
    let signature = Signature::from_bytes(&sig_bytes).unwrap();
    
    sender_pubkey.verify(payload.as_bytes(), &signature).is_ok()
}
```

# 8. The Cluster (WireGuard Mesh)
*Private internal network connecting Cerberus nodes.*

### 8.1 WireGuard Config: `/etc/wireguard/wg0.conf`
Encrypted P2P Mesh for Redis and Gossip traffic.

```ini
[Interface]
Address = 10.100.0.1/24
ListenPort = 51820
PrivateKey = <Private_Key>
# Firewall Marking for TC eBPF
FwMark = 0x51820

# Peer: Node 2
[Peer]
PublicKey = <Node2_Public_Key>
AllowedIPs = 10.100.0.2/32
Endpoint = node2.infra.corp:51820
PersistentKeepalive = 25

# Peer: Node 3 ...
```

### 8.2 Redis Cluster Config: `/etc/redis/redis.conf`
Optimized for shared state. Default to Standalone mode (supports 1-node setup).

```ini
bind 10.100.0.1      # Listen ONLY on WireGuard Interface
port 6379
cluster-enabled no   # Default: No. Enable only if 3+ nodes are available.
cluster-config-file nodes.conf
cluster-node-timeout 5000
appendonly yes
protected-mode yes   # Only accept connections from bound IPs
maxmemory 2gb        # Cap state size
maxmemory-policy allkeys-lru
```

### 8.3 Health Gossip Protocol (UDP)
Nodes broadcast a tiny JSON packet every 5 seconds to port 9000 (inside tunnel). This is separate from Redis for lightweight routing decisions.

**Struct Definition:**
```rust
#[derive(Serialize, Deserialize, Debug)]
pub struct GossipPacket {
    pub node_id: String,
    pub cpu_load: u8,        // 0-100
    pub tor_health: bool,    // Is local Tor daemon responsive?
    pub active_conns: u32,
    pub timestamp: u64,
}

// Broadcasting (Sender)
async fn broadcast_gossip(state: AppState) {
    let socket = UdpSocket::bind("10.100.0.1:0").await.unwrap();
    let peers = vec!["10.100.0.2:9000", "10.100.0.3:9000"];
    
    loop {
        let packet = GossipPacket {
            node_id: MY_NODE_ID.to_string(),
            cpu_load: system::get_load(),
            tor_health: check_tor_socks(),
            active_conns: haproxy::get_conn_count(),
            timestamp: now(),
        };
        
        let bytes = serde_json::to_vec(&packet).unwrap();
        for peer in &peers {
            socket.send_to(&bytes, peer).await.unwrap();
        }
        sleep(Duration::from_secs(5)).await;
    }
}
```

**Split-Brain Handling:**
If a node stops receiving gossip from >50% of the cluster for 30 seconds:
1.  It marks itself as "Isolated".
2.  It stops issuing Passport tokens (cannot guarantee peer health).
3.  It continues serving local traffic (fail-safe).
4.  It attempts to re-handshake WireGuard peers (`wg set wg0 peer ...`).

---

# 9. The Shielded Origin (Backend)
*The secret server hosting the actual application.*

### Security Architecture
1.  **Network Isolation:** The backend server has **NO public IP**. It is reachable *only* via a private VLAN or Tor circuit.
2.  **Tor Client Authorization:**
    - The Backend Tor service is configured with `HiddenServiceAuthorizeClient`.
    - Only Cerberus nodes possessing the correct `.auth` key can establish a connection.
    - **Unauthorized connection attempts (even if they know the onion address) are dropped by the backend's Tor daemon before any application logic runs.**

### Backend Config (Tor)
```bash
HiddenServiceDir /var/lib/tor/hidden_service/
HiddenServicePort 80 127.0.0.1:8080
HiddenServiceAuthorizeClient stealth cerberus_node_1,cerberus_node_2
```

---

# 10. Threat Model & Attack Kill Table
*What dies where.*

| Attack Type | L2 (XDP) | L3 (TC) | L4 (HAProxy) | L7 (Nginx) | L7+ (Fortify) |
|-------------|----------|---------|--------------|------------|---------------|
| **Volumetric Flood** | ✅ **Kills** | — | — | — | — |
| **SYN Flood** | ✅ **Kills** | — | — | — | — |
| **Relay Churn** | — | ✅ **Slows** | — | — | — |
| **Slowloris** | — | — | ✅ **Kills** | — | — |
| **HTTP Flood** | — | — | ✅ **Limits** | ✅ **Buffers** | — |
| **Bot/Scraper** | — | — | — | — | ✅ **Traps** |
| **Human User** | ❌ | ❌ | ❌ | ❌ | ❌ |

---

# 11. Implementation Phases
*From Zero to Hero.*

### Phase 1: MVP (Connectivity)
- Setup Tor, HAProxy (L4), Nginx (L7), Fortify (Stub).
- Goal: `Hello World` served through .onion.

### Phase 1.5: Kernel Shield
- Deploy XDP program and Kernel TCP sysctls.
- Goal: Survive `hping3` flood test.

### Phase 2-3: Hardening
- Implement HAProxy Stick Tables (Circuit Tracking).
- Implement Nginx Header Scrubbing & Static Gate.

### Phase 4-6: The Brain
- Implement RAM Pool & Ammo Box.
- Deploy 6 CAPTCHA variants.
- Enable Threat Dial Logic.

### Phase 7-10: Scale & Ops
- Setup WireGuard Cluster.
- Enable Passport Protocol.
- Deploy Monitoring Stack (Grafana).

---

# 12. CI/CD & Quality Assurance
*Automated pipelines to ensure security and stability.*

### 12.1 GitHub Actions Workflow: `.github/workflows/cerberus-ci.yml`
This pipeline runs on every Pull Request. It enforces security, compilation, and testing.

```yaml
name: Cerberus CI

on:
  push:
    branches: [ main ]
  pull_request:
    branches: [ main ]
  schedule:
    - cron: '0 3 * * *' # Daily security audit

env:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: "-Dwarnings" # Deny warnings

jobs:
  # --- JOB 1: Security Audit ---
  security:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      # 1. Dependency Scan (Rust)
      - name: Install cargo-audit
        uses: taiki-e/install-action@cargo-audit
      - name: Audit Dependencies
        run: cargo audit
      
      # 2. Secret Scan
      - name: Gitleaks Scan
        uses: gitleaks/gitleaks-action@v2
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          
      # 3. Shell Script Analysis
      - name: Shellcheck
        uses: ludeeus/action-shellcheck@master
        with:
          scandir: './deploy'

  # --- JOB 2: Build & Test (Rust + C) ---
  build-test:
    runs-on: ubuntu-latest
    needs: security
    steps:
      - uses: actions/checkout@v4
      
      # 1. Setup Rust
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      
      # 2. Setup BPF Toolchain
      - name: Install LLVM/Clang
        run: sudo apt-get update && sudo apt-get install -y clang llvm libbpf-dev gcc-multilib
      
      # 3. Linting
      - name: Rustfmt
        run: cargo fmt --all -- --check
      - name: Clippy
        run: cargo clippy --all-features -- -D warnings
        
      # 4. Compile XDP (C Layer)
      - name: Compile XDP
        run: |
          cd src/xdp
          make
          ls -l cerberus_xdp.o
          
      # 5. Compile & Test Fortify (Rust Layer)
      - name: Build Fortify
        run: cargo build --verbose
      - name: Run Unit Tests
        run: cargo test --verbose

  # --- JOB 3: Integration Test (Mocked Tor) ---
  integration:
    runs-on: ubuntu-latest
    needs: build-test
    steps:
      - uses: actions/checkout@v4
      
      # 1. Spin up Environment
      - name: Start Redis & HAProxy
        run: docker-compose -f tests/integration/docker-compose.yml up -d
        
      # 2. Run End-to-End Test
      - name: Test Flow
        run: |
          # Wait for services
          sleep 10
          # Hit Mock Entrypoint
          curl -v http://localhost:8080/
          # Verify Redis State
          redis-cli -h localhost get "circuit:tracking"
```

### 12.2 Code Review Standards
Every Pull Request must meet these criteria before merge.

| Category | Requirement | Check Method |
|----------|-------------|--------------|
| **Security** | No `unsafe` blocks in Rust without comment justification. | Manual Review |
| **Security** | No external runtime dependencies (CDNs, Google Fonts). | `grep` check |
| **Privacy** | No logging of raw IP addresses or request bodies. | Manual Review |
| **Performance** | XDP loops must be bounded (`#pragma unroll`). | Compiler/Verifier |
| **Reliability** | No `unwrap()` or `expect()` in production logic path. | `clippy` |
| **Tor-Native** | Failure handling for high-latency (timeouts > 5s). | Code Logic |

---

# 13. Operational Playbook
*Critical procedures for maintaining Cerberus in production.*

### 13.1 Key Rotation (Backend Tor Auth)
**Trigger:** Periodic (30 days) or Suspected Breach.

1.  **Generate New Keys (on Backend):**
    ```bash
    # Generate new client keypair
    cd /var/lib/tor/hidden_service/
    /usr/bin/tor-gencert --create-identity-key
    ```
2.  **Update `torrc` (Backend):**
    ```bash
    HiddenServiceAuthorizeClient stealth cerberus_node_1_v2,cerberus_node_2_v2
    ```
3.  **Reload Tor (Backend):** `systemctl reload tor`
4.  **Extract New Auth Cookies:** Check `/var/lib/tor/hidden_service/hostname`.
5.  **Distribute to Nodes:** Update `torrc` on all Cerberus nodes with the new `HidServAuth` line.
6.  **Reload Node Tor:** `systemctl reload tor` (Rolling restart).

### 13.2 Redis Cluster Backup
**Trigger:** Before upgrades or weekly.

```bash
# On any Redis node
redis-cli --cluster call 10.100.0.1:6379 BGSAVE
# Copy dump.rdb from /var/lib/redis/ to secure storage
scp /var/lib/redis/dump.rdb user@backup-server:/backups/redis-$(date +%F).rdb
```

### 13.3 Manual Ban (CLI)
**Trigger:** Operator detects abuse not caught by auto-rules.

```bash
# Connect to HAProxy Runtime API
echo "set table be_stick_tables key <circuit_id> data.gpc0 2" | socat stdio /var/run/haproxy.sock
```

### 13.4 XDP Live Update
**Trigger:** Deploying new eBPF filter logic without dropping traffic.

```bash
# 1. Compile new object
make cerberus_xdp.o

# 2. Atomic Replacement (Native Mode)
# ip link set xdp automatically replaces the program atomically
ip link set dev eth0 xdp obj cerberus_xdp.o sec xdp

# 3. Verify Maps
bpftool map show
```

---

# 14. Developer Guide
*Tools for local development and testing.*

### 14.1 Local Dev Stack (`docker-compose.dev.yml`)
Spins up a mock environment with Redis, HAProxy, and a dummy backend.

```yaml
version: '3.8'
services:
  redis:
    image: redis:alpine
    ports: ["6379:6379"]
    
  haproxy:
    image: haproxy:lts
    volumes:
      - ./deploy/haproxy.cfg:/usr/local/etc/haproxy/haproxy.cfg
    ports: ["8080:8080", "8081:8081"]
    
  fortify:
    build: .
    environment:
      - REDIS_URL=redis://redis:6379
      - RUST_LOG=debug
    depends_on: [redis]
    
  backend_mock:
    image: hashicorp/http-echo
    command: -text="Hello from Shielded Backend"
```

### 14.2 Setup Script (`scripts/setup-dev.sh`)
Installs dependencies on Ubuntu/Debian dev machine.

```bash
#!/bin/bash
sudo apt-get update
sudo apt-get install -y \
    build-essential clang llvm libbpf-dev \
    haproxy redis-tools tor \
    pkg-config libssl-dev

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Install BPF Tools
cargo install bpf-linker
```

---

# 15. Security Hardening
*OS-Level protections.*

### 15.1 SSH Hardening (`/etc/ssh/sshd_config`)
```ssh
PermitRootLogin no
PasswordAuthentication no
PubkeyAuthentication yes
AllowUsers admin_user
Port 2222
```

### 15.2 Firewall (UFW)
Strict deny-by-default.

```bash
ufw default deny incoming
ufw default allow outgoing

# Allow SSH (Custom Port)
ufw allow 2222/tcp

# Allow WireGuard (VPN)
ufw allow 51820/udp

# Allow Tor (if not using system Tor)
# Note: Tor usually makes outbound connections, so incoming port open not needed 
# unless running a Relay. For Onion Service, NO incoming ports needed except SSH/WG.

ufw enable
```
