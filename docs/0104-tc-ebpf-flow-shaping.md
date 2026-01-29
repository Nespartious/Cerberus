# 0104 - TC eBPF Flow Shaping for Tor Relays

## Purpose
Add a stateful, relay-aware traffic shaping layer between XDP and HAProxy. TC eBPF introduces cost and friction for abusive Tor relays, slows attackers, and signals HAProxy for adaptive escalation.

## Policy (Conceptual)
- Monitor per-relay IP: packets/sec, flows/sec, SYN rate, avg flow duration, retransmits, concurrent flows
- Base state: allow, minimal shaping, collect metrics
- Suspicion: add 50–200ms latency, cap new flows/sec, jitter
- Hostile: heavy delay (500ms–2s), hard cap concurrent flows, probabilistic drops (30–60%)
- Extreme: temporary relay quarantine, drop new SYNs for TTL
- Set skb marks for HAProxy to read and escalate PoW/CAPTCHA

## User Stories
- As an operator, I want abusive relays slowed or penalized, not just dropped, so attacks become expensive.
- As a defender, I want to see relay flow stats and penalties in real time.
- As a developer, I want cross-layer signaling (skb marks) to inform HAProxy of relay state.

## References
- [Linux TC eBPF docs](https://www.kernel.org/doc/html/latest/networking/filter.html)
- [eBPF Traffic Control](https://ebpf.io/what-is-ebpf/#traffic-control)
- [Tor relay consensus](https://consensus-health.torproject.org/)

---

> This doc is policy/logic/user stories only. Example code can be added later if needed.
