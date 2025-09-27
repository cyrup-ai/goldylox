//! Adaptive eviction with machine learning for hot tier cache
//!
//! This module implements intelligent eviction policies using SIMD-accelerated
//! algorithms and adaptive scoring for optimal cache performance.

pub mod engine;
pub mod machine_learning;
pub mod policies;
pub mod types;

// Re-export main types and structs
// Re-export engine implementation
pub use engine::EvictionEngine;
pub(crate) use types::EvictionStats;

// Re-export canonical config
