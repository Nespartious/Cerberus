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

### Implementation: `cerberus_xdp_kern.c`
The following eBPF program drops volumetric floods before the OS allocates memory.

```c
#include <linux/bpf.h>
#include <linux/if_ether.h>
#include <linux/ip.h>
#include <linux/udp.h>
#include <linux/tcp.h>
#include <bpf/bpf_helpers.h>

// Map for tracking per-IP packet rates (LRU Hash)
struct {
    __uint(type, BPF_MAP_TYPE_LRU_HASH);
    __uint(max_entries, 100000);
    __type(key, __u32);   // Source IP
    __type(value, __u64); // Packet Count / Timestamp
} rate_limit_map SEC(".maps");

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

    // 2. Protocol Filter
    // Allow TCP (Tor/HAProxy)
    if (ip->protocol == IPPROTO_TCP) {
        // Todo: Add SYN flood check here
        return XDP_PASS;
    }
    
    // Allow UDP only on WireGuard port (51820)
    if (ip->protocol == IPPROTO_UDP) {
        struct udphdr *udp = (void *)(ip + 1);
        if ((void *)(udp + 1) > data_end) return XDP_DROP;
        if (udp->dest != bpf_htons(51820)) return XDP_DROP;
        return XDP_PASS;
    }

    // Drop everything else (ICMP, etc.)
    return XDP_DROP;
}
char _license[] SEC("license") = "GPL";
```

### Deployment: `cerberus-init.sh` (XDP Auto-Detect)
This script detects the NIC driver and loads XDP in the best supported mode.

```bash
#!/bin/bash
INTERFACE=$(ip route get 8.8.8.8 | awk '{print $5; exit}')
DRIVER=$(ethtool -i $INTERFACE | grep driver | awk '{print $2}')

echo "Detected Interface: $INTERFACE (Driver: $DRIVER)"

# Try Native Mode first (Hardware Offload or Driver support)
echo "Attempting XDP Native Mode..."
ip link set dev $INTERFACE xdp obj cerberus_xdp.o sec xdp 2>/dev/null

if [ $? -eq 0 ]; then
    echo "✅ XDP Native Mode Loaded."
else
    echo "⚠️ Native Mode failed. Falling back to Generic Mode (SKB)..."
    # Fallback to Generic Mode (Works on any NIC, but slower)
    ip link set dev $INTERFACE xdpdgeneric obj cerberus_xdp.o sec xdp
    echo "✅ XDP Generic Mode Loaded."
fi
```

---

# 4. L3/L4: The Flow Shaper & Kernel Tuning
*Optimization of the Linux Network Stack.*

### Kernel Tuning: `/etc/sysctl.d/99-cerberus.conf`
Apply these settings to harden the TCP stack against exhaustion attacks.

```ini
# --- SYN Flood Protection ---
# Enable SYN cookies (fallback when queue is full)
net.ipv4.tcp_syncookies = 1
# Increase SYN backlog (default 1024 -> 4096)
net.ipv4.tcp_max_syn_backlog = 4096
# Reduce SYN-ACK retries (don't wait for spoofed IPs)
net.ipv4.tcp_synack_retries = 2

# --- Resource Exhaustion Defense ---
# Fast recycling of TIME_WAIT sockets
net.ipv4.tcp_tw_reuse = 1
# Max TIME_WAIT sockets (prevent RAM exhaustion)
net.ipv4.tcp_max_tw_buckets = 1440000
# Aggressive FIN timeout (close dead connections fast)
net.ipv4.tcp_fin_timeout = 15
# Keepalive: Check every 60s, fail after 3 probes
net.ipv4.tcp_keepalive_time = 60
net.ipv4.tcp_keepalive_probes = 3
net.ipv4.tcp_keepalive_intvl = 10

# --- Tor High-Load Optimization ---
# Increase max open files (file descriptors)
fs.file-max = 100000
# Max connection queue
net.core.somaxconn = 65535
# Max packet backlog
net.core.netdev_max_backlog = 16384
# Local port range (ephemeral ports)
net.ipv4.ip_local_port_range = 1024 65535
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
    bind 127.0.0.1:8081 accept-proxy
    
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

### A. The Ammo Box (RAM Pool Architecture)
Zero-allocation CAPTCHA serving using a pre-filled Ring Buffer.

```rust
// Core Structs
pub struct AmmoBox {
    pool: CrossbeamQueue<CaptchaChallenge>,
    capacity: usize,
    last_dump: Instant,
}

