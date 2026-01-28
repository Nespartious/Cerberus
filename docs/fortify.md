# Fortify - Layer 3: The Keeper (Rust Application)

## Overview
Fortify is the brain of Cerberus, a Rust-based application layer that handles all dynamic logic, CAPTCHA generation/verification, circuit reputation management, and threat analysis. Unlike HAProxy (connection management) and Nginx (protocol sanitization), Fortify makes intelligent decisions about user behavior, generates challenges, and updates the defense posture in real-time.

**Position in Stack:** Tor → HAProxy (Port 10000) → Nginx (Port 10001) → **Fortify (Port 10002)** → [Optional: Target Service]

---

## Core Responsibilities

### 1. CAPTCHA System (Challenge Generation & Verification)
- **Image Generation**: Create distorted text/image challenges on-demand
- **Solution Storage**: Maintain challenge-solution pairs with expiry
- **Validation Logic**: Verify user-submitted solutions with fuzzy matching
- **Progressive Difficulty**: Scale CAPTCHA complexity based on threat level
- **Anti-Bot Techniques**: Timing analysis, mouse movement tracking (future)

### 2. HAProxy Integration (Circuit Reputation Management)
- **Stick Table Updates**: Communicate with HAProxy via Unix socket or HTTP API
- **VIP Promotion**: Mark validated circuits as trusted (bypass future CAPTCHAs)
- **Circuit Banning**: Flag malicious circuits for instant rejection
- **Reputation Decay**: Demote circuits over time (prevent token hoarding)
- **Token Management**: Issue time-limited authentication tokens

### 3. Threat Intelligence & Analysis
- **Behavioral Analysis**: Track circuit patterns (request timing, frequency, endpoints)
- **Anomaly Detection**: Identify suspicious behavior (future: ML-based)
- **Attack Classification**: DDoS, scraping, credential stuffing, etc.
- **Adaptive Thresholds**: Dynamically adjust defense sensitivity
- **Swarm State Management**: Coordinate defense across multiple Cerberus nodes (future)

### 4. API Endpoints & Logic Gates
- **Public Endpoints**: CAPTCHA image delivery, solution verification
- **Authenticated Endpoints**: User-facing APIs (protected by VIP status)
- **Admin Endpoints**: System status, manual circuit management, metrics
- **Health Checks**: Service availability for HAProxy monitoring

---

## Architecture & Design Principles

### Rust Benefits for Security-Critical Code
- **Memory Safety**: No buffer overflows, use-after-free, or null pointer dereferences
- **Concurrency Safety**: No data races (enforced by compiler)
- **Zero-Cost Abstractions**: High-level code with C-like performance
- **Small Attack Surface**: Minimal dependencies, static linking

### Application Structure

```
fortify/
├── src/
│   ├── main.rs                 # Entry point, HTTP server setup
│   ├── lib.rs                  # Shared library exports
│   ├── config/
│   │   ├── mod.rs              # Configuration management
│   │   └── settings.rs         # Settings structs and validation
│   ├── captcha/
│   │   ├── mod.rs              # CAPTCHA module exports
│   │   ├── generator.rs        # Image generation (text distortion, noise)
│   │   ├── validator.rs        # Solution verification logic
│   │   └── storage.rs          # Challenge-solution storage (in-memory + persistence)
│   ├── haproxy/
│   │   ├── mod.rs              # HAProxy integration exports
│   │   ├── client.rs           # Socket/HTTP client for stick table updates
│   │   └── commands.rs         # HAProxy command builders
│   ├── circuit/
│   │   ├── mod.rs              # Circuit tracking exports
│   │   ├── reputation.rs       # Reputation scoring logic
│   │   ├── behavior.rs         # Behavioral analysis
│   │   └── database.rs         # Persistent circuit history
│   ├── api/
│   │   ├── mod.rs              # API module exports
│   │   ├── handlers.rs         # HTTP request handlers
│   │   ├── middleware.rs       # Auth, rate limiting, logging
│   │   └── models.rs           # Request/response structs
│   ├── swarm/
│   │   ├── mod.rs              # Swarm coordination (future)
│   │   └── state.rs            # Distributed state management
│   └── utils/
│       ├── mod.rs              # Utility exports
│       ├── crypto.rs           # Secure random, hashing
│       └── time.rs             # Timestamp utilities
├── tests/
│   ├── integration/            # Full-stack tests
│   └── unit/                   # Module-specific tests
└── benches/
    └── captcha_bench.rs        # Performance benchmarks
```

