#![allow(dead_code)]
// Performance Types - Complete performance analysis library with statistical analysis, percentile calculations, performance reports, operation tier classification, and comprehensive metrics analysis

//! Performance analysis and reporting
//!
//! This module provides statistical analysis of performance data
//! with comprehensive reporting capabilities.

use super::session::OperationTiming;

/// Complete performance report with analysis
#[derive(Debug, Clone)]
pub struct PerformanceReport {
    /// Session name
    pub name: String,
    /// Individual operation timings
    pub operations: Vec<OperationTiming>,
    /// Total session time
    pub total_time_ns: u64,
    /// Statistical analysis
    pub analysis: PerformanceAnalysis,
}

impl PerformanceReport {
    /// Create new performance report
    pub fn new(name: String, operations: Vec<OperationTiming>, total_time_ns: u64) -> Self {
        let analysis = PerformanceAnalysis::analyze(&operations, total_time_ns);

        Self {
            name,
            operations,
            total_time_ns,
            analysis,
        }
    }

    /// Get operations per second
    #[inline(always)]
    pub fn ops_per_second(&self) -> f64 {
        if self.total_time_ns > 0 {
            (self.operations.len() as f64) / (self.total_time_ns as f64 / 1_000_000_000.0)
        } else {
            0.0
        }
    }

    /// Get average operation time
    #[inline(always)]
    pub fn avg_operation_time_ns(&self) -> u64 {
        if !self.operations.is_empty() {
            self.operations.iter().map(|op| op.duration_ns).sum::<u64>()
                / self.operations.len() as u64
        } else {
            0
        }
    }

    /// Get slowest operation
    pub fn slowest_operation(&self) -> Option<&OperationTiming> {
        self.operations.iter().max_by_key(|op| op.duration_ns)
    }

    /// Get fastest operation
    pub fn fastest_operation(&self) -> Option<&OperationTiming> {
        self.operations.iter().min_by_key(|op| op.duration_ns)
    }

    /// Get operations by name pattern
    pub fn operations_matching(&self, pattern: &str) -> Vec<&OperationTiming> {
        self.operations
            .iter()
            .filter(|op| op.name.contains(pattern))
            .collect()
    }

    /// Get total duration of all operations
    #[inline(always)]
    pub fn total_operation_time_ns(&self) -> u64 {
        self.operations.iter().map(|op| op.duration_ns).sum()
    }

    /// Get overhead percentage
    #[inline(always)]
    pub fn overhead_percentage(&self) -> f64 {
        let productive_time = self.total_operation_time_ns();
        if self.total_time_ns > productive_time {
            ((self.total_time_ns - productive_time) as f64 / self.total_time_ns as f64) * 100.0
        } else {
            0.0
        }
    }

    /// Get operations grouped by performance tier
    pub fn operations_by_tier(&self) -> OperationTiers<'_> {
        if self.operations.is_empty() {
            return OperationTiers::default();
        }

        let mut durations: Vec<_> = self.operations.iter().map(|op| op.duration_ns).collect();
        durations.sort_unstable();

        let p25_index = (durations.len() as f64 * 0.25) as usize;
        let p75_index = (durations.len() as f64 * 0.75) as usize;

        let p25_threshold = durations[p25_index.min(durations.len() - 1)];
        let p75_threshold = durations[p75_index.min(durations.len() - 1)];

        let fast: Vec<_> = self
            .operations
            .iter()
            .filter(|op| op.duration_ns <= p25_threshold)
            .collect();

        let medium: Vec<_> = self
            .operations
            .iter()
            .filter(|op| op.duration_ns > p25_threshold && op.duration_ns <= p75_threshold)
            .collect();

        let slow: Vec<_> = self
            .operations
            .iter()
            .filter(|op| op.duration_ns > p75_threshold)
            .collect();

        OperationTiers { fast, medium, slow }
    }
}

/// Statistical analysis of performance data
#[derive(Debug, Clone)]
pub struct PerformanceAnalysis {
    /// Average operation duration
    pub avg_duration_ns: u64,
    /// Median operation duration
    pub median_duration_ns: u64,
    /// 95th percentile duration
    pub p95_duration_ns: u64,
    /// 99th percentile duration
    pub p99_duration_ns: u64,
    /// Standard deviation
    pub std_deviation_ns: f64,
    /// Total overhead percentage
    pub overhead_percent: f64,
}

impl PerformanceAnalysis {
    /// Analyze operation timings
    pub fn analyze(operations: &[OperationTiming], total_time_ns: u64) -> Self {
        if operations.is_empty() {
            return Self {
                avg_duration_ns: 0,
                median_duration_ns: 0,
                p95_duration_ns: 0,
                p99_duration_ns: 0,
                std_deviation_ns: 0.0,
                overhead_percent: 0.0,
            };
        }

        let mut durations: Vec<u64> = operations.iter().map(|op| op.duration_ns).collect();
        durations.sort_unstable();

        let sum: u64 = durations.iter().sum();
        let avg = sum / durations.len() as u64;

        let median = durations[durations.len() / 2];
        let p95_index = (durations.len() as f64 * 0.95) as usize;
        let p99_index = (durations.len() as f64 * 0.99) as usize;
        let p95 = durations[p95_index.min(durations.len() - 1)];
        let p99 = durations[p99_index.min(durations.len() - 1)];

        // Calculate standard deviation
        let variance: f64 = durations
            .iter()
            .map(|&duration| {
                let diff = duration as f64 - avg as f64;
                diff * diff
            })
            .sum::<f64>()
            / durations.len() as f64;
        let std_deviation = variance.sqrt();

        // Calculate overhead percentage
        let productive_time = sum;
        let overhead_percent = if total_time_ns > productive_time {
            ((total_time_ns - productive_time) as f64 / total_time_ns as f64) * 100.0
        } else {
            0.0
        };

        Self {
            avg_duration_ns: avg,
            median_duration_ns: median,
            p95_duration_ns: p95,
            p99_duration_ns: p99,
            std_deviation_ns: std_deviation,
            overhead_percent,
        }
    }

    /// Get performance summary using canonical PerformanceSummary
    pub fn summary(&self) -> PerformanceSummary {
        let mut summary = PerformanceSummary {
            avg_access_time_ns: self.avg_duration_ns,
            max_access_time_ns: self.p99_duration_ns,
            ..Default::default()
        };
        summary.update_qualitative_analysis(
            self.std_deviation_ns,
            self.p99_duration_ns,
            self.overhead_percent,
        );
        summary
    }
}

// PerformanceSummary moved to canonical location: crate::telemetry::performance_history::PerformanceSummary
// Use the enhanced canonical implementation with comprehensive quantitative metrics, error recovery tracking, and qualitative analysis
pub use crate::telemetry::performance_history::PerformanceSummary;

/// Operations grouped by performance tiers
#[derive(Debug, Clone, Default)]
pub struct OperationTiers<'a> {
    /// Fast operations (bottom 25%)
    pub fast: Vec<&'a OperationTiming>,
    /// Medium operations (25% - 75%)
    pub medium: Vec<&'a OperationTiming>,
    /// Slow operations (top 25%)
    pub slow: Vec<&'a OperationTiming>,
}
