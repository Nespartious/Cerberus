# Nginx - Layer 2: The Filter

## 📖 User Story

```
As an end user with JavaScript disabled (Tor Browser Safest mode)
I want to access the CAPTCHA gate and protected service
So that I maintain maximum privacy without compromising functionality

Acceptance Criteria:
- Static CAPTCHA HTML served without JavaScript
- Header scrubbing removes fingerprinting vectors
- CSP headers prevent XSS and data exfiltration
- Buffer limits prevent slowloris attacks
- Forwards Circuit ID to Fortify for verification
```

---

## Overview
Nginx serves as Cerberus's second line of defense, operating at Layer 7 (HTTP). It receives pre-filtered traffic from HAProxy and performs protocol sanitization, buffer management, static content delivery, and request routing. Nginx is the guardian between the raw internet and your application logic, ensuring only clean, well-formed HTTP requests reach Fortify.

**Position in Stack:** Tor → HAProxy (Port 10000) → **Nginx (Port 10001)** → Fortify (Port 10002) → Target

---

## Core Responsibilities

### 1. Protocol Sanitization & Header Scrubbing
- **Remove Fingerprintable Metadata**: Strip headers that reveal client info (User-Agent normalization, Accept-Language removal)
- **HTTP Protocol Enforcement**: Reject malformed requests, invalid methods, oversized headers
- **Request Normalization**: Ensure consistent request format before forwarding to Fortify
- **Security Headers**: Add hardening headers (CSP, X-Frame-Options, etc.)

### 2. Buffer Management & Attack Mitigation
- **Slowloris Defense**: Aggressive timeouts for slow clients
- **Large Payload Protection**: Strict `client_max_body_size` limits
- **Buffer Overflow Prevention**: Fixed-size buffers for headers and bodies
- **Request Smuggling Defense**: HTTP/1.1 compliance enforcement

### 3. Static Content Delivery (Offload Layer)
- **CAPTCHA Page Serving**: Deliver `captcha.html` directly without hitting Fortify
- **Landing Pages**: Serve welcome pages, maintenance notices, error pages
- **Asset Caching**: Static CSS, JS, images with aggressive caching
- **Zero-Trust Static Serving**: No dynamic processing, pure file delivery

### 4. Request Routing & Logic Gates
- **Path-Based Routing**: 
  - `/` → Static CAPTCHA page
  - `/verify-captcha` → Fortify (POST only)
  - `/api/*` → Fortify (authenticated requests)
  - All else → 404 or redirect to CAPTCHA
- **Method Filtering**: Only allow GET (static) and POST (captcha verification)
- **Rate Limiting**: Secondary rate limits (defense in depth after HAProxy)

---

## Key Features & Mechanisms

### Request Flow Decision Tree

```
Incoming Request from HAProxy
         |
         v
  [Method Check]
         |
    +----+----+
    |         |
   GET       POST
    |         |
    v         v
[Path]   [Path=/verify-captcha?]
    |         |
    |        Yes → Forward to Fortify
    |        No → 403 Forbidden
    v
  [Is Static Asset?]
    |
   Yes → Serve from /var/www/cerberus/static/
    |
   No → Serve captcha.html (catch-all)
```

### Static Gate (CAPTCHA Delivery)

**Concept:** Serve the CAPTCHA challenge page directly from Nginx's disk cache without invoking Fortify. This dramatically reduces CPU load during attacks.

```nginx
location / {
    root /var/www/cerberus/static;
    try_files $uri /captcha.html;
    
    # No backend forwarding
    # No dynamic processing
    # Pure static delivery
}
```

**Benefits:**
- 10x-100x faster than dynamic page generation
- Attackers waste resources solving CAPTCHAs, not killing your backend
- Fortify only processes CAPTCHA solutions (POST requests), not page views

### Header Scrubbing (Anti-Fingerprinting)

**Problem:** Headers like `User-Agent`, `Accept-Language`, `Accept-Encoding` leak client information and can be used for fingerprinting or deanonymization.

