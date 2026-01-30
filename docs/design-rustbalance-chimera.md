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

## 3. Protocol Flow: "The Roulette" (10-Minute Cycle)

### Phase 1: The Election (T-minus 1 Minute)
Nodes must agree on *who* will publish the next descriptor. We avoid heavy consensus (Paxos) in favor of **Deterministic Hashing** or **Redis Locking**.

**Proposed Algorithm: The Time-Hash**
1.  Calculate `Seed = Hash(MasterOnion + CurrentTimeSlot)`.
2.  Rank all healthy nodes by `Distance(NodeID, Seed)`.
3.  The node with the closest ID is the **Next Leader**.
*   *Benefit:* Every node knows who the leader *should* be without chatting. If Node A is dead, everyone knows Node B is next in line.

### Phase 2: The Shuffle (Descriptor Generation)
The Leader constructs the new descriptor.
1.  **Inventory:** Query Redis/Gossip for list of "Healthy" nodes.
2.  **Selection:** Pick `N` nodes to be Intro Points (e.g., 3 out of 10).
    *   *Strategy:* Prioritize nodes with low load. Randomize to confuse attackers.
3.  **Signing:** Use the Crown Jewels (Master Key) to sign the descriptor listing these `N` Intro Points.

### Phase 3: The Reveal (Publication)
The Leader uploads the signed descriptor to the Tor HSDirs.
*   **Propagation:** Takes ~30-60 seconds.
*   **Overlap:** The *old* descriptor (published by Previous Leader) is still valid for ~60 mins, but clients prefer the fresher revision.

### Phase 4: The Ghosting (Stale Node Defense)
*   **Scenario:** Node 1 was an Intro Point in the *Previous* descriptor. It is NOT in the *New* descriptor.
*   **State:** Node 1 enters **"Ghost Mode"**.
    *   It accepts *existing* connections (long-lived sessions).
    *   It expects *new* connection attempts to drop to near zero (as clients switch to the new descriptor).
    *   **Defense:** Any *surge* of new traffic to Node 1 in Ghost Mode is **confirmed attack traffic** (attacking a stale target). Node 1 can aggressively drop/blackhole this traffic without collateral damage.

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
