//! Core tier operations for warm tier cache
//!
//! This module implements the main cache operations including get, put, remove,
//! and memory management with atomic coordination.

use std::sync::atomic::Ordering;
use std::time::Duration;

use crossbeam_skiplist::SkipMap;
use crossbeam_utils::CachePadded;

use crate::cache::traits::AccessType;
use super::access_tracking::{AccessContext, ConcurrentAccessTracker};
use super::core::{LockFreeWarmTier, WarmCacheEntry, WarmCacheKey};
use super::data_structures::WarmTierConfig;
use super::eviction::{ConcurrentEvictionPolicy, EvictionPolicyType};
use super::monitoring::{MemoryPressureMonitor, TierStatsSnapshot};
use crate::cache::types::statistics::atomic_stats::AtomicTierStats;
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};

impl<K: CacheKey, V: CacheValue> LockFreeWarmTier<K, V> {
    /// Create new lock-free warm tier cache
    pub fn new(config: WarmTierConfig) -> Self {
        let memory_limit = config.max_memory_bytes;
        let (maintenance_tx, maintenance_rx) = crossbeam_channel::unbounded();
        let (analysis_tx, analysis_rx) = crossbeam_channel::unbounded();

        Self {
            storage: SkipMap::new(),
            access_tracker: ConcurrentAccessTracker::new(analysis_tx),
            eviction_policy: ConcurrentEvictionPolicy::new(),
            stats: AtomicTierStats::new(),
            config,
            memory_monitor: MemoryPressureMonitor::new(memory_limit),
            maintenance_tx,
            maintenance_rx,
            generation_counter: CachePadded::new(std::sync::atomic::AtomicU64::new(1)),
        }
    }

    /// Get entry from warm tier cache
    pub fn get(&self, key: &K) -> Option<V> {
        let warm_key = WarmCacheKey::from_cache_key(key);
        let access_start = std::time::Instant::now();

        if let Some(entry) = self.storage.get(&warm_key) {
            let warm_entry = entry.value();

            // Record access for tracking and eviction policies
            warm_entry.record_access();

            // Update access tracker
            let context = AccessContext::Random;

            let _ = self
                .access_tracker
                .record_access(key, AccessType::Hit, context);
            self.eviction_policy
                .record_access(&warm_key, AccessType::Hit);

            // Update statistics
            let access_latency = access_start.elapsed().as_nanos() as u64;
            self.stats.record_hit(access_latency);

            Some(warm_entry.value().clone())
        } else {
            // Cache miss
            let context = AccessContext::Random;

            let _ = self
                .access_tracker
                .record_access(key, AccessType::Miss, context);

            // Update statistics
            let access_latency = access_start.elapsed().as_nanos() as u64;
            self.stats.record_miss(access_latency);

            None
        }
    }

    /// Put entry into warm tier cache
    pub fn put(
        &self,
        key: K,
        value: V,
    ) -> Result<(), CacheOperationError> {
        let warm_key = WarmCacheKey::from_cache_key(&key);
        let generation = self.generation_counter.fetch_add(1, Ordering::Relaxed);
        let warm_entry = WarmCacheEntry::new(value, generation);

        let entry_size = warm_entry.estimated_size() as u64;

        // Check memory pressure before insertion
        let current_usage = self.memory_monitor.get_current_usage();
        let new_usage = current_usage + entry_size;

        // Trigger eviction if memory pressure is high
        let pressure = self.memory_monitor.get_pressure_level();
        if pressure > 0.8 {
            self.trigger_eviction(0.7)?; // Target 70% usage
        }

        // Insert entry
        let old_entry = self.storage.insert(warm_key.clone(), warm_entry);

        // Update memory usage
        if let Some(old) = old_entry {
            let old_size = old.estimated_size() as i64;
            self.memory_monitor
                .update_memory_usage(new_usage - old_size as u64)?;
            self.stats.update_memory_usage(entry_size as i64 - old_size);
        } else {
            self.memory_monitor.update_memory_usage(new_usage)?;
            self.stats.update_memory_usage(entry_size as i64);
            self.stats.update_entry_count(1);
        }

        // Record access for tracking
        let context = AccessContext::Sequential;

        let _ = self
            .access_tracker
            .record_access(&key, AccessType::Hit, context);
        self.eviction_policy
            .record_access(&warm_key, AccessType::Hit);

        Ok(())
    }

    /// Remove entry from warm tier cache
    pub fn remove(&self, key: &K) -> Option<V> {
        let warm_key = WarmCacheKey::from_cache_key(key);

        if let Some(entry) = self.storage.remove(&warm_key) {
            let warm_entry = entry.value();
            let entry_size = warm_entry.estimated_size() as u64;

            // Update memory usage
            let current_usage = self.memory_monitor.get_current_usage();
            let _ = self
                .memory_monitor
                .update_memory_usage(current_usage - entry_size);

            // Update statistics
            self.stats.update_memory_usage(-(entry_size as i64));
            self.stats.update_entry_count(-1);

            // Record eviction for tracking with actual policy type
            let actual_policy = self.eviction_policy.current_policy();
            self.eviction_policy.record_eviction_outcome(
                &warm_key,
                actual_policy,
                true,
                None,
            );

            Some(warm_entry.value().clone())
        } else {
            None
        }
    }

    /// Get current cache statistics
    pub fn get_stats(&self) -> TierStatsSnapshot {
        self.stats.get_snapshot()
    }

    /// Get memory pressure level
    pub fn get_memory_pressure(&self) -> f64 {
        self.memory_monitor.get_pressure_level()
    }

    /// Get all cache keys currently in warm tier (limited to prevent memory issues)
    pub fn get_cache_keys(&self, limit: usize) -> Vec<K> {
        let mut keys = Vec::new();

        for entry in self.storage.iter().take(limit) {
            keys.push(entry.key().to_cache_key().clone());
        }

        keys
    }

    /// Get idle cache keys (not accessed recently)
    pub fn get_idle_keys(&self, idle_threshold: Duration) -> Vec<K> {
        let mut idle_keys = Vec::new();

        for entry in self.storage.iter() {
            let key = entry.key();
            let warm_entry = entry.value();
            if warm_entry.last_access() > idle_threshold {
                idle_keys.push(key.to_cache_key().clone());
            }
        }

        idle_keys
    }

    /// Get frequently accessed keys for promotion analysis
    pub fn get_frequently_accessed_keys(&self, count: usize) -> Vec<K> {
        // Get most frequently accessed keys from eviction policy
        let warm_keys = self.eviction_policy.get_most_frequent_keys(count);
        warm_keys.iter().map(|k| k.to_cache_key()).collect()
    }

    /// Get current cache size (number of entries)
    pub fn get_cache_size(&self) -> usize {
        self.storage.len()
    }

    /// Get estimated memory usage in bytes
    pub fn get_memory_usage(&self) -> u64 {
        self.memory_monitor.get_current_usage()
    }
}