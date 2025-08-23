//! Pattern detection algorithms for access pattern analysis
//!
//! This module provides statistical analysis algorithms for detecting
//! sequential, temporal, and spatial access patterns.

use dashmap::DashMap;

use super::types::AccessPatternType;
use crate::cache::config::types::AnalyzerConfig;
use crate::cache::traits::core::CacheKey;

/// Pattern detection engine for statistical analysis
#[derive(Debug)]
pub struct PatternDetector {
    config: AnalyzerConfig,
}

impl PatternDetector {
    /// Create new pattern detector
    pub fn new(config: &AnalyzerConfig) -> Self {
        Self {
            config: config.clone(),
        }
    }

    /// Detect access pattern type using statistical analysis
    pub fn detect_pattern_type<K: CacheKey>(
        &self,
        key: &K,
        now_ns: u64,
        pattern_window: &DashMap<u64, Vec<(u64, AccessPatternType)>>,
    ) -> AccessPatternType {
        let key_hash = self.hash_key(key);

        // Get nearby key access history for pattern analysis
        let nearby_accesses = self.get_nearby_key_accesses(key_hash, now_ns, pattern_window);

        if nearby_accesses.len() < 3 {
            return AccessPatternType::Random; // Not enough data
        }

        // Analyze patterns using statistical methods
        if self.is_sequential_pattern(&nearby_accesses) {
            AccessPatternType::Sequential
        } else if self.is_temporal_pattern(&nearby_accesses, now_ns) {
            AccessPatternType::Temporal
        } else if self.is_spatial_pattern(&nearby_accesses, key_hash) {
            AccessPatternType::Spatial
        } else {
            AccessPatternType::Random
        }
    }

    /// Check if accesses follow sequential pattern
    fn is_sequential_pattern(&self, accesses: &[(u64, AccessPatternType)]) -> bool {
        if accesses.len() < 3 {
            return false;
        }

        // Look for monotonically increasing hash values
        let mut increasing_count = 0;
        for window in accesses.windows(2) {
            if window[1].0 > window[0].0 {
                increasing_count += 1;
            }
        }

        // Consider sequential if >70% of accesses are in increasing order
        let sequential_ratio = increasing_count as f64 / (accesses.len() - 1) as f64;
        sequential_ratio > 0.7
    }

    /// Check if accesses follow temporal pattern (regular intervals)
    fn is_temporal_pattern(&self, accesses: &[(u64, AccessPatternType)], _now_ns: u64) -> bool {
        if accesses.len() < 4 {
            return false;
        }

        // Calculate intervals between accesses
        let mut intervals = Vec::new();
        for window in accesses.windows(2) {
            let interval = window[1].0.saturating_sub(window[0].0);
            if interval > 0 {
                intervals.push(interval);
            }
        }

        if intervals.len() < 3 {
            return false;
        }

        // Calculate coefficient of variation for intervals
        let mean_interval: f64 =
            intervals.iter().map(|&x| x as f64).sum::<f64>() / intervals.len() as f64;
        if mean_interval == 0.0 {
            return false;
        }

        let variance: f64 = intervals
            .iter()
            .map(|&x| (x as f64 - mean_interval).powi(2))
            .sum::<f64>()
            / intervals.len() as f64;
        let std_dev = variance.sqrt();
        let cv = std_dev / mean_interval;

        // Regular temporal pattern has low coefficient of variation
        cv < 0.3
    }

    /// Check if accesses show spatial locality (similar hash values)
    fn is_spatial_pattern(&self, accesses: &[(u64, AccessPatternType)], base_hash: u64) -> bool {
        if accesses.len() < 3 {
            return false;
        }

        // Calculate how many accesses are within spatial locality range
        let locality_threshold = 0x1000000; // Hash distance threshold
        let nearby_count = accesses
            .iter()
            .filter(|(hash, _)| hash.abs_diff(base_hash) < locality_threshold)
            .count();

        // Consider spatial if >60% of accesses are nearby
        let spatial_ratio = nearby_count as f64 / accesses.len() as f64;
        spatial_ratio > 0.6
    }

    /// Get nearby key access patterns for analysis
    fn get_nearby_key_accesses(
        &self,
        key_hash: u64,
        now_ns: u64,
        pattern_window: &DashMap<u64, Vec<(u64, AccessPatternType)>>,
    ) -> Vec<(u64, AccessPatternType)> {
        let window_key = key_hash >> 20; // Group by high-order bits

        if let Some(entry) = pattern_window.get(&window_key) {
            // Filter recent accesses within analysis window
            let cutoff_time = now_ns.saturating_sub(self.config.time_bucket_duration_ns * 10);
            entry
                .value()
                .iter()
                .filter(|(timestamp, _)| *timestamp >= cutoff_time)
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Hash cache key for pattern analysis
    #[inline(always)]
    fn hash_key<K: CacheKey>(&self, key: &K) -> u64 {
        // Use generic CacheKey hash method
        key.cache_hash()
    }
}
