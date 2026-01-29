# 0107 - Protocol Normalization Layer

## Purpose
Normalize HTTP requests to kill edge-case abuse, prevent parser differentials, and enforce canonical forms before requests reach Nginx or Fortify.

## Policy
- Strip duplicate headers
- Normalize request paths
- Enforce canonical Host
- Enforce CRLF correctness

## User Stories
- As an operator, I want to prevent edge-case attacks that exploit parser differences.
- As a defender, I want to enforce strict protocol discipline without breaking Tor users.
- As a developer, I want normalization to be maintainable and auditable.

## Implementation Options
### Option 1: HAProxy/Nginx Config
- Use built-in config to strip/normalize headers and paths
- Simple, no extra daemons

### Option 2: Minimal Rust Filter
- Insert a Rust microservice for deep normalization
- Full control, future extensibility

## References
- [HTTP normalization](https://portswigger.net/web-security/http/request-smuggling)
- [Nginx header handling](https://nginx.org/en/docs/http/ngx_http_core_module.html)

---

> Both options are shown; choose based on operational and security needs.
