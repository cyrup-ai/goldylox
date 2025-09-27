//! Predictive prefetching with access pattern analysis
//!
//! This module implements intelligent prefetching algorithms that predict
//! future cache accesses based on historical patterns and preload data.

pub mod core;
pub mod hardware;
pub mod pattern_detection;
pub mod prediction;
pub mod queue_manager;
pub mod statistics;
pub mod types;

// Re-export main types for convenience
pub use core::PrefetchPredictor;

pub use types::{PrefetchConfig, PrefetchStats};
