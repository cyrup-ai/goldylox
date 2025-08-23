//! Memory pool manager with lock-free allocation and dynamic sizing
//!
//! This module implements efficient memory pool management with lock-free allocation,
//! automatic pool sizing, and comprehensive pool statistics tracking.

pub mod cleanup_manager;
pub mod configuration;
pub mod individual_pool;
pub mod manager;
pub mod statistics;

// Re-export main types for convenience
pub use cleanup_manager::{CompactionDecision, PoolCleanupManager};
pub use configuration::PoolConfiguration;
pub use individual_pool::MemoryPool;
pub use manager::MemoryPoolManager;
pub use statistics::PoolStatistics;
