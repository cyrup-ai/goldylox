//! LFU (Least Frequently Used) eviction policy implementation
//!
//! This module implements a concurrent LFU eviction policy with frequency estimation
//! and trend analysis for optimal cache performance.

use std::sync::atomic::{AtomicU64, Ordering};

use crossbeam_skiplist::SkipMap;
use crossbeam_utils::atomic::AtomicCell;

use super::types::*;
use crate::cache::tier::warm::core::WarmCacheKey;
use crate::cache::traits::CacheKey;
use crate::telemetry::cache::types::timestamp_nanos;

/// Concurrent LFU tracker with frequency estimation
#[derive(Debug)]
pub struct ConcurrentLfuTracker<K: CacheKey> {
    /// Frequency estimates per key
    frequencies: SkipMap<WarmCacheKey<K>, FrequencyData>,
    /// Global frequency statistics
    global_stats: FrequencyStats,
    /// LFU-specific statistics
    stats: LfuStats,
    /// Last decay operation timestamp (nanoseconds)
    last_decay_time: AtomicU64,
    /// Adaptive decay factor (0.5 to 0.95)
    decay_factor: AtomicCell<f64>,
    /// Last eviction count (for rate calculation)
    last_eviction_count: AtomicU64,
}

/// Frequency data for LFU tracking
#[derive(Debug)]
pub struct FrequencyData {
    /// Current frequency estimate
    frequency: AtomicCell<f64>,
    /// Access count
    access_count: AtomicU64,
    /// Last update timestamp
    last_update_ns: AtomicU64,
    /// Frequency trend (increasing/decreasing/stable)
    trend: AtomicCell<FrequencyTrend>,
}

impl<K: CacheKey> ConcurrentLfuTracker<K> {
    /// Create new concurrent LFU tracker
    pub fn new() -> Self {
        Self {
            frequencies: SkipMap::new(),
            global_stats: FrequencyStats::default(),
            stats: LfuStats::default(),
            last_decay_time: AtomicU64::new(timestamp_nanos()),
            decay_factor: AtomicCell::new(0.9),
            last_eviction_count: AtomicU64::new(0),
        }
    }

    /// Record access for LFU tracking
    pub fn record_access(&self, key: &WarmCacheKey<K>) {
        let now_ns = timestamp_nanos();
        self.stats.frequency_updates.fetch_add(1, Ordering::Relaxed);

        if let Some(freq_data) = self.frequencies.get(key) {
            // Update existing frequency data
            let old_count = freq_data
                .value()
                .access_count
                .fetch_add(1, Ordering::Relaxed);
            let new_frequency = self.calculate_frequency(old_count + 1, now_ns, freq_data.value());

            // Update trend analysis
            let old_frequency = freq_data.value().frequency.load();
            let trend = if new_frequency > old_frequency * 1.1 {
                FrequencyTrend::Increasing
            } else if new_frequency < old_frequency * 0.9 {
                FrequencyTrend::Decreasing
            } else {
                FrequencyTrend::Stable
            };

            freq_data.value().frequency.store(new_frequency);
            freq_data
                .value()
                .last_update_ns
                .store(now_ns, Ordering::Relaxed);
            freq_data.value().trend.store(trend);
        } else {
            // Create new frequency data
            let freq_data = FrequencyData {
                frequency: AtomicCell::new(1.0),
                access_count: AtomicU64::new(1),
                last_update_ns: AtomicU64::new(now_ns),
                trend: AtomicCell::new(FrequencyTrend::Stable),
            };
            self.frequencies.insert(key.clone(), freq_data);
            self.global_stats
                .unique_keys
                .fetch_add(1, Ordering::Relaxed);
        }

        self.global_stats
            .total_accesses
            .fetch_add(1, Ordering::Relaxed);
        self.update_global_average();
    }

    /// Calculate frequency based on access pattern
    fn calculate_frequency(
        &self,
        _access_count: u64,
        current_ns: u64,
        freq_data: &FrequencyData,
    ) -> f64 {
        let last_update_ns = freq_data.last_update_ns.load(Ordering::Relaxed);
        let time_delta_ns = current_ns.saturating_sub(last_update_ns);

        if time_delta_ns == 0 {
            return freq_data.frequency.load();
        }

        // Calculate instantaneous frequency (accesses per second)
        let time_delta_seconds = time_delta_ns as f64 / 1_000_000_000.0;
        let instantaneous_frequency = 1.0 / time_delta_seconds;

        // Exponential moving average with previous frequency
        let alpha = 0.1; // Smoothing factor
        let previous_frequency = freq_data.frequency.load();
        alpha * instantaneous_frequency + (1.0 - alpha) * previous_frequency
    }

