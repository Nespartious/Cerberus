# 🤖 AI Agent Directives (PRIME DIRECTIVES)

> **CRITICAL:** You must read and follow these rules before performing any task in this repository.

## 1. The Single Source of Truth
- **`docs/Project_Outline_R0.md`** is the **Bible** for this project.
- It contains the Master Architecture, Configs, and Implementation Details.
- **Do NOT** create new planning documents (e.g., `01xx-concept.md`). Update `Project_Outline_R0.md` or write actual code.
- **Archive:** Old planning docs are in `docs/archive/`. Read them for context if needed, but treat them as read-only history.

## 2. The Iron Laws of Cerberus
1.  **NO JavaScript:** The user-facing defense stack (Nginx/Fortify) must work 100% without JS.
    *   Exception: Admin Monitoring UI (Grafana) *can* use JS, as it's for operators, not Tor users.
2.  **Tor-Native:** All design decisions must assume:
    *   High Latency (500ms+ RTT).
    *   No stable Client IP (Circuit ID is ephemeral).
    *   Anonymity is paramount (No tracking pixels, no external CDNs).
3.  **Fail-Closed:** If a security component (XDP, HAProxy, Fortify) fails, traffic MUST stop. Never fail-open.
4.  **Privacy First:** Zero logging of traffic content. Log only metrics (counters, rates) or hashed identifiers.

## 3. Coding Standards
### Rust (Fortify)
- **Async Runtime:** Use `tokio` for everything.
- **Error Handling:** Use `thiserror` for libs and `anyhow` for binaries. **No `unwrap()`** in logic paths.
- **Serialization:** Use `serde` / `serde_json`.
- **Logging:** Use `tracing` (structured logging).

### C (XDP/eBPF)
- **Verifier Safe:** All loops must be bounded (`#pragma unroll`).
- **Memory:** Strict stack limits (512 bytes). Use Per-CPU Maps for high-volume counters.

### Configuration
- **Hardening:** All configs (Nginx, HAProxy, Sysctl) must be tuned for *hostile* environments (aggressive timeouts, resource caps).

---

# Cerberus Development & Security Instructions

## 📖 User Story

```
As an AI agent or developer
I want comprehensive security guidelines and Tor-specific gotchas
So that I avoid common pitfalls that could compromise user anonymity or security

Acceptance Criteria:
- Documents 25+ Tor/security gotchas with code examples
- Explains timing attacks, circuit correlation, JavaScript risks
- Provides defensive programming patterns (constant-time comparisons)
- Includes role definitions and user stories for guidance
```

---

## Document Purpose
This document serves as a comprehensive guide for developing and maintaining Cerberus, a high-assurance Tor Onion Service defense system. It contains critical security considerations, Tor-specific gotchas, and development workflow best practices. This content will inform future AI agent instructions and human developer onboarding.

---

