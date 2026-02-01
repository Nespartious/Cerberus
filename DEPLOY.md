# Cerberus One-Click Deployment Guide

Deploy a complete Cerberus defense system on a fresh Ubuntu server in under 10 minutes.

## Quick Start (Fresh Ubuntu 22.04/24.04)

### Option 1: Clone and Run (Recommended)

```bash
# SSH into your fresh Ubuntu server, then:

# 1. Install git (if not present)
sudo apt update && sudo apt install -y git

# 2. Clone the repository (public, no auth needed)
git clone https://github.com/YOUR_USER/Cerberus.git

# 3. Run deployment
cd Cerberus
sudo bash deploy/cerberus-one-click.sh
```

### Option 2: One-Liner (After Pushing to GitHub)

```bash
curl -sSL https://raw.githubusercontent.com/YOUR_USER/Cerberus/main/deploy/cerberus-one-click.sh | sudo bash
```

---

## What Gets Installed

| Component | Purpose | Port |
|-----------|---------|------|
| **Tor** | Hidden service hosting | - |
| **HAProxy** | L4 rate limiting, circuit tracking | 10000, 8404 (stats) |
| **Nginx** | L7 routing, header scrubbing | 10001 |
| **Redis** | Session/circuit state storage | 6379 |
| **Fortify** | CAPTCHA, passport validation | 8888 |

---

## Hardcoded Test Configuration

The deployment script uses these test values:

```
Backend (protected service): sigilahzwq5u34gdh2bl3ymokyc7kobika55kyhztsucdoub73hz7qid.onion
Vanity Prefix:               sigil (5 chars - address generated during deployment)
```

The script will **generate a fresh vanity address** matching the prefix during deployment.

To change these, edit the variables at the top of `deploy/cerberus-one-click.sh`:

```bash
BACKEND_ONION="your-backend.onion"
VANITY_PREFIX="prefix"  # First N characters of generated .onion
```

---

## Post-Deployment

### Verify Services

```bash
# Check all services are running
sudo systemctl status fortify haproxy nginx tor redis-server

# View your onion address
cat /var/lib/tor/cerberus_hs/hostname

# Test locally
curl http://127.0.0.1:10001/
curl http://127.0.0.1:10001/health
```

### View Logs

```bash
# Fortify logs (CAPTCHA, circuit tracking)
journalctl -u fortify -f

# Tor logs (hidden service status)
journalctl -u tor -f

# Nginx access logs
tail -f /var/log/nginx/access.log

# HAProxy stats (web dashboard)
# Open in browser: http://YOUR_SERVER_IP:8404/
```

### Restart Services

```bash
sudo systemctl restart fortify
sudo systemctl restart haproxy nginx tor
```

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                         TOR NETWORK                             │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  TOR DAEMON (Hidden Service)                                    │
│  - Exposes: sigilz3i4...onion:80                                │
│  - Forwards to: 127.0.0.1:10000 (HAProxy)                       │
│  - Exports Circuit ID via PROXY protocol                        │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  HAPROXY (Layer 4 - Connection Governor)                        │
│  - Extracts Tor Circuit ID                                      │
│  - Rate limits per circuit (10 conn, 20 req/10s)                │
│  - Bans malicious circuits                                      │
│  - VIP bypass for verified users                                │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  NGINX (Layer 7 - Gatekeeper)                                   │
│  - Header scrubbing (privacy normalization)                     │
│  - Route decisions                                              │
│  - Auth subrequest to Fortify                                   │
└─────────────────────────────────────────────────────────────────┘
                              │
                    ┌─────────┴─────────┐
                    │                   │
                    ▼                   ▼
┌──────────────────────────┐  ┌──────────────────────────┐
│  FORTIFY (L7+ Logic)     │  │  BACKEND (Protected)     │
│  - SVG CAPTCHA           │  │  sigilahz...onion        │
│  - Passport validation   │  │  (Your real service)     │
│  - Circuit reputation    │  │                          │
│  - Redis state           │  │                          │
└──────────────────────────┘  └──────────────────────────┘
```

---

## Customization

### Generate Your Own Vanity Address

```bash
cd /opt/cerberus
cargo run --release -p vanity-onion -- --prefix mysite --output /var/lib/tor/cerberus_hs
sudo chown -R debian-tor:debian-tor /var/lib/tor/cerberus_hs
sudo chmod 700 /var/lib/tor/cerberus_hs
sudo systemctl restart tor
```

### Adjust Rate Limits

Edit `/etc/haproxy/haproxy.cfg`:

```haproxy
# Max concurrent connections per circuit
http-request deny deny_status 429 if { sc0_conn_cur(be_stick_tables) gt 10 }

# Max requests per 10 seconds per circuit  
http-request deny deny_status 429 if { sc0_http_req_rate(be_stick_tables) gt 20 }
```

Then: `sudo systemctl reload haproxy`

### Fortify Configuration

Edit `/etc/cerberus/fortify/config.toml`:

```toml
[rate_limit]
max_failed_attempts = 5       # CAPTCHA failures before soft-ban
soft_lock_duration_secs = 300 # 5 minute cooldown
ban_duration_secs = 3600      # 1 hour hard ban
```

Then: `sudo systemctl restart fortify`

---

## Troubleshooting

### Tor Not Starting

```bash
# Check Tor logs
journalctl -u tor -n 50

# Verify permissions on hidden service directory
ls -la /var/lib/tor/cerberus_hs/
# Should be: drwx------ debian-tor debian-tor

# Fix permissions
sudo chown -R debian-tor:debian-tor /var/lib/tor/cerberus_hs
sudo chmod 700 /var/lib/tor/cerberus_hs
```

### Fortify Connection Refused

```bash
# Check if Redis is running
sudo systemctl status redis-server

# Check Fortify logs
journalctl -u fortify -n 50

# Test Redis connection
redis-cli ping
# Should return: PONG
```

### HAProxy Not Accepting Connections

```bash
# Check config syntax
haproxy -c -f /etc/haproxy/haproxy.cfg

# Check if ports are in use
ss -tlnp | grep -E '10000|10001|8404'
```

---

## Security Notes

1. **Firewall**: The deployment only binds to `127.0.0.1` - no public ports exposed
2. **Circuit Tracking**: Each Tor circuit gets a unique ID for rate limiting
3. **PoW Defense**: Tor 0.4.8+ native proof-of-work is enabled
4. **Vanguards**: Anti-sybil protection is installed and should be configured

---

## Uninstall

```bash
# Stop services
sudo systemctl stop fortify haproxy nginx tor redis-server

# Disable services  
sudo systemctl disable fortify haproxy nginx tor redis-server

# Remove Cerberus files
sudo rm -rf /etc/cerberus /opt/cerberus /var/lib/cerberus
sudo rm -rf /var/lib/tor/cerberus_hs
sudo rm -f /usr/local/bin/fortify
sudo rm -f /etc/systemd/system/fortify.service
sudo rm -f /etc/nginx/sites-enabled/cerberus
sudo rm -f /etc/sysctl.d/99-cerberus.conf

# Reload systemd
sudo systemctl daemon-reload
```
