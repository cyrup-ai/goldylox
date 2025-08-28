//! Utility functions and helper methods for unified cache manager
//!
//! This module contains helper methods, stub implementations, and utility
//! functions used throughout the unified cache management system.

// Removed unused import
use crate::cache::tier::warm::warm_get;
use crate::cache::types::CacheTier;
use super::types::UnifiedCacheManager;
use crate::cache::types::AccessPath;
use crate::cache::traits::{CacheKey, CacheValue};
use crate::cache::traits::entry_and_stats::TierStats;

impl<K: CacheKey + Default, V: CacheValue> UnifiedCacheManager<K, V> {
    /// Try to get value from hot tier
    pub fn try_hot_tier_get(&self, key: &K, access_path: &mut AccessPath) -> Option<V> {
        access_path.tried_hot = true;
        crate::cache::tier::hot::simd_hot_get::<K, V>(key)
    }

    /// Try to get value from warm tier
    pub fn try_warm_tier_get(&self, key: &K, access_path: &mut AccessPath) -> Option<V> {
        access_path.tried_warm = true;
        warm_get(key)
    }

    /// Try to get value from cold tier
    pub fn try_cold_tier_get(&self, key: &K, access_path: &mut AccessPath) -> Option<V> {
        access_path.tried_cold = true;
        crate::cache::tier::cold::cold_get::<K, V>(key).ok().flatten()
    }

    /// Record cache hit for statistics
    pub fn record_hit(&self, tier: CacheTier, elapsed_ns: u64) {
        match tier {
            CacheTier::Hot => {
                self.unified_stats.record_hot_hit(elapsed_ns);
            }
            CacheTier::Warm => {
                self.unified_stats.record_warm_hit(elapsed_ns);
            }
            CacheTier::Cold => {
                self.unified_stats.record_cold_hit(elapsed_ns);
            }
        }
    }

    /// Record cache miss for statistics
    pub fn record_miss(&self, elapsed_ns: u64) {
        self.unified_stats.record_miss(elapsed_ns);
        self.unified_stats.update_access_latency(elapsed_ns);
    }

    /// Calculate running average for latency tracking
    pub fn calculate_running_average(&self, new_value: u64) -> u64 {
        // Simple exponential moving average
        let current_avg = self.unified_stats.get_avg_access_latency_ns();
        if current_avg == 0 {
            new_value
        } else {
            (current_avg * 7 + new_value) / 8
        }
    }

    /// Check if tier has capacity for new entries
    pub fn has_tier_capacity(&self, tier: CacheTier) -> bool {
        let utilization = self.get_tier_utilization(tier);
        let capacity_threshold = match tier {
            CacheTier::Hot => 0.85,  // 85% capacity threshold for hot tier
            CacheTier::Warm => 0.90, // 90% capacity threshold for warm tier
            CacheTier::Cold => 0.95, // 95% capacity threshold for cold tier
        };

        utilization < capacity_threshold
    }

    /// Get tier utilization percentage
    pub fn get_tier_utilization(&self, tier: CacheTier) -> f32 {
        match tier {
            CacheTier::Hot => {
                // Calculate hot tier utilization from memory pool stats
                let stats = crate::cache::tier::hot::hot_tier_memory_stats::<K, V>();
                if stats.total_slots > 0 {
                    stats.occupied_slots as f32 / stats.total_slots as f32
                } else {
                    0.0
                }
            }
            CacheTier::Warm => {
                // Calculate warm tier utilization from actual stats
                crate::cache::tier::warm::get_memory_pressure::<K, V>().unwrap_or(0.3) as f32
                // Fallback to 30% if stats unavailable
            }
            CacheTier::Cold => {
                // Calculate cold tier utilization from actual stats
                crate::cache::tier::cold::get_stats::<K, V>()
                    .map(|stats| {
                        // Calculate utilization as ratio of hits to total accesses
                        let total_accesses = stats.total_hits() + stats.total_misses();
                        if total_accesses > 0 {
                            stats.total_hits() as f32 / total_accesses as f32
                        } else {
                            0.0
                        }
                    })
                    .unwrap_or(0.1) // Fallback to 10% if stats unavailable
            }
        }
    }
}

// PrecisionTimer moved to canonical location: crate::cache::types::performance::timer::PrecisionTimer
