# Future Feature: Turbo Mode (Multi-Core Tor) & RustBalance

**Status:** Planned (Post-V1)
**Goal:** Saturate high-end hardware (16+ Cores, 10Gbps) by overcoming Tor's single-threaded limitations.

---

## 1. The Problem: The Single-Threaded Bottleneck
The standard `tor` daemon uses a single CPU core for all cryptographic operations (circuit handshakes, cell encryption).
- **Result:** On a 32-core server, 1 core runs at 100%, 31 cores sit idle.
- **Limit:** Throughput caps at ~50-80 Mbps per instance (depending on CPU IPC).

## 2. The Solution: Turbo Mode (Multi-Instance)
Run multiple independent Tor daemons on the same machine and aggregate them into a single "Master Onion Address" using OnionBalance.

### Architecture
```
[ User ] -> [ Tor Network ]
               |
      (Distributes traffic across descriptors)
               |
        +------+------+------+
        |      |      |      |
      [Tor1] [Tor2] [Tor3] [Tor4]  <-- 4 Instances (Frontend Onions)
        |      |      |      |
        +------+------+------+
               |
          [ HAProxy ]  <-- Balances traffic to Nginx
```

### Performance Scaling
| Configuration | CPU Usage | Max Throughput | Concurrent Users |
|---------------|-----------|----------------|------------------|
| **Standard** (1 Instance) | 1 Core | ~60 Mbps | ~2,000 |
| **Turbo** (4 Instances) | 4 Cores | ~240 Mbps | ~8,000 |
| **Turbo Max** (16 Instances) | 16 Cores | ~1 Gbps | ~32,000 |

> **Note:** "Throughput" refers to crypto throughput. Actual HTTP throughput might be lower due to Tor network latency.

---

## 3. Implementation Plan

### Phase A: Integration with OnionBalance (Python/Go)
Use the existing tools to prove the concept.
1.  **Detection:** Install script detects > 8 Cores.
2.  **Config Generation:**
    - Generate `torrc.1` through `torrc.N`.
    - Configure OnionBalance with the Master Key.
3.  **Process Management:** `systemd` template `tor@.service` to manage instances.

### Phase B: "RustBalance" (Porting to Rust)
**Goal:** Rewrite GoBalance/OnionBalance in Rust to integrate directly into `Fortify`.

**Why Port to Rust?**
1.  **Memory Safety:** Critical for handling private keys.
2.  **Zero GC:** Go's Garbage Collector can cause latency spikes; Rust is deterministic.
3.  **Single Binary:** Instead of running `python3 onionbalance` or `gobalance` alongside `fortify`, we compile the balancing logic **into Fortify**.
    - Fortify manages the Master Key.
    - Fortify generates the aggregated descriptor.
    - Fortify publishes to Tor HSDirs.

**Feasibility Analysis:**
- **Complexity:** High. Requires implementing Tor Cell/Descriptor formats (ED25519 signing).
- **Libraries:** `arti` (The official Rust Tor implementation) creates a perfect foundation. We can use `arti-client` or `tor-proto` crates.
- **Strategic Value:** Massive. Makes Cerberus a stand-alone, zero-dependency "Super Tor Node".

### Phase C: The "Hydra" & "Roulette" Protocols
**Advanced Defense Strategies enabled by RustBalance.**

#### 1. The Immortal Cluster (High Availability)
- **Zero SPOF:** The Master Key lives in the encrypted RAM Vault of *all* nodes.
- **Leader Rotation:** Nodes randomly elect a new "Publisher" every 10 minutes.
- **Benefit:** If the current Publisher dies, another takes over instantly. The service never goes offline.

#### 2. The Hydra Maneuver (Anti-Intro Flood)
**Scenario:** Attacker floods Node 1 (Intro Point).
**Response:**
1.  Node 1 reports "Under Attack".
2.  RustBalance immediately generates a **New Descriptor**.
3.  **Removes** Node 1. **Adds** Node 4 (Ghost/Standby Node).
4.  **Result:** Attacker is stuck bombing a disconnected node. New clients connect to fresh nodes.

#### 3. The Roulette Protocol (Moving Target)
**Logic:** Every 10 minutes, the cluster completely reshuffles the deck.
-   **Pool:** 10 Nodes.
-   **Descriptor T=0:** Points to Nodes [1, 2, 3].
-   **Descriptor T=10:** Points to Nodes [4, 5, 6].
-   **Descriptor T=20:** Points to Nodes [7, 8, 9].
**Benefit:**
-   **Whack-a-Mole:** Attackers constantly target stale nodes.
-   **Traffic Washing:** Forces attacker to rediscover targets constantly.
-   **Anonymity:** Publication source moves, making correlation attacks impossible.

---

## 4. User Experience
**Dashboard Toggle:**
`[x] Enable Turbo Mode (Experimental)`
- Slider: `Number of Instances: [ 4 ]`
- Status: `Aggregating 4/4 Instances. Master Onion: xyz...onion`

---
