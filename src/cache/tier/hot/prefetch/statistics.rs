//! Prediction statistics and metrics tracking
//!
//! Provides comprehensive statistics tracking for prefetch prediction
//! accuracy and performance analysis.

#![allow(dead_code)] // Hot tier prefetch - Complete statistics library for prefetch prediction analysis and performance tracking

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

impl PredictionStats {
    /// Create new prediction statistics
    #[allow(dead_code)] // Hot tier prefetch - Statistics constructor for prediction performance tracking
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate prediction accuracy
    #[allow(dead_code)] // Hot tier prefetch - Accuracy calculation for prediction engine optimization
    pub fn accuracy(&self) -> f64 {
        if self.total_predictions > 0 {
            self.correct_predictions as f64 / self.total_predictions as f64
        } else {
            0.0
        }
    }

    /// Calculate prefetch hit rate
    #[allow(dead_code)] // Hot tier prefetch - Hit rate calculation for prefetch effectiveness analysis
    pub fn hit_rate(&self) -> f64 {
        let total_prefetches = self.prefetch_hits + self.prefetch_misses;
        if total_prefetches > 0 {
            self.prefetch_hits as f64 / total_prefetches as f64
        } else {
            0.0
        }
    }

    /// Record a correct prediction
    #[allow(dead_code)] // Hot tier prefetch - Correct prediction recording for accuracy tracking
    pub fn record_correct_prediction(&mut self) {
        self.correct_predictions += 1;
        self.total_predictions += 1;
    }

    /// Record a false prediction
    #[allow(dead_code)] // Hot tier prefetch - False prediction recording for accuracy analysis
    pub fn record_false_prediction(&mut self) {
        self.false_predictions += 1;
        self.total_predictions += 1;
    }

    /// Record a prefetch hit
    #[allow(dead_code)] // Hot tier prefetch - Hit recording for effectiveness measurement
    pub fn record_prefetch_hit(&mut self) {
        self.prefetch_hits += 1;
    }

    /// Record a prefetch miss
    #[allow(dead_code)] // Hot tier prefetch - Miss recording for effectiveness analysis
    pub fn record_prefetch_miss(&mut self) {
        self.prefetch_misses += 1;
    }

    /// Record pattern detection
    #[allow(dead_code)] // Hot tier prefetch - Pattern detection recording for analysis metrics
    pub fn record_pattern_detection(&mut self) {
        self.pattern_detections += 1;
    }

    /// Reset all statistics
    #[allow(dead_code)] // Hot tier prefetch - Statistics reset for periodic measurement windows
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Get summary statistics
    #[allow(dead_code)] // Hot tier prefetch - Summary generation for performance reporting
    pub fn summary(&self) -> StatsSummary {
        StatsSummary {
            total_predictions: self.total_predictions,
            accuracy: self.accuracy(),
            hit_rate: self.hit_rate(),
            pattern_detections: self.pattern_detections,
        }
    }
}

/// Statistics summary for reporting
#[derive(Debug)]
#[allow(dead_code)] // Hot tier prefetch - Statistics summary structure for performance reporting
pub struct StatsSummary {
    pub total_predictions: u64,
    pub accuracy: f64,
    pub hit_rate: f64,
    pub pattern_detections: u64,
}
