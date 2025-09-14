//! Statistics module decomposition
//!
//! This module provides comprehensive cache statistics tracking
//! and analysis capabilities across multiple tiers.

pub mod atomic_stats;
pub mod error_stats;
pub mod multi_tier;
pub mod tier_stats;

pub use error_stats::ErrorStatistics;

// REMOVED: Backwards compatibility re-exports that hide canonical API paths
// Users must now import from canonical module paths:
// - Use types::statistics::atomic_stats::AtomicTierStats
// - Use types::statistics::multi_tier::MultiTierStatistics
// - Use types::statistics::tier_stats::TierStatistics
