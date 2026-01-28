# Monitoring and Management UI

**Operational Visibility and Control for Headless Cerberus Deployments**
## 📖 User Story

```
As a service operator managing a headless deployment
I want to see real-time metrics and manually intervene during attacks
So that I can monitor system health and respond to sophisticated threats

Acceptance Criteria:
- Grafana dashboards accessible via Tor Onion Service
- Live session counts with color-coded statuses (VIP/PoW/Normal/Banned)
- Historical snapshots (5m, 15m, 1h, 24h, 7d, 30d, 90d, 365d)
- Admin panel for manual actions (promote/demote/ban circuits)
- Custom TUI for emergency SSH access when web UI unavailable
```
---

## 📋 Overview

Cerberus is designed for **headless server deployment** (no GUI, no desktop environment), yet operators require real-time visibility into:
- **Resource usage** (CPU, RAM, disk, network)
- **Live session metrics** (circuit counts, queue depth, ban/VIP status)
- **Attack detection** (DDoS events, abuse patterns, PoW challenges)
- **Manual interventions** (promote/demote circuits, force attack mode)

This document evaluates UI options for secure, stable, and practical operational monitoring.

---

## 🎯 UI Requirements

### Core Capabilities

**Must Have:**
1. ✅ **Headless Compatible**: No X11/Wayland required
2. ✅ **Secure by Default**: No clearnet exposure, Tor-only access
3. ✅ **Real-Time Updates**: <5 second latency for live metrics
4. ✅ **Historical Data**: Retention for 1-12 months
5. ✅ **Manual Actions**: Promote/demote circuits, trigger modes
6. ✅ **Multi-User**: Support for multiple admin accounts (readonly + admin roles)

**Nice to Have:**
- 📊 Custom dashboards (drag-and-drop widgets)
- 📈 Graphing and trend analysis
- 🔔 Alert notifications (Tor-based, not email)
- 📱 Mobile-friendly UI (for Tor Browser on phone)

### Security Constraints

**Tor-Only Access:**
- ❌ No clearnet HTTP ports (port 80/443 exposed = attack vector)
- ✅ Management UI accessible only via Tor Onion Service
- ✅ Authentication required (username/password + 2FA TOTP)
- ✅ Audit logging (all admin actions logged)

**Isolation:**
- UI must run in separate security context from defense layers
- Read-only access to metrics (cannot directly modify HAProxy/Nginx configs)
- Manual actions queue commands to Fortify's admin API (not direct execution)

---

## 🔍 UI Architecture Evaluation

### Option 1: TUI (Terminal User Interface) ⭐ Recommended for Local Access

**Technology**: `ratatui` (Rust TUI framework) or `ncurses`

**Pros:**
- ✅ Zero web server required (SSH only)
- ✅ Lightweight (~5MB RAM)
- ✅ Native feel for CLI admins
- ✅ Works over high-latency Tor SSH tunnels
- ✅ No authentication complexity (SSH keys provide auth)

**Cons:**
- ❌ Requires SSH access (must tunnel through Tor)
- ❌ Single-user at a time (no collaborative monitoring)
- ❌ Limited graphing capabilities (ASCII charts only)
- ❌ Not mobile-friendly

**Use Case:**
- Operators who SSH into servers regularly
- Emergency diagnostics (SSH in, run `cerberus-tui`)
- Low-resource environments (VPS with 512MB RAM)

**Example: `cerberus-tui`**

```
┌─────────────────────────────────────────────────────────────────────────┐
│ CERBERUS MONITORING - market7xjd4abc.onion           [23:41:32 UTC]    │
├─────────────────────────────────────────────────────────────────────────┤
│ SYSTEM RESOURCES                                                        │
│  CPU: [████████████░░░░░░░░] 65.2%   RAM: [██████░░░░░░░░░░] 42.1%    │
│  Disk: [██░░░░░░░░░░░░░░░░] 18.3%     Net: ↓152 Mbps ↑89 Mbps         │
├─────────────────────────────────────────────────────────────────────────┤
│ LIVE SESSIONS (Last 30s)                                                │
│  🟢 VIP Circuits:        127   🟡 PoW Validated:     2,847             │
│  🔵 Normal Circuits:   8,421   🔴 Banned Circuits:       89             │
│  ⚪ Queue Waiting:     1,234   🟠 CAPTCHA Pending:    3,456             │
│                                                                         │
│  Total Active: 16,174 / 10,000 max  ⚠️ OVERLOAD MODE ACTIVE            │
├─────────────────────────────────────────────────────────────────────────┤
│ SNAPSHOT REPORTS                         [Tab: 5m/15m/30m/1h/24h]     │
│  Interval: Last 5 minutes                                               │
│  ├─ New Circuits:          +432                                        │
│  ├─ Banned:                 +89 (20.6% of new)                         │
│  ├─ CAPTCHA Solved:        +287 (66.4% success rate)                   │
│  ├─ PoW Challenges Issued:  +34                                        │
│  └─ Attack Events:            2 (Slowloris x1, Circuit Flood x1)       │
├─────────────────────────────────────────────────────────────────────────┤
│ TOP BANNED CIRCUITS (Last 1h)                                          │
│  Circuit: a3f8...b2c1  Bans: 12  Reason: Failed CAPTCHA (brute force) │
│  Circuit: 7d4e...89af  Bans:  8  Reason: Slowloris attack              │
│  Circuit: 2c1b...45de  Bans:  6  Reason: Endpoint enumeration          │
├─────────────────────────────────────────────────────────────────────────┤
│ MANUAL ACTIONS                                                          │
│  [P] Promote Circuit   [D] Demote Circuit   [A] Force Attack Mode      │
│  [S] Force Safe Mode   [B] Ban Circuit      [U] Unban Circuit          │
│  [Q] Quit              [R] Refresh Now      [H] Help                   │
└─────────────────────────────────────────────────────────────────────────┘
Command: _
```