```nginx
# Remove dangerous headers
proxy_set_header User-Agent "Mozilla/5.0 (Windows NT 10.0; rv:109.0) Gecko/20100101 Firefox/115.0";
proxy_set_header Accept-Language "en-US,en;q=0.5";
proxy_set_header Accept-Encoding "gzip, deflate, br";

# Strip incoming client headers that might leak info
proxy_pass_header X-Circuit-ID;  # Keep this (from HAProxy)
proxy_set_header X-Real-IP $remote_addr;  # Will be 127.0.0.1 (safe)
proxy_set_header X-Forwarded-For "";  # Remove
proxy_set_header Via "";  # Remove
proxy_set_header X-Forwarded-Host "";  # Remove
```

**Result:** All requests to Fortify appear identical from a header perspective, focusing defense on behavior rather than fingerprints.

### Buffer Management (Attack Prevention)

```nginx
# Client request limits
client_body_buffer_size 16k;       # Max POST body buffer
client_header_buffer_size 1k;      # Header buffer
large_client_header_buffers 2 1k;  # Max 2 large headers
client_max_body_size 1m;           # Absolute max POST size

# Timeouts (kill slow connections)
client_body_timeout 5s;            # Time to receive POST body
client_header_timeout 5s;          # Time to receive headers
send_timeout 10s;                  # Time to send response
keepalive_timeout 5s;              # Connection reuse timeout
```

**Defense Rationale:**
- `client_body_timeout 5s`: Prevents Slowloris-style slow POST attacks
- `client_max_body_size 1m`: CAPTCHA solutions are tiny (<1KB), anything larger is suspicious
- Short `keepalive_timeout`: Force connection closure (limit persistence abuse)

### Rate Limiting (Secondary Defense Layer)

```nginx
# Define rate limit zones
limit_req_zone $binary_remote_addr zone=captcha_limit:10m rate=10r/s;
limit_req_zone $binary_remote_addr zone=api_limit:10m rate=5r/s;

# Apply to locations
location /verify-captcha {
    limit_req zone=captcha_limit burst=5 nodelay;
    proxy_pass http://fortify_backend;
}

location /api/ {
    limit_req zone=api_limit burst=3 nodelay;
    proxy_pass http://fortify_backend;
}
```

