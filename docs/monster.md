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

### Purpose
The absolute first line of defense. Filters packets at line rate (10M+ PPS) before the OS kernel allocates memory for socket buffers.

### Defense Capabilities
1.  **Volumetric Floods:** Drops massive SYN floods instantly.
2.  **UDP Floods:** Drops all UDP packets *except* port 51820 (WireGuard).
3.  **Spoofing:** Drops packets from invalid/private ranges on the public interface.
4.  **Port Scanning:** Silently drops traffic to non-whitelisted ports (only 80/443/51820 allowed).

### Fine Details & Implementation
- **Hook Point:** `XDP_DRV` (Driver mode) for maximum performance.
- **Map Structure:** `BPF_MAP_TYPE_LRU_HASH` to track per-relay IP packet rates.
- **Action:** `XDP_DROP` (silent drop) vs `XDP_PASS` (allow).

```c
// Concept: XDP Drop Logic
if (ip_protocol == IPPROTO_UDP && dest_port != 51820) {
    return XDP_DROP; // Drop all non-WireGuard UDP
}
if (syn_flood_detected(src_ip)) {
    return XDP_DROP; // Drop SYN flood before memory allocation
}
```

---

# 4. L3/L4: The Flow Shaper (TC eBPF & Kernel TCP)
*Located at the Traffic Control (TC) layer and Kernel TCP stack.*

### TC eBPF Policy
- **Role:** Stateful traffic shaping. Unlike XDP (stateless), TC knows about flows.
- **Relay Penalty:** If a Tor relay IP sends high connection churn, TC adds **50-200ms artificial latency** to its packets.
- **Signaling:** Sets `skb->mark` for HAProxy to read later.

### Kernel TCP Tuning (Sysctls)
Hardens the Linux TCP stack against resource exhaustion.

```bash
# /etc/sysctl.d/99-cerberus-tcp.conf

# SYN Flood Protection
net.ipv4.tcp_syncookies = 1
net.ipv4.tcp_max_syn_backlog = 4096
net.ipv4.tcp_synack_retries = 2

# Aggressive Cleanup (Anti-Slowloris/Exhaustion)
net.ipv4.tcp_fin_timeout = 15        # Close FIN-WAIT-2 sockets fast
net.ipv4.tcp_keepalive_time = 60     # Disconnect idle fast
net.ipv4.tcp_max_tw_buckets = 1440000 # Max TIME_WAIT sockets
net.ipv4.tcp_tw_reuse = 1            # Reuse TIME_WAIT for new conns

# Tor Optimization
net.core.somaxconn = 65535           # Max listen queue
net.core.netdev_max_backlog = 16384  # Max packet queue
```

---

# 5. L4: The Governor (HAProxy)
*Located at userland TCP/HTTP level.*

### Architecture: The Two-Lane System
HAProxy splits traffic into two distinct lanes based on port.

1.  **Lane A (Public): Port 8080**
    - **Target:** New, unverified connections.
    - **Policy:** Strict rate limits, lower connection caps, aggressive timeouts.
    - **Goal:** Filter out basic attacks and forced-browsing bots.

2.  **Lane B (VIP/Passport): Port 8081**
    - **Target:** Users with valid Passport tokens (redirected from peers) or VIP circuits.
    - **Policy:** Higher connection limits, relaxed timeouts.
    - **Goal:** Ensure smooth experience for verified humans.

### Circuit Tracking (Stick Tables)
Cerberus tracks **Tor Circuit IDs**, not IP addresses.

```haproxy
# Track circuit behavior
stick-table type string len 64 size 1m expire 30m store conn_cur,conn_rate(10s),http_req_rate(10s),gpc0

# Access Control Logic
acl is_banned src_get_gpc0(tor_ingress) eq 2
acl is_vip    src_get_gpc0(tor_ingress) eq 1
tcp-request connection reject if is_banned
```

### Defense Configuration
- **Max Connections:** 20,000 per port (configurable).
- **Slowloris Kill:** `timeout http-request 3s` (Headers must arrive in 3s).
- **Tor Smashing:** Prevents local Tor daemon FD exhaustion by queuing connections in HAProxy.
- **HTTP Normalization:** Strips duplicate headers, enforces CRLF, rejects malformed HTTP methods *before* Nginx sees them.

---

# 6. L7: The Gatekeeper (Nginx)
*Located at HTTP level, behind HAProxy.*

