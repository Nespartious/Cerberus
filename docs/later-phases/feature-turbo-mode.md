# Future Features: High-End Scaling & Defense

**Status:** Planned (Post-V1)
**Scope:** Advanced capabilities for high-performance servers and extreme threat models.

---

## Feature 1: "Turbo Mode" (Multi-Core Tor)
**Goal:** Saturate high-end hardware (16+ Cores, 10Gbps) by overcoming Tor's single-threaded limitations.

### 1. The Problem
The standard `tor` daemon uses a single CPU core for all cryptographic operations (circuit handshakes, cell encryption).
- **Result:** On a 32-core server, 1 core runs at 100%, 31 cores sit idle.
- **Limit:** Throughput caps at ~50-80 Mbps per instance (depending on CPU IPC).

### 2. The Solution: Multi-Instance Architecture
Run multiple independent Tor daemons on the same machine and aggregate them into a single "Master Onion Address".

**Architecture:**
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

### 3. Performance Scaling
| Configuration | CPU Usage | Max Throughput | Concurrent Users |
|---------------|-----------|----------------|------------------|
| **Standard** (1 Instance) | 1 Core | ~60 Mbps | ~2,000 |
| **Turbo** (4 Instances) | 4 Cores | ~240 Mbps | ~8,000 |
| **Turbo Max** (16 Instances) | 16 Cores | ~1 Gbps | ~32,000 |

---

## Feature 2: "Protocol Chimera" (Descriptor Rotation)
**Goal:** A Moving Target Defense that rotates the attack surface while maintaining persistent user sessions.

### 1. The Concept
Cerberus treats Tor Intro Points as **Disposable Munitions**. Instead of a static list of entry nodes, the cluster constantly reshuffles its public face using a custom implementation of OnionBalance ("RustBalance").

### 2. The "Roulette" Strategy (Periodic Rotation)
**Mechanism:** Every 10 minutes, the cluster elects a new Leader and generates a new Descriptor.
-   **T=0:** Descriptor points to Nodes [A, B, C].
-   **T=10:** Descriptor points to Nodes [D, E, F].
-   **Effect:** Attackers targeting [A, B, C] are bombing "Ghost Nodes" that no longer accept new connections. Legitimate new users connect to clean nodes [D, E, F].

### 3. The "Hydra" Maneuver (Reactive Defense)
**Mechanism:** Triggered by Intro Flood Detection.
1.  **Attack:** Node A reports 100% CPU due to Intro Flood.
2.  **Cut Head:** Cluster immediately removes Node A from the active descriptor.
3.  **Grow Head:** Cluster adds Standby Node X to the descriptor.
4.  **Publish:** New descriptor propagates (~1-2 mins).
5.  **Result:** The flood is blackholed at Node A (which can now drop packets aggressively or disconnect), while the service remains reachable via Node X.

### 4. Technical Feasibility Check
*   **Tor Protocol:** Supports dynamic descriptor updates. Existing circuits persist even if the Intro Point is removed.
*   **Session Persistence:** Handled by Redis Cluster. A user moving from Node A to Node D keeps their session.
*   **Availability:** HSDir propagation takes ~60-120 seconds. Overlapping descriptors ensure zero downtime.

---

## Feature 3: RustBalance (The Enabler)
**Goal:** Port OnionBalance to Rust and integrate it directly into `Fortify`.

**Why:**
1.  **Integrated Logic:** Fortify needs to control the descriptor to execute "Chimera" logic. External Python scripts are too slow/dumb.
2.  **Security:** Memory-safe handling of the Master Private Key in the Volatile Vault.
3.  **Zero Dependencies:** No Python runtime required.

**Roadmap:**
- Use `arti` (Rust Tor) libraries for crypto.
- Implement ED25519 descriptor signing.
- Implement HSDir upload logic.


---
