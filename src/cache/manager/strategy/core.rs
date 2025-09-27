//! Core cache strategy selection and management
//!
//! This module provides the main CacheStrategy enum and CacheStrategySelector
//! for intelligent cache strategy selection with performance monitoring.

use crossbeam_utils::atomic::AtomicCell;

use super::metrics::StrategyMetrics;
use super::switcher::StrategySwitcher;
use super::thresholds::StrategyThresholds;

/// Cache strategy algorithms for tier placement and eviction decisions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheStrategy {
    /// Adaptive Least Recently Used with machine learning
    AdaptiveLRU,
    /// Adaptive Least Frequently Used with pattern recognition
    AdaptiveLFU,
    /// Two-Queue algorithm for scan-resistant caching
    TwoQueue,
    /// Adaptive Replacement Cache (ARC) with self-tuning
    ARC,
    /// Machine Learning-based predictive caching
    MLPredictive,
}

/// Cache strategy selector for tier decisions
#[derive(Debug)]
pub struct CacheStrategySelector {
    /// Current cache strategy
    current_strategy: AtomicCell<CacheStrategy>,
    /// Strategy performance metrics
    strategy_metrics: StrategyMetrics,
    /// Strategy threshold parameters
    #[allow(dead_code)] // Strategy management - thresholds used in adaptive strategy selection
    strategy_thresholds: StrategyThresholds,
    /// Strategy switching logic
    #[allow(dead_code)]
    // Strategy management - switcher used in performance-based strategy changes
    strategy_switcher: StrategySwitcher,
}

impl CacheStrategy {
    /// Get strategy index for metrics arrays
    #[inline(always)]
    pub const fn index(self) -> usize {
        match self {
            CacheStrategy::AdaptiveLRU => 0,
            CacheStrategy::AdaptiveLFU => 1,
            CacheStrategy::TwoQueue => 2,
            CacheStrategy::ARC => 3,
            CacheStrategy::MLPredictive => 4,
        }
    }

    /// Get strategy name for debugging
    #[allow(dead_code)] // Strategy management - name used in strategy identification and debugging
    #[inline(always)]
    pub const fn name(self) -> &'static str {
        match self {
            CacheStrategy::AdaptiveLRU => "AdaptiveLRU",
            CacheStrategy::AdaptiveLFU => "AdaptiveLFU",
            CacheStrategy::TwoQueue => "TwoQueue",
            CacheStrategy::ARC => "ARC",
            CacheStrategy::MLPredictive => "MLPredictive",
        }
    }
}

impl Default for CacheStrategy {
    #[inline(always)]
    fn default() -> Self {
        CacheStrategy::AdaptiveLRU
    }
}

impl Default for CacheStrategySelector {
    fn default() -> Self {
        Self::new()
    }
}

impl CacheStrategySelector {
    /// Create new cache strategy selector
    #[allow(dead_code)] // Strategy management - strategy selector constructor for cache optimization
    pub fn new() -> Self {
        Self {
            current_strategy: AtomicCell::new(CacheStrategy::default()),
            strategy_metrics: StrategyMetrics::new(),
            strategy_thresholds: StrategyThresholds::new(),
            strategy_switcher: StrategySwitcher::new(),
        }
    }

    /// Get current cache strategy
    #[inline(always)]
    pub fn current_strategy(&self) -> CacheStrategy {
        self.current_strategy.load()
    }

    /// Record cache operation for strategy evaluation
    #[inline(always)]
    #[allow(dead_code)] // Strategy management - operation recording API for strategy performance tracking
    pub fn record_operation(&self, strategy: CacheStrategy, hit: bool, access_time_ns: u64) {
        if hit {
            self.strategy_metrics.record_hit(strategy, access_time_ns);
        } else {
            self.strategy_metrics.record_miss(strategy, access_time_ns);
        }
    }

    /// Evaluate and potentially switch cache strategy
    #[inline]
    #[allow(dead_code)] // Strategy management - strategy evaluation used in adaptive performance tuning
    pub fn evaluate_strategy_switch(&self) -> Option<CacheStrategy> {
        let current = self.current_strategy();
        self.strategy_switcher.evaluate_switch(
            current,
            &self.strategy_metrics,
            &self.strategy_thresholds,
        )
    }

    /// Switch to new cache strategy
    #[inline]
    #[allow(dead_code)] // Strategy management - strategy switching used in performance optimization
    pub fn switch_strategy(&self, new_strategy: CacheStrategy) {
        self.current_strategy.store(new_strategy);
        self.strategy_metrics.reset_evaluation();
    }

    /// Get strategy performance metrics
    #[inline(always)]
    #[allow(dead_code)] // Strategy management - metrics used in performance analysis
    pub fn get_metrics(&self) -> &StrategyMetrics {
        &self.strategy_metrics
    }

    /// Adapt strategy thresholds based on system conditions
    #[inline]
    #[allow(dead_code)] // Strategy management - threshold adaptation used in dynamic tuning
    pub fn adapt_thresholds(&self, system_load: f32, stability_ratio: f32) {
        self.strategy_thresholds.adapt(system_load, stability_ratio);
    }
}
