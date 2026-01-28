# 📖 User Story

**As a service operator under sophisticated bot attacks**  
**I want advanced image CAPTCHAs that resist AI solvers and paid solving services**  
**So that I can effectively block automated attacks without blocking legitimate Tor users**

**Acceptance Criteria:**
- Multiple CAPTCHA variants that combine different visual distortion techniques
- Text-based CAPTCHAs with multiple-choice answers to reduce friction
- Anti-solver mechanisms that detect solving patterns and adapt difficulty
- Server-side generation with no JavaScript dependency (Tor Browser Safest mode compatible)
- Sub-second generation time to avoid delaying legitimate users
- Configurable via Threat Dial integration for dynamic difficulty scaling

---

# Advanced CAPTCHA System

**Layer:** Layer 2 (Nginx) + Layer 3 (Fortify)  
**Status:** Planning  
**Dependencies:** Redis (session state), Image generation libraries  
**Related Docs:** [0101-layer2-nginx.md](0101-layer2-nginx.md), [0201-feature-threat-dial.md](0201-feature-threat-dial.md)

---

## Table of Contents

1. [Overview](#overview)
2. [Human-Cost Asymmetry Design Philosophy](#human-cost-asymmetry-design-philosophy)
3. [CAPTCHA Variants](#captcha-variants)
4. [Anti-Bypass Mechanisms](#anti-bypass-mechanisms)
5. [Multiple-Choice Text CAPTCHAs](#multiple-choice-text-captchas)
6. [Generation Pipeline](#generation-pipeline)
7. [Validation & Session Management](#validation--session-management)
8. [Threat Dial Integration](#threat-dial-integration)
9. [Performance Requirements](#performance-requirements)
10. [Security Considerations](#security-considerations)
11. [Implementation Phases](#implementation-phases)

---

## Overview

The Advanced CAPTCHA System provides **AI-resistant, human-solvable challenges** that work without JavaScript. Unlike traditional CAPTCHAs that rely on single techniques (distorted text only), this system combines **6 distinct variants** with adaptive difficulty to stay ahead of automated solvers.

### Design Goals

1. **Solver Resistance**: Defeat AI OCR, paid solving services (2Captcha, AntiCaptcha), and pattern recognition
2. **Human Solvable**: Average solve time < 10 seconds for legitimate users
3. **No JavaScript**: Full functionality in Tor Browser Safest mode
4. **Fast Generation**: < 200ms to generate CAPTCHA image + store session
5. **Adaptive Difficulty**: Scales with Threat Dial level (low threat = easier CAPTCHA)
6. **Accessibility**: Multiple-choice text options for users who struggle with images

### Key Challenges

- **AI OCR Advancement**: Modern AI can solve traditional distorted text with 90%+ accuracy
- **CAPTCHA Farms**: Paid services solve CAPTCHAs for $1-3 per 1000 challenges
- **Pattern Detection**: Bots learn CAPTCHA structure and adapt over time
- **User Friction**: Too difficult = legitimate users abandon session

---

## Human-Cost Asymmetry Design Philosophy

### The Golden Rule

**Make the cost of being wrong trivial for humans and expensive for bots.**

This is the fundamental design principle underlying all Cerberus defenses. Every challenge, every CAPTCHA, every rate limit is designed to:

- **For humans:** Gentle nudges, soft warnings, quick recovery
- **For bots:** Exponential costs, stateful penalties, compounding friction

### Core Principles

#### 1. Soft-Lock Escalation (Not Hard Blocks)

**Humans see nothing. Bots drown.**

```
Legitimate User Journey:
  Request 1: Instant access (no CAPTCHA)
  Request 2: Instant access
  Request 3: Instant access
  ...
  Request 100: Still instant (VIP status earned)

Bot Journey:
  Request 1: Instant access (honeypot)
  Request 2: Easy CAPTCHA (2 seconds to solve)
  Request 3: Medium CAPTCHA (5 seconds to solve)
  Request 4: Hard CAPTCHA (10 seconds to solve)
  Request 5: Multiple CAPTCHAs in sequence (30+ seconds)
  Request 6: 30-second timeout
  Request 7: 2-minute timeout
  Request 8: 5-minute timeout
  Request 9+: Banned for 30 minutes
```

**Why This Works:**
- First mistake costs nothing (could be human typo)
- Second mistake starts adding friction (small delay)
- Third mistake becomes expensive (hard CAPTCHA + delays)
- Bots optimizing for speed hit exponential walls
- Humans rarely make 3+ mistakes in sequence

---

#### 2. Gradual PoW Increases

**Start with zero PoW. Ramp up only when suspicious.**

```rust
fn calculate_pow_difficulty(circuit_metrics: &CircuitMetrics) -> u32 {
    let base_difficulty = 0;  // Start with NO PoW for clean circuits
    
    let suspicion_score = 
        circuit_metrics.request_rate * 10 +           // Fast requests = suspicious
        circuit_metrics.failed_captchas * 50 +        // Failed CAPTCHAs = very suspicious
        circuit_metrics.honeypot_triggers * 100;      // Honeypot hit = extremely suspicious
    
    match suspicion_score {
        0..=10 => 0,           // Clean: no PoW
        11..=50 => 12,         // Slightly suspicious: trivial PoW (~0.1 sec)
        51..=100 => 16,        // Suspicious: noticeable PoW (~1 sec)
        101..=200 => 20,       // Very suspicious: expensive PoW (~10 sec)
        _ => 24,               // Extremely suspicious: brutal PoW (~60 sec)
    }
}
```

**Impact:**
- Legitimate users: **Never see PoW** (suspicion score stays at 0)
- Slightly aggressive bots: Minor delays (1 second PoW)
- Aggressive bots: Major delays (60+ second PoW per request)
- Economic attack: Bot farm must spend CPU cycles that cost more than attack value

---

#### 3. Stateful Challenge Chains

**Remember everything. Build a behavioral profile.**

```rust
struct CircuitBehaviorProfile {
    // Timing patterns
    avg_time_between_requests: Duration,
    request_rate_variance: f64,           // Bots = low variance, humans = high
    
    // Interaction patterns
    captcha_solve_times: Vec<Duration>,   // Bots = consistent, humans = variable
    captcha_failures: Vec<CaptchaType>,   // What types do they fail?
    
    // Progression indicators
    unique_pages_visited: HashSet<String>, // Bots hit same endpoint, humans browse
    session_duration: Duration,            // Bots are fast, humans linger
    
    // Trust signals
    vip_promotions: u32,                   // Earned VIP status in past?
    xmr_payments: Vec<PaymentRecord>,      // Paid to skip queue?
    
    // Penalty accumulation
    total_penalty_score: i32,
    last_penalty_reset: Timestamp,
}
```

**Behavioral Scoring:**
```rust
fn should_challenge(profile: &CircuitBehaviorProfile) -> ChallengeLevel {
    // Good signals (reduce challenge)
    let trust_score = 
        profile.unique_pages_visited.len() as i32 * 5 +
        profile.vip_promotions as i32 * 50 +
        profile.xmr_payments.len() as i32 * 100;
    
    // Bad signals (increase challenge)
    let penalty_score = profile.total_penalty_score;
    
    let net_score = trust_score - penalty_score;
    
    match net_score {
        100.. => ChallengeLevel::None,        // Trusted: no challenge
        50..=99 => ChallengeLevel::Easy,      // Mostly trusted: easy CAPTCHA
        0..=49 => ChallengeLevel::Medium,     // Neutral: medium CAPTCHA
        -50..=-1 => ChallengeLevel::Hard,     // Suspicious: hard CAPTCHA
        ..-51 => ChallengeLevel::Multiple,    // Very suspicious: multiple CAPTCHAs
    }
}
```

**Why This Works:**
- Humans naturally accumulate trust signals (browse pages, linger, varied timing)
- Bots have uniform behavior (same timing, same pages, no variance)
- System learns: "This circuit behaves like human" vs "This circuit behaves like bot"
- Trust is portable across sessions (VIP status remembered for 7 days)

---

#### 4. Behavior Memory Across Sessions

**Track circuits across reconnections.**

```rust
// Store circuit fingerprint in Redis with 7-day TTL
fn generate_circuit_fingerprint(circuit_id: &str, user_agent: &str) -> String {
    let fingerprint_data = format!("{}{}", circuit_id, user_agent);
    blake3::hash(fingerprint_data.as_bytes()).to_string()
}

fn get_historical_behavior(fingerprint: &str) -> Option<CircuitBehaviorProfile> {
    redis_client.get(format!("circuit_history:{}", fingerprint))
}

// On new request:
let fingerprint = generate_circuit_fingerprint(circuit_id, user_agent);
if let Some(history) = get_historical_behavior(&fingerprint) {
    // Circuit has been here before
    if history.vip_promotions > 0 {
        // Previously earned VIP → fast-track now
        return allow_without_challenge();
    }
    if history.total_penalty_score > 500 {
        // Previously abusive → hard challenge now
        return require_hard_captcha();
    }
}
```

**Decay Mechanism:**
```rust
// Penalties decay over time (forgiveness)
fn decay_penalties(profile: &mut CircuitBehaviorProfile) {
    let time_since_last_penalty = now() - profile.last_penalty_reset;
    let decay_factor = (time_since_last_penalty.as_hours() as f64 / 24.0).min(1.0);
    
    profile.total_penalty_score = 
        (profile.total_penalty_score as f64 * (1.0 - decay_factor * 0.5)) as i32;
    
    // Full forgiveness after 7 days
    if time_since_last_penalty > Duration::days(7) {
        profile.total_penalty_score = 0;
    }
}
```

**Why This Works:**
- Good actors: Build reputation over time, challenges decrease
- Bad actors: Penalties remembered, must wait for forgiveness
- Tor circuit rotation: Doesn't help bots (fingerprint persists)
- Humans who make mistakes: Forgiven after time passes

---

### Real-World Example: Bot vs Human

**Scenario:** 10,000 requests to overwhelm service

**Bot Farm Attack:**
```
Request 1-100:   No challenge (0.1 sec each) = 10 seconds total
Request 101-200: Easy CAPTCHA (2 sec each) = 200 seconds total
Request 201-300: Medium CAPTCHA (5 sec each) = 500 seconds total
Request 301-400: Hard CAPTCHA (10 sec each) = 1000 seconds total
Request 401-500: Multiple CAPTCHAs (30 sec each) = 15,000 seconds total
Request 501+:    Banned (circuit burned, must rotate)

Total time for 500 requests: ~4.6 hours
Total time for 10,000 requests: ~920 hours (38 days) if circuit rotation allowed
Cost: $50-500 in CAPTCHA solving services + massive CPU time
```

**Legitimate Human:**
```
Request 1:    No challenge (instant)
Request 2:    No challenge (instant)
Request 3:    No challenge (instant)
...
Request 100:  VIP promotion earned (instant)
Request 101+: Instant access forever (trusted circuit)

Total time: Seconds (all instant)
Cost: $0
```

**Asymmetry Achieved:** 38 days for bot vs seconds for human.

---

## CAPTCHA Variants

### Variant 1: Multi-Layer Distorted Text

**Difficulty:** Medium  
**Solver Resistance:** Medium-High  
**Avg Solve Time:** 8-12 seconds

**Technique:**
- Base text with random font (from pool of 20+ fonts)
- Wave distortion (horizontal + vertical sine waves with random amplitude/frequency)
- Rotation: Each character rotated -30° to +30° individually
- Noise layer: Random lines, dots, and shapes overlaid
- Color variation: Text uses 3-5 random colors, background uses contrasting noise
- Character spacing: Random gaps between letters to break OCR word detection

**Example Generation:**
```
Text: "7K9PXM"
Font: Random from [Impact, Arial Black, Courier Bold, ...]
Wave: sin(x * 0.15) * 8px vertical, sin(y * 0.12) * 6px horizontal
Rotation: 7=+12°, K=-23°, 9=+8°, P=-15°, X=+25°, M=-9°
Noise: 150 random lines (1-3px thick), 300 dots
Colors: Text=#2A5FDD, #8B3A9C, #DD6B2A (rotated per char)
         Background=#F4F4F4 with #CCCCCC noise
```

**Anti-Solver Measures:**
- Character rotation breaks horizontal scanning OCR
- Multi-color text defeats grayscale normalization
- Wave distortion prevents grid-based segmentation
- Random noise forces AI to distinguish foreground from background

---

### Variant 2: Object Recognition + Text

**Difficulty:** High  
**Solver Resistance:** High  
**Avg Solve Time:** 12-18 seconds

**Technique:**
- Image contains 3-4 simple objects (car, tree, house, cat, etc.)
- Objects are partially occluded by distorted text overlay
- User must identify: "Type the word that appears over the CAR"
- Forces bot to solve two problems: object detection + OCR + spatial reasoning

**Example Generation:**
```
Objects: [car.png, tree.png, house.png] randomly placed
Overlay: "7XM" over car, "2PK" over tree, "9FL" over house
Question: "Type the letters over the TREE"
Answer: "2PK"
```

**Anti-Solver Measures:**
- Requires object detection AI (expensive to run)
- Occlusion makes both object recognition and OCR harder
- Spatial reasoning (which text is "over" which object) adds complexity
- Object pool of 50+ items prevents pattern memorization

---

### Variant 3: Pattern Completion

**Difficulty:** Medium  
**Solver Resistance:** Very High  
**Avg Solve Time:** 10-15 seconds

**Technique:**
- Show sequence of shapes/colors with one missing: `[●][■][●][■][?]`
- User types the missing shape: `circle`, `square`, or `triangle`
- Can use colors: `[Red][Blue][Red][Blue][?]` → answer: "Red"
- Abstract patterns harder for AI to learn without massive training data

**Example Generation:**
```
Pattern: Circle, Square, Circle, Square, [?]
Visual: ●  ■  ●  ■  ?
Answer: "square" (accept "square", "sq", "box")
```

**Anti-Solver Measures:**
- Infinite pattern variations (shapes, colors, alternating, incrementing, etc.)
- Requires reasoning, not just OCR
- Small training dataset available for bots (unlike text OCR with billions of examples)
- Can introduce red herrings: show 5 shapes but only first 4 form pattern

---

### Variant 4: Color-Text Mismatch

**Difficulty:** Medium  
**Solver Resistance:** High  
**Avg Solve Time:** 8-12 seconds

**Technique:**
- Show word "RED" but render it in BLUE color
- Ask: "What color is the TEXT?" (answer: blue) or "What does the WORD say?" (answer: red)
- Forces human cognitive processing (Stroop effect)
- OCR alone gives wrong answer

**Example Generation:**
```
Display: "RED" (rendered in blue color)
Question: "What COLOR is the text?"
Answer: "blue" (accept "blue", "blu", "azul")

OR

Display: "BLUE" (rendered in red color)
Question: "What WORD do you see?"
Answer: "blue"
```

**Anti-Solver Measures:**
- OCR + color detection both required
- Question variation forces bot to parse question semantics
- Cognitive load on human is low (< 10 sec) but bot must parse language + image

---

### Variant 5: Proof-of-Elapsed-Time (PoET)

**Difficulty:** Low (for humans)  
**Solver Resistance:** Very High  
**Avg Solve Time:** 4-8 seconds (enforced delay)

**Concept:** AI is fast. Humans are slow. Use that.

**Technique:**
- Server issues challenge token with embedded timestamp
- Token must be returned after **minimum 4 seconds** and **maximum 8 seconds**
- No visual puzzle, just a wait
- User sees: "Please wait... (countdown from 4)"
- Too fast → bot (solved via automated tool)
- Too slow → bot farm (queued for manual solving)
- Just right → human

**Example Flow:**
```
1. User requests page
2. Server generates token: 
   token = encrypt({"issued_at": 1738080000, "min_wait": 4, "max_wait": 8})
3. Page displays: "Please wait 4 seconds..."
   <form action="/verify" method="POST">
     <input type="hidden" name="token" value="abc123...">
     <button type="submit" id="submit-btn" disabled>Verifying...</button>
   </form>
   <meta http-equiv="refresh" content="4; url=javascript:document.getElementById('submit-btn').disabled=false;">
   
   OR (pure HTML, no JS):
   <meta http-equiv="refresh" content="4; url=/verify?token=abc123...">

4. Server validates:
   elapsed = now() - token.issued_at
   if elapsed < 4 seconds: REJECT (too fast = bot)
   if elapsed > 8 seconds: REJECT (too slow = bot farm)
   if 4 <= elapsed <= 8: ACCEPT (human timing)
```

**Anti-Solver Measures:**
- **Bots optimize for speed**: They want to solve CAPTCHAs instantly. PoET forces 4+ second wait.
- **Solver farms add latency**: Human solvers in overseas farms take 10-30 seconds (receive task, solve, return). PoET rejects > 8 seconds.
- **Replay attacks fail**: Token contains timestamp, single-use only (deleted from Redis after validation)
- **Parallelism collapses**: Bot must wait 4 seconds per request. 10,000 requests = 11+ hours minimum.

**Why Humans Pass Naturally:**
- 4-8 seconds is normal page load time
- User sees "Verifying..." (thinks it's processing, not suspicious)
- No cognitive load (no puzzle to solve)
- Accessible (works for blind users, no vision required)

**Implementation:**
```rust
fn validate_poet_token(token: &str, submitted_at: Timestamp) -> Result<(), PoETError> {
    // Decrypt token
    let token_data: PoETToken = decrypt_token(token)?;
    
    // Check if already used (replay attack)
    if redis_client.exists(format!("poet_used:{}", token)).unwrap_or(false) {
        return Err(PoETError::AlreadyUsed);
    }
    
    // Calculate elapsed time
    let elapsed = submitted_at - token_data.issued_at;
    
    // Validate timing window
    if elapsed < token_data.min_wait {
        log_suspicious_activity("PoET too fast", elapsed.as_secs());
        return Err(PoETError::TooFast);
    }
    
    if elapsed > token_data.max_wait {
        log_suspicious_activity("PoET too slow", elapsed.as_secs());
        return Err(PoETError::TooSlow);
    }
    
    // Mark token as used
    redis_client.set_ex(format!("poet_used:{}", token), "1", 300).unwrap();
    
    Ok(())
}
```

**Adaptive Timing:**
```rust
// Adjust timing window based on Threat Dial
fn get_poet_timing(threat_level: u8) -> (u64, u64) {
    match threat_level {
        1..=2 => (2, 10),   // Lenient: 2-10 seconds
        3..=5 => (4, 8),    // Normal: 4-8 seconds
        6..=8 => (5, 7),    // Strict: 5-7 seconds (narrow window)
        9..=10 => (6, 6),   // Extreme: exactly 6 seconds (very hard for bots)
        _ => (4, 8),
    }
}
```

**No JavaScript Required:**
```html
<!-- Pure HTML solution with meta refresh -->
<html>
<head>
  <meta http-equiv="refresh" content="4; url=/verify?token=abc123...">
  <title>Verifying...</title>
</head>
<body>
  <h1>Please wait while we verify your request...</h1>
  <p>This page will automatically continue in 4 seconds.</p>
  <p>Do not refresh or close this page.</p>
</body>
</html>
```

---

### Variant 6: Single-Use Interaction Puzzles (Contextual Continuity)

**Difficulty:** Low  
**Solver Resistance:** High  
**Avg Solve Time:** 5-10 seconds

**Concept:** Test memory and contextual understanding without images or ML.

**Technique:**
- **Page 1:** Display a random word: "Your access word is **EMBER**"
- **Page 2:** Ask a question about it: "Type the 2nd letter of the word you were given"
- **Answer:** "M"

**Why This Works:**
- **No images:** Can't use OCR or visual ML
- **No complex ML:** Simple text parsing, but...
- **Breaks headless relays:** Bot must maintain state across two page loads
- **Breaks CAPTCHA farms:** Human solver sees Page 1, but by the time they submit answer for Page 2, session expired (5 min TTL)
- **Parallelism collapses:** Each session needs unique word, bot must track thousands of sessions

**Example Variations:**

**1. Letter Position:**
```
Page 1: "Your access word is EMBER"
Page 2: "Type the 2nd letter of the word you were given"
Answer: M
```

**2. Reverse Spelling:**
```
Page 1: "Your access word is ROBOT"
Page 2: "Type your access word backwards"
Answer: TOBOR
```

**3. Letter Count:**
```
Page 1: "Your access word is CASTLE"
Page 2: "How many letters are in your access word?"
Answer: 6
```

**4. First/Last Letter:**
```
Page 1: "Your access word is THUNDER"
Page 2: "Type the first and last letter of your access word (no space)"
Answer: TR
```

**5. Color Association:**
```
Page 1: "Your access color is BLUE"
Page 2: "What color were you given? Type it backwards."
Answer: EULB
```

**Implementation:**
```rust
fn generate_interaction_puzzle() -> (String, InteractionPuzzle) {
    let words = vec!["EMBER", "ROBOT", "CASTLE", "THUNDER", "CRIMSON", "FALCON", "GRANITE"];
    let word = words[rand::random::<usize>() % words.len()];
    
    let puzzle_type = rand::random::<u8>() % 5;
    
    let (question, answer_fn): (String, Box<dyn Fn(&str) -> String>) = match puzzle_type {
        0 => {
            let pos = rand::random::<usize>() % word.len() + 1;
            (
                format!("Type the {} letter of the word you were given", ordinal(pos)),
                Box::new(move |w: &str| w.chars().nth(pos - 1).unwrap().to_string())
            )
        },
        1 => (
            "Type your access word backwards".to_string(),
            Box::new(|w: &str| w.chars().rev().collect())
        ),
        2 => (
            "How many letters are in your access word?".to_string(),
            Box::new(|w: &str| w.len().to_string())
        ),
        3 => (
            "Type the first and last letter (no space)".to_string(),
            Box::new(|w: &str| format!("{}{}", w.chars().next().unwrap(), w.chars().last().unwrap()))
        ),
        _ => (
            "Type your access word in lowercase".to_string(),
            Box::new(|w: &str| w.to_lowercase())
        ),
    };
    
    let token = generate_token();
    let expected_answer = answer_fn(word);
    
    // Store in Redis with 5-minute TTL
    redis_client.set_ex(
        format!("interaction_puzzle:{}", token),
        serde_json::to_string(&InteractionPuzzle {
            word: word.to_string(),
            expected_answer,
            issued_at: now(),
        }).unwrap(),
        300  // 5 min TTL
    ).unwrap();
    
    let page1 = format!("Your access word is <strong>{}</strong>", word);
    
    (page1, InteractionPuzzle { word: word.to_string(), question, token })
}
```

**Page Flow:**
```html
<!-- Page 1: Display word -->
<html>
<body>
  <h1>Step 1 of 2</h1>
  <p>Your access word is: <strong style="font-size: 2em; color: #2A5FDD;">EMBER</strong></p>
  <p>Please remember this word.</p>
  <form action="/verify-interaction" method="GET">
    <input type="hidden" name="token" value="abc123...">
    <button type="submit">Continue</button>
  </form>
</body>
</html>

<!-- Page 2: Ask question -->
<html>
<body>
  <h1>Step 2 of 2</h1>
  <p>Type the 2nd letter of the word you were given:</p>
  <form action="/submit-interaction" method="POST">
    <input type="hidden" name="token" value="abc123...">
    <input type="text" name="answer" required autofocus>
    <button type="submit">Submit</button>
  </form>
</body>
</html>
```

**Validation:**
```rust
fn validate_interaction_puzzle(token: &str, submitted_answer: &str) -> Result<(), CaptchaError> {
    // Lookup puzzle in Redis
    let puzzle: InteractionPuzzle = redis_client.get_del(format!("interaction_puzzle:{}", token))
        .map_err(|_| CaptchaError::ExpiredOrInvalid)?;
    
    // Check timing (reject if > 5 min)
    let elapsed = now() - puzzle.issued_at;
    if elapsed > Duration::minutes(5) {
        return Err(CaptchaError::Expired);
    }
    
    // Validate answer (case-insensitive)
    if submitted_answer.to_uppercase() == puzzle.expected_answer.to_uppercase() {
        Ok(())
    } else {
        Err(CaptchaError::WrongAnswer)
    }
}
```

**Why Bots Struggle:**
1. **State management**: Must remember word from Page 1 → Page 2
2. **Session tracking**: Each session has unique word (no pattern to learn)
3. **Timing pressure**: 5-minute TTL means solver farms can't queue tasks
4. **No ML advantage**: AI can solve it, but not faster/cheaper than other CAPTCHAs
5. **Breaks parallelism**: 10,000 bots = must track 10,000 unique words in memory

**Why Humans Pass:**
- Simple memory task (remember one word for 10 seconds)
- No visual processing required (accessible)
- Fast (5-10 seconds total)
- Low cognitive load ("What was the 2nd letter?" is easy)

---

## Anti-Bypass Mechanisms

### 1. Randomized Variant Selection

**Problem:** Bots train on specific CAPTCHA structure  
**Solution:** Randomly select variant per request weighted by Threat Dial level

```
Threat Level 1-2: 40% Variant 5 (PoET), 30% Variant 6 (interaction puzzle), 30% Variant 1 (distorted text)
Threat Level 3-5: 30% Variant 1, 25% Variant 6, 20% Variant 3, 15% Variant 5, 10% Variant 2
Threat Level 6-8: 30% Variant 2, 25% Variant 3, 20% Variant 4, 15% Variant 1, 10% Variant 6
Threat Level 9-10: 40% Variant 2, 30% Variant 4, 20% Variant 3, 10% Variant 1 (no easy options)
```

**Impact:** Bot cannot specialize for one CAPTCHA type

---

### 2. Solve-Time Analysis

**Problem:** AI solvers consistently solve in < 2 seconds (faster than humans)  
**Solution:** Track solve time per session, flag suspicious patterns

```rust
// Track solve times in Redis
struct SolveMetrics {
    times: Vec<u64>,        // Last 10 solve times in ms
    failures: u8,           // Failed attempts
    first_attempt: bool,    // Solved on first try?
}

fn is_suspicious(metrics: &SolveMetrics) -> bool {
    let avg_time = metrics.times.iter().sum::<u64>() / metrics.times.len() as u64;
    
    // Suspicion triggers:
    avg_time < 2000 ||                      // Consistently < 2 sec
    metrics.failures == 0 && metrics.times.len() > 5 ||  // Never fails
    metrics.times.iter().all(|&t| t < 3000)  // All solve < 3 sec
}
```

**Action on Suspicion:**
- Increase CAPTCHA difficulty (force Variant 2 or 3)
- Require multiple CAPTCHAs in sequence
- Rate limit this circuit (add to HAProxy penalty score)

---

### 3. Answer Timing & Keystroke Dynamics

**Problem:** Bots paste answers instantly, humans type gradually  
**Solution:** Track time between page load and first input, input patterns

```html
<!-- Hidden fields track timing (no JS, uses form submission timestamps) -->
<input type="hidden" name="page_load_time" value="1738080000123">
<input type="hidden" name="captcha_displayed_time" value="1738080001456">

<!-- On form submit, calculate: -->
time_to_first_input = submit_time - captcha_displayed_time
```

**Suspicious Patterns:**
- Answer submitted < 500ms after page load (bot pre-solved or pasting)
- Answer arrives exactly X seconds after load (scripted delay)

**Action:** Flag as suspicious, require second CAPTCHA

---

### 4. Session Entropy & Rate Limiting

**Problem:** CAPTCHA farms create thousands of sessions to solve CAPTCHAs in parallel  
**Solution:** Limit CAPTCHA requests per circuit, track failure patterns

```
Per Tor circuit limits:
- Max 3 CAPTCHA failures before 30-second timeout
- Max 10 CAPTCHAs requested per hour (even if all solved correctly)
- Each failure increases timeout exponentially: 30s, 2min, 5min, 15min

Per hidden service:
- Max 1000 CAPTCHAs generated per minute (prevents resource exhaustion)
- If > 500 CAPTCHAs active simultaneously, trigger Threat Dial increase
```

---

### 5. Answer Fuzzing & Honeypots

**Problem:** Bots may brute-force or use solving services  
**Solution:** Accept multiple answer formats, add decoy fields

**Fuzzy Matching:**
```rust
fn is_correct_answer(submitted: &str, expected: &str) -> bool {
    let normalized_submit = submitted.to_lowercase().trim();
    let normalized_expect = expected.to_lowercase();
    
    // Accept exact match
    if normalized_submit == normalized_expect {
        return true;
    }
    
    // Accept common typos for text answers
    let distance = levenshtein_distance(normalized_submit, normalized_expect);
    if distance <= 1 && normalized_expect.len() > 3 {
        return true;  // Allow 1 character off
    }
    
    // Accept synonyms (e.g., "blue" = "blu" = "azul")
    COLOR_SYNONYMS.get(normalized_expect)
        .map(|syns| syns.contains(&normalized_submit))
        .unwrap_or(false)
}
```

**Honeypot Fields:**
```html
<!-- Hidden field that humans won't see (CSS display:none) -->
<input type="text" name="email" value="" style="display:none;">

<!-- If this field is filled, it's a bot (bots fill all fields) -->
```

---

## Multiple-Choice Text CAPTCHAs

### Concept

Instead of typing text (prone to typos), users select from pre-defined choices. Reduces friction while maintaining security.

### Example 1: Visual Question

**Display Image:** Picture of 3 objects (apple, car, dog)  
**Question:** "Which object is RED?"  
**Choices:**
- [ ] Apple
- [ ] Car
- [ ] Dog

**Benefits:**
- No typing required (click radio button or type "A", "B", "C")
- Still requires visual recognition + reasoning
- Harder for bots (must solve image recognition)

---

### Example 2: Distorted Text Recognition

**Display Image:** Distorted text "7K9PXM"  
**Question:** "Which code is shown?"  
**Choices:**
- [ ] 7K9PXM ✓ (correct)
- [ ] 7K9PXN (decoy: one letter off)
- [ ] 7K9QXM (decoy: visually similar character)
- [ ] 7K8PXM (decoy: one number off)

**Benefits:**
- User reads text, selects match (easier than typing)
- Decoys are visually similar (user must read carefully)
- Bot cannot brute force (1 in 4 chance, but wrong answer = penalty)

---

### Example 3: Pattern Completion

**Display:** ● ■ ● ■ ?  
**Question:** "What comes next?"  
**Choices:**
- [ ] Circle ●
- [ ] Square ■
- [ ] Triangle ▲

**Benefits:**
- Pure reasoning, no OCR needed
- Accessible (screen readers can read shapes)
- Bot needs pattern recognition AI

---

### Implementation

```html
<form method="POST" action="/validate-captcha">
    <img src="/captcha/<token>.png" alt="CAPTCHA challenge">
    <p>Which object is RED?</p>
    
    <label><input type="radio" name="answer" value="apple"> Apple</label><br>
    <label><input type="radio" name="answer" value="car"> Car</label><br>
    <label><input type="radio" name="answer" value="dog"> Dog</label><br>
    
    <input type="hidden" name="token" value="<token>">
    <button type="submit">Submit</button>
</form>
```

**Decoy Selection Strategy:**
- Always include 3-4 decoys per question
- Decoys must be plausible (not obviously wrong)
- Randomize choice order (correct answer not always "A")
- Track which decoys are most commonly selected (tune difficulty)

---

## Generation Pipeline

### Architecture

```
┌─────────────────────────────────────────────────────────┐
│ Nginx (Layer 2)                                         │
│ ┌─────────────────────────────────────────────────────┐ │
│ │ 1. Intercept request → needs CAPTCHA?               │ │
│ │ 2. POST /captcha/generate → Fortify                 │ │
│ │ 3. Receive CAPTCHA token + metadata                 │ │
│ │ 4. Serve HTML form with embedded image              │ │
│ └─────────────────────────────────────────────────────┘ │
└───────────────────────┬─────────────────────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────────┐
│ Fortify (Layer 3)                                       │
│ ┌─────────────────────────────────────────────────────┐ │
│ │ CAPTCHA Generator Module                            │ │
│ │                                                     │ │
│ │ 1. Select variant based on Threat Dial level       │ │
│ │ 2. Generate image (distortion, noise, etc.)        │ │
│ │ 3. Store answer in Redis with TTL (5 min)          │ │
│ │ 4. Return token + image bytes                      │ │
│ └─────────────────────────────────────────────────────┘ │
│                                                          │
│ ┌─────────────────────────────────────────────────────┐ │
│ │ CAPTCHA Validator Module                            │ │
│ │                                                     │ │
│ │ 1. Receive token + user answer                     │ │
│ │ 2. Lookup expected answer in Redis                 │ │
│ │ 3. Validate (fuzzy match, timing checks)           │ │
│ │ 4. Update solve metrics (time, failures)           │ │
│ │ 5. Issue session cookie OR reject                  │ │
│ └─────────────────────────────────────────────────────┘ │
└───────────────────────┬─────────────────────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────────┐
│ Redis                                                    │
│ ┌─────────────────────────────────────────────────────┐ │
│ │ Key: captcha:<token>                                │ │
│ │ Value: {                                            │ │
│ │   "answer": "7K9PXM",                               │ │
│ │   "variant": "distorted_text",                      │ │
│ │   "created_at": 1738080000,                         │ │
│ │   "circuit_id": "abc123...",                        │ │
│ │   "ttl": 300                                        │ │
│ │ }                                                   │ │
│ └─────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
```

---

### Generation Steps (Variant 1: Distorted Text)

```rust
use image::{ImageBuffer, Rgb, RgbImage};
use imageproc::drawing::draw_text_mut;
use rusttype::{Font, Scale};
use rand::Rng;

fn generate_distorted_text_captcha(text: &str, threat_level: u8) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    
    // Step 1: Create base canvas
    let width = 300;
    let height = 100;
    let mut img: RgbImage = ImageBuffer::new(width, height);
    
    // Step 2: Fill background with noise
    for y in 0..height {
        for x in 0..width {
            let noise = rng.gen_range(200..255);
            img.put_pixel(x, y, Rgb([noise, noise, noise]));
        }
    }
    
    // Step 3: Draw distorted text
    let font = load_random_font();
    let scale = Scale::uniform(40.0 + (threat_level as f32 * 2.0));
    
    for (i, ch) in text.chars().enumerate() {
        let x = 30 + (i as u32 * 35);
        let y = 40;
        
        // Apply rotation and wave distortion
        let rotation = rng.gen_range(-30.0..30.0);
        let color = random_color();
        
        draw_rotated_char(&mut img, ch, x, y, rotation, color, &font, scale);
    }
    
    // Step 4: Add noise lines
    let num_lines = 50 + (threat_level as u32 * 10);
    for _ in 0..num_lines {
        let x1 = rng.gen_range(0..width);
        let y1 = rng.gen_range(0..height);
        let x2 = rng.gen_range(0..width);
        let y2 = rng.gen_range(0..height);
        draw_line(&mut img, x1, y1, x2, y2, random_color());
    }
    
    // Step 5: Encode as PNG
    let mut buffer = Vec::new();
    img.write_to(&mut buffer, image::ImageOutputFormat::Png).unwrap();
    buffer
}
```

---

## Validation & Session Management

### Flow

```
1. User submits CAPTCHA form with token + answer
2. Nginx forwards to Fortify /validate-captcha endpoint
3. Fortify:
   a. Lookup token in Redis
   b. Check if expired (TTL = 5 min)
   c. Validate answer (fuzzy match)
   d. Check solve metrics (time, failure rate)
   e. If valid → issue session cookie
   f. If invalid → increment failure count, return error
4. Nginx receives validation response:
   a. If valid → proxy request to backend with session cookie
   b. If invalid → re-render CAPTCHA form with error message
```

### Session Cookie

```
Name: cerberus_session
Value: <signed JWT token>
Max-Age: 3600 (1 hour)
Secure: true (HTTPS only)
HttpOnly: true (no JS access)
SameSite: Strict

JWT Payload:
{
  "circuit_id": "abc123...",
  "issued_at": 1738080000,
  "expires_at": 1738083600,
  "threat_level": 5,
  "captcha_solved": true
}
```

**Security:**
- Signed with HMAC-SHA256 secret (prevents tampering)
- Short TTL (1 hour, configurable)
- Tied to circuit ID (cannot share between circuits)
- Revocable via Redis blacklist if abuse detected

---

### Failure Handling

```rust
fn handle_failed_captcha(circuit_id: &str, token: &str) -> CaptchaResponse {
    let mut metrics = get_solve_metrics(circuit_id);
    metrics.failures += 1;
    
    // Exponential backoff for repeated failures
    let timeout = match metrics.failures {
        1..=2 => 0,           // No timeout, just re-render
        3 => 30,              // 30 seconds
        4 => 120,             // 2 minutes
        5 => 300,             // 5 minutes
        _ => 900,             // 15 minutes (max)
    };
    
    if timeout > 0 {
        set_circuit_timeout(circuit_id, timeout);
        return CaptchaResponse::RateLimited { retry_after: timeout };
    }
    
    // Increase difficulty after 2 failures
    let new_difficulty = if metrics.failures >= 2 {
        DifficultyLevel::Hard  // Force Variant 2 or 3
    } else {
        DifficultyLevel::Medium
    };
    
    let new_token = generate_captcha(circuit_id, new_difficulty);
    CaptchaResponse::Retry {
        token: new_token,
        message: "Incorrect answer. Please try again.",
        failures: metrics.failures,
    }
}
```

---

## Threat Dial Integration

### Automatic Difficulty Scaling

```rust
fn select_captcha_variant(threat_level: u8) -> CaptchaVariant {
    match threat_level {
        1..=2 => {
            // Low threat: easy CAPTCHAs, 80% arithmetic
            weighted_random(&[
                (CaptchaVariant::Arithmetic, 80),
                (CaptchaVariant::DistortedText, 20),
            ])
        },
        3..=5 => {
            // Medium threat: balanced mix
            weighted_random(&[
                (CaptchaVariant::DistortedText, 50),
                (CaptchaVariant::PatternCompletion, 30),
                (CaptchaVariant::ObjectRecognition, 20),
            ])
        },
        6..=8 => {
            // High threat: hard CAPTCHAs only
            weighted_random(&[
                (CaptchaVariant::ObjectRecognition, 40),
                (CaptchaVariant::PatternCompletion, 40),
                (CaptchaVariant::ColorTextMismatch, 20),
            ])
        },
        9..=10 => {
            // Critical threat: hardest + multi-CAPTCHA
            weighted_random(&[
                (CaptchaVariant::ObjectRecognition, 50),
                (CaptchaVariant::ColorTextMismatch, 30),
                (CaptchaVariant::PatternCompletion, 20),
            ])
        },
        _ => CaptchaVariant::DistortedText,  // Fallback
    }
}
```

### Multi-CAPTCHA Challenge (Threat 9-10)

At highest threat levels, require **2-3 CAPTCHAs in sequence**:

```
1. User solves first CAPTCHA → issues "captcha_stage_1" cookie
2. User solves second CAPTCHA → issues "captcha_stage_2" cookie
3. Both cookies required to pass → issues "cerberus_session" cookie
```

**Benefit:** Even if bot solves one CAPTCHA, must solve multiple in sequence (multiplies difficulty)

---

## Performance Requirements

### Generation Time

| Variant | Target Time | Max Acceptable |
|---------|-------------|----------------|
| Distorted Text | 50-100ms | 200ms |
| Object Recognition | 100-150ms | 300ms |
| Pattern Completion | 20-50ms | 100ms |
| Arithmetic Visual | 30-80ms | 150ms |
| Color-Text Mismatch | 40-90ms | 180ms |

**Optimization Strategies:**
- Pre-load fonts into memory at startup (avoid disk I/O per request)
- Use image generation libraries with SIMD optimizations (e.g., `image` crate with AVX2)
- Cache random noise patterns (generate 100 noise layers at startup, rotate usage)
- Parallel generation: if multi-CAPTCHA required, generate both simultaneously

---

### Resource Limits

```
Max concurrent CAPTCHA generations: 100
Max CAPTCHAs per second (global): 1000
Max Redis memory for CAPTCHA sessions: 512 MB (≈ 100k active sessions)
Image size limit: 50 KB per CAPTCHA (PNG with compression)
```

**Circuit Breaker:** If generation time exceeds 500ms for 10 consecutive requests, temporarily downgrade to simpler variants (Pattern Completion) to recover.

---

## Security Considerations

### 1. Token Security

**Problem:** Attacker could generate valid tokens without solving CAPTCHA  
**Solution:** Tokens are cryptographically random (32 bytes = 256 bits), stored in Redis with answer

```rust
use rand::Rng;
use base64::{Engine as _, engine::general_purpose};

fn generate_captcha_token() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 32] = rng.gen();
    general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}
```

**Token cannot be predicted or brute-forced** (2^256 possibilities)

---

### 2. Replay Attack Prevention

**Problem:** Attacker solves one CAPTCHA, reuses valid answer  
**Solution:** Token is single-use, deleted from Redis after validation

```rust
fn validate_captcha(token: &str, answer: &str) -> Result<(), CaptchaError> {
    // Atomic get-and-delete to prevent replay
    let expected = redis_client.get_del(format!("captcha:{}", token))?;
    
    if expected.is_none() {
        return Err(CaptchaError::ExpiredOrInvalid);
    }
    
    if !is_correct_answer(answer, &expected.unwrap()) {
        return Err(CaptchaError::WrongAnswer);
    }
    
    Ok(())
}
```

---

### 3. Timing Attack Resistance

**Problem:** Attacker measures response time to guess answer correctness  
**Solution:** Constant-time comparison for validation

```rust
use subtle::ConstantTimeEq;

fn is_correct_answer(submitted: &str, expected: &str) -> bool {
    let submitted_bytes = submitted.as_bytes();
    let expected_bytes = expected.as_bytes();
    
    // Pad to same length to avoid timing leak
    let max_len = submitted_bytes.len().max(expected_bytes.len());
    let mut sub_padded = vec![0u8; max_len];
    let mut exp_padded = vec![0u8; max_len];
    
    sub_padded[..submitted_bytes.len()].copy_from_slice(submitted_bytes);
    exp_padded[..expected_bytes.len()].copy_from_slice(expected_bytes);
    
    bool::from(sub_padded.ct_eq(&exp_padded))
}
```

---

### 4. Resource Exhaustion Defense

**Problem:** Attacker requests thousands of CAPTCHAs to exhaust memory/CPU  
**Solution:** Rate limits + Redis TTL

```
Per circuit:
- Max 10 CAPTCHA requests per minute
- Max 3 active CAPTCHA tokens simultaneously (older ones auto-deleted)

Global:
- Max 1000 CAPTCHA generations per second
- Max 100k active CAPTCHA tokens in Redis (oldest auto-expire)
```

---

## Implementation Phases

### Phase 1: Core Infrastructure (Week 1-2)

**Goal:** Basic CAPTCHA generation + validation pipeline

- [ ] Implement Variant 1 (Distorted Text) with basic distortion
- [ ] Redis integration for token storage
- [ ] Nginx endpoint to serve CAPTCHA form
- [ ] Fortify generation + validation modules
- [ ] Session cookie issuance on successful solve
- [ ] Unit tests for generation + validation logic

**Deliverables:**
- Functional distorted text CAPTCHA
- End-to-end flow: request → CAPTCHA → validate → session cookie

---

### Phase 2: Advanced Variants (Week 3-4)

**Goal:** Implement all 5 CAPTCHA variants

- [ ] Variant 2: Object Recognition + Text
- [ ] Variant 3: Pattern Completion
- [ ] Variant 4: Arithmetic with Visual Operators
- [ ] Variant 5: Color-Text Mismatch
- [ ] Weighted random selection based on threat level
- [ ] Performance benchmarks for each variant (< 200ms target)

**Deliverables:**
- All 5 variants functional
- Automatic variant selection tied to Threat Dial

---

### Phase 3: Anti-Bypass Mechanisms (Week 5-6)

**Goal:** Implement solver detection and mitigation

- [ ] Solve-time analysis (flag < 2 sec solves)
- [ ] Keystroke timing tracking (detect paste attacks)
- [ ] Session entropy tracking (detect CAPTCHA farms)
- [ ] Failure rate analysis + exponential backoff
- [ ] Honeypot fields in CAPTCHA form
- [ ] Answer fuzzing with Levenshtein distance

**Deliverables:**
- Bot detection heuristics active
- Suspicious circuits automatically escalated to harder CAPTCHAs

---

### Phase 4: Multiple-Choice UX (Week 7)

**Goal:** Reduce user friction with multiple-choice options

- [ ] Implement multiple-choice rendering for Variants 1, 3, 4
- [ ] Decoy answer generation (visually similar to correct answer)
- [ ] Randomize choice order per CAPTCHA
- [ ] A/B test: measure solve times (freeform vs multiple-choice)
- [ ] User survey: gather feedback on CAPTCHA difficulty

**Deliverables:**
- Multiple-choice option available for text-based CAPTCHAs
- Data on user preference (typing vs clicking)

---

### Phase 5: Threat Dial Integration (Week 8)

**Goal:** Dynamic difficulty scaling based on attack intensity

- [ ] Hook into Threat Dial state (read current level from Redis)
- [ ] Adjust variant selection weights per threat level
- [ ] Multi-CAPTCHA challenges at threat 9-10
- [ ] Automatic difficulty increase when suspicious patterns detected
- [ ] Dashboard metrics: CAPTCHA solve rates per threat level

**Deliverables:**
- CAPTCHA difficulty scales automatically with Threat Dial
- Operators can see CAPTCHA effectiveness in Monitoring UI

---

### Phase 6: Testing & Hardening (Week 9-10)

**Goal:** Validate solver resistance and performance

- [ ] Load testing: 1000 CAPTCHA generations per second
- [ ] Security audit: attempt to bypass with AI OCR (GPT-4 Vision, Google Cloud Vision API)
- [ ] Test against paid solving services (2Captcha, AntiCaptcha)
- [ ] Measure false positive rate (legitimate users failing CAPTCHA)
- [ ] Performance profiling: optimize generation time to < 100ms median
- [ ] Documentation: operator guide for tuning CAPTCHA difficulty

**Deliverables:**
- CAPTCHA withstands AI OCR testing (< 20% solve rate for bots)
- Sub-100ms generation time for all variants
- < 5% false positive rate (legitimate users blocked)

---

## Success Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Bot solve rate | < 20% | Test with GPT-4 Vision, Google Cloud Vision |
| Human solve rate | > 95% | User testing, A/B test with real traffic |
| Avg solve time (human) | 8-12 seconds | Track time from display → submit |
| Generation time | < 100ms (p50), < 200ms (p99) | Prometheus metrics |
| False positive rate | < 5% | Track legitimate users failing CAPTCHA |
| Paid solver cost | > $5 per 1000 | Make solving economically unviable |

---

## Open Questions

1. **Font Licensing:** Need to bundle 20+ fonts for distortion. Use open-source fonts (OFL license)?
2. **Accessibility:** Should we add audio CAPTCHA for visually impaired users, or is multiple-choice text sufficient?
3. **Localization:** Support non-English languages for text-based CAPTCHAs? (Pattern completion is language-agnostic)
4. **Honeypot Ethics:** Is it deceptive to include hidden fields that trap bots? (Industry standard practice)
5. **CAPTCHA Fatigue:** At threat level 10, users may see 2-3 CAPTCHAs. Is this acceptable UX? (Alternative: stricter rate limiting)

---

## References

- **CAPTCHA Security Analysis:** https://arxiv.org/abs/2312.12327
- **AI CAPTCHA Solvers:** https://2captcha.com/2captcha-api
- **Stroop Effect:** https://en.wikipedia.org/wiki/Stroop_effect
- **Image Distortion Techniques:** https://docs.rs/imageproc/latest/imageproc/
- **Levenshtein Distance:** https://en.wikipedia.org/wiki/Levenshtein_distance