**Why Secondary?** HAProxy already tracks per-circuit rates, but Nginx adds a safety net for:
- Misconfigured HAProxy
- Direct Nginx access (shouldn't happen, but defense in depth)
- Per-IP rate limiting within a circuit (if multiple users share a circuit)

---

## Attack Mitigation Strategies

### 1. Slowloris (Slow HTTP Headers)

**Detection:**
- `client_header_timeout 5s` enforces rapid header delivery

**Response:**
- Connection killed by Nginx after 5s
- No resources wasted waiting for headers

**Test:**
```bash
# Slowloris simulator
slowhttptest -c 1000 -H -g -o slowloris_test -i 10 -r 200 -t GET -u http://<onion>/ -x 240
```

### 2. Slow POST (Slow Body Attacks)

**Detection:**
- `client_body_timeout 5s` enforces rapid body delivery

**Response:**
- Connection killed if POST body not received within 5s
- Prevents tie-up of worker processes

### 3. Large Payload Attacks

**Detection:**
- `client_max_body_size 1m` enforces size limit

**Response:**
- 413 Payload Too Large error
- Request never reaches Fortify

### 4. HTTP Request Smuggling

**Detection:**
- Nginx strictly parses HTTP/1.1
- No tolerance for ambiguous Content-Length/Transfer-Encoding

**Response:**
- 400 Bad Request on malformed requests
- Prevents desync attacks between Nginx and Fortify

### 5. Invalid HTTP Methods

```nginx
# Only allow GET and POST
if ($request_method !~ ^(GET|POST)$) {
    return 405;
}
```

**Response:**
- 405 Method Not Allowed for PUT, DELETE, PATCH, etc.

---

## Configuration Sections

### Main Configuration (`nginx.conf`)

```nginx
user www-data;
worker_processes auto;  # Auto-detect CPU cores
pid /run/nginx.pid;

events {
    worker_connections 1024;
    use epoll;  # Linux-optimized event model
}

http {
    # Basic settings
    sendfile on;
    tcp_nopush on;
    tcp_nodelay on;
    keepalive_timeout 5s;
    types_hash_max_size 2048;
    server_tokens off;  # Hide Nginx version
    
    # Buffer settings
    client_body_buffer_size 16k;
    client_header_buffer_size 1k;
    client_max_body_size 1m;
    large_client_header_buffers 2 1k;
    
    # Timeout settings
    client_body_timeout 5s;
    client_header_timeout 5s;
    send_timeout 10s;
    
    # Logging
    access_log /var/log/nginx/access.log;
    error_log /var/log/nginx/error.log warn;
    
    # Rate limiting zones
    limit_req_zone $binary_remote_addr zone=captcha_limit:10m rate=10r/s;
    limit_req_zone $binary_remote_addr zone=api_limit:10m rate=5r/s;
    
    # Include site configs
    include /etc/nginx/sites-enabled/*;
}
```

### Site Configuration (`/etc/nginx/sites-available/cerberus`)

```nginx
# Fortify backend
upstream fortify_backend {
    server 127.0.0.1:10002;
    keepalive 32;
}

server {
    listen 127.0.0.1:10001;
    server_name _;
    
    root /var/www/cerberus/static;
    index captcha.html;
    
    # Security headers
    add_header X-Frame-Options "DENY" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-XSS-Protection "1; mode=block" always;
    add_header Content-Security-Policy "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline';" always;
    add_header Referrer-Policy "no-referrer" always;
    
    # Method filtering (global)
    if ($request_method !~ ^(GET|POST)$) {
        return 405;
    }
    
    # Location: Static assets (CSS, JS, images)
    location ~* \.(css|js|png|jpg|jpeg|gif|ico|svg|woff|woff2)$ {
        expires 1h;
        add_header Cache-Control "public, immutable";
    }
    
    # Location: CAPTCHA verification (POST only to Fortify)
    location = /verify-captcha {
        limit_req zone=captcha_limit burst=5 nodelay;
        
        if ($request_method != POST) {
            return 405;
        }
        
        # Forward to Fortify
        proxy_pass http://fortify_backend;
        proxy_http_version 1.1;
        
        # Pass Circuit ID from HAProxy
        proxy_set_header X-Circuit-ID $http_x_circuit_id;
        
        # Scrubbed headers
        proxy_set_header Host $host;
        proxy_set_header User-Agent "Mozilla/5.0 (Windows NT 10.0; rv:109.0) Gecko/20100101 Firefox/115.0";
        proxy_set_header X-Real-IP $remote_addr;
        
        # Timeouts
        proxy_connect_timeout 5s;
        proxy_send_timeout 10s;
        proxy_read_timeout 10s;
    }
    
    # Location: API endpoints (authenticated, VIP only)
    location /api/ {
        limit_req zone=api_limit burst=3 nodelay;
        
        # Forward to Fortify
        proxy_pass http://fortify_backend;
        proxy_http_version 1.1;
        
        proxy_set_header X-Circuit-ID $http_x_circuit_id;
        proxy_set_header Host $host;
    }
    
    # Location: Health check (for HAProxy)
    location /health {
        access_log off;
        return 200 "OK\n";
        add_header Content-Type text/plain;
    }
    
    # Location: Catch-all (serve CAPTCHA page)
    location / {
        try_files $uri /captcha.html;
    }
    
    # Custom error pages
    error_page 404 /404.html;
    error_page 500 502 503 504 /50x.html;
    
    location = /50x.html {
        root /var/www/cerberus/static;
    }
}
```

---

## Static Content Structure

### Directory Layout

```
/var/www/cerberus/static/
├── captcha.html          # Main CAPTCHA challenge page
├── 404.html              # Not found page
├── 50x.html              # Server error page
├── css/
│   └── captcha.css       # CAPTCHA page styling
├── js/
│   └── captcha.js        # Client-side CAPTCHA logic
└── images/
    ├── logo.png
    └── captcha-bg.png
```

### CAPTCHA Page (`captcha.html`)

**Design Principles:**
- **Zero JavaScript Required**: CAPTCHA should work without JS (progressive enhancement)
- **Minimal Fingerprinting**: No external resources, no CDN links, no fonts
- **Fast Load**: <10KB total page size
- **Tor-Friendly**: No client-side crypto that might leak timing info

**Basic Structure:**
```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Verification Required</title>
    <link rel="stylesheet" href="/css/captcha.css">
</head>
<body>
    <div class="captcha-container">
        <h1>Access Verification</h1>
        <p>To continue, please solve the challenge below.</p>
        
        <form action="/verify-captcha" method="POST">
            <!-- CAPTCHA image (generated by Fortify, served via Nginx) -->
            <img src="/api/captcha-image?challenge=<random-id>" alt="CAPTCHA">
            
            <input type="hidden" name="challenge_id" value="<random-id>">
            <input type="text" name="solution" placeholder="Enter the code" required>
            
            <button type="submit">Verify</button>
        </form>
        
        <p class="info">This verification helps protect the service from automated abuse.</p>
    </div>
    
    <script src="/js/captcha.js"></script>
</body>
</html>
```

**Workflow:**
1. User hits any path → Nginx serves `captcha.html`
2. `captcha.html` embeds CAPTCHA image from Fortify (`/api/captcha-image`)
3. User solves CAPTCHA → submits form to `/verify-captcha`
4. Nginx forwards POST to Fortify → Fortify validates → updates HAProxy stick table
5. HAProxy promotes circuit to VIP → future requests bypass CAPTCHA

---

## Integration with Fortify

### Fortify Endpoints Proxied by Nginx

1. **`GET /api/captcha-image?challenge=<id>`**
   - Fortify generates CAPTCHA image (PNG)
   - Nginx passes through (no caching, always fresh)

2. **`POST /verify-captcha`**
   - Body: `{ "challenge_id": "...", "solution": "..." }`
   - Fortify validates solution
   - Returns: `{ "success": true, "token": "..." }` or `{ "success": false }`

3. **`GET /api/*` (Future)**
   - Authenticated API endpoints
   - Requires VIP status (checked by HAProxy)

### Header Passing (Circuit ID Preservation)

```nginx
proxy_set_header X-Circuit-ID $http_x_circuit_id;
```

**Critical:** Nginx must forward the `X-Circuit-ID` header from HAProxy to Fortify. This allows Fortify to:
- Identify which circuit to promote/ban in HAProxy stick tables
- Track circuit behavior across requests
- Implement per-circuit rate limiting in application logic

---

## Security Hardening

### 1. Disable Unnecessary Modules

```nginx
# Compile Nginx without:
# - Autoindex module (directory listings)
# - SSI module (server-side includes)
# - Auth modules (using Fortify instead)
./configure --without-http_autoindex_module --without-http_ssi_module
```

### 2. File Permissions

```bash
# Static content read-only
chown -R www-data:www-data /var/www/cerberus/static
chmod -R 644 /var/www/cerberus/static
find /var/www/cerberus/static -type d -exec chmod 755 {} \;

# Nginx config read-only
chmod 644 /etc/nginx/sites-available/cerberus
```

### 3. Disable Server Tokens

```nginx
server_tokens off;  # Don't reveal Nginx version
```

### 4. Limit Worker Processes

```nginx
worker_processes auto;  # Match CPU core count
worker_rlimit_nofile 8192;  # File descriptor limit
```

---

## Logging & Monitoring

### Access Log Format

```nginx
log_format cerberus '$remote_addr - $remote_user [$time_local] '
                    '"$request" $status $body_bytes_sent '
                    '"$http_referer" "$http_user_agent" '
                    '"$http_x_circuit_id" $request_time';

access_log /var/log/nginx/access.log cerberus;
```

**Key Fields:**
- `$http_x_circuit_id`: Tor Circuit ID (from HAProxy)
- `$request_time`: Response time (detect slow requests)
- `$status`: HTTP status code (track errors)

### Metrics to Monitor

1. **Request Rates**
   - Total requests/sec
   - Static vs. dynamic request ratio
   - CAPTCHA verification rate

2. **Error Rates**
   - 413 errors (oversized payloads)
   - 408 errors (client timeouts)
   - 502/504 errors (Fortify backend issues)

3. **Performance Metrics**
   - Average request time (`$request_time`)
   - Worker process utilization
   - Connection queue length

---

## Testing & Validation

### Sprint 1 Tests

1. **Static CAPTCHA Delivery Test**
   ```bash
   curl -x socks5h://127.0.0.1:9050 http://<onion>/
   # Expected: captcha.html content
   ```

2. **POST Method Filtering Test**
   ```bash
   curl -x socks5h://127.0.0.1:9050 -X POST http://<onion>/
   # Expected: 403 Forbidden (no /verify-captcha path)
   ```

3. **Timeout Test (Slowloris Simulation)**
   ```bash
   (echo -n "GET / HTTP/1.1\r\nHost: <onion>\r\n"; sleep 10; echo "") | \
     socat - SOCKS4A:127.0.0.1:<onion>:80,socksport=9050
   # Expected: Connection closed after 5s
   ```

4. **Oversized Payload Test**
   ```bash
   dd if=/dev/zero bs=2M count=1 | curl -x socks5h://127.0.0.1:9050 \
     -X POST -d @- http://<onion>/verify-captcha
   # Expected: 413 Payload Too Large
   ```

5. **Header Scrubbing Verification**
   - Check Fortify logs
   - Verify all `User-Agent` headers are normalized

---

## Performance Tuning

### Expected Capacity
- **Static Content**: 10,000+ req/sec (limited by disk I/O)
- **Proxied Requests**: 1,000-5,000 req/sec (limited by Fortify)
- **Worker Processes**: 1 per CPU core (auto-scales)

### Bottlenecks
1. **Disk I/O**: Static file serving (mitigated by OS page cache)
2. **Fortify Latency**: Backend response time affects Nginx throughput
3. **Connection Limits**: `worker_connections` × `worker_processes`

### Optimization Strategies
- **Enable sendfile**: Direct kernel-to-socket transfers (zero-copy)
- **Enable gzip**: Compress static assets (lower bandwidth)
- **Increase worker_connections**: Scale to 4096+ per worker
- **Tune kernel**: Increase `net.core.somaxconn` and `net.ipv4.tcp_max_syn_backlog`

---

## Critical Configuration Checklist

- [ ] Listen on `127.0.0.1:10001` (not publicly exposed)
- [ ] Static root set to `/var/www/cerberus/static`
- [ ] `client_body_timeout` and `client_header_timeout` set to 5s
- [ ] `client_max_body_size` limited to 1m
- [ ] `server_tokens off` (hide version)
- [ ] Security headers added (CSP, X-Frame-Options, etc.)
- [ ] Method filtering (only GET/POST allowed)
- [ ] Rate limiting zones configured
- [ ] Circuit ID header forwarded to Fortify
- [ ] Health check endpoint enabled for HAProxy
- [ ] Custom error pages configured
- [ ] Logging format includes Circuit ID

---

## Future Enhancements (Post-Sprint 1)

1. **Dynamic CAPTCHA Difficulty**: Adjust complexity based on attack severity
2. **Client Fingerprinting**: Canvas fingerprinting, WebGL detection (privacy trade-off)
3. **Progressive Rate Limiting**: Start lenient, tighten during attacks
4. **Nginx Lua Module**: Custom logic without Fortify (e.g., simple challenges)
5. **HTTP/2 Support**: Faster static delivery (requires SSL, complex with Tor)
6. **Brotli Compression**: Better than gzip for static assets

---

## References
- Nginx Security: https://nginx.org/en/docs/http/ngx_http_core_module.html
- Rate Limiting: https://www.nginx.com/blog/rate-limiting-nginx/
- DDoS Mitigation: https://www.nginx.com/blog/mitigating-ddos-attacks-with-nginx-and-nginx-plus/
- Request Smuggling Prevention: https://portswigger.net/web-security/request-smuggling
