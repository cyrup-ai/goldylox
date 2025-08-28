//! Cache operations engine for unified cache manager
//!
//! This module implements the core cache operations including get, put,
//! and tier access coordination for the unified cache system.

use std::sync::atomic::Ordering;

use crate::cache::traits::core::{CacheKey, CacheValue};
use crate::cache::types::CacheTier;
use super::types::{UnifiedCacheManager, UnifiedStats};
use crate::cache::types::AccessPath;
use crate::cache::types::performance::timer::PrecisionTimer;
use crate::cache::traits::types_and_enums::CacheOperationError;

impl<K: CacheKey + Default, V: CacheValue> UnifiedCacheManager<K, V> {
    /// Get value from unified cache with intelligent tier selection
    pub fn get(&self, key: &K) -> Option<V> {
        let timer = PrecisionTimer::start();
        let mut access_path = AccessPath::new();

        // Record operation start
        self.unified_stats
            .total_operations
            .fetch_add(1, Ordering::Relaxed);

        // Try hot tier first (fastest)
        if let Some(value) = self.try_hot_tier_get(key, &mut access_path) {
            let elapsed_ns = timer.elapsed_ns();
            self.record_hit(CacheTier::Hot, elapsed_ns);
            let _ = self.policy_engine.pattern_analyzer.record_access(key);
            return Some(value);
        }

        // Try warm tier second
        if let Some(value) = self.try_warm_tier_get(key, &mut access_path) {
            let elapsed_ns = timer.elapsed_ns();
            self.record_hit(CacheTier::Warm, elapsed_ns);
            let _ = self.policy_engine.pattern_analyzer.record_access(key);

            // Consider promotion to hot tier using sophisticated API
            self.consider_promotion(
                key,
                &value,
                CacheTier::Warm,
                CacheTier::Hot,
                &access_path,
            );

            return Some(value);
        }

        // Try cold tier last
        if let Some(value) = self.try_cold_tier_get(key, &mut access_path) {
            let elapsed_ns = timer.elapsed_ns();
            self.record_hit(CacheTier::Cold, elapsed_ns);
            let _ = self.policy_engine.pattern_analyzer.record_access(key);

            // Consider promotion to warm or hot tier using sophisticated API
            self.consider_multi_tier_promotion(key, &value, &access_path);

            return Some(value);
        }

        // Cache miss
        let elapsed_ns = timer.elapsed_ns();
        self.record_miss(elapsed_ns);
        let _ = self.policy_engine.pattern_analyzer.record_miss(key);

        None
    }

    /// Put value in unified cache with optimal tier placement
    pub fn put(&self, key: K, value: V) -> Result<(), CacheOperationError> {
        let timer = PrecisionTimer::start();

        // Analyze value characteristics for optimal placement
        let placement_decision = self.analyze_placement(&key, &value);

        match placement_decision.primary_tier {
            CacheTier::Hot => {
                // Place in hot tier, potentially replicate to warm
                self.put_with_replication(
                    key,
                    value,
                    CacheTier::Hot,
                    placement_decision.replication_tiers,
                )?;
            }
            CacheTier::Warm => {
                // Place in warm tier, potentially replicate to cold
                self.put_with_replication(
                    key,
                    value,
                    CacheTier::Warm,
                    placement_decision.replication_tiers,
                )?;
            }
            CacheTier::Cold => {
                // Place only in cold tier (large, infrequently accessed)
                self.put_cold_tier_only(key, value)?;
            }
        }

        let elapsed_ns = timer.elapsed_ns();
        self.unified_stats.avg_access_latency_ns.store(
            self.calculate_running_average(elapsed_ns),
            Ordering::Relaxed,
        );

        Ok(())
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> UnifiedStats {
        UnifiedStats {
            total_operations: self.unified_stats.get_total_operations(),
            overall_hit_rate: self.unified_stats.get_overall_hit_rate(),
            hot_tier_hits: self.unified_stats.get_hot_hits(),
            warm_tier_hits: self.unified_stats.get_warm_hits(),
            cold_tier_hits: self.unified_stats.get_cold_hits(),
            total_misses: self.unified_stats.get_total_misses(),
            avg_access_latency_ns: self.unified_stats.get_avg_access_latency_ns(),
            promotions_performed: self.unified_stats.get_promotions_performed(),
            demotions_performed: self.unified_stats.get_demotions_performed(),
            total_memory_usage: self.unified_stats.get_total_memory_usage(),
        }
    }
}
