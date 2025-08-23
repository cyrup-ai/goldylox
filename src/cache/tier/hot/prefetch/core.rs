//! Core prefetch predictor implementation
//!
//! This module provides the main PrefetchPredictor struct that coordinates
//! pattern detection, prediction generation, and queue management.

use std::collections::VecDeque;

use super::pattern_detection::PatternDetector;
use super::prediction::PredictionEngine;
use super::queue_manager::QueueManager;
use super::types::{
    AccessSequence, DetectedPattern, PredictionStats, PrefetchConfig, PrefetchRequest,
    PrefetchStats,
};
use crate::cache::traits::CacheKey;

/// Prefetch predictor with pattern analysis
#[derive(Debug)]
pub struct PrefetchPredictor<K: CacheKey> {
    /// Recent access history for pattern detection
    access_history: VecDeque<AccessSequence<K>>,
    /// Detected patterns
    patterns: Vec<DetectedPattern<K>>,
    /// Pattern detector
    pattern_detector: PatternDetector,
    /// Prediction engine
    prediction_engine: PredictionEngine,
    /// Queue manager
    queue_manager: QueueManager<K>,
    /// Prediction statistics
    stats: PredictionStats,
    /// Configuration
    config: PrefetchConfig,
}

impl<K: CacheKey> PrefetchPredictor<K> {
    /// Create new prefetch predictor
    pub fn new(config: PrefetchConfig) -> Self {
        Self {
            access_history: VecDeque::with_capacity(config.history_size),
            patterns: Vec::with_capacity(config.max_patterns),
            pattern_detector: PatternDetector::new(config.clone()),
            prediction_engine: PredictionEngine::new(config.clone()),
            queue_manager: QueueManager::new(config.clone()),
            stats: PredictionStats::default(),
            config,
        }
    }

    /// Record cache access for pattern learning
    pub fn record_access(&mut self, key: &K, timestamp_ns: u64, context_hash: u64) {
        if !self.config.enabled {
            return;
        }

        let access = AccessSequence {
            key: key.clone(),
            timestamp: timestamp_ns,
            context_hash,
        };

        self.access_history.push_back(access);

        // Limit history size
        if self.access_history.len() > self.config.history_size {
            self.access_history.pop_front();
        }

        // Detect patterns periodically
        if self.access_history.len() % 50 == 0 {
            self.detect_patterns();
        }

        // Generate prefetch predictions
        self.generate_predictions(key, timestamp_ns, context_hash);
    }

    /// Detect access patterns in recent history
    fn detect_patterns(&mut self) {
        if self.access_history.len() < 10 {
            return;
        }

        // Detect new patterns
        let new_patterns = match self.pattern_detector.detect_patterns(&self.access_history) {
            Ok(patterns) => patterns,
            Err(_) => {
                // Pattern detection failed, skip this cycle
                return;
            }
        };

        // Add or update patterns
        for new_pattern in new_patterns {
            self.add_or_update_pattern(new_pattern);
        }

        // Clean up old patterns
        self.pattern_detector
            .cleanup_old_patterns(&mut self.patterns);
        self.stats.pattern_detections += 1;
    }

    /// Add or update pattern in collection
    fn add_or_update_pattern(&mut self, new_pattern: DetectedPattern<K>) {
        // Check if similar pattern already exists
        for existing in &mut self.patterns {
            if existing.pattern_type == new_pattern.pattern_type
                && self
                    .pattern_detector
                    .patterns_similar(&existing.sequence, &new_pattern.sequence)
            {
                existing.frequency += new_pattern.frequency;
                existing.last_seen = new_pattern.last_seen;
                existing.confidence = (existing.confidence + new_pattern.confidence) / 2.0;
                return;
            }
        }

        // Add new pattern if we have space
        if self.patterns.len() < self.config.max_patterns {
            self.patterns.push(new_pattern);
        } else {
            // Replace oldest pattern
            if let Some(oldest_idx) = self
                .patterns
                .iter()
                .enumerate()
                .min_by_key(|(_, p)| p.last_seen)
                .map(|(i, _)| i)
            {
                self.patterns[oldest_idx] = new_pattern;
            }
        }
    }

    /// Generate prefetch predictions based on current access
    fn generate_predictions(&mut self, current_key: &K, timestamp_ns: u64, _context_hash: u64) {
        if !self.config.enabled {
            return;
        }

        // Generate predictions from patterns
        let predictions =
            self.prediction_engine
                .generate_predictions(current_key, timestamp_ns, &self.patterns);

        // Add predictions to queue
        self.queue_manager.add_prefetch_requests(predictions);
        self.stats.total_predictions += 1;
    }

