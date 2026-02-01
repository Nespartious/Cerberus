//! Health Gossip Protocol (UDP)
//!
//! Implements lightweight health broadcasting between cluster nodes.
//! Each node broadcasts a tiny JSON packet every 5 seconds to port 9000
//! (inside the WireGuard tunnel).
//!
//! Used for:
//! - Load-based routing decisions
//! - Split-brain detection
//! - Peer health monitoring

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::RwLock;

/// Gossip protocol configuration
#[derive(Clone, Debug)]
pub struct GossipConfig {
    /// Local bind address (e.g., "10.100.0.1:9000")
    pub bind_addr: String,
    /// Peer addresses to broadcast to
    pub peers: Vec<String>,
    /// Broadcast interval in seconds
    pub interval_secs: u64,
    /// Peer timeout in seconds (mark as unhealthy after this)
    pub peer_timeout_secs: u64,
    /// Stale threshold (mark as stale after this percentage of cluster is unreachable)
    pub isolation_threshold: f32,
}

impl Default for GossipConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:9000".to_string(),
            peers: vec![],
            interval_secs: 5,
            peer_timeout_secs: 30,
            isolation_threshold: 0.5,
        }
    }
}

/// Gossip packet broadcast to peers
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GossipPacket {
    /// Unique node identifier
    pub node_id: String,
    /// CPU load percentage (0-100)
    pub cpu_load: u8,
    /// Is local Tor daemon healthy?
    pub tor_health: bool,
    /// Current active connection count
    pub active_conns: u32,
    /// Ammo box fill percentage
    pub ammo_fill: u8,
    /// Current threat level
    pub threat_level: u8,
    /// Unix timestamp
    pub timestamp: u64,
    /// Software version
    pub version: String,
}

