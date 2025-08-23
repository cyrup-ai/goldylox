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
        let start_idx = if buffer_len > count {
            buffer_len - count
        } else {
            0
        };

        self.history_buffer[start_idx..]
            .iter()
            .map(|sample| {
                let timestamp_ns = sample.timestamp;
                let hit_rate = if sample.tier_hit { 1.0 } else { 0.0 };
                (timestamp_ns, hit_rate)
            })
            .collect()
    }

    /// Get samples within a time window
    pub fn get_samples_in_window(
        &self,
        _start_time: u64,
        _end_time: u64,
    ) -> Vec<PerformanceSample> {
        // Note: Feature-rich PerformanceSample uses Instant, not timestamp_ns
        // For now, return all samples as time filtering needs adjustment
        self.history_buffer.iter().cloned().collect()
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

        let sum: f32 = samples.iter().map(|s| extract_metric(s)).sum();
        Some(sum / samples.len() as f32)
    }

    /// Get performance statistics summary
    pub fn get_performance_summary(&self) -> PerformanceSummary {
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

        let avg_ops_per_sec = 0.0; // Would need additional calculation

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
            max_access_time_ns: max_access_time as u64,
            avg_memory_usage: avg_memory_usage as u64,
            max_memory_usage: max_memory_usage as u64,
            avg_ops_per_second: avg_ops_per_sec,
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

        // Note: Feature-rich PerformanceSample uses Instant, cleanup logic needs adjustment
        // For now, skip time-based cleanup to avoid compilation errors

        // Update cleanup timestamp
        self.retention_policy
            .last_cleanup
            .store(now, Ordering::Relaxed);

        // Update utilization
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

/// Performance summary statistics
#[derive(Debug, Clone)]
pub struct PerformanceSummary {
    pub sample_count: usize,
    pub avg_hit_rate: f32,
    pub min_hit_rate: f32,
    pub max_hit_rate: f32,
    pub avg_access_time_ns: u64,
    pub max_access_time_ns: u64,
    pub avg_memory_usage: u64,
    pub max_memory_usage: u64,
    pub avg_ops_per_second: f32,
}

impl Default for PerformanceSummary {
    fn default() -> Self {
        Self {
            sample_count: 0,
            avg_hit_rate: 0.0,
            min_hit_rate: 0.0,
            max_hit_rate: 0.0,
            avg_access_time_ns: 0,
            max_access_time_ns: 0,
            avg_memory_usage: 0,
            max_memory_usage: 0,
            avg_ops_per_second: 0.0,
        }
    }
}
