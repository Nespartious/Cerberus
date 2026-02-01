//! Ammo Box: Pre-generated CAPTCHA pool with disk persistence.
//!
//! Implements the "Deep Storage" strategy from Project_Outline_R0.md Section 7.2:
//! - Tier 1: RAM Ring Buffer (fast dispatch)
//! - Tier 2: Disk Cache (sustainment during load spikes)
//!
//! The background worker ("Reloader") manages pool levels:
//! - Critical Low (<10%): Emergency load from disk or generate
//! - Normal Maintenance (<80%): Generate when CPU is available
//! - Surplus (>95%): Dump to disk for persistence

use anyhow::{Context, Result};
use cerberus_common::CaptchaDifficulty;
use crossbeam_queue::ArrayQueue;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// A pre-generated CAPTCHA ready for immediate dispatch
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PregenCaptcha {
    /// The answer text
    pub answer: String,
    /// Base64-encoded SVG image
    pub image_data: String,
    /// Difficulty level
    pub difficulty: CaptchaDifficulty,
    /// Unix timestamp when generated
    pub generated_at: i64,
}

/// Configuration for the Ammo Box
#[derive(Clone, Debug)]
pub struct AmmoBoxConfig {
    /// Maximum CAPTCHAs in RAM pool
    pub ram_capacity: usize,
    /// Path to disk cache directory
    pub disk_cache_path: PathBuf,
    /// Maximum CAPTCHAs on disk
    pub max_disk_cache: usize,
    /// Minimum free disk space in GB before stopping disk writes
    pub min_disk_free_gb: u64,
    /// How often to dump RAM to disk (seconds)
    pub dump_interval_secs: u64,
}

impl Default for AmmoBoxConfig {
    fn default() -> Self {
        Self {
            ram_capacity: 10_000,
            disk_cache_path: PathBuf::from("/var/lib/cerberus/ammo"),
            max_disk_cache: 100_000,
            min_disk_free_gb: 5,
            dump_interval_secs: 300,
        }
    }
}

/// The Ammo Box: Pre-generated CAPTCHA storage
pub struct AmmoBox {
    /// RAM pool (lock-free ring buffer)
    pool: ArrayQueue<PregenCaptcha>,
    /// Configuration
    config: AmmoBoxConfig,
    /// Last dump timestamp
    last_dump: Mutex<Instant>,
    /// Statistics
    stats: AmmoBoxStats,
}

/// Runtime statistics
#[derive(Default)]
pub struct AmmoBoxStats {
    /// Total CAPTCHAs served from pool
    pub served: AtomicU64,
    /// Total CAPTCHAs generated
    pub generated: AtomicU64,
    /// Total CAPTCHAs loaded from disk
    pub loaded_from_disk: AtomicU64,
    /// Total CAPTCHAs dumped to disk
    pub dumped_to_disk: AtomicU64,
    /// Pool misses (had to generate on-demand)
    pub pool_misses: AtomicU64,
}

impl AmmoBox {
    /// Create a new Ammo Box
    pub fn new(config: AmmoBoxConfig) -> Self {
        let capacity = config.ram_capacity;
        Self {
            pool: ArrayQueue::new(capacity),
            config,
            last_dump: Mutex::new(Instant::now()),
            stats: AmmoBoxStats::default(),
        }
    }

    /// Get pool capacity
    pub fn capacity(&self) -> usize {
        self.config.ram_capacity
    }

    /// Get current pool size
    pub fn len(&self) -> usize {
        self.pool.len()
    }

    /// Check if pool is empty
    pub fn is_empty(&self) -> bool {
        self.pool.is_empty()
    }

    /// Get pool fill percentage (0-100)
    pub fn fill_percent(&self) -> u8 {
        ((self.pool.len() as f64 / self.config.ram_capacity as f64) * 100.0) as u8
    }

