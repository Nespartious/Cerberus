# 0106 - HAProxy HTTP Gate & Protocol Correctness

## Purpose
Add a lightweight HTTP-mode gate between HAProxy (TCP) and Nginx to enforce protocol correctness, reject malformed requests, and reduce memory pressure on Nginx.

## Policy
- Enforce HTTP method correctness
- Enforce header count/size limits
- Reject malformed or non-canonical requests
- Strip duplicate headers
- Enforce canonical Host and CRLF

## User Stories
- As an operator, I want malformed HTTP to be rejected before Nginx sees it.
- As a defender, I want to minimize Nginx memory usage under attack.
- As a developer, I want protocol normalization to be cost-effective and Tor-safe.

## Implementation Options
### Option 1: HAProxy HTTP-mode
- Use HAProxy’s built-in HTTP rules for header/method enforcement
- Fast, low-overhead, easy to maintain

### Option 2: Minimal Rust Filter
- Insert a Rust microservice between HAProxy and Nginx
- Full control over normalization logic
- Can be extended for future protocol edge cases

## References
- [HAProxy HTTP rules](https://www.haproxy.com/documentation/hapee/latest/configuration/basics/http/)
- [HTTP normalization best practices](https://portswigger.net/web-security/http/request-smuggling)

---

> Both options are valid; choose based on operational needs and future extensibility.
