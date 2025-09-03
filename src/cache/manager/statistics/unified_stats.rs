//! UnifiedCacheStatistics implementation - MOVED TO CANONICAL LOCATION
//!
//! UnifiedCacheStatistics moved to canonical location: crate::telemetry::unified_stats::UnifiedCacheStatistics
//! Use the canonical implementation with comprehensive metrics and performance tracking.

// Canonical types available at: crate::telemetry::unified_stats::{CachePerformanceMetrics, UnifiedCacheStatistics}

// Duplicate impl block removed - all methods available in canonical location:
// crate::telemetry::unified_stats::UnifiedCacheStatistics

/*
REMOVED DUPLICATE IMPL BLOCK - USE CANONICAL VERSION

impl UnifiedCacheStatistics {
    /// Create new unified cache statistics
    #[inline]
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
            cold_tier_stats: TierStatistics::default(),
        }
    }

    /// Record a cache hit for specific tier
    #[inline(always)]
    pub fn record_hit(&self, tier: crate::cache::coherence::CacheTier, access_time_ns: u64) {
        match tier {
            crate::cache::coherence::CacheTier::Hot => {
                self.hot_hits.fetch_add(1, Ordering::Relaxed);
            }
            crate::cache::coherence::CacheTier::Warm => {
                self.warm_hits.fetch_add(1, Ordering::Relaxed);
            }
            crate::cache::coherence::CacheTier::Cold => {
                self.cold_hits.fetch_add(1, Ordering::Relaxed);
            }
        }

        self.total_operations.fetch_add(1, Ordering::Relaxed);
        self.update_latency(access_time_ns);
        self.update_hit_rate();
    }

    /// Record a cache miss
    #[inline(always)]
    pub fn record_miss(&self, access_time_ns: u64) {
        self.total_misses.fetch_add(1, Ordering::Relaxed);
        self.total_operations.fetch_add(1, Ordering::Relaxed);
        self.update_latency(access_time_ns);
        self.update_hit_rate();
    }

    /// Record a promotion operation
    #[inline(always)]
    pub fn record_promotion(&self) {
        self.promotions_performed.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a demotion operation
    #[inline(always)]
    pub fn record_demotion(&self) {
        self.demotions_performed.fetch_add(1, Ordering::Relaxed);
    }

    /// Update memory usage statistics
    #[inline(always)]
    pub fn update_memory_usage(&self, bytes: u64) {
        self.total_memory_usage.store(bytes, Ordering::Relaxed);
    }

    /// Get current hit rate
    #[inline(always)]
    pub fn hit_rate(&self) -> f64 {
        self.overall_hit_rate.load()
    }

    /// Get total operations count
    #[inline(always)]
    pub fn total_operations(&self) -> u64 {
        self.total_operations.load(Ordering::Relaxed)
    }

    /// Get average access latency
    #[inline(always)]
    pub fn avg_access_latency_ns(&self) -> u64 {
        self.avg_access_latency_ns.load(Ordering::Relaxed)
    }

    /// Get comprehensive performance metrics
    pub fn get_performance_metrics(&self) -> CachePerformanceMetrics {
        let total_ops = self.total_operations.load(Ordering::Relaxed);
        let hot_hits = self.hot_hits.load(Ordering::Relaxed);
        let warm_hits = self.warm_hits.load(Ordering::Relaxed);
        let cold_hits = self.cold_hits.load(Ordering::Relaxed);
        let _total_misses = self.cold_tier_stats.misses;

        let total_hits = hot_hits + warm_hits + cold_hits;
        let overall_hit_rate = if total_ops > 0 {
            total_hits as f64 / total_ops as f64
        } else {
            0.0
        };

        CachePerformanceMetrics {
            overall_hit_rate,
            total_operations: total_ops,
            avg_access_latency_ns: self.avg_access_latency_ns.load(Ordering::Relaxed),
            total_memory_usage_bytes: self.total_memory_usage.load(Ordering::Relaxed),
            promotions_performed: self.promotions_performed.load(Ordering::Relaxed),
            demotions_performed: self.demotions_performed.load(Ordering::Relaxed),
            hot_tier: TierStatistics::new(
                hot_hits,  // hits: u64 - REAL hit count from atomic counter
                0,          // misses: u64 - no per-tier breakdown available
                0,          // entry_count: usize - no per-tier data available
                0,          // memory_usage: usize - no per-tier breakdown available
                0,          // peak_memory: u64 - no per-tier data available
                0,          // total_size_bytes: u64 - no per-tier data available
                0,          // avg_access_time_ns: u64 - no per-tier breakdown available
                0.0,        // ops_per_second: f64 - not tracked per-tier
                0,          // error_count: u64 - no per-tier data available
            ),
            warm_tier: TierStatistics::new(
                warm_hits, // hits: u64 - REAL hit count from atomic counter
                0,          // misses: u64 - no per-tier breakdown available
                0,          // entry_count: usize - no per-tier data available
                0,          // memory_usage: usize - no per-tier breakdown available
                0,          // peak_memory: u64 - no per-tier data available
                0,          // total_size_bytes: u64 - no per-tier data available
                0,          // avg_access_time_ns: u64 - no per-tier breakdown available
                0.0,        // ops_per_second: f64 - not tracked per-tier
                0,          // error_count: u64 - no per-tier data available
            ),
            cold_tier: TierStatistics::new(
                cold_hits, // hits: u64 - REAL hit count from atomic counter
                0,          // misses: u64 - no per-tier breakdown available
                0,          // entry_count: usize - no per-tier data available
                0,          // memory_usage: usize - no per-tier breakdown available
                0,          // peak_memory: u64 - no per-tier data available
                0,          // total_size_bytes: u64 - no per-tier data available
                0,          // avg_access_time_ns: u64 - no per-tier breakdown available
                0.0,        // ops_per_second: f64 - not tracked per-tier
                0,          // error_count: u64 - no per-tier data available
            ),
        }
    }

    /// Reset all statistics
    pub fn reset(&self) {
        self.total_operations.store(0, Ordering::Relaxed);
        self.overall_hit_rate.store(0.0);
        self.hot_hits.store(0, Ordering::Relaxed);
        self.warm_hits.store(0, Ordering::Relaxed);
        self.cold_hits.store(0, Ordering::Relaxed);
        self.total_misses.store(0, Ordering::Relaxed);
        self.avg_access_latency_ns.store(0, Ordering::Relaxed);
        self.promotions_performed.store(0, Ordering::Relaxed);
        self.demotions_performed.store(0, Ordering::Relaxed);
        self.total_memory_usage.store(0, Ordering::Relaxed);
    }



    /// Update overall hit rate
    #[inline]
    fn update_hit_rate(&self) {
        let total_ops = self.total_operations.load(Ordering::Relaxed);
        if total_ops > 0 {
            let hot_hits = self.hot_hits.load(Ordering::Relaxed);
            let warm_hits = self.warm_hits.load(Ordering::Relaxed);
            let cold_hits = self.cold_hits.load(Ordering::Relaxed);
            let total_hits = hot_hits + warm_hits + cold_hits;
            let hit_rate = total_hits as f64 / total_ops as f64;
            self.overall_hit_rate.store(hit_rate);
        }
    }

    /// Get total operations count
    #[inline]
    pub fn get_total_operations(&self) -> u64 {
        self.total_operations.load(Ordering::Relaxed)
    }

    /// Get overall hit rate
    #[inline]
    pub fn get_overall_hit_rate(&self) -> f64 {
        self.overall_hit_rate.load()
    }

    /// Get hot tier hits
    #[inline]
    pub fn get_hot_hits(&self) -> u64 {
        self.hot_hits.load(Ordering::Relaxed)
    }

    /// Get warm tier hits
    #[inline]
    pub fn get_warm_hits(&self) -> u64 {
        self.warm_hits.load(Ordering::Relaxed)
    }

    /// Get cold tier hits
    #[inline]
    pub fn get_cold_hits(&self) -> u64 {
        self.cold_hits.load(Ordering::Relaxed)
    }

    /// Get total misses
    #[inline]
    pub fn get_total_misses(&self) -> u64 {
        self.total_misses.load(Ordering::Relaxed)
    }

    /// Get average access latency in nanoseconds
    #[inline]
    pub fn get_avg_access_latency_ns(&self) -> u64 {
        self.avg_access_latency_ns.load(Ordering::Relaxed)
    }

    /// Get promotions performed
    #[inline]
    pub fn get_promotions_performed(&self) -> u64 {
        self.promotions_performed.load(Ordering::Relaxed)
    }

    /// Get demotions performed
    #[inline]
    pub fn get_demotions_performed(&self) -> u64 {
        self.demotions_performed.load(Ordering::Relaxed)
    }

    /// Get total memory usage
    #[inline]
    pub fn get_total_memory_usage(&self) -> u64 {
        self.total_memory_usage.load(Ordering::Relaxed)
    }

    /// Record hot tier hit
    #[inline]
    pub fn record_hot_hit(&self, access_time_ns: u64) {
        self.hot_hits.fetch_add(1, Ordering::Relaxed);
        self.total_operations.fetch_add(1, Ordering::Relaxed);
        self.update_latency(access_time_ns);
        self.update_hit_rate();
    }

    /// Record warm tier hit
    #[inline]
    pub fn record_warm_hit(&self, access_time_ns: u64) {
        self.warm_hits.fetch_add(1, Ordering::Relaxed);
        self.total_operations.fetch_add(1, Ordering::Relaxed);
        self.update_latency(access_time_ns);
        self.update_hit_rate();
    }

    /// Record cold tier hit
    #[inline]
    pub fn record_cold_hit(&self, access_time_ns: u64) {
        self.cold_hits.fetch_add(1, Ordering::Relaxed);
        self.total_operations.fetch_add(1, Ordering::Relaxed);
        self.update_latency(access_time_ns);
        self.update_hit_rate();
    }

    /// Update access latency
    #[inline]
    pub fn update_access_latency(&self, latency_ns: u64) {
        self.update_latency(latency_ns);
    }

    /// Update latency with exponential moving average
*/

// All UnifiedCacheStatistics functionality now available through canonical import:
// use crate::telemetry::unified_stats::UnifiedCacheStatistics;
