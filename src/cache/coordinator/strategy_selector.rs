//! Cache strategy selector for optimal tier placement decisions
//!
//! This module implements intelligent cache strategy selection using atomic operations
//! and adaptive thresholds to optimize cache performance across different workloads.

// Internal coordinator architecture - components may not be used in minimal API

use crossbeam_utils::atomic::AtomicCell;

use crate::cache::config::CacheConfig;
use crate::cache::manager::strategy::{
    CacheStrategy, StrategyMetrics, StrategySwitcher, StrategyThresholds,
};
use crate::cache::traits::types_and_enums::CacheOperationError;

/// Cache strategy selector for optimal tier placement decisions
#[derive(Debug)]
pub struct CacheStrategySelector {
    /// Current active cache strategy (lock-free atomic)
    current_strategy: AtomicCell<CacheStrategy>,
    /// Strategy performance metrics with atomic counters
    strategy_metrics: StrategyMetrics,
    /// Strategy threshold parameters for switching decisions
    #[allow(dead_code)]
    // Cache coordination - strategy_thresholds used in strategy switching decisions
    strategy_thresholds: StrategyThresholds,
    /// Strategy switching logic with adaptive thresholds
    #[allow(dead_code)]
    // Cache coordination - strategy_switcher used in adaptive strategy switching
    strategy_switcher: StrategySwitcher,
}

impl CacheStrategySelector {
    /// Create new cache strategy selector with configuration
    pub fn new(_config: &CacheConfig) -> Result<Self, CacheOperationError> {
        let current_strategy = AtomicCell::new(CacheStrategy::AdaptiveLRU);
        let strategy_metrics = StrategyMetrics::new();
        let strategy_thresholds = StrategyThresholds::new();
        let strategy_switcher = StrategySwitcher::new();

        Ok(Self {
            current_strategy,
            strategy_metrics,
            strategy_thresholds,
            strategy_switcher,
        })
    }

    /// Get current active cache strategy
    pub fn current_strategy(&self) -> CacheStrategy {
        self.current_strategy.load()
    }

    /// Update strategy based on performance metrics
    #[allow(dead_code)] // Cache coordination - update_strategy used in adaptive strategy management
    pub fn update_strategy(&self) -> Result<(), CacheOperationError> {
        let current = self.current_strategy();
        let metrics = &self.strategy_metrics;

        if let Some(new_strategy) =
            self.strategy_switcher
                .evaluate_switch(current, metrics, &self.strategy_thresholds)
        {
            self.current_strategy.store(new_strategy);
            self.strategy_metrics.reset_evaluation();
        }

        Ok(())
    }

    /// Record strategy performance for adaptive optimization
    pub fn record_strategy_performance(&self, hit_rate: f64, latency_ns: u64) {
        let strategy = self.current_strategy();
        if hit_rate > 0.0 {
            self.strategy_metrics.record_hit(strategy, latency_ns);
        } else {
            self.strategy_metrics.record_miss(strategy, latency_ns);
        }
    }

    /// Get strategy performance metrics
    #[allow(dead_code)] // Library API - may be used by external consumers
    pub fn get_metrics(&self) -> &StrategyMetrics {
        &self.strategy_metrics
    }

    /// Get strategy thresholds
    #[allow(dead_code)] // Library API - may be used by external consumers
    pub fn get_thresholds(&self) -> &StrategyThresholds {
        &self.strategy_thresholds
    }

    /// Force strategy change (for testing or manual optimization)
    #[allow(dead_code)] // Library API - may be used by external consumers
    pub fn force_strategy(&self, strategy: CacheStrategy) {
        let _old_strategy = self.current_strategy.swap(strategy);
        self.strategy_metrics.reset_evaluation();
    }
}
