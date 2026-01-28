# Virtual Queue System with Proof-of-Work Priority

## Overview
Cerberus's queue system offloads connection management from the server to the client's browser using a **Virtual Waiting Room**. This approach prevents malicious actors from exhausting server resources while providing fair access for legitimate users during high-load scenarios.

---

## Problem Statement

### Traditional FIFO Queue Issues
1. **Bots monopolize queue**: Attackers flood the queue, legitimate users get 503 errors
2. **Server resource exhaustion**: Each queued connection holds a TCP socket (limited resource)
3. **Unfair access**: Real users at the back of the queue may never get served
4. **No prioritization**: Cannot distinguish between verified humans and potential bots

### Traditional Random Queue Issues
1. **No guarantee of service**: Unlucky legitimate users might wait indefinitely
2. **Still consumes server resources**: Queue entries hold memory/connections
3. **No incentive for patience**: Users spam refresh, making problem worse

---

## Solution: Hybrid Virtual Queue + Proof-of-Work

### Architecture

```
User Request
     ↓
[HAProxy: Check Circuit Reputation]
     ↓
   Is VIP? ────Yes──→ Instant Access (Lane 1)
     ↓ No
[Check Server Load]
     ↓
 Under maxconn? ──Yes──→ Forward to Nginx
     ↓ No
[Return "Virtual Queue" HTML Page]
     ↓
User waits in browser (meta refresh auto-retry)
     ↓
[Optional: Complete PoW Challenge]
     ↓
[Retry with Queue Token]
     ↓
[HAProxy: Validate Token + Age]
     ↓
Valid Token? ──Yes──→ Priority Access (Lane 2)
     ↓ No
[Continue Virtual Queue Wait]
```

---

## Queue Lanes (Priority System)

### Lane 1: VIP (Validated Users)
- **Who**: Circuits with valid CAPTCHA verification (HAProxy `gpc0=1`)
- **Priority**: **Instant access** (bypass all queues)
- **Expiry**: VIP status lasts 30 minutes of inactivity
- **HAProxy Logic**: `if is_vip → frontend priority path`

### Lane 2: PoW Completed (Patient Users)
- **Who**: Users who waited in virtual queue AND completed lightweight PoW
- **Priority**: **High priority** (gets next available slot)
- **PoW Mechanism**: Browser-side HTML form submission after timer expires
- **HAProxy Logic**: `if valid_queue_token → medium priority path`

### Lane 3: New/Unverified
- **Who**: First-time visitors, no queue token
- **Priority**: **Normal** (must wait if server at capacity)
- **Behavior**: Receives virtual queue page, begins wait cycle

---

## Virtual Queue Implementation

### Server-Side (HAProxy + Nginx)

**HAProxy Configuration:**
```haproxy
frontend tor_ingress
    bind 127.0.0.1:10000 accept-proxy
    
    # Check VIP status (Lane 1)
    acl is_vip src_get_gpc0(circuit_tracking) eq 1
    acl server_full fe_conn ge 9500  # 95% of maxconn 10000
    
    # Check queue token (Lane 2)
    acl has_queue_token hdr_sub(Cookie) queue_token=
    acl valid_token lua.validate_queue_token
    
    # Routing logic
    use_backend nginx_layer if is_vip
    use_backend nginx_layer if valid_token
    use_backend virtual_queue if server_full
    
    default_backend nginx_layer

backend nginx_layer
    mode http
    maxconn 9500
    server nginx1 127.0.0.1:10001 check

backend virtual_queue
    mode http
    server queue_page 127.0.0.1:10003 check  # Lightweight queue page server
```

**Nginx Virtual Queue Server (Port 10003):**
```nginx
server {
    listen 127.0.0.1:10003;
    root /var/www/cerberus/queue;
    
    location / {
        try_files /queue.html =503;
        
        # No caching
        add_header Cache-Control "no-store, must-revalidate";
        add_header Pragma "no-cache";
        
        # Set queue token cookie
        add_header Set-Cookie "queue_token=$request_id; Path=/; Max-Age=300; SameSite=Strict";
    }
}
```

### Client-Side (Pure HTML, No JavaScript)

