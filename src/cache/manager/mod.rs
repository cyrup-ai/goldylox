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
pub mod tier_manager;

// Re-export main types for convenience
pub use core::{
    AccessPath, BackgroundTask, MaintenanceOperation, PlacementDecision, PromotionDecision,
    StatisticsOperation, UnifiedCacheManager, UnifiedStats, ValueCharacteristics,
};

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
    CachePerformanceMetrics, StatisticsConfig, TierStatistics, UnifiedCacheStatistics,
};
pub use strategy::{
    CacheStrategy, CacheStrategySelector, StrategyMetrics, StrategySwitcher, StrategyThresholds,
};
pub use tier_manager::{
    DemotionCriteria, PromotionCriteria, PromotionPriority, PromotionQueue, PromotionStatistics,
    PromotionTask, PromotionTaskType, TierLocation, TierPromotionManager,
};
