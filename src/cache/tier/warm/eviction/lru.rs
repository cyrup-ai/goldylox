//! LRU (Least Recently Used) eviction policy implementation
//!
//! This module implements a concurrent LRU eviction policy using lock-free
//! data structures for high-performance cache management.

use std::sync::atomic::{AtomicU64, Ordering};

use crossbeam_skiplist::{SkipMap, SkipSet};

use super::types::*;
use crate::cache::tier::warm::core::WarmCacheKey;
use crate::cache::traits::CacheKey;

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
        self.select_oldest(count)
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
        // LRU doesn't need adaptation - it's a fixed policy
    }
}
