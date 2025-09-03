//! Core unified cache manager implementation
//!
//! This module contains the main UnifiedCacheManager struct and its core operations,
//! decomposed into logical submodules for better maintainability.

// HIGHLANDER RULES: ALL impl UnifiedCacheManager modules ELIMINATED - use canonical unified_manager instead:
// - initialization.rs ELIMINATED - use unified_manager::UnifiedCacheManager::new
// - management.rs ELIMINATED - use unified_manager methods
// - operations.rs ELIMINATED - use unified_manager methods  
// - placement.rs ELIMINATED - use unified_manager methods
// - utilities.rs ELIMINATED - use unified_manager methods
// HIGHLANDER RULES: types.rs module ELIMINATED - use canonical imports instead

// Re-export main types and structs
// HIGHLANDER RULES: management types moved to canonical locations
// HIGHLANDER RULES: All types must use canonical imports:
// - BackgroundTask, MaintenanceOperation, PromotionDecision, StatisticsOperation -> define in appropriate canonical locations
// - ValueCharacteristics -> crate::cache::coordinator::tier_operations::ValueCharacteristics  
// - UnifiedCacheManager -> crate::cache::coordinator::unified_manager::UnifiedCacheManager
// - UnifiedStats -> crate::telemetry::unified_stats::UnifiedStats
// Remove unused re-exports per HIGHLANDER RULES
// PrecisionTimer is now available from crate::cache::types::performance::timer::PrecisionTimer
