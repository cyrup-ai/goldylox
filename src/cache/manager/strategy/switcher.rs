//! Strategy switching logic with hysteresis and stability
//!
//! This module provides intelligent strategy switching with stability
//! considerations and emergency mode handling.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Instant;

use crossbeam_utils::atomic::AtomicCell;

use super::core::CacheStrategy;
use super::metrics::StrategyMetrics;
use super::thresholds::StrategyThresholds;

/// Strategy switching logic with hysteresis and stability
#[derive(Debug)]
pub struct StrategySwitcher {
    /// Current strategy performance score
    current_score: AtomicU32,
    /// Best alternative strategy and its score
    best_alternative: AtomicCell<(CacheStrategy, u32)>,
    /// Switch decision timestamps
    last_switch: AtomicCell<Instant>,
    /// Strategy stability counter
    stability_counter: AtomicU32,
    /// Emergency mode flag
    emergency_mode: AtomicBool,
}

impl Default for StrategySwitcher {
    fn default() -> Self {
        Self::new()
    }
}

impl StrategySwitcher {
    /// Create new strategy switcher
    #[inline]
    pub fn new() -> Self {
        Self {
            current_score: AtomicU32::new(0),
            best_alternative: AtomicCell::new((CacheStrategy::AdaptiveLFU, 0)),
            last_switch: AtomicCell::new(Instant::now()),
            stability_counter: AtomicU32::new(0),
            emergency_mode: AtomicBool::new(false),
        }
    }

    /// Evaluate strategy performance and decide on switches
    #[inline]
    pub fn evaluate_switch(
        &self,
        current_strategy: CacheStrategy,
        metrics: &StrategyMetrics,
        thresholds: &StrategyThresholds,
    ) -> Option<CacheStrategy> {
        let current_score = metrics.performance_score(current_strategy);
        self.current_score.store(current_score, Ordering::Relaxed);

        // Find best alternative strategy
        let mut best_strategy = current_strategy;
        let mut best_score = current_score;

        for &strategy in &[
            CacheStrategy::AdaptiveLRU,
            CacheStrategy::AdaptiveLFU,
            CacheStrategy::TwoQueue,
            CacheStrategy::ARC,
            CacheStrategy::MLPredictive,
        ] {
            if strategy != current_strategy {
                let score = metrics.performance_score(strategy);
                if score > best_score {
                    best_strategy = strategy;
                    best_score = score;
                }
            }
        }

        self.best_alternative.store((best_strategy, best_score));

        // Check cooldown period
        let now = Instant::now();
        let last_switch = self.last_switch.load();
        if now.duration_since(last_switch) < thresholds.switch_cooldown() {
            return None;
        }

        // Check emergency conditions
        let emergency = thresholds.is_emergency(current_score, best_score);
        if emergency {
            self.emergency_mode.store(true, Ordering::Relaxed);
            self.last_switch.store(now);
            self.stability_counter.store(0, Ordering::Relaxed);
            return Some(best_strategy);
        }

        // Check normal switching conditions
        if thresholds.should_switch(current_score, best_score) {
            self.emergency_mode.store(false, Ordering::Relaxed);
            self.last_switch.store(now);
            self.stability_counter.fetch_add(1, Ordering::Relaxed);
            return Some(best_strategy);
        }

        None
    }

    /// Get current stability ratio (0.0 = unstable, 1.0 = very stable)
    #[allow(dead_code)] // Strategy management - stability_ratio used in strategy stability analysis
    #[inline(always)]
    pub fn stability_ratio(&self) -> f32 {
        let counter = self.stability_counter.load(Ordering::Relaxed);
        (counter as f32 / 10.0).min(1.0)
    }

    /// Check if currently in emergency mode
    #[allow(dead_code)] // Strategy management - is_emergency_mode used in emergency strategy detection
    #[inline(always)]
    pub fn is_emergency_mode(&self) -> bool {
        self.emergency_mode.load(Ordering::Relaxed)
    }
}