    /// Pop a pre-generated CAPTCHA from the pool
    ///
    /// Returns None if pool is empty (caller should generate on-demand)
    pub fn pop(&self) -> Option<PregenCaptcha> {
        let captcha = self.pool.pop();
        if captcha.is_some() {
            self.stats.served.fetch_add(1, Ordering::Relaxed);
        } else {
            self.stats.pool_misses.fetch_add(1, Ordering::Relaxed);
        }
        captcha
    }

    /// Push a pre-generated CAPTCHA into the pool
    ///
    /// Returns the captcha back if pool is full
    pub fn push(&self, captcha: PregenCaptcha) -> Result<(), PregenCaptcha> {
        self.pool.push(captcha)
    }

    /// Push a batch of CAPTCHAs into the pool
    pub fn push_batch(&self, batch: Vec<PregenCaptcha>) -> usize {
        let mut pushed = 0;
        for captcha in batch {
            if self.pool.push(captcha).is_ok() {
                pushed += 1;
            } else {
                break; // Pool is full
            }
        }
        pushed
    }

    /// Generate a batch of CAPTCHAs
    pub fn generate_batch(&self, count: usize, difficulty: CaptchaDifficulty) -> Vec<PregenCaptcha> {
        use rand::Rng;

        let mut batch = Vec::with_capacity(count);
        let mut rng = rand::rng();
        let now = chrono::Utc::now().timestamp();

        for _ in 0..count {
            let answer = generate_answer(&mut rng, difficulty);
            let image_data = generate_svg(&answer, difficulty, &mut rng);

            batch.push(PregenCaptcha {
                answer,
                image_data,
                difficulty,
                generated_at: now,
            });

            self.stats.generated.fetch_add(1, Ordering::Relaxed);
        }

        batch
    }

    /// Load CAPTCHAs from disk cache
    pub async fn load_from_disk(&self, max_count: usize) -> Result<usize> {
        let cache_dir = &self.config.disk_cache_path;

        if !cache_dir.exists() {
            return Ok(0);
        }

        let mut loaded = 0;
        let mut read_dir = tokio::fs::read_dir(cache_dir).await?;
        let mut entries = Vec::new();

        while let Some(entry) = read_dir.next_entry().await? {
            entries.push(entry);
        }

        // Sort by name (oldest first)
        entries.sort_by_key(|e| e.file_name());

        for entry in entries.into_iter().take(max_count) {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "bin") {
                match self.load_batch_file(&path).await {
                    Ok(count) => {
                        loaded += count;
                        // Delete after loading
                        let _ = tokio::fs::remove_file(&path).await;
                    }
                    Err(e) => {
                        tracing::warn!(path = ?path, error = %e, "Failed to load ammo file");
                    }
                }
            }

            if loaded >= max_count {
                break;
            }
        }

        self.stats.loaded_from_disk.fetch_add(loaded as u64, Ordering::Relaxed);
        tracing::debug!(loaded = loaded, "Loaded CAPTCHAs from disk");