**Virtual Queue Page (`queue.html`):**
```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta http-equiv="refresh" content="5">  <!-- Auto-retry every 5 seconds -->
    <title>Please Wait - High Traffic</title>
    <style>
        body {
            font-family: sans-serif;
            text-align: center;
            padding: 50px;
            background: #1a1a1a;
            color: #fff;
        }
        .queue-box {
            max-width: 600px;
            margin: 0 auto;
            padding: 30px;
            background: #2a2a2a;
            border-radius: 8px;
        }
        .spinner {
            border: 4px solid #444;
            border-top: 4px solid #fff;
            border-radius: 50%;
            width: 50px;
            height: 50px;
            animation: spin 1s linear infinite;
            margin: 20px auto;
        }
        @keyframes spin {
            0% { transform: rotate(0deg); }
            100% { transform: rotate(360deg); }
        }
        .info {
            margin-top: 20px;
            font-size: 14px;
            color: #aaa;
        }
    </style>
</head>
<body>
    <div class="queue-box">
        <h1>🛡️ Cerberus Protection</h1>
        <div class="spinner"></div>
        <h2>Please Wait</h2>
        <p>The service is experiencing high traffic. You will be automatically connected when capacity is available.</p>
        
        <div class="info">
            <p><strong>This page will refresh automatically every 5 seconds.</strong></p>
            <p>Please do not close this tab or click refresh manually.</p>
            <p>To skip the queue, complete the CAPTCHA verification once connected.</p>
        </div>
    </div>
    
    <!-- Optional: PoW form for priority access -->
    <noscript>
        <!-- Pure HTML PoW: User must wait minimum time before form appears -->
        <!-- Implemented via server-side timing validation -->
    </noscript>
</body>
</html>
```

---

## Proof-of-Work Mechanism (Optional Priority)

### Lightweight PoW Challenge

**Goal**: Prove user is willing to wait + consume client resources (not a bot spamming)

**Implementation**:
1. **Server generates challenge**: `SHA256(circuit_id + timestamp + secret)` → first 4 hex digits = target
2. **Client must wait**: Minimum 30 seconds in queue before PoW form appears
3. **Client submits nonce**: Form field with random value
4. **Server validates**: `SHA256(challenge + nonce)` starts with target prefix?

**HAProxy Lua Script (PoW Validation):**
```lua
-- /etc/haproxy/validate_queue_token.lua
core.register_fetches("validate_queue_token", function(txn)
    local cookie = txn.sf:req_cook("queue_token")
    if not cookie then
        return 0
    end
    
    -- Parse token: timestamp|circuit_id|signature
    local parts = split(cookie, "|")
    local timestamp = tonumber(parts[1])
    local circuit_id = parts[2]
    local signature = parts[3]
    
    -- Check age (max 5 minutes)
    local age = os.time() - timestamp
    if age > 300 or age < 0 then
        return 0
    end
    
    -- Verify signature (HMAC)
    local expected_sig = hmac_sha256(secret_key, timestamp .. circuit_id)
    if signature == expected_sig then
        return 1
    end
    
    return 0
end)
```

---

## Token-Based Priority Access

### Queue Token Structure

```
queue_token = <timestamp>|<circuit_id>|<hmac_signature>
```

**Example**:
```
queue_token=1738123456|a3f9b2c1d4e5f6g7|9a8b7c6d5e4f3g2h1
```

**Token Lifecycle**:
1. **Issued**: When user first hits virtual queue
2. **Refreshed**: Every meta refresh (5s), updated timestamp
3. **Validated**: HAProxy checks age + signature
4. **Expired**: After 5 minutes (forces re-queue)

**Priority Calculation**:
```
priority_score = (current_time - token_timestamp) * 10
# Older tokens = higher score = priority access
```

### Token Security

**Prevent Token Forgery**:
- HMAC signature using server secret key
- Circuit ID binding (token only valid for issuing circuit)
- Timestamp validation (max age enforcement)
- One-time use (optional: burn token after successful access)

**Prevent Token Sharing**:
- Bind to Tor Circuit ID (cannot transfer between users)
- Short expiry (5 min) limits sharing window

---

## Resource Management

### Server-Side Efficiency

**Virtual Queue = Zero Server Load**:
- No TCP connections held (client disconnects after receiving queue page)
- No memory allocation (stateless tokens)
- No database queries (token validation in Lua)

**Capacity Calculation**:
```
maxconn = 10000
queue_trigger = 9500 (95% threshold)
vip_reserved = 500 (5% reserved for validated users)

Effective capacity:
- 9500 concurrent regular users
- 500 reserved VIP slots (always available)
```

### Client-Side Experience

**Average Wait Time**:
```
wait_time = (queue_position / throughput_rate) seconds

Example:
- Queue position: 1000
- Throughput: 100 requests/sec
- Wait time: 10 seconds
```

