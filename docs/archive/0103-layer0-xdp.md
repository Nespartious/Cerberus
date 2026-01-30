# 0103 - Layer 0: XDP/eBPF Flood Shield

## Purpose

Provide the first line of defense against volumetric attacks and raw packet floods targeting the Tor onion service host. XDP (eXpress Data Path) and eBPF run in the NIC driver context, allowing Cerberus to drop malicious or excessive packets before they reach the kernel TCP stack or any userland service.

## Why XDP?
- **Survives raw packet floods** (SYN, UDP, malformed, etc.)
- **Protects HAProxy and kernel** from socket exhaustion
- **Ultra-low latency**: runs before kernel allocates memory
- **Relay-aware**: can rate-limit per Tor relay IP

## Key Functions
- Drop packets exceeding per-relay IP rate limits
- Detect and block SYN floods, malformed packets, and fragment abuse
- Maintain eBPF maps for relay reputation and packet statistics
- Provide basic metrics for Prometheus (via eBPF exporter)

## Implementation Plan

### Phase 1.5: Minimal XDP Flood Shield
- [ ] Write a simple XDP program to drop packets above a global PPS threshold
- [ ] Integrate eBPF map for per-IP packet counting
- [ ] Allowlist Tor relay IPs (from consensus data)
- [ ] Expose drop/accept counters via eBPF exporter

### Phase 2: Per-Relay Rate Limiting
- [ ] Implement per-relay IP rate limiting (configurable PPS)
- [ ] Add SYN flood detection and drop logic
- [ ] Detect and drop malformed/fragmented packets
- [ ] Add relay reputation scoring (temporary throttling for abusers)

### Phase 3: Observability & Tuning
- [ ] Integrate with Prometheus/Grafana for real-time metrics
- [ ] Provide CLI for live stats and dynamic tuning
- [ ] Document safe defaults and tuning strategies

## User Stories
- **As a Tor onion operator,** I want my service to stay online during raw packet floods, so that legitimate users can still connect.
- **As a Cerberus admin,** I want to see real-time packet drop/accept stats, so I can tune thresholds and detect attacks early.
- **As a security engineer,** I want to block only abusive Tor relays, not all traffic, so that good relays are never penalized.

## References
- [XDP Project](https://xdp-project.net/)
- [eBPF Documentation](https://ebpf.io/)
- [Prometheus eBPF Exporter](https://github.com/cloudflare/ebpf_exporter)
- [Tor Relay Consensus Data](https://consensus-health.torproject.org/)

---

> **Planner Note:**
> This document is written per the Cerberus SUDO rules: clear phases, user stories, actionable steps, and references. XDP/eBPF is now a first-class layer in the Cerberus defense stack.
