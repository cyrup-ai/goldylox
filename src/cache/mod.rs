//! Cache system module
//!
//! High-performance hierarchical cache system with lock-free data structures
//! and atomic metadata tracking for blazing-fast text generic operations.

pub mod analyzer;
pub mod coherence;
pub mod config;
pub mod coordinator;
pub mod eviction;
pub mod manager;
pub mod memory;
pub mod serde;
pub mod tier;
pub mod traits;
pub mod types;
pub mod worker;

pub use config::*;
// REMOVED: Global function re-exports that hid generic architecture
// These re-exports allowed broken String-dependent code to compile
// Users must now import UnifiedCacheManager directly and specify generic types:
// - Use coordinator::unified_manager::UnifiedCacheManager::<K, V>::new()
// - No global functions - use manager instance methods
pub use coordinator::unified_manager::UnifiedCacheManager;
pub use serde::*;
pub use traits::{CacheKey, CacheTier, CacheValue};
pub use types::{CacheResult, TierStatistics};
pub use worker::{CacheMaintenanceWorker, MaintenanceTask, WorkerStats};

pub use crate::cache::traits::types_and_enums::CacheOperationError;
