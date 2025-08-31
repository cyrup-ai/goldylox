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

pub use hardware::{AccessPatternHint, HardwarePrefetchStats, HardwarePrefetcher, PrefetchHint};
pub use pattern_detection::PatternDetector;
pub use prediction::{PatternDistribution, PredictionEngine, PredictionEngineStats};
pub use queue_manager::{ConfidenceDistribution, QueueManager, QueueStats};
pub use statistics::PredictionStats;
pub use types::{
    AccessPattern, AccessSequence, DetectedPattern, PredictionConfidence,
    PrefetchConfig, PrefetchRequest, PrefetchStats,
};
