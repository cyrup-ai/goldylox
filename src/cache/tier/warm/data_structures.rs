//! Supporting data structures and utilities for warm tier cache
//!
//! This module provides a unified interface to all warm tier data structures
//! including configuration, metrics, coordination primitives, maintenance tasks,
//! and timing utilities. The implementation is decomposed into focused submodules.

// Re-export all types from other modules in the warm_tier directory
pub use super::config::*;
// Re-export atomic primitives
pub use super::coordination::atomic_primitives;
pub use super::coordination::*;
// Re-export the canonical LockFreeWarmTier from core.rs to avoid duplication
pub use super::core::LockFreeWarmTier;
pub use super::maintenance::*;
pub use super::metrics::*;
pub use super::timing::*;

// All LockFreeWarmTier implementations are now in core.rs
// This file only provides re-exports and supporting types
