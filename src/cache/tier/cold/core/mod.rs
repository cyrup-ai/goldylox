//! Core implementation modules for persistent cold tier cache
//!
//! This module provides the decomposed implementation of the PersistentColdTier
//! cache, organized into logical submodules for maintainability.

pub mod initialization;
pub mod operations;
pub mod storage;
pub mod types;
pub mod utilities;

// Re-export key types and functions for backward compatibility
pub use types::{timestamp_nanos, ColdTierStats, PrecisionTimer};
pub use utilities::{
    cold_get, cold_put, cold_stats, get_frequently_accessed_keys, get_stats, init_cold_tier,
    insert_demoted, remove_entry,
};
