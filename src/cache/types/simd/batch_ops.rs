//! SIMD-optimized batch operations for cache management
//!
//! This module provides high-performance batch processing capabilities
//! for cache operations with comprehensive performance tracking.

use std::sync::Arc;

use crate::cache::types::results::CacheResult;
use super::hasher::SimdHasher;
use super::vectorops::SimdVectorOps;
use crate::cache::traits::{CacheKey, CacheValue};

/// Type aliases for common SIMD configurations
pub type GenericBatchResult = BatchPerformanceMetrics;

/// SIMD-optimized batch operations manager
#[derive(Debug)]
pub struct BatchOperationManager {
    hasher: SimdHasher,
    max_batch_size: usize,
}

impl BatchOperationManager {
    /// Create new batch operation manager
    #[inline(always)]
    pub fn new(max_batch_size: usize) -> Self {
        Self {
            hasher: SimdHasher::default(),
            max_batch_size,
        }
    }

    /// Create batch manager with custom hasher
    #[inline(always)]
    pub fn with_hasher(hasher: SimdHasher, max_batch_size: usize) -> Self {
        Self {
            hasher,
            max_batch_size,
        }
    }

    /// Process batch of cache keys with SIMD hashing
    pub fn hash_batch<K: CacheKey>(&mut self, keys: &[K]) -> Vec<u64> {
        // Process in optimal batch sizes
        keys.chunks(self.max_batch_size)
            .flat_map(|chunk| self.hasher.hash_batch_keys(chunk))
            .collect()
    }

    /// Analyze batch operation performance
    pub fn analyze_batch_performance<V: CacheValue>(
        results: &[CacheResult<Arc<V>>],
    ) -> BatchPerformanceMetrics {
        let total_operations = results.len();
        let successful_operations = results.iter().filter(|r| r.is_success()).count();

        let latencies: Vec<f64> = results.iter().map(|r| r.latency_ns as f64).collect();

        BatchPerformanceMetrics {
            total_operations,
            successful_operations,
            success_rate: successful_operations as f64 / total_operations as f64,
            avg_latency_ns: SimdVectorOps::average_f64(&latencies).unwrap_or(0.0) as u64,
            min_latency_ns: SimdVectorOps::find_min_f64(&latencies).unwrap_or(0.0) as u64,
            max_latency_ns: SimdVectorOps::find_max_f64(&latencies).unwrap_or(0.0) as u64,
            total_latency_ns: SimdVectorOps::sum_f64(&latencies) as u64,
        }
    }

    /// Process batch of text strings for hashing
    pub fn hash_text_batch(&mut self, texts: &[&str]) -> Vec<u64> {
        texts
            .chunks(self.max_batch_size)
            .flat_map(|chunk| {
                chunk
                    .iter()
                    .map(|text| self.hasher.hash_string(text))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// Get optimal batch size for current configuration
    #[inline(always)]
    pub fn optimal_batch_size(&self) -> usize {
        self.max_batch_size
    }

    /// Update maximum batch size
    #[inline(always)]
    pub fn set_max_batch_size(&mut self, size: usize) {
        self.max_batch_size = size;
    }

    /// Reset internal hasher state
    #[inline(always)]
    pub fn reset_hasher(&mut self) {
        self.hasher.reset();
    }

    /// Calculate throughput metrics for batch operations
    pub fn calculate_throughput(
        metrics: &BatchPerformanceMetrics,
        duration_ns: u64,
    ) -> ThroughputMetrics {
        let ops_per_second = if duration_ns > 0 {
            (metrics.total_operations as f64) / (duration_ns as f64 / 1_000_000_000.0)
        } else {
            0.0
        };

        let successful_ops_per_second = if duration_ns > 0 {
            (metrics.successful_operations as f64) / (duration_ns as f64 / 1_000_000_000.0)
        } else {
            0.0
        };

        ThroughputMetrics {
            operations_per_second: ops_per_second,
            successful_operations_per_second: successful_ops_per_second,
            average_latency_ms: metrics.avg_latency_ns as f64 / 1_000_000.0,
            p95_latency_estimate_ms: metrics.max_latency_ns as f64 / 1_000_000.0 * 0.95,
            efficiency_score: metrics.success_rate * (1.0 / (metrics.avg_latency_ns as f64 + 1.0)),
        }
    }
}

impl Default for BatchOperationManager {
    fn default() -> Self {
        Self::new(64) // Default batch size of 64
    }
}

/// Batch operation performance metrics
#[derive(Debug, Clone)]
pub struct BatchPerformanceMetrics {
    /// Total number of operations in batch
    pub total_operations: usize,
    /// Number of successful operations
    pub successful_operations: usize,
    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,
    /// Average latency in nanoseconds
    pub avg_latency_ns: u64,
    /// Minimum latency in nanoseconds
    pub min_latency_ns: u64,
    /// Maximum latency in nanoseconds
    pub max_latency_ns: u64,
    /// Total latency for all operations
    pub total_latency_ns: u64,
}

impl BatchPerformanceMetrics {
    /// Create new batch performance metrics
    pub fn new(
        total_operations: usize,
        successful_operations: usize,
        avg_latency_ns: u64,
        min_latency_ns: u64,
        max_latency_ns: u64,
    ) -> Self {
        let success_rate = if total_operations > 0 {
            successful_operations as f64 / total_operations as f64
        } else {
            0.0
        };

        Self {
            total_operations,
            successful_operations,
            success_rate,
            avg_latency_ns,
            min_latency_ns,
            max_latency_ns,
            total_latency_ns: avg_latency_ns * total_operations as u64,
        }
    }

    /// Check if batch performance meets quality thresholds
    pub fn meets_quality_threshold(&self, min_success_rate: f64, max_avg_latency_ms: f64) -> bool {
        let avg_latency_ms = self.avg_latency_ns as f64 / 1_000_000.0;
        self.success_rate >= min_success_rate && avg_latency_ms <= max_avg_latency_ms
    }

    /// Get latency distribution summary
    pub fn latency_distribution(&self) -> LatencyDistribution {
        LatencyDistribution {
            min_ns: self.min_latency_ns,
            max_ns: self.max_latency_ns,
            avg_ns: self.avg_latency_ns,
            range_ns: self.max_latency_ns.saturating_sub(self.min_latency_ns),
            coefficient_of_variation: if self.avg_latency_ns > 0 {
                (self.max_latency_ns.saturating_sub(self.min_latency_ns) as f64)
                    / (self.avg_latency_ns as f64)
            } else {
                0.0
            },
        }
    }
}

/// Throughput analysis metrics
#[derive(Debug, Clone)]
pub struct ThroughputMetrics {
    /// Operations per second
    pub operations_per_second: f64,
    /// Successful operations per second
    pub successful_operations_per_second: f64,
    /// Average latency in milliseconds
    pub average_latency_ms: f64,
    /// Estimated 95th percentile latency in milliseconds
    pub p95_latency_estimate_ms: f64,
    /// Overall efficiency score (0.0 to 1.0)
    pub efficiency_score: f64,
}

/// Latency distribution analysis
#[derive(Debug, Clone)]
pub struct LatencyDistribution {
    /// Minimum latency
    pub min_ns: u64,
    /// Maximum latency
    pub max_ns: u64,
    /// Average latency
    pub avg_ns: u64,
    /// Latency range (max - min)
    pub range_ns: u64,
    /// Coefficient of variation (range/mean)
    pub coefficient_of_variation: f64,
}