    /// Update global frequency average
    fn update_global_average(&self) {
        let total_accesses = self.global_stats.total_accesses.load(Ordering::Relaxed);
        let unique_keys = self.global_stats.unique_keys.load(Ordering::Relaxed);

        if unique_keys > 0 {
            let avg_frequency = total_accesses as f64 / unique_keys as f64;
            self.global_stats.avg_frequency.store(avg_frequency);
            self.stats.avg_frequency.store(avg_frequency);
        }
    }

    /// Handle eviction event
    pub fn on_eviction(&self, key: &WarmCacheKey<K>) {
        if self.frequencies.remove(key).is_some() {
            self.global_stats
                .unique_keys
                .fetch_sub(1, Ordering::Relaxed);
            self.stats.lfu_evictions.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Select least frequent entries for eviction
    pub fn select_least_frequent(&self, count: usize) -> Vec<WarmCacheKey<K>> {
        let mut candidates: Vec<(WarmCacheKey<K>, f64)> = self
            .frequencies
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().frequency.load()))
            .collect();

        // Sort by frequency (ascending)
        candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        candidates
            .into_iter()
            .take(count)
            .map(|(key, _)| key)
            .collect()
    }

    /// Select victim from candidates using LFU policy
    pub fn select_victim(&self, candidates: &[WarmCacheKey<K>]) -> Option<WarmCacheKey<K>> {
        if candidates.is_empty() {
            return None;
        }

        let mut least_frequent_key: Option<WarmCacheKey<K>> = None;
        let mut lowest_frequency = f64::INFINITY;

        for candidate in candidates {
            if let Some(freq_data) = self.frequencies.get(candidate) {
                let frequency = freq_data.value().frequency.load();
                if frequency < lowest_frequency {
                    lowest_frequency = frequency;
                    least_frequent_key = Some(candidate.clone());
                }
            }
        }

        least_frequent_key
    }

    /// Get frequency for a key
    pub fn get_frequency(&self, key: &WarmCacheKey<K>) -> Option<f64> {
        self.frequencies
            .get(key)
            .map(|entry| entry.value().frequency.load())
    }

    /// Get access count for a key
    pub fn get_access_count(&self, key: &WarmCacheKey<K>) -> Option<u64> {
        self.frequencies
            .get(key)
            .map(|entry| entry.value().access_count.load(Ordering::Relaxed))
    }

    /// Get frequency trend for a key
    pub fn get_trend(&self, key: &WarmCacheKey<K>) -> Option<FrequencyTrend> {
        self.frequencies
            .get(key)
            .map(|entry| entry.value().trend.load())
    }

    /// Get global statistics
    pub fn global_stats(&self) -> &FrequencyStats {
        &self.global_stats
    }

    /// Get LFU-specific statistics
    pub fn stats(&self) -> &LfuStats {
        &self.stats
    }

    /// Get current size
    pub fn size(&self) -> usize {
        self.frequencies.len()
    }

    /// Check if key exists
    pub fn contains(&self, key: &WarmCacheKey<K>) -> bool {
        self.frequencies.contains_key(key)
    }

    /// Clear all entries
    pub fn clear(&self) {
        self.frequencies.clear();
        self.global_stats.unique_keys.store(0, Ordering::Relaxed);
        self.global_stats.total_accesses.store(0, Ordering::Relaxed);
        self.global_stats.avg_frequency.store(0.0);
    }

    /// Get keys sorted by frequency (ascending)
    pub fn keys_by_frequency(&self) -> Vec<(WarmCacheKey<K>, f64)> {
        let mut keys: Vec<(WarmCacheKey<K>, f64)> = self
            .frequencies
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().frequency.load()))
            .collect();

        keys.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        keys
    }

    /// Get most frequent keys
    pub fn most_frequent_keys(&self, count: usize) -> Vec<WarmCacheKey<K>> {
        let mut candidates: Vec<(WarmCacheKey<K>, f64)> = self
            .frequencies
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().frequency.load()))
            .collect();

        // Sort by frequency (descending)
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        candidates
            .into_iter()
            .take(count)
            .map(|(key, _)| key)
            .collect()
    }

    /// Get hit rate based on evictions vs accesses
    pub fn hit_rate(&self) -> f64 {
        let total_accesses = self.global_stats.total_accesses.load(Ordering::Relaxed);
        let evictions = self.stats.lfu_evictions.load(Ordering::Relaxed);

        if total_accesses == 0 {
            return 1.0;
        }

        let hits = total_accesses.saturating_sub(evictions);
        hits as f64 / total_accesses as f64
    }

    /// Get average frequency
    pub fn average_frequency(&self) -> f64 {
        self.stats.avg_frequency.load()
    }

    /// Apply frequency decay to all tracked entries
    /// 
    /// Multiplies all frequencies by the current decay factor and removes
    /// entries that have decayed below the significance threshold (0.01).
    /// This is called periodically by adapt() to prevent stale data accumulation.
    fn apply_frequency_decay(&self) {
        let decay = self.decay_factor.load();
        let mut _removed_count = 0usize;
        
        // Collect keys to remove (can't remove during SkipMap iteration)
        let mut keys_to_remove = Vec::new();
        
        // Iterate and decay all frequencies
        for entry in self.frequencies.iter() {
            let key = entry.key();
            let freq_data = entry.value();
            
            let old_frequency = freq_data.frequency.load();
            let new_frequency = old_frequency * decay;
            
            if new_frequency < 0.01 {
                // Frequency decayed to insignificance - mark for removal
                keys_to_remove.push(key.clone());
            } else {
                // Update frequency atomically
                freq_data.frequency.store(new_frequency);
            }
        }
        
        // Remove entries that decayed below threshold
        for key in keys_to_remove {
            if self.frequencies.remove(&key).is_some() {
                self.global_stats.unique_keys.fetch_sub(1, Ordering::Relaxed);
                _removed_count += 1;
            }
        }
        
        // Update global statistics after decay
        self.update_global_average();
    }
}