---

## Key Features & Implementation

### 1. CAPTCHA Generation (Text-Based)

**Approach:** Generate random strings, render as images with distortion, noise, and rotation.

**Library:** `captcha` crate (https://crates.io/crates/captcha)

**Implementation (`captcha/generator.rs`):**

```rust
use captcha::{Captcha, Difficulty};
use rand::Rng;

pub struct CaptchaChallenge {
    pub challenge_id: String,
    pub image: Vec<u8>,  // PNG bytes
    solution: String,    // Private
    created_at: u64,     // Unix timestamp
}

impl CaptchaChallenge {
    pub fn new(difficulty: Difficulty) -> Self {
        let mut rng = rand::thread_rng();
        let challenge_id = format!("{:x}", rng.gen::<u64>());
        
        // Generate CAPTCHA
        let captcha = Captcha::new()
            .add_chars(6)  // 6-character challenge
            .apply_filter(difficulty.into())
            .view(220, 120);  // Image dimensions
        
        let solution = captcha.chars_as_string();
        let image = captcha.as_png().unwrap();
        
        Self {
            challenge_id,
            image,
            solution,
            created_at: current_timestamp(),
        }
    }
    
    pub fn verify(&self, user_solution: &str) -> bool {
        // Case-insensitive comparison
        self.solution.to_lowercase() == user_solution.to_lowercase()
    }
    
    pub fn is_expired(&self, ttl_seconds: u64) -> bool {
        current_timestamp() - self.created_at > ttl_seconds
    }
}
```

**Difficulty Levels:**
- **Low**: 6 chars, minimal distortion (default)
- **Medium**: 6 chars, wave distortion + noise
- **High**: 8 chars, heavy distortion + rotation

**Storage (`captcha/storage.rs`):**

```rust
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub struct CaptchaStore {
    challenges: Arc<RwLock<HashMap<String, CaptchaChallenge>>>,
    ttl_seconds: u64,
}

impl CaptchaStore {
    pub fn new(ttl_seconds: u64) -> Self {
        Self {
            challenges: Arc::new(RwLock::new(HashMap::new())),
            ttl_seconds,
        }
    }
    
    pub fn store(&self, challenge: CaptchaChallenge) -> String {
        let id = challenge.challenge_id.clone();
        self.challenges.write().unwrap().insert(id.clone(), challenge);
        id
    }
    
    pub fn verify(&self, challenge_id: &str, solution: &str) -> bool {
        let mut store = self.challenges.write().unwrap();
        
        if let Some(challenge) = store.get(challenge_id) {
            if challenge.is_expired(self.ttl_seconds) {
                store.remove(challenge_id);
                return false;
            }
            
            let valid = challenge.verify(solution);
            if valid {
                store.remove(challenge_id);  // One-time use
            }
            return valid;
        }
        
        false
    }
    
    pub fn cleanup_expired(&self) {
        let mut store = self.challenges.write().unwrap();
        store.retain(|_, challenge| !challenge.is_expired(self.ttl_seconds));
    }
}
```

**Periodic Cleanup:**
```rust
// In main.rs, spawn background task
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    loop {
        interval.tick().await;
        captcha_store.cleanup_expired();
    }
});
```

### 2. HAProxy Integration (Stick Table Updates)

**Communication Method:** Unix socket to HAProxy Runtime API

**HAProxy Socket Setup:**
```haproxy
# In haproxy.cfg
global
    stats socket /var/run/haproxy.sock mode 660 level admin
```

**Implementation (`haproxy/client.rs`):**

```rust
use std::io::{Write, BufRead, BufReader};
use std::os::unix::net::UnixStream;

pub struct HaproxyClient {
    socket_path: String,
}

impl HaproxyClient {
    pub fn new(socket_path: String) -> Self {
        Self { socket_path }
    }
    
    pub fn set_gpc0(&self, circuit_id: &str, value: i32) -> Result<(), String> {
        let cmd = format!("set table tor_ingress key {} data.gpc0 {}\n", circuit_id, value);
        self.send_command(&cmd)
    }
    
    pub fn promote_circuit(&self, circuit_id: &str) -> Result<(), String> {
        // Set gpc0=1 (VIP flag)
        self.set_gpc0(circuit_id, 1)
    }
    
    pub fn ban_circuit(&self, circuit_id: &str) -> Result<(), String> {
        // Set gpc0=2 (Ban flag)
        self.set_gpc0(circuit_id, 2)
    }
    
    pub fn reset_circuit(&self, circuit_id: &str) -> Result<(), String> {
        // Set gpc0=0 (Normal)
        self.set_gpc0(circuit_id, 0)
    }
    
    fn send_command(&self, cmd: &str) -> Result<(), String> {
        let mut stream = UnixStream::connect(&self.socket_path)
            .map_err(|e| format!("Failed to connect to HAProxy socket: {}", e))?;
        
        stream.write_all(cmd.as_bytes())
            .map_err(|e| format!("Failed to write command: {}", e))?;
        
        // Read response
        let mut reader = BufReader::new(stream);
        let mut response = String::new();
        reader.read_line(&mut response)
            .map_err(|e| format!("Failed to read response: {}", e))?;
        
        if response.contains("Done") || response.is_empty() {
            Ok(())
        } else {
            Err(format!("HAProxy error: {}", response))
        }
    }
}
```

**Usage in API Handlers:**
```rust
// After successful CAPTCHA verification
if captcha_store.verify(&challenge_id, &solution) {
    haproxy_client.promote_circuit(&circuit_id)?;
    return Ok(Json(VerifyResponse { success: true }));
}
```

### 3. HTTP API (Using Axum Framework)

**Framework:** Axum (https://github.com/tokio-rs/axum) - Rust async web framework

**Server Setup (`main.rs`):**

```rust
use axum::{
    Router,
    routing::{get, post},
    extract::State,
    http::StatusCode,
};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    // Initialize shared state
    let captcha_store = Arc::new(CaptchaStore::new(300));  // 5 min TTL
    let haproxy_client = Arc::new(HaproxyClient::new("/var/run/haproxy.sock".into()));
    
    let app_state = AppState {
        captcha_store,
        haproxy_client,
    };
    
    // Build router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/captcha-image", get(captcha_image_handler))
        .route("/verify-captcha", post(verify_captcha_handler))
        .route("/api/status", get(status_handler))
        .with_state(Arc::new(app_state));
    
    // Run server
    let listener = tokio::net::TcpListener::bind("127.0.0.1:10002")
        .await
        .unwrap();
    
    println!("Fortify listening on 127.0.0.1:10002");
    axum::serve(listener, app).await.unwrap();
}

#[derive(Clone)]
struct AppState {
    captcha_store: Arc<CaptchaStore>,
    haproxy_client: Arc<HaproxyClient>,
}
```

**API Handlers (`api/handlers.rs`):**

```rust
use axum::{
    extract::{State, Query},
    response::{IntoResponse, Response},
    http::{StatusCode, HeaderMap},
    Json,
};
use serde::{Deserialize, Serialize};

// GET /api/captcha-image?challenge=new
pub async fn captcha_image_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<CaptchaParams>,
) -> Response {
    // Generate new CAPTCHA
    let challenge = CaptchaChallenge::new(Difficulty::Low);
    let challenge_id = state.captcha_store.store(challenge);
    
    // Return PNG image with challenge ID in header
    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", "image/png".parse().unwrap());
    headers.insert("X-Challenge-ID", challenge_id.parse().unwrap());
    headers.insert("Cache-Control", "no-store".parse().unwrap());
    
    (StatusCode::OK, headers, challenge.image).into_response()
}

// POST /verify-captcha
#[derive(Deserialize)]
pub struct VerifyRequest {
    challenge_id: String,
    solution: String,
}

#[derive(Serialize)]
pub struct VerifyResponse {
    success: bool,
    message: String,
}

pub async fn verify_captcha_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<VerifyRequest>,
) -> Response {
    // Extract Circuit ID from header (passed by Nginx)
    let circuit_id = match headers.get("X-Circuit-ID") {
        Some(id) => id.to_str().unwrap_or("unknown"),
        None => {
            return (StatusCode::BAD_REQUEST, Json(VerifyResponse {
                success: false,
                message: "Missing Circuit ID".into(),
            })).into_response();
        }
    };
    
    // Verify CAPTCHA
    if state.captcha_store.verify(&payload.challenge_id, &payload.solution) {
        // Promote circuit to VIP in HAProxy
        match state.haproxy_client.promote_circuit(circuit_id) {
            Ok(_) => {
                (StatusCode::OK, Json(VerifyResponse {
                    success: true,
                    message: "Access granted".into(),
                })).into_response()
            }
            Err(e) => {
                eprintln!("Failed to promote circuit: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(VerifyResponse {
                    success: false,
                    message: "Internal error".into(),
                })).into_response()
            }
        }
    } else {
        (StatusCode::FORBIDDEN, Json(VerifyResponse {
            success: false,
            message: "Invalid solution".into(),
        })).into_response()
    }
}

// GET /health
pub async fn health_check() -> StatusCode {
    StatusCode::OK
}
```

### 4. Circuit Reputation & Behavioral Analysis

**Reputation Scoring (`circuit/reputation.rs`):**

```rust
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub struct CircuitReputation {
    circuits: Arc<RwLock<HashMap<String, CircuitScore>>>,
}

pub struct CircuitScore {
    vip: bool,
    banned: bool,
    captcha_solves: u32,
    captcha_fails: u32,
    last_activity: u64,
    created_at: u64,
}

impl CircuitReputation {
    pub fn new() -> Self {
        Self {
            circuits: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    pub fn mark_captcha_success(&self, circuit_id: &str) {
        let mut circuits = self.circuits.write().unwrap();
        let score = circuits.entry(circuit_id.to_string()).or_insert(CircuitScore::default());
        score.captcha_solves += 1;
        score.vip = true;
        score.last_activity = current_timestamp();
    }
    
    pub fn mark_captcha_failure(&self, circuit_id: &str) {
        let mut circuits = self.circuits.write().unwrap();
        let score = circuits.entry(circuit_id.to_string()).or_insert(CircuitScore::default());
        score.captcha_fails += 1;
        score.last_activity = current_timestamp();
        
        // Auto-ban after 5 failures
        if score.captcha_fails >= 5 {
            score.banned = true;
        }
    }
    
    pub fn is_banned(&self, circuit_id: &str) -> bool {
        let circuits = self.circuits.read().unwrap();
        circuits.get(circuit_id).map_or(false, |s| s.banned)
    }
    
    pub fn decay_reputation(&self, max_age_seconds: u64) {
        let mut circuits = self.circuits.write().unwrap();
        let now = current_timestamp();
        
        circuits.retain(|_, score| {
            // Remove circuits inactive for >1 hour
            if now - score.last_activity > max_age_seconds {
                return false;
            }
            
            // Demote VIP status after 30 minutes
            if score.vip && (now - score.last_activity > 1800) {
                score.vip = false;
            }
            
            true
        });
    }
}
```

**Behavioral Analysis (Future Enhancement):**
- Track request intervals (human vs. bot timing)
- Detect headless browsers (Canvas fingerprinting)
- Mouse movement analysis (JS-based, privacy concerns)
- Challenge-response timing (humans take 5-30s, bots <1s)

---

## Configuration Management

**Config File (`config/settings.rs`):**

```rust
use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Settings {
    pub server: ServerConfig,
    pub captcha: CaptchaConfig,
    pub haproxy: HaproxyConfig,
    pub security: SecurityConfig,
}

#[derive(Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Deserialize, Clone)]
pub struct CaptchaConfig {
    pub ttl_seconds: u64,
    pub difficulty: String,  // "low", "medium", "high"
    pub length: u32,
}

#[derive(Deserialize, Clone)]
pub struct HaproxyConfig {
    pub socket_path: String,
    pub table_name: String,
}

#[derive(Deserialize, Clone)]
pub struct SecurityConfig {
    pub max_captcha_failures: u32,
    pub reputation_decay_seconds: u64,
}

impl Settings {
    pub fn from_file(path: &str) -> Result<Self, config::ConfigError> {
        let settings = config::Config::builder()
            .add_source(config::File::with_name(path))
            .build()?;
        
        settings.try_deserialize()
    }
}
```

**Example Config File (`fortify.toml`):**

```toml
[server]
host = "127.0.0.1"
port = 10002

[captcha]
ttl_seconds = 300
difficulty = "low"
length = 6

[haproxy]
socket_path = "/var/run/haproxy.sock"
table_name = "tor_ingress"

[security]
max_captcha_failures = 5
reputation_decay_seconds = 3600
```

---

## Security Considerations

### 1. CAPTCHA Security
- **One-Time Use**: Delete challenge after successful verification
- **Time-Limited**: Expire challenges after 5 minutes
- **No Solution Leakage**: Never expose solution in responses or logs
- **Rate Limiting**: Limit CAPTCHA generation per circuit (prevent grinding)

### 2. Input Validation
```rust
// Validate challenge ID format (hex only)
fn validate_challenge_id(id: &str) -> bool {
    id.len() == 16 && id.chars().all(|c| c.is_ascii_hexdigit())
}

// Sanitize user solution (alphanumeric only)
fn sanitize_solution(solution: &str) -> String {
    solution.chars().filter(|c| c.is_alphanumeric()).collect()
}
```

### 3. Error Handling (No Information Leakage)
```rust
// Bad: Leaks if challenge exists
if !captcha_exists {
    return "Challenge not found";
} else if expired {
    return "Challenge expired";
} else {
    return "Invalid solution";
}

// Good: Generic error
return "Verification failed";
```

### 4. Timing Attack Prevention
```rust
// Constant-time comparison for solutions
use subtle::ConstantTimeEq;

fn verify_solution_secure(expected: &str, user_input: &str) -> bool {
    expected.as_bytes().ct_eq(user_input.as_bytes()).into()
}
```

---

## Testing Strategy

### Unit Tests (`tests/unit/`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_captcha_generation() {
        let challenge = CaptchaChallenge::new(Difficulty::Low);
        assert_eq!(challenge.solution.len(), 6);
        assert!(!challenge.image.is_empty());
    }
    
    #[test]
    fn test_captcha_verification() {
        let challenge = CaptchaChallenge::new(Difficulty::Low);
        let solution = challenge.solution.clone();
        
        assert!(challenge.verify(&solution));
        assert!(!challenge.verify("wrong"));
    }
    
    #[test]
    fn test_captcha_expiry() {
        let mut challenge = CaptchaChallenge::new(Difficulty::Low);
        challenge.created_at = current_timestamp() - 400;  // 400s ago
        
        assert!(challenge.is_expired(300));  // Expired (TTL=300s)
        assert!(!challenge.is_expired(500));  // Not expired (TTL=500s)
    }
}
```

### Integration Tests (`tests/integration/`)

```rust
#[tokio::test]
async fn test_full_captcha_flow() {
    // Start test server
    let server = start_test_server().await;
    
    // 1. Request CAPTCHA image
    let res = reqwest::get("http://127.0.0.1:10002/api/captcha-image?challenge=new")
        .await
        .unwrap();
    
    let challenge_id = res.headers()
        .get("X-Challenge-ID")
        .unwrap()
        .to_str()
        .unwrap();
    
    // 2. Submit solution (mocked)
    let res = reqwest::Client::new()
        .post("http://127.0.0.1:10002/verify-captcha")
        .header("X-Circuit-ID", "test-circuit-123")
        .json(&serde_json::json!({
            "challenge_id": challenge_id,
            "solution": "MOCK_SOLUTION"
        }))
        .send()
        .await
        .unwrap();
    
    assert_eq!(res.status(), 200);
    
    // 3. Verify HAProxy was updated (check mock)
    // ...
}
```

---

## Performance Optimization

### 1. Async I/O (Tokio Runtime)
```rust
// Non-blocking HAProxy socket communication
pub async fn promote_circuit_async(&self, circuit_id: &str) -> Result<(), String> {
    let socket_path = self.socket_path.clone();
    let circuit_id = circuit_id.to_string();
    
    tokio::task::spawn_blocking(move || {
        // Offload blocking socket I/O to thread pool
        HaproxyClient::new(socket_path).promote_circuit(&circuit_id)
    }).await.unwrap()
}
```

### 2. CAPTCHA Image Caching (Pre-generation)
```rust
// Pre-generate CAPTCHA pool during idle time
pub struct CaptchaPool {
    pool: Arc<RwLock<Vec<CaptchaChallenge>>>,
    size: usize,
}

