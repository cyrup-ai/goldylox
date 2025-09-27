//! ARC (Adaptive Replacement Cache) eviction policy implementation
//!
//! This module implements the ARC algorithm which adaptively balances between
//! recency and frequency for optimal cache performance.

use std::sync::atomic::{AtomicU64, Ordering};

use crossbeam_skiplist::{SkipMap, SkipSet};
use crossbeam_utils::atomic::AtomicCell;

use super::types::*;
use crate::cache::tier::warm::core::WarmCacheKey;
use crate::cache::traits::core::CacheKey;
use crate::telemetry::cache::types::timestamp_nanos;

/// ARC eviction state for adaptive replacement
#[derive(Debug)]
pub struct ArcEvictionState<K: CacheKey> {
    /// T1 cache (recent items)
    t1_cache: SkipSet<WarmCacheKey<K>>,
    /// T2 cache (frequent items)
    t2_cache: SkipSet<WarmCacheKey<K>>,
    /// B1 ghost cache (recent evicted)
    b1_ghost: SkipSet<WarmCacheKey<K>>,
    /// B2 ghost cache (frequent evicted)
    b2_ghost: SkipSet<WarmCacheKey<K>>,
    /// Access time tracking for performance metrics
    access_times: SkipMap<WarmCacheKey<K>, AtomicU64>,
    /// Adaptation parameter
    adaptation_param: AtomicCell<f64>,
    /// Cache size target for T1
    t1_target_size: AtomicU64,
    /// Maximum cache size
    max_size: u64,
    /// ARC statistics
    stats: ArcStats,
}

impl<K: CacheKey> ArcEvictionState<K> {
    /// Create new ARC eviction state
    pub fn new(max_size: u64) -> Self {
        Self {
            t1_cache: SkipSet::new(),
            t2_cache: SkipSet::new(),
            b1_ghost: SkipSet::new(),
            b2_ghost: SkipSet::new(),
            access_times: SkipMap::new(),
            adaptation_param: AtomicCell::new(0.5),
            t1_target_size: AtomicU64::new(max_size / 2),
            max_size,
            stats: ArcStats::default(),
        }
    }

