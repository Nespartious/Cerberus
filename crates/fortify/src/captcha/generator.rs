//! CAPTCHA image generation.
//!
//! MVP: Generates simple text-based placeholder images.
//! The text shows random characters that the user must type.

use anyhow::Result;
use base64::{engine::general_purpose::STANDARD, Engine};
use cerberus_common::{CaptchaChallenge, CaptchaDifficulty};
use rand::Rng;
use redis::AsyncCommands;

use super::StoredChallenge;

/// CAPTCHA generator service
pub struct CaptchaGenerator {
    /// Challenge TTL in seconds
    pub challenge_ttl: u64,
}

impl CaptchaGenerator {
    pub fn new(challenge_ttl: u64) -> Self {
        Self { challenge_ttl }
    }

    /// Generate a new CAPTCHA challenge
    pub async fn generate(
        &self,
        redis: &mut redis::aio::ConnectionManager,
        circuit_id: Option<String>,
        difficulty: CaptchaDifficulty,
    ) -> Result<CaptchaChallenge> {
        let challenge_id = self.generate_challenge_id();
        let (answer, image_data) = self.create_placeholder_captcha(difficulty);

        let now = chrono::Utc::now().timestamp();
        let expires_at = now + self.challenge_ttl as i64;

        // Store challenge in Redis
        let stored = StoredChallenge {
            answer: answer.clone(),
            circuit_id: circuit_id.clone(),
            difficulty,
            created_at: now,
            expires_at,
        };

        let key = format!("captcha:{}", challenge_id);
        let value = serde_json::to_string(&stored)?;
        redis
            .set_ex::<_, _, ()>(&key, &value, self.challenge_ttl)
            .await?;

        tracing::debug!(
            challenge_id = %challenge_id,
            circuit_id = ?circuit_id,
            difficulty = ?difficulty,
            "Generated CAPTCHA challenge"
        );

        Ok(CaptchaChallenge {
            challenge_id,
            image_data,
            grid_size: difficulty.grid_size(),
            instructions: self.get_instructions(difficulty),
            expected_positions: vec![], // Not sent to client
            expires_at,
        })
    }

    /// Generate a cryptographically random challenge ID
    fn generate_challenge_id(&self) -> String {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let mut bytes = [0u8; 16];
        rand::rng().fill(&mut bytes);
        URL_SAFE_NO_PAD.encode(bytes)
    }

    /// Create a placeholder CAPTCHA (MVP)
    ///
    /// Returns (answer, base64_image_data)
    fn create_placeholder_captcha(&self, difficulty: CaptchaDifficulty) -> (String, String) {
        let mut rng = rand::rng();

        // Generate random alphanumeric answer
        let length = match difficulty {
            CaptchaDifficulty::Easy => 4,
            CaptchaDifficulty::Medium => 5,
            CaptchaDifficulty::Hard => 6,
            CaptchaDifficulty::Extreme => 8,
        };

        let answer: String = (0..length)
            .map(|_| {
                let idx = rng.random_range(0..36);
                if idx < 10 {
                    (b'0' + idx) as char
                } else {
                    (b'A' + idx - 10) as char
                }
            })
            .collect();

        // Create a simple SVG placeholder (works without image libraries)
        let svg = self.create_svg_captcha(&answer, difficulty);
        let image_data = format!("data:image/svg+xml;base64,{}", STANDARD.encode(&svg));

        (answer, image_data)
    }

    /// Create an SVG CAPTCHA image
    fn create_svg_captcha(&self, text: &str, difficulty: CaptchaDifficulty) -> String {
        let mut rng = rand::rng();

        let width = 200;
        let height = 80;

        // Background noise based on difficulty
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

        // Text characters with slight randomization
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
        svg
    }

    fn get_instructions(&self, difficulty: CaptchaDifficulty) -> String {
        match difficulty {
            CaptchaDifficulty::Easy => "Type the characters shown above".to_string(),
            CaptchaDifficulty::Medium => "Type the characters shown above (case insensitive)".to_string(),
            CaptchaDifficulty::Hard => "Type the characters exactly as shown".to_string(),
            CaptchaDifficulty::Extreme => "Type the characters within 20 seconds".to_string(),
        }
    }
}
