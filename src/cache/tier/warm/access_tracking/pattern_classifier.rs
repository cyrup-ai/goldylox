//! Temporal pattern classifier for access prediction
//!
//! This module implements pattern detection algorithms for classifying
//! access sequences into temporal patterns like periodic, sequential, burst, or random.

#![allow(dead_code)] // Warm tier access tracking - Complete pattern classification library for temporal pattern analysis

use crossbeam_utils::atomic::AtomicCell;

use super::confidence_tracker::ConfidenceTracker;
use super::types::{ClassificationParams, PatternState};
use crate::cache::traits::TemporalPattern;

/// Temporal pattern classifier for access prediction
#[derive(Debug)]
pub struct TemporalPatternClassifier {
    /// Classification parameters
    params: ClassificationParams,
    /// Current pattern state
    current_pattern: AtomicCell<PatternState>,
    /// Pattern confidence tracker
    confidence_tracker: ConfidenceTracker,
}

impl Default for TemporalPatternClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl TemporalPatternClassifier {
    /// Create new temporal pattern classifier
    pub fn new() -> Self {
        Self {
            params: ClassificationParams::default(),
            current_pattern: AtomicCell::new(PatternState::default()),
            confidence_tracker: ConfidenceTracker::new(),
        }
    }

    /// Classify access sequence patterns
    pub fn classify_sequence(&self, sequence: &[u64], window_size: usize) {
        if sequence.len() < window_size {
            return;
        }

        // Analyze intervals between accesses
        let mut intervals = Vec::new();
        for window in sequence.windows(2) {
            intervals.push(window[1] - window[0]);
        }

        // Detect pattern based on interval analysis
        let pattern = self.detect_pattern_from_intervals(&intervals);
        let confidence = self.calculate_pattern_confidence(&intervals, pattern);

        // Update current pattern if confidence exceeds threshold
        if confidence > self.params.confidence_threshold as f32 {
            let new_state = PatternState {
                pattern_type: pattern,
                confidence,
            };
            self.current_pattern.store(new_state);
            self.confidence_tracker
                .update_pattern_confidence(pattern, confidence);
        }
    }

    /// Detect temporal pattern from interval analysis
    fn detect_pattern_from_intervals(&self, intervals: &[u64]) -> TemporalPattern {
        if intervals.len() < 3 {
            return TemporalPattern::Random;
        }

        // Calculate variance in intervals
        let mean = intervals.iter().sum::<u64>() as f64 / intervals.len() as f64;
        let variance = intervals
            .iter()
            .map(|&x| {
                let diff = x as f64 - mean;
                diff * diff
            })
            .sum::<f64>()
            / intervals.len() as f64;

        let std_dev = variance.sqrt();
        let coefficient_variation = std_dev / mean;

        // Classify based on coefficient of variation
        if coefficient_variation < 0.1 {
            TemporalPattern::Periodic
        } else if coefficient_variation < 0.3 {
            TemporalPattern::Sequential
        } else if self.is_burst_pattern(intervals) {
            TemporalPattern::Bursty
        } else {
            TemporalPattern::Random
        }
    }

    /// Check if intervals indicate burst pattern
    fn is_burst_pattern(&self, intervals: &[u64]) -> bool {
        if intervals.len() < 5 {
            return false;
        }

        // Look for clusters of short intervals followed by long gaps
        let threshold = 1_000_000; // 1ms
        let mut short_count = 0;
        let mut has_long_gap = false;

        for &interval in intervals {
            if interval < threshold {
                short_count += 1;
            } else if interval > threshold * 100 {
                has_long_gap = true;
            }
        }

        short_count >= 3 && has_long_gap
    }

    /// Calculate confidence for detected pattern
    fn calculate_pattern_confidence(&self, intervals: &[u64], pattern: TemporalPattern) -> f32 {
        if intervals.is_empty() {
            return 0.0;
        }

        match pattern {
            TemporalPattern::Periodic => self.calculate_periodic_confidence(intervals),
            TemporalPattern::Sequential => self.calculate_sequential_confidence(intervals),
            TemporalPattern::Bursty => self.calculate_burst_confidence(intervals),
            TemporalPattern::Random => 0.1, // Low confidence for random
            TemporalPattern::Steady => 0.8, // High confidence for steady patterns
            TemporalPattern::BurstyHigh => self.calculate_burst_confidence(intervals),
            TemporalPattern::Declining => 0.6, // Medium confidence for declining patterns
            TemporalPattern::WriteHeavy => 0.7, // Medium-high confidence for write patterns
            TemporalPattern::Irregular => 0.2, // Low confidence for irregular patterns
        }
    }

    /// Calculate confidence for periodic pattern
    fn calculate_periodic_confidence(&self, intervals: &[u64]) -> f32 {
        let mean = intervals.iter().sum::<u64>() as f64 / intervals.len() as f64;
        let variance = intervals
            .iter()
            .map(|&x| {
                let diff = x as f64 - mean;
                diff * diff
            })
            .sum::<f64>()
            / intervals.len() as f64;

        let std_dev = variance.sqrt();
        let coefficient_variation = std_dev / mean;

        // High confidence for low variation
        (1.0 - coefficient_variation.min(1.0)) as f32
    }

    /// Calculate confidence for sequential pattern
    fn calculate_sequential_confidence(&self, intervals: &[u64]) -> f32 {
        // Check for gradual changes in intervals
        let mut trend_consistency = 0;
        for window in intervals.windows(3) {
            let trend1 = window[1] as i64 - window[0] as i64;
            let trend2 = window[2] as i64 - window[1] as i64;

            if (trend1 > 0 && trend2 > 0) || (trend1 < 0 && trend2 < 0) {
                trend_consistency += 1;
            }
        }

        if intervals.len() >= 3 {
            trend_consistency as f32 / (intervals.len() - 2) as f32
        } else {
            0.5
        }
    }

    /// Calculate confidence for burst pattern
    fn calculate_burst_confidence(&self, intervals: &[u64]) -> f32 {
        let threshold = 1_000_000; // 1ms
        let short_intervals = intervals.iter().filter(|&&x| x < threshold).count();
        let long_intervals = intervals.iter().filter(|&&x| x > threshold * 10).count();

        let burst_ratio = (short_intervals + long_intervals) as f32 / intervals.len() as f32;
        burst_ratio.min(1.0)
    }

    /// Get current pattern state
    pub fn get_current_pattern(&self) -> PatternState {
        self.current_pattern.load()
    }

    /// Get confidence tracker reference
    pub fn get_confidence_tracker(&self) -> &ConfidenceTracker {
        &self.confidence_tracker
    }

    /// Update classification parameters
    pub fn update_params(&mut self, params: ClassificationParams) {
        self.params = params;
    }

    /// Get current classification parameters
    pub fn get_params(&self) -> &ClassificationParams {
        &self.params
    }
}
