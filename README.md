# 🛡️ Cerberus - High-Assurance Tor Ingress Defense System

**Multi-Layered DDoS Mitigation and Access Control for Tor Onion Services**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Tor](https://img.shields.io/badge/Tor-V3%20Onion-7D4698.svg)](https://www.torproject.org/)
[![Rust](https://img.shields.io/badge/Rust-1.82%2B-orange.svg)](https://www.rust-lang.org/)
[![Status](https://img.shields.io/badge/Status-Phase%201%20Planning-yellow.svg)](docs/0001-project-scaffold.md)

> **⚠️ PROJECT STATUS**: Cerberus is in the **planning and design phase**. All architecture and specifications are documented. Code implementation begins with Phase 1.

---

## 🎯 What is Cerberus?

Cerberus is a specialized, defense-in-depth reverse proxy designed exclusively for **Tor Onion Services** operating in hostile environments. It provides **four layers of protection** between the Tor network and your backend service, starting with a Layer 0 XDP/eBPF flood shield at the NIC.

### Core Design Philosophy: Human-Cost Asymmetry

> **Make the cost of being wrong trivial for humans and expensive for bots.**

- **Humans breeze through**: One easy CAPTCHA, quick solve, mistakes forgiven
- **Bots drown**: Failed attempts trigger escalation → multi-CAPTCHA chains → soft-locks → bans

**Real-world impact**: A bot needs **38+ days** to make 10,000 requests. A human needs **seconds**.

### The Four Heads (Defense Layers)

0. **XDP/eBPF (Layer 0 - The Flood Shield)**: Drops abusive packets at the NIC, per-relay rate limiting, SYN/malformed flood protection
1. **TC eBPF (Layer 0.5 - Flow Shaper)**: Stateful relay-aware flow shaping, latency/delay, probabilistic drops, skb marks for HAProxy
2. **Kernel TCP Policy**: SYN cookies, backlog caps, aggressive cleanup, timeouts
3. **HAProxy (Layer 1 - The Shield, TCP/HTTP)**: Connection management, circuit reputation tracking, stick tables, HTTP protocol correctness, header limits
4. **Protocol Normalization**: Path/header normalization, CRLF, canonical Host, parser differential defense (HAProxy/Nginx or Rust filter)
5. **Nginx (Layer 2 - The Filter)**: Protocol sanitization, static CAPTCHA delivery, header scrubbing
6. **Nginx ↔ Fortify Isolation**: UNIX socket, strict timeouts, memory caps, queue governor
7. **Fortify (Layer 3 - The Keeper)**: Rust application for CAPTCHA verification, threat analysis, adaptive defense

### Built for Tor
- **Tor-Native**: Leverages Tor's PROXY protocol for per-circuit tracking
- **PoW-Enabled**: Integrates with Tor's Proof-of-Work defenses
- **Zero JavaScript**: Works in Tor Browser Safest Mode (100% server-side)
- **Privacy-First**: No IP logging, no fingerprinting—behavior-based defense only

---


## 📋 Implementation Phases

### 🚀 Phase 1: Minimal Viable Proxy (MVP)
> **Goal**: Working reverse proxy that serves a static "Hello World" page through all layers (except XDP).

- HAProxy → Nginx → Fortify → Static HTML response
- Verify traffic flows through the complete stack
- No CAPTCHA, no threat detection—just connectivity proof
- **Exit Criteria**: Tor Browser can reach `hello.html` through your .onion

### ⚡ Phase 1.5: XDP/eBPF Flood Shield
> **Goal**: Survive volumetric attacks before sockets are allocated.

- XDP program drops excess packets per Tor relay IP
- SYN flood and malformed packet detection
- eBPF maps for relay rate tracking
- **Exit Criteria**: Machine stays responsive under raw packet flood

### 🔧 Phase 2: HAProxy Hardening & Metrics
> **Goal**: Connection management, circuit tracking, stick tables, and basic observability.

- Parse Tor PROXY protocol for circuit IDs
- Implement rate limiting with stick tables
- Add connection limits per circuit
- Configure slowloris protection (aggressive timeouts)
- **Prometheus metrics for all connection events**
- **Exit Criteria**: HAProxy blocks flooding from single circuit, metrics visible

### 🛡️ Phase 3: Nginx Hardening
> **Goal**: Protocol sanitization and security headers.

- Header scrubbing (remove fingerprinting vectors)
- Strict CSP and security headers
- Body size limits and timeout tuning
- Static file serving optimization
- **Exit Criteria**: No leaky headers, clean protocol handling

### 🎯 Phase 4: Basic CAPTCHA System
> **Goal**: First working CAPTCHA gate in Fortify.

- **Distorted Text CAPTCHA** (single variant)
- Pre-generation pool for fast response
- Constant-time verification
- Basic session token management
- **Exit Criteria**: CAPTCHA blocks automated requests, passes Tor Browser

### 📊 Phase 5: Threat Dial System
> **Goal**: Dynamic defense intensity (1-10 levels).

- Implement threat level state machine
- Automatic escalation based on load metrics
- Per-level CAPTCHA difficulty adjustment
- HAProxy rate limit integration
- **Exit Criteria**: System auto-escalates during simulated attack

### 🧩 Phase 6: Advanced CAPTCHA System
> **Goal**: All 6 CAPTCHA variants with Human-Cost Asymmetry.

| Variant | Description |
|---------|-------------|
| **Distorted Text** | Warped text with noise overlays |
| **Object Recognition** | "Click all images containing X" |
| **Pattern Completion** | "Which image completes this pattern?" |
| **Color-Text Mismatch** | Color names in wrong colors |
| **PoET (Proof-of-Elapsed-Time)** | Minimum viewing time (4-8 seconds) |
| **Interaction Puzzles** | Contextual memory tests |

- Behavioral profiling and soft-lock escalation
- Multi-CAPTCHA chains for suspicious actors
- **Exit Criteria**: Bots need 38+ days for 10k requests

### ⏳ Phase 7: Virtual Queue System
> **Goal**: Browser-side waiting room with priority lanes.

- Three lanes: VIP (validated), PoW (working), Normal (waiting)
- Client-side PoW puzzle generation
- Server-side PoW verification
- Token-based position management
- **Exit Criteria**: Queue absorbs 10k connection burst gracefully

### 🧠 Phase 8: Behavioral Profiling
> **Goal**: Automated threat classification without fingerprinting.

- Request timing pattern analysis
- Endpoint diversity scoring
- Session behavior tracking
- Soft-lock escalation chains
- **Exit Criteria**: Bot patterns detected in <5 requests

### 📈 Phase 9: Monitoring & Admin UI
> **Goal**: Grafana dashboards and operator controls.

- Prometheus metrics export
- Pre-built Grafana dashboards
- Remote Grafana streaming (optional VPS)
- Admin panel for manual overrides
- **Exit Criteria**: Full visibility into system state

### 🌐 Phase 10: Cluster System
> **Goal**: Multi-node deployment with shared state.

- WireGuard peer-to-peer mesh
- Redis Cluster for shared stick tables
- Each node fully independent (no leader)
- Private network: 10.100.0.0/24
- **Exit Criteria**: 3-node cluster handles failover

### 💰 Phase 11: XMR Priority System
> **Goal**: Monero micropayments for queue fast-pass.

- Monero wallet integration
- Payment verification via RPC
- VIP lane promotion on payment
- Rate-limited to prevent abuse
- **Exit Criteria**: XMR payment grants instant access

### 🔒 Phase 12: Production Hardening
> **Goal**: Security audit and production readiness.

- External security audit
- Load testing (10k+ concurrent)
- Docker/systemd deployment
- Ansible automation playbooks
- **Exit Criteria**: Production-ready v1.0 release

---

## ✨ Feature Summary

| Category | Feature | Phase | Status |
|----------|---------|-------|--------|
| **Core Proxy** | HAProxy → Nginx → Fortify chain | 1 | 📋 Planned |
| **Core Proxy** | Circuit ID tracking (PROXY protocol) | 2 | 📋 Planned |
| **Core Proxy** | Stick table reputation | 2 | 📋 Planned |
| **Security** | Header scrubbing & CSP | 3 | 📋 Planned |
| **CAPTCHA** | Distorted Text variant | 4 | 📋 Planned |
| **CAPTCHA** | 6 AI-resistant variants | 6 | 📋 Planned |
| **CAPTCHA** | Human-Cost Asymmetry design | 6 | 📋 Planned |
| **Defense** | Threat Dial (1-10 levels) | 5 | 📋 Planned |
| **Defense** | Behavioral profiling | 8 | 📋 Planned |
| **Queue** | Virtual Queue (3 lanes) | 7 | 📋 Planned |
| **Queue** | Client-side PoW | 7 | 📋 Planned |
| **Monitoring** | Grafana dashboards | 9 | 📋 Planned |
| **Monitoring** | Remote streaming (VPS) | 9 | 📋 Planned |
| **Cluster** | WireGuard P2P mesh | 10 | 📋 Planned |
| **Cluster** | Redis Cluster state sync | 10 | 📋 Planned |
| **Premium** | XMR micropayments | 11 | 📋 Planned |

---


## 🏗️ Architecture Overview

```
┌────────────────────────────────────────────────────────────────────────────┐
│                                The Internet                               │
└────────────────────────────────────────────────────────────────────────────┘
                                      ↓
┌────────────────────────────────────────────────────────────────────────────┐
│                             Tor Network (3 hops)                          │
└────────────────────────────────────────────────────────────────────────────┘
                                      ↓
┌────────────────────────────────────────────────────────────────────────────┐
│   NIC / XDP / eBPF (Flood Shield)                                         │
│   • Drops excess packets per relay IP                                     │
│   • SYN flood/malformed packet detection                                  │
└────────────────────────────────────────────────────────────────────────────┘
                                      ↓
┌────────────────────────────────────────────────────────────────────────────┐
│   Kernel TCP Stack                                                        │
└────────────────────────────────────────────────────────────────────────────┘
                                      ↓
┌────────────────────────────────────────────────────────────────────────────┐
│   Tor Daemon                                                              │
│   • Proof-of-Work enabled (HiddenServicePoWDefensesEnabled)               │
│   • Vanguards protection                                                  │
│   • PROXY protocol (circuit IDs → downstream)                             │
└────────────────────────────────────────────────────────────────────────────┘
                                      ↓ (127.0.0.1:10000)
┌────────────────────────────────────────────────────────────────────────────┐
│   Layer 1: HAProxy (The Shield)                                           │
│   • Connection limits & stick tables                                      │
│   • Circuit reputation tracking                                           │
│   • Rate limiting & slowloris protection                                  │
│   • Prometheus metrics                                                    │
└────────────────────────────────────────────────────────────────────────────┘
                                      ↓ (127.0.0.1:10001)
┌────────────────────────────────────────────────────────────────────────────┐
│   Layer 2: Nginx (The Filter)                                             │
│   • Static CAPTCHA delivery                                               │
│   • Header scrubbing & CSP                                                │
│   • Protocol sanitization                                                 │
└────────────────────────────────────────────────────────────────────────────┘
                                      ↓ (127.0.0.1:10002)
┌────────────────────────────────────────────────────────────────────────────┐
│   Layer 3: Fortify (The Keeper)                                           │
│   • CAPTCHA generation & verification                                     │
│   • Threat Dial control                                                   │
│   • Behavioral profiling                                                  │
│   • Session management                                                    │
└────────────────────────────────────────────────────────────────────────────┘
                                      ↓
┌────────────────────────────────────────────────────────────────────────────┐
│                        Target Backend Service                             │
└────────────────────────────────────────────────────────────────────────────┘
```

**Defense in Depth**: Each layer is optimized for what it does best. L2/L3 (XDP) keeps the box alive, L4 (HAProxy) shapes connections, L7 (Nginx/Fortify) enforces identity and behavior.

---

## 🚀 Getting Started

### Current Status: Phase 1 Planning

All architecture and specifications are **complete**. Implementation begins with Phase 1: Minimal Viable Proxy.

### System Requirements

| Component | Version | Purpose |
|-----------|---------|---------|
| **OS** | Ubuntu 22.04 / Debian 12 | Primary targets |
| **Tor** | 0.4.8+ | PoW support |
| **HAProxy** | 2.8+ LTS | Connection management |
| **Nginx** | 1.26+ | Static delivery |
| **Rust** | 1.82+ | Fortify application |
| **RAM** | 2GB (4GB rec.) | All services |
| **CPU** | 2 cores (4 rec.) | Concurrent handling |

### Quick Start (Coming Soon)

```bash
# Phase 1 implementation will provide:
git clone https://github.com/Nespartious/Cerberus.git
cd Cerberus
./deploy/cerberus.sh install
./deploy/cerberus.sh start
```

---

## 📚 Documentation

**Core Documentation:**
- **[Master Architecture](docs/0000-master-architecture.md)**: Complete system design and philosophy
- **[Project Scaffold](docs/0001-project-scaffold.md)**: Folder structure and project organization
- **[Instructions](docs/0002-instructions.md)**: Security gotchas, Tor best practices, roles, user stories, and development workflow

**Defense Layers (0100-series):**
- **[XDP/eBPF Layer](docs/0103-layer0-xdp.md)**: NIC-level flood shield, per-relay rate limiting
- **[TC eBPF Flow Shaping](docs/0104-tc-ebpf-flow-shaping.md)**: Stateful relay-aware flow shaping, latency/delay, skb marks
- **[Kernel TCP Tuning](docs/0105-kernel-tcp-tuning.md)**: SYN cookies, backlog caps, timeouts
- **[HAProxy Layer](docs/0100-layer1-haproxy.md)**: Connection management and circuit tracking
- **[HAProxy HTTP Gate](docs/0106-haproxy-http-gate.md)**: HTTP protocol correctness, header limits
- **[Protocol Normalization](docs/0107-protocol-normalization.md)**: Path/header normalization, CRLF, canonical Host
- **[Nginx Layer](docs/0101-layer2-nginx.md)**: Protocol sanitization and static delivery
- **[Nginx ↔ Fortify Isolation](docs/0108-nginx-fortify-isolation.md)**: UNIX socket, timeouts, memory caps, queue governor
- **[Fortify Layer](docs/0102-layer3-fortify.md)**: Rust application logic and CAPTCHA system
**Security & Threat Modeling (0200-series):**
- **[Threat Model](docs/0204-threat-model.md)**: Practical and STRIDE threat models for Cerberus
- **[Attack Kill Table](docs/0205-attack-kill-table.md)**: What dies at which layer, mapped to Cerberus stack
---

## 🛡️ Attack Kill Table (Summary)

See [docs/0205-attack-kill-table.md](docs/0205-attack-kill-table.md) for the full table.

| Attack Type         | XDP | TC eBPF | Kernel TCP | HAProxy | Nginx | Fortify |
|---------------------|-----|---------|------------|---------|-------|---------|
| Packet flood        | ✅  | —       | —          | —       | —     | —       |
| SYN flood           | ✅  | ✅      | ✅         | —       | —     | —       |
| TCP exhaustion      | —   | ✅      | ✅         | ✅      | —     | —       |
| Connection churn    | —   | ✅      | —          | ✅      | —     | —       |
| Slowloris           | —   | —       | —          | ✅      | ✅    | —       |
| HTTP floods         | —   | —       | —          | ✅      | ✅    | —       |
| Malformed HTTP      | —   | —       | —          | —       | ✅    | —       |
| CAPTCHA bypass      | —   | —       | —          | —       | —     | ✅      |
| Bot navigation      | —   | —       | —          | —       | —     | ✅      |
| CAPTCHA farms       | —   | —       | —          | —       | —     | ⚠️      |
| Human users         | ❌  | ❌      | ❌         | ❌      | ❌    | ❌      |

Legend: ✅ = attack dies here, ⚠️ = attack slowed, ❌ = allowed, — = not relevant

---

## 🔒 Threat Model (Summary)

See [docs/0204-threat-model.md](docs/0204-threat-model.md) for both practical and STRIDE threat models.

**Assets:** Backend onion service, host CPU/memory, Tor intro capacity, human access
**Adversaries:** Script kiddies, botnets, Tor-aware attackers, well-funded adversaries
**Trust Boundaries:** Untrusted → XDP → TC eBPF → Kernel TCP → HAProxy → Nginx → Fortify → Backend
**Design Invariants:** No layer assumes previous succeeded; each layer validates, caps, fails closed

**Defense Features (0200-series):**
- **[Virtual Queue System](docs/0200-feature-virtual-queue.md)**: Browser-side waiting room with PoW priority
- **[Threat Dial System](docs/0201-feature-threat-dial.md)**: Dynamic defense intensity control
- **[XMR Priority System](docs/0202-feature-xmr-priority.md)**: Monero payment-based queue fast-pass
- **[Advanced CAPTCHA System](docs/0203-feature-advanced-captcha.md)**: AI-resistant image CAPTCHAs with multiple variants

**Operations (0300-series):**
- **[Monitoring & UI](docs/0300-operations-monitoring-ui.md)**: Grafana dashboards, admin panel, metrics
- **[Vanity Onion Generation](docs/0301-operations-vanity-onion.md)**: mkp224o integration for branded addresses

**Cluster & Scaling (0500-series):**
- **[Cluster System](docs/0500-operations-cluster-system.md)**: Multi-node deployment with shared state and load distribution

**Infrastructure (0400-series):**
- **[Dependencies Audit](docs/0400-infra-dependencies.md)**: Version matrix and compatibility
- **[CI/CD Workflows](docs/0401-infra-ci-cd.md)**: Automated checks and code review standards
- **[Development Environment](docs/0402-infra-dev-environment.md)**: Cross-platform development setup (Windows + Ubuntu VM)

---

## 🎯 Target Use Cases

### ✅ Designed For
- **High-Value Tor Onion Services** (marketplaces, forums, whistleblower platforms)
- **Services expecting constant DDoS** (circuit-based defense, not IP-based)
- **Privacy-first operations** (no fingerprinting, no JavaScript requirements)
- **Security researchers** (testing Tor infrastructure defenses)

### ❌ Not For
- Clearnet websites (use Cloudflare instead)
- Low-traffic personal sites (overkill)
- Rich JavaScript applications (Cerberus prioritizes NoJS)

---

## 🤝 Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Development Workflow
1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-defense`)
3. Write tests for your changes
4. Ensure all tests pass (`cargo test`, integration tests, Tor Browser tests)
5. Commit with clear messages (`git commit -m 'Add circuit clustering detection'`)
6. Push to your fork (`git push origin feature/amazing-defense`)
7. Open a Pull Request with detailed description

### Code Review Requirements
- ✅ All CI/CD checks pass (security audit, lint, tests)
- ✅ At least one maintainer approval
- ✅ No security vulnerabilities introduced
- ✅ Documentation updated (if user-facing changes)
- ✅ Tested in Tor Browser (Safest + Standard modes)

---

## 🔒 Security

### Reporting Vulnerabilities

**DO NOT** open public issues for security vulnerabilities.

- **Security Contact**: TBD (will be announced with v1.0 release)
- **Non-Security Issues**: Use [GitHub Issues](https://github.com/Nespartious/Cerberus/issues)

We follow responsible disclosure and will credit researchers in our security advisories.

### Security Audit Status
- **Last Audit**: Pending (project in active development)
- **Audit Scope**: Full stack (Tor, HAProxy, Nginx, Fortify)
- **Bug Bounty**: Planned (post-v1.0 release)

---

## 📊 Performance Targets

| Metric | Target | Rationale |
|--------|--------|-----------|
| Concurrent Connections | 10,000 | HAProxy + kernel tuning |
| CAPTCHA Generation | <100ms | Pre-generation pool |
| CAPTCHA Verification | <10ms | Constant-time comparison |
| Static Page Delivery | <5ms | Nginx direct serve |
| Circuit Ban Latency | <50ms | Stick table update |
| Memory (all services) | ~100MB | Lean deployment |

---

## 🗺️ Roadmap

### ✅ Planning Complete
- [x] Master architecture design (including XDP/eBPF Layer 0)
- [x] 17 planning documents
- [x] User stories for all features
- [x] CI/CD workflow specifications
- [x] Security guidelines and Tor best practices

### ⏳ Phase 1: MVP (Next)
- [ ] Project structure setup
- [ ] HAProxy basic configuration
- [ ] Nginx pass-through
- [ ] Fortify "Hello World" response
- [ ] End-to-end connectivity test

### 📋 Phases 2-6: Core Defense
- [ ] Circuit tracking and stick tables
- [ ] Header scrubbing and CSP
- [ ] CAPTCHA system (all 6 variants)
- [ ] Threat Dial implementation

### 📋 Phases 7-9: Advanced Features
- [ ] Virtual Queue with PoW
- [ ] Behavioral profiling
- [ ] Monitoring dashboards

### 📋 Phases 10-12: Scale & Ship
- [ ] WireGuard cluster mesh
- [ ] XMR priority payments
- [ ] Security audit and v1.0 release

---

## 📜 License

**MIT License** - Free and Open Source

```
Copyright (c) 2025 Cerberus Project

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

---

## 🙏 Acknowledgments

- **The Tor Project** for building the foundation of anonymous communication
- **HAProxy Team** for the most robust load balancer/firewall
- **Nginx Team** for high-performance web serving
- **Rust Community** for a secure systems programming language
- **Vanguards Developers** for protecting onion services from advanced attacks

---

## 📞 Community

- **GitHub**: [https://github.com/Nespartious/Cerberus](https://github.com/Nespartious/Cerberus)
- **Issues**: [Report bugs or request features](https://github.com/Nespartious/Cerberus/issues)
- **Discussions**: [Project discussions](https://github.com/Nespartious/Cerberus/discussions)

---

## ⚠️ Disclaimer

Cerberus is designed to protect legitimate Tor Onion Services from abuse. Users are responsible for ensuring their use of this software complies with applicable laws and regulations. The developers of Cerberus do not condone illegal activity and provide this software for defensive security purposes only.

**Remember**: Operating a Tor Onion Service comes with responsibilities. Understand your threat model, follow best practices, and respect user privacy.

---

<p align="center">
  <strong>Designing the future of Tor Onion Service defense.</strong>
</p>

<p align="center">
  A work in progress for the Tor community 🛡️
</p>
