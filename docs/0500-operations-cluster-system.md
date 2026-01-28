# 📖 User Story

**As a service operator experiencing traffic growth beyond a single instance's capacity**  
**I want to add additional Cerberus nodes to form a cluster**  
**So that I can distribute traffic, share defense state, and scale horizontally without downtime**

**Acceptance Criteria:**
- "Join Cluster" button in Monitoring UI to connect new node to existing cluster
- Cluster nodes share defense state (Threat Dial, circuit reputation, CAPTCHA sessions) via Redis
- HAProxy stick tables replicate across cluster for consistent circuit tracking
- Operators input real IP + port (not .onion) to establish secure P2P connection
- Cluster auto-balances traffic across nodes (via DNS round-robin or HAProxy load balancing)
- One node can fail without cluster-wide disruption (high availability)
- Monitoring UI shows all cluster nodes and their health status in real-time

---

# Cerberus Cluster System

**Layer:** Infrastructure (spans all 3 layers)  
**Status:** Planning  
**Dependencies:** Redis Cluster, HAProxy Peers, Secure P2P communication  
**Related Docs:** [0100-layer1-haproxy.md](0100-layer1-haproxy.md), [0300-operations-monitoring-ui.md](0300-operations-monitoring-ui.md), [0201-feature-threat-dial.md](0201-feature-threat-dial.md)

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Node Discovery & Joining](#node-discovery--joining)
4. [Shared State Management](#shared-state-management)
5. [Traffic Distribution](#traffic-distribution)
6. [Cluster Coordination](#cluster-coordination)
7. [High Availability](#high-availability)
8. [Security Considerations](#security-considerations)
9. [Monitoring & Observability](#monitoring--observability)
10. [Implementation Phases](#implementation-phases)

---

## Overview

The Cerberus Cluster System enables **horizontal scaling** by allowing multiple Cerberus nodes to work together as a unified defense system. Each node operates independently but shares critical state (circuit reputation, threat level, CAPTCHA sessions) to provide consistent protection across the cluster.

### Design Goals

1. **Zero-Downtime Scaling**: Add/remove nodes without interrupting service
2. **State Consistency**: All nodes make defense decisions based on shared global state
3. **Fault Tolerance**: Cluster survives individual node failures (no single point of failure)
4. **Simple Setup**: Operators join cluster via UI button + IP address (no complex config)
5. **Transparent to Users**: End users see single .onion address, unaware of backend cluster
6. **Secure P2P**: Nodes communicate over encrypted channels (not through Tor)

### Use Cases

- **Traffic Growth**: Single node at capacity (e.g., 10k req/sec), add nodes to handle 50k req/sec
- **Geographic Distribution**: Place nodes in different datacenters for lower latency
- **High Availability**: If one node crashes/reboots, others continue serving traffic
- **DDoS Mitigation**: Distribute attack traffic across multiple nodes to prevent saturation
- **Resource Specialization**: Some nodes handle CAPTCHA generation, others handle validation

---

## Architecture

### Topology: Shared-State Cluster

```
┌─────────────────────────────────────────────────────────────────┐
│ Tor Network                                                     │
│                                                                 │
│   user1.onion ───┐                                             │
│   user2.onion ───┼─→  [DNS Round Robin or Tor Load Balancer]  │
│   user3.onion ───┘                                             │
│                                                                 │
└──────────────────────────┬──────────────────────────────────────┘
                           │
                ┌──────────┴──────────┬──────────────┐
                │                     │              │
                ▼                     ▼              ▼
    ┌───────────────────┐ ┌───────────────────┐ ┌───────────────────┐
    │ Cerberus Node 1   │ │ Cerberus Node 2   │ │ Cerberus Node 3   │
    ├───────────────────┤ ├───────────────────┤ ├───────────────────┤
    │ HAProxy (Layer 1) │ │ HAProxy (Layer 1) │ │ HAProxy (Layer 1) │
    │ Nginx (Layer 2)   │ │ Nginx (Layer 2)   │ │ Nginx (Layer 2)   │
    │ Fortify (Layer 3) │ │ Fortify (Layer 3) │ │ Fortify (Layer 3) │
    └─────────┬─────────┘ └─────────┬─────────┘ └─────────┬─────────┘
              │                     │                     │
              └─────────────────────┼─────────────────────┘
                                    │
                                    ▼
                        ┌───────────────────────┐
                        │ Redis Cluster         │
                        │ (Shared State)        │
                        ├───────────────────────┤
                        │ • Circuit reputation  │
                        │ • Threat Dial level   │
                        │ • CAPTCHA sessions    │
                        │ • Rate limit counters │
                        │ • Cluster membership  │
                        └───────────────────────┘
```

### Key Components

1. **Redis Cluster**: Central shared state store (circuit scores, threat level, sessions)
2. **HAProxy Peers**: Stick table replication between nodes for circuit tracking
3. **Cluster Coordinator**: Lightweight service that manages node membership and health checks
4. **Monitoring UI**: Displays all cluster nodes and their status
5. **P2P Communication**: Nodes communicate directly via encrypted TCP (not through Tor)

---

## Node Discovery & Joining

### Workflow: Adding a New Node

```
Step 1: Operator starts new Cerberus instance on fresh server
Step 2: Operator opens Monitoring UI → clicks "Join Cluster"
Step 3: Operator inputs:
        - Cluster Leader IP: 198.51.100.10
        - Cluster Leader Port: 9000
        - Cluster Secret: <shared passphrase>
Step 4: New node sends join request to leader over HTTPS
Step 5: Leader validates secret, adds node to cluster membership list in Redis
Step 6: Leader responds with:
        - Redis Cluster connection string
        - List of all existing nodes (IP + port)
        - HAProxy peer configuration
Step 7: New node configures itself:
        - Connects to Redis Cluster
        - Establishes HAProxy peer connections
        - Syncs initial state (Threat Dial, circuit reputation)
Step 8: New node announces itself as "healthy" in Redis
Step 9: All nodes see new node in monitoring UI
```

### Join Request (HTTP POST)

```http
POST https://198.51.100.10:9000/cluster/join
Content-Type: application/json

{
  "node_id": "cerberus-node-2",
  "ip_address": "198.51.100.11",
  "port": 9000,
  "cluster_secret": "shared-passphrase-here"
}
```

**Response:**
```json
{
  "status": "accepted",
  "cluster_id": "cerberus-cluster-alpha",
  "redis_cluster": [
    "redis://198.51.100.10:6379",
    "redis://198.51.100.11:6380"
  ],
  "nodes": [
    {
      "node_id": "cerberus-node-1",
      "ip_address": "198.51.100.10",
      "port": 9000,
      "role": "leader"
    }
  ],
  "haproxy_peers": [
    {
      "name": "node1",
      "ip": "198.51.100.10",
      "port": 1024
    }
  ],
  "current_threat_level": 5
}
```

---

### Cluster Coordinator Service

**Role:** Manages node membership, health checks, and configuration distribution

**Implementation:** Lightweight Go service (runs on port 9000 alongside Fortify)

```go
package main

import (
    "encoding/json"
    "net/http"
    "sync"
)

type ClusterCoordinator struct {
    clusterSecret string
    nodes         map[string]*Node
    mutex         sync.RWMutex
    redis         *RedisClient
}

type Node struct {
    ID        string `json:"node_id"`
    IPAddress string `json:"ip_address"`
    Port      int    `json:"port"`
    Role      string `json:"role"` // "leader" or "member"
    Healthy   bool   `json:"healthy"`
    LastSeen  int64  `json:"last_seen"`
}

func (c *ClusterCoordinator) HandleJoinRequest(w http.ResponseWriter, r *http.Request) {
    var req JoinRequest
    json.NewDecoder(r.Body).Decode(&req)
    
    // Validate cluster secret
    if req.ClusterSecret != c.clusterSecret {
        http.Error(w, "Invalid cluster secret", http.StatusUnauthorized)
        return
    }
    
    // Add node to cluster
    node := &Node{
        ID:        req.NodeID,
        IPAddress: req.IPAddress,
        Port:      req.Port,
        Role:      "member",
        Healthy:   true,
        LastSeen:  time.Now().Unix(),
    }
    
    c.mutex.Lock()
    c.nodes[node.ID] = node
    c.mutex.Unlock()
    
    // Store in Redis for persistence
    c.redis.HSet("cluster:nodes", node.ID, node.Serialize())
    
    // Notify existing nodes of new member
    c.broadcastNodeJoined(node)
    
    // Respond with cluster config
    resp := JoinResponse{
        Status:            "accepted",
        ClusterID:         "cerberus-cluster-alpha",
        RedisCluster:      c.getRedisEndpoints(),
        Nodes:             c.getAllNodes(),
        HAProxyPeers:      c.getHAProxyPeers(),
        CurrentThreatLevel: c.getThreatLevel(),
    }
    
    json.NewEncoder(w).Encode(resp)
}
```

---

## Shared State Management

### State Categories

| State | Storage | Sync Method | TTL |
|-------|---------|-------------|-----|
| Circuit Reputation | Redis Hash | Write-through | 30 min |
| Threat Dial Level | Redis String | Pub/Sub broadcast | Persistent |
| CAPTCHA Sessions | Redis Hash | Write-through | 5 min |
| Rate Limit Counters | Redis Counter | Atomic increment | 1 min |
| Cluster Membership | Redis Hash | Write on change | Persistent |
| HAProxy Stick Tables | HAProxy Peers | Sync protocol | 30 min |

---

### Redis Cluster Configuration

**Deployment:** 3-node Redis Cluster (minimum for high availability)

```yaml
# Redis Cluster on 3 servers
Node 1: 198.51.100.10:6379 (master for slots 0-5460)
Node 2: 198.51.100.11:6379 (master for slots 5461-10922)
Node 3: 198.51.100.12:6379 (master for slots 10923-16383)

Each master has 1 replica for failover:
Node 1 Replica: 198.51.100.11:6380
Node 2 Replica: 198.51.100.12:6380
Node 3 Replica: 198.51.100.10:6380
```

**Benefits:**
- No single point of failure (any 1 node can fail)
- Automatic failover (replica promoted to master)
- Horizontal scaling (add more nodes = more capacity)

---

### Circuit Reputation Sharing

**Problem:** Circuit ABC123 is malicious, detected by Node 1. Node 2 must also block it.

**Solution:** All nodes read/write circuit scores to Redis

```rust
// Fortify on Node 1: detects malicious circuit
fn update_circuit_score(circuit_id: &str, penalty: i32) {
    let key = format!("circuit:{}:score", circuit_id);
    redis_client.incr_by(&key, penalty).unwrap();
    redis_client.expire(&key, 1800); // 30 min TTL
}

// Fortify on Node 2: checks circuit score before allowing request
fn get_circuit_score(circuit_id: &str) -> i32 {
    let key = format!("circuit:{}:score", circuit_id);
    redis_client.get(&key).unwrap_or(0)
}
```

**Consistency:** Redis guarantees atomic operations (INCRBY), so concurrent updates from multiple nodes are safe.

---

### Threat Dial Synchronization

**Problem:** Operator changes Threat Dial on Node 1 UI. All nodes must apply new level immediately.

**Solution:** Redis Pub/Sub for instant propagation

```rust
// Monitoring UI on Node 1: operator sets Threat Dial to 7
fn set_threat_dial(level: u8) {
    redis_client.set("global:threat_dial", level).unwrap();
    redis_client.publish("threat_dial:updates", level).unwrap();
}

// Fortify on Node 2, 3, ... N: subscribes to updates
fn subscribe_threat_dial_updates() {
    let mut pubsub = redis_client.get_pubsub().unwrap();
    pubsub.subscribe("threat_dial:updates").unwrap();
    
    loop {
        let msg = pubsub.get_message().unwrap();
        let new_level: u8 = msg.get_payload().unwrap();
        
        // Apply new threat level immediately
        GLOBAL_THREAT_LEVEL.store(new_level, Ordering::SeqCst);
        log::info!("Threat Dial updated to level {}", new_level);
    }
}
```

**Latency:** Sub-10ms propagation (Redis Pub/Sub is in-memory)

---

### HAProxy Stick Table Replication

**Problem:** Circuit ABC123 connects to Node 1, then reconnects to Node 2. Node 2 must remember circuit's history.

**Solution:** HAProxy Peers protocol (native stick table sync)

**Configuration:**
```haproxy
# Node 1: /etc/haproxy/haproxy.cfg
peers cerberus_cluster
    peer node1 198.51.100.10:1024
    peer node2 198.51.100.11:1024
    peer node3 198.51.100.12:1024

backend tor_backend
    stick-table type string len 64 size 1m expire 30m peers cerberus_cluster
    stick on req.hdr(X-Circuit-ID)
```

**What Gets Replicated:**
- Circuit ID → connection count
- Circuit ID → request rate (requests per second)
- Circuit ID → penalty score (from abuse detection)

**Sync Frequency:** Real-time (updates propagate within 100ms)

---

## Traffic Distribution

### Option 1: DNS Round Robin (Simple)

**Concept:** Multiple A/AAAA records for single .onion address

```
onion_address: abc123def456ghi789.onion

DNS Records (Tor internal):
abc123def456ghi789.onion → 198.51.100.10
abc123def456ghi789.onion → 198.51.100.11
abc123def456ghi789.onion → 198.51.100.12
```

**Behavior:** Tor client randomly selects one IP from DNS response

**Pros:**
- Zero configuration (Tor handles distribution)
- Each client "sticky" to one node (for session duration)

**Cons:**
- No load awareness (doesn't avoid overloaded nodes)
- No health checks (sends traffic to dead nodes)

---

### Option 2: HAProxy Frontend Load Balancer (Advanced)

**Concept:** Single HAProxy instance in front of cluster, distributes traffic

```
┌─────────────┐
│ Tor Network │
└──────┬──────┘
       │
       ▼
┌───────────────────┐
│ HAProxy LB        │ (Frontend load balancer)
│ 198.51.100.5:80   │
└─────────┬─────────┘
          │
    ┌─────┼─────┬─────────┐
    ▼     ▼     ▼         ▼
 [Node1][Node2][Node3][Node4]
```

**Configuration:**
```haproxy
frontend tor_frontend
    bind 198.51.100.5:80
    default_backend cerberus_cluster

backend cerberus_cluster
    balance roundrobin
    option httpchk GET /health
    server node1 198.51.100.10:80 check inter 5s
    server node2 198.51.100.11:80 check inter 5s
    server node3 198.51.100.12:80 check inter 5s
```

**Pros:**
- Health checks (removes dead nodes from pool)
- Advanced balancing (least connections, weighted round-robin)
- Single point of control

**Cons:**
- Frontend LB is single point of failure (mitigate with keepalived + VRRP)
- Adds latency (extra hop)

---

### Recommended: Hybrid Approach

1. **DNS Round Robin** for initial distribution across nodes
2. **HAProxy Stick Tables** for circuit persistence (same circuit → same node)
3. **Health Checks** in Monitoring UI warn operator if node is down

**Result:** Simple setup, good distribution, circuit consistency

---

## Cluster Coordination

### Node Roles

| Role | Responsibilities | Quantity |
|------|------------------|----------|
| **Leader** | Accepts join requests, distributes config, coordinates Threat Dial changes | 1 (elected) |
| **Member** | Processes traffic, shares state, reports health | N |

**Leader Election:** If leader dies, oldest surviving node becomes new leader (via Redis-based election)

---

### Heartbeat & Health Checks

**Mechanism:** Each node sends heartbeat to Redis every 10 seconds

```rust
fn send_heartbeat(node_id: &str) {
    let key = format!("cluster:heartbeat:{}", node_id);
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    
    redis_client.set_ex(&key, timestamp, 30).unwrap(); // 30 sec TTL
}

fn check_node_health(node_id: &str) -> bool {
    let key = format!("cluster:heartbeat:{}", node_id);
    redis_client.exists(&key).unwrap_or(false)
}
```

**Monitoring UI:** Displays all nodes with last-seen timestamp

```
┌─────────────────────────────────────────────┐
│ Cluster Status                              │
├─────────────┬───────────┬───────────────────┤
│ Node ID     │ Status    │ Last Seen         │
├─────────────┼───────────┼───────────────────┤
│ node-1      │ 🟢 Healthy│ 5 seconds ago     │
│ node-2      │ 🟢 Healthy│ 8 seconds ago     │
│ node-3      │ 🔴 Down   │ 2 minutes ago     │
└─────────────┴───────────┴───────────────────┘
```

**Alert:** If node heartbeat > 30 seconds old, mark as unhealthy and notify operator

---

### Configuration Sync

**Problem:** Operator updates Nginx config on Node 1. Other nodes must update too.

**Solution:** Git-based configuration management

```bash
# Configuration stored in Git repo (private, encrypted)
/etc/cerberus/
  ├── haproxy.cfg
  ├── nginx.conf
  ├── fortify.toml
  └── cluster.json

# On config change:
1. Operator commits change to Git
2. CI/CD pipeline pushes to all nodes
3. Each node runs: git pull && systemctl reload nginx haproxy fortify
```

**Alternative:** Configuration stored in Redis, nodes poll every 60 seconds for changes

---

## High Availability

### Failure Scenarios

| Failure | Impact | Mitigation |
|---------|--------|------------|
| **1 Node Dies** | Traffic redistributed to other nodes | DNS round-robin + HAProxy health checks |
| **Leader Dies** | New leader elected automatically | Redis-based election (oldest node wins) |
| **Redis Node Dies** | Replica promoted to master | Redis Cluster auto-failover (< 10 sec) |
| **Network Partition** | Cluster splits into sub-clusters | Quorum-based decisions (majority wins) |
| **Entire Datacenter Down** | Multi-region cluster continues | Deploy nodes in 2+ geographic regions |

---

### Split-Brain Prevention

**Problem:** Network partition splits cluster into 2 groups. Both think they're the cluster.

**Solution:** Quorum-based decisions (require majority agreement)

```rust
fn can_make_cluster_decision() -> bool {
    let total_nodes = get_cluster_size();
    let healthy_nodes = count_healthy_nodes();
    
    // Require > 50% of nodes to be reachable
    healthy_nodes > (total_nodes / 2)
}

// Example: 5-node cluster partitions into 3 nodes + 2 nodes
// Group with 3 nodes (majority) can make decisions
// Group with 2 nodes (minority) enters read-only mode
```

---

### Backup & Recovery

**Scenario:** Entire cluster fails, must rebuild from backup

**Backup Strategy:**
1. **Redis snapshots** every 6 hours → stored in S3/Backblaze B2
2. **Configuration files** in Git → can re-deploy from repo
3. **CAPTCHA sessions** are ephemeral → no backup needed (regenerate on restart)

**Recovery Time Objective (RTO):** 30 minutes (spin up new nodes, restore Redis from snapshot)

---

## Security Considerations

### 1. Cluster Secret Authentication

**Problem:** Attacker tries to join cluster with malicious node

**Solution:** Shared secret (passphrase) required to join

**Best Practice:**
- Secret stored in environment variable (not config file)
- Secret rotated every 90 days
- Secret has high entropy (e.g., `openssl rand -base64 32`)

```bash
# Generate cluster secret
CLUSTER_SECRET=$(openssl rand -base64 32)

# Set on all nodes
export CERBERUS_CLUSTER_SECRET="$CLUSTER_SECRET"
```

---

### 2. P2P Communication Encryption

**Problem:** Nodes communicate over public internet, traffic could be sniffed

**Solution:** TLS encryption for all node-to-node traffic

**Implementation:**
- Each node generates self-signed cert at startup
- Cluster coordinator validates certs via fingerprint (stored in Redis)
- All HTTP requests use HTTPS (TLS 1.3)

```go
// Cluster Coordinator: HTTPS server
tlsConfig := &tls.Config{
    MinVersion: tls.VersionTLS13,
    CertFile:   "/etc/cerberus/certs/node.crt",
    KeyFile:    "/etc/cerberus/certs/node.key",
}

server := &http.Server{
    Addr:      ":9000",
    TLSConfig: tlsConfig,
}
server.ListenAndServeTLS("", "")
```

---

### 3. Redis Security

**Problem:** Redis Cluster exposed to network, could be accessed by attacker

**Solution:** Redis authentication + network isolation

```bash
# Redis config: require password
requirepass your-strong-redis-password-here

# Network isolation: bind to private IPs only
bind 198.51.100.10 127.0.0.1

# Firewall: only allow traffic from cluster nodes
ufw allow from 198.51.100.11 to any port 6379
ufw allow from 198.51.100.12 to any port 6379
```

---

### 4. Rogue Node Detection

**Problem:** Compromised node sends malicious data to Redis (e.g., fake Threat Dial level)

**Solution:** Audit logging + anomaly detection

```rust
// Log all Threat Dial changes with node ID
fn set_threat_dial(node_id: &str, level: u8) {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    
    // Store in Redis
    redis_client.set("global:threat_dial", level).unwrap();
    
    // Audit log
    redis_client.lpush("audit:threat_dial", format!("{}|{}|{}", timestamp, node_id, level)).unwrap();
    redis_client.ltrim("audit:threat_dial", 0, 999).unwrap(); // Keep last 1000 changes
}

// Anomaly detection: alert if Threat Dial changes > 10 times per minute
fn detect_anomalies() {
    let changes = redis_client.llen("audit:threat_dial").unwrap();
    if changes > 10 {
        alert_operator("Suspicious Threat Dial activity detected!");
    }
}
```

---

## Monitoring & Observability

### Cluster Dashboard

**Location:** Monitoring UI → "Cluster" tab

**Metrics Displayed:**
```
┌─────────────────────────────────────────────────────────┐
│ Cerberus Cluster: cerberus-cluster-alpha               │
├─────────────────────────────────────────────────────────┤
│ Total Nodes: 4                                         │
│ Healthy Nodes: 3                                       │
│ Total Traffic: 45,320 req/sec                          │
│ Threat Dial: Level 5 (Medium)                          │
│                                                         │
│ Nodes:                                                  │
│ ┌────────────┬──────────┬─────────┬──────────────┐     │
│ │ Node ID    │ Status   │ Traffic │ Last Seen    │     │
│ ├────────────┼──────────┼─────────┼──────────────┤     │
│ │ node-1     │ 🟢 Healthy│ 12k/s   │ 3 sec ago    │     │
│ │ node-2     │ 🟢 Healthy│ 15k/s   │ 5 sec ago    │     │
│ │ node-3     │ 🟢 Healthy│ 18k/s   │ 2 sec ago    │     │
│ │ node-4     │ 🔴 Down  │ 0       │ 5 min ago    │     │
│ └────────────┴──────────┴─────────┴──────────────┘     │
│                                                         │
│ [Add Node] [Remove Node] [View Logs]                   │
└─────────────────────────────────────────────────────────┘
```

---

### Prometheus Metrics

**Exported by each node:**

```
# Node health
cerberus_node_healthy{node_id="node-1"} 1

# Traffic per node
cerberus_requests_per_second{node_id="node-1"} 12000

# Cluster membership
cerberus_cluster_size 4
cerberus_cluster_healthy_nodes 3

# Redis Cluster status
cerberus_redis_cluster_healthy 1
cerberus_redis_master_nodes 3
cerberus_redis_replica_nodes 3

# HAProxy peer sync status
cerberus_haproxy_peers_connected{peer="node-2"} 1
cerberus_haproxy_peers_connected{peer="node-3"} 1
```

---

### Alerting Rules

```yaml
# Alert if > 50% of nodes are down
- alert: ClusterDegraded
  expr: cerberus_cluster_healthy_nodes / cerberus_cluster_size < 0.5
  for: 1m
  annotations:
    summary: "Cerberus cluster has < 50% healthy nodes"

# Alert if Redis Cluster is down
- alert: RedisClusterDown
  expr: cerberus_redis_cluster_healthy == 0
  for: 30s
  annotations:
    summary: "Redis Cluster is unavailable"

# Alert if node hasn't sent heartbeat in 60 seconds
- alert: NodeUnresponsive
  expr: time() - cerberus_node_last_heartbeat > 60
  for: 1m
  annotations:
    summary: "Node {{ $labels.node_id }} is unresponsive"
```

---

## Implementation Phases

### Phase 1: Core Infrastructure (Week 1-2)

**Goal:** Basic cluster membership and Redis sharing

- [ ] Implement Cluster Coordinator service (join/leave endpoints)
- [ ] Redis Cluster setup (3-node cluster with replication)
- [ ] Node heartbeat mechanism (send to Redis every 10 seconds)
- [ ] Shared state: Circuit reputation in Redis (read/write from all nodes)
- [ ] Monitoring UI: Display cluster nodes and health status
- [ ] Manual testing: 2-node cluster, verify state sharing

**Deliverables:**
- 2-node cluster can share circuit reputation
- Monitoring UI shows both nodes

---

### Phase 2: HAProxy Peer Sync (Week 3)

**Goal:** Replicate HAProxy stick tables across nodes

- [ ] Configure HAProxy peers in haproxy.cfg
- [ ] Test stick table replication (circuit on Node 1 → visible on Node 2)
- [ ] Verify circuit persistence (reconnect to different node, state preserved)
- [ ] Performance testing: measure sync latency (< 100ms target)

**Deliverables:**
- HAProxy stick tables replicate in real-time

---

### Phase 3: Traffic Distribution (Week 4)

**Goal:** Distribute traffic across cluster nodes

- [ ] DNS round-robin setup for .onion address
- [ ] Test traffic distribution (verify requests spread across nodes)
- [ ] Circuit stickiness: same circuit → same node (via HAProxy stick table)
- [ ] Load testing: 50k req/sec across 4-node cluster

**Deliverables:**
- Traffic evenly distributed across nodes
- Circuit stickiness works (no session breakage)

---

### Phase 4: Threat Dial Sync (Week 5)

**Goal:** Synchronize Threat Dial changes across cluster

- [ ] Redis Pub/Sub for Threat Dial updates
- [ ] Monitoring UI: Change Threat Dial on one node → all nodes update
- [ ] Test latency: measure time from UI click → all nodes updated (< 100ms target)
- [ ] Audit logging: Track which node changed Threat Dial and when

**Deliverables:**
- Threat Dial changes propagate to all nodes in < 100ms

---

### Phase 5: High Availability (Week 6-7)

**Goal:** Cluster survives node failures

- [ ] Leader election: If leader dies, new leader elected
- [ ] Redis failover: Test Redis master node failure (replica promotion)
- [ ] HAProxy peer recovery: Node restarts → re-sync stick tables
- [ ] Quorum-based decisions: 5-node cluster → partition into 3+2 → verify majority group continues
- [ ] Monitoring UI: Alert if > 50% nodes down

**Deliverables:**
- Cluster survives 1 node failure with no service disruption
- Redis Cluster auto-failover works (< 10 sec downtime)

---

### Phase 6: Security Hardening (Week 8)

**Goal:** Secure inter-node communication

- [ ] Cluster secret authentication (reject join without valid secret)
- [ ] TLS encryption for all node-to-node traffic
- [ ] Redis authentication (password-protected)
- [ ] Firewall rules: Only allow cluster nodes to reach Redis
- [ ] Rogue node detection: Audit log + anomaly detection for suspicious activity
- [ ] Security audit: Attempt to join cluster with malicious node (should fail)

**Deliverables:**
- All cluster traffic encrypted (TLS 1.3)
- Unauthorized nodes cannot join cluster

---

### Phase 7: Monitoring & Observability (Week 9)

**Goal:** Full visibility into cluster health

- [ ] Cluster dashboard in Monitoring UI (node status, traffic, last seen)
- [ ] Prometheus metrics export (node health, traffic, Redis status)
- [ ] Alerting rules (cluster degraded, node unresponsive, Redis down)
- [ ] Grafana dashboard: Visualize cluster metrics over time
- [ ] Documentation: Operator guide for interpreting cluster metrics

**Deliverables:**
- Operators can monitor cluster health in real-time
- Alerts fire when nodes fail

---

### Phase 8: Testing & Validation (Week 10)

**Goal:** Validate cluster under realistic conditions

- [ ] Load testing: 100k req/sec across 10-node cluster
- [ ] Chaos engineering: Randomly kill nodes, verify recovery
- [ ] Network partition testing: Split cluster, verify quorum behavior
- [ ] Long-running test: 7-day soak test with simulated DDoS traffic
- [ ] Performance profiling: Identify bottlenecks (Redis, HAProxy sync, etc.)
- [ ] Documentation: Cluster setup guide for operators

**Deliverables:**
- Cluster handles 100k req/sec with < 50ms p99 latency
- Cluster survives chaos testing (random node failures)

---

## Success Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Horizontal scalability | 10x capacity per node added | Load testing: 10k/s per node |
| State sync latency | < 100ms | Measure time for Threat Dial change to propagate |
| Failover time | < 10 seconds | Kill Redis master, measure time to replica promotion |
| Cluster availability | 99.9% (< 8.76 hours downtime/year) | Monitor uptime over 90 days |
| Join time | < 30 seconds | Measure time from "Join Cluster" click → node healthy |
| Traffic distribution | ± 10% variance across nodes | Monitor requests per node, verify balance |

---

## Open Questions

1. **Cluster Size Limit:** What's the max cluster size before Redis/HAProxy sync becomes bottleneck? (Test with 50+ nodes)
2. **Geographic Distribution:** Should we support multi-region clusters (nodes in US + EU)? Higher latency but better fault tolerance.
3. **Onion Balancing:** Use Tor's OnionBalance feature instead of DNS round-robin? More complex but better load awareness.
4. **Automatic Scaling:** Should cluster auto-add nodes when traffic spikes? (Kubernetes-style auto-scaling)
5. **Split-Brain Recovery:** If network partition heals, how do we merge divergent state? (Last-write-wins, or manual operator intervention?)

---

## References

- **Redis Cluster Specification:** https://redis.io/docs/reference/cluster-spec/
- **HAProxy Peer Protocol:** https://www.haproxy.com/documentation/hapee/latest/clustering/peer-protocol/
- **Tor OnionBalance:** https://onionbalance.readthedocs.io/
- **Raft Consensus Algorithm:** https://raft.github.io/ (alternative to simple leader election)
- **CAP Theorem:** https://en.wikipedia.org/wiki/CAP_theorem (trade-offs in distributed systems)
