//! Performance monitoring and optimization module
//!
//! This module provides comprehensive performance monitoring, trend analysis,
//! alerting, and history management for cache operations.

pub mod alert_system;
pub mod core;
pub mod metrics_collector;
pub mod types;

// Re-export main types and structs
pub use core::{
    PerformanceMonitor, PerformanceRecommendation, PerformanceReport, RecommendationCategory,
    RecommendationPriority,
};

pub use alert_system::{AlertSystem, AlertSystemStats};
pub use metrics_collector::{MetricsBufferStats, MetricsCollector};
pub use types::*;
