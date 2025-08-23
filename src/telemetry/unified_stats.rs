//! Unified cache statistics across all tiers with atomic coordination
//!
//! This module implements the `UnifiedCacheStatistics` structure that provides
//! atomic performance tracking across hot, warm, and cold cache tiers.

use std::sync::atomic::{AtomicU64, Ordering};

use crossbeam_utils::{atomic::AtomicCell, CachePadded};

use super::super::coherence::CacheTier;
use super::data_structures::{OpsPerSecondState, TierHitRateState};

/// Unified cache statistics across all tiers with atomic coordination
#[derive(Debug)]
pub struct UnifiedCacheStatistics {
    /// Total cache operations counter (all tiers)
    total_operations: CachePadded<AtomicU64>,
    /// Overall hit rate with precision (rate * 1000)
    overall_hit_rate: CachePadded<AtomicCell<f64>>,
    /// Per-tier hit counts for detailed analysis
    hot_hits: CachePadded<AtomicU64>,
    warm_hits: CachePadded<AtomicU64>,
    cold_hits: CachePadded<AtomicU64>,
    /// Miss count for hit rate calculation
    total_misses: CachePadded<AtomicU64>,
    /// Running average access latency (nanoseconds)
    avg_access_latency_ns: CachePadded<AtomicU64>,
    /// Data promotion count for tier efficiency analysis
    promotions_performed: CachePadded<AtomicU64>,
    /// Data demotion count for tier efficiency analysis
    demotions_performed: CachePadded<AtomicU64>,
    /// Total memory usage across all tiers (bytes)
    total_memory_usage: CachePadded<AtomicU64>,
    /// Peak memory usage reached (bytes)
    peak_memory_usage: CachePadded<AtomicU64>,
    /// Operations per second calculation state
    ops_per_second_state: OpsPerSecondState,
    /// Hit rate calculation state per tier
    tier_hit_rates: TierHitRateState,
}

/// Unified statistics result structure
#[derive(Debug, Clone)]
pub struct UnifiedStats {
    pub total_operations: u64,
    pub overall_hit_rate: f64,
    pub hot_tier_hits: u64,
    pub warm_tier_hits: u64,
    pub cold_tier_hits: u64,
    pub total_misses: u64,
    pub avg_access_latency_ns: u64,
    pub promotions_performed: u64,
    pub demotions_performed: u64,
    pub total_memory_usage: u64,
    pub peak_memory_usage: u64,
    pub ops_per_second: f32,
    pub tier_hit_rates: [f32; 3], // Hot, Warm, Cold
}

impl UnifiedCacheStatistics {
    /// Create new unified statistics with atomic initialization
    pub fn new() -> Self {
        Self {
            total_operations: CachePadded::new(AtomicU64::new(0)),
            overall_hit_rate: CachePadded::new(AtomicCell::new(0.0)),
            hot_hits: CachePadded::new(AtomicU64::new(0)),
            warm_hits: CachePadded::new(AtomicU64::new(0)),
            cold_hits: CachePadded::new(AtomicU64::new(0)),
            total_misses: CachePadded::new(AtomicU64::new(0)),
            avg_access_latency_ns: CachePadded::new(AtomicU64::new(0)),
            promotions_performed: CachePadded::new(AtomicU64::new(0)),
            demotions_performed: CachePadded::new(AtomicU64::new(0)),
            total_memory_usage: CachePadded::new(AtomicU64::new(0)),
            peak_memory_usage: CachePadded::new(AtomicU64::new(0)),
            ops_per_second_state: OpsPerSecondState::new(),
            tier_hit_rates: TierHitRateState::new(),
        }
    }

    /// Record cache hit for specific tier with atomic update
    pub fn record_hit(&self, tier: CacheTier, access_time_ns: u64) {
        // Update total operations atomically
        self.total_operations.fetch_add(1, Ordering::Relaxed);

        // Update tier-specific hit counters
        match tier {
            CacheTier::Hot => {
                self.hot_hits.fetch_add(1, Ordering::Relaxed);
                self.tier_hit_rates.record_hit(0, access_time_ns);
            }
            CacheTier::Warm => {
                self.warm_hits.fetch_add(1, Ordering::Relaxed);
                self.tier_hit_rates.record_hit(1, access_time_ns);
            }
            CacheTier::Cold => {
                self.cold_hits.fetch_add(1, Ordering::Relaxed);
                self.tier_hit_rates.record_hit(2, access_time_ns);
            }
        }

        // Update running average latency
        self.update_average_latency(access_time_ns);

        // Recalculate overall hit rate
        self.recalculate_hit_rate();
    }

