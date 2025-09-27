#![allow(dead_code)]
// Memory pool management - Complete memory pool library with lock-free allocation, dynamic sizing, and comprehensive statistics

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
pub use manager::MemoryPoolManager;