impl CaptchaPool {
    pub fn new(size: usize) -> Self {
        let pool = Self {
            pool: Arc::new(RwLock::new(Vec::new())),
            size,
        };
        pool.refill();
        pool
    }
    
    pub fn get(&self) -> Option<CaptchaChallenge> {
        let mut pool = self.pool.write().unwrap();
        let challenge = pool.pop();
        
        // Async refill if below threshold
        if pool.len() < self.size / 2 {
            tokio::spawn(async { self.refill() });
        }
        
        challenge
    }
    
    fn refill(&self) {
        let mut pool = self.pool.write().unwrap();
        while pool.len() < self.size {
            pool.push(CaptchaChallenge::new(Difficulty::Low));
        }
    }
}
```

### 3. Memory Management
- Use `Arc<T>` for shared state (atomic reference counting)
- Use `RwLock` for read-heavy, write-rare data (stick tables)
- Periodic cleanup of expired data (prevent memory leaks)

---

## Deployment & Operations

### Build for Production
```bash
# Optimized release build
cargo build --release

# Binary location
./target/release/fortify
```

### Systemd Service (`/etc/systemd/system/fortify.service`)

```ini
[Unit]
Description=Fortify - Cerberus Layer 3
After=network.target haproxy.service

[Service]
Type=simple
User=fortify
Group=fortify
WorkingDirectory=/opt/cerberus/fortify
ExecStart=/opt/cerberus/fortify/target/release/fortify
Restart=always
RestartSec=5

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/run/haproxy.sock

