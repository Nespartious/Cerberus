# 🛡️ Cerberus - High-Assurance Tor Ingress Defense System

**Multi-Layered DDoS Mitigation and Access Control for Tor Onion Services**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Tor](https://img.shields.io/badge/Tor-V3%20Onion-7D4698.svg)](https://www.torproject.org/)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![Status](https://img.shields.io/badge/Status-Planning%20%2F%20Design-yellow.svg)](docs/scaffold.md)

> **⚠️ PROJECT STATUS**: Cerberus is currently in the **design and documentation phase**. The architecture, specifications, and implementation plans are complete, but the actual code has not been written yet. This README describes the intended design and capabilities.

---

## 🎯 What is Cerberus?

Cerberus is a specialized, defense-in-depth ingress architecture designed exclusively for **Tor Onion Services** operating in hostile environments. Unlike traditional web application firewalls, Cerberus assumes DDoS attacks, deanonymization attempts, and automated abuse are the norm—not the exception.

Named after the three-headed guardian of the underworld, Cerberus provides **three layers of protection**:

1. **HAProxy (Layer 1 - The Shield)**: Connection management, circuit reputation tracking, and aggressive rate limiting
2. **Nginx (Layer 2 - The Filter)**: Protocol sanitization, static CAPTCHA delivery, and buffer attack prevention  
3. **Fortify (Layer 3 - The Keeper)**: Rust-based application logic for CAPTCHA verification, threat analysis, and adaptive defense

### 🔒 Built for the Dark Web
- **Tor-Native**: Leverages Tor's PROXY protocol for per-circuit tracking and reputation scoring
- **PoW-Enabled**: Integrates with Tor's Proof-of-Work defenses (HiddenServicePoWDefensesEnabled)
- **Zero-Trust**: Works without JavaScript, cookies, or persistent client state (Tor Browser Safest Mode compatible)
- **Privacy-First**: No IP logging, no user tracking, no fingerprinting—just behavior-based defense

---

## ✨ Planned Features

### 🚀 Performance & Scalability Goals
- **10,000 concurrent connections** target with optimized configuration
- **Static CAPTCHA gate** design to offload traffic from application layer
- **Async Rust backend** (Tokio) planned for non-blocking I/O
- **Fast CAPTCHA generation** with pre-generation pool architecture
- **Virtual queue system** to prevent server resource exhaustion during attacks

### 🛡️ Advanced DDoS Mitigation
- **Per-Circuit Rate Limiting**: Track and throttle Tor circuits independently (not IPs)
- **Virtual Queue System**: Browser-side waiting room with token-based priority (offloads queue burden from server)
- **Stick Table Reputation**: VIP promotion for validated users, instant banning for abusers
- **Adaptive Thresholds**: Dynamic difficulty scaling based on attack severity
- **Slowloris Protection**: Aggressive timeouts at HAProxy and Nginx layers

### 🎯 Tor-Specific Defenses
- **Circuit ID Tracking**: Identify and ban malicious circuits without breaking Tor anonymity
- **Vanguards Integration**: Protect against guard discovery and circuit correlation attacks
- **Time-Limited Bans**: Circuits banned for 30-60 minutes (not permanent, respecting Tor's rotation)
- **Introduction Point Protection**: Defend against intro point flooding and enumeration

### 🔐 Security Hardening
- **Strict CSP Headers**: Block XSS, clickjacking, and data exfiltration
- **Header Scrubbing**: Remove fingerprinting vectors (User-Agent, Accept-Language, etc.)
- **Zero JavaScript**: Fully functional in Tor Browser Safest Mode (100% server-side rendering)
- **Constant-Time Comparisons**: Prevent timing attacks on CAPTCHA validation
- **Least Privilege**: Separate service users, chroot jails, file permission hardening

---

## 🏗️ Architecture Overview

```
  The Internet
       ↓
   Tor Network (3 hops)
       ↓
┌──────────────────────┐
│   Tor Daemon (PoW)   │ ← Proof of Work, Vanguards, Circuit IDs
└──────────────────────┘
       ↓ (127.0.0.1:10000)
┌──────────────────────┐
│  HAProxy (Layer 1)   │ ← Connection limits, Stick tables, Rate limiting
└──────────────────────┘
       ↓ (127.0.0.1:10001)
┌──────────────────────┐
│   Nginx (Layer 2)    │ ← Static CAPTCHA, Header scrubbing, Timeouts
└──────────────────────┘
       ↓ (127.0.0.1:10002)
┌──────────────────────┐
│  Fortify (Layer 3)   │ ← CAPTCHA verification, Reputation management
└──────────────────────┘
       ↓
  Target Service / Backend
```

**Defense in Depth**: Each layer handles different attack vectors. Attackers must bypass all three layers to reach your application.

---

## 🚀 Getting Started

### Current Status: Documentation Phase

Cerberus is currently in the **design and planning phase**. The complete architecture and specifications are documented, but implementation has not begun.

### Planned Deployment Environments

**Target Platforms:**
- **Ubuntu 22.04 LTS or 24.04 LTS** (Primary target)
- **Debian 11 (Bullseye) or 12 (Bookworm)** (Primary target)
- **Docker** (Planned containerized deployment)

**Planned System Requirements:**
- **Tor**: 0.4.8+ (for PoW support)
- **HAProxy**: 2.8+ LTS
- **Nginx**: 1.26+
- **Rust**: 1.82+ (for Fortify)
- **RAM**: 2GB minimum (4GB recommended)
- **CPU**: 2 cores minimum (4 cores recommended)
- **Disk**: 10GB minimum

### Implementation Roadmap

See [docs/scaffold.md](docs/scaffold.md) for the complete folder structure and [docs/CERBERUS_MASTER_ARCH.md](docs/CERBERUS_MASTER_ARCH.md) for architecture details.

**Sprint 1** (Current): Documentation and architecture design ✅  
**Sprint 2** (Next): Core implementation (HAProxy, Nginx, Fortify basics)  
**Sprint 3**: Advanced features (Virtual queue, adaptive defenses)  
**Sprint 4**: Testing, hardening, and production readiness

---

## 📚 Documentation

**Core Documentation:**
- **[Master Architecture](docs/0000-master-architecture.md)**: Complete system design and philosophy
- **[Project Scaffold](docs/0001-project-scaffold.md)**: Folder structure and project organization
- **[Instructions](docs/0002-instructions.md)**: Security gotchas, Tor best practices, roles, user stories, and development workflow

**Defense Layers (0100-series):**
- **[HAProxy Layer](docs/0100-layer1-haproxy.md)**: Connection management and circuit tracking
- **[Nginx Layer](docs/0101-layer2-nginx.md)**: Protocol sanitization and static delivery
- **[Fortify Layer](docs/0102-layer3-fortify.md)**: Rust application logic and CAPTCHA system

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

## 🛠️ Planned Configuration

### Intended Configuration Structure

The planned configuration system will use a simple INI-style format:

```ini
# Example cerberus.conf (not yet implemented)
TARGET_ONION=your-backend-onion.onion
TARGET_PORT=80
DDOS_SENSITIVITY=medium
MAX_CONNECTIONS=10000
CAPTCHA_TTL=300
```

### Configuration Documentation

Detailed configuration specifications available in:
- [HAProxy Configuration](docs/haproxy.md)
- [Nginx Configuration](docs/nginx.md)
- [Fortify Configuration](docs/fortify.md)

---

## 🧪 Planned Testing Strategy

Comprehensive testing approach documented in [docs/ci-cd-workflows.md](docs/ci-cd-workflows.md):

- **Unit Tests**: Individual component testing with Rust's cargo test
- **Integration Tests**: Full stack testing (Tor → HAProxy → Nginx → Fortify)
- **Load Testing**: Simulated DDoS scenarios with multiple circuits
- **Browser Tests**: Automated Tor Browser testing (Safest + Standard modes)
- **Security Tests**: Penetration testing and vulnerability scanning

---

## 🎯 Intended Use Cases

### Who Is Cerberus Being Designed For?

Once implemented, Cerberus will be intended for:

✅ **High-Value Tor Onion Services**
- Marketplaces, forums, whistleblower platforms
- Services anticipating constant DDoS attacks
- Privacy-critical applications requiring zero-trust architecture

✅ **Darknet Operators**
- Need circuit-based rate limiting (not IP-based)
- Require CAPTCHA without JavaScript (Safest Mode support)
- Want defense-in-depth without sacrificing anonymity

✅ **Security Researchers**
- Testing Tor infrastructure defenses
- Analyzing circuit-based attacks
- Studying onion service availability under load

❌ **Not Intended For**
- Clearnet websites (use Cloudflare or traditional WAFs instead)
- Low-traffic personal sites (overkill, unnecessary complexity)
- Services requiring rich JavaScript interactions (Cerberus prioritizes NoJS compatibility)

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

These are the **planned performance goals** for Cerberus once implemented:

| Metric | Target | Design Rationale |
|--------|--------|------------------|
| **Concurrent Connections** | 10,000 | HAProxy + kernel tuning |
| **CAPTCHA Generation** | <100ms | Pre-generation pool |
| **CAPTCHA Verification** | <10ms | Constant-time comparison |
| **Static Page Delivery** | <5ms | Nginx direct serve |
| **Circuit Ban Latency** | <50ms | HAProxy stick table update |
| **Memory Target** | ~100MB | All services combined |

*Targets based on similar production systems and architectural design. Actual performance will be measured during implementation.*

---

## 🗺️ Roadmap

### Sprint 1: Foundation (✅ Complete)
- [x] Architecture design and specification
- [x] Complete documentation suite
- [x] Virtual queue system design
- [x] Dependencies audit and version matrix
- [x] CI/CD workflow specifications
- [x] Security guidelines and Tor best practices

### Sprint 2: Core Implementation (⏳ Not Started)
- [ ] Project structure setup (create actual folders)
- [ ] Deployment scripts (`cerberus.sh`)
- [ ] HAProxy configuration with circuit tracking
- [ ] Nginx static CAPTCHA gate
- [ ] Fortify basic CAPTCHA system (Rust)
- [ ] Integration testing

### Sprint 2: Intelligence
- [ ] Persistent circuit reputation database (SQLite)
- [ ] Behavioral analysis (timing patterns, endpoint diversity)
- [ ] Adaptive CAPTCHA difficulty
- [ ] Admin dashboard (web UI for monitoring)

### Sprint 3: Advanced Defense
- [ ] Machine learning anomaly detection
- [ ] Circuit clustering (Sybil attack detection)
- [ ] Swarm coordination (multi-node Cerberus)
- [ ] hCaptcha/reCAPTCHA integration (optional)

### Sprint 4: Production Readiness
- [ ] Security audit (external firm)
- [ ] Load testing (10k+ concurrent connections)
- [ ] Docker/Kubernetes deployment
- [ ] Ansible playbooks for automated deployment

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
