# Design RFC: RustBalance & The Protocol Chimera
**A Distributed, Moving-Target Defense Architecture for Tor Onion Services.**

**Status:** Draft / RFC
**Author:** Cerberus Architecture Team
**Goal:** Define a zero-SPOF, self-repairing, rotating-target architecture where the attack surface shifts faster than an attacker can saturate it.

---

## 1. Abstract
Traditional OnionBalance architectures have a single "Master" node that publishes descriptors. If it dies, the service vanishes. "Protocol Chimera" replaces this with a **Leaderless Mesh** where every node is capable of being the Publisher. By rotating the Publisher and the Intro Points every 10 minutes ("The Roulette"), we force attackers to aim at stale targets while legitimate users seamlessly connect to fresh ones.

## 2. The Architecture: "The Hydra"

### 2.1 Component Roles
*   **The Crown Jewels (Master Identity):** The `hs_ed25519_secret_key` that defines the service's `.onion` address. This key **never** handles traffic. It only signs descriptors.
*   **The Heads (Frontend Nodes):** Each node runs a local Tor instance with a *disposable* onion key (`frontend_onion_1`). These handle the actual traffic.
*   **The Brain (RustBalance):** A module within Fortify running on *every* node. It manages the Crown Jewels and orchestrates the rotation.

### 2.2 The Encrypted Vault Sync
*   **Requirement:** All nodes must possess the Crown Jewels to become Leader.
*   **Mechanism:**
    *   Nodes join the WireGuard mesh.
    *   On join, the existing Leader syncs the encrypted `master_key.blob` to the new node's RAM Vault (`/mnt/cerberus_vault/`).
    *   **Security:** The key never touches disk unencrypted. If a node is seized/powered off, the key is gone.

---

## 3. Protocol Flow: Stable Leadership & Reactive Rotation

### Phase 1: The Election (Stable Default)
Nodes must agree on *who* will publish the descriptor.
*   **Default:** The node with the longest uptime (or highest ID) is Leader.
*   **Failover:** If Leader dies, the next node takes over immediately.
*   **Goal:** Maintain a stable descriptor to maximize user reachability.

### Phase 2: Reactive Rotation ("The Hydra")
Triggered only when an active Intro Point reports it is under attack (Intro Flood).
1.  **Report:** Attacked Node signals "Under Siege".
2.  **Shuffle:** Leader generates a new descriptor excluding the attacked node.
3.  **Publish:** New descriptor propagates. Attacker traffic is isolated to the "Ghost Node".

### Phase 3: Panic Mode ("The Roulette")
**Manual Activation Only.**
*   **Mechanism:** Force-rotate the descriptor every 10 minutes.
*   **Warning:** Causes connection latency for users. Use only when the cluster is being overwhelmed by a massive, dynamic botnet.

### Phase 4: The Ghosting (Stale Node Defense)
*   **Scenario:** Node 1 was an Intro Point in the *Previous* descriptor. It is NOT in the *New* descriptor.
*   **State:** Node 1 enters **"Ghost Mode"**.
    *   It accepts **Existing User Sessions** (Rendezvous circuits persist).
    *   It expects **New Intro Requests** to drop to zero.
    *   **Defense:** Any *new* Intro Request to a Ghost Node is **confirmed attack traffic**. Node 1 can blacklist the circuit immediately.

---

## 4. Failure Modes & Self-Repair

### Scenario A: Leader Death
*   **Event:** Leader (Node A) crashes mid-cycle.
*   **Detection:** Node B (Next in line) sees missed "I Published" gossip message.
*   **Recovery:** Node B assumes Leadership immediately and publishes.
*   **Impact:** Zero. Tor descriptors have overlap.

### Scenario B: Intro Point Death (The Hydra)
*   **Event:** Node 1 (Active Intro Point) is nuked by DDoS.
*   **Response:**
    1.  Node 1 stops gossiping health.
    2.  Leader detects Node 1 is dead.
    3.  Leader immediately generates **New Descriptor** replacing Node 1 with Node 4.
    4.  **Impact:** Attacker burns resources on dead Node 1. Users shift to Node 4.

---

## 5. Security Analysis

### Risk: Key Theft
*   **Vector:** Attacker compromises one node (Node X).
*   **Impact:** They dump `/mnt/cerberus_vault/` and steal the Master Key.
*   **Mitigation:**
    *   The "Crown Jewels" are only needed for *Signing* (milliseconds).
    *   **Advanced Idea:** Use **Threshold Signatures (TSS)**. Split the key into shards. Signing requires 3-of-5 nodes to agree. No single node holds the full key. (Future Phase).

### Risk: Partition / Split Brain
*   **Vector:** Network splits. Group A (Nodes 1,2) and Group B (Nodes 3,4) both elect a leader.
*   **Impact:** Two valid descriptors exist.
*   **Tor Behavior:** Tor handles this. Clients will pick one. Both work. Traffic is split. Service stays up.
*   **Recovery:** When mesh heals, nodes resync.

---

## 6. Implementation Roadmap (RustBalance)

1.  **Phase 1:** **Local Balance.** Port `onionbalance` logic to Rust. Run as a standalone service.
2.  **Phase 2:** **Cluster Balance.** Implement "Time-Hash" election and Redis-based state sharing.
3.  **Phase 3:** **The Chimera.** Implement "Ghost Mode" logic and dynamic rotation.
4.  **Phase 4:** **Threshold Signatures.** Remove the Master Key entirely; replace with distributed signing.

---
