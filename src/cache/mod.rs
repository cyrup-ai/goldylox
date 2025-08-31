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
// HIGHLANDER RULES: No re-exports allowed! All code must use canonical paths:
// - Use crate::cache::coordinator::unified_manager::UnifiedCacheManager::<K, V>::new()
// - No global functions - use manager instance methods with explicit generics
// - String-based usage is FORBIDDEN and will not compile
pub use serde::*;
pub use traits::{CacheKey, CacheTier, CacheValue};
pub use types::{CacheResult, TierStatistics};
pub use worker::{CacheMaintenanceWorker, MaintenanceTask, WorkerStats};

pub use crate::cache::traits::types_and_enums::CacheOperationError;
