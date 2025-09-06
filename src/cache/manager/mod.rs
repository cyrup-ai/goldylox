//! Unified cache manager module
//!
//! This module provides a decomposed implementation of the cache manager with
//! logical separation of concerns across multiple submodules.

 // Internal manager architecture - components may not be used in minimal API

pub mod background;
pub mod core;
pub mod error_recovery;
pub mod performance;
pub mod policy;
pub mod statistics;
pub mod strategy;

// Removed re-exports to eliminate type identity conflicts - use canonical imports instead
// Import directly from specific modules:
// - Use crate::cache::types::AccessPath
// - Use crate::cache::types::PlacementDecision  
// - Use crate::cache::coordinator::unified_manager::UnifiedCacheManager::<K, V>::new()
// - etc.

// NO RE-EXPORTS - Use canonical imports only:
// - Use crate::cache::manager::background::types::MaintenanceScheduler
// - Use crate::cache::manager::error_recovery::ErrorRecoverySystem
// Remove unused performance exports
// Remove unused policy exports
// REMOVED: Redirecting to canonical location
// Import UnifiedCacheStatistics from canonical location:
// - Use crate::telemetry::unified_stats::{UnifiedCacheStatistics, CachePerformanceMetrics, StatisticsConfig};
// Remove unused strategy exports
// Re-export TierStatistics from canonical location
// TierPromotionManager moved to canonical location: crate::cache::tier::manager::TierPromotionManager
// Use the sophisticated "best of best" implementation for all tier promotion needs
