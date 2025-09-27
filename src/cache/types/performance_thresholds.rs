//! Performance alert thresholds with atomic thread-safe operations
//!
//! This module provides the canonical AlertThresholds implementation
//! optimized for high-performance concurrent cache operations.

#![allow(dead_code)] // Performance Types - Complete atomic alert thresholds library with 6-decimal precision, thread-safe operations, and performance monitoring

use std::sync::atomic::{AtomicU64, Ordering};

/// Alert thresholds configuration - CANONICAL VERSION
///
/// Thread-safe atomic implementation with 6-decimal precision storage.
/// All values stored as integers scaled by 1,000,000 for precision.
#[derive(Debug)]
pub struct AlertThresholds {
    /// Critical hit rate threshold (stored as u64 with 6 decimal places)
    pub critical_hit_rate: AtomicU64,
    /// Warning hit rate threshold (stored as u64 with 6 decimal places)
    pub warning_hit_rate: AtomicU64,
    /// Maximum acceptable latency in nanoseconds
    pub max_latency_ns: AtomicU64,
    /// Memory warning threshold (stored as u64 with 6 decimal places)
    pub memory_warning_threshold: AtomicU64,
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            critical_hit_rate: AtomicU64::new(500_000), // 0.5 stored as 500_000
            warning_hit_rate: AtomicU64::new(700_000),  // 0.7 stored as 700_000
            max_latency_ns: AtomicU64::new(10_000_000), // 10ms
            memory_warning_threshold: AtomicU64::new(850_000), // 0.85 stored as 850_000
        }
    }
}

impl AlertThresholds {
    /// Get critical hit rate as f64 (0.0 to 1.0)
    #[inline]
    pub fn critical_hit_rate_f64(&self) -> f64 {
        self.critical_hit_rate.load(Ordering::Relaxed) as f64 / 1_000_000.0
    }

    /// Set critical hit rate from f64 (0.0 to 1.0)
    #[inline]
    pub fn set_critical_hit_rate(&self, rate: f64) {
        self.critical_hit_rate
            .store((rate * 1_000_000.0) as u64, Ordering::Relaxed);
    }

    /// Get memory warning threshold as f64 (0.0 to 1.0)
    #[inline]
    pub fn memory_threshold_f64(&self) -> f64 {
        self.memory_warning_threshold.load(Ordering::Relaxed) as f64 / 1_000_000.0
    }

    /// Set memory warning threshold from f64 (0.0 to 1.0)
    #[inline]
    pub fn set_memory_threshold(&self, threshold: f64) {
        self.memory_warning_threshold
            .store((threshold * 1_000_000.0) as u64, Ordering::Relaxed);
    }

    /// Get warning hit rate as f64 (0.0 to 1.0)
    #[inline]
    pub fn warning_hit_rate_f64(&self) -> f64 {
        self.warning_hit_rate.load(Ordering::Relaxed) as f64 / 1_000_000.0
    }

    /// Set warning hit rate from f64 (0.0 to 1.0)
    #[inline]
    pub fn set_warning_hit_rate(&self, rate: f64) {
        self.warning_hit_rate
            .store((rate * 1_000_000.0) as u64, Ordering::Relaxed);
    }

    /// Get maximum latency in nanoseconds
    #[inline]
    pub fn max_latency_ns(&self) -> u64 {
        self.max_latency_ns.load(Ordering::Relaxed)
    }

    /// Set maximum latency in nanoseconds
    #[inline]
    pub fn set_max_latency_ns(&self, latency_ns: u64) {
        self.max_latency_ns.store(latency_ns, Ordering::Relaxed);
    }
}