    /// Record cache miss with atomic update
    pub fn record_miss(&self, access_time_ns: u64) {
        // Update counters atomically
        self.total_operations.fetch_add(1, Ordering::Relaxed);
        self.total_misses.fetch_add(1, Ordering::Relaxed);

        // Update average latency (misses typically have higher latency)
        self.update_average_latency(access_time_ns);

        // Recalculate overall hit rate
        self.recalculate_hit_rate();
    }

    /// Update memory usage with atomic peak tracking
    pub fn update_memory_usage(&self, current_usage: u64) {
        self.total_memory_usage
            .store(current_usage, Ordering::Relaxed);

        // Update peak using compare-and-swap loop
        let mut current_peak = self.peak_memory_usage.load(Ordering::Relaxed);
        while current_usage > current_peak {
            match self.peak_memory_usage.compare_exchange_weak(
                current_peak,
                current_usage,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current_peak = actual,
            }
        }
    }

    /// Record data promotion between tiers
    pub fn record_promotion(&self) {
        self.promotions_performed.fetch_add(1, Ordering::Relaxed);
    }

    /// Record data demotion between tiers
    pub fn record_demotion(&self) {
        self.demotions_performed.fetch_add(1, Ordering::Relaxed);
    }

    /// Compute comprehensive unified statistics
    pub fn compute_unified_stats(&self) -> UnifiedStats {
        let total_ops = self.total_operations.load(Ordering::Relaxed);
        let hot_hits = self.hot_hits.load(Ordering::Relaxed);
        let warm_hits = self.warm_hits.load(Ordering::Relaxed);
        let cold_hits = self.cold_hits.load(Ordering::Relaxed);
        let total_misses = self.total_misses.load(Ordering::Relaxed);

        let total_hits = hot_hits + warm_hits + cold_hits;
        let overall_hit_rate = if total_ops > 0 {
            total_hits as f64 / total_ops as f64
        } else {
            0.0
        };

        UnifiedStats {
            total_operations: total_ops,
            overall_hit_rate,
            hot_tier_hits: hot_hits,
            warm_tier_hits: warm_hits,
            cold_tier_hits: cold_hits,
            total_misses,
            avg_access_latency_ns: self.avg_access_latency_ns.load(Ordering::Relaxed),
            promotions_performed: self.promotions_performed.load(Ordering::Relaxed),
            demotions_performed: self.demotions_performed.load(Ordering::Relaxed),
            total_memory_usage: self.total_memory_usage.load(Ordering::Relaxed),
            peak_memory_usage: self.peak_memory_usage.load(Ordering::Relaxed),
            ops_per_second: self.ops_per_second_state.get_current_ops_per_second(),
            tier_hit_rates: [
                self.tier_hit_rates.get_tier_rate(0),
                self.tier_hit_rates.get_tier_rate(1),
                self.tier_hit_rates.get_tier_rate(2),
            ],
        }
    }

    /// Reset all atomic counters (for testing or reinitialization)
    pub fn reset_all_counters(&self) {
        self.total_operations.store(0, Ordering::Relaxed);
        self.hot_hits.store(0, Ordering::Relaxed);
        self.warm_hits.store(0, Ordering::Relaxed);
        self.cold_hits.store(0, Ordering::Relaxed);
        self.total_misses.store(0, Ordering::Relaxed);
        self.avg_access_latency_ns.store(0, Ordering::Relaxed);
        self.promotions_performed.store(0, Ordering::Relaxed);
        self.demotions_performed.store(0, Ordering::Relaxed);
        self.total_memory_usage.store(0, Ordering::Relaxed);
        self.peak_memory_usage.store(0, Ordering::Relaxed);
        self.overall_hit_rate.store(0.0);

        // Reset operational state
        self.ops_per_second_state.reset();
        self.tier_hit_rates.reset();
    }

    /// Update running average access latency
    fn update_average_latency(&self, new_latency: u64) {
        let current_avg = self.avg_access_latency_ns.load(Ordering::Relaxed);
        let total_ops = self.total_operations.load(Ordering::Relaxed);

        let new_avg = if total_ops <= 1 {
            new_latency
        } else {
            // Exponential moving average with alpha = 0.1
            (current_avg * 9 + new_latency) / 10
        };

        self.avg_access_latency_ns.store(new_avg, Ordering::Relaxed);
    }

    /// Recalculate overall hit rate atomically
    fn recalculate_hit_rate(&self) {
        let total_ops = self.total_operations.load(Ordering::Relaxed);
        if total_ops == 0 {
            self.overall_hit_rate.store(0.0);
            return;
        }

        let hot_hits = self.hot_hits.load(Ordering::Relaxed);
        let warm_hits = self.warm_hits.load(Ordering::Relaxed);
        let cold_hits = self.cold_hits.load(Ordering::Relaxed);
        let total_hits = hot_hits + warm_hits + cold_hits;

        let hit_rate = total_hits as f64 / total_ops as f64;
        self.overall_hit_rate.store(hit_rate);
    }
}
