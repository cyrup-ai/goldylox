//! Utility functions for pattern analyzer
//!
//! This module provides utility functions for pattern window management,
//! cleanup operations, and timestamp handling.

use std::sync::atomic::Ordering;
use std::time::Instant;

use super::types::AccessPatternType;
use crate::cache::traits::core::CacheKey;
use crate::cache::traits::types_and_enums::CacheOperationError;

impl<K: CacheKey> super::analyzer_core::AccessPatternAnalyzer<K> {
    /// Update pattern analysis window with new access
    pub(super) fn update_pattern_window(
        &self,
        key: &K,
        now_ns: u64,
    ) -> Result<(), CacheOperationError> {
        let key_hash = self.hash_key(key);
        let window_key = key_hash >> 20; // Group by high-order bits

        // Detect pattern type based on recent access history
        let pattern_type = self.detect_pattern_type(key_hash, now_ns, window_key);

        // Add to pattern window
        match self.pattern_window.entry(window_key) {
            dashmap::mapref::entry::Entry::Occupied(mut entry) => {
                let accesses = entry.get_mut();
                accesses.push((key_hash, pattern_type));

                // Keep window bounded
                if accesses.len() > self.config.pattern_analysis_window {
                    accesses.remove(0);
                }
            }
            dashmap::mapref::entry::Entry::Vacant(entry) => {
                entry.insert(vec![(key_hash, pattern_type)]);
            }
        }

        Ok(())
    }

    /// Detect access pattern type based on recent history
    #[inline]
    fn detect_pattern_type(
        &self,
        key_hash: u64,
        now_ns: u64,
        window_key: u64,
    ) -> AccessPatternType {
        // Check existing pattern window for this key group
        if let Some(window_entry) = self.pattern_window.get(&window_key) {
            let accesses = window_entry.value();

            if accesses.len() >= 3 {
                // Analyze last few accesses for patterns
                let recent_hashes: Vec<u64> = accesses
                    .iter()
                    .rev()
                    .take(3)
                    .map(|(hash, _)| *hash)
                    .collect();

                // Check for sequential pattern (consecutive hash values)
                if self.is_sequential_pattern(&recent_hashes, key_hash) {
                    return AccessPatternType::Sequential;
                }

                // Check for temporal pattern (regular time intervals)
                if self.is_temporal_pattern(now_ns, &accesses) {
                    return AccessPatternType::Temporal;
                }

                // Check for spatial pattern (similar hash ranges)
                if self.is_spatial_pattern(&recent_hashes, key_hash) {
                    return AccessPatternType::Spatial;
                }
            }
        }

        // Default to random if no pattern detected
        AccessPatternType::Random
    }

    /// Check if accesses show sequential pattern
    #[inline]
    fn is_sequential_pattern(&self, recent_hashes: &[u64], current_hash: u64) -> bool {
        if recent_hashes.len() < 2 {
            return false;
        }

        // Check if hashes are incrementing with small gaps
        let mut is_sequential = true;
        let mut prev_hash = recent_hashes[0];

        for &hash in &recent_hashes[1..] {
            let diff = hash.wrapping_sub(prev_hash);
            if diff > 1000 {
                // Allow small gaps for hash collisions
                is_sequential = false;
                break;
            }
            prev_hash = hash;
        }

        // Check if current hash continues the sequence
        if is_sequential {
            let diff = current_hash.wrapping_sub(prev_hash);
            diff <= 1000
        } else {
            false
        }
    }

    /// Check if accesses show temporal pattern
    #[inline]
    fn is_temporal_pattern(&self, _now_ns: u64, _accesses: &[(u64, AccessPatternType)]) -> bool {
        // Simplified temporal pattern detection
        // In a full implementation, this would analyze time intervals between accesses
        // For now, return false to avoid complexity
        false
    }

    /// Check if accesses show spatial pattern
    #[inline]
    fn is_spatial_pattern(&self, recent_hashes: &[u64], current_hash: u64) -> bool {
        if recent_hashes.is_empty() {
            return false;
        }

        // Check if hashes are in similar ranges (spatial locality)
        let hash_range = 0x10000; // 64KB hash range for spatial grouping
        let current_bucket = current_hash / hash_range;

        // Check if recent accesses are in same or adjacent buckets
        recent_hashes.iter().any(|&hash| {
            let bucket = hash / hash_range;
            bucket.abs_diff(current_bucket) <= 1
        })
    }

    /// Hash cache key for pattern analysis
    #[inline(always)]
    pub(super) fn hash_key(&self, key: &K) -> u64 {
        // Use generic CacheKey hash method
        key.cache_hash()
    }

    /// Calculate time bucket index for sliding window
    #[inline(always)]
    pub(super) fn time_bucket_index(&self, timestamp_ns: u64) -> usize {
        ((timestamp_ns / self.config.time_bucket_duration_ns)
            % self.config.time_bucket_count as u64) as usize
    }

    /// Get current timestamp in nanoseconds
    #[inline(always)]
    pub(super) fn current_timestamp_ns(&self) -> Result<u64, CacheOperationError> {
        use std::sync::OnceLock;
        static START_TIME: OnceLock<Instant> = OnceLock::new();

        let start = START_TIME.get_or_init(|| Instant::now());
        let elapsed = Instant::now().duration_since(*start);
        Ok(elapsed.as_nanos() as u64)
    }

    /// Maybe perform periodic cleanup
    pub(super) fn maybe_cleanup(&self) -> Result<(), CacheOperationError> {
        let cleanup_counter = self.cleanup_counter.fetch_add(1, Ordering::Relaxed);

        // Cleanup every N operations
        if cleanup_counter % self.config.cleanup_interval == 0 {
            self.perform_cleanup()?;
        }

        Ok(())
    }

    /// Perform cleanup of expired entries
    fn perform_cleanup(&self) -> Result<(), CacheOperationError> {
        let now_ns = self.current_timestamp_ns()?;
        let cutoff_time = now_ns.saturating_sub(self.config.cleanup_age_threshold_ns);

        // Remove entries older than threshold
        self.access_history
            .retain(|_key, record| record.last_access_ns() > cutoff_time);

        // Clean pattern window
        self.pattern_window.retain(|_window_key, accesses| {
            accesses.retain(|(timestamp, _)| *timestamp > cutoff_time);
            !accesses.is_empty()
        });

        Ok(())
    }

    /// Evict oldest entry when at capacity (LRU eviction)
    pub(super) fn evict_oldest_entry(&self) -> Result<(), CacheOperationError> {
        let mut oldest_key: Option<K> = None;
        let mut oldest_time = u64::MAX;

        // Find oldest entry (this is O(n) but only done at capacity)
        for entry in self.access_history.iter() {
            let last_access = entry.value().last_access_ns();
            if last_access < oldest_time {
                oldest_time = last_access;
                oldest_key = Some(entry.key().clone());
            }
        }

        // Remove oldest entry
        if let Some(key) = oldest_key {
            self.access_history.remove(&key);
        }

        Ok(())
    }

    /// Estimate current memory usage
    pub(super) fn estimate_memory_usage(&self) -> u64 {
        let base_size = std::mem::size_of::<Self>() as u64;
        let map_overhead = self.access_history.len() as u64 * 64; // Estimate per-entry overhead
        let record_size = self
            .access_history
            .iter()
            .map(|entry| entry.value().memory_usage() as u64)
            .sum::<u64>();

        base_size + map_overhead + record_size
    }
}
