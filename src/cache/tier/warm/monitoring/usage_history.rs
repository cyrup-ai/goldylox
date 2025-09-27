//! Historical memory usage tracking and pattern analysis
//!
//! This module implements circular buffer storage for memory usage samples
//! and provides trend analysis capabilities.

#![allow(dead_code)] // Warm tier monitoring - Complete usage history tracking library

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use crossbeam_utils::CachePadded;

use super::trend_analysis::TrendAnalysis;
use crate::cache::traits::types_and_enums::CacheOperationError;

/// Historical memory usage tracking
#[derive(Debug)]
pub struct MemoryUsageHistory {
    /// Circular buffer of usage samples
    pub samples: [CachePadded<AtomicU64>; 256],
    /// Actual capacity being used (may be less than 256)
    pub actual_capacity: usize,
    /// Current position in circular buffer
    pub position: CachePadded<AtomicUsize>,
    /// Sample timestamps
    pub timestamps: [CachePadded<AtomicU64>; 256],
    /// Trend analysis results
    pub trend_analysis: TrendAnalysis,
}

impl Default for MemoryUsageHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryUsageHistory {
    pub fn new() -> Self {
        Self::new_with_capacity(256, true)
    }

    /// Create with specified capacity (up to 256)
    pub fn new_with_capacity(capacity: usize, _leak_detection_active: bool) -> Self {
        let actual_capacity = capacity.min(256);

        // Initialize arrays directly - cannot fail
        let samples = [const { CachePadded::new(AtomicU64::new(0)) }; 256];
        let timestamps = [const { CachePadded::new(AtomicU64::new(0)) }; 256];

        Self {
            samples,
            actual_capacity,
            position: CachePadded::new(AtomicUsize::new(0)),
            timestamps,
            trend_analysis: TrendAnalysis::new_with_config(true),
        }
    }

    /// Record memory usage sample
    pub fn record_usage(&self, usage: u64, timestamp_ns: u64) {
        let pos = self.position.fetch_add(1, Ordering::Relaxed) % 256;
        self.samples[pos].store(usage, Ordering::Relaxed);
        self.timestamps[pos].store(timestamp_ns, Ordering::Relaxed);
    }

    /// Update trend analysis based on recent samples
    pub fn update_trend_analysis(&self) -> Result<(), CacheOperationError> {
        let current_pos = self.position.load(Ordering::Relaxed);

        // Collect recent samples for analysis
        let mut recent_samples = Vec::with_capacity(32);
        let mut recent_timestamps = Vec::with_capacity(32);

        for i in 0..32 {
            let idx = (current_pos + 256 - 32 + i) % 256;
            let usage = self.samples[idx].load(Ordering::Relaxed);
            let timestamp = self.timestamps[idx].load(Ordering::Relaxed);

            if timestamp > 0 {
                recent_samples.push(usage);
                recent_timestamps.push(timestamp);
            }
        }

        if recent_samples.len() < 8 {
            return Ok(()); // Not enough data for trend analysis
        }

        self.trend_analysis
            .analyze_samples(&recent_samples, &recent_timestamps)?;

        Ok(())
    }

    /// Get usage samples within a time window
    pub fn get_samples_in_window(
        &self,
        window_start_ns: u64,
        window_end_ns: u64,
    ) -> Vec<(u64, u64)> {
        let mut samples_in_window = Vec::new();
        let current_pos = self.position.load(Ordering::Relaxed);

        // Search through all samples (circular buffer might wrap)
        for i in 0..256 {
            let idx = (current_pos + 256 - i) % 256;
            let usage = self.samples[idx].load(Ordering::Relaxed);
            let timestamp = self.timestamps[idx].load(Ordering::Relaxed);

            if timestamp >= window_start_ns && timestamp <= window_end_ns {
                samples_in_window.push((timestamp, usage));
            } else if timestamp < window_start_ns {
                // Since we're going backwards in time, we can stop here
                break;
            }
        }

        // Sort by timestamp (oldest first)
        samples_in_window.sort_by_key(|&(timestamp, _)| timestamp);
        samples_in_window
    }

    /// Calculate average usage over a time period
    pub fn average_usage_in_period(&self, period_ns: u64) -> Option<u64> {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let window_start = current_time.saturating_sub(period_ns);
        let samples = self.get_samples_in_window(window_start, current_time);

        if samples.is_empty() {
            return None;
        }

        let total_usage: u64 = samples.iter().map(|(_, usage)| *usage).sum();
        Some(total_usage / samples.len() as u64)
    }

    /// Get peak usage in a time period
    pub fn peak_usage_in_period(&self, period_ns: u64) -> Option<u64> {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let window_start = current_time.saturating_sub(period_ns);
        let samples = self.get_samples_in_window(window_start, current_time);

        samples.iter().map(|(_, usage)| *usage).max()
    }

    /// Get the trend analysis instance
    pub fn get_trend_analysis(&self) -> &TrendAnalysis {
        &self.trend_analysis
    }
}
