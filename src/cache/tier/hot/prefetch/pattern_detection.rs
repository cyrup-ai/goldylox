//! Pattern detection algorithms for prefetch prediction
//!
//! This module implements various algorithms to detect access patterns
//! in cache usage history for predictive prefetching.
//!
//! CANONICAL IMPLEMENTATION: This is the canonical PatternDetector implementation.
//! The analyzer module provides a compatibility wrapper for backward compatibility.

use std::collections::{HashMap, VecDeque};

use super::types::{
    AccessPattern, AccessSequence, DetectedPattern, PatternDetectionError, PrefetchConfig,
};
use crate::cache::traits::CacheKey;

/// Pattern detection engine
#[derive(Debug)]
pub struct PatternDetector {
    config: PrefetchConfig,
}

impl PatternDetector {
    /// Create new pattern detector
    pub fn new(config: PrefetchConfig) -> Self {
        Self { config }
    }

    /// Detect all types of patterns in access history
    pub fn detect_patterns<K: CacheKey>(
        &self,
        access_history: &VecDeque<AccessSequence<K>>,
    ) -> Result<Vec<DetectedPattern<K>>, PatternDetectionError> {
        if access_history.len() < 10 {
            return Ok(Vec::new()); // Not an error, just insufficient data
        }

        let mut patterns = Vec::new();

        // Detect different pattern types with error handling
        match self.detect_temporal_patterns(access_history) {
            Ok(temporal_patterns) => patterns.extend(temporal_patterns),
            Err(PatternDetectionError::InsufficientData) => {
                // Continue with other pattern types - insufficient data is expected sometimes
            }
            Err(_e) => {
                // Continue with other pattern types - temporal detection failed but others may work
            }
        }

        match self.detect_periodic_patterns(access_history) {
            Ok(periodic_patterns) => patterns.extend(periodic_patterns),
            Err(_e) => {
                // Continue with other pattern types - periodic detection failed but others may work
            }
        }

        // Continue with other pattern detection methods (no Result return needed for these)
        patterns.extend(self.detect_sequential_patterns(access_history));
        patterns.extend(self.detect_spatial_patterns(access_history));

        Ok(patterns)
    }

    /// Detect sequential access patterns
    fn detect_sequential_patterns<K: CacheKey>(
        &self,
        access_history: &VecDeque<AccessSequence<K>>,
    ) -> Vec<DetectedPattern<K>> {
        let mut patterns = Vec::new();
        let window_size = 5;

        if access_history.len() < window_size {
            return patterns;
        }

        // Look for sequences where keys follow a predictable pattern
        for window_start in 0..access_history.len().saturating_sub(window_size) {
            let window: Vec<_> = access_history
                .range(window_start..window_start + window_size)
                .map(|acc| acc.key.clone())
                .collect();

            // Check if this forms a sequential pattern
            if self.is_sequential_pattern(&window) {
                let pattern: DetectedPattern<K> = DetectedPattern {
                    pattern_type: AccessPattern::Sequential,
                    sequence: window,
                    confidence: 0.8,
                    frequency: 1,
                    last_seen: access_history[window_start + window_size - 1].timestamp,
                };

                patterns.push(pattern);
            }
        }

        patterns
    }

    /// Detect temporal access patterns (time-based repetition)
    fn detect_temporal_patterns<K: CacheKey>(
        &self,
        access_history: &VecDeque<AccessSequence<K>>,
    ) -> Result<Vec<DetectedPattern<K>>, PatternDetectionError> {
        let mut patterns = Vec::new();

        // Early validation
        if access_history.is_empty() {
            return Err(PatternDetectionError::EmptyAccessHistory);
        }

        // Group accesses by key and analyze timing intervals
        let mut key_accesses: HashMap<K, Vec<u64>> = HashMap::new();

        for access in access_history {
            key_accesses
                .entry(access.key.clone())
                .or_default()
                .push(access.timestamp);
        }

        for (key, timestamps) in key_accesses {
            if timestamps.len() >= 3 {
                // Calculate intervals between accesses
                let intervals: Vec<u64> = timestamps.windows(2).map(|w| w[1] - w[0]).collect();

                // Check for regular intervals (periodic access)
                if self.has_regular_intervals(&intervals) {
                    let last_seen = timestamps
                        .last()
                        .copied()
                        .ok_or(PatternDetectionError::TemporalDataCorrupted)?;

                    let pattern: DetectedPattern<K> = DetectedPattern {
                        pattern_type: AccessPattern::Temporal,
                        sequence: vec![key.clone()],
                        confidence: 0.7,
                        frequency: timestamps.len() as u32,
                        last_seen,
                    };

                    patterns.push(pattern);
                }
            }
        }

        Ok(patterns)
    }

    /// Detect spatial access patterns (related keys accessed together)
    fn detect_spatial_patterns<K: CacheKey>(
        &self,
        access_history: &VecDeque<AccessSequence<K>>,
    ) -> Vec<DetectedPattern<K>> {
        let mut patterns = Vec::new();
        let time_window = 1_000_000_000; // 1 second in nanoseconds

        for (i, access) in access_history.iter().enumerate() {
            let mut related_keys = vec![access.key.clone()];

            // Find other accesses within time window
            for (j, other_access) in access_history.iter().enumerate() {
                if i != j
                    && other_access.timestamp.abs_diff(access.timestamp) <= time_window
                    && !related_keys.contains(&other_access.key)
                {
                    related_keys.push(other_access.key.clone());
                }
            }

            // If we found multiple related keys, create a spatial pattern
            if related_keys.len() >= 3 {
                let pattern: DetectedPattern<K> = DetectedPattern {
                    pattern_type: AccessPattern::Spatial,
                    sequence: related_keys,
                    confidence: 0.6,
                    frequency: 1,
                    last_seen: access.timestamp,
                };

                patterns.push(pattern);
            }
        }

        patterns
    }

