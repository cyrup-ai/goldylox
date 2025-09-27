//! Configuration management for write propagation system
//!
//! This module provides different configuration presets and management
//! for write propagation behavior optimization.

use super::types::PropagationConfig;

impl Default for PropagationConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl PropagationConfig {
    /// Default balanced configuration
    #[allow(dead_code)] // MESI coherence - configuration used in write propagation setup
    pub fn new() -> Self {
        Self {
            writeback_delay_ns: 1_000_000_000, // 1 second
            max_queue_size: 1000,
            batch_size: 32,
            adaptive_switching: true,
            worker_threads: 2,
        }
    }

    /// High throughput configuration for heavy workloads
    pub fn high_throughput() -> Self {
        Self {
            writeback_delay_ns: 100_000_000, // 100ms
            max_queue_size: 5000,
            batch_size: 128,
            adaptive_switching: true,
            worker_threads: 4,
        }
    }

    /// Low latency configuration for real-time applications
    pub fn low_latency() -> Self {
        Self {
            writeback_delay_ns: 10_000_000, // 10ms
            max_queue_size: 100,
            batch_size: 8,
            adaptive_switching: false,
            worker_threads: 1,
        }
    }

    /// Memory-constrained configuration for limited resources
    pub fn memory_constrained() -> Self {
        Self {
            writeback_delay_ns: 500_000_000, // 500ms
            max_queue_size: 200,
            batch_size: 16,
            adaptive_switching: true,
            worker_threads: 1,
        }
    }

    /// Validate configuration parameters
    pub fn validate(&self) -> Result<(), String> {
        if self.writeback_delay_ns == 0 {
            return Err("writeback_delay_ns must be greater than 0".to_string());
        }
        if self.max_queue_size == 0 {
            return Err("max_queue_size must be greater than 0".to_string());
        }
        if self.batch_size == 0 {
            return Err("batch_size must be greater than 0".to_string());
        }
        if self.worker_threads == 0 {
            return Err("worker_threads must be greater than 0".to_string());
        }
        Ok(())
    }
}
