Project Cerberus: High-Assurance Tor Ingress Architecture

## 📖 User Story

```
As a new contributor
I want to understand Cerberus' three-layer defense architecture and design philosophy
So that I can make informed decisions that align with the project's goals

Acceptance Criteria:
- Explains why three layers (HAProxy/Nginx/Fortify) vs single WAF
- Documents circuit-based tracking vs IP-based approach
- Clarifies zero-JavaScript requirement and Tor-specific constraints
- Provides rationale for technology choices (Rust, HAProxy 2.8, etc.)
```

---

1. Executive Summary
Cerberus is a specialized, multi-tiered ingress defense system designed exclusively for Tor Onion Services. Unlike standard web stacks, Cerberus assumes a hostile environment (DDoS, Deanonymization, Floods) by default. It utilizes a "Defense in Depth" strategy, separating connection management (L4) from protocol sanitization (L7) and business logic.

2. Core Architecture
Traffic Flow: The Internet -> Tor Network -> Cerberus Node

Layer 0: Tor Daemon & OS Hardening

Role: Network Transport.

Features: HiddenServicePoWDefensesEnabled (Proof of Work), Vanguards (Anti-Sybil/Deanonymization), Custom Firewall (NFTables).

Layer 1: HAProxy (The Shield)

Role: Connection & Queue Management.

Input: Raw TCP via Tor PROXY protocol (includes Circuit IDs).

Responsibilities:

Global Connection Limits (maxconn).

Stick Tables (tracking reputation by Circuit ID).

Aggressive Queuing (FIFO or Priority).

DDoS mitigation (dropping abusive circuits instantly).

Layer 2: Nginx (The Filter)

Role: Protocol Sanitization & Static Defense.

Input: HTTP traffic from HAProxy.

Responsibilities:

Header Scrubbing (removing fingerprintable metadata).

Buffer Management (Stop Slowloris/Large Payload attacks).

Static Gate: Serves the captcha.html directly (offloading the app).

Layer 3: The Keeper (Rust Application)

Role: Logic & Verification.

Input: Only valid POST requests containing CAPTCHA solutions.

Responsibilities:

Validates Tokens/CAPTCHAs.

Updates HAProxy Stick Tables (promoting users to VIP).

Manages the "Swarm" state.

3. The "Cerberus Script" (Deployment)
A single, user-configurable bash script (cerberus.sh) that acts as the orchestrator.

User Config: cerberus.conf (Target Onion, Ports, DDoS sensitivity levels).

Functions: install_deps, harden_kernel, config_tor, build_rust, audit_system.

Phase 3: Sprint 1 (The Foundation)
Goal: Establish the "Plumbing." Get traffic flowing from Tor -> HAProxy -> Nginx -> Target. No Rust logic yet. Just pure infrastructure stability with Tor defenses enabled.

Sprint 1 Breakdown:

Step 1: The "Cerberus" Deployer Skeleton
Create cerberus.sh.

Feature: It must detect the OS (Debian/Ubuntu/Alpine) and install: tor, haproxy, nginx, python3-pip, git.

Feature: It must install vanguards via pip.

Feature: Create the directory structure: /etc/cerberus/{tor,haproxy,nginx}.

Step 2: Tor Configuration (The Moat)
Generate a torrc that creates a Hidden Service.

Critical Settings:

HiddenServicePort 80 127.0.0.1:10000 (Forward to HAProxy).

HiddenServiceExportCircuitID haproxy (Crucial for tracking users).

HiddenServicePoWDefensesEnabled 1 (Native Tor Anti-DDoS).

HiddenServicePoWQueueRate 50 & HiddenServicePoWQueueBurst 100.

Step 3: HAProxy Configuration (The Guard)
Configure HAProxy to listen on 127.0.0.1:10000.

Critical Settings:

accept-proxy (To read the Circuit ID from Tor).

Define a basic stick-table (even if unused yet) to prove we can track IDs.

Set maxconn 500 (Artificially low to test queuing).

Forward traffic to 127.0.0.1:10001 (Nginx).

Step 4: Nginx Configuration (The Proxy)
Configure Nginx to listen on 127.0.0.1:10001.

Critical Settings:

client_body_timeout 5s; (Kill slow connections).

client_header_timeout 5s;

Target: proxy_pass http://YOUR_REAL_ONION_OR_IP;

Note: Since you mentioned proxying to another onion service, Nginx cannot natively resolve .onion addresses.

Workaround for Sprint 1: We will proxy to a "Mock Target" (a simple Python http.server running on port 8080) to prove the pipeline works. Proxying to an external onion requires a SOCKS5 tunnel which is a Sprint 2 task.

Step 5: Verification
Run the stack.

Connect via Tor Browser.

Success Metric: You see the "Mock Target" page.

Audit: Check HAProxy logs. Do we see 127.0.0.1 as the source, or do we see the Tor Circuit IDs? (We must confirm we are seeing Circuit IDs).
