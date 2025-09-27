//! Multi-tier statistics aggregation and analysis
//!
//! This module provides comprehensive statistics aggregation
//! across multiple cache tiers with performance analysis.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

use crossbeam_utils::CachePadded;

use super::tier_stats::TierStatistics;

/// Error classification for detailed tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)] // Complete error type enumeration
pub enum ErrorType {
    /// Cache operation timeout
    Timeout,
    /// Memory allocation failure
    OutOfMemory,
    /// Memory allocation failure (alias for compatibility)
    MemoryAllocationFailure,
    /// Serialization/deserialization error
    Serialization,
    /// Cache coherence protocol violation
    CoherenceViolation,
    /// SIMD operation failure
    SimdFailure,
    /// Tier transition failure
    TierTransition,
    /// Background worker error
    WorkerFailure,
    /// Corrupted data detected
    CorruptedData,
    /// Disk I/O operation failed
    DiskIOError,
    /// Generic operation error
    GenericError,
}

/// Time-windowed error rate tracker with atomic precision
#[derive(Debug)]
#[allow(dead_code)] // Complete error rate tracking structure
pub struct ErrorRateTracker {
    /// Error counts per type (atomic for lock-free updates)
    error_counts: [CachePadded<AtomicU64>; 8], // One for each ErrorType variant
    /// Total operations count
    total_operations: CachePadded<AtomicU64>,
    /// Circular buffer for time-windowed error rates
    error_window: [AtomicU64; 60], // 60-second sliding window
    /// Current window position
    window_position: CachePadded<AtomicUsize>,
    /// Window start timestamp for proper rotation
    window_start: CachePadded<AtomicU64>,
    /// Total error count in current window
    window_error_count: CachePadded<AtomicU64>,
    /// Total operations in current window
    window_ops_count: CachePadded<AtomicU64>,
}

#[allow(dead_code)] // Complete error rate tracker implementation
impl ErrorRateTracker {
    /// Create new error rate tracker
    pub fn new() -> Self {
        let now_nanos = Self::timestamp_nanos();

        Self {
            error_counts: [
                CachePadded::new(AtomicU64::new(0)), // Timeout
                CachePadded::new(AtomicU64::new(0)), // OutOfMemory
                CachePadded::new(AtomicU64::new(0)), // Serialization
                CachePadded::new(AtomicU64::new(0)), // CoherenceViolation
                CachePadded::new(AtomicU64::new(0)), // SimdFailure
                CachePadded::new(AtomicU64::new(0)), // TierTransition
                CachePadded::new(AtomicU64::new(0)), // WorkerFailure
                CachePadded::new(AtomicU64::new(0)), // GenericError
            ],
            total_operations: CachePadded::new(AtomicU64::new(0)),
            error_window: [const { AtomicU64::new(0) }; 60],
            window_position: CachePadded::new(AtomicUsize::new(0)),
            window_start: CachePadded::new(AtomicU64::new(now_nanos)),
            window_error_count: CachePadded::new(AtomicU64::new(0)),
            window_ops_count: CachePadded::new(AtomicU64::new(0)),
        }
    }