---

### Option 2: Web UI (Tor-Accessible Dashboard) ⭐⭐ Recommended for Remote Access

**Technology**: Lightweight Rust web framework (Axum + Tera templates) or Grafana

**Pros:**
- ✅ Multi-user simultaneous access
- ✅ Mobile-friendly (Tor Browser on phones)
- ✅ Rich graphing capabilities (Chart.js, Plotly)
- ✅ No SSH required (Tor Onion Service only)
- ✅ Familiar web interface

**Cons:**
- ⚠️ Adds attack surface (web server vulnerabilities)
- ⚠️ Requires authentication system (TOTP 2FA, session management)
- ⚠️ Higher resource usage (~50-100MB RAM for Node.js/Python, ~10MB for Rust)
- ⚠️ Tor latency affects UX (5-10 second page loads)

**Use Case:**
- Multiple operators monitoring from different locations
- Non-technical staff need access (easier than SSH)
- Mobile monitoring (check status from Tor Browser mobile app)

**Security Requirements:**
- Tor Onion Service only (never bind to 0.0.0.0)
- HTTPS with self-signed cert (prevent MITM over Tor)
- TOTP 2FA mandatory (no password-only auth)
- Rate limiting (10 req/min per circuit)
- Session timeout (15 min idle)

**Example Architecture:**

```
┌─────────────────────────────────────────────────────────────────┐
│  Tor Browser → Tor Network → Onion Service (cerberus-admin.onion)│
│                                      ↓                           │
│                         Web UI (Rust + Axum, port 10100)         │
│                                      ↓                           │
│                    Admin API (Fortify, Unix socket)              │
│                                      ↓                           │
│              Read: Prometheus metrics, SQLite DB                 │
│              Write: Command queue (promote/demote/ban)           │
└─────────────────────────────────────────────────────────────────┘
```

---

### Option 3: Grafana + Prometheus ⭐⭐⭐ Recommended for Production

**Technology**: Industry-standard monitoring stack

**Pros:**
- ✅ **Battle-tested**: Used by millions of deployments
- ✅ **Zero custom code**: Pre-built dashboards, alerting, graphing
- ✅ **Extensive integrations**: HAProxy exporter, Node exporter, custom metrics
- ✅ **Historical data**: Built-in time-series database (Prometheus)
- ✅ **Alerting**: Built-in alert manager (can send to Tor-based webhook)
- ✅ **Professional UI**: Polished, feature-rich dashboards

**Cons:**
- ⚠️ Higher resource usage (~200-300MB RAM for both)
- ⚠️ Requires separate Grafana + Prometheus instances
- ⚠️ No manual action capabilities out-of-box (need custom panel/API)

**Use Case:**
- Production deployments with dedicated monitoring VMs
- Operators familiar with Prometheus/Grafana ecosystem
- Integration with existing monitoring infrastructure

**Recommended Setup:**

```
┌─────────────────────────────────────────────────────────────────┐
│                  Cerberus Monitoring Stack                       │
├─────────────────────────────────────────────────────────────────┤
│  1. Prometheus (Metrics Collection)                             │
│     ├─ HAProxy Exporter (stick table stats, circuit counts)     │
│     ├─ Nginx Exporter (request rates, response times)           │
│     ├─ Node Exporter (CPU, RAM, disk, network)                  │
│     ├─ Fortify Custom Exporter (CAPTCHA stats, circuit rep)     │
│     └─ Scrape interval: 5 seconds (live metrics)                │
├─────────────────────────────────────────────────────────────────┤
│  2. Grafana (Visualization)                                      │
│     ├─ Tor Onion Service: cerberus-mon.onion                    │
│     ├─ Dashboards: System, Defense Layers, Circuit Analysis     │
│     ├─ Alerting: Telegram bot via Tor (or Matrix homeserver)    │
│     └─ Authentication: TOTP 2FA + Tor circuit verification      │
├─────────────────────────────────────────────────────────────────┤
│  3. Custom Admin Panel (Optional, for manual actions)           │
│     ├─ Minimal Rust web UI (Axum)                               │
│     ├─ Actions: Promote/Demote/Ban circuits                     │
│     ├─ Commands queued to Fortify Admin API                     │
│     └─ Accessible via separate onion: cerberus-admin.onion      │
└─────────────────────────────────────────────────────────────────┘
```

---

## 📊 Dashboard Design: Critical Metrics

### Overview Dashboard (Landing Page)

