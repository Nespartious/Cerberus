# Dependencies Audit & Version Matrix

## 📖 User Story

```
As a developer setting up a deployment environment
I want to know exact versions of all dependencies and their compatibility
So that I avoid version conflicts and security vulnerabilities

Acceptance Criteria:
- Complete version matrix for Tor, HAProxy, Nginx, Rust, and all crates
- Compatibility notes for Ubuntu 22.04/24.04 and Debian 11/12
- Security update schedule and EOL dates documented
- Installation commands with version pinning
- Known breaking changes between major versions noted
```

---

## Document Purpose
This document tracks all dependencies required for Cerberus deployment, their recommended versions, compatibility notes, and future considerations. We prioritize **latest stable** releases unless newer versions offer groundbreaking features critical to Tor defense.

**Last Updated**: January 28, 2026  
**Next Review**: February 28, 2026 (monthly)

---

## Core System Dependencies

### Operating System

| Dependency | Recommended Version | Minimum Version | Notes |
|------------|-------------------|-----------------|-------|
| **Ubuntu** | 24.04 LTS (Noble) | 22.04 LTS | LTS releases only for stability |
| **Debian** | 12 (Bookworm) | 11 (Bullseye) | Stable branch preferred |
| **Kernel** | 6.5+ | 5.15+ | Newer kernels have better TCP/IP stack performance |

**Recommendation**: **Ubuntu 24.04 LTS** for production (5-year support, latest stable packages)

---

## Network Stack

### 1. Tor

| Version | Status | Release Date | Notes |
|---------|--------|--------------|-------|
| **0.4.8.13** | **Recommended (Stable)** | Jan 2026 | Full PoW support, circuit tracking stable |
| 0.4.9.x | Latest (Alpha) | Ongoing | New features but potentially unstable |
| 0.4.7.x | EOL | 2024 | Missing PoW improvements |

**Recommendation**: **Tor 0.4.8.13** (latest stable)

**Critical Features Required**:
- `HiddenServicePoWDefensesEnabled` (0.4.8+)
- `HiddenServiceExportCircuitID` (0.4.6+)
- V3 Onion Support (0.3.5+)

**Installation**:
```bash
# Add Tor repository (Ubuntu/Debian)
sudo apt install apt-transport-https
echo "deb [signed-by=/usr/share/keyrings/tor-archive-keyring.gpg] https://deb.torproject.org/torproject.org $(lsb_release -cs) main" | sudo tee /etc/apt/sources.list.d/tor.list
wget -qO- https://deb.torproject.org/torproject.org/A3C4F0F979CAA22CDBA8F512EE8CBC9E886DDD89.asc | gpg --dearmor | sudo tee /usr/share/keyrings/tor-archive-keyring.gpg
sudo apt update
sudo apt install tor=0.4.8.13-1
```

**Breaking Changes**: None between 0.4.8.x releases

---

### 2. Vanguards

| Version | Status | Release Date | Notes |
|---------|--------|--------------|-------|
| **0.3.1** | **Recommended** | 2024 | Stable guard protection |
| 0.4.0-alpha | Alpha | 2025 | Experimental features |

**Recommendation**: **Vanguards 0.3.1** (latest stable)

**Installation**:
```bash
sudo apt install python3-pip
sudo pip3 install vanguards==0.3.1
```

**Purpose**: Prevents guard discovery and circuit correlation attacks

---

## Layer 1: HAProxy

### Version Analysis

| Version | Status | Release Date | Key Features | Notes |
|---------|--------|--------------|--------------|-------|
| **2.8.11 LTS** | **Recommended** | Jan 2026 | Stick tables, Lua scripting, PROXY protocol v2 | Stable, 5-year support |
| 3.0.7 | Latest Stable | Dec 2025 | HTTP/3, QUIC support | Breaking config changes from 2.x |
| 2.6.x | EOL | 2024 | - | Security updates ended |

**Recommendation**: **HAProxy 2.8.11 LTS** (latest stable LTS)

**Why Not 3.0?**
- Breaking changes in config syntax
- HTTP/3 not needed for Tor (Tor uses HTTP/1.1)
- 2.8 LTS has longer support window (until 2029)

**Installation**:
```bash
sudo apt install software-properties-common
sudo add-apt-repository ppa:vbernat/haproxy-2.8
sudo apt update
sudo apt install haproxy=2.8.11-1ppa1~$(lsb_release -cs)
```

**Critical Features Used**:
- Stick tables (circuit reputation tracking)
- PROXY protocol v2 (Circuit ID extraction)
- Lua scripting (queue token validation)
- ACLs and conditionals (ban/VIP logic)

