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
pub use tracker::ConcurrentAccessTracker;
// Canonical location
