# Report I: Technical Audit of the Cerberus (Projected) Framework

This report provides a formal technical audit of the Cerberus and Endgame V3 defensive frameworks for Tor hidden services. It analyzes their architectural components, mitigation strategies against DDoS attacks, and the strategic implications of their design choices.

Cerberus is an advanced, multi-layer defensive framework designed for high-risk hidden services within the Tor network. Its architecture is characterized by a decoupled "segregation of concerns" model, moving away from monolithic filtering toward a hardware-accelerated and kernel-level defensive pipeline.

## Architectural Segregation and Component Logic
The Cerberus design utilizes a six-layer stack to scrutinize traffic before it reaches the core application. By isolating functional roles, the system ensures that a resource-exhaustion event at one layer does not necessarily cascade to the entire infrastructure.

| Layer | Component | Functional Classification | Technical Mechanism |
|-------|-----------|---------------------------|---------------------|
| L0 | NIC / XDP / eBPF | The Flood Shield | Per-Relay IP packet dropping via eBPF maps |
| L1 | HAProxy | The Shield | Connection management and circuit reputation tracking |
| L2 | Nginx | The Filter | Static asset delivery and header sanitization |
| L3 | Fortify (Rust) | The Keeper | Adaptive logic, behavioral profiling, and session state |
| L4 | Backend | Core Service | Isolated internal application environment |

### Layer 0: Hardware-Level Packet Filtering (XDP/eBPF)
A primary innovation of Cerberus is the utilization of XDP (eXpress Data Path) and eBPF. This enables packet processing at the earliest possible point in the network driver—before the Linux kernel even allocates a socket buffer (sk_buff).
* **Performance:** XDP programs can process millions of packets per second with minimal CPU overhead, often dropping malicious traffic in less than 1 microsecond.
* **Mechanism:** It implements per-relay IP throttling, identifying aggressive Tor relays and dropping their packets at the NIC level to prevent kernel TCP stack saturation.

### Layer 1: Circuit Management (HAProxy)
Acting as "The Shield," HAProxy manages connection limits and maintains Stick Tables to track circuit IDs.1
* **Reputation Tracking:** By integrating the PROXY protocol, Cerberus identifies specific Tor circuits and applies penalties to those displaying aggressive behavioral patterns (e.g., slowloris attacks or high-frequency GET requests).
* **Observability:** The system is designed to export metrics to Prometheus for real-time visualization of connection health.

### Layer 3: The "Fortify" Intelligence Engine (Rust)
"The Keeper" serves as the centralized logic controller. Following a strict Rust-only development path, this component prioritizes memory safety and high-performance execution without the overhead of a garbage collector.2
* **Threat Dial Control:** An adaptive global setting that increases the rigor of verification challenges (e.g., more complex proof-of-work or behavioral traps) as attack intensity scales.
* **Behavioral Profiling:** Sophisticated algorithms analyze user interaction timing and request sequences to distinguish between legitimate human users and automated scrapers.

# Report II: Comparative Analysis and Vulnerability Assessment
This report compares the production-ready Endgame V3 framework with the projected Cerberus architecture, outlining the strengths and weaknesses inherent in their differing approaches to hidden service security.

## Comparative System Overview

| Feature | Endgame V3 | Cerberus (Projected) |
|---------|------------|----------------------|
| Logic Layer | Nginx / LuaJIT | Rust (Fortify Engine) |
| L2-L4 Filtering | Nginx Application Layer | NIC-Level XDP / eBPF |
| Load Balancing | GoBalance (Go) | HAProxy (C-based) |
| Verification | Rust-based Captcha 2 | Behavioral Profiling & Threat Dial |
| Maturity | Battle-tested / Production 1 | Design-Phase / Development |

## Structural Comparisons

### 1. Consolidation vs. Decoupling
* **Endgame V3** is a highly consolidated system. It leverages the "magic" of the lua-nginx-module to handle filtering, session management, and captchas directly within the Nginx worker processes.1 This reduces internal latency by minimizing inter-process communication (IPC) but makes Nginx a single point of failure if worker processes are CPU-starved.
* **Cerberus** adopts a decoupled model. By offloading heavy network filtering to XDP and connection management to HAProxy, it ensures that the "Filter" (Nginx) and "Keeper" (Rust logic) layers only receive pre-vetted, high-quality traffic.

### 2. Performance and Resource Management
* **Endgame** relies heavily on high-clock-speed CPUs (3GHz+) to handle single-threaded Tor cell processing and Lua execution.1
* **Cerberus** gains a significant performance edge through XDP/eBPF. Because XDP bypasses most of the Linux networking stack, it can withstand massive network-layer floods that would traditionally trigger kernel-level "oom-killers" or CPU lockups on an Endgame frontend.

### 3. Language Safety and Reliability
* **Endgame V3** is a hybrid of Go (80.4%), Lua (7.8%), and Rust (5.1%).2 While GoBalance provides excellent descriptor management, the core filtering logic remains in Lua, which is less strictly typed and lacks the exhaustive compile-time safety guarantees of Rust.
* **Cerberus's** commitment to a Rust-only development path for its logic layer (Fortify) minimizes vulnerabilities related to memory corruption and concurrency, which are critical in high-concurrency DDoS scenarios.

## Strategic Weaknesses and Considerations

### Endgame V3 Weaknesses
* **Single-Threaded Bottlenecks:** Tor is inherently single-threaded. While Endgame scales horizontally through GoBalance, each individual "Front" is still limited by the speed of its primary CPU core.1
* **Static Captchas:** While computationally intensive for the client, static captchas are increasingly vulnerable to AI-based solving services, necessitating frequent updates to the generation algorithms.2

### Cerberus Weaknesses
* **Architectural Latency:** The decoupled nature of Cerberus (XDP -> HAProxy -> Nginx -> Rust) introduces multiple internal "hops." In a high-latency environment like Tor, these additional processing steps could impact user experience if not perfectly optimized.
* **Deployment Complexity:** Setting up XDP/eBPF requires specific Linux kernel versions (≥5.10) and hardware compatibility that may not be available on all standard VPS providers.

## Conclusion
Endgame V3 is currently the state-of-the-art for operators requiring an immediate, battle-tested deployment. Its use of GoBalance to "scale to the moon" provides a robust horizontal scaling strategy that is easily automated.1

Cerberus represents the next generation of architectural maturity. By moving the defensive line to the hardware level (XDP) and utilizing a Rust-only logic engine, it provides a much more resilient "castle" design capable of weathering sophisticated network-layer attacks that might overwhelm the application-layer filters of current-gen systems.

## Works cited
* onionltd/EndGame: EndGame DDoS filter. - GitHub, accessed January 30, 2026, https://github.com/onionltd/EndGame
* EndGame is a front-end system that protects core application servers on an onion service, ensuring privacy without third-party reliance. Locally run and free for all to use, it combines multiple technologies to deliver secure computing magic! - GitHub, accessed January 30, 2026, https://github.com/akshzyx/EndGame