**Breaking Changes**: None within 2.8.x series

---

## Layer 2: Nginx

### Version Analysis

| Version | Status | Release Date | Key Features | Notes |
|---------|--------|--------------|--------------|-------|
| **1.26.2 Mainline** | **Recommended** | Jan 2026 | Latest features, active development | Stable enough for production |
| 1.24.0 Stable | Stable | Apr 2023 | Conservative, long-term stable | Missing recent security patches |
| 1.22.x | EOL | 2023 | - | No longer maintained |

**Recommendation**: **Nginx 1.26.2 Mainline** (latest stable mainline)

**Why Mainline Over Stable?**
- Mainline gets security fixes faster
- "Stable" branch only gets critical patches
- Mainline is production-ready (used by Cloudflare, etc.)

**Installation**:
```bash
# Official Nginx repository
sudo apt install curl gnupg2 ca-certificates lsb-release ubuntu-keyring
curl https://nginx.org/keys/nginx_signing.key | gpg --dearmor | sudo tee /usr/share/keyrings/nginx-archive-keyring.gpg
echo "deb [signed-by=/usr/share/keyrings/nginx-archive-keyring.gpg] http://nginx.org/packages/mainline/ubuntu $(lsb_release -cs) nginx" | sudo tee /etc/apt/sources.list.d/nginx.list
sudo apt update
sudo apt install nginx=1.26.2-1~$(lsb_release -cs)
```

**Critical Features Used**:
- `proxy_pass` (backend forwarding)
- `limit_req` (rate limiting)
- Buffer controls (`client_body_buffer_size`, etc.)
- Header manipulation (`proxy_set_header`)

**Breaking Changes**: None within 1.26.x series

---

## Layer 3: Fortify (Rust)

### Rust Toolchain

| Component | Recommended Version | Minimum Version | Notes |
|-----------|-------------------|-----------------|-------|
| **Rust** | 1.82.0 | 1.75.0 | Use rustup for management |
| **Cargo** | 1.82.0 | 1.75.0 | Bundled with Rust |
| **Edition** | 2021 | 2021 | Latest stable edition |

**Recommendation**: **Rust 1.82.0** (latest stable)

**Why Latest Rust?**
- Security fixes (memory safety improvements)
- Performance improvements (LLVM backend updates)
- Better async runtime optimizations (critical for Tokio)
- Improved compile times

**Installation**:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default 1.82.0
rustup update
```

**No Breaking Changes**: Rust follows semantic versioning strictly

---

### Rust Crates (Dependencies)

#### Core Async Runtime

| Crate | Recommended Version | Purpose | Notes |
|-------|-------------------|---------|-------|
| **tokio** | 1.41.1 | Async runtime | Full features: `["full"]` |
| **tokio-util** | 0.7.12 | Tokio utilities | For codec/framing |

**Why Tokio?**
- Industry standard for async Rust
- Excellent performance (used by Discord, AWS, Cloudflare)
- Active maintenance

---

#### Web Framework

| Crate | Recommended Version | Purpose | Notes |
|-------|-------------------|---------|-------|
| **axum** | 0.7.7 | HTTP server framework | Built on Tokio, Tower |
| **tower** | 0.5.1 | Middleware framework | Required by Axum |
| **tower-http** | 0.6.2 | HTTP middleware | CORS, compression, etc. |
| **hyper** | 1.5.1 | HTTP implementation | Backend for Axum |

**Why Axum?**
- Minimal boilerplate
- Type-safe extractors
- Excellent error handling
- Built by Tokio team (same maintainers)

**Alternative Considered**: Actix-web (rejected: more complex, older async model)

---

#### CAPTCHA Generation

| Crate | Recommended Version | Purpose | Notes |
|-------|-------------------|---------|-------|
| **captcha** | 0.0.9 | Image CAPTCHA generation | Lightweight, no external deps |
| **image** | 0.25.5 | Image manipulation | Required by captcha crate |
| **imageproc** | 0.25.0 | Image processing | Distortion effects |

**Why `captcha` crate?**
- Pure Rust (no C bindings)
- No external dependencies (no ImageMagick, etc.)
- Customizable difficulty

**Future Enhancement**: Consider `hcaptcha-rs` for hCaptcha integration (post-MVP)

---

#### Serialization

| Crate | Recommended Version | Purpose | Notes |
|-------|-------------------|---------|-------|
| **serde** | 1.0.215 | Serialization framework | Core serialization |
| **serde_json** | 1.0.133 | JSON support | API requests/responses |
| **toml** | 0.8.19 | TOML config parsing | Read `fortify.toml` |

**Standard**: Serde is the de facto standard for Rust serialization

---

#### Cryptography

| Crate | Recommended Version | Purpose | Notes |
|-------|-------------------|---------|-------|
| **sha2** | 0.10.8 | SHA-256 hashing | Queue token signatures |
| **hmac** | 0.12.1 | HMAC implementation | Token HMAC signing |
| **rand** | 0.8.5 | Random number generation | CAPTCHA challenges |
| **subtle** | 2.6.1 | Constant-time operations | Prevent timing attacks |

**Why These?**
- `sha2`, `hmac`: RustCrypto organization (audited, widely used)
- `rand`: Standard RNG library
- `subtle`: Constant-time comparisons (critical for security)

---

#### Utilities

| Crate | Recommended Version | Purpose | Notes |
|-------|-------------------|---------|-------|
| **tracing** | 0.1.41 | Structured logging | Better than `log` crate |
| **tracing-subscriber** | 0.3.19 | Log formatting | JSON/pretty output |
| **anyhow** | 1.0.93 | Error handling | Simplified error propagation |
| **thiserror** | 2.0.3 | Custom error types | Library error definitions |

**Why Tracing?**
- Async-aware (thread-safe logging)
- Structured output (JSON for production)
- Performance instrumentation

---

### Cargo.toml (Complete)

```toml
[package]
name = "fortify"
version = "0.1.0"
edition = "2021"
rust-version = "1.75.0"

