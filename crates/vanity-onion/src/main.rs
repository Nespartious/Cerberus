//! # Vanity Onion Address Generator
//!
//! Generates Tor v3 (.onion) addresses with a custom prefix (vanity addresses).
//!
//! ## How Tor v3 Addresses Work
//! ```text
//! onion_address = base32(pubkey || checksum || version)
//! 
//! Where:
//! - pubkey: 32-byte Ed25519 public key
//! - checksum: first 2 bytes of SHA3-256(".onion checksum" || pubkey || version)
//! - version: 0x03 (v3)
//! ```
//!
//! ## Usage
//! ```bash
//! # Generate address starting with "sigil"
//! vanity-onion --prefix sigil
//!
//! # Use all CPU cores and save to file
//! vanity-onion --prefix sigil --threads 0 --output keys/
//! ```

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::Parser;
use ed25519_dalek::{SigningKey, VerifyingKey};
use indicatif::{ProgressBar, ProgressStyle};
use rand::rngs::OsRng;
use rayon::prelude::*;
use sha2::{Digest as Sha2Digest, Sha512};
use sha3::{Digest, Sha3_256};

/// Cerberus Vanity Onion Address Generator
#[derive(Parser, Debug)]
#[command(name = "vanity-onion")]
#[command(author, version, about = "Generate branded .onion addresses", long_about = None)]
struct Args {
    /// Prefix to search for (case-insensitive, base32 chars only: a-z, 2-7)
    #[arg(short, long)]
    prefix: String,

    /// Number of threads (0 = auto-detect)
    #[arg(short, long, default_value = "0")]
    threads: usize,

    /// Output directory for keys
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Number of addresses to generate (stop after finding this many)
    #[arg(short, long, default_value = "1")]
    count: usize,

    /// Maximum attempts before giving up (0 = unlimited)
    #[arg(long, default_value = "0")]
    max_attempts: u64,

    /// Maximum time in seconds before giving up (0 = unlimited)
    #[arg(long, default_value = "0")]
    timeout: u64,

    /// Show estimated time and difficulty
    #[arg(long)]
    estimate: bool,

    /// Test mode: if prefix too long, auto-shorten for faster testing
    #[arg(long)]
    test_mode: bool,
}

/// Tor v3 onion address version byte
const ONION_V3_VERSION: u8 = 0x03;

/// Checksum prefix for Tor v3
const CHECKSUM_PREFIX: &[u8] = b".onion checksum";

