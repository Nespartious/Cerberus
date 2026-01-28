# HAProxy - Layer 1: The Shield

## 📖 User Story

```
As a service operator under DDoS attack
I want HAProxy to track and rate-limit malicious circuits before they reach my application
So that my backend service stays responsive for legitimate users

Acceptance Criteria:
- Circuit ID extracted from Tor PROXY protocol
- Stick tables track per-circuit connection rates and reputation
- Automatic banning of circuits exceeding thresholds
- HAProxy Runtime API allows Fortify to promote/ban circuits dynamically
- Graceful handling of 10,000+ concurrent connections
```

---

## Overview
HAProxy serves as Cerberus's first line of defense after Tor. It operates at both Layer 4 (TCP) and Layer 7 (HTTP), providing connection management, circuit tracking, and aggressive DDoS mitigation. HAProxy receives raw TCP connections with Tor Circuit IDs and makes real-time decisions about whether to queue, rate-limit, or drop connections.

**Position in Stack:** Tor Daemon → **HAProxy (Port 10000)** → Nginx (Port 10001) → Fortify → Target

---

## Core Responsibilities

### 1. Connection Management & Queuing
- **Global Connection Limits**: Enforce strict `maxconn` values to prevent resource exhaustion
- **FIFO/Priority Queuing**: Hold excess connections in managed queues instead of rejecting immediately
- **Connection Throttling**: Per-circuit and global rate limiting
- **Graceful Degradation**: Shed load intelligently during attacks

### 2. Circuit ID Tracking & Reputation
- **Accept Tor PROXY Protocol**: Parse Circuit IDs from Tor daemon connections
- **Stick Tables**: Maintain per-circuit reputation scores and connection counts
- **Circuit Fingerprinting**: Track behavioral patterns (request rate, connection frequency)
- **Dynamic Classification**: Promote good actors, demote abusers, ban malicious circuits

### 3. DDoS Mitigation
- **Connection Flood Protection**: Drop circuits exceeding connection thresholds
- **Slowloris Defense**: Timeout stalled connections aggressively
- **SYN Flood Mitigation**: TCP-level protections (via kernel + HAProxy config)
- **Resource Exhaustion Prevention**: Memory and CPU limits per circuit

### 4. Traffic Routing & Load Balancing
- **Backend Selection**: Route to Nginx or alternative backends
- **Health Checks**: Monitor Nginx/Fortify availability
- **Failover Logic**: Redirect to maintenance page if backends fail
- **Sticky Sessions**: Maintain user sessions via Circuit ID persistence

---

## Key Features & Mechanisms

### Stick Tables (Circuit Reputation System)

```haproxy
# Track circuit behavior in memory
stick-table type string len 64 size 100k expire 30m store conn_cur,conn_rate(10s),http_req_rate(10s),gpc0
```

**Tracked Metrics:**
- `conn_cur`: Current concurrent connections from this circuit
- `conn_rate(10s)`: Connection rate over last 10 seconds
- `http_req_rate(10s)`: HTTP request rate (Layer 7)
- `gpc0`: General Purpose Counter 0 (used for reputation score/ban flag)

**Use Cases:**
- **VIP Promotion**: Circuits with valid CAPTCHA tokens get `gpc0=1` (whitelist flag)
- **Abuse Detection**: Circuits exceeding thresholds get `gpc0=2` (ban flag)
- **Rate Limiting**: Apply different limits based on reputation tier

### Access Control Lists (ACLs)

```haproxy
# Example ACL logic
acl is_banned src_get_gpc0(circuit_tracking) eq 2
acl is_vip src_get_gpc0(circuit_tracking) eq 1
acl too_many_conns src_conn_cur(circuit_tracking) ge 10
acl high_req_rate src_http_req_rate(circuit_tracking) ge 20
```

**Decision Tree:**
1. **Banned** → TCP-RST (instant drop)
2. **VIP** → Fast-path to Nginx (bypass queues)
3. **High Rate + Not VIP** → Tarpit/Deny
4. **Normal** → Standard queue + forward

### Tor PROXY Protocol Integration

HAProxy must accept the Tor PROXY protocol to receive Circuit IDs:

```haproxy
bind 127.0.0.1:10000 accept-proxy
```

**Received Data Format:**
```
PROXY TCP4 127.0.0.1 127.0.0.1 <src_port> 10000
[Circuit ID Header: X-Circuit-ID: <base64-encoded-circuit-id>]
```

**Critical:** Without this, all connections appear as `127.0.0.1`, defeating per-circuit tracking.

### Queue Management

```haproxy
# Global queue settings
maxconn 500               # Total concurrent connections
timeout queue 5s          # Max time in queue before rejection
```

**Queue Behaviors:**
- **Normal Load**: Connections flow through immediately
- **Moderate Load**: Excess connections wait in queue (up to `timeout queue`)
- **Heavy Load**: Queue fills → new connections get 503 errors
- **Attack Mode**: Ban logic triggers → malicious circuits get TCP-RST before queuing

### Timeout Configuration

