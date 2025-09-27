#![allow(dead_code)]
// Telemetry System - Complete performance monitoring and telemetry library with unified statistics, trend analysis, performance history, alerting, and comprehensive measurement capabilities

//! Telemetry module - consolidated performance tracking, measurement, and monitoring
//!
//! This module provides comprehensive telemetry functionality combining the best features
//! from performance_tracking, measurement, and cache/performance_tracking modules.
//!
//! Features:

//! - Zero-contention performance monitoring with atomic operations
//! - Advanced alert systems with pattern recognition
//! - Comprehensive metrics collection and analysis
//! - Trend analysis and prediction
//! - Memory-efficient data structures
//! - Real-time performance history tracking

// Import core types - use specific imports to avoid conflicts (crate private)

// PrecisionTimer available from crate::cache::types::performance::timer::PrecisionTimer when needed
// Import canonical CacheEntry from traits module (crate private)

// Module declarations for comprehensive telemetry functionality
// REMOVED: pub mod alert_system; (canonicalized to cache::manager::performance::alert_system)
// REMOVED: pub mod alerts; (canonicalized to cache::manager::performance::alert_system)
pub(crate) mod cache;
pub(crate) mod data_structures;
pub(crate) mod history;
// REMOVED: pub mod metrics; (canonicalized to cache::manager::performance::metrics_collector)
// REMOVED: pub mod metrics_collector; (canonicalized to cache::manager::performance::metrics_collector)
pub(crate) mod monitor;
pub(crate) mod performance_history;
// REMOVED: pub mod statistics; (canonicalized to unified_stats.rs)
// REMOVED: pub mod trend_analyzer; (file not found - canonicalized with trends.rs)
pub(crate) mod trends;
pub(crate) mod types;
pub(crate) mod unified_stats;

// - Use telemetry::metrics::MetricsCollector
// - Use telemetry::monitor::{CachePerformanceMonitor, PerformanceMonitor}
// - Use telemetry::performance_history::*
// - Use telemetry::statistics::UnifiedCacheStatistics
// - Use telemetry::trends::TrendAnalyzer
// - Use telemetry::types::* directly