**System Health (Top Row):**
```
┌──────────────────────┬──────────────────────┬──────────────────────┐
│  CPU Usage           │  RAM Usage           │  Disk Usage          │
│  [████████░░] 78%    │  [██████░░░░] 56%    │  [██░░░░░░░░] 18%    │
│  8.2 / 16 cores      │  4.5 GB / 8 GB       │  18 GB / 100 GB      │
└──────────────────────┴──────────────────────┴──────────────────────┘
┌──────────────────────────────────────────────────────────────────────┐
│  Network I/O (Last 5 min)                                            │
│  ↓ Inbound: 287 Mbps (avg)   ↑ Outbound: 143 Mbps (avg)            │
│  [Live Graph: Line chart showing last 30 minutes]                   │
└──────────────────────────────────────────────────────────────────────┘
```

**Live Session Counts (Middle Section):**
```
┌─────────────────────────────────────────────────────────────────────┐
│  ACTIVE CIRCUITS (Realtime)                        Updated: 2s ago  │
├─────────────────────────────────────────────────────────────────────┤
│  🟢 VIP (Validated):        127    [Progress: 1.3% of total]       │
│  🟡 PoW (Proof-of-Work):  2,847    [Progress: 28.5% of total]      │
│  🔵 Normal (CAPTCHA OK):  8,421    [Progress: 84.2% of total]      │
│  🔴 Banned:                  89    [Progress: 0.9% of total]       │
│  ⚪ Queue (Waiting Room): 1,234    [Progress: 12.3% of total]      │
│  🟠 CAPTCHA Pending:      3,456    [Progress: 34.6% of total]      │
├─────────────────────────────────────────────────────────────────────┤
│  TOTAL ACTIVE: 16,174 / 10,000 configured max  ⚠️ OVERLOAD         │
└─────────────────────────────────────────────────────────────────────┘
```

**Session Origins/Destinations (Bottom Left):**
```
┌──────────────────────────────────────────────────────────────────┐
│  TOP ENDPOINTS (Last 1 hour)                                     │
├──────────────────────────────────────────────────────────────────┤
│  /api/login       3,421 req   (28.3% of traffic)                │
│  /market/search   2,847 req   (23.5%)                           │
│  /                1,923 req   (15.9%)                           │
│  /static/logo.png 1,234 req   (10.2%)                           │
│  /api/orders        982 req    (8.1%)                           │
└──────────────────────────────────────────────────────────────────┘
```

**Recent Events (Bottom Right):**
```
┌──────────────────────────────────────────────────────────────────┐
│  ATTACK EVENTS (Last 24 hours)                                   │
├──────────────────────────────────────────────────────────────────┤
│  23:38 UTC  🔴 Slowloris detected (12 circuits, auto-banned)    │
│  23:21 UTC  🟠 Circuit flood (+2,847 circuits in 30s)           │
│  22:14 UTC  🟡 PoW queue activated (load > 80%)                 │
│  19:42 UTC  🔵 Normal load resumed                              │
└──────────────────────────────────────────────────────────────────┘
```

---

### Snapshot Reports Dashboard

**Configurable Time Windows:**
- Quick snapshots: 5m, 15m, 30m, 1h, 2h, 4h, 8h, 12h, 24h, 72h
- Calendar periods: Week, Month, Year (1-5)
- Historical lookback: Past 5, 7, 14, 30, 60, 90, 180, 365, 720 days

**Metrics Per Snapshot:**
```
┌──────────────────────────────────────────────────────────────────────┐
│  SNAPSHOT: Last 15 Minutes                      [Dropdown: Change]  │
├──────────────────────────────────────────────────────────────────────┤
│  New Circuits:           +1,234                                      │
│  Banned Circuits:          +234  (19.0% of new)                      │
│  CAPTCHA Challenges:        +892  (72.3% of new)                     │
│  CAPTCHA Success Rate:      67.2% (599 solved / 892 issued)         │
│  PoW Challenges Issued:      +42  (3.4% of new)                      │
│  VIP Promotions:             +12  (from Normal → VIP)                │
│  Attack Events:                3  (Slowloris x2, Flood x1)           │
│  Avg Response Time:       124ms  (median: 98ms, p95: 342ms)         │
│  Bandwidth Used:         2.3 GB  (↓1.8 GB, ↑512 MB)                 │
└──────────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────────┐
│  HISTORICAL COMPARISON                                               │
├──────────────────────────────────────────────────────────────────────┤
│  vs. Previous 15 min:   +12.3% circuits, -4.2% bans  ✅ Improving   │
│  vs. Same time yesterday:  -8.7% circuits, +2.1% bans  ⚠️ Degrading │
│  vs. Last 7 days avg:  +45.2% circuits  🔴 Abnormal (attack?)       │
└──────────────────────────────────────────────────────────────────────┘
```

**Graph: Traffic Over Time**
```
  Circuits
   15000 │                                              ╭─╮
   12000 │                                         ╭────╯ ╰──╮
    9000 │                                    ╭────╯          ╰─╮
    6000 │                            ╭───────╯                 ╰─╮
    3000 │        ╭───────────────────╯                           ╰──
       0 │────────╯
         └────┬────┬────┬────┬────┬────┬────┬────┬────┬────┬────┬
           00:00  02:00  04:00  06:00  08:00  10:00  12:00  14:00
           
  Legend: 🟢 Normal  🟡 PoW  🔴 Banned
```

---

### Circuit Analysis Dashboard

