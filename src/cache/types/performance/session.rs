#![allow(dead_code)]
// Performance Types - Complete session-based performance measurement library with operation tracking, precise timing handles, batch timing analysis, and detailed session management

//! Performance measurement sessions
//!
//! This module provides session-based performance tracking with
//! operation timing and measurement capabilities.

use std::time::Instant;

use super::analysis::PerformanceReport;
use super::timer::{PrecisionTimer, timestamp_nanos};

/// Performance measurement session for detailed profiling
#[derive(Debug, Clone)]
pub struct PerformanceSession {
    /// Session start time
    start_time: Instant,
    /// Individual operation timings
    operations: Vec<OperationTiming>,
    /// Total elapsed time
    total_time_ns: u64,
    /// Session name for identification
    name: String,
}

impl PerformanceSession {
    /// Create new performance session
    #[inline(always)]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            start_time: Instant::now(),
            operations: Vec::new(),
            total_time_ns: 0,
            name: name.into(),
        }
    }

    /// Start measuring an operation
    #[inline(always)]
    pub fn start_operation(&mut self, operation_name: impl Into<String>) -> OperationHandle {
        let handle = OperationHandle {
            name: operation_name.into(),
            start_time: PrecisionTimer::start(),
            index: self.operations.len(),
        };

        // Convert RDTSC cycles to absolute nanosecond timestamp
        let start_ns = {
            use std::time::Instant;

            use super::timer::timestamp_nanos;
            timestamp_nanos(Instant::now())
        };

        self.operations.push(OperationTiming {
            name: handle.name.clone(),
            duration_ns: 0, // Will be filled when operation completes
            start_ns,
            end_ns: 0,
        });

        handle
    }

    /// Complete an operation measurement
    #[inline(always)]
    pub fn complete_operation(&mut self, handle: OperationHandle) {
        let elapsed_ns = handle.start_time.elapsed_ns();

        if let Some(operation) = self.operations.get_mut(handle.index) {
            operation.duration_ns = elapsed_ns;
            operation.end_ns = operation.start_ns + elapsed_ns;
        }
    }

    /// Finish the performance session
    #[inline(always)]
    pub fn finish(mut self) -> PerformanceReport {
        self.total_time_ns = timestamp_nanos(Instant::now()) - timestamp_nanos(self.start_time);

        PerformanceReport::new(self.name, self.operations, self.total_time_ns)
    }

    /// Get current session duration
    #[inline(always)]
    pub fn current_duration_ns(&self) -> u64 {
        timestamp_nanos(Instant::now()) - timestamp_nanos(self.start_time)
    }

    /// Get operation count
    #[inline(always)]
    pub fn operation_count(&self) -> usize {
        self.operations.len()
    }

    /// Get session name
    #[inline(always)]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Add operation directly (for external timing)
    pub fn add_operation(&mut self, timing: OperationTiming) {
        self.operations.push(timing);
    }

    /// Get operations reference (for inspection)
    pub fn operations(&self) -> &[OperationTiming] {
        &self.operations
    }

    /// Clear all operations
    pub fn clear_operations(&mut self) {
        self.operations.clear();
    }

    /// Get total operation time so far
    pub fn total_operation_time_ns(&self) -> u64 {
        self.operations.iter().map(|op| op.duration_ns).sum()
    }

    /// Get average operation time
    pub fn avg_operation_time_ns(&self) -> u64 {
        if self.operations.is_empty() {
            0
        } else {
            self.total_operation_time_ns() / self.operations.len() as u64
        }
    }
}

/// Handle for tracking individual operation timing
#[derive(Debug)]
pub struct OperationHandle {
    /// Operation name
    name: String,
    /// Start timer
    start_time: PrecisionTimer,
    /// Index in operations vector
    index: usize,
}

impl OperationHandle {
    /// Get operation name
    #[inline(always)]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get current elapsed time
    #[inline(always)]
    pub fn elapsed_ns(&self) -> u64 {
        self.start_time.elapsed_ns()
    }

    /// Get operation index
    #[inline(always)]
    pub fn index(&self) -> usize {
        self.index
    }
}

/// Individual operation timing record
#[derive(Debug, Clone)]
pub struct OperationTiming {
    /// Operation name
    pub name: String,
    /// Operation duration in nanoseconds
    pub duration_ns: u64,
    /// Start timestamp
    pub start_ns: u64,
    /// End timestamp
    pub end_ns: u64,
}

impl OperationTiming {
    /// Create new operation timing
    pub fn new(name: impl Into<String>, duration_ns: u64, start_ns: u64) -> Self {
        Self {
            name: name.into(),
            duration_ns,
            start_ns,
            end_ns: start_ns + duration_ns,
        }
    }

    /// Create timing from start and end times
    pub fn from_timestamps(name: impl Into<String>, start_ns: u64, end_ns: u64) -> Self {
        Self {
            name: name.into(),
            duration_ns: end_ns.saturating_sub(start_ns),
            start_ns,
            end_ns,
        }
    }

    /// Get duration as microseconds
    #[inline(always)]
    pub fn duration_micros(&self) -> u64 {
        self.duration_ns / 1_000
    }

    /// Get duration as milliseconds
    #[inline(always)]
    pub fn duration_millis(&self) -> u64 {
        self.duration_ns / 1_000_000
    }

    /// Get duration as seconds
    #[inline(always)]
    pub fn duration_secs_f64(&self) -> f64 {
        self.duration_ns as f64 / 1_000_000_000.0
    }

    /// Check if operation is slow (above threshold)
    #[inline(always)]
    pub fn is_slow(&self, threshold_ns: u64) -> bool {
        self.duration_ns > threshold_ns
    }

    /// Get operation throughput (operations per second)
    #[inline(always)]
    pub fn throughput_ops_per_sec(&self) -> f64 {
        if self.duration_ns > 0 {
            1_000_000_000.0 / self.duration_ns as f64
        } else {
            f64::INFINITY
        }
    }
}

/// Batch operation timing for measuring groups of operations
#[derive(Debug, Clone)]
pub struct BatchTiming {
    /// Batch name
    pub name: String,
    /// Individual operation timings
    pub operations: Vec<OperationTiming>,
    /// Total batch duration
    pub total_duration_ns: u64,
    /// Batch start timestamp
    pub start_ns: u64,
}

impl BatchTiming {
    /// Create new batch timing
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            operations: Vec::new(),
            total_duration_ns: 0,
            start_ns: timestamp_nanos(Instant::now()),
        }
    }

    /// Add operation to batch
    pub fn add_operation(&mut self, timing: OperationTiming) {
        self.operations.push(timing);
    }

    /// Finish batch timing
    pub fn finish(mut self) -> Self {
        let now_ns = timestamp_nanos(Instant::now());
        self.total_duration_ns = now_ns.saturating_sub(self.start_ns);
        self
    }

    /// Get batch throughput (operations per second)
    pub fn throughput_ops_per_sec(&self) -> f64 {
        if self.total_duration_ns > 0 {
            (self.operations.len() as f64) / (self.total_duration_ns as f64 / 1_000_000_000.0)
        } else {
            0.0
        }
    }

    /// Get average operation time
    pub fn avg_operation_time_ns(&self) -> u64 {
        if self.operations.is_empty() {
            0
        } else {
            self.operations.iter().map(|op| op.duration_ns).sum::<u64>()
                / self.operations.len() as u64
        }
    }
}
