# Threat Dial: Dynamic Defense Intensity Control

**Adaptive Defense Multiplier System for Real-Time Attack Response**

---

## 📋 Concept Overview

A **Threat Dial** is a single control (-10 to +10) that dynamically adjusts **all defense thresholds** simultaneously. Operators can dial up during attacks (stricter limits) or dial down during normal traffic (relaxed limits) without restarting services or editing configs.

### Mental Model

```
    ← RELAXED                    STRICT →
-10 ─────── -5 ─────── 0 ─────── +5 ─────── +10
 ▼                      ▼                      ▼
Permit               Default              Maximum
Everything           Settings             Defense

Multiplier:
2.0x limits          1.0x limits          0.1x limits
(double capacity)    (baseline)           (90% reduction)
```

### Example Behavior

**Scenario**: Rate limit normally set to 100 requests/min

```
Dial Position: -10  → 100 * 2.0  = 200 req/min  (very permissive)
Dial Position: -5   → 100 * 1.5  = 150 req/min
Dial Position:  0   → 100 * 1.0  = 100 req/min  (default)
Dial Position: +5   → 100 * 0.5  =  50 req/min
Dial Position: +10  → 100 * 0.1  =  10 req/min  (lockdown)
```

---

## 🎯 Problem Being Solved

### Current Limitation: Binary Modes

Existing design has two modes:
1. **Safe Mode**: Relaxed defenses (low attack traffic)
2. **Attack Mode**: Aggressive defenses (under DDoS)