pub struct CaptchaChallenge {
    pub id: Uuid,
    pub image_png: Vec<u8>,
    pub answer_hash: [u8; 32], // Sha256(answer + salt)
    pub variant: CaptchaVariant,
}

// Background Worker: The "Reloader"
// Keeps the pool full and dumps to disk when idle
async fn maintain_ammo_box(ammo: Arc<AmmoBox>) {
    loop {
        // 1. Refill if low
        if ammo.pool.len() < ammo.capacity * 0.8 {
            let batch = generate_batch(100); // Generate 100 CAPTCHAs
            ammo.pool.push(batch);
        }
        
        // 2. Safe Dump Trigger (Capacity-Based)
        // Dump if FULL and hasn't dumped in 10 mins
        if ammo.pool.is_full() && ammo.last_dump.elapsed() > Duration::from_secs(600) {
             serialize_to_disk(&ammo.pool, "ammo.bin").await;
             ammo.last_dump = Instant::now();
        }
        
        sleep(Duration::from_secs(1)).await;
    }
}
```

### B. Logic Core & Load Shedding
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
            let passport = crypto::mint_passport(peer.id);
            // Redirect to peer with passport token
            Response::redirect(format!("http://{}/?passport_token={}", peer.onion, passport))
        },
        
        // Bunker Mode: Queue (Meta-Refresh)
        _ => Response::html(QUEUE_PAGE_HTML), // Static page with <meta refresh=15>
    }
}
```

### C. The Passport Protocol (Crypto)
Cryptographic tokens for inter-node trust.

```rust
// Minting a Passport
fn mint_passport(target_node: NodeId) -> String {
    let expiry = now() + 30; // Valid for 30 seconds
    let payload = format!("{}:{}", target_node, expiry);
    let signature = ed25519::sign(&MY_PRIVATE_KEY, payload.as_bytes());
    
    base64::encode(format!("{}:{}:{}", payload, signature, MY_NODE_ID))
}

// Validating (on the receiver node)
fn validate_passport(token: String) -> bool {
    let (payload, sig, sender_id) = parse_token(token);
    let sender_pubkey = cluster::get_pubkey(sender_id);
    
    // 1. Check Signature
    if !ed25519::verify(sender_pubkey, payload, sig) { return false; }
    
    // 2. Check Expiry
    if payload.expiry < now() { return false; }
    
    // 3. Check Target (Is it for me?)
    if payload.target_node != MY_NODE_ID { return false; }
    
    return true;
}
```

# 8. The Cluster (WireGuard Mesh)
*Private internal network connecting Cerberus nodes.*

### WireGuard Config: `/etc/wireguard/wg0.conf`
Encrypted P2P Mesh for Redis and Gossip traffic.

```ini
[Interface]
Address = 10.100.0.1/24
ListenPort = 51820
PrivateKey = <Private_Key>

# Peer: Node 2
[Peer]
PublicKey = <Node2_Public_Key>
AllowedIPs = 10.100.0.2/32
Endpoint = node2.infra.corp:51820
PersistentKeepalive = 25

# Peer: Node 3 ...
```

### Redis Cluster Config: `/etc/redis/redis.conf`
 Optimized for shared state.

```ini
bind 10.100.0.1      # Listen ONLY on WireGuard Interface
port 6379
cluster-enabled yes
cluster-config-file nodes.conf
cluster-node-timeout 5000
appendonly yes
protected-mode yes   # Only accept connections from bound IPs
```

### Health Gossip (UDP 51820)
Nodes broadcast a tiny JSON packet every 5 seconds to port 51820 (reusing WG port? No, must use different port inside tunnel, e.g., 9000).

```json
{
  "node_id": "node-01",
  "cpu_load": 45,
  "tor_health": "healthy",
  "timestamp": 1738200000
}
```

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