[dependencies]
# Async runtime
tokio = { version = "1.41.1", features = ["full"] }
tokio-util = "0.7.12"

# Web framework
axum = "0.7.7"
tower = "0.5.1"
tower-http = { version = "0.6.2", features = ["cors", "compression-gzip"] }
hyper = "1.5.1"

# CAPTCHA
captcha = "0.0.9"
image = "0.25.5"
imageproc = "0.25.0"

# Serialization
serde = { version = "1.0.215", features = ["derive"] }
serde_json = "1.0.133"
toml = "0.8.19"

# Cryptography
sha2 = "0.10.8"
hmac = "0.12.1"
rand = "0.8.5"
subtle = "2.6.1"

# Utilities
tracing = "0.1.41"
tracing-subscriber = { version = "0.3.19", features = ["json", "env-filter"] }
anyhow = "1.0.93"
thiserror = "2.0.3"

[dev-dependencies]
# Testing
axum-test = "16.6.0"
mockall = "0.13.1"

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true  # Remove debug symbols
panic = "abort"  # Smaller binary
```

---

## Supporting Tools

### Development Tools

| Tool | Recommended Version | Purpose | Installation |
|------|-------------------|---------|--------------|
| **rustfmt** | 1.82.0 | Code formatting | `rustup component add rustfmt` |
| **clippy** | 1.82.0 | Linter | `rustup component add clippy` |
| **cargo-audit** | 0.21.0 | Security audit | `cargo install cargo-audit` |
| **cargo-outdated** | 0.15.0 | Check outdated deps | `cargo install cargo-outdated` |

### System Utilities

| Tool | Recommended Version | Purpose | Installation |
|------|-------------------|---------|--------------|
| **curl** | 8.5+ | HTTP testing | `sudo apt install curl` |
| **socat** | 1.7+ | Unix socket interaction | `sudo apt install socat` |
| **jq** | 1.6+ | JSON parsing | `sudo apt install jq` |
| **shellcheck** | 0.10+ | Shell script linting | `sudo apt install shellcheck` |

---

## Docker Dependencies (Optional)

### Docker Engine

| Component | Recommended Version | Minimum Version | Notes |
|-----------|-------------------|-----------------|-------|
| **Docker** | 27.4.1 | 24.0.0 | Latest stable |
| **Docker Compose** | 2.30.3 | 2.20.0 | V2 required (Python version deprecated) |

**Installation**:
```bash
# Docker official repository
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo gpg --dearmor -o /usr/share/keyrings/docker-archive-keyring.gpg
echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/docker-archive-keyring.gpg] https://download.docker.com/linux/ubuntu $(lsb_release -cs) stable" | sudo tee /etc/apt/sources.list.d/docker.list
sudo apt update
sudo apt install docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
```

### Base Images

| Image | Recommended Tag | Purpose | Notes |
|-------|----------------|---------|-------|
| **Ubuntu** | 24.04 | Cerberus services | Official image, LTS |
| **Rust** | 1.82-slim | Fortify build | Official Rust image |
| **Alpine** | 3.21 | Lightweight alternative | Smaller size, but compatibility issues |

**Recommendation**: **Ubuntu 24.04** for Docker base (consistency with bare-metal)

---

## Compatibility Matrix

### Cross-Dependency Conflicts

| Component A | Version | Component B | Version | Conflict? | Resolution |
|-------------|---------|-------------|---------|-----------|------------|
| HAProxy 2.8 | 2.8.11 | Tor 0.4.8 | 0.4.8.13 | ✅ Compatible | PROXY protocol v2 supported |
| Nginx 1.26 | 1.26.2 | HAProxy 2.8 | 2.8.11 | ✅ Compatible | Standard HTTP backend |
| Tokio 1.41 | 1.41.1 | Axum 0.7 | 0.7.7 | ✅ Compatible | Axum built for Tokio 1.x |
| Rust 1.82 | 1.82.0 | All crates | (see above) | ✅ Compatible | All crates support 1.75+ |

**No Known Conflicts** ✅

---

## Update Schedule

### Critical Security Updates (Immediate)
- Tor security releases
- Rust CVE patches
- HAProxy/Nginx security advisories

**Monitor**:
- https://blog.torproject.org/
- https://blog.rust-lang.org/
- https://www.haproxy.com/blog/tag/release/
- https://nginx.org/en/security_advisories.html

### Routine Updates (Monthly)
- Rust crate updates (`cargo update`)
- Minor version bumps (HAProxy 2.8.x, Nginx 1.26.x)

### Major Version Updates (Quarterly Review)
- Tor 0.4.9 (when stable)
- HAProxy 3.0 (evaluate breaking changes)
- Nginx 1.28+ (when available)

---

## Future Dependencies (Planned)

### Sprint 2+

| Dependency | Version | Purpose | Priority |
|------------|---------|---------|----------|
| **Redis** | 7.4+ | Persistent circuit reputation | High |
| **SQLite** | 3.46+ | Session storage, circuit history | Medium |
| **Prometheus Client** | 0.13+ | Metrics export | Medium |
| **syslog-ng** | 4.8+ | Centralized logging | Low |

### Groundbreaking Features to Watch

1. **Tor 0.4.9 Congestion Control**: Improved onion service performance (monitor for stability)
2. **HAProxy 3.0 HTTP/3**: Not needed for Tor, but future-proofing for clearnet gateways
3. **Rust 1.85+ const trait impl**: Better compile-time optimizations

---

## Dependency Maintenance Commands

### Check for Outdated Dependencies

```bash
# Rust crates
cd keeper
cargo outdated

