//! Cache strategy selection and management
//!
//! This module handles intelligent cache strategy selection with performance monitoring
//! and adaptive switching between different caching algorithms.

pub mod core;
pub mod metrics;
pub mod switcher;
pub mod thresholds;

pub use core::CacheStrategy;

pub use metrics::StrategyMetrics;
pub use switcher::StrategySwitcher;
pub use thresholds::StrategyThresholds;
