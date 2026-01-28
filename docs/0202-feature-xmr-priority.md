# XMR Payment-Based Priority System

**Economic Proof-of-Work: Monero Payments for Queue Fast-Pass**

---

## 📋 Concept Overview

Allow users to **pay a small Monero (XMR) fee** (1-5 cents USD) to bypass the virtual queue and gain immediate VIP access. Payment proof is embedded in the URL path, verified by Cerberus against the Monero blockchain.

### User Experience Flow

```
1. User visits: cerberus.onion
2. Sees queue wait time: "Estimated wait: 15 minutes"
3. Option displayed: "Skip queue for 0.0002 XMR (~$0.03)"
4. User sends XMR to provided address
5. User appends transaction ID to URL: cerberus.onion/xmr/<tx_id>
6. Cerberus verifies transaction on blockchain
   - 0 confirmations: Grant gentle access (higher rate limits)
   - 1+ confirmations: Grant full VIP access (bypass all limits)
7. VIP status lasts 24 hours (or configurable duration)
```

---

## 🎯 Problem Being Solved

### Current Virtual Queue Limitations

The existing virtual queue system (see [virtual-queue-system.md](virtual-queue-system.md)) has three priority tiers:
1. **VIP**: Requires admin manual promotion (not accessible to users)
2. **PoW**: Proof-of-Work challenge (CPU-intensive, slow on mobile)
3. **Normal**: Age-based priority (fair, but still requires waiting)

