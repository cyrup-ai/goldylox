//! Memory management module with decomposed submodules
//!
//! This module coordinates memory allocation, pool management, garbage collection,
//! pressure monitoring, and efficiency analysis through specialized submodules.

// Internal memory architecture - components may not be used in minimal API

// Submodule declarations
pub mod allocation_manager;
pub mod allocation_stats;
pub mod efficiency_analyzer;
pub mod gc_coordinator;
pub mod pool_manager;
pub mod pressure_monitor;
pub mod types;

// Re-export key types and functions (most unused by simple API)
// Other re-exports removed as unused by simple public API
