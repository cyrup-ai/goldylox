//! Metrics collection for performance monitoring
//!
//! This module handles the collection and aggregation of performance metrics
//! from cache operations for analysis and monitoring.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use super::types::{AtomicMetrics, PerformanceSnapshot};

/// Performance metrics collector
#[derive(Debug)]
pub struct MetricsCollector {
    /// Atomic metrics for thread-safe collection
    metrics: AtomicMetrics,
    /// Collection interval
    collection_interval: Duration,
    /// Last collection timestamp
    last_collection: AtomicU64, // Store as nanos since epoch
    /// Collection status
    collection_active: AtomicBool,
}

impl MetricsCollector {
    /// Create new metrics collector
    pub fn new() -> Self {
        Self {
            metrics: AtomicMetrics::default(),
            collection_interval: Duration::from_secs(1),
            last_collection: AtomicU64::new(0),
            collection_active: AtomicBool::new(true),
        }
    }

    /// Create metrics collector with custom interval
    pub fn with_interval(interval: Duration) -> Self {
        let mut collector = Self::new();
        collector.collection_interval = interval;
        collector
    }

    /// Collect current performance snapshot
    #[inline]
    pub fn collect_current_snapshot(&self) -> PerformanceSnapshot {
        if !self.collection_active.load(Ordering::Relaxed) {
            return self.create_snapshot();
        }

        let now = Instant::now();
        let now_nanos = now.elapsed().as_nanos() as u64;
        let last_collection_nanos = self.last_collection.load(Ordering::Relaxed);

        if last_collection_nanos == 0
            || now_nanos.saturating_sub(last_collection_nanos)
                >= self.collection_interval.as_nanos() as u64
        {
            self.last_collection.store(now_nanos, Ordering::Relaxed);
        }

        self.create_snapshot()
    }

    /// Force immediate collection of metrics
    pub fn force_collection(&self) -> PerformanceSnapshot {
        let now_nanos = Instant::now().elapsed().as_nanos() as u64;
        self.last_collection.store(now_nanos, Ordering::Relaxed);
        self.create_snapshot()
    }

    /// Record cache hit
    #[inline(always)]
    pub fn record_hit(&self, access_time_ns: u64) {
        self.metrics.total_hits.fetch_add(1, Ordering::Relaxed);
        self.metrics
            .total_access_time_ns
            .fetch_add(access_time_ns, Ordering::Relaxed);
    }

    /// Record cache miss
    #[inline(always)]
    pub fn record_miss(&self, access_time_ns: u64) {
        self.metrics.total_misses.fetch_add(1, Ordering::Relaxed);
        self.metrics
            .total_access_time_ns
            .fetch_add(access_time_ns, Ordering::Relaxed);
    }

    /// Record memory usage sample
    #[inline]
    pub fn record_memory_usage(&self, memory_bytes: u64) {
        self.metrics
            .current_memory_bytes
            .store(memory_bytes, Ordering::Relaxed);
    }

    /// Enable or disable collection
    pub fn set_collection_active(&self, active: bool) {
        self.metrics
            .collection_active
            .store(active, Ordering::Relaxed);
        self.collection_active.store(active, Ordering::Relaxed);
    }

    /// Check if collection is active
    pub fn is_collection_active(&self) -> bool {
        self.collection_active.load(Ordering::Relaxed)
    }

    /// Reset all metrics
    pub fn reset_metrics(&self) {
        self.metrics.total_hits.store(0, Ordering::Relaxed);
        self.metrics.total_misses.store(0, Ordering::Relaxed);
        self.metrics
            .total_access_time_ns
            .store(0, Ordering::Relaxed);
        self.metrics
            .current_memory_bytes
            .store(0, Ordering::Relaxed);
        self.last_collection.store(0, Ordering::Relaxed);
    }

    /// Get current metrics buffer statistics
    pub fn get_buffer_stats(&self) -> MetricsBufferStats {
        let hit_count = self.metrics.total_hits.load(Ordering::Relaxed);
        let miss_count = self.metrics.total_misses.load(Ordering::Relaxed);
        let total_ops = hit_count + miss_count;

        MetricsBufferStats {
            hit_count,
            miss_count,
            total_operations: total_ops,
            total_access_time: self.metrics.total_access_time_ns.load(Ordering::Relaxed),
            current_sample_index: 0, // Simplified
        }
    }

