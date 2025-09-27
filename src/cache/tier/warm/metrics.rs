//! Performance monitoring metrics for warm tier cache
//!
//! This module contains atomic metrics tracking structures for monitoring
//! cache performance including operation latencies, throughput measurements,
//! resource utilization, and cache effectiveness.
#![allow(unused)] // Warm tier metrics - comprehensive API for performance monitoring

use std::sync::atomic::{AtomicU64, Ordering};

use crossbeam_utils::{CachePadded, atomic::AtomicCell};

/// Performance monitoring metrics
#[derive(Debug)]
pub struct PerformanceMetrics {
    /// Operation latencies in nanoseconds
    pub operation_latencies: LatencyMetrics,
    /// Throughput measurements
    pub throughput_metrics: ThroughputMetrics,
    /// Resource utilization
    pub resource_utilization: ResourceMetrics,
    /// Cache effectiveness
    pub cache_effectiveness: EffectivenessMetrics,
}

/// Latency metrics for different operations
#[derive(Debug)]
pub struct LatencyMetrics {
    /// Get operation latency
    pub get_latency_ns: CachePadded<AtomicU64>,
    /// Put operation latency
    pub put_latency_ns: CachePadded<AtomicU64>,
    /// Remove operation latency
    pub remove_latency_ns: CachePadded<AtomicU64>,
    /// Eviction operation latency
    pub eviction_latency_ns: CachePadded<AtomicU64>,
    /// Pattern analysis latency
    pub analysis_latency_ns: CachePadded<AtomicU64>,
    /// 99th percentile latencies
    pub p99_latencies: P99LatencyMetrics,
}

/// 99th percentile latency tracking
#[derive(Debug)]
pub struct P99LatencyMetrics {
    /// Get operation P99 latency
    pub get_p99_ns: CachePadded<AtomicU64>,
    /// Put operation P99 latency
    pub put_p99_ns: CachePadded<AtomicU64>,
    /// Remove operation P99 latency
    pub remove_p99_ns: CachePadded<AtomicU64>,
    /// Eviction operation P99 latency
    pub eviction_p99_ns: CachePadded<AtomicU64>,
}

/// Throughput measurement metrics
#[derive(Debug)]
pub struct ThroughputMetrics {
    /// Operations per second
    pub ops_per_second: AtomicCell<f64>,
    /// Bytes processed per second
    pub bytes_per_second: AtomicCell<f64>,
    /// Peak throughput achieved
    pub peak_ops_per_second: AtomicCell<f64>,
    /// Sustained throughput average
    pub sustained_ops_per_second: AtomicCell<f64>,
    /// Throughput efficiency ratio
    pub efficiency_ratio: AtomicCell<f64>,
}

/// Resource utilization metrics
#[derive(Debug)]
pub struct ResourceMetrics {
    /// CPU utilization percentage
    pub cpu_utilization: AtomicCell<f64>,
    /// Memory utilization percentage
    pub memory_utilization: AtomicCell<f64>,
    /// Cache line utilization
    pub cache_line_utilization: AtomicCell<f64>,
    /// Network bandwidth utilization
    pub network_utilization: AtomicCell<f64>,
    /// Disk I/O utilization
    pub disk_utilization: AtomicCell<f64>,
}

/// Cache effectiveness metrics
#[derive(Debug)]
pub struct EffectivenessMetrics {
    /// Hit rate percentage
    pub hit_rate: AtomicCell<f64>,
    /// Miss rate percentage
    pub miss_rate: AtomicCell<f64>,
    /// Eviction accuracy
    pub eviction_accuracy: AtomicCell<f64>,
    /// Prefetch success rate
    pub prefetch_success_rate: AtomicCell<f64>,
    /// Working set capture ratio
    pub working_set_ratio: AtomicCell<f64>,
}

/// Operation types for performance tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationType {
    Get,
    Put,
    Remove,
    Eviction,
    Analysis,
}

// TierStatsSnapshot moved to canonical location: crate::cache::tier::warm::monitoring::types::TierStatsSnapshot

