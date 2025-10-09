//! LRU (Least Recently Used) eviction policy implementation
//!
//! This module implements a concurrent LRU eviction policy using lock-free
//! data structures for high-performance cache management.

use std::sync::atomic::{AtomicU64, Ordering};

use crossbeam_skiplist::{SkipMap, SkipSet};

use super::types::*;
use crate::cache::tier::warm::core::WarmCacheKey;
use crate::cache::traits::CacheKey;
use crate::telemetry::cache::types::timestamp_nanos;

/// Concurrent LRU tracker using lock-free structures
#[derive(Debug)]
pub struct ConcurrentLruTracker<K: CacheKey> {
    /// Access order tracking
    access_order: SkipSet<LruEntry<K>>,
    /// Current logical time
    logical_time: AtomicU64,
    /// Key to timestamp mapping
    key_timestamps: SkipMap<WarmCacheKey<K>, u64>,
    /// LRU statistics
    stats: LruStats,
    /// Adaptive age threshold in nanoseconds
    /// Entries older than this threshold become eviction candidates
    age_threshold_ns: AtomicU64,
    /// Last adaptation timestamp for controlling adaptation frequency
    last_adaptation_time: AtomicU64,
}

/// LRU entry for ordering
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LruEntry<K: CacheKey> {
    /// Access time (for ordering)
    access_time: u64,
    /// Cache key
    cache_key: WarmCacheKey<K>,
}

impl<K: CacheKey> ConcurrentLruTracker<K> {
    /// Create new concurrent LRU tracker
    pub fn new() -> Self {
        Self {
            access_order: SkipSet::new(),
            logical_time: AtomicU64::new(0),
            key_timestamps: SkipMap::new(),
            stats: LruStats::default(),
            age_threshold_ns: AtomicU64::new(60_000_000_000), // 60 seconds
            last_adaptation_time: AtomicU64::new(timestamp_nanos()),
        }
    }

    /// Record access for LRU tracking
    pub fn record_access(&self, key: &WarmCacheKey<K>) {
        let access_time = self.logical_time.fetch_add(1, Ordering::Relaxed);
        self.stats.total_accesses.fetch_add(1, Ordering::Relaxed);

        // Remove old entry if exists
        self.remove_key_entries(key);

        // Insert new entry with current time
        let entry = LruEntry {
            access_time,
            cache_key: key.clone(),
        };
        self.access_order.insert(entry);
        self.key_timestamps.insert(key.clone(), access_time);

        // Update average access age
        self.update_average_access_age();
    }

    /// Record access with explicit timestamp
    pub fn record_access_with_timestamp(&self, key: &WarmCacheKey<K>, timestamp_ns: u64) {
        self.stats.total_accesses.fetch_add(1, Ordering::Relaxed);

        // Remove old timestamp entry if exists
        if let Some(old_timestamp) = self.key_timestamps.get(key) {
            let old_ts = *old_timestamp.value();
            self.access_order.remove(&LruEntry {
                access_time: old_ts,
                cache_key: key.clone(),
            });
        }

        // Insert new timestamp
        let entry = LruEntry {
            access_time: timestamp_ns,
            cache_key: key.clone(),
        };
        self.access_order.insert(entry);
        self.key_timestamps.insert(key.clone(), timestamp_ns);

        // Update average access age
        self.update_average_access_age();
    }

    /// Handle eviction event
    pub fn on_eviction(&self, key: &WarmCacheKey<K>) {
        self.remove_key_entries(key);
        self.stats.lru_evictions.fetch_add(1, Ordering::Relaxed);
    }

    /// Remove all entries for a specific key
    fn remove_key_entries(&self, key: &WarmCacheKey<K>) {
        if let Some(timestamp_entry) = self.key_timestamps.remove(key) {
            let timestamp = *timestamp_entry.value();
            let entry = LruEntry {
                access_time: timestamp,
                cache_key: key.clone(),
            };
            self.access_order.remove(&entry);
        }
    }