    /// Record an error atomically with proper window management
    #[inline(always)]
    pub fn record_error(&self, error_type: ErrorType) {
        // Update total error count for this type
        let error_index = error_type as usize;
        self.error_counts[error_index].fetch_add(1, Ordering::Relaxed);

        // Update operation count
        self.total_operations.fetch_add(1, Ordering::Relaxed);

        // Update time-windowed counters
        self.update_window();
        self.window_error_count.fetch_add(1, Ordering::Relaxed);
        self.window_ops_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Record successful operation for accurate rate calculation
    #[inline(always)]
    pub fn record_success(&self) {
        self.total_operations.fetch_add(1, Ordering::Relaxed);
        self.update_window();
        self.window_ops_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get overall error rate (lifetime)
    #[inline(always)]
    pub fn overall_error_rate(&self) -> f64 {
        let total_ops = self.total_operations.load(Ordering::Relaxed);
        if total_ops == 0 {
            return 0.0;
        }

        let total_errors: u64 = self
            .error_counts
            .iter()
            .map(|counter| counter.load(Ordering::Relaxed))
            .sum();

        total_errors as f64 / total_ops as f64
    }

    /// Get error rate for specific error type
    #[inline(always)]
    pub fn error_rate_by_type(&self, error_type: ErrorType) -> f64 {
        let total_ops = self.total_operations.load(Ordering::Relaxed);
        if total_ops == 0 {
            return 0.0;
        }

        let error_count = self.error_counts[error_type as usize].load(Ordering::Relaxed);
        error_count as f64 / total_ops as f64
    }

    /// Get current windowed error rate (last 60 seconds)
    #[inline(always)]
    pub fn windowed_error_rate(&self) -> f64 {
        self.update_window(); // Ensure window is current

        let window_ops = self.window_ops_count.load(Ordering::Relaxed);
        if window_ops == 0 {
            return 0.0;
        }

        let window_errors = self.window_error_count.load(Ordering::Relaxed);
        window_errors as f64 / window_ops as f64
    }

    /// Get error distribution across types
    pub fn error_distribution(&self) -> [f64; 8] {
        let total_errors: u64 = self
            .error_counts
            .iter()
            .map(|counter| counter.load(Ordering::Relaxed))
            .sum();

        if total_errors == 0 {
            return [0.0; 8];
        }

        let mut distribution = [0.0; 8];
        for (i, counter) in self.error_counts.iter().enumerate() {
            let count = counter.load(Ordering::Relaxed);
            distribution[i] = count as f64 / total_errors as f64;
        }
        distribution
    }

    /// Check if error rate exceeds threshold
    #[inline(always)]
    pub fn exceeds_threshold(&self, threshold: f64) -> bool {
        self.windowed_error_rate() > threshold
    }

    /// Get statistical summary
    pub fn statistical_summary(&self) -> ErrorRateStatistics {
        ErrorRateStatistics {
            overall_rate: self.overall_error_rate(),
            windowed_rate: self.windowed_error_rate(),
            total_operations: self.total_operations.load(Ordering::Relaxed),
            total_errors: self
                .error_counts
                .iter()
                .map(|c| c.load(Ordering::Relaxed))
                .sum(),
            error_distribution: self.error_distribution(),
        }
    }

    /// Update time window position and reset expired counters
    #[inline(always)]
    fn update_window(&self) {
        let now_nanos = Self::timestamp_nanos();
        let window_start = self.window_start.load(Ordering::Relaxed);

        // Check if we need to advance the window (every second)
        if now_nanos.saturating_sub(window_start) >= 1_000_000_000 {
            let current_pos = self.window_position.load(Ordering::Relaxed);
            let new_pos = (current_pos + 1) % 60;

            // Reset the new position's counters
            self.error_window[new_pos].store(0, Ordering::Relaxed);

            // Update window position and start time
            self.window_position.store(new_pos, Ordering::Relaxed);
            self.window_start.store(now_nanos, Ordering::Relaxed);

            // Recalculate window totals
            self.recalculate_window_totals();
        }
    }

    /// Recalculate window totals after position advance
    fn recalculate_window_totals(&self) {
        let total_errors: u64 = self
            .error_window
            .iter()
            .map(|counter| counter.load(Ordering::Relaxed))
            .sum();

        self.window_error_count
            .store(total_errors, Ordering::Relaxed);
    }

    /// Get nanosecond timestamp
    #[inline(always)]
    fn timestamp_nanos() -> u64 {
        use std::time::SystemTime;
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_nanos() as u64
    }
}

impl Default for ErrorRateTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistical summary of error rates
#[derive(Debug, Clone)]
#[allow(dead_code)] // Complete error rate statistics structure
pub struct ErrorRateStatistics {
    /// Overall error rate (lifetime)
    pub overall_rate: f64,
    /// Current windowed error rate (last 60 seconds)
    pub windowed_rate: f64,
    /// Total operations performed
    pub total_operations: u64,
    /// Total errors encountered
    pub total_errors: u64,
    /// Error distribution by type
    pub error_distribution: [f64; 8],
}

/// Statistics aggregator for multiple tiers with sophisticated error tracking
#[derive(Debug)]
#[allow(dead_code)] // Complete multi-tier statistics structure
pub struct MultiTierStatistics {
    /// Hot tier statistics
    pub hot_tier: TierStatistics,
    /// Warm tier statistics
    pub warm_tier: TierStatistics,
    /// Cold tier statistics
    pub cold_tier: TierStatistics,
    /// Per-tier error rate trackers
    pub error_trackers: [ErrorRateTracker; 3], // Hot, Warm, Cold
    /// Global error rate tracker
    pub global_error_tracker: ErrorRateTracker,
}

#[allow(dead_code)] // Complete multi-tier statistics implementation
impl MultiTierStatistics {
    /// Create new multi-tier statistics with error tracking
    pub fn new(
        hot_tier: TierStatistics,
        warm_tier: TierStatistics,
        cold_tier: TierStatistics,
    ) -> Self {
        Self {
            hot_tier,
            warm_tier,
            cold_tier,
            error_trackers: [
                ErrorRateTracker::new(), // Hot tier
                ErrorRateTracker::new(), // Warm tier
                ErrorRateTracker::new(), // Cold tier
            ],
            global_error_tracker: ErrorRateTracker::new(),
        }
    }

    /// Record error for specific tier
    #[inline(always)]
    pub fn record_tier_error(&self, tier_index: usize, error_type: ErrorType) {
        if tier_index < 3 {
            self.error_trackers[tier_index].record_error(error_type);
            self.global_error_tracker.record_error(error_type);
        }
    }

    /// Record successful operation for specific tier
    #[inline(always)]
    pub fn record_tier_success(&self, tier_index: usize) {
        if tier_index < 3 {
            self.error_trackers[tier_index].record_success();
            self.global_error_tracker.record_success();
        }
    }

    /// Get error rate for specific tier
    #[inline(always)]
    pub fn tier_error_rate(&self, tier_index: usize) -> f64 {
        if tier_index < 3 {
            self.error_trackers[tier_index].windowed_error_rate()
        } else {
            0.0
        }
    }

    /// Get global error rate across all tiers
    #[inline(always)]
    pub fn global_error_rate(&self) -> f64 {
        self.global_error_tracker.windowed_error_rate()
    }

    /// Get error statistics for all tiers
    pub fn error_statistics(&self) -> [ErrorRateStatistics; 3] {
        [
            self.error_trackers[0].statistical_summary(),
            self.error_trackers[1].statistical_summary(),
            self.error_trackers[2].statistical_summary(),
        ]
    }

    /// Check if any tier exceeds error threshold
    pub fn exceeds_error_threshold(&self, threshold: f64) -> Option<usize> {
        for (index, tracker) in self.error_trackers.iter().enumerate() {
            if tracker.exceeds_threshold(threshold) {
                return Some(index);
            }
        }
        None
    }

    /// Get most error-prone tier
    pub fn most_error_prone_tier(&self) -> (usize, f64) {
        let mut max_error_rate = 0.0;
        let mut max_tier = 0;

        for (index, tracker) in self.error_trackers.iter().enumerate() {
            let error_rate = tracker.windowed_error_rate();
            if error_rate > max_error_rate {
                max_error_rate = error_rate;
                max_tier = index;
            }
        }

        (max_tier, max_error_rate)
    }

    /// Get aggregate statistics across all tiers
    pub fn aggregate(&self) -> TierStatistics {
        let total_entries =
            self.hot_tier.entry_count + self.warm_tier.entry_count + self.cold_tier.entry_count;

        let total_memory =
            self.hot_tier.memory_usage + self.warm_tier.memory_usage + self.cold_tier.memory_usage;

        let _weighted_hit_rate = if total_entries > 0 {
            (self.hot_tier.hit_rate * self.hot_tier.entry_count as f64
                + self.warm_tier.hit_rate * self.warm_tier.entry_count as f64
                + self.cold_tier.hit_rate * self.cold_tier.entry_count as f64)
                / total_entries as f64
        } else {
            0.0
        };

        let weighted_avg_time = if total_entries > 0 {
            (self.hot_tier.avg_access_time_ns * self.hot_tier.entry_count as u64
                + self.warm_tier.avg_access_time_ns * self.warm_tier.entry_count as u64
                + self.cold_tier.avg_access_time_ns * self.cold_tier.entry_count as u64)
                / total_entries as u64
        } else {
            0
        };

        let total_ops_per_second = self.hot_tier.ops_per_second
            + self.warm_tier.ops_per_second
            + self.cold_tier.ops_per_second;

        let _weighted_error_rate = if total_entries > 0 {
            (self.hot_tier.error_rate * self.hot_tier.entry_count as f64
                + self.warm_tier.error_rate * self.warm_tier.entry_count as f64
                + self.cold_tier.error_rate * self.cold_tier.entry_count as f64)
                / total_entries as f64
        } else {
            0.0
        };

        // Calculate total hits and misses from individual tier statistics
        let total_hits = self.hot_tier.hits + self.warm_tier.hits + self.cold_tier.hits;
        let total_misses = self.hot_tier.misses + self.warm_tier.misses + self.cold_tier.misses;
        let total_errors =
            self.hot_tier.error_count + self.warm_tier.error_count + self.cold_tier.error_count;
        let max_peak_memory = self
            .hot_tier
            .peak_memory
            .max(self.warm_tier.peak_memory)
            .max(self.cold_tier.peak_memory);
        let total_size_bytes = self.hot_tier.total_size_bytes
            + self.warm_tier.total_size_bytes
            + self.cold_tier.total_size_bytes;

        TierStatistics::new(
            total_hits,           // hits: u64 - sum of all tier hits
            total_misses,         // misses: u64 - sum of all tier misses
            total_entries,        // entry_count: usize - sum of all tier entries
            total_memory,         // memory_usage: usize - sum of all tier memory
            max_peak_memory,      // peak_memory: u64 - max across tiers
            total_size_bytes,     // total_size_bytes: u64 - sum across tiers
            weighted_avg_time,    // avg_access_time_ns: u64 - weighted average
            total_ops_per_second, // ops_per_second: f64 - sum across tiers
            total_errors,         // error_count: u64 - sum of all tier errors
        )
    }

    /// Get tier with best hit rate
    pub fn best_performing_tier(&self) -> (&str, &TierStatistics) {
        let tiers = [
            ("hot", &self.hot_tier),
            ("warm", &self.warm_tier),
            ("cold", &self.cold_tier),
        ];

        tiers
            .iter()
            .max_by(|(_, a), (_, b)| {
                a.hit_rate
                    .partial_cmp(&b.hit_rate)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(name, stats)| (*name, *stats))
            .unwrap_or(("none", &self.hot_tier))
    }

    /// Get tier with worst performance for optimization
    pub fn worst_performing_tier(&self) -> (&str, &TierStatistics) {
        let tiers = [
            ("hot", &self.hot_tier),
            ("warm", &self.warm_tier),
            ("cold", &self.cold_tier),
        ];

        tiers
            .iter()
            .min_by(|(_, a), (_, b)| {
                a.hit_rate
                    .partial_cmp(&b.hit_rate)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(name, stats)| (*name, *stats))
            .unwrap_or(("none", &self.hot_tier))
    }

    /// Get memory distribution across tiers
    pub fn memory_distribution(&self) -> (f64, f64, f64) {
        let total_memory = self.aggregate().memory_usage as f64;

        if total_memory > 0.0 {
            let hot_ratio = self.hot_tier.memory_usage as f64 / total_memory;
            let warm_ratio = self.warm_tier.memory_usage as f64 / total_memory;
            let cold_ratio = self.cold_tier.memory_usage as f64 / total_memory;
            (hot_ratio, warm_ratio, cold_ratio)
        } else {
            (0.0, 0.0, 0.0)
        }
    }

    /// Get entry distribution across tiers
    pub fn entry_distribution(&self) -> (f64, f64, f64) {
        let total_entries = self.aggregate().entry_count as f64;

        if total_entries > 0.0 {
            let hot_ratio = self.hot_tier.entry_count as f64 / total_entries;
            let warm_ratio = self.warm_tier.entry_count as f64 / total_entries;
            let cold_ratio = self.cold_tier.entry_count as f64 / total_entries;
            (hot_ratio, warm_ratio, cold_ratio)
        } else {
            (0.0, 0.0, 0.0)
        }
    }

    /// Check if tier balance is optimal
    pub fn is_balanced(&self) -> bool {
        let (hot_entries, warm_entries, cold_entries) = self.entry_distribution();

        // Optimal distribution: hot ~30%, warm ~50%, cold ~20%
        let hot_optimal = (0.2..=0.4).contains(&hot_entries);
        let warm_optimal = (0.4..=0.6).contains(&warm_entries);
        let cold_optimal = (0.1..=0.3).contains(&cold_entries);

        hot_optimal && warm_optimal && cold_optimal
    }

    /// Get overall system health score (0.0 to 1.0) with real error rate consideration
    pub fn health_score(&self) -> f64 {
        let aggregate_stats = self.aggregate();
        let balance_score = if self.is_balanced() { 1.0 } else { 0.7 };

        // Factor in real error rates (lower error rates = higher health)
        let global_error_rate = self.global_error_rate();
        let error_health_score = if global_error_rate <= 0.001 {
            1.0 // Excellent error rate (< 0.1%)
        } else if global_error_rate <= 0.005 {
            0.9 // Good error rate (< 0.5%)
        } else if global_error_rate <= 0.01 {
            0.8 // Acceptable error rate (< 1%)
        } else if global_error_rate <= 0.05 {
            0.6 // Poor error rate (< 5%)
        } else {
            0.3 // Critical error rate (>= 5%)
        };

        // Weighted average: efficiency (40%), balance (30%), error health (30%)
        aggregate_stats.efficiency_score() * 0.4 + balance_score * 0.3 + error_health_score * 0.3
    }

    /// Get system reliability score based on error patterns
    pub fn reliability_score(&self) -> f64 {
        let global_stats = self.global_error_tracker.statistical_summary();

        // Analyze error distribution for reliability assessment
        let error_distribution = global_stats.error_distribution;

        // Critical error types have higher weight in reliability calculation
        let critical_error_weight = error_distribution[ErrorType::OutOfMemory as usize] * 3.0
            + error_distribution[ErrorType::CoherenceViolation as usize] * 2.5
            + error_distribution[ErrorType::SimdFailure as usize] * 2.0;

        let reliability = 1.0 - (global_stats.windowed_rate + critical_error_weight).min(1.0);
        reliability.max(0.0)
    }
}
