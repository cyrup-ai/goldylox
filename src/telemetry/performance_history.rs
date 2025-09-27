#![allow(dead_code)]
// Telemetry System - Complete performance history library with circular buffer management, sample retention, error recovery integration, qualitative analysis, and comprehensive performance tracking

//! Performance history with efficient circular buffer
//!
//! This module implements the `PerformanceHistory` structure for storing
//! and managing historical performance samples with automatic cleanup.

use std::sync::atomic::{AtomicUsize, Ordering};

use arrayvec::ArrayVec;
use crossbeam_utils::CachePadded;

use super::data_structures::RetentionPolicy;
use super::types::{MonitorConfig, PerformanceSample};
use crate::cache::traits::types_and_enums::CacheOperationError;

/// Performance history with efficient circular buffer
#[derive(Debug)]
pub struct PerformanceHistory {
    /// Circular buffer for performance samples
    history_buffer: CachePadded<ArrayVec<PerformanceSample, 1024>>,
    /// Buffer write position (atomic)
    write_position: CachePadded<AtomicUsize>,
    /// Buffer capacity utilization
    utilization: CachePadded<AtomicUsize>,
    /// History retention policy
    retention_policy: RetentionPolicy,
}

impl PerformanceHistory {
    /// Create new performance history with configuration
    pub fn new(_config: MonitorConfig) -> Result<Self, CacheOperationError> {
        Ok(Self {
            history_buffer: CachePadded::new(ArrayVec::new()),
            write_position: CachePadded::new(AtomicUsize::new(0)),
            utilization: CachePadded::new(AtomicUsize::new(0)),
            retention_policy: RetentionPolicy::new(),
        })
    }

    /// Add performance sample to history
    pub fn add_sample(&mut self, sample: PerformanceSample) -> Result<(), CacheOperationError> {
        // Check if cleanup is needed
        self.cleanup_old_samples()?;

        // Add sample to buffer
        if self.history_buffer.len() >= self.history_buffer.capacity() {
            // Buffer is full, remove oldest sample (circular buffer behavior)
            self.history_buffer.remove(0);
        }

        self.history_buffer.push(sample);

        // Update position and utilization
        let _new_pos = self.write_position.fetch_add(1, Ordering::Relaxed) + 1;
        self.utilization
            .store(self.history_buffer.len(), Ordering::Relaxed);

        Ok(())
    }

    /// Get recent samples (up to specified count)
    pub fn get_recent_samples(&self, count: usize) -> Vec<(u64, f32)> {
        let buffer_len = self.history_buffer.len();
        let start_idx = buffer_len.saturating_sub(count);

        self.history_buffer[start_idx..]
            .iter()
            .map(|sample| {
                let timestamp_ns = sample.timestamp_ns;
                let hit_rate = if sample.tier_hit { 1.0 } else { 0.0 };
                (timestamp_ns, hit_rate)
            })
            .collect()
    }

    /// Get samples within a time window
    pub fn get_samples_in_window(&self, start_time: u64, end_time: u64) -> Vec<PerformanceSample> {
        // Filter samples by timestamp_ns field in PerformanceSample
        self.history_buffer
            .iter()
            .filter(|sample| sample.timestamp_ns >= start_time && sample.timestamp_ns <= end_time)
            .cloned()
            .collect()
    }

    /// Get sample count
    pub fn sample_count(&self) -> usize {
        self.utilization.load(Ordering::Relaxed)
    }

    /// Get buffer utilization percentage
    pub fn utilization_percentage(&self) -> f32 {
        let current_count = self.utilization.load(Ordering::Relaxed);
        (current_count as f32 / 1024.0) * 100.0
    }

    /// Calculate average metric over time window
    pub fn calculate_average_metric<F>(&self, window_ns: u64, extract_metric: F) -> Option<f32>
    where
        F: Fn(&PerformanceSample) -> f32,
    {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        let window_start = now.saturating_sub(window_ns);
        let samples = self.get_samples_in_window(window_start, now);

        if samples.is_empty() {
            return None;
        }

        let sum: f32 = samples.iter().map(extract_metric).sum();
        Some(sum / samples.len() as f32)
    }