    /// Handle cache hit
    pub fn on_hit(&self, key: &WarmCacheKey<K>) {
        let now = timestamp_nanos();

        // Update access time for performance tracking
        if let Some(access_time) = self.access_times.get(key) {
            access_time.value().store(now, Ordering::Relaxed);
        } else {
            self.access_times.insert(key.clone(), AtomicU64::new(now));
        }

        if self.t1_cache.contains(key) {
            // Move from T1 to T2
            self.t1_cache.remove(key);
            self.t2_cache.insert(key.clone());
            self.stats.t1_hits.fetch_add(1, Ordering::Relaxed);
        } else if self.t2_cache.contains(key) {
            // Already in T2, just update position (T2 is already frequent)
            // No action needed for SkipSet
            self.stats.t2_hits.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Handle cache miss
    pub fn on_miss(&self, key: &WarmCacheKey<K>) {
        if self.b1_ghost.contains(key) {
            // Hit in B1 ghost - increase T1 target size
            self.adapt_for_b1_hit();
            self.b1_ghost.remove(key);
            self.t2_cache.insert(key.clone());
            self.stats.b1_ghost_hits.fetch_add(1, Ordering::Relaxed);
        } else if self.b2_ghost.contains(key) {
            // Hit in B2 ghost - decrease T1 target size
            self.adapt_for_b2_hit();
            self.b2_ghost.remove(key);
            self.t2_cache.insert(key.clone());
            self.stats.b2_ghost_hits.fetch_add(1, Ordering::Relaxed);
        } else {
            // Cold miss - add to T1
            self.t1_cache.insert(key.clone());
        }
    }

    /// Handle eviction
    pub fn on_eviction(&self, key: &WarmCacheKey<K>) {
        if self.t1_cache.remove(key).is_some() {
            self.b1_ghost.insert(key.clone());
        } else if self.t2_cache.remove(key).is_some() {
            self.b2_ghost.insert(key.clone());
        }

        // Maintain ghost cache sizes
        self.maintain_ghost_sizes();
    }

    /// Adapt T1 target size for B1 hit
    fn adapt_for_b1_hit(&self) {
        let b1_size = self.b1_ghost.len() as u64;
        let b2_size = self.b2_ghost.len() as u64;

        let delta = if b1_size >= b2_size {
            1
        } else {
            b2_size / b1_size
        };
        let current_target = self.t1_target_size.load(Ordering::Relaxed);
        let new_target = (current_target + delta).min(self.max_size);

        self.t1_target_size.store(new_target, Ordering::Relaxed);
        self.stats.adaptations.fetch_add(1, Ordering::Relaxed);
    }

    /// Adapt T1 target size for B2 hit
    fn adapt_for_b2_hit(&self) {
        let b1_size = self.b1_ghost.len() as u64;
        let b2_size = self.b2_ghost.len() as u64;

        let delta = if b2_size >= b1_size {
            1
        } else {
            b1_size / b2_size
        };
        let current_target = self.t1_target_size.load(Ordering::Relaxed);
        let new_target = current_target.saturating_sub(delta);

        self.t1_target_size.store(new_target, Ordering::Relaxed);
        self.stats.adaptations.fetch_add(1, Ordering::Relaxed);
    }

    /// Maintain ghost cache sizes
    fn maintain_ghost_sizes(&self) {
        let max_ghost_size = self.max_size;

        // Trim B1 ghost if too large
        while self.b1_ghost.len() as u64 > max_ghost_size {
            if let Some(first) = self.b1_ghost.iter().next() {
                let key = first.value().clone();
                self.b1_ghost.remove(&key);
            } else {
                break;
            }
        }

        // Trim B2 ghost if too large
        while self.b2_ghost.len() as u64 > max_ghost_size {
            if let Some(first) = self.b2_ghost.iter().next() {
                let key = first.value().clone();
                self.b2_ghost.remove(&key);
            } else {
                break;
            }
        }
    }

    /// Select eviction candidates from ARC state
    pub fn select_candidates(&self, count: usize) -> Vec<WarmCacheKey<K>> {
        let mut candidates = Vec::new();
        let t1_target = self.t1_target_size.load(Ordering::Relaxed);
        let t1_size = self.t1_cache.len() as u64;

        // Select from T1 if over target size
        if t1_size > t1_target {
            let t1_evict_count = ((t1_size - t1_target) as usize).min(count);
            for entry in self.t1_cache.iter().take(t1_evict_count) {
                candidates.push(entry.value().clone());
            }
        }

        // Fill remaining from T2 if needed
        let remaining = count.saturating_sub(candidates.len());
        if remaining > 0 {
            for entry in self.t2_cache.iter().take(remaining) {
                candidates.push(entry.value().clone());
            }
        }

        candidates
    }

    /// Get current T1 size
    pub fn t1_size(&self) -> usize {
        self.t1_cache.len()
    }

    /// Get current T2 size
    pub fn t2_size(&self) -> usize {
        self.t2_cache.len()
    }

    /// Get current B1 ghost size
    pub fn b1_ghost_size(&self) -> usize {
        self.b1_ghost.len()
    }

    /// Get current B2 ghost size
    pub fn b2_ghost_size(&self) -> usize {
        self.b2_ghost.len()
    }

    /// Get T1 target size
    pub fn t1_target_size(&self) -> u64 {
        self.t1_target_size.load(Ordering::Relaxed)
    }

    /// Get adaptation parameter
    pub fn adaptation_param(&self) -> f64 {
        self.adaptation_param.load()
    }

    /// Get ARC statistics
    pub fn stats(&self) -> &ArcStats {
        &self.stats
    }

    /// Check if key is in T1
    pub fn is_in_t1(&self, key: &WarmCacheKey<K>) -> bool {
        self.t1_cache.contains(key)
    }

    /// Check if key is in T2
    pub fn is_in_t2(&self, key: &WarmCacheKey<K>) -> bool {
        self.t2_cache.contains(key)
    }

    /// Check if key is in B1 ghost
    pub fn is_in_b1_ghost(&self, key: &WarmCacheKey<K>) -> bool {
        self.b1_ghost.contains(key)
    }

    /// Check if key is in B2 ghost
    pub fn is_in_b2_ghost(&self, key: &WarmCacheKey<K>) -> bool {
        self.b2_ghost.contains(key)
    }

    /// Get total cache size (T1 + T2)
    pub fn total_cache_size(&self) -> usize {
        self.t1_cache.len() + self.t2_cache.len()
    }

    /// Get total ghost size (B1 + B2)
    pub fn total_ghost_size(&self) -> usize {
        self.b1_ghost.len() + self.b2_ghost.len()
    }

    /// Clear all caches
    pub fn clear(&self) {
        self.t1_cache.clear();
        self.t2_cache.clear();
        self.b1_ghost.clear();
        self.b2_ghost.clear();
        self.t1_target_size
            .store(self.max_size / 2, Ordering::Relaxed);
        self.adaptation_param.store(0.5);
    }

    /// Get hit rate
    pub fn hit_rate(&self) -> f64 {
        let t1_hits = self.stats.t1_hits.load(Ordering::Relaxed);
        let t2_hits = self.stats.t2_hits.load(Ordering::Relaxed);
        let b1_ghost_hits = self.stats.b1_ghost_hits.load(Ordering::Relaxed);
        let b2_ghost_hits = self.stats.b2_ghost_hits.load(Ordering::Relaxed);

        let total_hits = t1_hits + t2_hits;
        let total_accesses = total_hits + b1_ghost_hits + b2_ghost_hits;

        if total_accesses == 0 {
            return 1.0;
        }

        total_hits as f64 / total_accesses as f64
    }

    /// Get adaptation efficiency
    pub fn adaptation_efficiency(&self) -> f64 {
        let adaptations = self.stats.adaptations.load(Ordering::Relaxed);
        let ghost_hits = self.stats.b1_ghost_hits.load(Ordering::Relaxed)
            + self.stats.b2_ghost_hits.load(Ordering::Relaxed);

        if adaptations == 0 {
            return 1.0;
        }

        ghost_hits as f64 / adaptations as f64
    }
}

impl<K: CacheKey> EvictionPolicy<WarmCacheKey<K>> for ArcEvictionState<K> {
    fn on_access(&self, key: &WarmCacheKey<K>, hit: bool) {
        if hit {
            self.on_hit(key);
        } else {
            self.on_miss(key);
        }
    }

    fn on_eviction(&self, key: &WarmCacheKey<K>) {
        self.on_eviction(key);
    }

    fn select_candidates(&self, count: usize) -> Vec<WarmCacheKey<K>> {
        self.select_candidates(count)
    }

    fn performance_metrics(&self) -> PolicyPerformanceMetrics {
        PolicyPerformanceMetrics {
            hit_rate: self.hit_rate(),
            avg_access_time_ns: self.calculate_average_access_time_ns(),
            eviction_efficiency: self.adaptation_efficiency(),
        }
    }

    /// Calculate average access time based on recent access patterns
    fn calculate_average_access_time_ns(&self) -> u64 {
        let current_time = timestamp_nanos();
        let mut total_time_diff = 0u64;
        let mut count = 0usize;

        // Sample recent entries to calculate average access time
        for entry in self.access_times.iter().rev().take(100) {
            let last_access = entry.value().load(Ordering::Relaxed);
            if last_access > 0 {
                let time_diff = current_time.saturating_sub(last_access);
                total_time_diff += time_diff;
                count += 1;
            }
        }

        if count > 0 {
            total_time_diff / count as u64
        } else {
            1000 // Default 1 microsecond if no data available
        }
    }

    fn adapt(&self) {
        // ARC adapts automatically based on ghost cache hits
        // No explicit adaptation needed
    }
}
