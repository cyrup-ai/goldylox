//! Write propagation system for cache coherence protocol
//!
//! This module implements write-back and write-through mechanisms for maintaining
//! data consistency across cache tiers with background processing capabilities.

// Internal write propagation architecture - components may not be used in minimal API

pub mod configuration;
pub mod engine;
pub mod statistics;
pub mod types;
pub mod worker_system;

// Re-export only actually used types
pub use types::WritePropagationSystem;
