//! Passport Protocol - Cryptographic Inter-Node Trust
//!
//! Implements ed25519-signed tokens for secure node-to-node handoffs.
//! When a node is overloaded, it can issue a "passport" token that
//! allows a client to bypass the CAPTCHA on the target node.
//!
//! Token format: base64(target:expiry:signature:sender)
//!
//! Security properties:
//! - Tokens are short-lived (30 seconds default)
//! - Tokens are bound to a specific target node
//! - Only nodes with valid keypairs can issue tokens
//! - Only nodes with the issuer's public key can validate

use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey, Signature};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

/// Passport service configuration
#[derive(Clone, Debug)]
pub struct PassportConfig {
    /// Token validity duration in seconds
    pub token_ttl_secs: u64,
    /// Our node ID
    pub node_id: String,
    /// Path to our private key file
    pub private_key_path: Option<String>,
    /// Known peer public keys (node_id -> base64 pubkey)
    pub peer_pubkeys: HashMap<String, String>,
}

impl Default for PassportConfig {
    fn default() -> Self {
        Self {
            token_ttl_secs: 30,
            node_id: "unknown".to_string(),
            private_key_path: None,
            peer_pubkeys: HashMap::new(),
        }
    }
}

/// A passport token for cross-node authentication
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PassportToken {
    /// Target node ID this passport is valid for
    pub target: String,
    /// Expiry timestamp (unix seconds)
    pub expiry: u64,
    /// Issuing node ID
    pub issuer: String,
    /// Circuit ID this passport was issued for
    pub circuit_id: Option<String>,
}

impl PassportToken {
    /// Check if the token has expired
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.expiry < now
    }
}

/// Passport service for issuing and validating tokens
pub struct PassportService {
    /// Configuration
    config: PassportConfig,
    /// Our signing key
    signing_key: Option<SigningKey>,
    /// Our public key
    verifying_key: Option<VerifyingKey>,
    /// Known peer public keys (node_id -> VerifyingKey)
    peer_keys: Arc<RwLock<HashMap<String, VerifyingKey>>>,
}

impl PassportService {
    /// Create a new passport service
    pub fn new(config: PassportConfig) -> Result<Self> {
        let (signing_key, verifying_key) = if let Some(ref path) = config.private_key_path {
            // Load existing key
            let key_bytes = std::fs::read(path)
                .context("Failed to read private key file")?;
            
            if key_bytes.len() != 32 {
                bail!("Invalid private key length (expected 32 bytes)");
            }
            
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(&key_bytes);
            let signing = SigningKey::from_bytes(&bytes);
            let verifying = signing.verifying_key();
            (Some(signing), Some(verifying))
        } else {
            // Generate ephemeral key using OsRng (compatible with ed25519-dalek)
            use rand_core::OsRng;
            let signing = SigningKey::generate(&mut OsRng);
            let verifying = signing.verifying_key();
            
            tracing::warn!("Using ephemeral passport key (will change on restart)");
            (Some(signing), Some(verifying))
        };

        // Parse peer public keys
        let mut peer_keys = HashMap::new();
        for (node_id, pubkey_b64) in &config.peer_pubkeys {
            let pubkey_bytes = URL_SAFE_NO_PAD.decode(pubkey_b64)
                .context("Failed to decode peer public key")?;
            
            if pubkey_bytes.len() != 32 {
                bail!("Invalid public key length for node {}", node_id);
            }
            
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(&pubkey_bytes);
            let verifying = VerifyingKey::from_bytes(&bytes)
                .context("Invalid public key")?;
            
            peer_keys.insert(node_id.clone(), verifying);
        }

        Ok(Self {
            config,
            signing_key,
            verifying_key,
            peer_keys: Arc::new(RwLock::new(peer_keys)),
        })
    }

    /// Get our node ID
    pub fn node_id(&self) -> &str {
        &self.config.node_id
    }

    /// Get our public key as base64
    pub fn public_key_b64(&self) -> Option<String> {
        self.verifying_key.as_ref().map(|k| URL_SAFE_NO_PAD.encode(k.as_bytes()))
    }

    /// Issue a passport token for a client to present to another node
    pub fn mint(&self, target_node: &str, circuit_id: Option<String>) -> Result<String> {
        let signing_key = self.signing_key.as_ref()
            .context("No signing key available")?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let expiry = now + self.config.token_ttl_secs;
        
        // Create payload to sign
        let payload = format!("{}:{}:{}", target_node, expiry, self.config.node_id);
        
        // Sign the payload
        let signature = signing_key.sign(payload.as_bytes());
        
        // Encode token: payload:signature_b64
        let sig_b64 = URL_SAFE_NO_PAD.encode(signature.to_bytes());
        let token = format!("{}:{}", payload, sig_b64);
        
        tracing::debug!(
            target = target_node,
            circuit_id = ?circuit_id,
            expiry = expiry,
            "Issued passport token"
        );

        Ok(URL_SAFE_NO_PAD.encode(token.as_bytes()))
    }

