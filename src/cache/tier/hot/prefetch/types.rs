//! Types and configuration for prefetch system
//!
//! This module contains all type definitions, enums, and configuration
//! structures used by the prefetch prediction system.

use std::time::Duration;

use crate::cache::tier::hot::types::HotTierError;
use crate::cache::traits::CacheKey;

/// Prefetch prediction confidence levels
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PredictionConfidence {
    Low,
    Medium,
    High,
    VeryHigh,
}

/// Enhanced prefetch request with metadata
#[derive(Debug, Clone)]
pub struct PrefetchRequest<K: CacheKey> {
    pub key: K,
    pub confidence: PredictionConfidence,
    pub predicted_access_time: u64,
    pub pattern_type: AccessPattern,
    pub priority: u8,
    /// Request timestamp
    pub timestamp_ns: u64,
    /// Expected access pattern
    pub access_pattern: Option<AccessPattern>,
    /// Estimated value size in bytes
    pub estimated_size: Option<usize>,
}

/// Access pattern types for prediction
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AccessPattern {
    Sequential,
    Temporal,
    Spatial,
    Periodic,
    Contextual,
    Random,
}

/// Access sequence for pattern analysis
#[derive(Debug, Clone)]
pub struct AccessSequence<K: CacheKey> {
    pub key: K,
    pub timestamp: u64,
    pub context_hash: u64,
}

/// Detected access pattern
#[derive(Debug, Clone)]
pub struct DetectedPattern<K: CacheKey> {
    pub pattern_type: AccessPattern,
    pub sequence: Vec<K>,
    pub confidence: f64,
    pub frequency: u32,
    pub last_seen: u64,
}

/// Pattern detection specific errors
#[derive(Debug, Clone)]
pub enum PatternDetectionError {
    /// Insufficient data to detect patterns
    InsufficientData,
    /// Empty access history during pattern analysis
    EmptyAccessHistory,
    /// Temporal data corruption or unavailable
    TemporalDataCorrupted,
    /// Pattern analysis interrupted by concurrent access
    ConcurrentAccessError,
    /// Memory pressure preventing pattern analysis
    MemoryPressure,
}

impl From<PatternDetectionError> for HotTierError {
    fn from(err: PatternDetectionError) -> Self {
        match err {
            PatternDetectionError::InsufficientData => HotTierError::NotFound,
            PatternDetectionError::EmptyAccessHistory => HotTierError::NotFound,
            PatternDetectionError::TemporalDataCorrupted => {
                HotTierError::SyncError("Temporal data corrupted".to_string())
            }
            PatternDetectionError::ConcurrentAccessError => {
                HotTierError::SyncError("Concurrent access error".to_string())
            }
            PatternDetectionError::MemoryPressure => HotTierError::MemoryLimitExceeded,
        }
    }
}

/// Prediction statistics
#[derive(Debug, Default)]
pub struct PredictionStats {
    pub total_predictions: u64,
    pub correct_predictions: u64,
    pub false_predictions: u64,
    pub prefetch_hits: u64,
    pub prefetch_misses: u64,
    pub pattern_detections: u64,
}

/// Prefetch configuration
#[derive(Debug, Clone)]
pub struct PrefetchConfig {
    pub enabled: bool,
    pub history_size: usize,
    pub max_prefetch_distance: usize,
    pub min_confidence_threshold: f64,
    pub pattern_detection_window: Duration,
    pub max_patterns: usize,
    pub prefetch_queue_size: usize,
}

impl Default for PrefetchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            history_size: 1000,
            max_prefetch_distance: 5,
            min_confidence_threshold: 0.6,
            pattern_detection_window: Duration::from_secs(300),
            max_patterns: 100,
            prefetch_queue_size: 50,
        }
    }
}

/// Prefetch statistics for external reporting
#[derive(Debug, Clone)]
pub struct PrefetchStats {
    pub enabled: bool,
    pub total_predictions: u64,
    pub accuracy: f64,
    pub hit_rate: f64,
    pub patterns_detected: usize,
    pub queue_size: usize,
    pub avg_confidence: f64,
}

impl PrefetchStats {
    /// Check if prefetching is effective
    pub fn is_effective(&self, threshold: f64) -> bool {
        self.hit_rate >= threshold && self.accuracy >= threshold
    }

    /// Get overall prefetch score
    pub fn effectiveness_score(&self) -> f64 {
        (self.hit_rate + self.accuracy) / 2.0
    }

    /// Merge statistics from another PrefetchStats
    pub fn merge(&mut self, other: PrefetchStats) {
        // Keep enabled if either is enabled
        self.enabled = self.enabled || other.enabled;

        // Sum total predictions
        self.total_predictions += other.total_predictions;

        // Average accuracy, hit rate, and confidence
        self.accuracy = (self.accuracy + other.accuracy) / 2.0;
        self.hit_rate = (self.hit_rate + other.hit_rate) / 2.0;
        self.avg_confidence = (self.avg_confidence + other.avg_confidence) / 2.0;

        // Sum patterns detected and queue size
        self.patterns_detected += other.patterns_detected;
        self.queue_size += other.queue_size;
    }
}

impl Default for PrefetchStats {
    fn default() -> Self {
        Self {
            enabled: false,
            total_predictions: 0,
            accuracy: 0.0,
            hit_rate: 0.0,
            patterns_detected: 0,
            queue_size: 0,
            avg_confidence: 0.0,
        }
    }
}

impl<K: CacheKey> PrefetchRequest<K> {
    /// Create new prefetch request with current timestamp
    pub fn new(key: K, confidence: PredictionConfidence, predicted_access_time: u64, pattern_type: AccessPattern, priority: u8) -> Self {
        Self {
            key,
            confidence,
            predicted_access_time,
            pattern_type,
            priority,
            timestamp_ns: crate::cache::types::timestamp_nanos(std::time::Instant::now()),
            access_pattern: None,
            estimated_size: None,
        }
    }
    
    /// Add access pattern prediction
    pub fn with_pattern(mut self, pattern: AccessPattern) -> Self {
        self.access_pattern = Some(pattern);
        self
    }
    
    /// Add size estimation
    pub fn with_estimated_size(mut self, size: usize) -> Self {
        self.estimated_size = Some(size);
        self
    }
}