**Meta Refresh Strategy**:
- 5 second refresh interval (balance between responsiveness and server load)
- Exponential backoff during severe attacks (5s → 10s → 20s)
- Reset to 5s once capacity available

---

## Attack Mitigation

### Attack: Bot Floods Virtual Queue

**Defense**:
- Tokens expire (5 min) → bots must re-request
- Circuit reputation tracking → ban malicious circuits
- Rate limit queue page requests (max 1 per 5s per circuit)

### Attack: Token Harvesting

**Defense**:
- HMAC prevents forgery
- Circuit ID binding prevents sharing
- One-time use (burn on successful access)

### Attack: PoW Grinding

**Defense**:
- PoW difficulty adjusts based on server load (higher load = harder PoW)
- Minimum wait time enforced (30s) before PoW form appears
- Diminishing returns (completing PoW only gives marginal priority)

### Attack: Meta Refresh Spam

**Defense**:
- HAProxy rate limits queue page requests
- Excessive refresh attempts → circuit ban
- Server-side timing validation (token age must be realistic)

---

## Configuration Examples

### HAProxy Settings

```haproxy
global
    maxconn 10000
    lua-load /etc/haproxy/validate_queue_token.lua

frontend tor_ingress
    bind 127.0.0.1:10000 accept-proxy
    maxconn 10000
    
    # Stick table for circuit tracking
    stick-table type string len 64 size 100k expire 30m store gpc0,conn_cur
    
    # ACLs
    acl is_vip src_get_gpc0(circuit_tracking) eq 1
    acl server_full fe_conn ge 9500
    acl has_valid_token lua.validate_queue_token eq 1
    
    # Routing
    use_backend nginx_layer if is_vip
    use_backend nginx_layer if has_valid_token !server_full
    use_backend virtual_queue if server_full
    
    default_backend nginx_layer
```

### Fortify Integration

```rust
// Generate queue token
pub fn generate_queue_token(circuit_id: &str) -> String {
    let timestamp = current_timestamp();
    let message = format!("{}|{}", timestamp, circuit_id);
    let signature = hmac_sha256(&SECRET_KEY, &message);
    
    format!("{}|{}", message, signature)
}

// Validate token age and promote to priority queue
pub fn validate_and_promote(circuit_id: &str, token: &str) -> Result<bool> {
    let parts: Vec<&str> = token.split('|').collect();
    let timestamp: u64 = parts[0].parse()?;
    let age = current_timestamp() - timestamp;
    
    // Must wait at least 30 seconds before priority
    if age >= 30 && age <= 300 {
        // Promote to medium priority in HAProxy
        haproxy_client.set_gpc0(circuit_id, 3)?;  // gpc0=3 for queue priority
        Ok(true)
    } else {
        Ok(false)
    }
}
```

---

## Performance Characteristics

### Capacity Scaling

| Scenario | Concurrent Users | Queue Behavior |
|----------|-----------------|----------------|
| **Normal Load** | < 9,500 | No queue, instant access |
| **High Load** | 9,500 - 15,000 | Virtual queue active, 5-30s wait |
| **Attack** | 15,000+ | Virtual queue + aggressive bans |

### Latency Impact

| User Type | Latency |
|-----------|---------|
| **VIP (Verified)** | < 50ms (instant) |
| **Queue Token (30s+)** | 30-60s wait |
| **New User (Attack)** | 60-300s wait |

---

## Future Enhancements

1. **Dynamic Wait Time Display**: Show estimated wait time on queue page (requires JavaScript or server-side rendering)
2. **Priority Boost for Long Waits**: Automatically promote users who wait 5+ minutes
3. **PoW Difficulty Adjustment**: Real-time difficulty scaling based on attack severity
4. **Multi-Node Queue**: Distributed queue tokens across Cerberus swarm
5. **Queue Analytics**: Track wait times, conversion rates, abandonment

---

## Summary

**Key Benefits**:
- ✅ Offloads queue burden to client browsers (zero server memory)
- ✅ Fair access via age-based priority (older tokens get priority)
- ✅ No JavaScript required (meta refresh + HTML forms)
- ✅ Prevents bot queue flooding (token expiry + circuit bans)
- ✅ Rewards patient users (PoW optional for priority)
- ✅ Scales to 10,000+ concurrent without degradation

**User Experience**:
- Legitimate users see professional "please wait" page
- Automatic retry (no manual refresh needed)
- Clear communication (no confusing errors)
- VIP bypass for verified users (incentivizes CAPTCHA completion)