    /// Get next prefetch request
    pub fn get_next_prefetch(&mut self) -> Option<PrefetchRequest<K>> {
        self.queue_manager.get_next_prefetch()
    }

    /// Get multiple prefetch requests
    pub fn get_next_prefetches(&mut self, count: usize) -> Vec<PrefetchRequest<K>> {
        self.queue_manager.get_next_prefetches(count)
    }

    /// Check if key should be prefetched
    pub fn should_prefetch(&self, key: &K) -> bool {
        self.queue_manager.should_prefetch(key)
    }

    /// Record prefetch result for learning
    pub fn record_prefetch_result(&mut self, key: &K, hit: bool) {
        if hit {
            self.stats.prefetch_hits += 1;
            self.stats.correct_predictions += 1;
        } else {
            self.stats.prefetch_misses += 1;
            self.stats.false_predictions += 1;
        }

        // Remove from queue if present
        self.queue_manager.remove_request(key);

        // Update pattern confidence based on result
        self.update_pattern_confidence_for_key(key, hit);
    }

    /// Update pattern confidence based on prefetch result
    fn update_pattern_confidence_for_key(&mut self, key: &K, hit: bool) {
        for pattern in &mut self.patterns {
            if pattern.sequence.contains(key) {
                self.prediction_engine
                    .update_pattern_confidence(pattern, hit);
            }
        }
    }

    /// Get prefetch statistics
    pub fn get_stats(&self) -> PrefetchStats {
        let accuracy = if self.stats.total_predictions > 0 {
            self.stats.correct_predictions as f64 / self.stats.total_predictions as f64
        } else {
            0.0
        };

        let hit_rate = if self.stats.prefetch_hits + self.stats.prefetch_misses > 0 {
            self.stats.prefetch_hits as f64
                / (self.stats.prefetch_hits + self.stats.prefetch_misses) as f64
        } else {
            0.0
        };

        PrefetchStats {
            enabled: self.config.enabled,
            total_predictions: self.stats.total_predictions,
            accuracy,
            hit_rate,
            patterns_detected: self.patterns.len(),
            queue_size: self.queue_manager.queue_len(),
            avg_confidence: self.calculate_avg_confidence(),
        }
    }

    /// Calculate average prediction confidence
    fn calculate_avg_confidence(&self) -> f64 {
        let queue_stats = self.queue_manager.get_queue_stats();
        let confidence_dist = &queue_stats.confidence_distribution;

        if queue_stats.total_requests == 0 {
            return 0.0;
        }

        let total_confidence = confidence_dist.low as f64 * 0.4
            + confidence_dist.medium as f64 * 0.65
            + confidence_dist.high as f64 * 0.8
            + confidence_dist.very_high as f64 * 0.95;

        total_confidence / queue_stats.total_requests as f64
    }

    /// Clear all patterns and predictions
    pub fn clear(&mut self) {
        self.access_history.clear();
        self.patterns.clear();
        self.queue_manager.clear();
        self.stats = PredictionStats::default();
    }

    /// Get queue status
    pub fn queue_status(&self) -> (usize, usize) {
        self.queue_manager.queue_status()
    }

    /// Get detailed queue statistics
    pub fn get_queue_stats(&self) -> super::queue_manager::QueueStats {
        self.queue_manager.get_queue_stats()
    }

    /// Get pattern statistics
    pub fn get_pattern_stats(&self) -> super::prediction::PredictionEngineStats {
        self.prediction_engine.get_prediction_stats(&self.patterns)
    }

    /// Update configuration
    pub fn update_config(&mut self, new_config: PrefetchConfig) {
        self.config = new_config.clone();
        self.queue_manager.update_config(new_config);
    }

    /// Get current configuration
    pub fn get_config(&self) -> &PrefetchConfig {
        &self.config
    }

    /// Remove expired requests
    pub fn cleanup_expired_requests(
        &mut self,
        current_time_ns: u64,
        expiry_threshold_ns: u64,
    ) -> usize {
        self.queue_manager
            .remove_expired_requests(current_time_ns, expiry_threshold_ns)
    }

    /// Get access history length
    pub fn access_history_len(&self) -> usize {
        self.access_history.len()
    }

    /// Get number of detected patterns
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }
}
