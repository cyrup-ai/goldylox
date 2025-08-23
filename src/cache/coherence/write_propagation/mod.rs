//! Write propagation system for cache coherence protocol
//!
//! This module implements write-back and write-through mechanisms for maintaining
//! data consistency across cache tiers with background processing capabilities.

pub mod configuration;
pub mod engine;
pub mod statistics;
pub mod types;
pub mod worker_system;

// Re-export main types and structs
// Re-export configuration presets
pub use types::{
    PropagationConfig, PropagationPolicy, PropagationStatistics, PropagationStatisticsSnapshot,
    WorkerHealth, WriteBackRequest, WritePriority, WritePropagationSystem,
};
// Re-export worker system components
pub use worker_system::{WorkerChannels, WriteBackCompletion, WriteBackResult, WriteBackTask};
