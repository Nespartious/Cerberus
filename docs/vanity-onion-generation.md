# Vanity Onion Address Generation with mkp224o

**Integration of mkp224o for Automated Vanity .onion Address Creation**

---

## 📋 Overview

Cerberus will integrate **mkp224o** to automatically generate vanity `.onion` addresses that match the protected backend service's address. This provides operational security benefits by allowing operators to create recognizable, branded addresses or maintain address continuity when rotating services.

### What is mkp224o?

[mkp224o](https://github.com/cathugger/mkp224o) is a high-performance vanity address generator for Tor V3 onion services. It uses:
- **Multi-threaded CPU mining** (AVX2/SSE2 optimized)
- **Ed25519 key generation** with filtering
- **Blazing fast**: ~1M keys/second on modern CPUs (64-core can reach 50M+)

### Why Vanity Addresses?

**Operational Benefits:**
- **Brand Recognition**: Match your backend service name (e.g., `marketabc...onion` → `marketabc...onion`)
- **Trust Signals**: Users recognize legitimate addresses vs phishing clones
- **Address Rotation**: Generate new addresses matching old prefixes when rotating services
- **Memorability**: Easier for users to remember and verify addresses

**Example Use Case:**
```
Backend Service:  market7xjd4...onion
Cerberus Frontend: market7xjd4...onion (same first 11 chars)
                   ↑ Vanity match for continuity
```

---

## 🎯 Default Behavior: Address Replication

### Automatic Prefix Matching

**Default Mode**: Cerberus will attempt to replicate the **first 6 characters** of the protected backend service's `.onion` address.

```bash
# Example workflow
Backend Address: market7xjd4abc123...onion
Target Prefix:   market7  (first 6 chars)

mkp224o generates: market7abc123xyz...onion
                   ↑ Match!
```

### Why 6 Characters?

| Prefix Length | Estimated Time | Probability |
|---------------|----------------|-------------|
| 4 chars | ~1 second | 1 in 1.1M |
| 5 chars | ~10 seconds | 1 in 33M |
| **6 chars** | **~5 minutes** | **1 in 1.07B** |
| 7 chars | ~2.5 hours | 1 in 34B |
| 8 chars | ~3 days | 1 in 1.1T |
| 9 chars | ~4 months | 1 in 35T |
| 10 chars | ~11 years | 1 in 1.15 quadrillion |

**Balance**: 6 characters provides reasonable generation time (~5 minutes) while maintaining recognizable branding.

---

## ⚙️ Configuration

### cerberus.conf Settings

```ini
[VanityOnion]
# Enable automatic vanity address generation
VANITY_ENABLED=true

# Prefix to search for (auto-detected from backend or manual)
# If empty, will use first 6 chars of TARGET_ONION
VANITY_PREFIX=

# Number of characters to match (4-12 supported)
# Default: 6 (optimal balance of time vs recognizability)
VANITY_LENGTH=6

# Maximum time to spend generating (in minutes)
# Default: 5 minutes, Max: 7680 (128 hours)
VANITY_TIMEOUT_MINUTES=5

# Require user approval before starting generation
# If true, cerberus.sh will prompt before running mkp224o
VANITY_REQUIRE_APPROVAL=true

# Number of CPU threads to dedicate (0 = auto-detect all cores)
VANITY_THREADS=0

# Case sensitivity (true = exact case match, false = case-insensitive)
# Note: Onion addresses are base32 (a-z, 2-7), so case doesn't matter for V3
VANITY_CASE_SENSITIVE=false

# Fallback behavior if timeout reached
# Options: USE_RANDOM (generate standard random address), ABORT (stop deployment)
VANITY_TIMEOUT_ACTION=USE_RANDOM
```

---

## 🚀 Integration Workflow

### Deployment Script Flow

```bash
#!/bin/bash
# cerberus.sh deployment workflow

# 1. Load configuration
source /etc/cerberus/cerberus.conf

# 2. Detect backend address
BACKEND_ONION=$(cat /var/lib/tor/backend/hostname)
echo "Backend service: $BACKEND_ONION"

# 3. Generate vanity prefix
if [ -z "$VANITY_PREFIX" ]; then
    # Auto-detect first N characters
    VANITY_PREFIX=$(echo "$BACKEND_ONION" | head -c "$VANITY_LENGTH")
    echo "Auto-detected prefix: $VANITY_PREFIX"
fi

# 4. User approval (if enabled)
if [ "$VANITY_REQUIRE_APPROVAL" = true ]; then
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "⚠️  VANITY ADDRESS GENERATION"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "Prefix: $VANITY_PREFIX ($VANITY_LENGTH characters)"
    echo "Estimated time: ~5 minutes (for 6 chars)"
    echo "Max timeout: $VANITY_TIMEOUT_MINUTES minutes"
    echo ""
    read -p "Proceed with generation? [y/N]: " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "⚠️  Vanity generation cancelled. Using random address."
        VANITY_ENABLED=false
    fi
fi

# 5. Run mkp224o with timeout
if [ "$VANITY_ENABLED" = true ]; then
    echo "🔨 Generating vanity address for prefix: $VANITY_PREFIX"
    
    timeout "${VANITY_TIMEOUT_MINUTES}m" \
        mkp224o "$VANITY_PREFIX" \
        -n 1 \
        -d /var/lib/tor/cerberus-vanity \
        -t "$VANITY_THREADS" \
        -q
    
    EXIT_CODE=$?
    
    # Check timeout or success
    if [ $EXIT_CODE -eq 124 ]; then
        echo "⏱️  Timeout reached after $VANITY_TIMEOUT_MINUTES minutes"
        
        if [ "$VANITY_TIMEOUT_ACTION" = "USE_RANDOM" ]; then
            echo "⚠️  Falling back to random address generation"
            VANITY_ENABLED=false
        else
            echo "❌ Deployment aborted (VANITY_TIMEOUT_ACTION=ABORT)"
            exit 1
        fi
    elif [ $EXIT_CODE -eq 0 ]; then
        echo "✅ Vanity address generated successfully!"
        
        # Move keys to Tor directory
        VANITY_DIR=$(find /var/lib/tor/cerberus-vanity -name "${VANITY_PREFIX}*" -type d | head -n 1)
        cp "$VANITY_DIR/hs_ed25519_secret_key" /var/lib/tor/cerberus/
        cp "$VANITY_DIR/hs_ed25519_public_key" /var/lib/tor/cerberus/
        cp "$VANITY_DIR/hostname" /var/lib/tor/cerberus/
        
        CERBERUS_ONION=$(cat /var/lib/tor/cerberus/hostname)
        echo "Cerberus address: $CERBERUS_ONION"
    else
        echo "❌ mkp224o failed with exit code $EXIT_CODE"
        exit 1
    fi
fi

# 6. Continue with standard deployment...
```

---

## 🔒 Security Considerations

### Key Generation Safety

**✅ Safe Practices:**
- mkp224o generates cryptographically secure Ed25519 keys
- Private keys never leave the generation system
- Use offline generation for maximum security
- Verify mkp224o source code before compilation

**⚠️ Risks:**
- **Extended Generation Times**: Long prefixes (8+ chars) may take days/weeks
- **Resource Exhaustion**: High CPU usage during generation (use `nice` priority)
- **Key Storage**: Generated keys must be protected with file permissions (0600)

### Operational Security

```bash
# Recommended: Generate offline, transfer securely
# Step 1: Generate on air-gapped system
mkp224o "market7" -n 1 -d /tmp/vanity -t 16

# Step 2: Transfer keys via encrypted USB
# Step 3: Import to Tor directory with strict permissions
chmod 600 /var/lib/tor/cerberus/hs_ed25519_*
chown debian-tor:debian-tor /var/lib/tor/cerberus/hs_ed25519_*
```

---

## ⏱️ Timeout Management

### Default Timeout: 5 Minutes

**Rationale:**
- 6-character prefix: ~5 minutes on modern CPU (16+ threads)
- Reasonable wait time for automated deployments
- Prevents deployment delays from hanging

### Extended Timeout (Manual Override)

**When to Use Longer Timeouts:**
- **7-char prefix**: Set `VANITY_TIMEOUT_MINUTES=180` (3 hours)
- **8-char prefix**: Set `VANITY_TIMEOUT_MINUTES=4320` (3 days)
- **9-char prefix**: Set `VANITY_TIMEOUT_MINUTES=20160` (14 days)
- **10-char prefix**: Not recommended (years of compute time)

### Maximum Timeout: 128 Hours (5.3 days)

**Hard Limit Rationale:**
- Prevents indefinite deployment hangs
- Forces operators to choose realistic prefixes
- 128 hours sufficient for 7-8 character prefixes on consumer hardware

**Override Example:**
```bash
# cerberus.sh --vanity-timeout 4320  # 3 days
# For 8-character prefix attempts
```

---

## 📊 Performance Estimates

### Hardware Benchmarks

| CPU | Threads | Keys/sec | 6-char Time | 7-char Time |
|-----|---------|----------|-------------|-------------|
| Intel i7-12700K | 20 | 5M | ~3 min | ~2 hours |
| AMD Ryzen 9 5950X | 32 | 8M | ~2 min | ~1.5 hours |
| Intel Xeon E5-2697v2 | 48 | 12M | ~1.5 min | ~1 hour |
| AMD EPYC 7742 | 128 | 35M | ~30 sec | ~25 min |

*Estimates based on AVX2-optimized mkp224o builds*

### Prefix Length Recommendations

**Production Deployments:**
- **4-5 chars**: Instant, low uniqueness
- **6 chars**: ⭐ **Recommended default** (5 min, good balance)
- **7 chars**: High-value services only (2-3 hours)
- **8+ chars**: Not recommended for automated deployment

---

## 🛠️ Manual Prefix Configuration

### Override Auto-Detection

```ini
# cerberus.conf - Manual prefix
[VanityOnion]
VANITY_PREFIX=cerberus  # Force specific prefix
VANITY_LENGTH=8         # Ignore auto-detection
VANITY_TIMEOUT_MINUTES=4320  # 3 days for 8-char attempt
```

### Interactive Override

```bash
# During deployment
$ sudo ./cerberus.sh deploy

Detected backend: market7xjd4abc...onion
Auto-detected prefix: market7 (6 chars)

Override prefix? [y/N]: y
Enter custom prefix (4-12 chars): cerberus
Enter timeout (5-7680 minutes) [5]: 180

Proceeding with:
  Prefix: cerberus (8 chars)
  Timeout: 180 minutes (3 hours)
  
⚠️  WARNING: 8-character prefix may take 1-3 days!
Continue? [y/N]:
```

---

## 🧩 Integration with Tor

### Directory Structure

```
/var/lib/tor/
├── cerberus/              # Cerberus frontend onion service
│   ├── hs_ed25519_secret_key  ← Generated by mkp224o
│   ├── hs_ed25519_public_key  ← Generated by mkp224o
│   └── hostname           ← Vanity address (cerberus...onion)
├── backend/               # Protected backend service
│   └── hostname           ← Original address (market7xjd4...onion)
└── cerberus-vanity/       # Temporary generation directory
    └── cerberus*/         ← mkp224o output (cleaned after import)
```

### torrc Configuration

```
# /etc/tor/torrc
HiddenServiceDir /var/lib/tor/cerberus
HiddenServicePort 80 127.0.0.1:10000  # Cerberus frontend
HiddenServiceVersion 3

# PoW defenses (propagated to vanity address)
HiddenServicePoWDefensesEnabled 1
HiddenServicePoWQueueRate 250
HiddenServicePoWQueueBurst 2500
```

---

## 🔍 Verification and Testing

### Validate Generated Keys

```bash
# Check key format
openssl pkey -in /var/lib/tor/cerberus/hs_ed25519_secret_key -text -noout

# Verify hostname matches prefix
HOSTNAME=$(cat /var/lib/tor/cerberus/hostname)
PREFIX=$(echo "$HOSTNAME" | head -c 8)
echo "Generated prefix: $PREFIX"

# Test Tor service startup
sudo systemctl restart tor@cerberus
sudo journalctl -u tor@cerberus -f
# Look for: "Established a circuit with purpose HS_SERVICE_INTRO"
```

### Fallback Testing

```bash
# Simulate timeout scenario
VANITY_TIMEOUT_MINUTES=1  # Force immediate timeout
./cerberus.sh deploy

# Should fallback to random address with warning
```

---

## 📝 Implementation Checklist

**Sprint 2: Basic Integration**
- [ ] Add mkp224o to dependencies.md (version 1.7.0+)
- [ ] Create vanity generation wrapper script (`scripts/generate-vanity.sh`)
- [ ] Add configuration options to cerberus.conf template
- [ ] Implement timeout handling in cerberus.sh
- [ ] Add user approval prompts

**Sprint 3: Advanced Features**
- [ ] Interactive prefix override during deployment
- [ ] Progress reporting (keys/sec, estimated time remaining)
- [ ] Multi-prefix search (accept any of 3 prefixes)
- [ ] Integration with admin UI (show generation status)

**Sprint 4: Production Hardening**
- [ ] Offline generation guide for air-gapped systems
- [ ] Key import validation and security checks
- [ ] Automated benchmarking (estimate time for user's hardware)
- [ ] Graceful degradation (fallback to random if mkp224o not installed)

---

## 🌐 Alternative: mkp224o-donna

For systems without AVX2 support, use **mkp224o-donna** (Curve25519-donna backend):

```bash
# Compile with donna backend (portable, slower)
./autogen.sh
./configure --enable-donna
make -j$(nproc)
```

**Performance**: ~50% slower than AVX2, but works on all x86_64 CPUs.

---

## 📖 References

- **mkp224o GitHub**: https://github.com/cathugger/mkp224o
- **Tor V3 Onion Services**: https://community.torproject.org/onion-services/
- **Vanity Address Security**: https://blog.torproject.org/vanity-onion-addresses/
- **Ed25519 Key Format**: https://github.com/torproject/torspec/blob/main/rend-spec-v3.txt

---

## ⚠️ Legal and Ethical Considerations

**Trademark Concerns:**
- Do not generate vanity addresses impersonating other services
- Respect trademark law when choosing prefixes
- Use vanity addresses for **your own services only**

**Resource Usage:**
- Long generation attempts consume significant electricity
- Consider environmental impact of 7+ character attempts
- Use energy-efficient hardware for extended searches

**Transparency:**
- Document vanity address usage in service documentation
- Inform users that frontend address is intentionally matched to backend
- Maintain address verification mechanisms (PGP signatures, blockchain records)

---

**Status**: 📝 Design Document (Implementation in Sprint 2)
