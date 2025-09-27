//! Confidence tracker for pattern classification
//!
//! This module provides confidence tracking and statistics for pattern
//! classification accuracy and reliability assessment.

#![allow(dead_code)] // Warm tier access tracking - Complete confidence tracking library for pattern classification reliability

use std::sync::atomic::Ordering;

use crossbeam_skiplist::SkipMap;

use super::types::{ConfidenceData, GlobalConfidenceStats};
use crate::cache::traits::TemporalPattern;

/// Confidence tracker for pattern classification
#[derive(Debug)]
pub struct ConfidenceTracker {
    /// Confidence levels per pattern
    pattern_confidences: SkipMap<TemporalPattern, ConfidenceData>,
    /// Global confidence statistics
    global_stats: GlobalConfidenceStats,
}

impl Default for ConfidenceTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfidenceTracker {
    /// Create new confidence tracker
    pub fn new() -> Self {
        Self {
            pattern_confidences: SkipMap::new(),
            global_stats: GlobalConfidenceStats::new(),
        }
    }

    /// Update confidence for specific pattern
    pub fn update_pattern_confidence(&self, pattern: TemporalPattern, confidence: f32) {
        if let Some(data) = self.pattern_confidences.get(&pattern) {
            data.value().confidence.store(confidence);
            data.value().sample_count.fetch_add(1, Ordering::Relaxed);
        } else {
            let data = ConfidenceData {
                confidence: crossbeam_utils::atomic::AtomicCell::new(confidence),
                accuracy: crossbeam_utils::atomic::AtomicCell::new(0.8), // Default accuracy
                sample_count: std::sync::atomic::AtomicU64::new(1),
            };
            self.pattern_confidences.insert(pattern, data);
        }

        self.global_stats.update_confidence(confidence);
    }

    /// Get confidence for pattern
    pub fn get_confidence(&self, pattern: TemporalPattern) -> f32 {
        self.pattern_confidences
            .get(&pattern)
            .map(|entry| entry.value().confidence.load())
            .unwrap_or(0.5)
    }

    /// Get accuracy for pattern
    pub fn get_accuracy(&self, pattern: TemporalPattern) -> f32 {
        self.pattern_confidences
            .get(&pattern)
            .map(|entry| entry.value().accuracy.load())
            .unwrap_or(0.8)
    }

    /// Get sample count for pattern
    pub fn get_sample_count(&self, pattern: TemporalPattern) -> u64 {
        self.pattern_confidences
            .get(&pattern)
            .map(|entry| entry.value().sample_count.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    /// Update accuracy for pattern based on prediction results
    pub fn update_accuracy(&self, pattern: TemporalPattern, was_correct: bool) {
        if let Some(data) = self.pattern_confidences.get(&pattern) {
            let current_accuracy = data.value().accuracy.load();
            let sample_count = data.value().sample_count.load(Ordering::Relaxed);

            // Update accuracy using running average
            let new_accuracy = if sample_count > 0 {
                let weight = 1.0 / (sample_count as f32 + 1.0);
                let correction = if was_correct { 1.0 } else { 0.0 };
                current_accuracy * (1.0 - weight) + correction * weight
            } else if was_correct {
                1.0
            } else {
                0.0
            };

            data.value().accuracy.store(new_accuracy);
        }
    }

    /// Get global confidence statistics
    pub fn get_global_stats(&self) -> &GlobalConfidenceStats {
        &self.global_stats
    }

    /// Get average confidence across all patterns
    pub fn get_average_confidence(&self) -> f32 {
        self.global_stats.avg_confidence.load()
    }

    /// Get global accuracy rate
    pub fn get_accuracy_rate(&self) -> f32 {
        self.global_stats.accuracy_rate.load()
    }

    /// Get total number of predictions made
    pub fn get_total_predictions(&self) -> u64 {
        self.global_stats.total_predictions.load(Ordering::Relaxed)
    }

    /// Clear all confidence data
    pub fn clear(&self) {
        self.pattern_confidences.clear();
        self.global_stats.avg_confidence.store(0.5);
        self.global_stats.accuracy_rate.store(0.8);
        self.global_stats
            .total_predictions
            .store(0, Ordering::Relaxed);
    }

    /// Get confidence statistics for all patterns
    pub fn get_all_pattern_stats(&self) -> Vec<(TemporalPattern, f32, f32, u64)> {
        let mut stats = Vec::new();

        for entry in self.pattern_confidences.iter() {
            let pattern = *entry.key();
            let data = entry.value();
            let confidence = data.confidence.load();
            let accuracy = data.accuracy.load();
            let sample_count = data.sample_count.load(Ordering::Relaxed);

            stats.push((pattern, confidence, accuracy, sample_count));
        }

        stats
    }
}