/// Frequency trend analysis for access patterns
pub enum FrequencyTrend {
    Increasing,
    Decreasing,
    Stable,
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl PerformanceMetrics {
    /// Create new performance metrics with atomic initialization
    #[inline]
    pub fn new() -> Self {
        Self {
            operation_latencies: LatencyMetrics::new(),
            throughput_metrics: ThroughputMetrics::new(),
            resource_utilization: ResourceMetrics::new(),
            cache_effectiveness: EffectivenessMetrics::new(),
        }
    }

    /// Record operation latency
    #[inline]
    pub fn record_operation_latency(&self, operation: OperationType, latency_ns: u64) {
        match operation {
            OperationType::Get => {
                self.operation_latencies
                    .get_latency_ns
                    .store(latency_ns, Ordering::Relaxed);
                self.update_p99_latency(
                    &self.operation_latencies.p99_latencies.get_p99_ns,
                    latency_ns,
                );
            }
            OperationType::Put => {
                self.operation_latencies
                    .put_latency_ns
                    .store(latency_ns, Ordering::Relaxed);
                self.update_p99_latency(
                    &self.operation_latencies.p99_latencies.put_p99_ns,
                    latency_ns,
                );
            }
            OperationType::Remove => {
                self.operation_latencies
                    .remove_latency_ns
                    .store(latency_ns, Ordering::Relaxed);
                self.update_p99_latency(
                    &self.operation_latencies.p99_latencies.remove_p99_ns,
                    latency_ns,
                );
            }
            OperationType::Eviction => {
                self.operation_latencies
                    .eviction_latency_ns
                    .store(latency_ns, Ordering::Relaxed);
                self.update_p99_latency(
                    &self.operation_latencies.p99_latencies.eviction_p99_ns,
                    latency_ns,
                );
            }
            OperationType::Analysis => {
                self.operation_latencies
                    .analysis_latency_ns
                    .store(latency_ns, Ordering::Relaxed);
            }
        }
    }

    /// Update P99 latency using exponential moving average
    #[inline]
    fn update_p99_latency(&self, p99_atomic: &CachePadded<AtomicU64>, latency_ns: u64) {
        let current_p99 = p99_atomic.load(Ordering::Relaxed);
        let new_p99 = if current_p99 == 0 {
            latency_ns
        } else {
            // Simple P99 estimation using exponential moving average with higher weight for outliers
            let weight = if latency_ns > current_p99 { 0.1 } else { 0.01 };
            ((current_p99 as f64) * (1.0 - weight) + (latency_ns as f64) * weight) as u64
        };
        p99_atomic.store(new_p99, Ordering::Relaxed);
    }

    /// Update throughput metrics
    #[inline]
    pub fn update_throughput(&self, ops_delta: u64, bytes_delta: u64, time_delta_sec: f64) {
        if time_delta_sec > 0.0 {
            let ops_rate = ops_delta as f64 / time_delta_sec;
            let bytes_rate = bytes_delta as f64 / time_delta_sec;

            // Update current rates
            self.throughput_metrics.ops_per_second.store(ops_rate);
            self.throughput_metrics.bytes_per_second.store(bytes_rate);

            // Update peak if necessary
            let current_peak = self.throughput_metrics.peak_ops_per_second.load();
            if ops_rate > current_peak {
                self.throughput_metrics.peak_ops_per_second.store(ops_rate);
            }

            // Update sustained average (exponential moving average)
            let current_sustained = self.throughput_metrics.sustained_ops_per_second.load();
            let new_sustained = if current_sustained == 0.0 {
                ops_rate
            } else {
                current_sustained * 0.95 + ops_rate * 0.05
            };
            self.throughput_metrics
                .sustained_ops_per_second
                .store(new_sustained);

            // Calculate efficiency ratio
            if current_peak > 0.0 {
                let efficiency = ops_rate / current_peak;
                self.throughput_metrics.efficiency_ratio.store(efficiency);
            }
        }
    }
}

impl Default for LatencyMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl LatencyMetrics {
    #[inline]
    pub fn new() -> Self {
        Self {
            get_latency_ns: CachePadded::new(AtomicU64::new(0)),
            put_latency_ns: CachePadded::new(AtomicU64::new(0)),
            remove_latency_ns: CachePadded::new(AtomicU64::new(0)),
            eviction_latency_ns: CachePadded::new(AtomicU64::new(0)),
            analysis_latency_ns: CachePadded::new(AtomicU64::new(0)),
            p99_latencies: P99LatencyMetrics::new(),
        }
    }
}

impl Default for P99LatencyMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl P99LatencyMetrics {
    #[inline]
    pub fn new() -> Self {
        Self {
            get_p99_ns: CachePadded::new(AtomicU64::new(0)),
            put_p99_ns: CachePadded::new(AtomicU64::new(0)),
            remove_p99_ns: CachePadded::new(AtomicU64::new(0)),
            eviction_p99_ns: CachePadded::new(AtomicU64::new(0)),
        }
    }
}

impl Default for ThroughputMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl ThroughputMetrics {
    #[inline]
    pub fn new() -> Self {
        Self {
            ops_per_second: AtomicCell::new(0.0),
            bytes_per_second: AtomicCell::new(0.0),
            peak_ops_per_second: AtomicCell::new(0.0),
            sustained_ops_per_second: AtomicCell::new(0.0),
            efficiency_ratio: AtomicCell::new(1.0),
        }
    }

    /// Update operations per second (from batch ops version)
    #[inline]
    pub fn update_ops_per_second(&self, ops_per_second: f64) {
        self.ops_per_second.store(ops_per_second);

        // Update peak if this is higher
        let current_peak = self.peak_ops_per_second.load();
        if ops_per_second > current_peak {
            self.peak_ops_per_second.store(ops_per_second);
        }
    }