    /// Get performance statistics summary with error recovery provider
    pub fn get_performance_summary(
        &self,
        error_provider: &dyn crate::cache::manager::error_recovery::ErrorRecoveryProvider,
    ) -> PerformanceSummary {
        if self.history_buffer.is_empty() {
            return PerformanceSummary::default();
        }

        let samples = &self.history_buffer[..];
        let count = samples.len();

        // Calculate averages using feature-rich PerformanceSample fields
        let avg_hit_rate = samples
            .iter()
            .map(|s| if s.tier_hit { 1.0 } else { 0.0 })
            .sum::<f32>()
            / count as f32;

        let avg_access_time = samples
            .iter()
            .map(|s| s.operation_latency_ns as f32)
            .sum::<f32>()
            / count as f32;

        let avg_memory_usage =
            samples.iter().map(|s| s.memory_usage as f32).sum::<f32>() / count as f32;

        let avg_ops_per_sec = if count > 0 {
            // Calculate average operations per second from existing per-sample data
            let total_ops_per_sec_x100 = samples
                .iter()
                .map(|s| s.ops_per_second_x100 as f32)
                .sum::<f32>();
            (total_ops_per_sec_x100 / count as f32) / 100.0 // Unscale from x100
        } else {
            0.0
        };

        // Find min/max values using feature-rich fields
        let min_hit_rate = samples
            .iter()
            .map(|s| if s.tier_hit { 1.0 } else { 0.0 })
            .fold(f32::INFINITY, f32::min);

        let max_hit_rate = samples
            .iter()
            .map(|s| if s.tier_hit { 1.0 } else { 0.0 })
            .fold(f32::NEG_INFINITY, f32::max);

        let max_access_time = samples
            .iter()
            .map(|s| s.operation_latency_ns)
            .max()
            .unwrap_or(0);

        let max_memory_usage = samples.iter().map(|s| s.memory_usage).max().unwrap_or(0);

        PerformanceSummary {
            sample_count: count,
            avg_hit_rate,
            min_hit_rate,
            max_hit_rate,
            avg_access_time_ns: avg_access_time as u64,
            max_access_time_ns: max_access_time,
            avg_memory_usage: avg_memory_usage as u64,
            max_memory_usage,
            avg_ops_per_second: avg_ops_per_sec,
            // Error recovery metrics - use error recovery provider data
            total_errors: error_provider.get_total_errors(),
            error_distribution: {
                let dist_map = error_provider.get_error_distribution();
                let mut dist_array = [0.0; 16];
                let total: u32 = dist_map.values().sum();
                if total > 0 {
                    let error_types = [
                        "timeout",
                        "out_of_memory",
                        "serialization",
                        "coherence_violation",
                        "simd_failure",
                        "tier_transition",
                        "worker_failure",
                        "generic_error",
                    ];
                    for (i, &error_type) in error_types.iter().enumerate() {
                        if let Some(&count) = dist_map.get(error_type) {
                            dist_array[i] = count as f64 / total as f64;
                        }
                    }
                }
                dist_array
            },
            health_score: error_provider.get_health_score() as f64,
            active_recoveries: error_provider.get_active_recoveries(),
            mttr_ms: error_provider.get_mttr_ms() as f64,
            // Qualitative analysis flags
            is_fast: avg_access_time < 1_000_000.0, // < 1ms is fast
            is_consistent: {
                // Calculate consistency based on coefficient of variation
                if count < 2 {
                    true // Not enough data to determine inconsistency
                } else {
                    let variance: f64 = self
                        .history_buffer
                        .iter()
                        .map(|sample| {
                            let diff = sample.avg_access_time_ns as f64 - avg_access_time as f64;
                            diff * diff
                        })
                        .sum::<f64>()
                        / (count - 1) as f64;

                    let std_dev = variance.sqrt();
                    let coefficient_of_variation = if avg_access_time > 0.0 {
                        std_dev / avg_access_time as f64
                    } else {
                        0.0
                    };

                    // Consistent if coefficient of variation < 15%
                    coefficient_of_variation < 0.15
                }
            },
            has_outliers: {
                // Detect outliers using IQR method
                if count < 4 {
                    false // Need at least 4 samples for IQR
                } else {
                    let mut access_times: Vec<f64> = self
                        .history_buffer
                        .iter()
                        .map(|sample| sample.avg_access_time_ns as f64)
                        .collect();
                    access_times
                        .sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

                    let q1_idx = (count as f64 * 0.25) as usize;
                    let q3_idx = (count as f64 * 0.75) as usize;
                    let q1 = access_times[q1_idx.min(count - 1)];
                    let q3 = access_times[q3_idx.min(count - 1)];
                    let iqr = q3 - q1;

                    let lower_bound = q1 - 1.5 * iqr;
                    let upper_bound = q3 + 1.5 * iqr;

                    // Check if any samples are outliers
                    access_times
                        .iter()
                        .any(|&time| time < lower_bound || time > upper_bound)
                }
            },
            overhead_acceptable: {
                // Calculate overhead compared to theoretical direct access
                // Estimate direct access time as ~50ns (memory access + basic computation)
                let estimated_direct_access_ns = 50.0;
                let overhead_ratio = if estimated_direct_access_ns > 0.0 {
                    avg_access_time / estimated_direct_access_ns
                } else {
                    1.0
                };

                // Acceptable if cache overhead is less than 20x direct access
                // (accounting for cache benefits like network/disk avoidance)
                overhead_ratio < 20.0
            },
        }
    }

