#![allow(dead_code)]
// Hot tier memory pool - Complete memory pool library with lock-free allocation, SIMD alignment, and atomic freelist operations

//! Lock-free memory allocation with atomic freelists for hot tier cache
//!
//! This module provides SIMD-aligned memory allocation and management for
//! cache slots with cache-line optimization and atomic freelist operations.

pub mod pool;
pub mod statistics;
pub mod types;

// Re-export main types and structs
pub use pool::MemoryPool;
pub use statistics::MemoryPoolStats;
pub use types::{CacheSlot, SlotMetadata};