fn main() {
    let args = Args::parse();

    // Validate prefix (base32 only: a-z, 2-7)
    let mut prefix = args.prefix.to_lowercase();
    if !prefix.chars().all(|c| c.is_ascii_lowercase() || ('2'..='7').contains(&c)) {
        eprintln!("Error: Prefix must contain only base32 characters (a-z, 2-7)");
        eprintln!("       Invalid characters will never match");
        std::process::exit(1);
    }

    // Test mode: shorten prefix if too long for fast testing
    if args.test_mode && prefix.len() > 3 {
        let original = prefix.clone();
        prefix = prefix[..3].to_string();
        println!("⚡ TEST MODE: Shortened prefix '{}' → '{}' for faster generation", original, prefix);
        println!();
    }

    // Calculate difficulty
    let difficulty = 32u64.pow(prefix.len() as u32);
    let expected_attempts = difficulty; // ~50% chance after this many

    println!("🔍 Vanity Onion Generator");
    println!("========================");
    println!("Prefix: {}", prefix);
    println!("Difficulty: 1 in {}", format_number(difficulty));
    println!("Expected attempts: ~{}", format_number(expected_attempts));
    
    if args.max_attempts > 0 {
        println!("Max attempts: {}", format_number(args.max_attempts));
    }
    if args.timeout > 0 {
        println!("Timeout: {}s", args.timeout);
    }

    if args.estimate {
        // Benchmark first
        let rate = benchmark_rate();
        let eta_secs = expected_attempts / rate.max(1);
        println!("Estimated rate: ~{}/sec", format_number(rate));
        println!("Estimated time: {}", format_duration(eta_secs));
        return;
    }

    // Set thread count
    let threads = if args.threads == 0 {
        num_cpus()
    } else {
        args.threads
    };
    println!("Threads: {}", threads);
    println!();

    // Configure rayon
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global()
        .ok();

    // Shared state
    let found = Arc::new(AtomicBool::new(false));
    let attempts = Arc::new(AtomicU64::new(0));
    let start = Instant::now();

    // Progress bar
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} [{elapsed_precise}] {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner()),
    );

    // Start background progress updater
    let attempts_clone = Arc::clone(&attempts);
    let found_clone = Arc::clone(&found);
    let pb_clone = pb.clone();
    let timeout_secs = args.timeout;
    let max_attempts = args.max_attempts;
    std::thread::spawn(move || {
        while !found_clone.load(Ordering::Relaxed) {
            let count = attempts_clone.load(Ordering::Relaxed);
            let elapsed = start.elapsed().as_secs().max(1);
            let rate = count / elapsed;
            
            // Check timeout
            if timeout_secs > 0 && elapsed >= timeout_secs {
                found_clone.store(true, Ordering::Relaxed); // Signal to stop
                pb_clone.set_message(format!("TIMEOUT after {}s", elapsed));
                break;
            }
            
            // Check max attempts
            if max_attempts > 0 && count >= max_attempts {
                found_clone.store(true, Ordering::Relaxed); // Signal to stop
                pb_clone.set_message(format!("MAX ATTEMPTS reached: {}", format_number(count)));
                break;
            }
            
            pb_clone.set_message(format!(
                "Attempts: {} | Rate: {}/s | Elapsed: {}s",
                format_number(count),
                format_number(rate),
                elapsed
            ));
            std::thread::sleep(Duration::from_millis(100));
        }
    });

    // Track if we hit limits
    let hit_limit = Arc::new(AtomicBool::new(false));
    let hit_limit_clone = Arc::clone(&hit_limit);
    let max_attempts_check = args.max_attempts;
    let timeout_check = args.timeout;

    // Generate in parallel
    let result: Option<(SigningKey, String)> = (0..u64::MAX)
        .into_par_iter()
        .find_map_any(|_| {
            if found.load(Ordering::Relaxed) {
                return None;
            }

            let current = attempts.fetch_add(1, Ordering::Relaxed);
            
            // Check limits within worker
            if max_attempts_check > 0 && current >= max_attempts_check {
                hit_limit_clone.store(true, Ordering::Relaxed);
                found.store(true, Ordering::Relaxed);
                return None;
            }
            
            if timeout_check > 0 && start.elapsed().as_secs() >= timeout_check {
                hit_limit_clone.store(true, Ordering::Relaxed);
                found.store(true, Ordering::Relaxed);
                return None;
            }

            // Generate random keypair
            let signing_key = SigningKey::generate(&mut OsRng);
            let onion = compute_onion_address(&signing_key.verifying_key());

            if onion.starts_with(&prefix) {
                found.store(true, Ordering::Relaxed);
                Some((signing_key, onion))
            } else {
                None
            }
        });

    pb.finish_and_clear();

    let elapsed = start.elapsed();
    let total_attempts = attempts.load(Ordering::Relaxed);
    let was_limited = hit_limit.load(Ordering::Relaxed);

    match result {
        Some((secret_key, onion_address)) => {
            println!("✅ Found matching address!");
            println!();
            println!("🧅 Onion Address: {}.onion", onion_address);
            println!();
            println!("📊 Statistics:");
            println!("   Attempts: {}", format_number(total_attempts));
            println!("   Time: {:.2?}", elapsed);
            println!(
                "   Rate: {}/s",
                format_number(total_attempts / elapsed.as_secs().max(1))
            );

            // Save keys if output specified
            if let Some(output_dir) = args.output {
                if let Err(e) = save_keys(&output_dir, &secret_key, &onion_address) {
                    eprintln!("Error saving keys: {}", e);
                    std::process::exit(1);
                }
                println!();
                println!("📁 Keys saved to: {}/", output_dir.display());
            } else {
                println!();
                println!("⚠️  Keys not saved! Use --output <dir> to save keys.");
                println!();
                // Print secret key in hex for manual saving
                println!("🔑 Secret Key (KEEP PRIVATE):");
                let expanded = secret_key.to_keypair_bytes();
                println!("   {}", hex_encode(&expanded));
            }
        }
        None => {
            if was_limited {
                println!();
                println!("⏱️  Search stopped due to limits:");
                println!("   Attempts: {}", format_number(total_attempts));
                println!("   Time: {:.2?}", elapsed);
                println!();
                println!("💡 Tips:");
                println!("   - Use a shorter prefix (3-4 chars) for faster results");
                println!("   - Use --test-mode to auto-shorten long prefixes");
                println!("   - Increase --timeout or --max-attempts");
                println!();
                std::process::exit(2); // Exit code 2 = hit limit
            } else {
                println!("❌ Search interrupted or failed");
                std::process::exit(1);
            }
        }
    }
}

/// Compute the full onion address from a public key
fn compute_onion_address(pubkey: &VerifyingKey) -> String {
    let pubkey_bytes = pubkey.as_bytes();

    // Compute checksum: SHA3-256(".onion checksum" || pubkey || version)
    let mut hasher = Sha3_256::new();
    hasher.update(CHECKSUM_PREFIX);
    hasher.update(pubkey_bytes);
    hasher.update([ONION_V3_VERSION]);
    let hash = hasher.finalize();
    let checksum = &hash[..2];

    // Concatenate: pubkey (32) + checksum (2) + version (1) = 35 bytes
    let mut address_bytes = [0u8; 35];
    address_bytes[..32].copy_from_slice(pubkey_bytes);
    address_bytes[32..34].copy_from_slice(checksum);
    address_bytes[34] = ONION_V3_VERSION;

    // Base32 encode (lowercase, no padding)
    base32::encode(base32::Alphabet::Rfc4648Lower { padding: false }, &address_bytes)
}

