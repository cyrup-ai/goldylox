//! Operation results and utility types for cache synchronization
//!
//! Provides result types, statistics structures, and timestamp utilities
//! for cache operation tracking and performance analysis.

/// Cache operation result with detailed timing
#[derive(Debug)]
pub struct OperationResult<T> {
    pub result: T,
    pub latency_ns: u64,
    pub generation: u64,
}

impl<T> OperationResult<T> {
    /// Create new operation result
    pub fn new(result: T, latency_ns: u64, generation: u64) -> Self {
        Self {
            result,
            latency_ns,
            generation,
        }
    }

    /// Check if operation was fast (< 1μs)
    pub fn is_fast(&self) -> bool {
        self.latency_ns < 1_000
    }

    /// Check if operation was slow (> 10μs)
    pub fn is_slow(&self) -> bool {
        self.latency_ns > 10_000
    }
}

/// Tier statistics snapshot
#[derive(Debug, Clone, Copy)]
pub struct TierStatistics {
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
    pub entry_count: usize,
    pub memory_usage: u64,
    pub peak_memory: u64,
    pub avg_access_latency_ns: u64,
    pub error_count: u64,
}

impl TierStatistics {
    /// Get cache efficiency score (0.0 to 1.0)
    pub fn efficiency_score(&self) -> f64 {
        let hit_rate_weight = 0.6;
        let latency_weight = 0.3;
        let error_weight = 0.1;

        let hit_score = self.hit_rate;
        let latency_score = if self.avg_access_latency_ns > 0 {
            1.0 / (1.0 + self.avg_access_latency_ns as f64 / 1000.0) // Normalize to microseconds
        } else {
            1.0
        };
        let error_score = if self.hits + self.misses > 0 {
            1.0 - (self.error_count as f64 / (self.hits + self.misses) as f64)
        } else {
            1.0
        };

        hit_rate_weight * hit_score + latency_weight * latency_score + error_weight * error_score
    }

    /// Check if performance is acceptable
    pub fn is_performing_well(&self) -> bool {
        self.hit_rate > 0.8 && self.avg_access_latency_ns < 10_000 && self.error_count == 0
    }

    /// Get memory utilization ratio
    pub fn memory_utilization(&self) -> f64 {
        if self.peak_memory > 0 {
            self.memory_usage as f64 / self.peak_memory as f64
        } else {
            0.0
        }
    }
}

/// Timestamp conversion utilities
pub mod timestamp {
    use std::time::Instant;

    /// Convert Instant to nanoseconds timestamp
    #[inline(always)]
    pub fn instant_to_nanos(instant: Instant) -> u64 {
        use std::sync::OnceLock;
        static START_TIME: OnceLock<Instant> = OnceLock::new();

        let start = START_TIME.get_or_init(|| Instant::now());
        instant.duration_since(*start).as_nanos() as u64
    }

    /// Get current timestamp in nanoseconds
    #[inline(always)]
    pub fn now_nanos() -> u64 {
        instant_to_nanos(Instant::now())
    }
}