### Core Responsibilities
1.  **Protocol Sanitization:** Removes fingerprintable headers (`User-Agent`, `Accept-Language`).
2.  **Static Gate:** Serves static assets (images, CSS, `captcha.html`) directly from RAM cache.
3.  **Buffer Defense:** Prevents Slowloris body attacks and large payload DoS.

### Configuration Hardening
```nginx
# Anti-Fingerprinting
proxy_set_header User-Agent "Mozilla/5.0 (Windows NT 10.0; rv:109.0) Gecko/20100101 Firefox/115.0";
proxy_set_header Accept-Language "en-US,en;q=0.5";
proxy_set_header Via "";

# Buffer & Timeouts
client_body_buffer_size 16k;       # Max POST buffer
client_max_body_size 1m;           # Max upload size (tiny for CAPTCHAs)
client_body_timeout 5s;            # Kill slow POSTs
client_header_timeout 5s;          # Kill slow headers
```

### Static Gate Logic
Nginx attempts to serve content from disk first. Only specific API calls reach Fortify.
- `GET /` -> Serves `captcha.html` (Static)
- `POST /verify` -> Proxies to Fortify (Dynamic)
- `GET /assets/*` -> Serves static files (Static)

---

# 7. L7+: Fortify (The Logic Engine)
*Custom Rust binary. The Brain of the operation.*

### A. The Logic Core (Decision Matrix)
Fortify monitors system health (CPU/RAM) and makes admission decisions:
- **Load < 80%:** Normal Mode. Serve CAPTCHA.
- **Load > 90%:** Shed Mode. Issue **Passport Tokens** redirecting users to healthy peer nodes.
- **Load > 99%:** Bunker Mode. Serve **Queue Page** (Static HTML).

### B. The CAPTCHA System (RAM Pool / Ammo Box)
Instead of generating CAPTCHAs on-demand (slow, CPU intensive), Cerberus uses a pre-generation strategy.

1.  **RAM Pool:**
    - Holds **50,000 – 100,000** pre-generated CAPTCHA challenges in memory.
    - **Zero-Allocation:** Serving a CAPTCHA takes nanoseconds (pop from stack).

2.  **The Ammo Box (Persistence):**
    - **Safe Dump:** Periodically (e.g., every 5 mins) and on graceful shutdown, the pool is serialized to `ammo.bin`.
    - **Fast Load:** On startup, `ammo.bin` is mmapped/loaded instantly.
    - **Re-Hashing:** Solutions are re-salted with a new ephemeral key on boot to prevent replay attacks.

### C. The Queue System (No-JS)
- **Mechanism:** A static HTML page served from Nginx RAM cache.
- **Logic:** Uses standard `<meta http-equiv="refresh" content="15">`.
- **Why:** Keeps the browser waiting without holding a TCP socket open on the server.

### D. The Passport Protocol (Cluster Routing)
Used when a node is shedding load.
- **Trigger:** Node Load > 90%.
- **Action:** Generates a signed URL: `https://peer-node.onion/passport?token=<signature>`.
- **Crypto:** Ed25519 signature containing `(expiry_timestamp + target_node_id)`.
- **Result:** Peer node validates signature and bypasses CAPTCHA for that user.

---

# 8. The Cluster (WireGuard Mesh)
*Private internal network connecting Cerberus nodes.*

### Architecture
- **Transport:** WireGuard (UDP 51820). Fast, encrypted, kernel-level.
- **Topology:** Peer-to-Peer Mesh (Full or Partial).
- **Communication:**
    1.  **UDP Gossip (Health):** Every 5s, nodes broadcast tiny JSON packets (`NodeID`, `CPU_Load`, `Tor_Health`). Used for routing decisions.
    2.  **Redis Cluster (State):** Shared storage for Stick Tables, CAPTCHA sessions, and Threat Dial state.

### Defenses
- **Blind Redirect Prevention:** Gossip ensures nodes never redirect users to a dead peer.
- **Split-Brain Recovery:** Gossip protocol allows nodes to re-discover peers automatically.

---

# 9. The Shielded Origin (Backend)
*The secret server hosting the actual application.*

### Architecture
- **Location:** Separate server/container, IP unknown to public.
- **Access Control:** Accepts traffic *only* via Tor Hidden Service protocol from authenticated Cerberus nodes.
- **Tor Client Auth:** Cerberus nodes possess a cryptographic key required to connect. Unauthorized connections (even if they know the onion address) are dropped by the backend's Tor daemon.

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
