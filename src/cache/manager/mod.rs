//! Unified cache manager module
//!
//! This module provides a decomposed implementation of the cache manager with
//! logical separation of concerns across multiple submodules.

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
// - Use crate::cache::manager::core::types::UnifiedCacheManager
// - etc.

pub use background::{
    BackgroundWorkerState, MaintenanceConfig, MaintenanceScheduler,
    MaintenanceStats, MaintenanceTask, MaintenanceTaskType, WorkerStatus,
};
pub use crate::cache::coordinator::background_coordinator::BackgroundCoordinator;
pub use error_recovery::{
    CircuitBreaker, CircuitState, ErrorDetector, ErrorRecoverySystem, ErrorStatistics, ErrorType,
    HealthStatus, RecoveryStrategies, RecoveryStrategy,
};
pub use performance::{
    AlertSeverity, AlertSystem, AlertType, MetricsCollector, PerformanceMonitor,
    PerformanceSnapshot,
};
pub use policy::{
    AccessType, CachePolicyEngine, PatternType, PrefetchPredictor, ReplacementAlgorithm,
    ReplacementPolicies, WritePolicy, WritePolicyManager,
};
pub use statistics::types::{
    CachePerformanceMetrics, StatisticsConfig, UnifiedCacheStatistics,
};
pub use strategy::{
    CacheStrategy, CacheStrategySelector, StrategyMetrics, StrategySwitcher, StrategyThresholds,
};
// Re-export TierStatistics from canonical location
pub use crate::cache::types::statistics::tier_stats::TierStatistics;
// TierPromotionManager moved to canonical location: crate::cache::tier::manager::TierPromotionManager
// Use the sophisticated "best of best" implementation for all tier promotion needs
