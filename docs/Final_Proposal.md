# Cerberus Master Architecture Summary (Final Proposal)

This is the blueprint for the system we are building.

---

## 1. The Kernel Shield (XDP / eBPF)
Located at the Linux Network Driver level. The first line of defense.

**It Defends Against:**
- Volumetric Floods: Drops massive SYN floods before the OS allocates RAM.
- UDP Floods: Drops all UDP packets except for the specific WireGuard port.
- Spoofing: Drops packets from invalid or private IP ranges on the public interface.
- Port Scanning: Silently drops traffic to any port other than 80/443 (Tor) and 51820 (WireGuard).

**What It Does:**
- Filters packets at line rate (10M+ pps).
- Protect the Linux Kernel from crashing due to interrupt overload.
- Ensures CPU is preserved for logic, not packet processing.

---

## 2. The Governor (HAProxy)
Located at the TCP/Socket level. The connection manager.

**It Defends Against:**
- Connection Exhaustion: Enforces a hard limit (e.g., 20,000) on open sockets per port.
- Slowloris Attacks: Kills connections that send headers too slowly (3s timeout).
- Tor Smashing: Prevents the local Tor daemon from running out of file descriptors.

**What It Does:**
- Port Separation: Splits traffic into two lanes:
  - Lane A (Public): Port 8080. Strict limits.
  - Lane B (VIP/Passport): Port 8081. Higher limits for users redirected from other nodes.
- TCP Reset: Instantly cuts off bad actors without wasting application threads.

---

## 3. The Gatekeeper (Nginx)
Located at the HTTP level. The router and static cache.

**It Defends Against:**
- Backend Overload: Ensures unauthenticated traffic never touches the real application.
- Buffer Overflow: Disables request body buffering to prevent RAM exhaustion.
- Disk I/O Spikes: Serves all static assets (CSS, Images, HTML) directly from RAM (open_file_cache).

**What It Does:**
- Cookie Check: Inspects headers for the cerb_session cookie.
  - If Valid: Proxies traffic to the Tor Tunnel (Backend).
  - If Invalid: Proxies traffic to Fortify (The Brain).
- Zero-JS Handling: Serves standard HTML responses compatible with "Safest" Tor security settings.

---

## 4. The Brain (Fortify - Rust Binary)
The custom logic engine running on every node. It makes all the decisions.

### A. The Logic Core
- Monitoring: Checks system CPU and RAM usage in real-time.
- Decision Making:
  - If Load < 80%: Admit Users (Serve Captcha).
  - If Load > 90%: Shed Users (Serve Passport Link to Peer).
  - If Load > 99%: Bunker Mode (Serve Queue Page).

### B. The Captcha System (Elastic Pool)
- RAM Pool: Holds 50,000–100,000 pre-generated Captchas in memory.
- Zero-Allocation: Pops a captcha from the stack in nanoseconds when a user requests one.
- The "Ammo Box":
  - On Shutdown: Dumps 50k images/answers to a raw binary file (ammo.bin).
  - On Startup: Loads ammo.bin instantly into RAM and re-hashes answers with a fresh session key.
- Result: System is ready to defend < 1 second after reboot.

### C. The Queue System (No-JS)
- Static Page: A lightweight HTML file served from RAM.
- Mechanism: Uses <meta http-equiv="refresh" content="15"> to auto-reload.
- Goal: Keeps the user's browser waiting without holding a server socket open.

### D. The Passport Protocol (Routing)
- Trigger: Activates only when the local node is dying.
- Action: Mints a cryptographic token signed with the node's Ed25519 Private Key.
- Payload: Contains Expiry (30s) and Target_Node.
- Result: Users clicking the link are accepted by the target node without solving a new captcha.

---

## 5. The Cluster (WireGuard Mesh)
The private internal network connecting Cerberus nodes.

**It Defends Against:**
- Blind Redirects: Prevents sending users to a dead node.
- Split-Brain: Ensures nodes know who is alive.

**What It Does:**
- Quick Join: New nodes simply add the WireGuard config and start gossiping.
- UDP Gossip: Every 5 seconds, nodes broadcast a tiny JSON packet:
  - Node_ID
  - CPU_Load
  - Tor_Health_Status (Canary Check)
- Health Mapping: Fortify maintains a real-time "Peer Table" in RAM to know exactly where to send refugee traffic.

---

## 6. The Shielded Origin (Backend)
The secret server hosting the actual application.

**It Defends Against:**
- IP Discovery: The IP is unknown to the public and the Cerberus nodes.
- Direct Attacks: Accepts traffic only via the Tor Hidden Service protocol.
- Unauthorized Access: Uses Tor Client Authorization to drop any connection attempt that doesn't possess the correct cryptographic key (even if they know the onion address).

**What It Does:**
- Receives sanitized, authenticated requests from Cerberus nodes.
- Processes business logic (Database, Chat, Market).
- Returns HTML responses back through the tunnel.
