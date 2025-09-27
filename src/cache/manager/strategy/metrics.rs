//! Strategy performance metrics tracking
//!
//! This module provides performance metrics collection and analysis
//! for cache strategy evaluation and comparison.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crossbeam_utils::{CachePadded, atomic::AtomicCell};

use super::core::CacheStrategy;

/// Performance metrics for cache strategy evaluation
#[derive(Debug)]
#[repr(align(64))]
pub struct StrategyMetrics {
    /// Hit rates per strategy (x1000 for precision)
    hit_rates: CachePadded<[AtomicU32; 5]>,
    /// Average access times per strategy (nanoseconds)
    access_times: CachePadded<[AtomicU64; 5]>,
    /// Operation counts per strategy
    operation_counts: CachePadded<[AtomicU64; 5]>,
    /// Memory efficiency scores per strategy (x1000)
    memory_efficiency: CachePadded<[AtomicU32; 5]>,
    /// Evaluation period for strategy comparison

    #[allow(dead_code)]
    // Strategy management - evaluation_period used in strategy metrics evaluation timing
    evaluation_period: Duration,
    /// Last evaluation timestamp
    last_evaluation: AtomicCell<Instant>,
}

impl Default for StrategyMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl StrategyMetrics {
    /// Create new strategy metrics tracker
    #[inline]
    pub fn new() -> Self {
        Self {
            hit_rates: CachePadded::new([const { AtomicU32::new(0) }; 5]),
            access_times: CachePadded::new([const { AtomicU64::new(0) }; 5]),
            operation_counts: CachePadded::new([const { AtomicU64::new(0) }; 5]),
            memory_efficiency: CachePadded::new([const { AtomicU32::new(0) }; 5]),
            evaluation_period: Duration::from_secs(30),
            last_evaluation: AtomicCell::new(Instant::now()),
        }
    }

    /// Record hit for strategy
    #[inline(always)]
    pub fn record_hit(&self, strategy: CacheStrategy, access_time_ns: u64) {
        let idx = strategy.index();
        self.hit_rates[idx].fetch_add(1000, Ordering::Relaxed); // Increment hit count
        self.access_times[idx].fetch_add(access_time_ns, Ordering::Relaxed);
        self.operation_counts[idx].fetch_add(1, Ordering::Relaxed);
    }

    /// Record miss for strategy  
    #[inline(always)]
    pub fn record_miss(&self, strategy: CacheStrategy, access_time_ns: u64) {
        let idx = strategy.index();
        self.access_times[idx].fetch_add(access_time_ns, Ordering::Relaxed);
        self.operation_counts[idx].fetch_add(1, Ordering::Relaxed);
    }

    /// Get hit rate for strategy (0-1000)
    #[allow(dead_code)] // Strategy management - hit_rate used in strategy performance evaluation
    #[inline(always)]
    pub fn hit_rate(&self, strategy: CacheStrategy) -> u32 {
        let idx = strategy.index();
        let hits = self.hit_rates[idx].load(Ordering::Relaxed);
        let total = self.operation_counts[idx].load(Ordering::Relaxed);
        if total > 0 {
            ((hits as u64 * 100) / total) as u32
        } else {
            0
        }
    }

    /// Get average access time for strategy
    #[inline(always)]
    pub fn avg_access_time(&self, strategy: CacheStrategy) -> u64 {
        let idx = strategy.index();
        let total_time = self.access_times[idx].load(Ordering::Relaxed);
        let total_ops = self.operation_counts[idx].load(Ordering::Relaxed);
        if total_ops > 0 {
            total_time / total_ops
        } else {
            0
        }
    }

    /// Calculate performance score for strategy (higher is better)
    #[inline(always)]
    pub fn performance_score(&self, strategy: CacheStrategy) -> u32 {
        let idx = strategy.index();
        let hits = self.hit_rates[idx].load(Ordering::Relaxed);
        let total_ops = self.operation_counts[idx].load(Ordering::Relaxed);
        let hit_rate = hits as f64 / total_ops as f64;
        let avg_time = self.avg_access_time(strategy);
        // Score based on hit rate (80%) and speed (20%)
        let time_score = if avg_time > 0 {
            1_000_000 / avg_time.max(1)
        } else {
            1000
        };
        ((hit_rate * 8.0) as u32 + time_score as u32 * 2) / 10
    }

    /// Reset metrics for evaluation period
    #[inline]
    pub fn reset_evaluation(&self) {
        for i in 0..5 {
            self.hit_rates[i].store(0, Ordering::Relaxed);
            self.access_times[i].store(0, Ordering::Relaxed);
            self.operation_counts[i].store(0, Ordering::Relaxed);
            self.memory_efficiency[i].store(0, Ordering::Relaxed);
        }
        self.last_evaluation.store(Instant::now());
    }
}