**Problem:**
- ❌ Too coarse (only 2 states)
- ❌ No granularity (can't fine-tune response)
- ❌ Binary switching is jarring (users suddenly blocked)
- ❌ May over-react (light attacks trigger full lockdown)

### Threat Dial Advantages

**Granular Control:**
- ✅ 21 positions (-10 to +10) = gradual response
- ✅ Dial up incrementally (test if attack stops)
- ✅ Dial down slowly (avoid reopening floodgates)

**Operator Intuition:**
- ✅ Simple mental model (one knob)
- ✅ Reversible (easy to undo if wrong dial setting)
- ✅ Predictable (multiplier applies uniformly)

**Adaptive Response:**
- ✅ Light attack: Dial to +2 (20% reduction)
- ✅ Moderate attack: Dial to +5 (50% reduction)
- ✅ Severe attack: Dial to +10 (90% reduction)

---

## ✅ Feasibility Analysis

### Technical Viability: ⭐⭐⭐⭐⭐ (Very High)

**Implementation Complexity: LOW**

No new infrastructure needed:
- ✅ Modifies existing thresholds (HAProxy/Nginx/Fortify configs)
- ✅ Simple arithmetic (multiply by scalar)
- ✅ Rust/Bash can calculate in microseconds
- ✅ No external dependencies

**Compared to alternatives:**
- Simpler than ML-based adaptive systems
- Easier than per-circuit manual tuning
- More flexible than hard-coded modes

### Operational Viability: ⭐⭐⭐⭐⭐ (Perfect)

**Operator Perspective:**

```
During Attack:
1. Open admin UI
2. See current dial position: 0 (default)
3. Drag slider to +5 (or click "Dial Up" button)
4. Watch metrics: Attack slowing down?
   - Yes: Keep at +5
   - No: Dial to +8
5. Attack stopped: Dial back to +2 (cautious), then 0 after 10 min
```

**Benefits:**
- ⏱️ **Fast**: Adjust in <5 seconds
- 🔄 **Reversible**: Dial back if wrong
- 📊 **Observable**: See immediate metric changes
- 🧠 **Intuitive**: No need to understand HAProxy syntax

---

## 🏗️ Architecture Design

### Affected Parameters

**The dial multiplier applies to:**

#### 1. HAProxy (Layer 1)

```bash
# Base configuration (Dial = 0)
CONN_RATE_LIMIT=100          # connections/min per circuit
CONN_CUR_MAX=10              # concurrent connections per circuit
STICK_TABLE_SIZE=10000       # max tracked circuits
SLOWLORIS_TIMEOUT=10         # seconds
QUEUE_MAX=1000               # virtual queue depth

# With Dial = +5 (multiplier = 0.5)
CONN_RATE_LIMIT=50           # 100 * 0.5
CONN_CUR_MAX=5               # 10 * 0.5
STICK_TABLE_SIZE=5000        # 10000 * 0.5 (track fewer circuits)
SLOWLORIS_TIMEOUT=5          # 10 * 0.5 (faster timeouts)
QUEUE_MAX=500                # 1000 * 0.5 (shorter queue)

# With Dial = -5 (multiplier = 1.5)
CONN_RATE_LIMIT=150          # 100 * 1.5
CONN_CUR_MAX=15              # 10 * 1.5
STICK_TABLE_SIZE=15000       # 10000 * 1.5
SLOWLORIS_TIMEOUT=15         # 10 * 1.5
QUEUE_MAX=1500               # 1000 * 1.5
```

#### 2. Nginx (Layer 2)

```bash
# Base configuration (Dial = 0)
REQ_RATE_LIMIT=60            # requests/min per circuit
REQ_BURST=10                 # burst allowance
CLIENT_TIMEOUT=30            # seconds
BUFFER_SIZE=8192             # bytes

# With Dial = +5 (multiplier = 0.5)
REQ_RATE_LIMIT=30            # 60 * 0.5
REQ_BURST=5                  # 10 * 0.5
CLIENT_TIMEOUT=15            # 30 * 0.5
BUFFER_SIZE=4096             # 8192 * 0.5

# With Dial = -5 (multiplier = 1.5)
REQ_RATE_LIMIT=90            # 60 * 1.5
REQ_BURST=15                 # 10 * 0.5
CLIENT_TIMEOUT=45            # 30 * 1.5
BUFFER_SIZE=12288            # 8192 * 1.5
```

#### 3. Fortify (Layer 3 - Rust)

```rust
// Base configuration (Dial = 0)
CAPTCHA_DIFFICULTY=6         // characters in CAPTCHA
CAPTCHA_TTL=300              // seconds
CAPTCHA_RETRY_MAX=3          // attempts before ban
POW_DIFFICULTY=18            // bits (for PoW challenges)
CIRCUIT_BAN_DURATION=1800    // seconds (30 min)

// With Dial = +5 (multiplier = 0.5)
CAPTCHA_DIFFICULTY=3         // 6 * 0.5 (easier, but doesn't make sense for char count)
                             // Better: Keep difficulty, reduce TTL
CAPTCHA_TTL=150              // 300 * 0.5 (must solve faster)
CAPTCHA_RETRY_MAX=1          // 3 * 0.5 = 1.5 → round down to 1
POW_DIFFICULTY=27            // 18 * 1.5 (INCREASE, harder for attackers)
CIRCUIT_BAN_DURATION=900     // 1800 * 0.5 (shorter bans? NO, should increase)

// CORRECTION: Some params should INVERT
// Higher dial = HARDER for users, LONGER bans
POW_DIFFICULTY=18 * (1 + (dial/10))  // Dial +10 → 36 bits (2^18 harder)
CIRCUIT_BAN_DURATION=1800 * (1 + (dial/10))  // Dial +10 → 3600s (1 hour)
```

### Multiplier Calculation

**Formula:**
```rust
fn calculate_multiplier(dial: i8) -> f32 {
    match dial {
        -10..=-1 => 1.0 + (dial.abs() as f32 * 0.1),  // -10 → 2.0, -5 → 1.5
        0        => 1.0,                                // baseline
        1..=10   => 1.0 - (dial as f32 * 0.09),        // +10 → 0.1, +5 → 0.55
    }
}

// Examples
calculate_multiplier(-10) = 2.0   // 100% increase
calculate_multiplier(-5)  = 1.5   // 50% increase
calculate_multiplier(0)   = 1.0   // baseline
calculate_multiplier(5)   = 0.55  // 45% reduction
calculate_multiplier(10)  = 0.1   // 90% reduction
```

**Why not linear from +10 to -10?**
- Need asymmetry: +10 should be extreme lockdown (0.1x)
- -10 should be permissive but not reckless (2.0x, not 10x)

---

## ⚙️ Configuration

### cerberus.conf Settings

```ini
[ThreatDial]
# Enable dynamic threat dial (if false, always uses dial=0)
THREAT_DIAL_ENABLED=true

# Current dial position (-10 to +10)
# Default: 0 (baseline defenses)
THREAT_DIAL_POSITION=0

# Auto-adjustment (experimental)
# If true, dial adjusts automatically based on metrics
THREAT_DIAL_AUTO=false

# Auto-dial thresholds (only if THREAT_DIAL_AUTO=true)
# If new circuits/min > threshold, dial up
AUTO_DIAL_UP_THRESHOLD=500   # circuits/min
AUTO_DIAL_UP_INCREMENT=1     # dial += 1 per minute above threshold
AUTO_DIAL_DOWN_THRESHOLD=100 # circuits/min
AUTO_DIAL_DOWN_INCREMENT=1   # dial -= 1 per minute below threshold
AUTO_DIAL_MAX=8              # never auto-dial above +8 (prevent lockout)
AUTO_DIAL_MIN=-3             # never auto-dial below -3 (prevent abuse)

# Dial change rate limiting (prevent flapping)
THREAT_DIAL_MIN_INTERVAL=60  # seconds between dial changes
```

---

## 🎮 User Interface Design

### Admin Panel: Threat Dial Control

**Desktop UI (Grafana/Web Dashboard):**

```
┌──────────────────────────────────────────────────────────────────┐
│  THREAT DIAL CONTROL                         [Auto Mode: OFF]    │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Current Position: +5                                            │
│                                                                  │
│      RELAXED                              STRICT                 │
│  -10 ─────── -5 ─────── 0 ─────── +5 ─────── +10               │
│   ●           ●          ●          🔴          ●                │
│   ▲                      ▲                      ▲                │
│  2.0x                  1.0x                   0.1x               │
│  Permit All            Default              Lockdown             │
│                                                                  │
│  [◀ Dial Down]  [Reset to 0]  [Dial Up ▶]                      │
│                                                                  │
│  Multiplier: 0.55x                                               │
│  Effect: All rate limits reduced to 55% of baseline             │
│                                                                  │
│  Last Changed: 2 minutes ago by admin_alice                     │
│  Reason: "Moderate DDoS attack detected, dialing up"            │
│                                                                  │
│  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━  │
│                                                                  │
│  AFFECTED PARAMETERS (Preview)                                   │
│  ├─ HAProxy: Conn Rate 100→55/min, Conn Max 10→5               │
│  ├─ Nginx:   Req Rate 60→33/min, Timeout 30→16s                │
│  └─ Fortify: CAPTCHA TTL 300→165s, Ban 30→16min                │
│                                                                  │
│  [Apply Changes]  [Revert]                                      │
└──────────────────────────────────────────────────────────────────┘
```

**Mobile UI (Tor Browser on Phone):**

```
┌────────────────────────────────┐
│  🛡️ Threat Dial                │
├────────────────────────────────┤
│  Position: +5 (Strict)         │
│  Multiplier: 0.55x             │
│                                │
│  [◀◀ -5]  [◀ -1]  [0]          │
│  [+1 ▶]   [+5 ▶▶]              │
│                                │
│  Last: 2m ago by admin_alice   │
│  "Moderate DDoS detected"      │
│                                │
│  [Apply]                       │
└────────────────────────────────┘
```

### TUI (Terminal UI via SSH)

```
┌──────────────────────────────────────────────────────────────────┐
│  THREAT DIAL: +5                                [a]uto [m]anual  │
├──────────────────────────────────────────────────────────────────┤
│   -10    -5     0     +5    +10                                  │
│    ●      ●     ●     🔴     ●                                   │
│  [←][→] adjust   [r]eset   [q]uit                               │
│                                                                  │
│  Multiplier: 0.55x  (45% reduction)                              │
│  Applied: 2 minutes ago                                          │
│                                                                  │
│  Parameters affected:                                            │
│  • HAProxy conn rate: 100 → 55/min                               │
│  • Nginx req rate: 60 → 33/min                                   │
│  • Fortify ban duration: 30 → 16 min                             │
└──────────────────────────────────────────────────────────────────┘
```

---

## 🔧 Implementation Details

### Dynamic Config Reload

**Challenge**: Changing dial should update configs without restarting services

**Solution 1: HAProxy Runtime API**
```bash
# Update stick table limits dynamically
echo "set table cerberus_circuits key <circuit_id> data conn_rate $NEW_LIMIT" \
    | socat stdio /run/haproxy/admin.sock

# Update global rate limits (requires HAProxy 2.8+)
echo "set rate-limit connections global $NEW_LIMIT" \
    | socat stdio /run/haproxy/admin.sock
```

**Solution 2: Nginx Reload (Graceful)**
```bash
# Generate new config with adjusted limits
./scripts/generate-nginx-config.sh --dial $DIAL_POSITION

# Graceful reload (no dropped connections)
nginx -s reload
```

**Solution 3: Fortify Hot Reload**
```rust
// Fortify watches for config changes (inotify)
use notify::Watcher;

let (tx, rx) = channel();
let mut watcher = notify::watcher(tx, Duration::from_secs(1))?;
watcher.watch("/etc/cerberus/cerberus.conf", RecursiveMode::NonRecursive)?;

// On config change, reload dial position
loop {
    match rx.recv() {
        Ok(event) => {
            let new_dial = parse_config().threat_dial_position;
            apply_dial_adjustments(new_dial)?;
            log_info!("Threat dial updated to: {}", new_dial);
        }
        Err(e) => log_error!("Watch error: {}", e),
    }
}
```

### Dial Change Flow

```
1. Admin sets dial to +5 via UI
        ↓
2. UI sends POST /api/dial/set?position=5
        ↓
3. Fortify validates (-10 ≤ 5 ≤ +10) ✅
        ↓
4. Update cerberus.conf: THREAT_DIAL_POSITION=5
        ↓
5. Calculate multiplier: 0.55
        ↓
6. Regenerate configs:
   - haproxy.cfg (conn_rate = 100 * 0.55 = 55)
   - nginx.conf (req_rate = 60 * 0.55 = 33)
        ↓
7. Reload services:
   - HAProxy: socat command (instant)
   - Nginx: nginx -s reload (graceful)
   - Fortify: inotify triggers reload (instant)
        ↓
8. Log to audit trail:
   [2026-01-28 23:45:12] DIAL_CHANGE
     From: 0 → +5
     By: admin_alice
     Reason: "Moderate DDoS attack detected"
        ↓
9. Notify monitoring UI: Update dial widget
```

---

## 📊 Auto-Dial Mode (Optional)

### Automatic Adjustment Based on Metrics

**Concept**: System dials up/down automatically based on attack indicators

**Triggers:**

```rust
// Check metrics every 60 seconds
let circuits_per_min = get_new_circuits_last_minute();
let ban_rate = get_bans_last_minute();
let captcha_fail_rate = get_captcha_failures_last_minute();

// Dial UP conditions (attack detected)
if circuits_per_min > 500 || ban_rate > 100 || captcha_fail_rate > 80 {
    if dial_position < 8 && last_dial_change > 60_seconds {
        dial_position += 1;
        log_warn!("Auto-dial UP to +{}: High attack traffic", dial_position);
        apply_dial(dial_position)?;
    }
}

// Dial DOWN conditions (attack subsiding)
if circuits_per_min < 100 && ban_rate < 20 && captcha_fail_rate < 30 {
    if dial_position > 0 && last_dial_change > 120_seconds {
        dial_position -= 1;
        log_info!("Auto-dial DOWN to +{}: Traffic normalizing", dial_position);
        apply_dial(dial_position)?;
    }
}
```

**Safety Limits:**
- Max auto-dial: +8 (prevent full lockout, +10 requires manual intervention)
- Min auto-dial: -3 (prevent abuse, lower requires manual approval)
- Rate limit: Max 1 dial change per minute (prevent flapping)

**Manual Override:**
```rust
// Admin can force dial position (disables auto for 30 minutes)
if admin_manual_dial {
    AUTO_DIAL_ENABLED = false;
    AUTO_DIAL_RESUME_AT = now + 30_minutes;
    log_info!("Auto-dial disabled by manual override (resume in 30m)");
}
```

---

## 🔐 Security Considerations

### Audit Logging

**All dial changes must be logged:**
```sql
CREATE TABLE threat_dial_log (
    id INTEGER PRIMARY KEY,
    timestamp INTEGER NOT NULL,
    previous_position INTEGER NOT NULL,
    new_position INTEGER NOT NULL,
    changed_by TEXT NOT NULL,  -- username or "auto-dial"
    reason TEXT,
    manual BOOLEAN NOT NULL,   -- true = manual, false = auto
    duration_seconds INTEGER   -- time spent at previous position
);
```

**Example Entries:**
```
[2026-01-28 23:30:00] DIAL_CHANGE: 0 → +2  (manual, admin_alice, "Light attack detected")
[2026-01-28 23:35:00] DIAL_CHANGE: +2 → +5 (manual, admin_alice, "Attack intensifying")
[2026-01-28 23:50:00] DIAL_CHANGE: +5 → +8 (auto, "circuits_per_min=847 > threshold")
[2026-01-29 00:10:00] DIAL_CHANGE: +8 → +5 (auto, "traffic normalizing")
[2026-01-29 00:30:00] DIAL_CHANGE: +5 → 0  (manual, admin_alice, "Attack resolved")
```

### Access Control

**Only admin role can change dial:**
```rust
// In Fortify admin API
if user.role != "admin" {
    return Err("Dial control requires admin privileges");
}

// Readonly users can VIEW dial position but not modify
if user.role == "readonly" {
    return dial_position;  // GET only
}
```

### Prevent Abuse

**Scenario**: Malicious admin sets dial to -10 (2x capacity) → opens floodgates

**Mitigation 1: Approval for Extreme Positions**
```rust
if dial_position < -5 || dial_position > 8 {
    // Require 2FA confirmation
    if !verify_totp(user, totp_code) {
        return Err("Extreme dial positions require 2FA");
    }
    
    // Log with extra detail
    log_critical!("EXTREME DIAL: {} set to {}", user, dial_position);
}
```

**Mitigation 2: Rate Limiting**
```rust
// Prevent rapid dial changes (flapping)
if last_dial_change < 60_seconds_ago {
    return Err("Dial changes limited to 1/minute");
}
```

**Mitigation 3: Auto-Revert**
```rust
// If dial set to -10, auto-revert after 10 minutes
if dial_position <= -8 {
    schedule_auto_revert(dial_position, 10_minutes);
    log_warn!("Extreme permissive dial will auto-revert in 10 minutes");
}
```

---

## 📊 Metrics and Monitoring

### Dashboard Widgets

**Threat Dial History Graph:**
```
  Dial
  +10 │                                              ╭─╮
   +8 │                                         ╭────╯ ╰──╮
   +5 │                                    ╭────╯          ╰─╮
   +2 │                            ╭───────╯                 ╰─╮
    0 │────────────────────────────╯                           ╰──
   -5 │
  -10 │
      └────┬────┬────┬────┬────┬────┬────┬────┬────┬────┬
        22:00  22:30  23:00  23:30  00:00  00:30  01:00

  Annotations:
  23:15 - "Manual dial to +5: DDoS attack" (admin_alice)
  23:45 - "Auto-dial to +8: circuits_per_min=847"
  00:20 - "Manual dial to 0: Attack resolved" (admin_alice)
```

**Dial Impact Metrics:**
```
┌──────────────────────────────────────────────────────────────┐
│  DIAL IMPACT (Last 1 hour)                                   │
├──────────────────────────────────────────────────────────────┤
│  Average Dial: +4.2                                          │
│  Time in Attack Mode (dial > 0): 45 minutes (75%)            │
│  Time in Safe Mode (dial < 0): 0 minutes (0%)                │
│  Time in Default (dial = 0): 15 minutes (25%)                │
│                                                              │
│  Effectiveness:                                              │
│  • Banned circuits: 234 (vs 89 baseline = +163%)            │
│  • Legitimate users blocked: 12 (vs 0 baseline)              │
│  • Attack traffic reduced: 78% (from 2400 → 530 req/min)    │
└──────────────────────────────────────────────────────────────┘
```

---

## ⚠️ Risks and Mitigations

### Risk 1: False Positives (Dial Too High)

**Scenario**: Dial set to +10 during false alarm → legitimate users blocked

**Mitigation**:
- Monitor "legitimate user blocks" metric (increase in CAPTCHA failures from known-good circuits)
- Alert if dial > +5 for > 30 minutes: "Extended high dial may block legitimate users"
- Recommend dial +5 max for most attacks (only +8/+10 for severe)

### Risk 2: Operator Confusion (What Should I Set?)

**Scenario**: New operator doesn't know if dial +3 or +7 is appropriate

**Mitigation**:
- Provide dial presets:
  ```
  Light Attack:    +2 to +4  (20-40% reduction)
  Moderate Attack: +5 to +7  (50-70% reduction)
  Severe Attack:   +8 to +10 (80-90% reduction)
  ```
- Show real-time impact preview: "Setting dial to +5 will reduce conn_rate to X"
- Auto-dial mode (if enabled) handles it automatically

### Risk 3: Dial Flapping (Oscillation)

**Scenario**: Auto-dial bounces +3 → +5 → +3 → +5 every minute

**Mitigation**:
- Hysteresis: Use different thresholds for dial-up vs dial-down
  ```rust
  // Dial UP if traffic > 500/min
  // Dial DOWN if traffic < 100/min (not 499/min)
  // Prevents oscillation around single threshold
  ```
- Rate limiting: Max 1 dial change per minute
- Require sustained condition (e.g., 3 minutes above threshold before dial up)

### Risk 4: Unintended Side Effects

**Scenario**: Dial to +10 → some parameter multiplied incorrectly → service breaks

**Mitigation**:
- Unit tests for all multiplier calculations
- Dry-run mode: Preview config changes before applying
- Staged rollout: Apply to HAProxy first, then Nginx, then Fortify (abort if errors)

---

## 🔍 Comparison to Alternatives

### Threat Dial vs Binary Modes

| Aspect | Threat Dial | Binary Modes (Attack/Safe) |
|--------|-------------|----------------------------|
| **Granularity** | 21 positions | 2 positions |
| **Flexibility** | ✅ Fine-tune | ❌ All-or-nothing |
| **Ease of Use** | ✅ Intuitive slider | ⚠️ Mode switch |
| **Reversibility** | ✅ Gradual dial-down | ⚠️ Abrupt switch |
| **Complexity** | ⚠️ Moderate (multipliers) | ✅ Simple (if/else) |

**Verdict**: Threat Dial is superior for production deployments

### Threat Dial vs Manual Config Edits

| Aspect | Threat Dial | Manual Edits |
|--------|-------------|--------------|
| **Speed** | <5 seconds | Minutes (edit, test, reload) |
| **Reversibility** | ✅ One click | ❌ Re-edit configs |
| **Risk** | ⚠️ Moderate (wrong dial) | 🔴 High (syntax errors break service) |
| **Audit Trail** | ✅ Automatic logging | ❌ Manual (if remembered) |

**Verdict**: Threat Dial is far safer and faster

### Threat Dial vs ML Auto-Scaling

| Aspect | Threat Dial | ML Auto-Scaling |
|--------|-------------|-----------------|
| **Predictability** | ✅ Known multiplier | ❌ Black box |
| **Operator Control** | ✅ Full control | ⚠️ Limited |
| **Implementation** | ✅ Simple (arithmetic) | 🔴 Complex (ML models) |
| **Resource Usage** | ✅ Minimal | ⚠️ High (model inference) |

**Verdict**: Threat Dial for Sprint 3, ML for Sprint 5 (research)

---

## 🛠️ Implementation Plan

### Sprint 2: Foundation (Multiplier System)

- [ ] Define base parameters (all rate limits, timeouts, etc.)
- [ ] Implement multiplier calculation function
- [ ] Add `THREAT_DIAL_POSITION` to cerberus.conf
- [ ] Create config generation scripts (apply multiplier to base params)
- [ ] Test: Generate configs at dial -10, 0, +10 (verify math)

### Sprint 3: Manual Dial Control

- [ ] Add dial control API to Fortify (`POST /api/dial/set?position=5`)
- [ ] Implement HAProxy runtime updates (socat commands)
- [ ] Implement Nginx graceful reload
- [ ] Implement Fortify hot reload (inotify config watcher)
- [ ] Create admin UI widget (slider + preview)
- [ ] Add audit logging (threat_dial_log table)
- [ ] Security: RBAC (only admin role can change dial)

### Sprint 4: Auto-Dial Mode

- [ ] Implement metric collection (circuits/min, ban rate, etc.)
- [ ] Define auto-dial thresholds (when to dial up/down)
- [ ] Implement auto-dial logic (background thread in Fortify)
- [ ] Add safety limits (max +8, min -3, rate limit 1/min)
- [ ] Manual override (disable auto for 30 min after manual change)
- [ ] Dashboard: Dial history graph, auto-dial annotations

### Sprint 5: Advanced Features

- [ ] Dial presets (Light/Moderate/Severe attack buttons)
- [ ] Scheduled dial changes (e.g., dial to +2 during peak hours)
- [ ] A/B testing (dial to +5 for 50% of circuits, measure impact)
- [ ] Integration with alerting (Slack/Matrix: "Auto-dial to +8")

---

## 📖 References

- **HAProxy Runtime API**: https://www.haproxy.com/blog/dynamic-configuration-haproxy-runtime-api/
- **Nginx Graceful Reload**: https://nginx.org/en/docs/control.html
- **Control Theory (Hysteresis)**: https://en.wikipedia.org/wiki/Hysteresis
- **Adaptive Systems Design**: https://martinfowler.com/articles/patterns-of-distributed-systems/

---

## ✅ Recommendation: PROCEED

**Verdict**: ⭐⭐⭐⭐⭐ Highly Recommended

**Why:**
- ✅ Simple to implement (arithmetic multiplier)
- ✅ Intuitive for operators (one knob)
- ✅ Reversible and safe (easy to undo)
- ✅ Solves real problem (binary modes too coarse)
- ✅ Low resource overhead (no ML, no external deps)

**When to Implement:**
- **Sprint 2-3**: Foundation + manual dial
- **Sprint 4**: Auto-dial mode (optional but recommended)

**Priority Level:**
- 🔴 **Critical** for production deployments
- 🟢 Medium for personal/hobby projects (binary modes sufficient)

**Alternative if Rejected:**
- Fallback to binary Attack/Safe modes (already in design)
- Still better than no adaptive response

---

**Status**: 📝 Design Document (Implementation in Sprint 2-3)