**Gap**: Legitimate users with urgent need have no way to skip the queue besides:
- Waiting (frustrating for paying customers)
- Solving PoW (battery drain on mobile, may take minutes)
- Requesting admin promotion (doesn't scale)

### Why Economic Proof-of-Work?

**Sybil Resistance:**
- Attacker needs 10,000 transactions @ $0.03 each = **$300 to flood with VIPs**
- Compare to PoW: 10,000 browsers solving puzzles = free
- Economic cost is **real** and **unrecoverable** (unlike CPU cycles)

**Darknet Alignment:**
- Markets/services already accept XMR
- Users have wallets configured
- Paying for priority is culturally accepted (escrow fees, vendor bonds, etc.)

**Funding Model:**
- Service operators earn revenue to cover hosting costs
- Incentivizes maintaining high availability (happy users = more payments)
- Self-sustaining (unlike donation-based funding)

---

## ✅ Feasibility Analysis

### Technical Viability: ⭐⭐⭐⭐ (High)

**Required Components:**

1. **Monero Daemon (`monerod`)**
   - Full node or remote node connection
   - Syncs blockchain to verify transactions
   - ~100GB disk space for full node (pruned: ~40GB)
   - CPU: Low (idle), Moderate (syncing)

2. **Monero Wallet RPC (`monero-wallet-rpc`)**
   - Monitors incoming transactions to configured address
   - Provides API for transaction verification
   - JSON-RPC interface (easy to integrate with Rust)

3. **Rust Integration (`monero-rs` crate)**
   - Parse transaction IDs
   - Query wallet RPC for transaction status
   - Verify amount, confirmations, destination address

### Complexity Assessment

**Low Complexity:**
- ✅ Monero RPC is well-documented
- ✅ Existing Rust crates (`monero`, `monero-rpc`)
- ✅ No smart contracts (just blockchain queries)
- ✅ No payment processing (users send directly to address)

**Medium Complexity:**
- ⚠️ Address rotation (avoid correlation attacks)
- ⚠️ Confirmation threshold tuning (0 vs 1+ confirmations)
- ⚠️ Rate limiting (prevent transaction replay)

**High Complexity:**
- 🔴 Double-spend detection (mitigated by waiting for confirmations)
- 🔴 Wallet security (hot wallet holds funds)

### Security Evaluation: ⭐⭐⭐⭐ (High, with caveats)

**Pros:**
- ✅ Monero privacy protections (RingCT, Stealth Addresses)
- ✅ No PII collected (no email, no KYC)
- ✅ Transaction ID != user identity (unlinkable)
- ✅ Payments are push (service doesn't need user's wallet)

**Risks:**
- ⚠️ **Address Reuse**: Using single address for all payments = correlation vector
  - **Mitigation**: Rotate addresses frequently (every 1000 transactions or daily)
- ⚠️ **Hot Wallet Risk**: Funds stored in online wallet = theft target
  - **Mitigation**: Auto-sweep to cold wallet every 100 transactions
- ⚠️ **Double-Spend (0-conf)**: Attacker broadcasts conflicting transaction
  - **Mitigation**: Only grant "gentle" access for 0-conf, full VIP after 1+ confirmations
- ⚠️ **Regulatory Risk**: Accepting payments may trigger money transmission laws
  - **Mitigation**: Document as "donations for priority support" (not payment for service access)

### Darknet Market Fit: ⭐⭐⭐⭐⭐ (Perfect)

**Why This Works for Darknet:**
- ✅ XMR is **the** standard currency (superior to BTC for privacy)
- ✅ Users already have wallets and understand XMR
- ✅ Cultural acceptance of paying for better service (vendor bonds, escrow, etc.)
- ✅ Aligns with cypherpunk ethos (pay with untraceable currency, not credit card)

**Real-World Precedent:**
- AlphaBay (2014-2017): Vendor bonds required 0.05 BTC (~$50)
- Dread Forum: Donator badges for site support
- Hidden Wiki mirrors: Donations for premium listings

---

## 🏗️ Architecture Design

### Component Overview

```
┌─────────────────────────────────────────────────────────────────┐
│  User (Tor Browser)                                             │
└─────────────────────────────────────────────────────────────────┘
                            ↓
         http://cerberus.onion/xmr/a3f8b2c1d4e5f6...
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│  Nginx (Layer 2)                                                │
│  ├─ Proxy to Fortify: /xmr/<tx_id>                             │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│  Fortify (Layer 3 - Rust Application)                           │
│  ├─ Parse transaction ID from URL path                          │
│  ├─ Query XMR Verification Service                              │
│  ├─ Verify: amount >= min, destination = our address            │
│  ├─ Check confirmations (0, 1, 2+)                              │
│  ├─ Promote circuit to VIP (HAProxy stick table update)         │
│  └─ Store payment in SQLite (prevent replay)                    │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│  XMR Verification Service (Separate Process/Module)             │
│  ├─ Connects to monero-wallet-rpc (JSON-RPC)                   │
│  ├─ Monitors incoming transactions                              │
│  ├─ Caches transaction status (avoid repeated blockchain queries)│
│  └─ Exposes Rust API to Fortify                                 │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│  Monero Wallet RPC (monero-wallet-rpc)                          │
│  ├─ Listens on 127.0.0.1:18082                                 │
│  ├─ Manages wallet (receive addresses, balances)                │
│  └─ Queries monerod for blockchain data                         │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│  Monero Daemon (monerod)                                        │
│  ├─ Full blockchain sync (or remote node connection)            │
│  ├─ Validates transactions                                      │
│  └─ Provides confirmation counts                                │
└─────────────────────────────────────────────────────────────────┘
```

---

## ⚙️ Configuration

### cerberus.conf Settings

```ini
[XMRPriority]
# Enable XMR payment-based priority system
XMR_ENABLED=true

# Minimum payment amount (in XMR)
# Current equivalent: ~$0.03 USD = 0.0002 XMR (as of Jan 2026)
XMR_MIN_AMOUNT=0.0002

# VIP duration after successful payment (in seconds)
# Default: 86400 (24 hours)
XMR_VIP_DURATION=86400

# Confirmation tiers
# 0 confirmations: Grant "gentle" access (2x rate limits)
# 1+ confirmations: Grant full VIP (bypass all limits)
XMR_0_CONF_MULTIPLIER=2.0
XMR_1_CONF_VIP=true

# Wallet RPC connection
XMR_WALLET_RPC_HOST=127.0.0.1
XMR_WALLET_RPC_PORT=18082
XMR_WALLET_RPC_USER=cerberus
XMR_WALLET_RPC_PASSWORD=<generated-secure-password>

# Address rotation (generate new address every N transactions)
XMR_ADDRESS_ROTATION_INTERVAL=1000

# Auto-sweep to cold storage (every N XMR accumulated)
XMR_AUTO_SWEEP_THRESHOLD=1.0
XMR_COLD_WALLET_ADDRESS=<offline-wallet-address>

# Transaction replay prevention (block reuse of same tx_id)
XMR_TX_CACHE_DURATION=2592000  # 30 days
```

---

## 💡 User Interface Design

### Queue Landing Page (Without Payment)

```html
┌──────────────────────────────────────────────────────────────────┐
│  🛡️ Cerberus Protection Active                                   │
├──────────────────────────────────────────────────────────────────┤
│  Your position in queue: #234                                    │
│  Estimated wait time: ~12 minutes                                │
│                                                                  │
│  [Automatic refresh in 10 seconds...]                           │
│                                                                  │
│  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━  │
│                                                                  │
│  ⚡ SKIP THE QUEUE                                               │
│                                                                  │
│  Pay 0.0002 XMR (~$0.03 USD) for instant VIP access             │
│                                                                  │
│  1. Send XMR to:                                                 │
│     [89a3f8b2c1d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6] │
│     [Copy Address]                                               │
│                                                                  │
│  2. After sending, paste your transaction ID below:              │
│     ┌────────────────────────────────────────────────────────┐  │
│     │ Transaction ID (64 hex characters)                     │  │
│     └────────────────────────────────────────────────────────┘  │
│     [Verify Payment]                                             │
│                                                                  │
│  ⚠️ Your VIP access will last 24 hours after payment confirms    │
└──────────────────────────────────────────────────────────────────┘
```

### Alternative: Direct URL Access

```
User clicks "Skip Queue" → Redirects to payment page with pre-generated address

After payment:
  User visits: http://cerberus.onion/xmr/a3f8b2c1d4e5f6a7b8c9d0e1f2a3b4c5...
  
  Page shows:
  ✅ Payment verified! (0 confirmations)
  🟡 Limited access granted. Full VIP after 1 confirmation (~2 minutes)
  
  [Continue to Site] ← Bypasses queue
```

---

## 🔐 Security Considerations

### Address Correlation Prevention

**Problem**: Single address for all payments = all VIP users linkable

**Solution: Subaddress Rotation**
```rust
// Generate new subaddress every 1000 transactions
let subaddress_index = payment_count / 1000;
let receive_address = wallet.get_address(account_index, subaddress_index)?;

// Display to user
format!("Send XMR to: {}", receive_address);
```

**Benefits:**
- All payments go to different addresses
- Still single wallet (no key management complexity)
- Unlinkable (Monero stealth addresses + subaddress rotation)

### Transaction Replay Prevention

**Attack**: User pays once, reuses same `tx_id` multiple times

**Mitigation**:
```rust
// Store verified transactions in SQLite
CREATE TABLE xmr_payments (
    tx_id TEXT PRIMARY KEY,
    circuit_id TEXT NOT NULL,
    amount REAL NOT NULL,
    verified_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL
);

// Before granting VIP, check if tx_id already used
if db.payment_exists(tx_id) {
    return Err("Transaction already used");
}

// After verification, store with 24-hour expiry
db.insert_payment(tx_id, circuit_id, amount, now, now + 86400);
```

### Double-Spend Protection (0-conf Risk)

**Attack**: User broadcasts transaction, gets 0-conf access, double-spends with higher fee

**Mitigation**:
1. **0-conf = Gentle Access Only**: 2x rate limits, not full VIP
2. **1+ confirmations = Full VIP**: Irreversible on Monero blockchain
3. **Monitoring**: If 0-conf transaction disappears (not mined), revoke access

```rust
// Check transaction status every 5 minutes
if confirmations == 0 && age > 30_minutes {
    // Transaction still unconfirmed after 30 min = suspicious
    revoke_gentle_access(circuit_id);
    log_warn!("Possible double-spend attempt: {}", tx_id);
}
```

### Hot Wallet Security

**Risk**: Funds stored in online wallet = theft target

**Mitigation 1: Auto-Sweep**
```bash
# Cron job: Every hour, sweep funds to cold wallet
0 * * * * monero-wallet-cli --wallet-file /var/lib/cerberus/hot_wallet \
    --command "sweep_all <COLD_WALLET_ADDRESS>"
```

**Mitigation 2: Minimal Balance**
```rust
// If balance > 1.0 XMR, auto-sweep
if wallet.get_balance()? > 1.0 {
    wallet.sweep_all(COLD_WALLET_ADDRESS)?;
    log_info!("Auto-swept {} XMR to cold storage", balance);
}
```

**Mitigation 3: Encrypted Wallet**
```bash
# Wallet file encrypted at rest
monero-wallet-rpc --wallet-file /var/lib/cerberus/hot_wallet \
    --password-file /etc/cerberus/wallet_password.txt \
    --rpc-bind-ip 127.0.0.1 --rpc-bind-port 18082
```

### Privacy Best Practices

**For Operators:**
- ✅ Run own Monero node (don't use public nodes = traffic correlation)
- ✅ Tor-ify monerod connections (if using remote node)
- ✅ Rotate subaddresses frequently
- ✅ Never reuse addresses

**For Users:**
- ✅ Use Monero GUI/CLI with own node (not web wallets)
- ✅ Send from Tor-enabled wallet
- ✅ Use subaddresses for sending (not primary address)

---

## 📊 Economic Analysis

### Pricing Strategy

**Current Calculation:**
```
XMR Price (Jan 2026): ~$150 USD
Target Fee: $0.02 - $0.05 USD
Minimum XMR: 0.0002 XMR (~$0.03 USD)
```

**Dynamic Pricing (Optional):**
```rust
// Adjust price based on queue depth
let base_price = 0.0002;  // XMR
let queue_multiplier = if queue_depth > 1000 { 2.0 } else { 1.0 };
let dynamic_price = base_price * queue_multiplier;

// Display to user
format!("Skip queue for {} XMR (~${} USD)", dynamic_price, xmr_to_usd(dynamic_price));
```

**Attack Economics:**
- 10,000 VIP slots @ $0.03 = **$300 total cost**
- Compare: Legitimate DDoS-for-hire: $50-500/hour
- **Conclusion**: Raises attack cost significantly vs free PoW

### Revenue Potential

**Conservative Estimate (Small Service):**
- 100 queue-skip payments/day
- $0.03 per payment
- **$3/day = $90/month**
- Covers: Basic VPS ($5/mo) + profit

**Moderate Estimate (Medium Service):**
- 1,000 payments/day
- $0.03 per payment
- **$30/day = $900/month**
- Covers: Dedicated server ($200/mo) + profit

**High Volume (Large Marketplace):**
- 10,000 payments/day
- $0.03 per payment
- **$300/day = $9,000/month**
- Covers: Infrastructure + security audits + bug bounty

---

## 🛠️ Implementation Plan

### Sprint 3: Basic XMR Integration

- [ ] Research `monero-rs` crate vs `monero-rpc-rs`
- [ ] Set up monero-wallet-rpc in development environment
- [ ] Implement transaction verification API (Fortify module)
- [ ] Create `/xmr/<tx_id>` endpoint in Fortify
- [ ] Add payment verification logic (amount, destination, confirmations)
- [ ] Integrate with HAProxy stick table promotion
- [ ] Store verified transactions in SQLite (prevent replay)

### Sprint 4: Production Hardening

- [ ] Implement subaddress rotation
- [ ] Auto-sweep to cold wallet
- [ ] Double-spend monitoring (0-conf transactions)
- [ ] Add XMR payment UI to queue landing page
- [ ] Create admin dashboard widget (XMR revenue tracking)
- [ ] Security audit (wallet encryption, RPC authentication)
- [ ] Load testing (1000 concurrent payment verifications)

### Sprint 5: Advanced Features (Optional)

- [ ] Dynamic pricing based on queue depth
- [ ] Multi-tier pricing ($0.03 = 24h, $0.10 = 7 days, $0.50 = 30 days)
- [ ] Refund mechanism (service downtime = auto-refund)
- [ ] Integration with monitoring UI (real-time payment notifications)
- [ ] Anonymous voucher system (pay once, share code with friends)

---

## ⚠️ Risks and Mitigations

### Risk 1: Regulatory Compliance

**Risk**: Accepting payments may classify service as "money transmitter"

**Mitigation**:
- Frame as "donations for priority support" (not payment for access)
- No KYC collection (anonymous donations)
- Document in ToS: "Donations do not guarantee service availability"
- Consult legal counsel if revenue exceeds $10k/year

### Risk 2: XMR Price Volatility

**Risk**: XMR price drops 50% → fees become too expensive in USD terms

**Mitigation**:
- Auto-adjust min amount based on USD equivalent
- Query XMR/USD price from Kraken API (over Tor)
- Update `XMR_MIN_AMOUNT` in config weekly

```rust
// Pseudo-code
let xmr_usd_price = fetch_price_from_kraken()?;  // $150
let target_usd = 0.03;
let xmr_min_amount = target_usd / xmr_usd_price;  // 0.0002 XMR
```

### Risk 3: User Confusion (UX Complexity)

**Risk**: Users don't understand how to send XMR or find transaction ID

**Mitigation**:
- Provide detailed instructions with screenshots
- Link to guides: "How to send XMR with Monero GUI"
- Fallback option: Submit payment address (slower verification)

### Risk 4: Blockchain Congestion

**Risk**: Monero network congested → slow confirmations → angry users

**Mitigation**:
- Set reasonable expectations: "Confirmations take 2-20 minutes"
- Grant immediate access with 0-conf (limited)
- Offer PoW as alternative (instant, no payment)

---

## 🔍 Comparison to Alternatives

### XMR Payment vs PoW Challenge

| Aspect | XMR Payment | PoW Challenge |
|--------|-------------|---------------|
| **Attack Cost** | $300 for 10k VIPs | Free (just CPU) |
| **User Friction** | Moderate (send XMR) | Low (click solve) |
| **Mobile Friendly** | ✅ Yes | ❌ Battery drain |
| **Revenue** | ✅ $90-9k/month | ❌ None |
| **Implementation** | Complex (wallet integration) | Simple (built-in) |

**Verdict**: Both should coexist (XMR = premium fast-pass, PoW = free alternative)

### XMR Payment vs CAPTCHA

| Aspect | XMR Payment | CAPTCHA |
|--------|-------------|---------|
| **Bot Resistance** | ✅ Perfect (economic cost) | ⚠️ Moderate (OCR bypass) |
| **User Friction** | Moderate (send XMR) | Low (type 6 chars) |
| **Accessibility** | ✅ Works for blind users | ❌ Vision required |
| **Revenue** | ✅ $90-9k/month | ❌ None |

**Verdict**: XMR payment is superior for VIP tier, CAPTCHA for Normal tier

---

## 📖 References

- **Monero Documentation**: https://www.getmonero.org/resources/developer-guides/
- **monero-wallet-rpc**: https://www.getmonero.org/resources/developer-guides/wallet-rpc.html
- **monero-rs Crate**: https://crates.io/crates/monero
- **Darknet Market Economics**: https://gwern.net/dnm-arrests (academic research)
- **Subaddress BIP**: https://github.com/monero-project/monero/pull/2056

---

## ✅ Recommendation: PROCEED

**Verdict**: ⭐⭐⭐⭐⭐ Highly Recommended

**Why:**
- ✅ Perfect fit for darknet/Tor economy
- ✅ Strong Sybil resistance (real economic cost)
- ✅ Revenue generation for operators
- ✅ Technically feasible (mature Monero libraries)
- ✅ Aligns with cypherpunk ethos (privacy-preserving payments)

**When to Implement:**
- **Sprint 3-4** (after core features stable)
- Not Sprint 2 (focus on fundamentals first)

**Priority Level:**
- High for darknet market deployments
- Medium for general Tor services
- Low for personal/hobby projects

---

**Status**: 📝 Design Document (Implementation in Sprint 3-4)
