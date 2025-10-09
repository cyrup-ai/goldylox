//! Cache system module
//!
//! High-performance hierarchical cache system with lock-free data structures
//! and atomic metadata tracking for blazing-fast text generic operations.

pub(crate) mod analyzer;
pub(crate) mod async_channel;
pub mod coherence;
pub(crate) mod config;
pub(crate) mod coordinator;
pub(crate) mod eviction;
pub mod manager;
pub(crate) mod memory;
pub(crate) mod serde;
pub mod sync_wrapper;
pub mod tier;
pub mod traits;
pub(crate) mod types;
pub(crate) mod worker;

// Internal re-exports for crate use only
