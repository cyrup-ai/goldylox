//! Statistics module decomposition for cache coherence protocol
//!
//! This module provides comprehensive statistics collection and monitoring
//! capabilities decomposed into logical units for better maintainability.

// Module declarations
pub mod core_statistics;
pub mod operation_metrics;
pub mod performance_metrics;
pub mod tier_statistics;

// Re-export main types for convenience
pub use core_statistics::{CoherenceStatistics, CoherenceStatisticsSnapshot};
pub use operation_metrics::OperationMetrics;
pub use performance_metrics::CoherencePerformanceMetrics;
pub use tier_statistics::{TierCoherenceStats, TierSpecificStats};