# System packages
sudo apt list --upgradable
```

### Security Audits

```bash
# Rust dependencies
cargo audit

# System CVEs
sudo apt install debian-security-support
check-security-support
```

### Update All Dependencies

```bash
# Rust (conservative, patch only)
cargo update

# Rust (aggressive, minor versions)
cargo update --aggressive

# System packages
sudo apt update && sudo apt upgrade
```

---

## Conflict Resolution Guide

### If HAProxy Update Breaks Config
1. Check release notes: `https://www.haproxy.com/blog/haproxy-2-8-11-released/`
2. Review breaking changes in config syntax
3. Test in Docker first before bare-metal
4. Keep previous version in apt cache: `sudo apt install haproxy=<old-version>`

### If Rust Crate Update Breaks Build
1. Pin problematic crate: `tokio = "=1.41.1"` (exact version)
2. Check crate changelog: `https://github.com/tokio-rs/tokio/releases`
3. File issue with crate maintainer if bug
4. Use `cargo tree` to identify dependency conflicts

### If Tor Update Changes Behavior
1. Test onion service connectivity after update
2. Check Circuit ID extraction still works (HAProxy logs)
3. Verify PoW defenses still active: `tor --verify-config`
4. Rollback if critical: `sudo apt install tor=<old-version>`

---

## Verification Checklist

After updating dependencies:

- [ ] All services start without errors
- [ ] Tor generates onion address
- [ ] HAProxy extracts Circuit IDs (check logs)
- [ ] Nginx forwards requests to Fortify
- [ ] Fortify handles CAPTCHA generation/verification
- [ ] Virtual queue page loads correctly
- [ ] Circuit banning works (test with curl loop)
- [ ] VIP promotion works (test CAPTCHA flow)
- [ ] No performance regression (benchmark before/after)

---

## Summary

**Current Stable Stack**:
- Ubuntu 24.04 LTS
- Tor 0.4.8.13
- Vanguards 0.3.1
- HAProxy 2.8.11 LTS
- Nginx 1.26.2 Mainline
- Rust 1.82.0 + Latest stable crates

**Update Policy**: Monthly patch updates, quarterly major version reviews, immediate security patches.

**No Conflicts**: All components tested and compatible ✅