**Top Banned Circuits:**
```
┌──────────────────────────────────────────────────────────────────────┐
│  CIRCUIT REPUTATION (Last 24h)                                       │
├──────────────────────────────────────────────────────────────────────┤
│  Circuit ID      │ Bans │ CAPTCHAs │ Success Rate │ Actions          │
├──────────────────┼──────┼──────────┼──────────────┼──────────────────┤
│  a3f8...b2c1     │  42  │   156    │    4.2%      │ [Unban] [Detail]│
│  7d4e...89af     │  28  │    89    │   12.3%      │ [Unban] [Detail]│
│  2c1b...45de     │  19  │    64    │   18.7%      │ [Unban] [Detail]│
│  9f3a...12cd     │  15  │    42    │   28.6%      │ [Unban] [Detail]│
└──────────────────────────────────────────────────────────────────────┘
```

**VIP Circuits:**
```
┌──────────────────────────────────────────────────────────────────────┐
│  VIP CIRCUITS (Promoted Users)                                       │
├──────────────────────────────────────────────────────────────────────┤
│  Circuit ID      │ Requests │ Uptime   │ Promoted   │ Actions       │
├──────────────────┼──────────┼──────────┼────────────┼───────────────┤
│  c8a2...4f9e     │  12,432  │ 18h 32m  │ 2d ago     │ [Demote]      │
│  1b7d...8c3a     │   8,921  │ 12h 14m  │ 4d ago     │ [Demote]      │
│  5e9f...2a1b     │   6,234  │  8h 45m  │ 1d ago     │ [Demote]      │
└──────────────────────────────────────────────────────────────────────┘
```

---

## 🎮 Manual Actions Interface

### Action Types

**Circuit Management:**
1. **Promote Circuit**: VIP status (bypass queues, priority handling)
2. **Demote Circuit**: Remove VIP, return to Normal tier
3. **Ban Circuit**: Add to ban list (30-60 min timeout)
4. **Unban Circuit**: Remove from ban list immediately
5. **Reset Circuit**: Clear reputation (neutral state)

**System Modes:**
1. **Force Attack Mode**: Aggressive defenses (PoW for all, stricter timeouts)
2. **Force Safe Mode**: Relaxed defenses (fewer CAPTCHAs, higher thresholds)
3. **Auto Mode** (default): Adaptive thresholds based on load

**Bulk Actions:**
1. **Ban IP Range** (if clearnet proxy detected): Ban all circuits from specific exit node
2. **Purge Queue**: Clear virtual queue (reject all waiting circuits)
3. **Reload Configuration**: Apply config changes without restart

### Action Flow (Web UI Example)

```
┌──────────────────────────────────────────────────────────────────────┐
│  MANUAL ACTION: Promote Circuit                                      │
├──────────────────────────────────────────────────────────────────────┤
│  Circuit ID: 7d4e89af3b2c1...                                        │
│                                                                      │
│  Current Status:  🔵 Normal                                          │
│  Reputation:      287 requests, 3 CAPTCHAs solved, 0 bans           │
│  First Seen:      2 hours ago                                        │
│  Last Activity:   12 seconds ago                                     │
│                                                                      │
│  Reason (optional):                                                  │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │ Known legitimate user, tested with admin account             │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                      │
│  Duration: ◉ Permanent    ○ 1 hour    ○ 12 hours    ○ 7 days       │
│                                                                      │
│  ⚠️  WARNING: VIP circuits bypass rate limits and queues.           │
│      Only promote trusted circuits.                                 │
│                                                                      │
│  [Confirm Promotion]  [Cancel]                                      │
└──────────────────────────────────────────────────────────────────────┘
```

**Audit Log Entry:**
```
[2026-01-28 23:45:32 UTC] PROMOTE_CIRCUIT
  Circuit: 7d4e89af3b2c1...
  Admin: operator_alice (2FA verified)
  Reason: Known legitimate user, tested with admin account
  Duration: Permanent
  Status: SUCCESS
```

---

## 🏗️ Recommended Architecture: Hybrid Approach

### Best of All Worlds

**For Most Deployments:**

1. **Grafana + Prometheus** (primary monitoring)
   - System metrics, circuit analytics, historical trends
   - Accessible via Tor Onion Service: `cerberus-mon.onion`
   - TOTP 2FA authentication

2. **Custom Admin Panel** (manual actions)
   - Minimal Rust web UI (Axum + Tera templates)
   - Actions: Promote/Demote/Ban circuits, force modes
   - Accessible via separate onion: `cerberus-admin.onion`
   - Role-based access: `readonly` vs `admin`

3. **Emergency TUI** (backup/diagnostics)
   - SSH-accessible: `cerberus-tui` command
   - Works when Tor is down or web UI inaccessible
   - Local-only, no network dependencies

**Traffic Flow:**
```
Operator's Tor Browser
        ↓
┌─────────────────────────────────────────┐
│  Tor Network (3 hops)                   │
└─────────────────────────────────────────┘
        ↓
┌─────────────────────────────────────────┐
│  cerberus-mon.onion (Grafana)           │ ← Read-only metrics
│  Port 10200                             │
└─────────────────────────────────────────┘
        ↓
┌─────────────────────────────────────────┐
│  Prometheus (Metrics Storage)           │
│  ← HAProxy Exporter                     │
│  ← Nginx Exporter                       │
│  ← Node Exporter                        │
│  ← Fortify Custom Metrics               │
└─────────────────────────────────────────┘

┌─────────────────────────────────────────┐
│  cerberus-admin.onion (Custom Panel)    │ ← Manual actions
│  Port 10201                             │
└─────────────────────────────────────────┘
        ↓
┌─────────────────────────────────────────┐
│  Fortify Admin API (Unix Socket)        │
│  /run/cerberus/admin.sock               │
└─────────────────────────────────────────┘
        ↓
  Execute: Promote/Ban/Mode Change
```

