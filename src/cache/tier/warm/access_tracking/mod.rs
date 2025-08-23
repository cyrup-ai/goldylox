//! Concurrent access tracking and pattern analysis for warm tier cache
//!
//! This module implements lock-free access pattern tracking, temporal pattern
//! classification, and frequency estimation for intelligent cache management.

pub mod confidence_tracker;
pub mod frequency_estimator;
pub mod pattern_classifier;
pub mod tracker;
pub mod types;

// Re-export main types for convenience
pub use confidence_tracker::ConfidenceTracker;
pub use frequency_estimator::FrequencyEstimator;
pub use pattern_classifier::TemporalPatternClassifier;
pub use tracker::{AccessAnalysisTask, ConcurrentAccessTracker};
pub use types::{
    AccessContext, AccessRecord, AccessType, ClassificationParams, ConfidenceData,
    GlobalConfidenceStats, HitStatus, PatternState, PatternType, TemporalPattern,
};
