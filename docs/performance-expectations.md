# Performance Expectations & Capacity Planning
*Estimated resilience of Cerberus across various hardware tiers.*

> **Note:** These estimates assume a **Linux Kernel 5.10+** environment with **Native XDP** support. If utilizing Generic XDP (SKB mode), reduce Packet Flood resilience by ~40% and increase CPU usage significantly.

---

## 1. The Scaling Model
Cerberus scales differently at each layer depending on the resource type.

| Layer | Primary Constraint | Scaling Factor |
|-------|--------------------|----------------|
| **L0 (XDP)** | **CPU Cores** & NIC | Can drop ~1.5M PPS per dedicated core (Native Mode). |
| **L4 (HAProxy)** | **RAM** & CPU | 100k Stick Table entries ≈ 50MB RAM. Connection handling is cheap. |
| **L7 (Nginx)** | **CPU** | SSL termination (if any) and Request Parsing. |
| **L7+ (Fortify)** | **CPU** (Gen) / **RAM** (Storage) | Generating CAPTCHAs is heavy. Serving from Ammo Box is cheap. |
| **Tor Daemon** | **Single Core Speed** | Tor is single-threaded. 1 Instance ≈ 50-80 Mbps throughput. |

**The Bottleneck:** For legitimate traffic, **Tor is the bottleneck**. For Attack traffic, **XDP/CPU** is the limit.

---

## 2. Capacity Estimates by Hardware Tier

**Definitions:**
*   **Attack PPS:** Max Packets Per Second (Volumetric Flood) the system can drop without crashing.
*   **Legit RPS:** Max HTTP Requests Per Second from verified humans (Tor throughput limited).
*   **Max Concurrent:** Max simultaneous TCP connections (Stick Table capacity).

| Hardware Tier (Cores / RAM) | Attack Resilience (PPS) | Legit Traffic (RPS) | Max Concurrent Conns | Primary Bottleneck | Recommended Role |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **1 Core / 4 GB** | ~800k PPS | ~200 RPS | 20,000 | **CPU** (Tor + System fight for 1 core) | Dev / Testing Only |
| **2 Core / 4 GB** | ~1.5M PPS | ~500 RPS | 50,000 | **RAM** (OS + Tor + Redis tight) | Minimum Viable Prod |
| **2 Core / 8 GB** | ~1.5M PPS | ~500 RPS | 100,000 | **CPU** (Tor limits throughput) | Small Cluster Node |
| **4 Core / 4 GB** | ~3.0M PPS | ~1,000 RPS | 50,000 | **RAM** (Cannot hold large Ammo Box) | High-Packet Scrubber |
| **4 Core / 8 GB** | ~3.0M PPS | ~1,200 RPS | 200,000 | **Balanced** | **Standard Production** |
| **4 Core / 16 GB** | ~3.0M PPS | ~1,200 RPS | 500,000 | **Tor** (Need multi-instance Tor) | High-Capacity Node |
| **8 Core / 8 GB** | ~6.0M PPS | ~2,500 RPS | 200,000 | **RAM** (High CPU but low state) | Heavy Compute (Gen) |
| **8 Core / 16 GB** | ~6.0M PPS | ~3,000 RPS | 1,000,000 | **Tor** (Run 4+ Tor Instances) | Major Entry Point |
| **8 Core / 32 GB** | ~6.0M PPS | ~3,000 RPS | 2,000,000 | **Network/NIC** | Cluster Hub |
| **16 Core / 32 GB+** | ~10M+ PPS | ~6,000 RPS | 5,000,000+ | **Upstream Bandwidth** | Enterprise Frontend |
| **32 Core / 64 GB+** | Line Rate (10G) | ~10,000 RPS | 10M+ | **Upstream Bandwidth** | Nation-State Defense |
| **64 Core / 128 GB+**| Line Rate (40G) | ~20,000 RPS | 20M+ | **Tor Network itself** | Overkill / Mega-Cluster |

---

## 3. Analysis & Recommendations

### The "Sweet Spot": 4 Cores / 8 GB
*   **Why:** Tor requires high single-core performance. 4 Cores allows:
    *   Core 0: OS + XDP (Network Interrupts)
    *   Core 1: Tor Instance 1
    *   Core 2: Tor Instance 2
    *   Core 3: Fortify (Logic/Redis)
*   **8GB RAM:** Enough for a large Redis Cluster state + 1 Million Ammo Box items without swapping.

### Low-End Survival (1-2 Cores)
*   **Tuning:** Reduce `MAX_CONN` in HAProxy to 20k.
*   **Ammo Box:** Cap disk usage to 100k items.
*   **Risk:** If Tor eats 100% of Core 0, SSH becomes unresponsive.
*   **Mitigation:** `cpulimit` or `nice` on Tor process to prioritize SSH/XDP.

### High-End Scaling (16+ Cores)
*   **Multi-Instance Tor:** A single Tor process cannot use 16 cores. You **must** run `tor-instance-1` ... `tor-instance-10` behind HAProxy to utilize the hardware.
*   **OnionBalance:** Use OnionBalance to aggregate these instances into one master address.
*   **Redis:** Enable Sharding (Cluster Mode) to utilize the massive RAM.

### RAM vs CPU Tradeoff
*   **High RAM / Low CPU:** Good for holding massive connection tables (Slowloris resilience) but weak against fast packet floods.
*   **High CPU / Low RAM:** Good for scrubbing packet floods (XDP) and generating CAPTCHAs, but crashes if too many users stay connected (OOM).

---

> **Verdict:** Cerberus is designed to be **efficient**. Even a **2 Core / 4 GB** node provides defense superior to most legacy setups. Scaling horizontally (adding more 4C/8G nodes) is often more effective than scaling vertically (one massive 64C node) due to Tor's architectural limits.