---

## 📦 Implementation Plan

### Sprint 2: Basic Metrics Collection

- [ ] Implement Fortify metrics exporter (Prometheus format)
- [ ] Configure HAProxy stats socket
- [ ] Set up Nginx stub_status module
- [ ] Install Node Exporter (system metrics)
- [ ] Create basic Prometheus scrape config

### Sprint 3: Grafana Dashboards

- [ ] Deploy Grafana as Tor Onion Service
- [ ] Create Overview Dashboard (system health, live sessions)
- [ ] Create Circuit Analysis Dashboard (top banned, VIP list)
- [ ] Configure TOTP 2FA authentication
- [ ] Set up alert rules (CPU > 90%, circuit flood detected)

### Sprint 4: Admin Panel

- [ ] Build minimal Rust web UI (Axum + Tera)
- [ ] Implement manual action forms (promote/ban/mode change)
- [ ] Create Fortify Admin API (command queue via Unix socket)
- [ ] Add audit logging (all actions logged to SQLite)
- [ ] Deploy as separate Tor Onion Service

### Sprint 5: TUI (Optional)

- [ ] Build `cerberus-tui` with ratatui framework
- [ ] Implement live metrics view (refresh every 2s)
- [ ] Add keyboard shortcuts (P=promote, B=ban, etc.)
- [ ] SSH integration guide for remote access

### Sprint 6: Remote Grafana Streaming

- [ ] Configure Prometheus remote write endpoint
- [ ] Set up Grafana Cloud agent (or self-hosted Grafana instance)
- [ ] Create connection string generator in Monitoring UI
- [ ] Test streaming over Tor (latency, reliability)
- [ ] Document clearnet IP/port access option (for low-latency monitoring)
- [ ] Implement authentication (bearer tokens for remote access)

---

## 📡 Remote Grafana Monitoring

### User Story

**As a service operator managing multiple Cerberus nodes remotely**  
**I want to stream metrics to my own Grafana instance**  
**So that I can monitor all nodes from a single dashboard without SSH/Tor access to each VPS**

**Acceptance Criteria:**
- Copy connection string from Cerberus Monitoring UI
- Paste connection string into remote Grafana as data source
- Metrics stream in real-time (< 30 second delay)
- No database storage on VPS (pure streaming, minimal resource usage)
- Support both Tor-based streaming (slow, anonymous) and clearnet IP/port (fast, requires firewall rules)

---

### Streaming Architecture Options

#### Option 1: Prometheus Remote Write (Recommended) ⭐⭐⭐

**How It Works:**
```
Cerberus Node (VPS)                 Operator's Grafana (Home/Office)
┌────────────────────┐              ┌─────────────────────────┐
│ Prometheus         │              │ Remote Grafana Instance │
│ (scrapes metrics)  │              │                         │
│                    │   HTTP POST  │  Prometheus Remote      │
│  Remote Write ────────────────────►  Write Receiver         │
│  Endpoint          │   (stream)   │                         │
└────────────────────┘              └─────────────────────────┘
                                              │
                                              ▼
                                    [View dashboards with
                                     all nodes' metrics]
```

**Pros:**
- ✅ **No database on VPS**: Prometheus stores minimal data (15 min buffer), immediately pushes to remote
- ✅ **Pure streaming**: Metrics sent as they're collected (10-30 second intervals)
- ✅ **Native Prometheus**: Works with any Prometheus-compatible receiver (Grafana Cloud, Mimir, Thanos, VictoriaMetrics)
- ✅ **Low resource usage**: ~50MB RAM for Prometheus with remote write (vs 500MB+ for full local storage)
- ✅ **Built-in retry**: If network drops, Prometheus queues samples and retries

**Cons:**
- ⚠️ Requires outbound HTTP from VPS (configure firewall to allow)
- ⚠️ Network interruption = data loss for that window (no long-term local storage)

**Configuration:**
```yaml
# /etc/prometheus/prometheus.yml
global:
  scrape_interval: 15s
  evaluation_interval: 15s
  external_labels:
    node: 'cerberus-node-1'  # Identify this node in multi-node setup
    cluster: 'production'

scrape_configs:
  - job_name: 'cerberus'
    static_configs:
      - targets:
        - 'localhost:9100'  # Node exporter (system metrics)
        - 'localhost:9101'  # HAProxy exporter
        - 'localhost:9102'  # Fortify exporter
        - 'localhost:9103'  # Nginx exporter

remote_write:
  - url: 'https://your-grafana-instance.com/api/v1/push'
    basic_auth:
      username: 'cerberus-node-1'
      password: 'your-secret-token-here'
    queue_config:
      capacity: 10000        # Buffer up to 10k samples if network slow
      max_shards: 5          # Parallel HTTP connections
      min_shards: 1
      max_samples_per_send: 500
      batch_send_deadline: 5s
    # Optional: Send via Tor for anonymity (slower)
    proxy_url: 'socks5://127.0.0.1:9050'  # Tor SOCKS5 proxy
```

