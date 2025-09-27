//! Adaptive thresholds for strategy switching
//!
//! This module provides adaptive threshold management for cache strategy
//! switching decisions with system load and stability considerations.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

/// Adaptive thresholds for strategy switching decisions
#[derive(Debug)]
pub struct StrategyThresholds {
    /// Minimum hit rate improvement required for switch (x1000)
    #[allow(dead_code)]
    // Strategy management - min_hit_rate_improvement used in strategy switching decisions
    min_hit_rate_improvement: AtomicU32,
    /// Maximum acceptable access time increase (nanoseconds)
    #[allow(dead_code)]
    // Strategy management - max access time thresholds used in performance-based strategy switching
    max_access_time_increase: AtomicU64,
    /// Minimum evaluation period before strategy switch
    #[allow(dead_code)]
    // Strategy management - evaluation periods used in strategy switching timing
    min_evaluation_period: Duration,
    /// Strategy switching cooldown period
    switch_cooldown: Duration,
    /// Performance degradation threshold for emergency switch
    emergency_threshold: AtomicU32,
}

impl Default for StrategyThresholds {
    fn default() -> Self {
        Self::new()
    }
}

impl StrategyThresholds {
    /// Create new adaptive thresholds
    #[inline]
    pub fn new() -> Self {
        Self {
            min_hit_rate_improvement: AtomicU32::new(50), // 5% improvement required
            max_access_time_increase: AtomicU64::new(100_000), // 100µs max increase
            min_evaluation_period: Duration::from_secs(10),
            switch_cooldown: Duration::from_secs(60),
            emergency_threshold: AtomicU32::new(200), // 20% performance drop
        }
    }

    /// Check if strategy switch is beneficial
    #[inline(always)]
    pub fn should_switch(&self, current_score: u32, alternative_score: u32) -> bool {
        let min_improvement = self.min_hit_rate_improvement.load(Ordering::Relaxed);
        alternative_score > current_score + min_improvement
    }

    /// Check if emergency switch is needed
    #[inline(always)]
    pub fn is_emergency(&self, current_score: u32, reference_score: u32) -> bool {
        let threshold = self.emergency_threshold.load(Ordering::Relaxed);
        reference_score > current_score + threshold
    }

    /// Get switch cooldown duration
    #[inline(always)]
    pub fn switch_cooldown(&self) -> Duration {
        self.switch_cooldown
    }

    /// Adapt thresholds based on system performance
    #[inline]
    pub fn adapt(&self, system_load: f32, stability_ratio: f32) {
        // Increase thresholds under high load to reduce switching
        let load_factor = (system_load * 100.0) as u32;
        let stability_factor = (stability_ratio * 50.0) as u32;

        let new_improvement = (50 + load_factor + stability_factor).min(200);
        self.min_hit_rate_improvement
            .store(new_improvement, Ordering::Relaxed);

        let new_emergency = (200 + load_factor).min(500);
        self.emergency_threshold
            .store(new_emergency, Ordering::Relaxed);
    }
}