/// Save the key files in Tor's expected format
fn save_keys(
    output_dir: &PathBuf,
    secret_key: &SigningKey,
    onion_address: &str,
) -> std::io::Result<()> {
    std::fs::create_dir_all(output_dir)?;

    // hs_ed25519_secret_key (Tor format: header + 64-byte expanded secret key)
    // Tor expects the expanded secret key, not the seed!
    // Format: "== ed25519v1-secret: type0 ==" (29 bytes) + 3 null bytes + 64 byte expanded key
    let mut secret_file = output_dir.clone();
    secret_file.push("hs_ed25519_secret_key");

    let header = b"== ed25519v1-secret: type0 ==\x00\x00\x00";
    
    // Compute the expanded secret key the same way Ed25519 does:
    // h = SHA512(seed), then clamp h[0..32] for the scalar, h[32..64] for nonce
    let seed_bytes = secret_key.to_bytes();
    let mut hasher = Sha512::new();
    hasher.update(&seed_bytes);
    let hash_result = hasher.finalize();
    
    let mut expanded_bytes = [0u8; 64];
    expanded_bytes.copy_from_slice(&hash_result);
    
    // Apply Ed25519 clamping to the first 32 bytes (the scalar)
    expanded_bytes[0] &= 248;
    expanded_bytes[31] &= 127;
    expanded_bytes[31] |= 64;
    
    let mut secret_data = Vec::with_capacity(32 + 64);
    secret_data.extend_from_slice(header);
    secret_data.extend_from_slice(&expanded_bytes);
    std::fs::write(&secret_file, &secret_data)?;

    // hs_ed25519_public_key (Tor format: header + 32-byte public key)
    let mut public_file = output_dir.clone();
    public_file.push("hs_ed25519_public_key");

    let pub_header = b"== ed25519v1-public: type0 ==\x00\x00\x00";
    let mut pub_data = Vec::with_capacity(32 + 32);
    pub_data.extend_from_slice(pub_header);
    pub_data.extend_from_slice(secret_key.verifying_key().as_bytes());
    std::fs::write(&public_file, &pub_data)?;

    // hostname
    let mut hostname_file = output_dir.clone();
    hostname_file.push("hostname");
    std::fs::write(&hostname_file, format!("{}.onion\n", onion_address))?;

    // Also save as JSON for programmatic access
    let mut json_file = output_dir.clone();
    json_file.push("vanity_key.json");
    let json = serde_json::json!({
        "onion_address": format!("{}.onion", onion_address),
        "prefix": onion_address.chars().take(6).collect::<String>(),
        "generated_at": chrono_now_iso(),
    });
    std::fs::write(&json_file, serde_json::to_string_pretty(&json).unwrap_or_default())?;

    Ok(())
}

/// Benchmark key generation rate
fn benchmark_rate() -> u64 {
    let start = Instant::now();
    let iterations = 10_000;

    for _ in 0..iterations {
        let signing_key = SigningKey::generate(&mut OsRng);
        let _ = compute_onion_address(&signing_key.verifying_key());
    }

    let elapsed = start.elapsed().as_secs_f64();
    (iterations as f64 / elapsed) as u64
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(4)
}

fn format_number(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.2}B", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.2}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn format_duration(secs: u64) -> String {
    if secs >= 86400 * 365 {
        format!("{:.1} years", secs as f64 / (86400.0 * 365.0))
    } else if secs >= 86400 {
        format!("{:.1} days", secs as f64 / 86400.0)
    } else if secs >= 3600 {
        format!("{:.1} hours", secs as f64 / 3600.0)
    } else if secs >= 60 {
        format!("{:.1} minutes", secs as f64 / 60.0)
    } else {
        format!("{} seconds", secs)
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn chrono_now_iso() -> String {
    // Simple ISO timestamp without chrono dependency
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{}", now)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_onion_address_generation() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let onion = compute_onion_address(&signing_key.verifying_key());

        // V3 onion addresses are 56 characters
        assert_eq!(onion.len(), 56);

        // Should only contain base32 characters
        assert!(onion.chars().all(|c| c.is_ascii_lowercase() || ('2'..='7').contains(&c)));
    }

    #[test]
    fn test_onion_address_deterministic() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let onion1 = compute_onion_address(&signing_key.verifying_key());
        let onion2 = compute_onion_address(&signing_key.verifying_key());

        assert_eq!(onion1, onion2);
    }
}
