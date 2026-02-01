//! Circuit tracking module.
//!
//! Tracks Tor circuit state, rate limits, and reputation.

mod tracker;

pub use tracker::CircuitTracker;