```haproxy
timeout connect 5s        # Backend connection timeout
timeout client 30s        # Client idle timeout
timeout server 30s        # Backend server timeout
timeout http-request 10s  # Time to receive full HTTP request
timeout http-keep-alive 2s # Keep-alive timeout (short for Tor)
```

**Defense Rationale:**
- Short timeouts prevent Slowloris attacks
- `http-request` timeout kills slow POST attacks
- Aggressive `http-keep-alive` limits connection reuse abuse

---

## Attack Mitigation Strategies

### 1. Connection Flood (Rapid Circuit Connections)

**Detection:**
```haproxy
acl conn_flood src_conn_rate(circuit_tracking) ge 10
```

**Response:**
- First offense: Tarpit (slow down responses)
- Repeat offense: Set `gpc0=2` (ban) → TCP-RST all future connections

### 2. Slowloris (Slow Header/Body Attacks)

**Detection:**
- `timeout http-request 10s` enforces fast header delivery
- Nginx provides secondary defense

**Response:**
- HAProxy kills connection if headers not received within 10s
- No resources wasted on stalled sockets

### 3. HTTP Request Flood

**Detection:**
```haproxy
acl req_flood src_http_req_rate(circuit_tracking) ge 50
```

**Response:**
- Non-VIP circuits exceeding 50 req/10s get denied
- VIP circuits (CAPTCHA-validated) get higher limits (e.g., 200 req/10s)

### 4. Resource Exhaustion (Per-Circuit Limits)

**Detection:**
```haproxy
acl too_many_conns src_conn_cur(circuit_tracking) ge 15
```

**Response:**
- Deny new connections from circuits with 15+ concurrent connections
- Forces attackers to cycle circuits (making attacks more expensive)

### 5. Layer 7 Attacks (HTTP Method Abuse)

**Detection:**
```haproxy
acl invalid_method method POST PUT DELETE PATCH
acl is_captcha_submission path_beg /verify-captcha
acl bad_request invalid_method !is_captcha_submission
```

**Response:**
- Only allow POST to `/verify-captcha` endpoint
- Block all other POST/PUT/DELETE requests before Nginx

---

## Configuration Sections

### Frontend (Public-Facing)

```haproxy
frontend tor_ingress
    bind 127.0.0.1:10000 accept-proxy
    mode http
    
    # Stick table for circuit tracking
    stick-table type string len 64 size 100k expire 30m store conn_cur,conn_rate(10s),http_req_rate(10s),gpc0
    
    # Track by Circuit ID (from Tor PROXY protocol header)
    tcp-request connection track-sc0 hdr(X-Circuit-ID)
    
    # Global limits
    maxconn 500
    timeout http-request 10s
    
    # ACLs (Access Control Logic)
    acl is_banned src_get_gpc0(circuit_tracking) eq 2
    acl is_vip src_get_gpc0(circuit_tracking) eq 1
    acl conn_flood src_conn_rate(circuit_tracking) ge 10
    acl req_flood src_http_req_rate(circuit_tracking) ge 50
    
    # Decision logic
    tcp-request connection reject if is_banned
    http-request deny if req_flood !is_vip
    http-request deny if conn_flood !is_vip
    
    # Route to backend
    default_backend nginx_layer
```

### Backend (Nginx)

```haproxy
backend nginx_layer
    mode http
    balance roundrobin
    
    # Health check
    option httpchk GET /health
    
    # Backend server
    server nginx1 127.0.0.1:10001 check inter 5s
    
    # Timeouts
    timeout connect 5s
    timeout server 30s
    timeout http-keep-alive 2s
```

### Stats & Monitoring

```haproxy
listen stats
    bind 127.0.0.1:9000
    mode http
    stats enable
    stats uri /stats
    stats refresh 5s
    stats show-legends
    stats show-node
```

**Accessible at:** `http://127.0.0.1:9000/stats`

---

## Integration with Fortify (Rust Layer)

### Fortify → HAProxy Communication (Stick Table Updates)

Fortify needs to update HAProxy stick tables when:
1. **CAPTCHA Validated**: Promote circuit to VIP (`gpc0=1`)
2. **Suspicious Behavior Detected**: Ban circuit (`gpc0=2`)
3. **Token Expired**: Demote circuit back to normal (`gpc0=0`)

**Method:** HAProxy Runtime API (Unix Socket)

```bash
# Example: Promote circuit to VIP
echo "set table tor_ingress key <circuit-id> data.gpc0 1" | socat stdio /var/run/haproxy.sock
```

**Fortify Implementation:**
```rust
// fortify/src/haproxy/client.rs
pub fn promote_circuit_to_vip(circuit_id: &str) -> Result<()> {
    let cmd = format!("set table tor_ingress key {} data.gpc0 1\n", circuit_id);
    // Send to HAProxy socket at /var/run/haproxy.sock
}

pub fn ban_circuit(circuit_id: &str) -> Result<()> {
    let cmd = format!("set table tor_ingress key {} data.gpc0 2\n", circuit_id);
    // Send to HAProxy socket
}
```

---

## Logging & Observability

### Log Format

