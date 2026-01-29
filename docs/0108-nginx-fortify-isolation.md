# 0108 - Nginx ↔ Fortify Isolation & Backpressure

## Purpose
Ensure that failures or stalls in Fortify do not cascade to Nginx or the rest of the stack. Provide hard isolation, strict timeouts, memory caps, and queue governance.

## Policy
- Use UNIX socket for Nginx ↔ Fortify communication
- Enforce strict request timeouts
- Cap memory usage for Fortify
- Implement fixed-size request queue with drop-on-full
- Provide backpressure signaling to Nginx

## User Stories
- As an operator, I want Nginx to survive even if Fortify fails or stalls.
- As a defender, I want to prevent Fortify from becoming a bottleneck under siege.
- As a developer, I want clear boundaries and resource caps between layers.

## References
- [Nginx proxy settings](https://nginx.org/en/docs/http/ngx_http_proxy_module.html)
- [UNIX sockets vs TCP](https://www.nginx.com/blog/tuning-nginx/)

---

> This doc covers both isolation and queue governance for robust fail safety.
