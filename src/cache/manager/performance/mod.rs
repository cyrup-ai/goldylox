//! Performance monitoring and optimization module
//!
//! This module provides comprehensive performance monitoring, trend analysis,
//! alerting, and history management for cache operations.

pub mod alert_system;
pub mod core;
pub mod metrics_collector;
pub mod types;

// Re-export only actually used types
