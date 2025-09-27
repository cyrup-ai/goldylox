//! Management operations for SimdHotTier
//!
//! This module contains statistics collection, maintenance operations,
//! and configuration access for the SIMD hot tier cache.

#![allow(dead_code)] // Hot tier SIMD - Complete management operations with statistics collection, maintenance, and configuration access

use super::core::SimdHotTier;
use crate::cache::config::types::HotTierConfig;
use crate::cache::tier::hot::memory_pool::MemoryPool;
use crate::cache::tier::hot::synchronization::WriteGuard;
use crate::cache::traits::{CacheKey, CacheValue};
use crate::cache::types::statistics::tier_stats::TierStatistics;

impl<K: CacheKey + Default, V: CacheValue> SimdHotTier<K, V> {
    /// Get cache statistics
    pub fn stats(&self) -> TierStatistics {
        self.stats.snapshot()
    }

    /// Get detailed memory pool statistics
    pub fn memory_stats(&self) -> crate::cache::tier::hot::memory_pool::MemoryPoolStats {
        self.memory_pool.stats()
    }

    /// Get eviction statistics
    pub fn eviction_stats(&self) -> crate::cache::tier::hot::eviction::EvictionStats {
        self.eviction_engine.get_stats()
    }

    /// Get prefetch statistics
    pub fn prefetch_stats(&self) -> crate::cache::tier::hot::prefetch::PrefetchStats {
        self.prefetch_predictor.get_stats()
    }

    /// Process pending prefetch requests
    pub fn process_prefetch_requests(&mut self) -> usize {
        let mut processed = 0;

        while let Some(request) = self.prefetch_predictor.get_next_prefetch() {
            // Hardware prefetch hint
            crate::cache::tier::hot::prefetch::hardware::HardwarePrefetcher::prefetch_for_access(
                &request.key,
                &self.memory_pool.entries,
            );
            processed += 1;

            // Limit processing to avoid blocking
            if processed >= 10 {
                break;
            }
        }

        processed
    }

    /// Compact memory pool to reduce fragmentation
    pub fn compact(&mut self) -> usize {
        if let Some(_guard) = WriteGuard::new(&self.coordination) {
            self.memory_pool.compact()
        } else {
            0
        }
    }

    /// Check if cache should be optimized
    pub fn should_optimize(&self) -> bool {
        let stats = self.stats();
        let memory_stats = self.memory_stats();

        stats.hit_rate < crate::cache::tier::hot::types::thresholds::MIN_HIT_RATE
            || memory_stats.fragmentation_ratio > 0.3
            || stats.avg_access_time_ns
                > crate::cache::tier::hot::types::thresholds::MAX_ACCESS_LATENCY.as_nanos() as u64
    }

    /// Get configuration
    #[allow(dead_code)] // Hot tier SIMD - Configuration accessor for SIMD hot tier settings
    pub fn config(&self) -> &HotTierConfig {
        &self.config
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        if let Some(_guard) = WriteGuard::new(&self.coordination) {
            for i in 0..self.memory_pool.capacity() {
                self.memory_pool.clear_slot(i);
            }
            self.lru_tracker.reset();
            self.stats.reset();
            self.prefetch_predictor.clear();
        }
    }

    /// Access memory pool for advanced operations
    pub fn memory_pool(&self) -> &MemoryPool<K, V> {
        &self.memory_pool
    }

    /// Access memory pool mutably for advanced operations
    pub fn memory_pool_mut(&mut self) -> &mut MemoryPool<K, V> {
        &mut self.memory_pool
    }
}