**Connection String Generator** (in Monitoring UI):
```
┌────────────────────────────────────────────────────────────┐
│ Remote Monitoring Setup                                    │
├────────────────────────────────────────────────────────────┤
│ 1. Your remote Grafana URL:                                │
│    [https://your-grafana.com                    ] [Test]   │
│                                                            │
│ 2. Authentication:                                         │
│    Username: [cerberus-node-1                  ]           │
│    Password: [●●●●●●●●●●●●●●●●●●●●●●●●]           │
│                                                            │
│ 3. Connection Method:                                      │
│    ○ Clearnet (Fast, requires firewall rule)              │
│    ● Tor (Slow ~5-10 sec delay, anonymous)                │
│                                                            │
│ 4. Copy this to /etc/prometheus/prometheus.yml:           │
│ ┌────────────────────────────────────────────────────────┐ │
│ │ remote_write:                                          │ │
│ │   - url: 'https://your-grafana.com/api/v1/push'       │ │
│ │     basic_auth:                                        │ │
│ │       username: 'cerberus-node-1'                      │ │
│ │       password: 'generated-token-abc123'               │ │
│ │     proxy_url: 'socks5://127.0.0.1:9050'  # Tor       │ │
│ └────────────────────────────────────────────────────────┘ │
│                                                            │
│ [Copy to Clipboard] [Download prometheus.yml]             │
└────────────────────────────────────────────────────────────┘
```

---

#### Option 2: Grafana Agent (Lightweight Alternative) ⭐⭐

**How It Works:**
```
Cerberus Node (VPS)                 Operator's Grafana
┌────────────────────┐              ┌─────────────────────┐
│ Grafana Agent      │   Remote     │ Grafana Cloud or    │
│ (collects metrics  │   Write      │ Self-Hosted Mimir   │
│  directly, no      ├──────────────► (Receives metrics)  │
│  Prometheus)       │              │                     │
└────────────────────┘              └─────────────────────┘
```

**Pros:**
- ✅ **Even lighter**: ~30MB RAM (vs 50MB for Prometheus)
- ✅ **Drop-in replacement**: Compatible with Prometheus remote write protocol
- ✅ **Better for multi-node**: Designed for distributed monitoring
- ✅ **Native Grafana Cloud support**: One-click setup with Grafana Cloud

**Cons:**
- ⚠️ Less mature than Prometheus (newer project)
- ⚠️ Fewer ecosystem tools (Prometheus has more exporters)

**Configuration:**
```yaml
# /etc/grafana-agent/agent.yml
server:
  log_level: info

metrics:
  wal_directory: /var/lib/grafana-agent/wal
  global:
    scrape_interval: 15s
    external_labels:
      node: 'cerberus-node-1'
      cluster: 'production'
    remote_write:
      - url: 'https://your-grafana.com/api/v1/push'
        basic_auth:
          username: 'cerberus-node-1'
          password: 'your-secret-token-here'

  configs:
    - name: cerberus
      scrape_configs:
        - job_name: 'cerberus'
          static_configs:
            - targets: ['localhost:9100', 'localhost:9101', 'localhost:9102']
```

---

#### Option 3: Direct Prometheus Query API (Read-Only) ⭐

**How It Works:**
```
Cerberus Node (VPS)                 Operator's Grafana
┌────────────────────┐              ┌─────────────────────┐
│ Prometheus         │              │ Remote Grafana      │
│ (stores metrics    │   HTTP GET   │                     │
│  locally)          │◄─────────────│ Queries Prometheus  │
│                    │   /api/v1/   │ directly via API    │
│ Exposed on         │   query      │                     │
│ IP:9090 or Tor     │              │                     │
└────────────────────┘              └─────────────────────┘
```

**Pros:**
- ✅ **No configuration on VPS**: Just expose Prometheus port
- ✅ **Full local storage**: Metrics retained on VPS (useful for debugging)
- ✅ **Pull model**: VPS doesn't need outbound connectivity

**Cons:**
- ❌ **Requires inbound access**: Must expose Prometheus port (security risk)
- ❌ **More resource usage**: Prometheus stores full TSDB locally (~500MB RAM, ~2GB disk)
- ❌ **Slower over Tor**: Query latency 5-10 seconds per dashboard load

**Configuration:**
```yaml
# /etc/prometheus/prometheus.yml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'cerberus'
    static_configs:
      - targets: ['localhost:9100', 'localhost:9101', 'localhost:9102']

# Expose on specific IP (NOT 0.0.0.0 publicly!)
web:
  listen-address: '127.0.0.1:9090'  # Local only
  # OR expose via Tor hidden service

# /etc/tor/torrc - Add Prometheus as hidden service
HiddenServiceDir /var/lib/tor/cerberus-prometheus
HiddenServicePort 9090 127.0.0.1:9090
```

**Grafana Data Source Config:**
```
Name: Cerberus Node 1
Type: Prometheus
URL: http://abc123def456.onion:9090  (via Tor)
  OR http://198.51.100.10:9090       (clearnet with firewall rule)
Auth: Basic Auth (username/password)
```

---

### Comparison: Storage & Resource Usage

| Method | RAM Usage | Disk Storage | Latency | Security |
|--------|-----------|--------------|---------|----------|
| **Prometheus Remote Write** | 50MB | ~100MB (15 min buffer) | 15-30s | ✅ Outbound only |
| **Grafana Agent** | 30MB | ~50MB (WAL only) | 15-30s | ✅ Outbound only |
| **Direct Query API (Tor)** | 500MB | ~2GB (full TSDB) | 5-10s | ⚠️ Inbound via Tor |
| **Direct Query API (Clearnet)** | 500MB | ~2GB (full TSDB) | < 1s | ❌ Inbound clearnet |