        Ok(loaded)
    }

    /// Load a single batch file
    async fn load_batch_file(&self, path: &Path) -> Result<usize> {
        let data = tokio::fs::read(path).await?;
        let batch: Vec<PregenCaptcha> = bincode::deserialize(&data)?;
        let count = self.push_batch(batch);
        Ok(count)
    }

    /// Dump current pool to disk
    pub async fn dump_to_disk(&self, batch_size: usize) -> Result<usize> {
        let cache_dir = &self.config.disk_cache_path;

        // Ensure directory exists
        tokio::fs::create_dir_all(cache_dir).await?;

        // Pop items from pool
        let mut batch = Vec::with_capacity(batch_size);
        for _ in 0..batch_size {
            if let Some(captcha) = self.pool.pop() {
                batch.push(captcha);
            } else {
                break;
            }
        }

        if batch.is_empty() {
            return Ok(0);
        }

        let count = batch.len();

        // Serialize and write
        let data = bincode::serialize(&batch)?;
        let filename = format!("ammo_{}.bin", chrono::Utc::now().timestamp_millis());
        let path = cache_dir.join(filename);

        tokio::fs::write(&path, data).await?;

        self.stats.dumped_to_disk.fetch_add(count as u64, Ordering::Relaxed);
        tracing::debug!(count = count, path = ?path, "Dumped CAPTCHAs to disk");

        // Put items back in pool (they're now also on disk)
        self.push_batch(batch);

        Ok(count)
    }

    /// Get statistics snapshot
    pub fn get_stats(&self) -> AmmoBoxStatsSnapshot {
        AmmoBoxStatsSnapshot {
            pool_size: self.pool.len(),
            pool_capacity: self.config.ram_capacity,
            fill_percent: self.fill_percent(),
            served: self.stats.served.load(Ordering::Relaxed),
            generated: self.stats.generated.load(Ordering::Relaxed),
            loaded_from_disk: self.stats.loaded_from_disk.load(Ordering::Relaxed),
            dumped_to_disk: self.stats.dumped_to_disk.load(Ordering::Relaxed),
            pool_misses: self.stats.pool_misses.load(Ordering::Relaxed),
        }
    }

    /// Check if we should dump to disk
    pub async fn should_dump(&self) -> bool {
        let last = self.last_dump.lock().await;
        last.elapsed() > Duration::from_secs(self.config.dump_interval_secs)
    }

    /// Update last dump time
    pub async fn mark_dumped(&self) {
        let mut last = self.last_dump.lock().await;
        *last = Instant::now();
    }
}

/// Snapshot of Ammo Box statistics
#[derive(Clone, Debug, Serialize)]
pub struct AmmoBoxStatsSnapshot {
    pub pool_size: usize,
    pub pool_capacity: usize,
    pub fill_percent: u8,
    pub served: u64,
    pub generated: u64,
    pub loaded_from_disk: u64,
    pub dumped_to_disk: u64,
    pub pool_misses: u64,
}

/// Background worker that maintains the Ammo Box
pub async fn ammo_box_worker(
    ammo: Arc<AmmoBox>,
    mut shutdown: tokio::sync::broadcast::Receiver<()>,
) {
    tracing::info!("🎯 Ammo Box worker started (capacity: {})", ammo.capacity());

    loop {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(1)) => {
                if let Err(e) = maintain_ammo_box(&ammo).await {
                    tracing::error!(error = %e, "Ammo Box maintenance error");
                }
            }
            _ = shutdown.recv() => {
                tracing::info!("🎯 Ammo Box worker shutting down...");
                // Dump remaining pool to disk on shutdown
                if let Err(e) = ammo.dump_to_disk(ammo.len()).await {
                    tracing::error!(error = %e, "Failed to dump pool on shutdown");
                }
                break;
            }
        }
    }
}

/// Maintenance logic for the Ammo Box
async fn maintain_ammo_box(ammo: &AmmoBox) -> Result<()> {
    let pool_len = ammo.len();
    let pool_max = ammo.capacity();
    let fill_pct = ammo.fill_percent();

    // Get CPU load (simplified - use sysinfo crate in production)
    let cpu_load = get_cpu_load().await;

    // 1. Critical Low (< 10%): Emergency Action
    if fill_pct < 10 {
        if cpu_load > 80 {
            // CPU High: Load from Disk (Cheap I/O)
            tracing::warn!(fill_pct = fill_pct, "Ammo critical - loading from disk");
            ammo.load_from_disk(1000).await?;
        } else {
            // CPU Low: Generate (Expensive but necessary)
            tracing::warn!(fill_pct = fill_pct, "Ammo critical - generating batch");
            let batch = ammo.generate_batch(500, CaptchaDifficulty::Medium);
            ammo.push_batch(batch);
        }
    }
    // 2. Normal Maintenance (< 80%)
    else if fill_pct < 80 {
        if cpu_load < 50 {
            // Only generate if system is healthy
            let batch = ammo.generate_batch(100, CaptchaDifficulty::Medium);
            ammo.push_batch(batch);
        }
    }
    // 3. Surplus Strategy (> 95%): Deep Storage
    else if fill_pct > 95 && cpu_load < 20 {
        // Check if we should dump to disk
        if ammo.should_dump().await {
            tracing::debug!("Pool surplus - dumping to disk");
            ammo.dump_to_disk(1000).await?;
            ammo.mark_dumped().await;
        }
    }

    Ok(())
}