    /// Update sustained operations per second with exponential moving average
    #[inline]
    pub fn update_sustained_ops_per_second(&self, ops_per_second: f64) {
        let current_sustained = self.sustained_ops_per_second.load();
        let alpha = 0.1; // Smoothing factor
        let new_sustained = if current_sustained == 0.0 {
            ops_per_second
        } else {
            alpha * ops_per_second + (1.0 - alpha) * current_sustained
        };
        self.sustained_ops_per_second.store(new_sustained);
    }

    /// Update efficiency ratio
    #[inline]
    pub fn update_efficiency_ratio(&self, success_rate: f64, avg_latency_ms: f64) {
        let efficiency = success_rate * (1.0 / (avg_latency_ms + 1.0));
        self.efficiency_ratio.store(efficiency);
    }

    /// Calculate throughput from batch metrics (enhanced from SIMD version)
    pub fn calculate_from_batch_metrics(
        total_operations: usize,
        successful_operations: usize,
        duration_ns: u64,
        avg_latency_ns: u64,
    ) -> Self {
        let ops_per_second = if duration_ns > 0 {
            (total_operations as f64) / (duration_ns as f64 / 1_000_000_000.0)
        } else {
            0.0
        };

        let successful_ops_per_second = if duration_ns > 0 {
            (successful_operations as f64) / (duration_ns as f64 / 1_000_000_000.0)
        } else {
            0.0
        };

        let success_rate = if total_operations > 0 {
            successful_operations as f64 / total_operations as f64
        } else {
            0.0
        };

        let _avg_latency_ms = avg_latency_ns as f64 / 1_000_000.0;
        let efficiency_score = success_rate * (1.0 / (avg_latency_ns as f64 + 1.0));

        Self {
            ops_per_second: AtomicCell::new(ops_per_second),
            bytes_per_second: AtomicCell::new(0.0), // Not available from batch metrics
            peak_ops_per_second: AtomicCell::new(ops_per_second),
            sustained_ops_per_second: AtomicCell::new(successful_ops_per_second),
            efficiency_ratio: AtomicCell::new(efficiency_score),
        }
    }

    /// Get current operations per second
    #[inline]
    pub fn operations_per_second(&self) -> f64 {
        self.ops_per_second.load()
    }

    /// Get successful operations per second (alias for sustained)
    #[inline]
    pub fn successful_operations_per_second(&self) -> f64 {
        self.sustained_ops_per_second.load()
    }

    /// Get average latency in milliseconds (estimated from efficiency)
    #[inline]
    pub fn average_latency_ms(&self) -> f64 {
        let efficiency = self.efficiency_ratio.load();
        if efficiency > 0.0 {
            1.0 / efficiency - 1.0
        } else {
            0.0
        }
    }

    /// Get estimated 95th percentile latency
    #[inline]
    pub fn p95_latency_estimate_ms(&self) -> f64 {
        self.average_latency_ms() * 1.5 // Conservative estimate
    }

    /// Get efficiency score
    #[inline]
    pub fn efficiency_score(&self) -> f64 {
        self.efficiency_ratio.load()
    }

    /// Update write-specific metrics (from policy version)
    #[inline]
    pub fn update_write_metrics(
        &self,
        writes_per_second: u32,
        bytes_per_second: u64,
        avg_write_latency: u64,
    ) {
        self.ops_per_second.store(writes_per_second as f64);
        self.bytes_per_second.store(bytes_per_second as f64);
        // Convert write latency to efficiency approximation
        let latency_ms = avg_write_latency as f64 / 1_000_000.0;
        let efficiency = 1.0 / (latency_ms + 1.0);
        self.efficiency_ratio.store(efficiency);
    }

    /// Get writes per second (alias for ops_per_second)
    #[inline]
    pub fn writes_per_second(&self) -> u32 {
        self.ops_per_second.load() as u32
    }

    /// Get avg write latency (estimated from efficiency)
    #[inline]
    pub fn avg_write_latency(&self) -> u64 {
        (self.average_latency_ms() * 1_000_000.0) as u64
    }

    /// Get write queue depth (not tracked, return 0)
    #[inline]
    pub fn write_queue_depth(&self) -> u32 {
        0 // Not tracked in canonical version
    }
}

impl Default for ResourceMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceMetrics {
    #[inline]
    pub fn new() -> Self {
        Self {
            cpu_utilization: AtomicCell::new(0.0),
            memory_utilization: AtomicCell::new(0.0),
            cache_line_utilization: AtomicCell::new(0.0),
            network_utilization: AtomicCell::new(0.0),
            disk_utilization: AtomicCell::new(0.0),
        }
    }
}

impl Default for EffectivenessMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl EffectivenessMetrics {
    #[inline]
    pub fn new() -> Self {
        Self {
            hit_rate: AtomicCell::new(0.0),
            miss_rate: AtomicCell::new(0.0),
            eviction_accuracy: AtomicCell::new(0.0),
            prefetch_success_rate: AtomicCell::new(0.0),
            working_set_ratio: AtomicCell::new(0.0),
        }
    }
}
