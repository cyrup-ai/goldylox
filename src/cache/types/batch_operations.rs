#![allow(dead_code)]
// Batch Operations - Complete batch processing library with timed results, statistical analysis, operation builders, latency tracking, and comprehensive multi-key operation coordination

//! Batch operation support for efficient multi-key operations
//!
//! This module provides batch operation types for processing multiple
//! cache operations efficiently in a single call.

use std::collections::HashMap;
use std::time::Instant;

use super::core_types::CacheResult;
use crate::cache::traits::{CacheKey, CacheValue};

/// Timed result wrapper that captures operation timing alongside the result
#[derive(Debug, Clone)]
pub struct TimedResult<V: CacheValue> {
    /// The actual operation result
    pub result: CacheResult<V>,
    /// Operation execution time in nanoseconds
    pub execution_time_ns: u64,
}

impl<V: CacheValue> TimedResult<V> {
    /// Create a new timed result
    pub fn new(result: CacheResult<V>, execution_time_ns: u64) -> Self {
        Self {
            result,
            execution_time_ns,
        }
    }

    /// Create timed result by executing a closure and measuring its duration
    pub fn measure<F>(operation: F) -> Self
    where
        F: FnOnce() -> CacheResult<V>,
    {
        let start = Instant::now();
        let result = operation();
        let execution_time_ns = start.elapsed().as_nanos() as u64;

        Self::new(result, execution_time_ns)
    }

    /// Check if the operation was successful
    pub fn is_success(&self) -> bool {
        self.result.is_ok()
    }
}

/// Extension trait for Result to provide is_success method
trait ResultExt<T, E> {
    fn is_success(&self) -> bool;
}

impl<T, E> ResultExt<T, E> for Result<T, E> {
    fn is_success(&self) -> bool {
        self.is_ok()
    }
}

/// Batch operation result for multi-key operations
#[derive(Debug, Clone)]
pub struct BatchResult<K: CacheKey, V: CacheValue> {
    /// Individual operation results with timing
    pub results: HashMap<K, TimedResult<V>>,
    /// Total batch operation time
    pub total_time_ns: u64,
    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,
    /// Average latency per operation
    pub avg_latency_ns: u64,
}

impl<K: CacheKey, V: CacheValue> BatchResult<K, V> {
    /// Create new batch result from timed results with proper latency tracking
    pub fn new(results: HashMap<K, TimedResult<V>>) -> Self {
        let total_operations = results.len();
        let successful_operations = results.values().filter(|r| r.is_success()).count();

        // Calculate real timing metrics from captured execution times
        let execution_times: Vec<u64> = results.values().map(|r| r.execution_time_ns).collect();
        let total_latency: u64 = execution_times.iter().sum();
        let total_time_ns = execution_times.iter().max().copied().unwrap_or(0);

        let success_rate = if total_operations > 0 {
            successful_operations as f64 / total_operations as f64
        } else {
            0.0
        };

        let avg_latency_ns = if total_operations > 0 {
            total_latency / total_operations as u64
        } else {
            0
        };

        Self {
            results,
            total_time_ns,
            success_rate,
            avg_latency_ns,
        }
    }

    // REMOVED: from_cache_results() backward compatibility function
    // This function converted between result types and hid timing requirements
    // Users must now create TimedResult instances directly with proper timing data

    /// Get successful results with timing information
    pub fn successes(&self) -> impl Iterator<Item = (&K, &TimedResult<V>)> {
        self.results
            .iter()
            .filter(|(_, result)| result.is_success())
    }

    /// Get failed results with timing information
    pub fn failures(&self) -> impl Iterator<Item = (&K, &TimedResult<V>)> {
        self.results
            .iter()
            .filter(|(_, result)| !result.is_success())
    }

    /// Get successful results (just the CacheResult part)
    pub fn success_results(&self) -> impl Iterator<Item = (&K, &CacheResult<V>)> {
        self.results
            .iter()
            .filter(|(_, result)| result.is_success())
            .map(|(k, r)| (k, &r.result))
    }

    /// Get failed results (just the CacheResult part)
    pub fn failure_results(&self) -> impl Iterator<Item = (&K, &CacheResult<V>)> {
        self.results
            .iter()
            .filter(|(_, result)| !result.is_success())
            .map(|(k, r)| (k, &r.result))
    }

    /// Get total number of operations
    pub fn total_operations(&self) -> usize {
        self.results.len()
    }

    /// Get number of successful operations
    pub fn successful_operations(&self) -> usize {
        self.results.values().filter(|r| r.is_success()).count()
    }

    /// Get number of failed operations
    pub fn failed_operations(&self) -> usize {
        self.results.values().filter(|r| !r.is_success()).count()
    }

    /// Check if all operations succeeded
    pub fn all_succeeded(&self) -> bool {
        self.success_rate == 1.0
    }

    /// Check if any operations succeeded
    pub fn any_succeeded(&self) -> bool {
        self.success_rate > 0.0
    }

