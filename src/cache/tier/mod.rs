//! Intelligent tier management with SIMD-optimized promotion/demotion system
//!
//! This module implements sophisticated tier promotion and demotion algorithms
//! using lock-free data structures and SIMD-accelerated scoring for optimal performance.

// Internal tier architecture - components may not be used in minimal API

// Declare submodules
pub mod cold;
pub mod criteria;
pub mod hot;
pub mod manager;
pub mod queue;
pub mod statistics;
pub mod warm;

// Re-export key types for public API