    /// Validate a passport token presented by a client
    pub async fn validate(&self, token: &str) -> Result<PassportToken> {
        // Decode outer base64
        let decoded = URL_SAFE_NO_PAD.decode(token)
            .context("Invalid token encoding")?;
        let token_str = String::from_utf8(decoded)
            .context("Invalid token UTF-8")?;

        // Parse: target:expiry:issuer:signature
        let parts: Vec<&str> = token_str.split(':').collect();
        if parts.len() != 4 {
            bail!("Invalid token format (expected 4 parts, got {})", parts.len());
        }

        let target = parts[0];
        let expiry: u64 = parts[1].parse()
            .context("Invalid expiry timestamp")?;
        let issuer = parts[2];
        let sig_b64 = parts[3];

        // 1. Check if token is for us
        if target != self.config.node_id {
            bail!("Token not for this node (target: {}, we are: {})", target, self.config.node_id);
        }

        // 2. Check expiry
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        if expiry < now {
            bail!("Token expired (expired at {}, now is {})", expiry, now);
        }

        // 3. Get issuer's public key
        let peer_keys = self.peer_keys.read().await;
        let issuer_key = peer_keys.get(issuer)
            .context(format!("Unknown issuer: {}", issuer))?;

        // 4. Verify signature
        let sig_bytes = URL_SAFE_NO_PAD.decode(sig_b64)
            .context("Invalid signature encoding")?;
        
        if sig_bytes.len() != 64 {
            bail!("Invalid signature length");
        }
        
        let mut sig_array = [0u8; 64];
        sig_array.copy_from_slice(&sig_bytes);
        let signature = Signature::from_bytes(&sig_array);

        let payload = format!("{}:{}:{}", target, expiry, issuer);
        issuer_key.verify(payload.as_bytes(), &signature)
            .context("Invalid signature")?;

        tracing::debug!(
            issuer = issuer,
            target = target,
            expires_in = expiry - now,
            "Validated passport token"
        );

        Ok(PassportToken {
            target: target.to_string(),
            expiry,
            issuer: issuer.to_string(),
            circuit_id: None, // Not stored in token for privacy
        })
    }

    /// Add a peer's public key at runtime
    pub async fn add_peer_key(&self, node_id: &str, pubkey_b64: &str) -> Result<()> {
        let pubkey_bytes = URL_SAFE_NO_PAD.decode(pubkey_b64)
            .context("Failed to decode public key")?;
        
        if pubkey_bytes.len() != 32 {
            bail!("Invalid public key length");
        }
        
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&pubkey_bytes);
        let verifying = VerifyingKey::from_bytes(&bytes)
            .context("Invalid public key")?;

        let mut peer_keys = self.peer_keys.write().await;
        peer_keys.insert(node_id.to_string(), verifying);
        
        tracing::info!(node_id = node_id, "Added peer public key");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_passport_mint_and_validate() {
        // Create two nodes
        let config1 = PassportConfig {
            node_id: "node-1".to_string(),
            token_ttl_secs: 30,
            ..Default::default()
        };
        let service1 = PassportService::new(config1).unwrap();

        let config2 = PassportConfig {
            node_id: "node-2".to_string(),
            token_ttl_secs: 30,
            ..Default::default()
        };
        let service2 = PassportService::new(config2).unwrap();

        // Exchange public keys
        let pubkey1 = service1.public_key_b64().unwrap();
        service2.add_peer_key("node-1", &pubkey1).await.unwrap();

        // Node 1 mints a passport for node 2
        let token = service1.mint("node-2", Some("circuit-123".to_string())).unwrap();

        // Node 2 validates the token
        let passport = service2.validate(&token).await.unwrap();
        assert_eq!(passport.target, "node-2");
        assert_eq!(passport.issuer, "node-1");
        assert!(!passport.is_expired());
    }

    #[tokio::test]
    async fn test_passport_wrong_target() {
        let config = PassportConfig {
            node_id: "node-1".to_string(),
            token_ttl_secs: 30,
            ..Default::default()
        };
        let service = PassportService::new(config).unwrap();

        // Mint for node-2, but try to validate on node-1
        let token = service.mint("node-2", None).unwrap();
        let result = service.validate(&token).await;
        assert!(result.is_err());
    }
}