    /// Get timed result for specific key
    pub fn get_timed_result(&self, key: &K) -> Option<&TimedResult<V>> {
        self.results.get(key)
    }

    /// Get operation result for specific key (just the CacheResult part)
    pub fn get_result(&self, key: &K) -> Option<&CacheResult<V>> {
        self.results.get(key).map(|r| &r.result)
    }

    /// Check if operation for key was successful
    pub fn was_successful(&self, key: &K) -> bool {
        self.results.get(key).is_some_and(|r| r.is_success())
    }

    /// Get execution time for specific key operation
    pub fn get_execution_time(&self, key: &K) -> Option<u64> {
        self.results.get(key).map(|r| r.execution_time_ns)
    }

    /// Get batch statistics summary with real timing data
    pub fn summary(&self) -> BatchSummary {
        let execution_times: Vec<u64> =
            self.results.values().map(|r| r.execution_time_ns).collect();

        BatchSummary {
            total_operations: self.total_operations(),
            successful_operations: self.successful_operations(),
            failed_operations: self.failed_operations(),
            success_rate: self.success_rate,
            total_time_ns: self.total_time_ns,
            avg_latency_ns: self.avg_latency_ns,
            min_latency_ns: execution_times.iter().min().copied().unwrap_or(0),
            max_latency_ns: execution_times.iter().max().copied().unwrap_or(0),
        }
    }
}

/// Batch operation summary statistics
#[derive(Debug, Clone, Copy)]
pub struct BatchSummary {
    /// Total number of operations
    pub total_operations: usize,
    /// Number of successful operations
    pub successful_operations: usize,
    /// Number of failed operations
    pub failed_operations: usize,
    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,
    /// Total batch time in nanoseconds
    pub total_time_ns: u64,
    /// Average latency per operation
    pub avg_latency_ns: u64,
    /// Minimum operation latency
    pub min_latency_ns: u64,
    /// Maximum operation latency
    pub max_latency_ns: u64,
}

impl BatchSummary {
    /// Get throughput in operations per second
    pub fn throughput_ops_per_sec(&self) -> f64 {
        if self.total_time_ns > 0 {
            (self.total_operations as f64) / (self.total_time_ns as f64 / 1_000_000_000.0)
        } else {
            0.0
        }
    }

    /// Get latency statistics in a readable format
    pub fn latency_stats_ms(&self) -> LatencyStats {
        LatencyStats {
            avg_ms: self.avg_latency_ns as f64 / 1_000_000.0,
            min_ms: self.min_latency_ns as f64 / 1_000_000.0,
            max_ms: self.max_latency_ns as f64 / 1_000_000.0,
        }
    }
}

/// Latency statistics in milliseconds
#[derive(Debug, Clone, Copy)]
pub struct LatencyStats {
    /// Average latency in milliseconds
    pub avg_ms: f64,
    /// Minimum latency in milliseconds
    pub min_ms: f64,
    /// Maximum latency in milliseconds
    pub max_ms: f64,
}

/// Batch operation builder for constructing batch requests
#[derive(Debug)]
pub struct BatchOperationBuilder<K: CacheKey, V: CacheValue> {
    operations: Vec<BatchOperation<K, V>>,
    timeout_ms: Option<u64>,
    max_parallel: Option<usize>,
}

/// Individual operation in a batch
#[derive(Debug, Clone)]
pub enum BatchOperation<K: CacheKey, V: CacheValue> {
    /// Get operation
    Get { key: K },
    /// Put operation
    Put { key: K, value: V },
    /// Remove operation
    Remove { key: K },
}

impl<K: CacheKey, V: CacheValue> BatchOperationBuilder<K, V> {
    /// Create new batch operation builder
    pub fn new() -> Self {
        Self {
            operations: Vec::new(),
            timeout_ms: None,
            max_parallel: None,
        }
    }

    /// Add get operation to batch
    pub fn get(mut self, key: K) -> Self {
        self.operations.push(BatchOperation::Get { key });
        self
    }

    /// Add put operation to batch
    pub fn put(mut self, key: K, value: V) -> Self {
        self.operations.push(BatchOperation::Put { key, value });
        self
    }

    /// Add remove operation to batch
    pub fn remove(mut self, key: K) -> Self {
        self.operations.push(BatchOperation::Remove { key });
        self
    }

    /// Set batch timeout in milliseconds
    pub fn timeout_ms(mut self, timeout: u64) -> Self {
        self.timeout_ms = Some(timeout);
        self
    }

    /// Set maximum parallel operations
    pub fn max_parallel(mut self, max: usize) -> Self {
        self.max_parallel = Some(max);
        self
    }

    /// Build batch operation list
    pub fn build(self) -> Vec<BatchOperation<K, V>> {
        self.operations
    }

    /// Get operation count
    pub fn operation_count(&self) -> usize {
        self.operations.len()
    }

    /// Check if batch is empty
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }
}

impl<K: CacheKey, V: CacheValue> Default for BatchOperationBuilder<K, V> {
    fn default() -> Self {
        Self::new()
    }
}
