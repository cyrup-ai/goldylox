//! System configuration builder methods
//!
//! This module provides builder methods for configuring monitoring, worker, and analyzer
//! systems of the cache with fluent API patterns.

use crate::cache::config::types::AlertThresholdsConfig;
use super::core::CacheConfigBuilder;

impl CacheConfigBuilder {
    /// Enable or disable performance monitoring
    #[inline(always)]
    pub const fn monitoring_enabled(mut self, enabled: bool) -> Self {
        self.config.monitoring.enabled = enabled;
        self
    }

    /// Set monitoring sample interval in nanoseconds
    #[inline(always)]
    pub const fn monitoring_interval_ns(mut self, interval_ns: u64) -> Self {
        self.config.monitoring.sample_interval_ns = interval_ns;
        self
    }

    /// Set monitoring max history samples
    #[inline(always)]
    pub const fn max_history_samples(mut self, samples: u32) -> Self {
        self.config.monitoring.max_history_samples = samples;
        self
    }

    /// Enable or disable alerts
    #[inline(always)]
    pub const fn enable_alerts(mut self, enabled: bool) -> Self {
        self.config.monitoring.enable_alerts = enabled;
        self
    }

    /// Enable or disable tracing
    #[inline(always)]
    pub const fn enable_tracing(mut self, enabled: bool) -> Self {
        self.config.monitoring.enable_tracing = enabled;
        self
    }

    /// Set alert thresholds
    #[inline(always)]
    pub const fn alert_thresholds(mut self, thresholds: AlertThresholdsConfig) -> Self {
        self.config.monitoring.alert_thresholds = thresholds;
        self
    }

    /// Set metrics frequency in Hz
    #[inline(always)]
    pub const fn metrics_frequency_hz(mut self, frequency: u16) -> Self {
        self.config.monitoring.metrics_frequency_hz = frequency;
        self
    }

    /// Enable or disable background worker
    #[inline(always)]
    pub const fn worker_enabled(mut self, enabled: bool) -> Self {
        self.config.worker.enabled = enabled;
        self
    }

    /// Set worker thread pool size
    #[inline(always)]
    pub const fn worker_threads(mut self, threads: u8) -> Self {
        self.config.worker.thread_pool_size = threads;
        self
    }

    /// Set task queue capacity
    #[inline(always)]
    pub const fn task_queue_capacity(mut self, capacity: u32) -> Self {
        self.config.worker.task_queue_capacity = capacity;
        self
    }

    /// Set maintenance interval in nanoseconds
    #[inline(always)]
    pub const fn maintenance_interval_ns(mut self, interval_ns: u64) -> Self {
        self.config.worker.maintenance_interval_ns = interval_ns;
        self
    }

    /// Enable or disable auto tier management
    #[inline(always)]
    pub const fn auto_tier_management(mut self, enabled: bool) -> Self {
        self.config.worker.auto_tier_management = enabled;
        self
    }

    /// Set CPU affinity mask
    #[inline(always)]
    pub const fn cpu_affinity_mask(mut self, mask: u64) -> Self {
        self.config.worker.cpu_affinity_mask = mask;
        self
    }

    /// Set priority level
    #[inline(always)]
    pub const fn priority_level(mut self, level: u8) -> Self {
        self.config.worker.priority_level = level;
        self
    }

    /// Set batch size
    #[inline(always)]
    pub const fn batch_size(mut self, size: u16) -> Self {
        self.config.worker.batch_size = size;
        self
    }

    /// Set analyzer maximum tracked keys
    #[inline(always)]
    pub const fn analyzer_max_keys(mut self, max_keys: usize) -> Self {
        self.config.analyzer.max_tracked_keys = max_keys;
        self
    }

    /// Set analyzer frequency decay constant (nanoseconds)
    #[inline(always)]
    pub const fn analyzer_frequency_decay(mut self, decay_ns: f64) -> Self {
        self.config.analyzer.frequency_decay_constant = decay_ns;
        self
    }

    /// Set analyzer recency half-life (nanoseconds)
    #[inline(always)]
    pub const fn analyzer_recency_half_life(mut self, half_life_ns: f64) -> Self {
        self.config.analyzer.recency_half_life = half_life_ns;
        self
    }
}