    /// Select oldest entries for eviction
    pub fn select_oldest(&self, count: usize) -> Vec<WarmCacheKey<K>> {
        let mut candidates = Vec::new();

        for entry in self.access_order.iter().take(count) {
            candidates.push(entry.value().cache_key.clone());
        }

        candidates
    }

    /// Select candidates by age threshold
    /// Only returns entries older than age_threshold_ns
    pub fn select_candidates_by_age(&self, count: usize) -> Vec<WarmCacheKey<K>> {
        let threshold_ns = self.age_threshold_ns.load(Ordering::Relaxed);
        let current_time_ns = timestamp_nanos();
        let mut candidates = Vec::new();

        // Iterate through entries in LRU order (oldest first)
        for entry in self.access_order.iter() {
            if candidates.len() >= count {
                break;
            }

            let key = &entry.value().cache_key;
            if let Some(access_time_entry) = self.key_timestamps.get(key) {
                let access_time = *access_time_entry.value();
                let age_ns = current_time_ns.saturating_sub(access_time);

                // Only include if older than threshold
                if age_ns > threshold_ns {
                    candidates.push(key.clone());
                }
            }
        }

        candidates
    }

    /// Select victim using LRU policy from candidates
    pub fn select_victim(&self, candidates: &[WarmCacheKey<K>]) -> Option<WarmCacheKey<K>> {
        if candidates.is_empty() {
            return None;
        }

        let mut oldest_key: Option<WarmCacheKey<K>> = None;
        let mut oldest_timestamp = u64::MAX;

        // Find candidate with oldest timestamp
        for candidate in candidates {
            if let Some(timestamp_entry) = self.key_timestamps.get(candidate) {
                let timestamp = *timestamp_entry.value();
                if timestamp < oldest_timestamp {
                    oldest_timestamp = timestamp;
                    oldest_key = Some(candidate.clone());
                }
            }
        }

        oldest_key
    }

    /// Update average access age statistic
    fn update_average_access_age(&self) {
        let current_time = self.logical_time.load(Ordering::Relaxed);
        let mut total_age = 0u64;
        let mut count = 0usize;

        // Sample recent entries to calculate average age
        for entry in self.access_order.iter().rev().take(100) {
            let age = current_time.saturating_sub(entry.value().access_time);
            total_age += age;
            count += 1;
        }

        if count > 0 {
            let avg_age = total_age as f64 / count as f64;
            self.stats.avg_access_age_ns.store(avg_age);
        }
    }

    /// Get LRU statistics
    pub fn stats(&self) -> &LruStats {
        &self.stats
    }

    /// Get current size
    pub fn size(&self) -> usize {
        self.access_order.len()
    }

    /// Check if key exists
    pub fn contains(&self, key: &WarmCacheKey<K>) -> bool {
        self.key_timestamps.contains_key(key)
    }

    /// Get access timestamp for key
    pub fn get_access_time(&self, key: &WarmCacheKey<K>) -> Option<u64> {
        self.key_timestamps.get(key).map(|entry| *entry.value())
    }

    /// Clear all entries
    pub fn clear(&self) {
        self.access_order.clear();
        self.key_timestamps.clear();
    }

    /// Get oldest key
    pub fn oldest_key(&self) -> Option<WarmCacheKey<K>> {
        self.access_order
            .iter()
            .next()
            .map(|entry| entry.value().cache_key.clone())
    }

    /// Get newest key
    pub fn newest_key(&self) -> Option<WarmCacheKey<K>> {
        self.access_order
            .iter()
            .next_back()
            .map(|entry| entry.value().cache_key.clone())
    }

    /// Get keys in LRU order (oldest first)
    pub fn keys_lru_order(&self) -> Vec<WarmCacheKey<K>> {
        self.access_order
            .iter()
            .map(|entry| entry.value().cache_key.clone())
            .collect()
    }

