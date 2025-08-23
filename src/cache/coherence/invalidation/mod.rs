//! Invalidation management for cache coherence protocol
//!
//! This module handles coordinated invalidations across cache tiers to maintain
//! consistency and implements invalidation queuing and retry mechanisms.

pub mod configuration;
pub mod manager;
pub mod statistics;
pub mod types;

// Re-export main types for convenience
pub use configuration::InvalidationConfig;
pub use manager::InvalidationManager;
pub use statistics::{InvalidationStatistics, InvalidationStatisticsSnapshot};
pub use types::{InvalidationPriority, InvalidationRequest, InvalidationResult};
