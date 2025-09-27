//! Prediction logic for prefetch system
//!
//! This module handles generating prefetch predictions based on detected
//! patterns and current access context.

#![allow(dead_code)] // Hot tier prefetch - Complete prediction engine library for pattern-based prefetch generation

use super::types::{
    AccessPattern, DetectedPattern, PredictionConfidence, PrefetchConfig, PrefetchRequest,
};
use crate::cache::traits::CacheKey;

/// Prediction engine for generating prefetch requests
#[derive(Debug)]
pub struct PredictionEngine {
    config: PrefetchConfig,
}

impl PredictionEngine {
    /// Create new prediction engine
    pub fn new(config: PrefetchConfig) -> Self {
        Self { config }
    }

    /// Generate prefetch predictions based on current access and patterns
    pub fn generate_predictions<K: CacheKey>(
        &self,
        current_key: &K,
        timestamp_ns: u64,
        patterns: &[DetectedPattern<K>],
    ) -> Vec<PrefetchRequest<K>> {
        let mut predictions = Vec::new();

        // Look for patterns that match current access
        for pattern in patterns {
            if let Some(prediction) = self.predict_from_pattern(pattern, current_key, timestamp_ns)
            {
                predictions.push(prediction);
            }
        }

        predictions
    }

    /// Predict next access from a detected pattern
    fn predict_from_pattern<K: CacheKey>(
        &self,
        pattern: &DetectedPattern<K>,
        current_key: &K,
        timestamp_ns: u64,
    ) -> Option<PrefetchRequest<K>> {
        // Find current key position in pattern
        let position = pattern.sequence.iter().position(|key| key == current_key)?;

        // Predict next key in sequence
        if position + 1 < pattern.sequence.len() {
            let next_key = pattern.sequence[position + 1].clone();

            let confidence = self.calculate_confidence(pattern);

            // Only predict if confidence is above threshold
            if pattern.confidence >= self.config.min_confidence_threshold {
                Some(PrefetchRequest::<K> {
                    key: next_key,
                    confidence,
                    predicted_access_time: timestamp_ns + self.calculate_prediction_delay(pattern),
                    pattern_type: pattern.pattern_type,
                    priority: self.calculate_priority(pattern, confidence),
                    timestamp_ns,
                    access_pattern: Some(pattern.pattern_type),
                    estimated_size: None, // Unknown at prediction time
                })
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Calculate prediction confidence based on pattern
    fn calculate_confidence<K: crate::cache::traits::CacheKey>(
        &self,
        pattern: &DetectedPattern<K>,
    ) -> PredictionConfidence {
        match pattern.confidence {
            c if c > 0.9 => PredictionConfidence::VeryHigh,
            c if c > 0.75 => PredictionConfidence::High,
            c if c > 0.6 => PredictionConfidence::Medium,
            _ => PredictionConfidence::Low,
        }
    }

    /// Calculate predicted access delay based on pattern type
    fn calculate_prediction_delay<K: crate::cache::traits::CacheKey>(
        &self,
        pattern: &DetectedPattern<K>,
    ) -> u64 {
        match pattern.pattern_type {
            AccessPattern::Sequential => 500_000,   // 0.5ms
            AccessPattern::Temporal => 1_000_000,   // 1ms
            AccessPattern::Spatial => 100_000,      // 0.1ms
            AccessPattern::Periodic => 2_000_000,   // 2ms
            AccessPattern::Contextual => 1_500_000, // 1.5ms
            AccessPattern::Random => 5_000_000,     // 5ms
        }
    }

    /// Calculate prefetch priority
    fn calculate_priority<K: crate::cache::traits::CacheKey>(
        &self,
        pattern: &DetectedPattern<K>,
        confidence: PredictionConfidence,
    ) -> u8 {
        let base_priority = match confidence {
            PredictionConfidence::VeryHigh => 90,
            PredictionConfidence::High => 70,
            PredictionConfidence::Medium => 50,
            PredictionConfidence::Low => 30,
        };

        let frequency_bonus = (pattern.frequency.min(10) * 2) as u8;
        let pattern_bonus = match pattern.pattern_type {
            AccessPattern::Sequential => 10,
            AccessPattern::Temporal => 8,
            AccessPattern::Periodic => 6,
            AccessPattern::Spatial => 4,
            AccessPattern::Contextual => 3,
            AccessPattern::Random => 0,
        };

        base_priority + frequency_bonus + pattern_bonus
    }

    /// Check if key should be prefetched based on patterns
    #[allow(dead_code)] // Hot tier prefetch - Pattern-based prefetch decision engine for intelligent caching
    pub fn should_prefetch<K: CacheKey>(&self, key: &K, patterns: &[DetectedPattern<K>]) -> bool {
        patterns.iter().any(|pattern| {
            pattern.sequence.contains(key)
                && pattern.confidence >= self.config.min_confidence_threshold
        })
    }

    /// Predict multiple steps ahead in a pattern
    #[allow(dead_code)] // Hot tier prefetch - Multi-step sequence prediction for advanced prefetching strategies
    pub fn predict_sequence<K: CacheKey>(
        &self,
        pattern: &DetectedPattern<K>,
        current_key: &K,
        steps: usize,
        timestamp_ns: u64,
    ) -> Vec<PrefetchRequest<K>> {
        let mut predictions = Vec::new();

        if let Some(position) = pattern.sequence.iter().position(|key| key == current_key) {
            let max_steps = (pattern.sequence.len() - position - 1).min(steps);

            for step in 1..=max_steps {
                if position + step < pattern.sequence.len() {
                    let next_key = pattern.sequence[position + step].clone();
                    let confidence = self.calculate_confidence(pattern);

                    // Reduce confidence for further predictions
                    let adjusted_confidence = match step {
                        1 => confidence,
                        2 => match confidence {
                            PredictionConfidence::VeryHigh => PredictionConfidence::High,
                            PredictionConfidence::High => PredictionConfidence::Medium,
                            PredictionConfidence::Medium => PredictionConfidence::Low,
                            PredictionConfidence::Low => continue,
                        },
                        _ => PredictionConfidence::Low,
                    };

                    let delay = self.calculate_prediction_delay(pattern) * step as u64;

                    predictions.push(PrefetchRequest::<K> {
                        key: next_key,
                        confidence: adjusted_confidence,
                        predicted_access_time: timestamp_ns + delay,
                        pattern_type: pattern.pattern_type,
                        priority: self.calculate_priority(pattern, adjusted_confidence)
                            - (step as u8 * 10),
                        timestamp_ns: timestamp_ns + delay,
                        access_pattern: Some(pattern.pattern_type),
                        estimated_size: None, // Unknown at prediction time
                    });
                }
            }
        }

        predictions
    }

    /// Calculate prediction accuracy for a pattern
    #[allow(dead_code)] // Hot tier prefetch - Pattern accuracy calculation for prediction engine optimization
    pub fn calculate_pattern_accuracy<K: CacheKey>(
        &self,
        pattern: &DetectedPattern<K>,
        recent_accesses: &[K],
    ) -> f64 {
        if pattern.sequence.is_empty() || recent_accesses.is_empty() {
            return 0.0;
        }

        let mut correct_predictions = 0;
        let mut total_predictions = 0;

        // Check how well the pattern predicts recent accesses
        for window in recent_accesses.windows(pattern.sequence.len()) {
            total_predictions += 1;

            let matches = window
                .iter()
                .zip(pattern.sequence.iter())
                .filter(|(actual, predicted)| actual == predicted)
                .count();

            if matches as f64 / pattern.sequence.len() as f64 > 0.8 {
                correct_predictions += 1;
            }
        }

        if total_predictions > 0 {
            correct_predictions as f64 / total_predictions as f64
        } else {
            0.0
        }
    }

    /// Update pattern confidence based on prediction results
    pub fn update_pattern_confidence<K: CacheKey>(
        &self,
        pattern: &mut DetectedPattern<K>,
        hit: bool,
    ) {
        let adjustment = if hit { 0.05 } else { -0.1 };
        pattern.confidence = (pattern.confidence + adjustment).clamp(0.0, 1.0);

        if hit {
            pattern.frequency += 1;
        }
    }

    /// Get prediction statistics for a set of patterns
    #[allow(dead_code)] // Hot tier prefetch - Statistics collection for prediction engine performance analysis
    pub fn get_prediction_stats<K: CacheKey>(
        &self,
        patterns: &[DetectedPattern<K>],
    ) -> PredictionEngineStats {
        let total_patterns = patterns.len();
        let high_confidence_patterns = patterns.iter().filter(|p| p.confidence > 0.8).count();

        let avg_confidence = if !patterns.is_empty() {
            patterns.iter().map(|p| p.confidence).sum::<f64>() / patterns.len() as f64
        } else {
            0.0
        };

        let pattern_distribution = self.calculate_pattern_distribution(patterns);

        PredictionEngineStats {
            total_patterns,
            high_confidence_patterns,
            avg_confidence,
            pattern_distribution,
        }
    }

    /// Calculate distribution of pattern types
    #[allow(dead_code)] // Hot tier prefetch - Pattern distribution analysis for prediction strategy optimization
    fn calculate_pattern_distribution<K: CacheKey>(
        &self,
        patterns: &[DetectedPattern<K>],
    ) -> PatternDistribution {
        let mut distribution = PatternDistribution::default();

        for pattern in patterns {
            match pattern.pattern_type {
                AccessPattern::Sequential => distribution.sequential += 1,
                AccessPattern::Temporal => distribution.temporal += 1,
                AccessPattern::Spatial => distribution.spatial += 1,
                AccessPattern::Periodic => distribution.periodic += 1,
                AccessPattern::Contextual => distribution.contextual += 1,
                AccessPattern::Random => distribution.random += 1,
            }
        }

        distribution
    }
}

/// Statistics for prediction engine
#[derive(Debug, Clone)]
#[allow(dead_code)] // Hot tier prefetch - Prediction engine statistics structure for performance monitoring
pub struct PredictionEngineStats {
    pub total_patterns: usize,
    pub high_confidence_patterns: usize,
    pub avg_confidence: f64,
    pub pattern_distribution: PatternDistribution,
}

/// Distribution of pattern types
#[derive(Debug, Clone, Default)]
#[allow(dead_code)] // Hot tier prefetch - Pattern distribution metrics for prediction engine analysis
pub struct PatternDistribution {
    pub sequential: usize,
    pub temporal: usize,
    pub spatial: usize,
    pub periodic: usize,
    pub contextual: usize,
    pub random: usize,
}