## Table of Contents
1. [Tor Network Fundamentals & Gotchas](#tor-network-fundamentals--gotchas)
2. [Tor Browser Security Considerations](#tor-browser-security-considerations)
3. [JavaScript: The Double-Edged Sword](#javascript-the-double-edged-sword)
4. [Onion Service Specific Concerns](#onion-service-specific-concerns)
5. [Project Roles & Responsibilities](#project-roles--responsibilities)
6. [User Stories](#user-stories)
7. [Development Workflow & Best Practices](#development-workflow--best-practices)
8. [Testing in Tor Environments](#testing-in-tor-environments)
9. [Security Audit Checklist](#security-audit-checklist)
10. [Common Pitfalls & Mistakes](#common-pitfalls--mistakes)
11. [Defensive Programming Principles](#defensive-programming-principles)

---

## Tor Network Fundamentals & Gotchas

### Circuit Behavior & Limitations

**GOTCHA #1: Circuit IDs Are NOT IP Addresses**
- **Problem**: You cannot treat Circuit IDs like traditional client IPs
- **Why**: Multiple users may share the same exit/guard nodes, circuits rotate every 10 minutes
- **Impact**: Per-"user" rate limiting must account for circuit churn
- **Solution**: Track circuits with awareness that legitimate users will have multiple IDs over time
- **Code Implication**: Don't permanently ban circuits; use time-limited bans (30min-1hr)

```rust
// BAD: Permanent ban
if is_malicious(circuit_id) {
    ban_forever(circuit_id);  // User will just get a new circuit
}

// GOOD: Time-limited ban
if is_malicious(circuit_id) {
    ban_for_duration(circuit_id, Duration::from_secs(1800));  // 30 min
}
```

**GOTCHA #2: Circuit Rotation = Session Loss**
- **Problem**: Tor Browser rotates circuits every 10 minutes by default
- **Why**: Privacy preservation (prevent long-term tracking)
- **Impact**: Session cookies/tokens tied to Circuit IDs become invalid
- **Solution**: 
  - Use short-lived tokens (5-10 min max)
  - Implement graceful re-authentication (redirect to CAPTCHA, not error page)
  - Store session state server-side with circuit-independent identifiers

**GOTCHA #3: No Source IP = No Geo-Blocking**
- **Problem**: You cannot geo-block traffic (no real IPs visible)
- **Why**: All traffic appears as `127.0.0.1` from the Tor daemon
- **Impact**: Traditional IP-based WAFs and geo-filters are useless
- **Solution**: Behavior-based blocking only (rate limits, CAPTCHA difficulty, circuit reputation)

**GOTCHA #4: Onion Routing Adds Latency**
- **Problem**: 3-hop encryption adds 300-1000ms latency per request
- **Why**: Guard → Middle → Exit → Onion Service (4+ hops for hidden services)
- **Impact**: Timeouts must be generous (5s+ for HTTP requests)
- **Solution**: 
  - Set `client_header_timeout 5s` (not 2s)
  - Set `proxy_read_timeout 10s` in Nginx
  - Optimize for minimal round trips (inline CSS, no external resources)

### Timing Attacks & Deanonymization

**GOTCHA #5: Response Time = Information Leakage**
- **Problem**: Consistent response times can fingerprint services or reveal internal state
- **Why**: Attackers use timing analysis to correlate circuits or identify users
- **Impact**: Authentication failures, CAPTCHA results, database queries leak info
- **Solution**: 
  - Always use constant-time comparisons for secrets
  - Add random jitter to response times (±50-200ms)
  - Never reveal "user exists" vs "wrong password" timing differences

```rust
// BAD: Timing attack vulnerable
if user_exists(username) {
    if check_password(username, password) {  // Fast if user doesn't exist
        return success;
    }
}

// GOOD: Constant-time
let user = fetch_user(username);  // Always hit DB
let valid = constant_time_compare(user.password_hash, hash(password));
add_random_delay(50, 200);  // Jitter
return valid;
```

**GOTCHA #6: Circuit Correlation Attacks**
- **Problem**: Attackers can link multiple circuits to the same user
- **Why**: Behavioral fingerprinting (typing speed, request patterns, session timing)
- **Impact**: Deanonymization even through Tor
- **Solution**: 
  - Don't store unnecessary metadata (user-agent, language, timezone)
  - Normalize all responses (same headers for all users)
  - Avoid personalized content that could leak identity

### Traffic Analysis Vulnerabilities

**GOTCHA #7: Packet Size Fingerprinting**
- **Problem**: Response sizes can fingerprint content or user actions
- **Why**: "View profile" vs "Edit profile" have different HTML sizes
- **Impact**: Passive adversaries can infer user behavior
- **Solution**: 
  - Pad responses to fixed sizes (e.g., all HTML pages = 16KB blocks)
  - Use compression uniformly (gzip all responses)
  - Avoid revealing database query results via response size

**GOTCHA #8: Request Frequency Reveals Intent**
- **Problem**: Rapid polling = automated scraper, slow browsing = human
- **Why**: Timing patterns are distinctive
- **Impact**: False positives (block humans) or false negatives (allow bots)
- **Solution**: 
  - Use adaptive thresholds (spike detection, not absolute limits)
  - Combine multiple signals (rate + payload size + endpoint diversity)
  - Progressive challenges (easy CAPTCHA → harder if suspicious)

---

## Tor Browser Security Considerations

### Security Levels (SafeMode)

**CRITICAL: Default Assumption = Safest Mode**
- **Tor Browser "Safest" Settings**:
  - **JavaScript DISABLED globally**
  - **No plugins** (Flash, Java, etc.)
  - **No fonts** (except system fonts)
  - **No SVG** (can contain JS)
  - **No MathML**
  - **No media codecs** (H.264, etc.)

**GOTCHA #9: Your Site MUST Work Without JavaScript**
- **Problem**: Many modern frameworks require JS (React, Vue, Angular)
- **Why**: 50%+ of Tor users use Safest mode
- **Impact**: JS-dependent sites are unusable for high-security users
- **Solution**: 
  - Use server-side rendering (SSR) for all critical functionality
  - Progressive enhancement (site works without JS, better with JS)
  - No Single-Page Applications (SPAs) unless absolutely necessary

```html
<!-- BAD: JS-only CAPTCHA -->
<div id="captcha-root"></div>
<script>ReactDOM.render(<Captcha />, document.getElementById('captcha-root'));</script>

<!-- GOOD: HTML form with optional JS enhancement -->
<form action="/verify-captcha" method="POST">
    <img src="/api/captcha-image" alt="CAPTCHA" />
    <input type="text" name="solution" required />
    <button type="submit">Verify</button>
</form>
<script src="/js/captcha-enhance.js"></script>  <!-- Optional, enhances UX -->
```

**GOTCHA #10: Standard vs Safest Mode Detection**
- **Problem**: You can't reliably detect which security level users are on
- **Why**: Feature detection via JS won't work if JS is disabled
- **Impact**: Cannot adjust UX based on security level
- **Solution**: 
  - Design for Safest mode by default
  - Use `<noscript>` tags for fallback content
  - Server-side rendering for all primary flows

### Fingerprinting & Privacy

**GOTCHA #11: Tor Browser Homogenizes User-Agents**
- **Problem**: All Tor Browser users have identical `User-Agent` strings
- **Why**: Prevent browser fingerprinting
- **Impact**: You cannot detect bots via user-agent analysis
- **Solution**: 
  - Don't rely on `User-Agent` for anything security-critical
  - Use behavioral analysis instead (request patterns, timing)
  - Accept that some entropy is lost

**GOTCHA #12: No Persistent Storage Assumptions**
- **Problem**: Tor Browser clears all storage on exit (cookies, localStorage, IndexedDB)
- **Why**: Prevent cross-session tracking
- **Impact**: "Remember me" features don't work, sessions are short-lived
- **Solution**: 
  - Design for ephemeral sessions (10-30 min max)
  - Require re-authentication frequently
  - Don't assume users have cookies enabled

**GOTCHA #13: Font Enumeration = Fingerprinting Vector**
- **Problem**: CSS font detection can enumerate installed fonts
- **Why**: Unique font combinations = unique users
- **Impact**: Deanonymization via font fingerprinting
- **Solution**: 
  - Only use system-safe fonts (Arial, Times, Courier)
  - Never use web fonts (@font-face)
  - Set `font-family: sans-serif` (generic)

```css
/* BAD: Web fonts */
@import url('https://fonts.googleapis.com/css2?family=Roboto');
body { font-family: 'Roboto', sans-serif; }

/* GOOD: System fonts only */
body { font-family: sans-serif; }
code { font-family: monospace; }
```

---

## JavaScript: Prohibited for Security

### Zero-JavaScript Policy

**CRITICAL: Cerberus Does NOT Use JavaScript**

Unlike most web applications, Cerberus operates under a **zero-JavaScript policy**. This is not optional—it's a fundamental security requirement.

**Rationale**:
1. **Tor Browser Safest Mode**: 50%+ of Tor users disable JavaScript for maximum anonymity
2. **Attack Surface**: Every line of JS is a potential attack vector (fingerprinting, XSS, data exfiltration)
3. **Fingerprinting**: JavaScript enables canvas, WebGL, and timing attacks that deanonymize users
4. **Simplicity**: Server-side rendering is more secure and easier to audit

### Threat Model: Why No JavaScript?

**GOTCHA #14: JavaScript Can Deanonymize Users**
- **Attacks via JS**:
  - **Canvas Fingerprinting**: Render text/graphics, hash pixel output (unique per GPU/driver)
  - **WebGL Fingerprinting**: Query GPU info (vendor, renderer, extensions)
  - **AudioContext Fingerprinting**: Audio output characteristics
  - **Battery API**: Battery level/charging status (deprecated but still risky)
  - **Timing Attacks**: `performance.now()` for high-resolution timing
  - **WebRTC Leak**: Real IP address exposure (Tor Browser disables, but check)

**GOTCHA #15: JS Can Exfiltrate Data**
- **Attack Vectors**:
  - **DNS Prefetch**: `<link rel="dns-prefetch">` leaks visited domains
  - **Fetch/XHR**: Unauthorized requests to attacker-controlled servers
  - **WebSockets**: Bypass same-origin policy
  - **Service Workers**: Persistent background scripts
  - **Beacon API**: Send data even after page unload

**Defense Strategy: Zero-Trust JavaScript**

### Content Security Policy (CSP) - Your First Line of Defense

**MANDATORY: Strict CSP Headers**

```nginx
# Nginx configuration
add_header Content-Security-Policy "
    default-src 'none';
    script-src 'self' 'unsafe-inline';
    style-src 'self' 'unsafe-inline';
    img-src 'self' data:;
    font-src 'self';
    connect-src 'self';
    form-action 'self';
    base-uri 'self';
    frame-ancestors 'none';
    block-all-mixed-content;
    upgrade-insecure-requests;
" always;
```

**CSP Breakdown**:
- `default-src 'none'`: Block everything by default
- `script-src 'self'`: Only allow scripts from your domain (no CDNs)
- `'unsafe-inline'`: Required for inline scripts (minimize usage)
- `connect-src 'self'`: Block external API calls (no data exfiltration)
- `frame-ancestors 'none'`: Prevent clickjacking
- `block-all-mixed-content`: No HTTP on HTTPS pages

**GOTCHA #16: 'unsafe-inline' is Required for Many Use Cases**
- **Problem**: Nonces/hashes are ideal, but complex to implement
- **Why**: Inline event handlers (`onclick="..."`) need `'unsafe-inline'`
- **Impact**: Slightly weaker CSP, but still blocks remote scripts
- **Solution**: 
  - Minimize inline scripts (use external .js files)
  - Use event listeners instead of inline handlers
  - Plan to migrate to nonces in future (CSP Level 3)

### JavaScript Sandboxing Techniques

**GOTCHA #17: Subresource Integrity (SRI) is Useless on Tor**
- **Problem**: SRI validates external resources (CDNs)
- **Why**: We don't use CDNs (CSP blocks them)
- **Impact**: SRI is irrelevant for self-hosted scripts
- **Solution**: Self-host all JS/CSS, use file integrity checks in CI/CD

**GOTCHA #18: eval() and Function() Are Attacks Waiting to Happen**
- **Problem**: Dynamic code execution from strings
- **Why**: XSS vulnerabilities become code execution
- **Impact**: Attacker-controlled code runs in user context
- **Solution**: 
  - Never use `eval()`, `Function()`, `setTimeout(string)`, `setInterval(string)`
  - Use static code only
  - CSP can block `unsafe-eval` (we do)

```javascript
// BAD: eval is evil
eval(user_input);  // RCE vulnerability

// GOOD: Parse JSON safely
JSON.parse(user_input);  // Throws error on invalid input, no code execution
```

### Minimal JS Surface Area

**GOTCHA #19: Less JavaScript = Less Attack Surface**
- **Philosophy**: Every line of JS is a potential vulnerability
- **Strategy**: 
  - Use JS only for progressive enhancement (not core functionality)
  - Vanilla JS preferred over frameworks (smaller surface area)
  - No external libraries (lodash, jQuery, etc.) unless audited

**Example: CAPTCHA Enhancement (Optional JS)**

```javascript
// captcha-enhance.js - Progressive enhancement only
(function() {
    'use strict';
    
    // Feature detection
    if (!document.querySelector || !window.fetch) return;
    
    const form = document.querySelector('#captcha-form');
    if (!form) return;
    
    // Enhance form with client-side validation (optional)
    form.addEventListener('submit', function(e) {
        const input = form.querySelector('input[name="solution"]');
        if (!input.value || input.value.length < 4) {
            e.preventDefault();
            alert('Please enter the CAPTCHA code');
        }
    });
    
    // No external requests, no DOM manipulation beyond validation
})();
```

**Key Principles**:
- ✅ Works without JS (form submits normally)
- ✅ Enhances UX if JS enabled (client-side validation)
- ✅ No external requests (no exfiltration risk)
- ✅ No DOM manipulation (no XSS injection points)
- ✅ Strict mode (prevents silent errors)

---

## Onion Service Specific Concerns

### V3 Onion Addresses (.onion)

**GOTCHA #20: Only V3 Onions Are Secure**
- **V2 Onions (Deprecated)**: 16-character addresses, 1024-bit RSA (broken)
- **V3 Onions (Current)**: 56-character addresses, Ed25519 (secure)
- **Impact**: Never use V2, migrate immediately if legacy
- **Solution**: Ensure `HiddenServiceVersion 3` in `torrc`

```
# torrc
HiddenServiceDir /var/lib/tor/cerberus/
HiddenServiceVersion 3
HiddenServicePort 80 127.0.0.1:10000
```

**GOTCHA #21: Onion Addresses Are Public Keys**
- **Problem**: .onion address = public key hash, but private key is on disk
- **Why**: Compromise of `/var/lib/tor/cerberus/` = impersonation
- **Impact**: Attacker can clone your service
- **Solution**: 
  - Encrypt hidden service directory
  - Restrict permissions: `chmod 700 /var/lib/tor/cerberus/`
  - Backup private keys securely (offline storage)

### Onion Service DoS Protection

**GOTCHA #22: Onion Services Have Built-In PoW Defense**
- **Feature**: `HiddenServicePoWDefensesEnabled` (Proof of Work)
- **How**: Forces clients to solve computational puzzles before connection
- **When**: Activates during high load (auto-scaling difficulty)
- **Impact**: Legitimate users may experience delays (1-5s) during attacks
- **Solution**: 
  - Enable PoW in `torrc` (default in Tor 0.4.8+)
  - Monitor PoW queue metrics (`HiddenServicePoWQueueRate`)
  - Tune thresholds based on legitimate traffic patterns

```
# torrc - PoW configuration
HiddenServicePoWDefensesEnabled 1
HiddenServicePoWQueueRate 50
HiddenServicePoWQueueBurst 100
```

**GOTCHA #23: Introduction Point Flooding**
- **Attack**: Overload introduction points (rendezvous layer)
- **Symptom**: Service becomes unreachable, Tor logs show intro point failures
- **Defense**: 
  - Use Vanguards addon (prevents intro point enumeration)
  - Monitor intro point churn rate (normal: <10/hour)
  - Implement application-layer rate limiting (HAProxy)

### Cross-Origin Considerations

**GOTCHA #24: .onion Addresses Are Origins**
- **Problem**: Each .onion address is a unique origin (same-origin policy applies)
- **Why**: Cannot share cookies/storage between onions
- **Impact**: Subdomain strategies don't work (no `*.example.onion`)
- **Solution**: 
  - Single .onion per service (no multi-tenant architecture)
  - Use path-based routing (not subdomain-based)
  - CORS is irrelevant (no cross-onion requests)

**GOTCHA #25: No HTTPS = No Mixed Content**
- **Problem**: Onions use HTTP (not HTTPS), but Tor encrypts end-to-end
- **Why**: HTTPS is redundant (onion routing provides encryption)
- **Impact**: No SSL cert warnings, but some browser features require "secure context"
- **Solution**: 
  - Accept HTTP-only (normal for onions)
  - Be aware some APIs require HTTPS (Web Crypto API, Geolocation)
  - Don't implement HTTPS (pointless overhead)

---

## Project Roles & Responsibilities

To maintain clarity and ensure comprehensive project execution, Cerberus development follows a role-based approach. Each role represents a distinct perspective and set of responsibilities throughout the project lifecycle.

### Role Definitions

#### 🎯 Planner

**Primary Responsibility**: Strategic design and feasibility analysis

**Key Duties**:
- Create comprehensive planning documents for new features and architectural changes
- **Reference user stories to understand user needs and perspectives** (see [User Stories](#user-stories))
- Research and evaluate potential alternatives and competing approaches
- Master understanding of all aspects of the Cerberus project (architecture, security, performance, deployment)
- Act as project guardian: ensure all plans are safe, worthwhile, and aligned with project goals
- Perform threat modeling and risk assessment for proposed changes
- Identify dependencies, integration points, and potential conflicts with existing systems
- Document design decisions with clear rationale and trade-off analysis

**Outputs**:
- Planning documents (e.g., `xmr-priority-system.md`, `threat-dial-system.md`)
- Feasibility assessments with recommendations (PROCEED/DEFER/REJECT)
- Alternative solution comparisons
- Security and performance impact analyses

**Quality Standards**:
- **Must align with relevant user stories** (which users benefit, how does it serve their needs)
- All plans must include security considerations
- Must identify risks and provide mitigation strategies
- Should include implementation complexity estimates
- Must verify alignment with zero-JavaScript policy and Tor best practices

**Example Work**: When a new feature idea emerges (e.g., "XMR payment priority"), the Planner researches Monero integration, evaluates darknet market fit, identifies security risks (hot wallet, double-spend), and produces a detailed planning document with PROCEED/DEFER recommendation.

---

#### 📋 Coach

**Primary Responsibility**: Sprint planning and implementation guidance

**Key Duties**:
- Transform planning documents into actionable sprint execution plans
- Review current codebase to understand existing implementations and integration points
- Create precise, step-by-step implementation guides with zero assumptions or placeholder information
- Define clear phases, milestones, and acceptance criteria for each sprint
- Identify required files, functions, configurations, and dependencies
- Sequence tasks to minimize blocking dependencies
- Provide code structure recommendations and integration patterns
- Ensure sprint plans are complete and executable by developers

**Outputs**:
- Sprint documents with detailed task breakdowns
- Implementation checklists with specific file paths and function signatures
- Dependency installation commands and version requirements
- Configuration templates with exact parameter values
- Testing strategies with specific test cases

**Quality Standards**:
- No fake information or guessed details (verify against actual code)
- All file paths must be real (check if files exist, create if needed)
- Dependencies must specify exact versions and installation methods
- Each task must have clear acceptance criteria (how to verify completion)
- Must include rollback plans for risky changes

**Example Work**: Taking the "Threat Dial System" planning doc, the Coach reviews HAProxy/Nginx/Fortify code, identifies exactly which configuration parameters need multiplier logic, specifies the Rust functions to implement (`calculate_multiplier()`, `apply_dial_adjustments()`), and creates a Sprint 2 document with 15 specific tasks, each with file paths, expected changes, and test commands.

---

#### 📚 Librarian

**Primary Responsibility**: Documentation maintenance and verification

**Key Duties**:
- Review completed sprints and verify all planned features were implemented
- Audit code changes to ensure they match documentation claims
- Update existing documentation to reflect new features, APIs, and configurations
- Create or reorganize documentation for clarity and discoverability
- Maintain consistency across all documentation (terminology, formatting, cross-references)
- Ensure examples and code snippets in docs are accurate and tested
- Archive outdated documentation and update deprecation notices
- Verify all links, references, and citations are valid

**Outputs**:
- Updated technical documentation (architecture, API, configuration guides)
- Verified README.md with accurate feature lists and status
- Organized docs/ directory with clear navigation
- Changelog entries for each sprint
- Deprecated feature notices and migration guides

**Quality Standards**:
- All documentation must be verified against actual code (no stale docs)
- Code examples must be tested and executable
- Must maintain project-wide consistency (e.g., "Circuit ID" not "circuit-id", "Fortify" not "fortify-app")
- Must update Table of Contents and cross-references when adding new sections
- Must identify and fix broken links or outdated references

**Example Work**: After Sprint 2 completes Threat Dial implementation, the Librarian verifies the feature exists (checks for `THREAT_DIAL_POSITION` in cerberus.conf, `calculate_multiplier()` function in Fortify), updates the main README to include "Threat Dial" in features list, creates user-facing documentation (`docs/threat-dial-usage.md`), and updates `docs/fortify.md` to document the new Admin API endpoints.

---

### Role Interaction Flow

```
┌─────────────────────────────────────────────────────────────────┐
│  New Feature Idea / User Request                                │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│  🎯 PLANNER                                                      │
│  • Research alternatives                                         │
│  • Assess feasibility and risks                                 │
│  • Create planning document                                     │
│  • Recommend: PROCEED / DEFER / REJECT                          │
└─────────────────────────────────────────────────────────────────┘
                            ↓ (If PROCEED)
┌─────────────────────────────────────────────────────────────────┐
│  📋 COACH                                                        │
│  • Review planning doc + current codebase                       │
│  • Create sprint execution plan                                 │
│  • Break into phases with specific tasks                        │
│  • Define acceptance criteria and testing strategy              │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│  IMPLEMENTATION (Developer / AI Agent)                           │
│  • Execute sprint tasks                                          │
│  • Write code, tests, configs                                   │
│  • Commit changes with clear messages                           │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│  📚 LIBRARIAN                                                    │
│  • Verify implementation matches plan                           │
│  • Audit code for accuracy                                      │
│  • Update documentation (README, technical docs, API)           │
│  • Maintain consistency and fix stale references                │
└─────────────────────────────────────────────────────────────────┘
                            ↓
                     Feature Complete
```

---

### Role Separation Principles

**Why separate roles?**

1. **Clarity of Purpose**: Each role has a single, well-defined responsibility
2. **Quality Assurance**: Planner catches bad ideas early, Librarian catches documentation drift
3. **Efficiency**: Coach can focus on executable plans without worrying about documentation maintenance
4. **Verification**: Librarian acts as independent audit (did we actually build what we planned?)
5. **Scalability**: Roles can be distributed across team members or AI agents

**Role Boundaries**:
- ❌ Planner does NOT write sprint tasks (that's Coach's job)
- ❌ Coach does NOT implement features (focuses on planning only)
- ❌ Librarian does NOT design features (documents what exists, not what could be)

**Exception**: Small changes (typo fixes, minor doc updates) may skip formal role workflow.

---

## User Stories

User stories capture the "who, what, and why" of features from different perspectives. They guide decision-making throughout planning, implementation, and documentation phases. All roles should reference these stories to maintain user-centric focus.

### Story Format

```
As a [role/persona]
I want [goal/desire]
So that [benefit/value]

Acceptance Criteria:
- [Testable condition 1]
- [Testable condition 2]
- [Testable condition 3]
```

### Core User Personas

**1. Service Operator** - Runs the onion service, manages Cerberus deployment  
**2. End User** - Legitimate visitor accessing the protected service via Tor Browser  
**3. Admin/Monitor** - Security team member monitoring attacks and managing defenses  
**4. Attacker** - Adversary attempting DDoS or deanonymization (understand to defend)  
**5. Developer** - Future contributor implementing features or fixing bugs

---

### User Stories: Service Operator

**Story 1.1: Quick Deployment**
```
As a service operator
I want to deploy Cerberus with a single command
So that I can protect my onion service without manual configuration

Acceptance Criteria:
- `./cerberus.sh deploy` installs all dependencies
- Automatically detects backend onion service address
- Generates default configs for HAProxy, Nginx, Tor, Fortify
- Completes deployment in under 5 minutes on Ubuntu 24.04
- Provides clear error messages if dependencies missing
```

**Story 1.2: Attack Response**
```
As a service operator under DDoS attack
I want to adjust defense intensity without restarting services
So that I can stop the attack without downtime or dropped connections

Acceptance Criteria:
- Threat dial adjusts defenses within 5 seconds
- No service restarts required (HAProxy/Nginx/Fortify hot reload)
- Audit log records all dial changes with timestamp and reason
- Can dial back down when attack subsides
- Real-time metrics show immediate impact of dial changes
```

**Story 1.3: Resource Constraints**
```
As a service operator on a low-resource VPS (2GB RAM, 2 CPU)
I want Cerberus to perform efficiently under load
So that I don't need expensive hardware to defend my service

Acceptance Criteria:
- All services (HAProxy, Nginx, Tor, Fortify) use <500MB RAM combined
- Can handle 1,000 concurrent connections on 2 CPU cores
- Graceful degradation under overload (queue, not crash)
- Memory leaks detected and prevented during load testing
```

**Story 1.4: Monitoring Visibility**
```
As a service operator
I want to see real-time metrics of attacks and defense effectiveness
So that I know when attacks are happening and if my defenses are working

Acceptance Criteria:
- Dashboard shows live session counts (VIP/PoW/Normal/Banned/Queue)
- Attack events logged with timestamps and severity
- Metrics update every 2-5 seconds (near real-time)
- Accessible via Tor Onion Service (no clearnet exposure)
- Mobile-friendly UI for Tor Browser on phone
```

---

### User Stories: End User (Legitimate Visitor)

**Story 2.1: Accessibility Without JavaScript**
```
As an end user with Tor Browser in Safest mode (JavaScript disabled)
I want to access the protected service
So that I maintain maximum privacy while browsing

Acceptance Criteria:
- CAPTCHA displays correctly without JavaScript
- Form submission works with standard HTTP POST
- No JavaScript required for any core functionality
- Graceful fallback if JS disabled mid-session
```

**Story 2.2: Mobile Access**
```
As an end user on Tor Browser mobile (Android/iOS)
I want to solve CAPTCHAs quickly without battery drain
So that I can access the service on-the-go

Acceptance Criteria:
- CAPTCHA images load in under 3 seconds
- No PoW challenges that drain battery (optional PoW for fast-pass only)
- Touch-friendly interface (large tap targets, no hover states)
- Form inputs work with mobile keyboards
```

**Story 2.3: Fair Queue Experience**
```
As an end user waiting in the virtual queue
I want to see my estimated wait time and position
So that I know whether to wait or come back later

Acceptance Criteria:
- Queue page displays position number and estimated time
- Auto-refreshes every 10 seconds (meta-refresh, no JS)
- Option to pay XMR to skip queue (if enabled)
- Option to solve PoW for priority (if available)
```

**Story 2.4: Session Persistence**
```
As an end user who solved a CAPTCHA
I want my session to last at least 10 minutes
So that I'm not repeatedly challenged while browsing

Acceptance Criteria:
- VIP token valid for 10-30 minutes (configurable)
- Graceful re-authentication when token expires (redirect to CAPTCHA, not error)
- Token survives circuit rotation (tied to circuit reputation, not single circuit ID)
```

---

### User Stories: Admin/Monitor

**Story 3.1: Manual Intervention**
```
As an admin monitoring an active attack
I want to manually promote, demote, or ban specific circuits
So that I can respond to sophisticated attacks that bypass automated defenses

Acceptance Criteria:
- Admin panel shows top circuits by request rate
- One-click promote/demote/ban with reason field
- Changes apply within 5 seconds via HAProxy Runtime API
- All actions logged in audit trail with admin username
- TOTP 2FA required for ban/promote actions
```

**Story 3.2: Historical Analysis**
```
As an admin investigating a past attack
I want to view snapshots of traffic and defense metrics
So that I can understand attack patterns and improve defenses

Acceptance Criteria:
- Snapshot reports available for 5m, 15m, 30m, 1h, 4h, 24h, 7d, 30d, 90d, 365d
- Shows new circuits, bans, CAPTCHA success rate, attack events
- Can export data as CSV for external analysis
- Grafana dashboards with drill-down capability
```

**Story 3.3: Alert Notifications**
```
As an admin managing multiple services
I want to receive alerts when attacks are detected
So that I can respond quickly without constant monitoring

Acceptance Criteria:
- Alerts sent via Tor (Matrix bot, Telegram via SOCKS)
- Configurable thresholds (circuit flood > 500/min, CPU > 90%, etc.)
- Alert includes severity, metric value, and suggested action
- Can acknowledge alerts to suppress duplicate notifications
```

---

### User Stories: Developer

**Story 4.1: Local Development**
```
As a developer working on Windows
I want to develop Rust code locally and test on Ubuntu VM
So that I can contribute without running Linux as primary OS

Acceptance Criteria:
- VS Code Remote-SSH connects to Ubuntu VM
- Rust-analyzer works correctly for Linux target
- Unit tests run on both Windows and Ubuntu
- Clear documentation for cross-platform workflow
```

**Story 4.2: Contribution Clarity**
```
As a new contributor
I want to understand the codebase architecture quickly
So that I can implement features without breaking existing functionality

Acceptance Criteria:
- README explains three-layer architecture (HAProxy/Nginx/Fortify)
- Each component has its own docs/ guide
- Code comments explain complex logic (circuit tracking, stick tables)
- Example config files with inline documentation
```

**Story 4.3: Testing Confidence**
```
As a developer implementing a new feature
I want comprehensive tests to verify correctness
So that I don't accidentally break production deployments

Acceptance Criteria:
- Unit tests for Rust code (cargo test)
- Integration tests for full stack (Tor → HAProxy → Nginx → Fortify)
- Load tests simulate DDoS scenarios
- All tests pass in CI/CD before merge
```

---

### User Stories: Understanding the Attacker

**Story 5.1: Economic Sybil Attack**
```
As an attacker with $100 budget
I want to bypass defenses by creating many circuits
So that I can overwhelm the service

Defense Requirements:
- Circuit-based rate limiting prevents single-IP flooding
- Virtual queue prevents resource exhaustion (queue waits on client, not server)
- PoW/XMR payment raises economic cost (free → $0.03 per circuit)
- Ban duration (30 min) makes circuit rotation expensive (need fresh Tor circuits)
```

**Story 5.2: CAPTCHA Bypass**
```
As an attacker with OCR tools
I want to solve CAPTCHAs automatically
So that I can maintain my botnet connections

Defense Requirements:
- Adaptive CAPTCHA difficulty (increase complexity under attack)
- Constant-time validation (prevent timing attacks)
- Retry limits (3 failures = ban)
- CAPTCHA expiry (5 min TTL, can't pre-solve)
```

**Story 5.3: Deanonymization Attempt**
```
As an attacker
I want to correlate multiple circuits to the same user
So that I can deanonymize service operators or users

Defense Requirements:
- No fingerprinting vectors in responses (header scrubbing)
- Constant-time operations (prevent timing correlation)
- No IP logging (circuit IDs only, time-limited)
- Minimal metadata storage (no user-agent, language, timezone)
```

---

### How to Use User Stories

**For Planners (🎯):**
1. **Before planning a feature**, review relevant user stories
2. Ask: "Which persona does this serve? How does it help them?"
3. If no story exists for the feature, consider if it's truly needed
4. **Include story references in planning docs**: "This addresses Story 1.2 (Attack Response)"
5. Ensure acceptance criteria from stories are covered in feasibility analysis

**For Coaches (📋):**
1. **Sprint tasks should map to acceptance criteria** from user stories
2. Testing strategy must verify story acceptance criteria
3. If implementation deviates from story, update the story (don't ignore mismatch)

**For Librarians (📚):**
1. **Verify completed features satisfy user stories**
2. Update stories if user needs changed during implementation
3. Mark stories as "Implemented" in documentation
4. Create new stories based on user feedback or discovered edge cases

**For All Roles:**
- User stories are **living documents** (update as understanding improves)
- When ambiguous decisions arise, refer to stories for guidance
- New features should add new stories (don't implement without user need)

---

## Development Workflow & Best Practices

### Branching Strategy

**REQUIRED: Git Flow Model**

```
main (production)
  ├── develop (integration)
  │   ├── feature/haproxy-circuit-tracking
  │   ├── feature/nginx-static-gate
  │   ├── feature/fortify-captcha
  │   ├── bugfix/timeout-handling
  │   └── hotfix/security-csp-bypass
  └── release/v1.0.0
```

**Branch Rules**:
1. `main`: Production-ready code only, tagged releases
2. `develop`: Integration branch, all features merge here first
3. `feature/*`: New features, branched from `develop`
4. `bugfix/*`: Non-critical bugs, branched from `develop`
5. `hotfix/*`: Critical security fixes, branched from `main`
6. `release/*`: Release preparation, branched from `develop`

**Naming Convention**:
```bash
git checkout -b feature/short-description
git checkout -b bugfix/issue-123-fix-captcha-validation
git checkout -b hotfix/cve-2024-xxxx-xss
```

### Pull Request (PR) Workflow

**MANDATORY PR CHECKLIST (NEVER MERGE WITHOUT)**

```markdown
## PR Title Format
[TYPE] Brief description (#issue-number)

Types: FEATURE, BUGFIX, HOTFIX, DOCS, REFACTOR, TEST

## Description
- What: [What changes were made]
- Why: [Why these changes were necessary]
- How: [How the problem was solved]

## Security Considerations
- [ ] No secrets committed (keys, passwords, tokens)
- [ ] Input validation added for all user inputs
- [ ] No new JavaScript without CSP review
- [ ] Tor-specific edge cases considered
- [ ] Timing attack vulnerabilities checked

## Testing
- [ ] Unit tests pass (`cargo test`, `pytest`, etc.)
- [ ] Integration tests pass
- [ ] Manual testing in Tor Browser (Safest mode)
- [ ] Manual testing in Tor Browser (Standard mode with JS)
- [ ] Load testing performed (if performance-critical)

## Documentation
- [ ] Code comments added for complex logic
- [ ] README updated (if user-facing changes)
- [ ] API documentation updated (if endpoints changed)
- [ ] CHANGELOG.md updated

## Pre-Merge Checklist
- [ ] Branch is up-to-date with `develop`
- [ ] No merge conflicts
- [ ] CI/CD pipeline passes (all checks green)
- [ ] At least 1 code review approval
- [ ] Security review approval (if security-critical)
```

### Code Review Guidelines

**REQUIRED: At Least One Approval**

**Reviewer Responsibilities**:
1. **Correctness**: Does the code do what it claims?
2. **Security**: Any new attack vectors introduced?
3. **Performance**: Will this scale under load?
4. **Maintainability**: Is the code readable and documented?
5. **Testing**: Are tests comprehensive?

**Red Flags (Immediate Rejection)**:
- ❌ Hardcoded secrets or credentials
- ❌ `eval()`, `exec()`, or dynamic code execution
- ❌ Unvalidated user input passed to system calls
- ❌ Timing-attack-vulnerable comparisons (`==` for secrets)
- ❌ Missing error handling (unwrap() in Rust without justification)
- ❌ External dependencies without security audit
- ❌ JavaScript that violates CSP or fingerprints users

**Yellow Flags (Needs Discussion)**:
- ⚠️ Complex logic without comments
- ⚠️ Performance-critical code without benchmarks
- ⚠️ New dependencies (supply chain risk)
- ⚠️ Breaking API changes
- ⚠️ Database schema changes (migration required)

### CI/CD Pipeline

**REQUIRED: Automated Checks Before Merge**

```yaml
# .github/workflows/ci.yml (example)
name: Cerberus CI

on: [push, pull_request]

jobs:
  security-audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Run security audit
        run: |
          cargo audit  # Rust dependency vulnerabilities
          npm audit    # JS dependency vulnerabilities (if applicable)
          ./scripts/audit/check-secrets.sh  # Scan for committed secrets

  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Lint code
        run: |
          cargo clippy -- -D warnings  # Rust linter
          shellcheck scripts/**/*.sh   # Bash linter
          eslint static/js/**/*.js     # JS linter (if used)

  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Run unit tests
        run: |
          cargo test --all-features
          # Python tests (if mock-target uses tests)

  integration-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Setup test environment
        run: |
          docker-compose -f tests/docker-compose.yml up -d
      - name: Run integration tests
        run: |
          ./tests/integration/test-full-pipeline.sh
      - name: Cleanup
        run: docker-compose down

  tor-browser-test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Test in Tor Browser
        run: |
          # Automated Tor Browser testing (Selenium + Tor)
          ./tests/browser/test-captcha-flow.sh

  build-release:
    runs-on: ubuntu-latest
    needs: [security-audit, lint, unit-tests, integration-tests]
    steps:
      - uses: actions/checkout@v3
      - name: Build production artifacts
        run: |
          cargo build --release
          strip target/release/fortify  # Remove debug symbols
```

**Pipeline Stages**:
1. **Security Audit**: Check for known vulnerabilities
2. **Linting**: Enforce code style and catch common mistakes
3. **Unit Tests**: Test individual functions/modules
4. **Integration Tests**: Test full stack (Tor → HAProxy → Nginx → Fortify)
5. **Browser Tests**: Automated Tor Browser interaction
6. **Build**: Produce release artifacts

**Failure = No Merge**: All stages must pass before PR can be merged.

### Testing Discipline

**MANDATORY: Test-Driven Development (TDD) for Critical Code**

```rust
// ALWAYS: Write test first, then implementation

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_captcha_verification_valid_solution() {
        let challenge = CaptchaChallenge::new(Difficulty::Low);
        let solution = challenge.solution.clone();  // Simulate correct answer
        
        assert!(challenge.verify(&solution));
    }
    
    #[test]
    fn test_captcha_verification_invalid_solution() {
        let challenge = CaptchaChallenge::new(Difficulty::Low);
        
        assert!(!challenge.verify("WRONG"));
    }
    
    #[test]
    fn test_captcha_expiry() {
        let mut challenge = CaptchaChallenge::new(Difficulty::Low);
        challenge.created_at = current_timestamp() - 400;  // 400s ago
        
        assert!(challenge.is_expired(300));  // Should be expired (TTL=300s)
    }
    
    #[test]
    fn test_timing_attack_resistance() {
        let challenge = CaptchaChallenge::new(Difficulty::Low);
        
        let start = Instant::now();
        challenge.verify("WRONG");
        let wrong_duration = start.elapsed();
        
        let start = Instant::now();
        challenge.verify(&challenge.solution);
        let correct_duration = start.elapsed();
        
        // Verification time should be similar (within 10ms)
        assert!((wrong_duration.as_millis() as i64 - correct_duration.as_millis() as i64).abs() < 10);
    }
}
```

**Test Categories**:
1. **Unit Tests**: Test individual functions (fast, isolated)
2. **Integration Tests**: Test module interactions (medium speed)
3. **System Tests**: Test full stack (slow, requires Tor)
4. **Security Tests**: Test attack scenarios (critical)
5. **Performance Tests**: Benchmark under load (CI + manual)

---

## Testing in Tor Environments

### Local Tor Testing Setup

**REQUIRED: Development Tor Instance**

**Supported OS: Ubuntu 22.04/24.04 LTS or Debian 11/12**

```bash
# Install Tor (Ubuntu/Debian)
sudo apt update
sudo apt install tor

# Configure test torrc
cat > /tmp/test-torrc <<EOF
SocksPort 9150
ControlPort 9151
HiddenServiceDir /tmp/cerberus-test
HiddenServiceVersion 3
HiddenServicePort 80 127.0.0.1:10000
HiddenServicePoWDefensesEnabled 1
EOF

# Start Tor with test config
tor -f /tmp/test-torrc

# Get onion address
cat /tmp/cerberus-test/hostname
# Example: s7hbf8n3k2jd6s9p.onion
```

**Testing Workflow**:
1. Start local Tor with test config
2. Start Cerberus stack (HAProxy → Nginx → Fortify)
3. Connect via Tor Browser to test onion
4. Verify functionality in Safest + Standard modes

### Automated Testing Through Tor

**GOTCHA #26: Tor Circuit Isolation for Tests**
- **Problem**: Tests may interfere with each other (shared circuits)
- **Why**: Tor reuses circuits for performance
- **Impact**: Test A's ban affects Test B
- **Solution**: Use `IsolateDestAddr` and `IsolateDestPort` in test torrc

```bash
# Test torrc with circuit isolation
SocksPort 9150 IsolateDestAddr IsolateDestPort

# Each test gets a fresh circuit
curl -x socks5h://127.0.0.1:9150 http://test1.onion  # Circuit A
curl -x socks5h://127.0.0.1:9150 http://test2.onion  # Circuit B
```

### Tor Browser Testing

**REQUIRED: Manual Testing Checklist**

**Safest Mode (JavaScript Disabled)**:
- [ ] Homepage loads without errors
- [ ] CAPTCHA image displays
- [ ] CAPTCHA form submits via POST
- [ ] Successful verification redirects properly
- [ ] Failed verification shows error message
- [ ] All navigation works (no JS-dependent links)
- [ ] Images load (CSS, backgrounds, CAPTCHA)
- [ ] Forms validate server-side (not client-side only)

**Standard Mode (JavaScript Enabled)**:
- [ ] All Safest mode tests pass
- [ ] Optional JS enhancements work (client-side validation)
- [ ] No JS errors in browser console
- [ ] CSP doesn't block legitimate scripts
- [ ] No fingerprinting APIs used (canvas, WebGL, audio)
- [ ] No external requests (check Network tab)

**Safer Mode (SVG/Fonts Enabled, JS Disabled)**:
- [ ] Same as Safest mode (we don't use SVG/fonts anyway)

---

## Security Audit Checklist

### Pre-Deployment Audit

**MANDATORY: Complete Before Going Live**

#### Configuration Audit
- [ ] Tor `torrc` reviewed (PoW enabled, V3 onions, correct ports)
- [ ] HAProxy config reviewed (Circuit ID tracking, stick tables, timeouts)
- [ ] Nginx config reviewed (CSP headers, rate limits, buffer sizes)
- [ ] Fortify config reviewed (CAPTCHA TTL, socket permissions)
- [ ] All default passwords changed
- [ ] Secrets stored securely (not in Git, encrypted at rest)

#### Code Audit
- [ ] No hardcoded credentials or API keys
- [ ] All user inputs validated and sanitized
- [ ] SQL injection prevention (parameterized queries only)
- [ ] XSS prevention (output encoding, CSP)
- [ ] CSRF protection (tokens, SameSite cookies)
- [ ] Timing attack prevention (constant-time comparisons)
- [ ] Error messages don't leak sensitive info (no stack traces)

#### Network Audit
- [ ] Only `127.0.0.1` listening (not `0.0.0.0`)
- [ ] Firewall rules restrict external access
- [ ] HAProxy not exposed to internet (Tor only)
- [ ] Nginx not exposed to internet (HAProxy only)
- [ ] Fortify not exposed to internet (Nginx only)
- [ ] No unnecessary services running (disable SSH, FTP, etc.)

#### Dependency Audit
- [ ] All dependencies scanned for vulnerabilities (`cargo audit`, `npm audit`)
- [ ] Dependencies pinned to specific versions (no wildcards)
- [ ] Supply chain verification (checksum validation)
- [ ] Minimal dependencies (remove unused crates/packages)
- [ ] Regular dependency updates scheduled (monthly)

#### Access Control Audit
- [ ] File permissions set correctly (640 for configs, 600 for keys)
- [ ] Service users have minimal privileges (no root)
- [ ] HAProxy socket accessible only to Fortify user
- [ ] Tor hidden service directory encrypted and backed up
- [ ] Logs sanitized (no Circuit IDs in public logs)

---

## Common Pitfalls & Mistakes

### Mistake #1: Treating Tor Like Regular HTTP

**WRONG ASSUMPTION**: "It's just HTTP through a proxy"

**REALITY**: 
- Latency is 5-10x higher
- Circuits change every 10 minutes
- No persistent client identity
- Attackers are more sophisticated (hostile environment)

**SOLUTION**: Design for high latency, ephemeral sessions, and adversarial users.

### Mistake #2: JavaScript-Dependent UI

**WRONG APPROACH**: Build React SPA, add Tor support later

**REALITY**: 50% of users can't use your site (Safest mode)

**SOLUTION**: Server-side rendering first, progressive enhancement second.

### Mistake #3: Trusting Circuit IDs

**WRONG ASSUMPTION**: "Circuit ID = User, ban it forever"

**REALITY**: 
- One user = many circuits (rotation every 10 min)
- One circuit = potentially many users (exit relay shared)
- Banning permanently doesn't work (user gets new circuit)

**SOLUTION**: Time-limited bans, reputation decay, behavior-based blocking.

### Mistake #4: Logging Sensitive Data

**WRONG PRACTICE**: Log everything for debugging

**REALITY**: 
- Circuit IDs can deanonymize users if correlated
- IP addresses (even 127.0.0.1) can leak info
- Request payloads may contain secrets

**SOLUTION**: 
- Log only aggregated metrics (counts, rates)
- Redact sensitive fields (passwords, tokens, Circuit IDs)
- Rotate logs frequently, encrypt at rest

### Mistake #5: Ignoring CSP

**WRONG PRACTICE**: "CSP is too restrictive, skip it"

**REALITY**: CSP is your primary defense against XSS and data exfiltration

**SOLUTION**: Start with strict CSP, relax only if absolutely necessary.

### Mistake #6: Not Testing in Tor Browser

**WRONG PRACTICE**: Test in Chrome/Firefox, assume Tor works

**REALITY**: Tor Browser has unique behaviors (no JS, no fonts, strict CSP)

**SOLUTION**: Always test in Tor Browser (Safest + Standard modes).

### Mistake #7: Permanent State Assumptions

**WRONG ASSUMPTION**: "Users will have cookies next session"

**REALITY**: Tor Browser clears all state on exit

**SOLUTION**: Design for stateless sessions, short-lived tokens.

### Mistake #8: Single-Threaded Blocking Code

**WRONG PRACTICE**: Use synchronous I/O (HAProxy socket, database)

**REALITY**: Blocking one request blocks all requests (poor concurrency)

**SOLUTION**: Use async/await (Tokio), non-blocking I/O, connection pools.

### Mistake #9: Weak CAPTCHA Validation

**WRONG PRACTICE**: Case-sensitive comparison, no expiry

**REALITY**: Bots will brute-force or reuse solutions

**SOLUTION**: 
- Case-insensitive comparison
- One-time use (delete after validation)
- Expiry (5-10 min TTL)
- Rate limit verification attempts

### Mistake #10: Ignoring DoS Attack Vectors

**WRONG ASSUMPTION**: "Tor PoW is enough, don't need application layer defenses"

**REALITY**: Application-layer attacks bypass Tor PoW (HTTP floods, slow requests)

**SOLUTION**: Defense in depth (HAProxy limits + Nginx timeouts + Fortify logic).

---

## Defensive Programming Principles

### Principle #1: Fail Securely

**CONCEPT**: When errors occur, default to denying access (not granting)

```rust
// BAD: Default to allow on error
fn is_authorized(user_id: &str) -> bool {
    match check_permission(user_id) {
        Ok(result) => result,
        Err(_) => true,  // ERROR: Grants access on failure!
    }
}

// GOOD: Default to deny on error
fn is_authorized(user_id: &str) -> bool {
    match check_permission(user_id) {
        Ok(result) => result,
        Err(e) => {
            log::error!("Authorization check failed: {}", e);
            false  // Deny access on error
        }
    }
}
```

### Principle #2: Validate All Inputs

**CONCEPT**: Never trust user input, validate everything

```rust
// Validation checklist
fn validate_captcha_solution(solution: &str) -> Result<String, ValidationError> {
    // 1. Length check
    if solution.len() < 4 || solution.len() > 12 {
        return Err(ValidationError::InvalidLength);
    }
    
    // 2. Character whitelist (alphanumeric only)
    if !solution.chars().all(|c| c.is_alphanumeric()) {
        return Err(ValidationError::InvalidCharacters);
    }
    
    // 3. Sanitize (strip whitespace, lowercase)
    let sanitized = solution.trim().to_lowercase();
    
    // 4. Blacklist check (no SQL keywords, no shell metacharacters)
    if sanitized.contains("select") || sanitized.contains("drop") {
        return Err(ValidationError::BlacklistedContent);
    }
    
    Ok(sanitized)
}
```

### Principle #3: Minimize Attack Surface

**CONCEPT**: Less code = fewer bugs = stronger security

**Strategies**:
- Remove unused features (comment out, don't ship)
- Disable unnecessary endpoints (no admin UI if not needed)
- Strip debug symbols from binaries (`strip` command)
- Minimize dependencies (vendor code if possible)
- No dynamic loading (static linking only)

### Principle #4: Defense in Depth

**CONCEPT**: Multiple layers of defense, no single point of failure

**Cerberus Example**:
1. **Layer 0**: Tor PoW (network layer)
2. **Layer 1**: HAProxy rate limits (connection layer)
3. **Layer 2**: Nginx timeouts + CSP (protocol layer)
4. **Layer 3**: Fortify CAPTCHA (application layer)
5. **Layer 4**: Kernel hardening (OS layer)

**Result**: Attacker must bypass ALL layers (exponentially harder).

### Principle #5: Least Privilege

**CONCEPT**: Grant minimum permissions required for functionality

```bash
# Service users (no shell, no home directory)
useradd -r -s /bin/false -M haproxy
useradd -r -s /bin/false -M nginx
useradd -r -s /bin/false -M fortify

# File permissions (read-only configs, write-only logs)
chmod 640 /etc/haproxy/haproxy.cfg
chmod 640 /etc/nginx/nginx.conf
chmod 600 /etc/fortify/fortify.toml
chmod 700 /var/lib/tor/cerberus/

# Socket permissions (Fortify can write, HAProxy can read)
chown haproxy:fortify /var/run/haproxy.sock
chmod 660 /var/run/haproxy.sock
```

### Principle #6: Audit Everything

**CONCEPT**: Log security-relevant events for forensics

**What to Log**:
- ✅ Authentication attempts (success + failure)
- ✅ CAPTCHA verifications (success + failure)
- ✅ Circuit bans/promotions (reputation changes)
- ✅ Configuration changes (who, what, when)
- ✅ Service restarts (anomaly detection)
- ✅ Error rates (spike detection)

**What NOT to Log**:
- ❌ Passwords or tokens (even hashed)
- ❌ Full request payloads (may contain secrets)
- ❌ Circuit IDs in publicly accessible logs (deanonymization)
- ❌ User-Agent strings (fingerprinting)

### Principle #7: Secure Defaults

**CONCEPT**: Default configuration must be secure out-of-the-box

**Cerberus Defaults**:
- ✅ Tor PoW enabled by default
- ✅ HAProxy maxconn set conservatively (500)
- ✅ Nginx timeouts aggressive (5s)
- ✅ CAPTCHA TTL short (5 min)
- ✅ Circuit bans time-limited (30 min, not permanent)
- ✅ CSP strict (deny all by default, allow explicitly)

**User Can Relax**: Advanced users can tune thresholds, but defaults are safe.

---

## Final Notes for AI Agents

### When Writing Code
1. **Always** check if JavaScript is required (prefer server-side)
2. **Always** validate user inputs (no assumptions)
3. **Always** use constant-time comparisons for secrets
4. **Always** add error handling (no panics/unwraps in production)
5. **Always** write tests before implementation (TDD)
6. **Always** consider Tor-specific edge cases (circuit rotation, latency)

### When Reviewing Code
1. **Question** every external dependency (supply chain risk)
2. **Verify** CSP compliance (no fingerprinting APIs)
3. **Test** in Tor Browser (Safest mode minimum)
4. **Check** for timing attack vulnerabilities
5. **Ensure** defense in depth (multiple layers)

### When Debugging
1. **Reproduce** in Tor Browser first (not regular browser)
2. **Check** HAProxy logs for Circuit ID tracking
3. **Verify** CSP isn't blocking legitimate resources
4. **Test** with and without JavaScript
5. **Monitor** metrics (connection rates, queue length, ban counts)

### When Deploying
1. **Audit** all configurations (checklist above)
2. **Test** under load (simulate DDoS)
3. **Backup** Tor hidden service keys
4. **Monitor** logs for anomalies
5. **Document** changes in CHANGELOG.md

---

## Document Maintenance

This document is a living guide and should be updated when:
- New Tor vulnerabilities are discovered
- Browser behavior changes (Tor Browser updates)
- New attack vectors emerge
- Lessons learned from incidents
- Technology stack changes (new frameworks, libraries)

**Last Updated**: January 28, 2026  
**Next Review**: February 28, 2026 (monthly)

---

**Remember**: Cerberus operates in a hostile environment by default. Every decision should prioritize security over convenience. When in doubt, choose the more restrictive option.
