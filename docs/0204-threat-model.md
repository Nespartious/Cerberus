# 0204 - Cerberus Threat Model

## Practical Format
### Assets
- Backend onion service availability
- Host CPU & memory
- Tor introduction capacity
- Human user access

### Adversaries
- Script kiddies (flooders)
- Botnet operators (relay churn, CAPTCHA farms)
- Tor-aware attackers (circuit churn, timing)
- Well-funded adversaries (distributed, persistent)

### Trust Boundaries
- Untrusted → XDP → TC eBPF → Kernel TCP → HAProxy → Nginx → Fortify → Backend

### Design Invariants
- No layer assumes previous layer succeeded
- Each layer validates, caps resources, fails closed

---

## STRIDE Format
### S: Spoofing
- Not possible (Tor hides client IP)
### T: Tampering
- Packet/flow tampering mitigated by XDP/TC eBPF
### R: Repudiation
- No client identity; rely on relay/circuit behavior
### I: Information Disclosure
- No logging/fingerprinting; privacy by design
### D: Denial of Service
- Multi-layer defense, cost asymmetry, resource caps
### E: Elevation of Privilege
- No trust escalation between layers; strict boundaries

---

> Both practical and STRIDE threat models are provided for your review.
