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

impl PredictionConfidence {
    /// Convert confidence level to numeric value for comparisons and calculations
    pub fn as_f64(&self) -> f64 {
        match self {
            PredictionConfidence::Low => 0.25,
            PredictionConfidence::Medium => 0.50,
            PredictionConfidence::High => 0.75,
            PredictionConfidence::VeryHigh => 0.95,
        }
    }
}

/// Enhanced prefetch request with metadata
#[derive(Debug, Clone)]
#[allow(dead_code)] // Hot tier prefetch - Core prefetch request structure with pattern prediction and priority scheduling
pub struct PrefetchRequest<K: CacheKey> {
    pub key: K,
    pub confidence: PredictionConfidence,
    pub predicted_access_time: u64,
    pub pattern_type: AccessPattern,
    pub priority: u8,
    /// Request timestamp
    pub timestamp_ns: u64,
    /// Expected access pattern
    #[allow(dead_code)]
    // Hot tier prefetch - Access pattern prediction for intelligent prefetching
    pub access_pattern: Option<AccessPattern>,
    /// Estimated value size in bytes
    #[allow(dead_code)]
    // Hot tier prefetch - Size estimation for memory planning and cache capacity management
    pub estimated_size: Option<usize>,
}

/// Access pattern types for prediction
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AccessPattern {
    Sequential,
    Temporal,
    Spatial,
    Periodic,
    #[allow(dead_code)] // Hot tier prefetch - Contextual pattern for semantic access relationships
    Contextual,
    #[allow(dead_code)] // Hot tier prefetch - Random pattern for unpredictable access behavior
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

// PredictionStats moved to canonical location:
// crate::cache::tier::hot::prefetch::statistics::PredictionStats

/// Prefetch configuration
#[derive(Debug, Clone)]
pub struct PrefetchConfig {
    pub history_size: usize,
    #[allow(dead_code)]
    // Hot tier prefetch - Maximum lookahead distance for sequential prefetching
    pub max_prefetch_distance: usize,
    pub min_confidence_threshold: f64,
    pub pattern_detection_window: Duration,
    pub max_patterns: usize,
    pub prefetch_queue_size: usize,
}

impl Default for PrefetchConfig {
    fn default() -> Self {
        Self {
            history_size: 1000,
            max_prefetch_distance: 5,
            min_confidence_threshold: 0.6,
            pattern_detection_window: Duration::from_secs(300),
            max_patterns: 100,
            prefetch_queue_size: 50,
        }
    }
}

/// Comprehensive prefetch statistics for external reporting (CANONICAL - Best of Best)
/// Combines comprehensive hot tier features with policy engine performance metrics
#[derive(Debug, Clone)]
pub struct PrefetchStats {
    pub total_predictions: u64,
    pub accuracy: f64,
    pub hit_rate: f64,
    pub patterns_detected: usize,
    pub queue_size: usize,
    pub avg_confidence: f64,
    // Enhanced fields from policy engine version for performance tracking
    #[allow(dead_code)]
    // Hot tier prefetch - Average latency tracking for performance optimization
    pub average_latency_ns: u64,
    #[allow(dead_code)]
    // Hot tier prefetch - Success count for hit rate calculation and effectiveness metrics
    pub successful_count: u64,
    #[allow(dead_code)]
    // Hot tier prefetch - Failure count for accuracy analysis and error monitoring
    pub failed_count: u64,
}

impl PrefetchStats {
    /// Check if prefetching is effective
    #[allow(dead_code)] // Hot tier prefetch - Effectiveness evaluation for adaptive prefetching strategies
    pub fn is_effective(&self, threshold: f64) -> bool {
        self.hit_rate >= threshold && self.accuracy >= threshold
    }

    /// Get overall prefetch score
    #[allow(dead_code)] // Hot tier prefetch - Overall effectiveness scoring for algorithm comparison
    pub fn effectiveness_score(&self) -> f64 {
        (self.hit_rate + self.accuracy) / 2.0
    }

    /// Merge statistics from another PrefetchStats
    #[allow(dead_code)] // Hot tier prefetch - Statistics aggregation for multi-tier reporting
    pub fn merge(&mut self, other: PrefetchStats) {
        // Sum total predictions
        self.total_predictions += other.total_predictions;

        // Average accuracy, hit rate, and confidence
        self.accuracy = (self.accuracy + other.accuracy) / 2.0;
        self.hit_rate = (self.hit_rate + other.hit_rate) / 2.0;
        self.avg_confidence = (self.avg_confidence + other.avg_confidence) / 2.0;

        // Sum patterns detected and queue size
        self.patterns_detected += other.patterns_detected;
        self.queue_size += other.queue_size;

        // Enhanced fields from policy engine: average latency and sum counts
        self.average_latency_ns = (self.average_latency_ns + other.average_latency_ns) / 2;
        self.successful_count += other.successful_count;
        self.failed_count += other.failed_count;
    }
}

impl Default for PrefetchStats {
    fn default() -> Self {
        Self {
            total_predictions: 0,
            accuracy: 0.0,
            hit_rate: 0.0,
            patterns_detected: 0,
            queue_size: 0,
            avg_confidence: 0.0,
            // Enhanced fields from policy engine version
            average_latency_ns: 0,
            successful_count: 0,
            failed_count: 0,
        }
    }
}

impl<K: CacheKey> PrefetchRequest<K> {
    /// Create new prefetch request with current timestamp
    #[allow(dead_code)] // Hot tier prefetch - Core constructor for prefetch request creation
    pub fn new(
        key: K,
        confidence: PredictionConfidence,
        predicted_access_time: u64,
        pattern_type: AccessPattern,
        priority: u8,
    ) -> Self {
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
    #[allow(dead_code)] // Hot tier prefetch - Builder pattern for enhanced request metadata
    pub fn with_pattern(mut self, pattern: AccessPattern) -> Self {
        self.access_pattern = Some(pattern);
        self
    }

    /// Add size estimation
    #[allow(dead_code)] // Hot tier prefetch - Builder pattern for memory capacity planning
    pub fn with_estimated_size(mut self, size: usize) -> Self {
        self.estimated_size = Some(size);
        self
    }
}