```haproxy
log-format "%ci:%cp [%tr] %ft %b/%s %TR/%Tw/%Tc/%Tr/%Ta %ST %B %CC %CS %tsc %ac/%fc/%bc/%sc/%rc %sq/%bq %hr %hs {%[ssl_c_verify]} %{+Q}r %[var(txn.circuit_id)]"
```

**Key Fields:**
- `%ci:%cp`: Client IP:Port (will be 127.0.0.1 from Tor)
- `%[var(txn.circuit_id)]`: Tor Circuit ID (critical for tracking)
- `%ac/%fc`: Active/Frontend connections
- `%ST`: HTTP status code
- `%Tr`: Time in queue

### Metrics to Monitor

1. **Connection Metrics**
   - Total connections (`%ac`)
   - Queue length (`%sq`)
   - Connection rate per circuit

2. **Reputation Metrics**
   - VIP count (circuits with `gpc0=1`)
   - Banned count (circuits with `gpc0=2`)
   - Circuit churn rate (new circuits/min)

3. **Attack Indicators**
   - Spike in connection rate
   - High TCP-RST count (banned circuits retrying)
   - Sudden increase in unique circuit IDs (Sybil attack)

---

## Security Hardening

### 1. HAProxy Process Isolation
```bash
# Run as non-privileged user
User haproxy
Group haproxy

# Chroot jail (optional)
chroot /var/lib/haproxy
```

### 2. Kernel Tuning (SYN Flood Protection)
```bash
# /etc/sysctl.conf
net.ipv4.tcp_syncookies = 1
net.ipv4.tcp_max_syn_backlog = 8192
net.ipv4.tcp_synack_retries = 2
net.core.netdev_max_backlog = 5000
```

### 3. Rate Limiting at Kernel Level (NFTables)
```nft
# Limit connections to HAProxy port
table inet filter {
    chain input {
        type filter hook input priority 0; policy drop;
        
        # Allow localhost
        iif "lo" accept
        
        # Limit Tor connections (if exposed, shouldn't be)
        tcp dport 10000 ip saddr 127.0.0.1 accept
    }
}
```

---

## Testing & Validation

### Sprint 1 Tests

1. **Circuit ID Extraction Test**
   ```bash
   # Check HAProxy logs for Circuit IDs (not 127.0.0.1)
   tail -f /var/log/haproxy.log | grep "X-Circuit-ID"
   ```

2. **Stick Table Population Test**
   ```bash
   # Query stick table via stats socket
   echo "show table tor_ingress" | socat stdio /var/run/haproxy.sock
   ```

3. **Connection Limit Test**
   ```bash
   # Simulate connection flood
   for i in {1..600}; do curl -x socks5h://127.0.0.1:9050 http://<onion>/ & done
   # Expected: 503 errors after maxconn reached
   ```

4. **Ban Logic Test**
   ```bash
   # Manually ban a circuit via Fortify API
   # Verify circuit gets TCP-RST on next connection
   ```

---

## Performance Tuning

### Expected Capacity
- **Baseline**: 500-1000 concurrent connections (conservative)
- **Optimized**: 5000-10000 concurrent connections (with kernel tuning)
- **Hardware**: 2 CPU cores, 2GB RAM sufficient for stick tables

### Bottlenecks
1. **Stick Table Size**: 100k entries = ~10MB RAM
2. **Log I/O**: Async logging to avoid blocking
3. **Backend Latency**: Nginx response time affects HAProxy queue

### Scaling Strategies
- **Vertical**: Increase `maxconn` and stick table size
- **Horizontal**: Multiple HAProxy instances (requires shared stick tables via peers)
- **Hybrid**: HAProxy peer clustering for stick table replication

---

## Critical Configuration Checklist

- [ ] `accept-proxy` enabled on frontend bind
- [ ] Stick table defined with circuit tracking
- [ ] `tcp-request connection track-sc0` configured for Circuit ID
- [ ] Ban logic (gpc0=2 → reject) implemented
- [ ] VIP logic (gpc0=1 → bypass limits) implemented
- [ ] Aggressive timeouts configured (http-request, client, server)
- [ ] Backend health checks enabled
- [ ] Stats socket enabled for Fortify integration
- [ ] Logging configured with Circuit ID field
- [ ] HAProxy socket permissions set for Fortify user

---

## Future Enhancements (Post-Sprint 1)

1. **Geo-Reputation**: Track circuit behavior across sessions (persistent stick tables)
2. **Machine Learning Integration**: Feed metrics to Fortify for anomaly detection
3. **Dynamic Threshold Adjustment**: Auto-tune rate limits based on attack severity
4. **Circuit Clustering**: Identify Sybil attacks via circuit correlation
5. **Multi-Tier Queuing**: Separate queues for VIP, normal, and suspect traffic

---

## References
- HAProxy PROXY Protocol: https://www.haproxy.org/download/1.8/doc/proxy-protocol.txt
- Tor PROXY Protocol: https://gitlab.torproject.org/tpo/core/tor/-/wikis/doc/tor-manpage
- Stick Tables: https://www.haproxy.com/blog/introduction-to-haproxy-stick-tables/
- DDoS Mitigation: https://www.haproxy.com/blog/application-layer-ddos-attack-protection-with-haproxy/
