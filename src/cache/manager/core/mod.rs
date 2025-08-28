//! Core unified cache manager implementation
//!
//! This module contains the main UnifiedCacheManager struct and its core operations,
//! decomposed into logical submodules for better maintainability.

pub mod initialization;
pub mod management;
pub mod operations;
pub mod placement;
pub mod types;
pub mod utilities;

// Re-export main types and structs
// Re-export management types
pub use management::{CacheHealthStatus, HealthLevel};
pub use types::{
    BackgroundTask, MaintenanceOperation, PromotionDecision,
    StatisticsOperation, UnifiedCacheManager, UnifiedStats, ValueCharacteristics,
};
// Re-export from correct locations
pub use crate::cache::types::{AccessPath, PlacementDecision};
// PrecisionTimer is now available from crate::cache::types::performance::timer::PrecisionTimer
