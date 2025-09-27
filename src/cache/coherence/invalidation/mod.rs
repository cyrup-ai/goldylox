//! Invalidation management for cache coherence protocol
//!
//! This module handles coordinated invalidations across cache tiers to maintain
//! consistency and implements invalidation queuing and retry mechanisms.

// Internal invalidation architecture - components may not be used in minimal API

pub mod configuration;
pub mod manager;
pub mod statistics;
pub mod types;

// Re-export only actually used types
pub use statistics::InvalidationStatistics;