/// Get CPU load (0-100)
async fn get_cpu_load() -> u8 {
    // Simplified implementation - in production use sysinfo crate
    // For now, return a low value to allow generation
    10
}

/// Generate random answer string
fn generate_answer(rng: &mut impl rand::Rng, difficulty: CaptchaDifficulty) -> String {
    let length = match difficulty {
        CaptchaDifficulty::Easy => 4,
        CaptchaDifficulty::Medium => 5,
        CaptchaDifficulty::Hard => 6,
        CaptchaDifficulty::Extreme => 8,
    };

    (0..length)
        .map(|_| {
            let idx = rng.random_range(0..36u8);
            if idx < 10 {
                (b'0' + idx) as char
            } else {
                (b'A' + idx - 10) as char
            }
        })
        .collect()
}

/// Generate SVG CAPTCHA image
fn generate_svg(text: &str, difficulty: CaptchaDifficulty, rng: &mut impl rand::Rng) -> String {
    use base64::{Engine, engine::general_purpose::STANDARD};

    let width = 200;
    let height = 80;

    let noise_count = match difficulty {
        CaptchaDifficulty::Easy => 5,
        CaptchaDifficulty::Medium => 15,
        CaptchaDifficulty::Hard => 30,
        CaptchaDifficulty::Extreme => 50,
    };

    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}">"#,
        width, height
    );

    // Background
    svg.push_str(r##"<rect width="100%" height="100%" fill="#1a1a2e"/>"##);

    // Noise lines
    for _ in 0..noise_count {
        let x1 = rng.random_range(0..width);
        let y1 = rng.random_range(0..height);
        let x2 = rng.random_range(0..width);
        let y2 = rng.random_range(0..height);
        let opacity = rng.random_range(20..50);
        svg.push_str(&format!(
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="rgba(255,255,255,0.{})" stroke-width="1"/>"#,
            x1, y1, x2, y2, opacity
        ));
    }

    // Text characters
    let char_width = width as f32 / (text.len() as f32 + 1.0);
    for (i, c) in text.chars().enumerate() {
        let x = char_width * (i as f32 + 0.8);
        let y = 50 + rng.random_range(-10..10);
        let rotation = rng.random_range(-15..15);
        let color = format!(
            "rgb({},{},{})",
            rng.random_range(150..255),
            rng.random_range(150..255),
            rng.random_range(150..255)
        );

        svg.push_str(&format!(
            r#"<text x="{}" y="{}" font-family="monospace" font-size="32" font-weight="bold" fill="{}" transform="rotate({} {} {})">{}</text>"#,
            x, y, color, rotation, x, y, c
        ));
    }

    svg.push_str("</svg>");

    format!("data:image/svg+xml;base64,{}", STANDARD.encode(&svg))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ammo_box_basic() {
        let config = AmmoBoxConfig {
            ram_capacity: 100,
            ..Default::default()
        };
        let ammo = AmmoBox::new(config);

        // Generate and push
        let batch = ammo.generate_batch(50, CaptchaDifficulty::Medium);
        assert_eq!(batch.len(), 50);

        let pushed = ammo.push_batch(batch);
        assert_eq!(pushed, 50);
        assert_eq!(ammo.len(), 50);
        assert_eq!(ammo.fill_percent(), 50);

        // Pop
        let captcha = ammo.pop();
        assert!(captcha.is_some());
        assert_eq!(ammo.len(), 49);
    }

    #[test]
    fn test_generate_answer() {
        let mut rng = rand::rng();
        let answer = generate_answer(&mut rng, CaptchaDifficulty::Medium);
        assert_eq!(answer.len(), 5);
        assert!(answer.chars().all(|c| c.is_ascii_alphanumeric()));
    }
}
