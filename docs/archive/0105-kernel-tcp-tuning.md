# 0105 - Kernel TCP Tuning for Tor Defense

## Purpose
Harden the Linux TCP stack to resist connection abuse, SYN floods, and resource exhaustion before traffic reaches HAProxy.

## Policy (Conceptual)
- Enable SYN cookies
- Cap TCP backlog and listen queues
- Aggressive FIN/RST cleanup
- Lower TIME_WAIT reuse thresholds
- Reduce TCP retry windows
- Shorten idle timeouts

## User Stories
- As an operator, I want the kernel to drop abusive connections before they reach userland.
- As a defender, I want to minimize half-open sockets and resource exhaustion.
- As a developer, I want to tune TCP policy for Tor’s unique traffic patterns.

## References
- [Linux TCP tuning](https://wiki.nikhef.nl/grid/Linux_TCP_tuning)
- [SYN cookies](https://lwn.net/Articles/277146/)
- [Tor scaling best practices](https://community.torproject.org/relay/setup/guard/)

---

> This doc is conceptual policy only. Sysctl examples can be added later if needed.
