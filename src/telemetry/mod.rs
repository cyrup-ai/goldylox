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

// Import core types - use specific imports to avoid conflicts
pub use crate::cache::types::core_types::{
    timestamp_nanos, BatchOperation, BatchRequest, BatchResult, CacheResult,
};
use crate::cache::types::statistics::tier_stats::TierStatistics;
// PrecisionTimer available from crate::cache::types::performance::timer::PrecisionTimer when needed
// Import canonical CacheEntry from traits module
pub use crate::cache::traits::cache_entry::CacheEntry;


// Module declarations for comprehensive telemetry functionality
pub mod alert_system;
pub mod alerts;
pub mod cache;
pub mod data_structures;
pub mod history;
pub mod metrics;
pub mod metrics_collector;
pub mod monitor;
pub mod performance_history;
pub mod statistics;
pub mod trend_analyzer;
pub mod trends;
pub mod types;
pub mod unified_stats;

// REMOVED: Compatibility re-exports that hide canonical API paths
// These re-exports enabled broken code to compile by providing aliases
// Users must now import from canonical module paths:
// - Use telemetry::alert_system::AlertSystem
// - Use telemetry::alerts::AlertSystem
// - Use telemetry::data_structures::{AlertPatternState, CollectionState, ...}
// - Use telemetry::history::PerformanceHistory
// - Use telemetry::metrics::MetricsCollector
// - Use telemetry::monitor::{CachePerformanceMonitor, PerformanceMonitor}
// - Use telemetry::performance_history::*
// - Use telemetry::statistics::UnifiedCacheStatistics
// - Use telemetry::trends::TrendAnalyzer
// - Use telemetry::types::* directly