[Install]
WantedBy=multi-user.target
```

### Logging
```rust
// Use `tracing` crate for structured logging
use tracing::{info, warn, error};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    
    info!("Fortify starting on 127.0.0.1:10002");
    // ...
}
```

---

## Future Enhancements

### Sprint 2+
1. **Persistent Circuit Database**: SQLite/Redis for long-term reputation tracking
2. **Machine Learning**: Anomaly detection via supervised learning (Python bridge)
3. **Swarm Coordination**: Distributed state across multiple Cerberus nodes
4. **Advanced CAPTCHAs**: hCaptcha/reCAPTCHA integration, puzzle challenges
5. **Behavioral Biometrics**: Keystroke dynamics, mouse patterns
6. **Adaptive Difficulty**: Real-time CAPTCHA complexity adjustment
7. **Admin Dashboard**: Web UI for monitoring, manual circuit management
8. **Metrics Export**: Prometheus endpoint for Grafana dashboards

---

## Critical Checklist

- [ ] CAPTCHA generation works (image + solution)
- [ ] CAPTCHA verification logic correct (case-insensitive, timeout)
- [ ] HAProxy socket client functional (promote/ban circuits)
- [ ] API endpoints handle errors gracefully (no panics)
- [ ] Circuit ID extraction from headers
- [ ] Concurrent request handling (async/await)
- [ ] Memory leak prevention (periodic cleanup)
- [ ] Logging configured (stdout or file)
- [ ] Security: No solution leakage in logs/responses
- [ ] Performance: <100ms CAPTCHA generation, <10ms verification

---

## References
- Axum Framework: https://github.com/tokio-rs/axum
- Captcha Crate: https://crates.io/crates/captcha
- Tokio Async Runtime: https://tokio.rs/
- HAProxy Runtime API: https://www.haproxy.com/documentation/hapee/latest/api/runtime-api/
- Rust Security Best Practices: https://anssi-fr.github.io/rust-guide/
