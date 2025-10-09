//! Type definitions for access tracking and pattern analysis
//!
//! This module contains all the core types, enums, and data structures
//! used for access pattern tracking and classification.

#![allow(dead_code)] // Warm tier access tracking - Complete type definitions for access pattern tracking and classification

use std::sync::atomic::{AtomicU64, Ordering};

use crossbeam_utils::atomic::AtomicCell;

pub(crate) use crate::cache::traits::{AccessType, TemporalPattern};
pub use crate::cache::types::error_types::HitStatus; // Canonical location

// AccessType moved to canonical location: crate::cache::traits::types_and_enums

/// Pattern type classification for access prediction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternType {
    /// Sequential access pattern
    Sequential,
    /// Random access pattern
    Random,
    /// Temporal locality pattern
    Temporal,
    /// Spatial locality pattern
    Spatial,
    /// Burst access pattern
    Burst,
    /// Working set pattern
    Working,
    /// Unknown or mixed pattern
    Unknown,
}

/// Classification parameters for pattern recognition
#[derive(Debug, Clone)]
pub struct ClassificationParams {
    /// Minimum sample size for pattern detection
    pub min_sample_size: usize,
    /// Confidence threshold for pattern confirmation (0.0-1.0)
    pub confidence_threshold: f64,
    /// Temporal window for pattern analysis (nanoseconds)
    pub temporal_window_ns: u64,
    /// Sequential access threshold (distance between accesses)
    pub sequential_threshold: usize,
    /// Burst detection threshold (accesses per time window)
    pub burst_threshold: u32,
    /// Spatial locality threshold (memory distance)
    pub spatial_threshold: usize,
}

/// Access context for pattern analysis
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessContext {
    /// Sequential context
    Sequential,
    /// Random context
    Random,
    /// Burst context
    Burst,
}

/// Temporal pattern classification for access prediction
// TemporalPattern moved to canonical location: crate::cache::traits::types_and_enums
/// Pattern state for temporal pattern tracking
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PatternState {
    /// Current pattern type
    pub pattern_type: TemporalPattern,
    /// Confidence level (0.0-1.0)
    pub confidence: f32,
}

// HitStatus CANONICALIZED: moved to canonical location:
// crate::cache::types::error_types::HitStatus
// Use the enhanced canonical version with Error and Partial support for multi-tier architecture

/// Access record for pattern analysis
#[derive(Debug, Clone)]
pub struct AccessRecord {
    /// Access timestamp
    pub timestamp_ns: u64,
    /// Access type classification
    pub access_type: AccessType,
    /// Thread ID that made the access
    pub thread_id: u64,
    /// Hit/miss status
    pub hit_status: HitStatus,
    /// Key hash for spatial locality analysis
    pub key_hash: u64,
}

/// Confidence data for specific patterns
#[derive(Debug)]
pub struct ConfidenceData {
    /// Current confidence level
    pub confidence: AtomicCell<f32>,
    /// Historical accuracy
    pub accuracy: AtomicCell<f32>,
    /// Sample count
    pub sample_count: AtomicU64,
}

impl Clone for ConfidenceData {
    fn clone(&self) -> Self {
        Self {
            confidence: AtomicCell::new(self.confidence.load()),
            accuracy: AtomicCell::new(self.accuracy.load()),
            sample_count: AtomicU64::new(
                self.sample_count.load(std::sync::atomic::Ordering::Relaxed),
            ),
        }
    }
}

/// Global confidence statistics
#[derive(Debug)]
pub struct GlobalConfidenceStats {
    /// Average confidence across patterns
    pub avg_confidence: AtomicCell<f32>,
    /// Prediction accuracy rate
    pub accuracy_rate: AtomicCell<f32>,
    /// Total predictions made
    pub total_predictions: AtomicU64,
}

impl Default for ClassificationParams {
    fn default() -> Self {
        Self {
            min_sample_size: 10,
            confidence_threshold: 0.7,
            temporal_window_ns: 60_000_000_000, // 60 seconds
            sequential_threshold: 16,
            burst_threshold: 10,
            spatial_threshold: 4096,
        }
    }
}

impl Default for PatternState {
    fn default() -> Self {
        Self {
            pattern_type: TemporalPattern::Random,
            confidence: 0.0,
        }
    }
}

impl Default for GlobalConfidenceStats {
    fn default() -> Self {
        Self::new()
    }
}

impl GlobalConfidenceStats {
    /// Create new global confidence statistics
    pub fn new() -> Self {
        Self {
            avg_confidence: AtomicCell::new(0.5),
            accuracy_rate: AtomicCell::new(0.8),
            total_predictions: AtomicU64::new(0),
        }
    }

    /// Update global confidence statistics
    pub fn update_confidence(&self, new_confidence: f32) {
        let current_avg = self.avg_confidence.load();
        let total = self.total_predictions.fetch_add(1, Ordering::Relaxed) + 1;

        // Running average update
        let updated_avg = (current_avg * (total - 1) as f32 + new_confidence) / total as f32;
        self.avg_confidence.store(updated_avg);
    }
}