impl GossipPacket {
    /// Create a new gossip packet with current state
    pub fn new(
        node_id: String,
        cpu_load: u8,
        tor_health: bool,
        active_conns: u32,
        ammo_fill: u8,
        threat_level: u8,
    ) -> Self {
        Self {
            node_id,
            cpu_load,
            tor_health,
            active_conns,
            ammo_fill,
            threat_level,
            timestamp: chrono::Utc::now().timestamp() as u64,
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// Health status of a peer node
#[derive(Clone, Debug)]
pub struct NodeHealth {
    /// Last received gossip packet
    pub last_packet: GossipPacket,
    /// Last seen timestamp
    pub last_seen: Instant,
    /// Is this node considered healthy?
    pub is_healthy: bool,
}

/// Gossip service for cluster health monitoring
pub struct GossipService {
    /// Configuration
    config: GossipConfig,
    /// Our node ID
    node_id: String,
    /// Known peer health states
    peers: Arc<RwLock<HashMap<String, NodeHealth>>>,
    /// Are we isolated from the cluster?
    isolated: Arc<RwLock<bool>>,
}

impl GossipService {
    /// Create a new gossip service
    pub fn new(config: GossipConfig, node_id: String) -> Self {
        Self {
            config,
            node_id,
            peers: Arc::new(RwLock::new(HashMap::new())),
            isolated: Arc::new(RwLock::new(false)),
        }
    }

    /// Get our node ID
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// Check if we're isolated from the cluster
    pub async fn is_isolated(&self) -> bool {
        *self.isolated.read().await
    }

    /// Get all known peer health states
    pub async fn get_peers(&self) -> HashMap<String, NodeHealth> {
        self.peers.read().await.clone()
    }

    /// Get healthy peers (sorted by load)
    pub async fn get_healthy_peers(&self) -> Vec<GossipPacket> {
        let peers = self.peers.read().await;
        let mut healthy: Vec<_> = peers
            .values()
            .filter(|p| p.is_healthy)
            .map(|p| p.last_packet.clone())
            .collect();

        // Sort by load (lowest first)
        healthy.sort_by_key(|p| p.cpu_load);
        healthy
    }

    /// Get the least loaded healthy peer for load shedding
    pub async fn get_shed_target(&self) -> Option<GossipPacket> {
        let peers = self.peers.read().await;
        peers
            .values()
            .filter(|p| p.is_healthy && p.last_packet.cpu_load < 80)
            .min_by_key(|p| p.last_packet.cpu_load)
            .map(|p| p.last_packet.clone())
    }

    /// Run the gossip broadcaster
    pub async fn run_broadcaster(
        &self,
        mut get_state: impl FnMut() -> GossipPacket + Send + 'static,
        mut shutdown: tokio::sync::broadcast::Receiver<()>,
    ) -> Result<()> {
        let socket = UdpSocket::bind("0.0.0.0:0")
            .await
            .context("Failed to bind gossip sender socket")?;

        let peers = self.config.peers.clone();
        let interval = Duration::from_secs(self.config.interval_secs);

        tracing::info!(
            peers = ?peers,
            interval = ?interval,
            "🗣️ Gossip broadcaster started"
        );

        loop {
            tokio::select! {
                _ = tokio::time::sleep(interval) => {
                    let packet = get_state();
                    let bytes = match serde_json::to_vec(&packet) {
                        Ok(b) => b,
                        Err(e) => {
                            tracing::error!(error = %e, "Failed to serialize gossip packet");
                            continue;
                        }
                    };

                    for peer in &peers {
                        if let Err(e) = socket.send_to(&bytes, peer).await {
                            tracing::warn!(peer = %peer, error = %e, "Failed to send gossip");
                        }
                    }
                }
                _ = shutdown.recv() => {
                    tracing::info!("🗣️ Gossip broadcaster shutting down");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Run the gossip receiver
    pub async fn run_receiver(
        &self,
        mut shutdown: tokio::sync::broadcast::Receiver<()>,
    ) -> Result<()> {
        let socket = UdpSocket::bind(&self.config.bind_addr)
            .await
            .context("Failed to bind gossip receiver socket")?;

        let mut buf = vec![0u8; 1024];
        let timeout = Duration::from_secs(self.config.peer_timeout_secs);

        tracing::info!(
            addr = %self.config.bind_addr,
            "👂 Gossip receiver started"
        );

        loop {
            tokio::select! {
                result = socket.recv_from(&mut buf) => {
                    match result {
                        Ok((len, addr)) => {
                            self.handle_packet(&buf[..len], addr).await;
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "Gossip receive error");
                        }
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(1)) => {
                    // Periodic cleanup and isolation check
                    self.check_peer_health(timeout).await;
                }
                _ = shutdown.recv() => {
                    tracing::info!("👂 Gossip receiver shutting down");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Handle an incoming gossip packet
    async fn handle_packet(&self, data: &[u8], addr: SocketAddr) {
        let packet: GossipPacket = match serde_json::from_slice(data) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(addr = %addr, error = %e, "Invalid gossip packet");
                return;
            }
        };

        // Don't process our own packets
        if packet.node_id == self.node_id {
            return;
        }

        tracing::trace!(
            node = %packet.node_id,
            cpu = packet.cpu_load,
            conns = packet.active_conns,
            "Received gossip"
        );

        // Update peer state
        let mut peers = self.peers.write().await;
        peers.insert(
            packet.node_id.clone(),
            NodeHealth {
                last_packet: packet,
                last_seen: Instant::now(),
                is_healthy: true,
            },
        );
    }

    /// Check peer health and isolation status
    async fn check_peer_health(&self, timeout: Duration) {
        let mut peers = self.peers.write().await;
        let total_peers = peers.len();
        let mut unhealthy_count = 0;

        for health in peers.values_mut() {
            if health.last_seen.elapsed() > timeout {
                if health.is_healthy {
                    tracing::warn!(
                        node = %health.last_packet.node_id,
                        "Peer marked unhealthy (timeout)"
                    );
                }
                health.is_healthy = false;
                unhealthy_count += 1;
            }
        }

        drop(peers);

        // Check isolation
        if total_peers > 0 {
            let unhealthy_ratio = unhealthy_count as f32 / total_peers as f32;
            let isolated = unhealthy_ratio >= self.config.isolation_threshold;

            let mut is_isolated = self.isolated.write().await;
            if isolated != *is_isolated {
                if isolated {
                    tracing::error!(
                        unhealthy = unhealthy_count,
                        total = total_peers,
                        "⚠️ Node is ISOLATED from cluster"
                    );
                } else {
                    tracing::info!("✅ Node reconnected to cluster");
                }
                *is_isolated = isolated;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gossip_packet_serialization() {
        let packet = GossipPacket::new(
            "node-1".to_string(),
            45,
            true,
            1234,
            80,
            2,
        );

        let json = serde_json::to_string(&packet).unwrap();
        let parsed: GossipPacket = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.node_id, "node-1");
        assert_eq!(parsed.cpu_load, 45);
        assert!(parsed.tor_health);
    }
}