**Recommendation:**
- **Low resource VPS (< 2GB RAM)**: Use **Grafana Agent** with remote write
- **High traffic monitoring**: Use **Prometheus Remote Write** (more mature, better retry logic)
- **Offline/airgapped**: Use **Direct Query API via Tor** (no outbound connectivity needed)

---

### Security Considerations

#### Clearnet IP/Port Exposure

**Problem:** Exposing Prometheus port on clearnet = attack surface

**Mitigations:**
1. **Firewall whitelist**: Only allow operator's home IP
   ```bash
   ufw allow from 203.0.113.50 to any port 9090 proto tcp
   ufw deny 9090/tcp  # Block all others
   ```

2. **VPN/WireGuard**: Operator connects to VPS via WireGuard tunnel
   ```
   Operator's Grafana ──[WireGuard Tunnel]──► VPS Prometheus (10.8.0.1:9090)
   ```

3. **HTTP Basic Auth + TLS**: Require username/password over HTTPS
   ```yaml
   # Prometheus with TLS + auth
   web:
     listen-address: '198.51.100.10:9090'
     tls_server_config:
       cert_file: /etc/prometheus/tls/cert.pem
       key_file: /etc/prometheus/tls/key.pem
   basic_auth_users:
     admin: $2y$10$hashed_password_here
   ```

4. **Rate limiting**: Prevent query DoS attacks
   ```yaml
   # Prometheus rate limiting (requires reverse proxy like Nginx)
   limit_req_zone $binary_remote_addr zone=prom:10m rate=10r/s;
   ```

#### Tor-Based Streaming

**Benefits:**
- ✅ Anonymous (VPS IP not revealed to Grafana instance)
- ✅ No firewall configuration needed
- ✅ NAT traversal (works from behind NAT)

**Drawbacks:**
- ⚠️ High latency (~5-10 seconds per batch)
- ⚠️ Tor circuit failures = temporary data loss
- ⚠️ Slower dashboard rendering

**Configuration:**
```yaml
# Prometheus remote write via Tor
remote_write:
  - url: 'http://your-grafana-onion.onion/api/v1/push'
    proxy_url: 'socks5://127.0.0.1:9050'
    queue_config:
      max_shards: 2  # Reduce parallelism (Tor doesn't benefit from many shards)
      batch_send_deadline: 30s  # Longer batches to compensate for latency
```

---

### Implementation Checklist

**For Operators:**
- [ ] Decide: Remote write (streaming) or direct query (pull)
- [ ] Choose: Clearnet (fast) or Tor (anonymous)
- [ ] Set up remote Grafana instance (Grafana Cloud or self-hosted)
- [ ] Generate bearer token or basic auth credentials
- [ ] Copy connection string from Cerberus Monitoring UI
- [ ] Paste into `/etc/prometheus/prometheus.yml` (remote write section)
- [ ] Restart Prometheus: `systemctl restart prometheus`
- [ ] Verify metrics flowing: Check Grafana data source "Last Scraped"
- [ ] Create dashboards for all Cerberus nodes
- [ ] Set up alerts (CPU > 90%, circuit flood, node offline)

**For Development:**
- [ ] Add "Remote Monitoring Setup" page to Monitoring UI
- [ ] Implement connection string generator (pre-fill YAML config)
- [ ] Add test button (verify Grafana URL is reachable)
- [ ] Support multiple remote write destinations (primary + backup)
- [ ] Document troubleshooting (firewall issues, Tor failures, auth errors)
- [ ] Create example Grafana dashboards (JSON exports)

---

## 🔒 Security Hardening

### Tor-Only Access

```
# /etc/tor/torrc - Monitoring Onion Services

# Grafana (read-only metrics)
HiddenServiceDir /var/lib/tor/cerberus-mon
HiddenServicePort 80 127.0.0.1:10200
HiddenServiceVersion 3

# Admin Panel (write access)
HiddenServiceDir /var/lib/tor/cerberus-admin
HiddenServicePort 80 127.0.0.1:10201
HiddenServiceVersion 3
HiddenServiceAuthorizeClient stealth admin1,admin2  # Client auth required
```

### Authentication Layers

**Grafana:**
1. TOTP 2FA (Google Authenticator, Authy)
2. Strong passwords (20+ chars, generated)
3. Session timeout (15 min idle)
4. IP allowlist (Tor circuits only, no clearnet)

**Admin Panel:**
1. TOTP 2FA (mandatory)
2. Client authorization (Tor stealth auth)
3. Rate limiting (5 actions/min per user)
4. Audit logging (all actions immutable)

### Least Privilege

```bash
# Separate service users
useradd -r -s /bin/false prometheus
useradd -r -s /bin/false grafana
useradd -r -s /bin/false cerberus-admin

# Read-only access to metrics
chmod 440 /run/cerberus/metrics.sock
chown cerberus:prometheus /run/cerberus/metrics.sock

# Write access to admin API (admin panel only)
chmod 660 /run/cerberus/admin.sock
chown cerberus:cerberus-admin /run/cerberus/admin.sock
```

---

## 📊 Grafana Dashboard Panels (Detailed)

