//! Cache coordination system for unified multi-tier cache management
//!
//! This module implements the core cache coordination logic that orchestrates
//! all cache tiers (Hot, Warm, Cold) with atomic state management and SIMD optimization.
//!
//! ## Architecture
//!
//! The cache coordinator is decomposed into focused modules:
//! - `unified_manager`: Core UnifiedCacheManager struct and main operations
//! - `strategy_selector`: Cache strategy selection and adaptive optimization
//! - `tier_operations`: Low-level tier access and placement analysis
//! - (deprecated: global_api removed - use explicit UnifiedCacheManager instances)
//!
//! ## Key Features
//!
//! - **Multi-tier coordination**: Intelligent placement across hot, warm, and cold tiers
//! - **SIMD optimization**: Hardware-accelerated operations for performance
//! - **Atomic state management**: Lock-free coordination using atomic operations
//! - **Coherence protocol**: MESI-like cache coherence for consistency
//! - **Adaptive strategies**: Machine learning-based cache strategy optimization
//! - **Background processing**: Work-stealing task queue for maintenance operations

// Module declarations
pub mod strategy_selector;
pub mod tier_operations;
pub mod unified_manager;

// REMOVED: Compatibility re-exports that hide canonical API paths
// These re-exports allowed broken code to import via coordinator:: instead of specific modules
// Users must now import from canonical module paths:
// - Deprecated: coordinator::global_api removed (use UnifiedCacheManager::<K,V>::new directly)
// - Use coordinator::strategy_selector::CacheStrategySelector
// - Use coordinator::tier_operations::TierOperations
// - Use coordinator::unified_manager::UnifiedCacheManager