    /// Detect periodic access patterns
    fn detect_periodic_patterns<K: CacheKey>(
        &self,
        access_history: &VecDeque<AccessSequence<K>>,
    ) -> Result<Vec<DetectedPattern<K>>, PatternDetectionError> {
        let mut patterns = Vec::new();

        // Validate access history before processing
        if access_history.is_empty() {
            return Err(PatternDetectionError::EmptyAccessHistory);
        }

        // Use Fast Fourier Transform-like analysis for period detection
        // Simplified version: look for repeating sequences
        let min_period = 3;
        let max_period = 20;

        for period in min_period..=max_period {
            if access_history.len() < period * 3 {
                continue;
            }

            let mut period_matches = 0;
            let sequence_len = access_history.len();

            // Check if access pattern repeats every 'period' accesses
            for start in 0..period {
                let mut matches = 0;
                let mut total_checks = 0;

                for offset in (start..sequence_len).step_by(period) {
                    if offset + period < sequence_len {
                        let key1 = &access_history[offset].key;
                        let key2 = &access_history[offset + period].key;

                        total_checks += 1;
                        if key1 == key2 {
                            matches += 1;
                        }
                    }
                }

                if total_checks > 0 && matches as f64 / total_checks as f64 > 0.7 {
                    period_matches += 1;
                }
            }

            // If enough matches found, create periodic pattern
            if period_matches >= period / 2 {
                let pattern_sequence: Vec<_> = access_history
                    .iter()
                    .skip(sequence_len.saturating_sub(period))
                    .map(|acc| acc.key.clone())
                    .collect();

                let last_access = access_history
                    .back()
                    .ok_or(PatternDetectionError::EmptyAccessHistory)?;

                let pattern: DetectedPattern<K> = DetectedPattern {
                    pattern_type: AccessPattern::Periodic,
                    sequence: pattern_sequence,
                    confidence: 0.75,
                    frequency: (sequence_len / period) as u32,
                    last_seen: last_access.timestamp,
                };

                patterns.push(pattern);
            }
        }

        Ok(patterns)
    }

    /// Check if sequence represents a sequential pattern
    fn is_sequential_pattern<K: CacheKey>(&self, sequence: &[K]) -> bool {
        if sequence.len() < 3 {
            return false;
        }

        // Use statistical analysis approach from analyzer::PatternDetector
        // Build hash sequence for analysis
        let hashes: Vec<u64> = sequence.iter().map(|k| k.cache_hash()).collect();

        // Look for monotonically increasing or decreasing patterns
        let mut increasing_count = 0;
        let mut decreasing_count = 0;
        let mut stride_consistent = true;
        let mut last_stride = None;

        for window in hashes.windows(2) {
            let hash1 = window[0];
            let hash2 = window[1];

            if hash2 > hash1 {
                increasing_count += 1;

                // Check for consistent stride (common in sequential access)
                let stride = hash2 - hash1;
                if let Some(last) = last_stride {
                    // Allow 20% variance in stride
                    if stride.abs_diff(last) > last / 5 {
                        stride_consistent = false;
                    }
                }
                last_stride = Some(stride);
            } else if hash1 > hash2 {
                decreasing_count += 1;

                // Check for consistent stride in reverse
                let stride = hash1 - hash2;
                if let Some(last) = last_stride
                    && stride.abs_diff(last) > last / 5
                {
                    stride_consistent = false;
                }
                last_stride = Some(stride);
            }
        }

        let total_windows = sequence.len() - 1;
        let sequential_ratio = increasing_count.max(decreasing_count) as f64 / total_windows as f64;

        // Consider it sequential if:
        // 1. >70% of accesses are in monotonic order, OR
        // 2. >60% are monotonic AND stride is consistent
        sequential_ratio > 0.7 || (sequential_ratio > 0.6 && stride_consistent)
    }

    /// Check if intervals show regular timing pattern
    fn has_regular_intervals(&self, intervals: &[u64]) -> bool {
        if intervals.len() < 2 {
            return false;
        }

        let mean_interval = intervals.iter().sum::<u64>() / intervals.len() as u64;
        let variance = intervals
            .iter()
            .map(|&interval| {
                let diff = interval as i64 - mean_interval as i64;
                diff.saturating_mul(diff) as u64
            })
            .fold(0u64, |acc, x| acc.saturating_add(x))
            .saturating_div(intervals.len().max(1) as u64);

        let std_dev = (variance as f64).sqrt();
        let coefficient_of_variation = std_dev / mean_interval as f64;

        // Regular if coefficient of variation is low
        coefficient_of_variation < 0.3
    }

    /// Check if two pattern sequences are similar
    pub fn patterns_similar<K: CacheKey>(&self, seq1: &[K], seq2: &[K]) -> bool {
        if seq1.len() != seq2.len() {
            return false;
        }

        let matches = seq1
            .iter()
            .zip(seq2.iter())
            .filter(|(k1, k2)| k1 == k2)
            .count();

        matches as f64 / seq1.len() as f64 > 0.8
    }

    /// Clean up old patterns based on age
    pub fn cleanup_old_patterns<K: CacheKey>(&self, patterns: &mut Vec<DetectedPattern<K>>) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let threshold = self.config.pattern_detection_window.as_nanos() as u64;
        patterns.retain(|pattern| now - pattern.last_seen < threshold);
    }
}
