//! Supporting data structures and utilities for warm tier cache
//!
//! This module provides a unified interface to all warm tier data structures
//! including configuration, metrics, coordination primitives, maintenance tasks,
//! and timing utilities. The implementation is decomposed into focused submodules.

// Removed problematic re-export to eliminate type identity conflicts
// Use direct import: crate::cache::tier::warm::config::{WarmTierConfig, EvictionConfig, ...}
// Re-export atomic primitives
// Re-export the canonical LockFreeWarmTier from core.rs to avoid duplication
pub use super::maintenance::*;

// All LockFreeWarmTier implementations are now in core.rs
// This file only provides re-exports and supporting types