    /// Get keys in MRU order (newest first)
    pub fn keys_mru_order(&self) -> Vec<WarmCacheKey<K>> {
        self.access_order
            .iter()
            .rev()
            .map(|entry| entry.value().cache_key.clone())
            .collect()
    }

    /// Get hit rate based on evictions vs accesses
    pub fn hit_rate(&self) -> f64 {
        let total_accesses = self.stats.total_accesses.load(Ordering::Relaxed);
        let evictions = self.stats.lru_evictions.load(Ordering::Relaxed);

        if total_accesses == 0 {
            return 1.0;
        }

        let hits = total_accesses.saturating_sub(evictions);
        hits as f64 / total_accesses as f64
    }

    /// Get average access age in nanoseconds
    pub fn average_access_age_ns(&self) -> f64 {
        self.stats.avg_access_age_ns.load()
    }
}

impl<K: CacheKey> Default for ConcurrentLruTracker<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: CacheKey> EvictionPolicy<WarmCacheKey<K>> for ConcurrentLruTracker<K> {
    fn on_access(&self, key: &WarmCacheKey<K>, _hit: bool) {
        self.record_access(key);
    }

    fn on_eviction(&self, key: &WarmCacheKey<K>) {
        self.on_eviction(key);
    }

    fn select_candidates(&self, count: usize) -> Vec<WarmCacheKey<K>> {
        self.select_candidates_by_age(count)
    }

    fn performance_metrics(&self) -> PolicyPerformanceMetrics {
        PolicyPerformanceMetrics {
            hit_rate: self.hit_rate(),
            avg_access_time_ns: self.average_access_age_ns() as u64,
            eviction_efficiency: self.hit_rate(), // Use hit rate as efficiency proxy
        }
    }

    fn calculate_average_access_time_ns(&self) -> u64 {
        self.average_access_age_ns() as u64
    }

    fn adapt(&self) {
        let now_ns = timestamp_nanos();
        let last_adapt = self.last_adaptation_time.load(Ordering::Relaxed);

        // Adaptation interval: 60 seconds
        const ADAPTATION_INTERVAL_NS: u64 = 60_000_000_000;
        let time_elapsed = now_ns.saturating_sub(last_adapt);

        // Check if enough time has passed
        if time_elapsed < ADAPTATION_INTERVAL_NS {
            return; // Not yet time to adapt
        }

        // Get current hit rate from existing method
        let hit_rate = self.hit_rate();

        // Target hit rate: 85%
        const TARGET_HIT_RATE: f64 = 0.85;
        const HIGH_HIT_RATE_MARGIN: f64 = 0.05; // 5% above target

        // Load current threshold
        let current_threshold = self.age_threshold_ns.load(Ordering::Relaxed);

        // Calculate new threshold based on hit rate
        let new_threshold = if hit_rate < TARGET_HIT_RATE {
            // Hit rate too low - keep entries longer
            // Increase threshold by 10%
            let increased = (current_threshold as f64 * 1.1) as u64;
            // Max: 1 hour (3600 seconds)
            increased.min(3_600_000_000_000)
        } else if hit_rate > TARGET_HIT_RATE + HIGH_HIT_RATE_MARGIN {
            // Hit rate very high - might be wasting memory
            // Decrease threshold by 10%
            let decreased = (current_threshold as f64 * 0.9) as u64;
            // Min: 1 second
            decreased.max(1_000_000_000)
        } else {
            // Hit rate in acceptable range - no change
            current_threshold
        };

        // Update threshold if changed
        if new_threshold != current_threshold {
            self.age_threshold_ns.store(new_threshold, Ordering::Relaxed);

            // Log adaptation for observability
            log::debug!(
                "LRU adapted: hit_rate={:.2}%, threshold_ns={} ({:.1}s)",
                hit_rate * 100.0,
                new_threshold,
                new_threshold as f64 / 1_000_000_000.0
            );
        }

        // Update last adaptation time
        self.last_adaptation_time.store(now_ns, Ordering::Relaxed);
    }
}
