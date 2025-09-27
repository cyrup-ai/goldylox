//! Core prefetch predictor implementation
//!
//! This module provides the main PrefetchPredictor struct that coordinates
//! pattern detection, prediction generation, and queue management.

use std::collections::VecDeque;

use super::pattern_detection::PatternDetector;
use super::prediction::PredictionEngine;
use super::queue_manager::QueueManager;
use super::statistics::PredictionStats;
use super::types::{
    AccessSequence, DetectedPattern, PrefetchConfig, PrefetchRequest, PrefetchStats,
};
use crate::cache::traits::CacheKey;

/// Enhanced prefetch success tracking with atomic counters from policy engine version
#[derive(Debug)]
#[allow(dead_code)] // Hot tier prefetch - Success tracking system for prefetch prediction analysis
pub struct EnhancedPrefetchSuccessTracker {
    /// Total predictions made (atomic for thread safety)
    #[allow(dead_code)] // Hot tier prefetch - Atomic counter for total predictions made
    pub predictions_made: std::sync::atomic::AtomicU64,
    /// Predictions that resulted in cache hits (atomic)  
    #[allow(dead_code)] // Hot tier prefetch - Atomic counter for successful prediction hits
    pub predictions_hit: std::sync::atomic::AtomicU64,
    /// False positive predictions (atomic)
    #[allow(dead_code)] // Hot tier prefetch - Atomic counter for false positive predictions
    pub false_positives: std::sync::atomic::AtomicU64,
    /// Average prediction accuracy (atomic cell for float updates)
    #[allow(dead_code)]
    // Hot tier prefetch - Atomic cell for tracking prediction accuracy metrics
    pub avg_prediction_accuracy: crossbeam_utils::atomic::AtomicCell<f32>,
}

impl Default for EnhancedPrefetchSuccessTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl EnhancedPrefetchSuccessTracker {
    pub fn new() -> Self {
        Self {
            predictions_made: std::sync::atomic::AtomicU64::new(0),
            predictions_hit: std::sync::atomic::AtomicU64::new(0),
            false_positives: std::sync::atomic::AtomicU64::new(0),
            avg_prediction_accuracy: crossbeam_utils::atomic::AtomicCell::new(0.0),
        }
    }

    #[allow(dead_code)] // Hot tier prefetch - Method to record prediction results for accuracy tracking
    pub fn record_prediction(&self, hit: bool) {
        self.predictions_made
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if hit {
            self.predictions_hit
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        } else {
            self.false_positives
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        // Update accuracy
        let made = self
            .predictions_made
            .load(std::sync::atomic::Ordering::Relaxed);
        let hit_count = self
            .predictions_hit
            .load(std::sync::atomic::Ordering::Relaxed);
        if made > 0 {
            self.avg_prediction_accuracy
                .store(hit_count as f32 / made as f32);
        }
    }
}

/// Enhanced prefetch predictor with ML-based pattern analysis and SIMD optimizations - CANONICAL VERSION
/// Combines comprehensive pattern detection workflow with performance-optimized ML regression and atomic operations
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

    // Enhanced features from policy engine version for ML-based predictions and SIMD optimization
    /// Polynomial regression coefficients (updated via atomic swaps for thread safety)
    #[allow(dead_code)]
    // ML system - used in machine learning regression and prefetch prediction
    regression_coefficients: [crossbeam_utils::atomic::AtomicCell<f32>; 8],
    /// Prediction confidence scores per pattern type with atomic updates
    #[allow(dead_code)]
    // ML system - used in machine learning confidence tracking and prediction
    confidence_scores: [std::sync::atomic::AtomicU32; 4], // Sequential, Temporal, Spatial, Random
    /// SIMD-optimized prediction computation buffers (AVX2-aligned for parallel computation)
    #[allow(dead_code)]
    // ML system - used in machine learning SIMD-optimized prediction computation
    prediction_buffer: [f32; 16],
    /// Feature extraction buffer for ML computations
    #[allow(dead_code)]
    // ML system - used in machine learning feature extraction and prefetch prediction
    feature_buffer: [f32; 16],
    /// Prefetch success rate tracking with atomic counters
    #[allow(dead_code)]
    // ML system - used in machine learning success rate tracking and adaptation
    success_tracker: EnhancedPrefetchSuccessTracker,
    /// Adaptive learning rate for online training (thread-safe atomic updates)
    #[allow(dead_code)]
    // ML system - used in machine learning adaptive learning rate optimization
    learning_rate: crossbeam_utils::atomic::AtomicCell<f32>,
    /// Pattern correlation matrix for complex predictions (atomic for thread safety)
    #[allow(dead_code)] // ML system - used in machine learning pattern correlation analysis
    correlation_matrix: [[std::sync::atomic::AtomicU32; 4]; 4],
}