impl<K: CacheKey> Default for ConcurrentLfuTracker<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: CacheKey> EvictionPolicy<WarmCacheKey<K>> for ConcurrentLfuTracker<K> {
    fn on_access(&self, key: &WarmCacheKey<K>, _hit: bool) {
        self.record_access(key);
    }

    fn on_eviction(&self, key: &WarmCacheKey<K>) {
        self.on_eviction(key);
    }

    fn select_candidates(&self, count: usize) -> Vec<WarmCacheKey<K>> {
        self.select_least_frequent(count)
    }

    fn performance_metrics(&self) -> PolicyPerformanceMetrics {
        PolicyPerformanceMetrics {
            hit_rate: self.hit_rate(),
            avg_access_time_ns: self.calculate_average_access_time_ns(),
            eviction_efficiency: self.hit_rate(), // Use hit rate as efficiency proxy
        }
    }

    /// Calculate average access time in nanoseconds based on frequency update times
    fn calculate_average_access_time_ns(&self) -> u64 {
        let current_time = timestamp_nanos();
        let mut total_time_diff = 0u64;
        let mut count = 0usize;

        // Sample recent entries to calculate average access time
        for entry in self.frequencies.iter().rev().take(100) {
            let last_update = entry.value().last_update_ns.load(Ordering::Relaxed);
            if last_update > 0 {
                let time_diff = current_time.saturating_sub(last_update);
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
        let now_ns = timestamp_nanos();
        let last_decay = self.last_decay_time.load(Ordering::Relaxed);
        
        // Decay interval: 5 minutes in nanoseconds
        const DECAY_INTERVAL_NS: u64 = 300_000_000_000; // 5 min = 300 sec = 300B ns
        
        let time_elapsed = now_ns.saturating_sub(last_decay);
        
        // Check if enough time has passed
        if time_elapsed < DECAY_INTERVAL_NS {
            return; // Not yet time to decay
        }
        
        // Calculate eviction rate (evictions per minute)
        let current_evictions = self.stats.lfu_evictions.load(Ordering::Relaxed);
        let last_evictions = self.last_eviction_count.load(Ordering::Relaxed);
        let evictions_delta = current_evictions.saturating_sub(last_evictions);
        
        let time_minutes = time_elapsed as f64 / 60_000_000_000.0; // ns to minutes
        let eviction_rate = if time_minutes > 0.0 {
            evictions_delta as f64 / time_minutes
        } else {
            0.0
        };
        
        // Adapt decay factor based on cache pressure
        let current_decay = self.decay_factor.load();
        let new_decay = if eviction_rate > 100.0 {
            // High eviction rate (>100/min) = high churn
            // Decrease factor (more aggressive) to clear stale entries faster
            (current_decay * 0.9).max(0.5)
        } else if eviction_rate < 10.0 {
            // Low eviction rate (<10/min) = stable workload
            // Increase factor (less aggressive) to preserve history
            (current_decay * 1.05).min(0.95)
        } else {
            // Medium eviction rate = balanced workload
            current_decay // No change
        };
        
        // Update decay factor if changed
        if (new_decay - current_decay).abs() > 0.001 {
            self.decay_factor.store(new_decay);
        }
        
        // Apply the frequency decay
        self.apply_frequency_decay();
        
        // Update tracking state
        self.last_decay_time.store(now_ns, Ordering::Relaxed);
        self.last_eviction_count.store(current_evictions, Ordering::Relaxed);
    }
}