    /// Cleanup old samples based on retention policy
    fn cleanup_old_samples(&mut self) -> Result<(), CacheOperationError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        let last_cleanup = self.retention_policy.last_cleanup.load(Ordering::Relaxed);
        if now - last_cleanup < self.retention_policy.cleanup_frequency {
            return Ok(()); // Not time for cleanup yet
        }

        // Calculate cutoff timestamp for cleanup
        let cutoff_timestamp = now.saturating_sub(self.retention_policy.max_sample_age);

        // Remove samples older than cutoff using timestamp_ns field
        let initial_len = self.history_buffer.len();
        self.history_buffer
            .retain(|sample| sample.timestamp_ns >= cutoff_timestamp);
        let final_len = self.history_buffer.len();

        // Log cleanup results if samples were removed
        if initial_len > final_len {
            let removed_count = initial_len - final_len;
            log::info!(
                "Performance history cleanup: removed {} old samples (cutoff: {} ns)",
                removed_count,
                cutoff_timestamp
            );
        }

        // Update cleanup timestamp
        self.retention_policy
            .last_cleanup
            .store(now, Ordering::Relaxed);

        // Update utilization to reflect actual buffer size after cleanup
        self.utilization
            .store(self.history_buffer.len(), Ordering::Relaxed);

        Ok(())
    }

    /// Clear all history
    pub fn clear(&mut self) {
        self.history_buffer.clear();
        self.write_position.store(0, Ordering::Relaxed);
        self.utilization.store(0, Ordering::Relaxed);
    }
}

/// Enhanced performance summary statistics - canonical implementation combining comprehensive metrics with qualitative analysis
#[derive(Debug, Clone)]
pub struct PerformanceSummary {
    // Core quantitative metrics (from telemetry version - most comprehensive)
    pub sample_count: usize,
    pub avg_hit_rate: f32,
    pub min_hit_rate: f32,
    pub max_hit_rate: f32,
    pub avg_access_time_ns: u64,
    pub max_access_time_ns: u64,
    pub avg_memory_usage: u64,
    pub max_memory_usage: u64,
    pub avg_ops_per_second: f32,

    // Error recovery metrics (from error_recovery version - specialized domain)
    pub total_errors: u64,
    pub error_distribution: [f64; 16],
    pub health_score: f64,
    pub active_recoveries: u32,
    pub mttr_ms: f64,

    // Qualitative analysis flags (from analysis version - grading capability)
    pub is_fast: bool,
    pub is_consistent: bool,
    pub has_outliers: bool,
    pub overhead_acceptable: bool,
}

impl Default for PerformanceSummary {
    fn default() -> Self {
        Self {
            // Core quantitative metrics
            sample_count: 0,
            avg_hit_rate: 0.0,
            min_hit_rate: 0.0,
            max_hit_rate: 0.0,
            avg_access_time_ns: 0,
            max_access_time_ns: 0,
            avg_memory_usage: 0,
            max_memory_usage: 0,
            avg_ops_per_second: 0.0,

            // Error recovery metrics
            total_errors: 0,
            error_distribution: [0.0; 16],
            health_score: 1.0, // Perfect health by default
            active_recoveries: 0,
            mttr_ms: 0.0,

            // Qualitative analysis flags (optimistic defaults)
            is_fast: true,
            is_consistent: true,
            has_outliers: false,
            overhead_acceptable: true,
        }
    }
}

impl PerformanceSummary {
    /// Get overall performance grade based on qualitative flags (from analysis version)
    pub fn grade(&self) -> char {
        let score = self.is_fast as u8
            + self.is_consistent as u8
            + (!self.has_outliers) as u8
            + self.overhead_acceptable as u8;

        match score {
            4 => 'A',
            3 => 'B',
            2 => 'C',
            1 => 'D',
            _ => 'F',
        }
    }

    /// Calculate qualitative flags from quantitative metrics
    pub fn update_qualitative_analysis(
        &mut self,
        std_deviation_ns: f64,
        p99_duration_ns: u64,
        overhead_percent: f64,
    ) {
        self.is_fast = self.avg_access_time_ns < 1_000_000; // Less than 1ms average
        self.is_consistent = std_deviation_ns < (self.avg_access_time_ns as f64 * 0.5); // StdDev < 50% of average
        self.has_outliers = p99_duration_ns > (self.avg_access_time_ns * 10); // P99 > 10x average
        self.overhead_acceptable = overhead_percent < 10.0; // Less than 10% overhead
    }

    /// Update error recovery metrics (for error_recovery compatibility)
    pub fn update_error_metrics(
        &mut self,
        total_errors: u64,
        error_distribution: [f64; 16],
        health_score: f64,
        active_recoveries: u32,
        mttr_ms: f64,
    ) {
        self.total_errors = total_errors;
        self.error_distribution = error_distribution;
        self.health_score = health_score;
        self.active_recoveries = active_recoveries;
        self.mttr_ms = mttr_ms;
    }
}
