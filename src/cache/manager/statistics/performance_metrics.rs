//! CachePerformanceMetrics implementation
//!
//! This module provides implementation for cache performance analysis including
//! efficiency scoring, tier distribution analysis, and health monitoring.

use crate::telemetry::unified_stats::CachePerformanceMetrics;
use crate::cache::types::statistics::tier_stats::TierStatistics;

impl Default for CachePerformanceMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl CachePerformanceMetrics {
    /// Create new performance metrics
    #[allow(dead_code)] // Performance metrics - constructor for cache performance tracking
    pub fn new() -> Self {
        Self {
            overall_hit_rate: 0.0,
            total_operations: 0,
            avg_access_latency_ns: 0,
            total_memory_usage_bytes: 0,
            promotions_performed: 0,
            demotions_performed: 0,
            hot_tier: TierStatistics::default(),
            warm_tier: TierStatistics::default(),
            cold_tier: TierStatistics::default(),
        }
    }

    /// Calculate cache efficiency score (0.0 to 1.0)
    #[allow(dead_code)] // Performance metrics - efficiency scoring API for performance analysis
    pub fn efficiency_score(&self) -> f64 {
        if self.total_operations == 0 {
            return 0.0;
        }

        // Weight hit rate (70%) and access speed (30%)
        let hit_rate_score = self.overall_hit_rate;
        let speed_score = if self.avg_access_latency_ns > 0 {
            // Normalize against 1ms reference (lower is better)
            (1_000_000.0 / self.avg_access_latency_ns as f64).min(1.0)
        } else {
            1.0
        };

        hit_rate_score * 0.7 + speed_score * 0.3
    }

    /// Get tier distribution percentages
    #[allow(dead_code)] // Performance metrics - tier distribution analysis API for cache optimization
    pub fn tier_distribution(&self) -> (f64, f64, f64) {
        let total_hits = self.hot_tier.hits + self.warm_tier.hits + self.cold_tier.hits;
        if total_hits == 0 {
            return (0.0, 0.0, 0.0);
        }

        let hot_pct = self.hot_tier.hits as f64 / total_hits as f64 * 100.0;
        let warm_pct = self.warm_tier.hits as f64 / total_hits as f64 * 100.0;
        let cold_pct = self.cold_tier.hits as f64 / total_hits as f64 * 100.0;

        (hot_pct, warm_pct, cold_pct)
    }

    /// Check if cache performance is healthy
    #[allow(dead_code)] // Performance metrics - health check API for system monitoring
    pub fn is_healthy(&self) -> bool {
        // Consider cache healthy if:
        // 1. Hit rate > 80%
        // 2. Average latency < 10ms
        // 3. Hot tier getting reasonable traffic (> 10%)

        let (hot_pct, _, _) = self.tier_distribution();

        self.overall_hit_rate > 0.8
            && self.avg_access_latency_ns < 10_000_000 // 10ms
            && hot_pct > 10.0
    }
}
