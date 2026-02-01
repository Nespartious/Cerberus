//! Cluster coordination modules.
//!
//! Implements:
//! - Health Gossip Protocol (UDP broadcast)
//! - Passport Protocol (cryptographic inter-node trust)
//! - State synchronization

mod gossip;
mod passport;

pub use gossip::{GossipConfig, GossipPacket, GossipService, NodeHealth};
pub use passport::{PassportConfig, PassportService, PassportToken};
