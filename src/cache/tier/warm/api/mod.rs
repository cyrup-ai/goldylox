//! Warm tier cache API modules
//!
//! This module provides the decomposed API for warm tier cache operations,
//! including global functions, tier operations, and maintenance tasks.

// Declare submodules
pub mod global_functions;
pub mod maintenance;
pub mod tier_operations;

// Re-export key types and functions for public API
pub use global_functions::{
    check_warm_tier_alerts, cleanup_expired_entries, force_eviction, get_cache_size,
    get_frequently_accessed_keys, get_idle_keys, get_memory_pressure, get_memory_usage,
    get_warm_tier_keys, init_warm_tier, insert_demoted,
    insert_promoted, is_initialized, process_background_maintenance,
    shutdown_warm_tier, warm_get, warm_put, warm_remove,
};
pub use maintenance::MaintenanceTask;
pub use tier_operations::*;