//! Statistics module decomposition for cache coherence protocol
//!
//! This module provides comprehensive statistics collection and monitoring
//! capabilities decomposed into logical units for better maintainability.

// Internal statistics architecture - components may not be used in minimal API

// Module declarations
pub mod core_statistics;

// Re-export only actually used types
pub use core_statistics::{CoherenceStatistics, CoherenceStatisticsSnapshot};