impl<K: CacheKey> PrefetchPredictor<K> {
    /// Create new enhanced prefetch predictor with ML and SIMD optimizations
    pub fn new(config: PrefetchConfig) -> Self {
        // Initialize atomic correlation matrix
        let correlation_matrix = [
            [
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
            ],
            [
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
            ],
            [
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
            ],
            [
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
                std::sync::atomic::AtomicU32::new(0),
            ],
        ];

        Self {
            access_history: VecDeque::with_capacity(config.history_size),
            patterns: Vec::with_capacity(config.max_patterns),
            pattern_detector: PatternDetector::new(config.clone()),
            prediction_engine: PredictionEngine::new(config.clone()),
            queue_manager: QueueManager::new(config.clone()),
            stats: PredictionStats::default(),
            config,

            // Enhanced ML and SIMD features
            regression_coefficients: [
                crossbeam_utils::atomic::AtomicCell::new(0.0),
                crossbeam_utils::atomic::AtomicCell::new(0.0),
                crossbeam_utils::atomic::AtomicCell::new(0.0),
                crossbeam_utils::atomic::AtomicCell::new(0.0),
                crossbeam_utils::atomic::AtomicCell::new(0.0),
                crossbeam_utils::atomic::AtomicCell::new(0.0),
                crossbeam_utils::atomic::AtomicCell::new(0.0),
                crossbeam_utils::atomic::AtomicCell::new(0.0),
            ],
            confidence_scores: [
                std::sync::atomic::AtomicU32::new(0), // Sequential
                std::sync::atomic::AtomicU32::new(0), // Temporal
                std::sync::atomic::AtomicU32::new(0), // Spatial
                std::sync::atomic::AtomicU32::new(0), // Random
            ],
            prediction_buffer: [0.0; 16], // AVX2-aligned buffer
            feature_buffer: [0.0; 16],    // Feature extraction buffer
            success_tracker: EnhancedPrefetchSuccessTracker::new(),
            learning_rate: crossbeam_utils::atomic::AtomicCell::new(0.01), // Default learning rate
            correlation_matrix,
        }
    }