    /// Create performance snapshot from current metrics
    fn create_snapshot(&self) -> PerformanceSnapshot {
        let hit_count = self.metrics.total_hits.load(Ordering::Relaxed);
        let miss_count = self.metrics.total_misses.load(Ordering::Relaxed);
        let total_ops = hit_count + miss_count;

        let hit_rate = if total_ops > 0 {
            hit_count as f64 / total_ops as f64
        } else {
            0.0
        };

        let avg_latency = if total_ops > 0 {
            self.metrics.total_access_time_ns.load(Ordering::Relaxed) / total_ops
        } else {
            0
        };

        let memory_utilization = self.calculate_memory_utilization();
        let throughput = self.calculate_throughput(total_ops);
        let efficiency_score = self.calculate_efficiency_score(hit_rate, avg_latency);

        PerformanceSnapshot {
            timestamp: Instant::now(),
            hit_rate,
            avg_latency_ns: avg_latency,
            memory_utilization,
            throughput_ops_per_sec: throughput,
            efficiency_score,
        }
    }

    /// Calculate memory utilization
    fn calculate_memory_utilization(&self) -> f64 {
        let current_memory = self.metrics.current_memory_bytes.load(Ordering::Relaxed);
        // Assume 1GB total memory for utilization calculation
        (current_memory as f64) / (1024.0 * 1024.0 * 1024.0)
    }

    /// Calculate throughput based on operations and time
    fn calculate_throughput(&self, total_ops: u64) -> u32 {
        // Simplified throughput calculation
        if self.collection_interval.as_secs() > 0 {
            (total_ops / self.collection_interval.as_secs()) as u32
        } else {
            total_ops as u32
        }
    }

    /// Calculate efficiency score using adaptive weighting based on system characteristics
    fn calculate_efficiency_score(&self, hit_rate: f64, avg_latency: u64) -> f64 {
        // Dynamic latency normalization based on actual system performance
        let baseline_latency_ns = self.calculate_baseline_latency();
        let latency_factor = if avg_latency > 0 && baseline_latency_ns > 0.0 {
            let latency_ratio = avg_latency as f64 / baseline_latency_ns;
            // Exponential decay for latency penalty - worse performance has exponentially higher penalty
            (-0.5 * latency_ratio.ln()).exp().min(1.0).max(0.0)
        } else {
            1.0
        };

        // Adaptive weight calculation based on cache tier performance characteristics
        let (hit_weight, latency_weight) = self.calculate_adaptive_weights(hit_rate, avg_latency);

        hit_rate * hit_weight + latency_factor * latency_weight
    }

    /// Calculate baseline latency from historical performance data
    fn calculate_baseline_latency(&self) -> f64 {
        let total_hits = self.metrics.total_hits.load(Ordering::Relaxed);
        let total_misses = self.metrics.total_misses.load(Ordering::Relaxed);
        let total_ops = total_hits + total_misses;
        let total_time = self.metrics.total_access_time_ns.load(Ordering::Relaxed);

        if total_ops > 100 {
            // Need sufficient samples for reliable baseline
            let avg_latency = total_time as f64 / total_ops as f64;
            // Use 95th percentile of historical latency as baseline (estimated as 1.5x average)
            avg_latency * 1.5
        } else {
            // Conservative fallback for cold start - assume 100μs baseline
            100_000.0
        }
    }

    /// Calculate adaptive weights based on current performance profile
    fn calculate_adaptive_weights(&self, hit_rate: f64, avg_latency: u64) -> (f64, f64) {
        // For high-latency operations, prioritize latency optimization
        if avg_latency > 1_000_000 {
            // > 1ms
            (0.4, 0.6) // Prioritize latency reduction
        }
        // For low hit rates, prioritize cache effectiveness
        else if hit_rate < 0.5 {
            (0.8, 0.2) // Prioritize hit rate improvement
        }
        // Balanced approach for well-performing cache
        else {
            (0.6, 0.4) // Balanced optimization
        }
    }
}

/// Statistics about the metrics buffer
#[derive(Debug, Clone)]
pub struct MetricsBufferStats {
    pub hit_count: u64,
    pub miss_count: u64,
    pub total_operations: u64,
    pub total_access_time: u64,
    pub current_sample_index: u32,
}

impl MetricsBufferStats {
    /// Calculate hit rate from buffer stats
    pub fn hit_rate(&self) -> f64 {
        if self.total_operations > 0 {
            self.hit_count as f64 / self.total_operations as f64
        } else {
            0.0
        }
    }

    /// Calculate average access time from buffer stats
    pub fn avg_access_time(&self) -> u64 {
        if self.total_operations > 0 {
            self.total_access_time / self.total_operations
        } else {
            0
        }
    }
}
