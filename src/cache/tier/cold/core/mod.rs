//! Core implementation modules for persistent cold tier cache
//!
//! This module provides the decomposed implementation of the PersistentColdTier
//! cache, organized into logical submodules for maintainability.

pub mod initialization;
pub mod operations;
pub mod storage;
pub mod types;
pub mod utilities;

// Re-export key types for backward compatibility
pub use types::{timestamp_nanos, ColdTierStats};
// PrecisionTimer is now available from crate::cache::types::performance::timer::PrecisionTimer