### Dashboard 1: System Overview

**Row 1: Resource Gauges**
- Panel: CPU Usage (gauge, 0-100%)
- Panel: RAM Usage (gauge, 0-100%)
- Panel: Disk Usage (gauge, 0-100%)
- Panel: Network I/O (stat, Mbps in/out)

**Row 2: Live Sessions**
- Panel: Circuit Status Breakdown (pie chart: VIP/PoW/Normal/Banned/Queue)
- Panel: Session Count Timeline (line graph, last 6 hours)
- Panel: Current Capacity (bar gauge: active / max configured)

**Row 3: Snapshot Metrics**
- Panel: Requests/sec (stat with sparkline)
- Panel: CAPTCHA Success Rate (gauge, 0-100%)
- Panel: Ban Rate (stat, bans/min)
- Panel: Avg Response Time (stat, ms)

**Row 4: Recent Events**
- Panel: Attack Events Log (table, last 50 events)
- Panel: Error Rate (graph, errors/min over last 24h)

---

### Dashboard 2: Circuit Analysis

**Row 1: Top Lists**
- Panel: Top Banned Circuits (table: Circuit ID, Ban Count, Reason)
- Panel: Top VIP Circuits (table: Circuit ID, Request Count, Uptime)
- Panel: Top Endpoints (table: Path, Request Count, % of Total)

**Row 2: Reputation Trends**
- Panel: Bans Over Time (area graph, stacked by reason)
- Panel: VIP Promotions Over Time (line graph)
- Panel: CAPTCHA Difficulty Distribution (histogram)

**Row 3: Behavioral Analysis**
- Panel: Request Rate Distribution (heatmap: circuit vs req/min)
- Panel: Endpoint Diversity (gauge: unique endpoints per circuit)
- Panel: Timing Patterns (scatter plot: request intervals)

---

### Dashboard 3: Historical Reports

**Snapshot Selector:**
- Variable: `$timewindow` (5m, 15m, 30m, 1h, 2h, 4h, 8h, 12h, 24h, 72h, 7d, 30d, 90d, 365d)

**Dynamic Panels:**
- Panel: New Circuits (stat, compare to previous period)
- Panel: Bandwidth Used (graph, in/out)
- Panel: Attack Events (table, filtered by timewindow)
- Panel: Performance Metrics (table: p50/p95/p99 latency)

---

## 🚨 Alerting Rules

### Critical Alerts (Immediate Action)

1. **System Resources:**
   - CPU > 90% for 5 minutes → Alert: "CPU overload, possible attack"
   - RAM > 95% for 2 minutes → Alert: "Memory exhaustion"
   - Disk > 95% → Alert: "Disk space critical"

2. **Attack Detection:**
   - Circuit flood: +1000 circuits in 60s → Alert: "Circuit flood attack"
   - Ban rate > 100/min → Alert: "Aggressive blocking, investigate"
   - CAPTCHA success rate < 20% → Alert: "CAPTCHA bypass attempt?"

3. **Service Health:**
   - HAProxy down → Alert: "Layer 1 offline, service exposed"
   - Nginx down → Alert: "Layer 2 offline, CAPTCHA gate broken"
   - Fortify down → Alert: "Layer 3 offline, no circuit reputation"

### Warning Alerts (Monitor)

1. CPU > 75% for 15 minutes
2. Ban rate > 50/min for 10 minutes
3. CAPTCHA solve time > 500ms (performance degradation)
4. Queue depth > 500 circuits for 5 minutes

**Alert Delivery:**
- Matrix bot via Tor (send to homeserver over onion service)
- Telegram bot via Tor (webhooks over Tor SOCKS proxy)
- Email via Tor (send through Tor exit, use ProtonMail SMTP bridge)

---

## 📝 User Roles and Permissions

### Role Definitions

**1. Admin (Full Access)**
- ✅ View all metrics and dashboards
- ✅ Execute manual actions (promote/ban/mode change)
- ✅ Modify configuration
- ✅ Access audit logs
- ✅ Create/delete other user accounts

**2. Operator (Limited Write)**
- ✅ View all metrics and dashboards
- ✅ Execute manual actions (promote/ban only, no mode changes)
- ❌ Cannot modify configuration
- ✅ View audit logs (read-only)
- ❌ Cannot manage users

**3. Monitor (Read-Only)**
- ✅ View all metrics and dashboards
- ❌ Cannot execute manual actions
- ❌ Cannot modify configuration
- ✅ View audit logs (read-only)
- ❌ Cannot manage users

**4. Auditor (Logs Only)**
- ❌ Cannot view live metrics
- ❌ Cannot execute actions
- ❌ Cannot modify configuration
- ✅ View audit logs (read-only, full historical access)
- ❌ Cannot manage users

---

## 📖 References

- **Grafana Documentation**: https://grafana.com/docs/
- **Prometheus Exporters**: https://prometheus.io/docs/instrumenting/exporters/
- **Ratatui (Rust TUI)**: https://github.com/ratatui-org/ratatui
- **Axum Web Framework**: https://github.com/tokio-rs/axum
- **HAProxy Prometheus Exporter**: https://github.com/prometheus/haproxy_exporter
- **Nginx Prometheus Exporter**: https://github.com/nginxinc/nginx-prometheus-exporter

---

**Status**: 📝 Design Document (Implementation in Sprint 3-4)