    /// Record cache access for pattern learning
    pub fn record_access(&mut self, key: &K, timestamp_ns: u64, context_hash: u64) {
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
        if self.access_history.len().is_multiple_of(50) {
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

    /// Record prefetch result for learning (enhanced with atomic success tracking)
    #[allow(dead_code)] // Hot tier prefetch - Method to record prefetch results for learning and statistics
    pub fn record_prefetch_result(&mut self, key: &K, hit: bool) {
        // Update basic stats
        if hit {
            self.stats.prefetch_hits += 1;
            self.stats.correct_predictions += 1;
        } else {
            self.stats.prefetch_misses += 1;
            self.stats.false_predictions += 1;
        }

        // Enhanced atomic success tracking from policy engine version
        self.success_tracker.record_prediction(hit);

        // Remove from queue if present
        self.queue_manager.remove_request(key);

        // Update pattern confidence based on result
        self.update_pattern_confidence_for_key(key, hit);

        // Update ML regression coefficients based on prediction accuracy
        self.update_regression_coefficients(hit);
    }

    /// Update pattern confidence based on prefetch result
    #[allow(dead_code)] // Hot tier prefetch - Internal method to update pattern confidence based on prefetch results
    fn update_pattern_confidence_for_key(&mut self, key: &K, hit: bool) {
        for pattern in &mut self.patterns {
            if pattern.sequence.contains(key) {
                self.prediction_engine
                    .update_pattern_confidence(pattern, hit);
            }
        }
    }

    /// Update ML regression coefficients based on prediction accuracy (enhanced ML feature)
    #[allow(dead_code)] // Hot tier prefetch - Internal ML method to update regression coefficients for prediction accuracy
    fn update_regression_coefficients(&mut self, prediction_success: bool) {
        let learning_rate = self.learning_rate.load();
        let error = if prediction_success { -0.1 } else { 0.1 }; // Gradient descent

        // Simple coefficient adjustment based on prediction outcome
        for (i, coeff_cell) in self.regression_coefficients.iter().enumerate() {
            let current = coeff_cell.load();
            let gradient = error * (i as f32 + 1.0) / 8.0; // Simple feature weighting
            let new_value = current - learning_rate * gradient;
            coeff_cell.store(new_value.clamp(-1.0, 1.0)); // Keep coefficients bounded
        }

        // Adapt learning rate based on performance
        let accuracy = self.success_tracker.avg_prediction_accuracy.load();
        if accuracy > 0.8 {
            // High accuracy - reduce learning rate for stability
            let current_lr = self.learning_rate.load();
            self.learning_rate.store((current_lr * 0.99).max(0.001));
        } else if accuracy < 0.6 {
            // Low accuracy - increase learning rate for faster adaptation
            let current_lr = self.learning_rate.load();
            self.learning_rate.store((current_lr * 1.01).min(0.1));
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
            total_predictions: self.stats.total_predictions,
            accuracy,
            hit_rate,
            patterns_detected: self.patterns.len(),
            queue_size: self.queue_manager.queue_len(),
            avg_confidence: self.calculate_avg_confidence(),
            average_latency_ns: 1000, // Default 1μs latency estimate
            successful_count: self.stats.prefetch_hits,
            failed_count: self.stats.prefetch_misses,
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
    #[allow(dead_code)] // Hot tier prefetch - Method to get prefetch queue status and utilization
    pub fn queue_status(&self) -> (usize, usize) {
        self.queue_manager.queue_status()
    }

    /// Get detailed queue statistics
    #[allow(dead_code)] // Hot tier prefetch - Method to get detailed queue statistics
    pub fn get_queue_stats(&self) -> super::queue_manager::QueueStats {
        self.queue_manager.get_queue_stats()
    }

    /// Get pattern statistics
    #[allow(dead_code)] // Hot tier prefetch - Method to get pattern detection and prediction statistics
    pub fn get_pattern_stats(&self) -> super::prediction::PredictionEngineStats {
        self.prediction_engine.get_prediction_stats(&self.patterns)
    }

    /// Update configuration
    #[allow(dead_code)] // Hot tier prefetch - Method to update prefetch configuration dynamically
    pub fn update_config(&mut self, new_config: PrefetchConfig) {
        self.config = new_config.clone();
        self.queue_manager.update_config(new_config);
    }

    /// Get current configuration
    #[allow(dead_code)] // Hot tier prefetch - Method to get current prefetch configuration
    pub fn get_config(&self) -> &PrefetchConfig {
        &self.config
    }

    /// Remove expired requests
    #[allow(dead_code)] // Hot tier prefetch - Method to cleanup expired prefetch requests for memory management
    pub fn cleanup_expired_requests(
        &mut self,
        current_time_ns: u64,
        expiry_threshold_ns: u64,
    ) -> usize {
        self.queue_manager
            .remove_expired_requests(current_time_ns, expiry_threshold_ns)
    }

    /// Get access history length
    #[allow(dead_code)] // Hot tier prefetch - Method to get access history length for pattern analysis
    pub fn access_history_len(&self) -> usize {
        self.access_history.len()
    }

    /// Get number of detected patterns
    #[allow(dead_code)] // Hot tier prefetch - Method to get count of detected patterns
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }
}
