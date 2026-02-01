//! # Cerberus Common
//!
//! Shared types, traits, and utilities used across Cerberus components.
//!
//! ## Modules
//! - `types` - Core data structures (ThreatLevel, CircuitState, etc.)
//! - `error` - Common error types
//! - `constants` - Shared configuration constants

pub mod constants;
pub mod error;
pub mod types;

pub use error::CerberusError;
pub use types::*;
