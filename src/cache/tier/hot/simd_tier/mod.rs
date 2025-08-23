//! SIMD-optimized hot tier cache implementation
//!
//! This module provides the core SimdHotTier struct with SIMD-accelerated
//! cache operations, intelligent eviction, and prefetch prediction.
//!
//! The implementation is decomposed into logical modules:
//! - `core`: Struct definition and constructor
//! - `operations`: Main cache operations (get, put, remove)
//! - `simd_helpers`: SIMD-specific helper functions
//! - `management`: Statistics, maintenance, and configuration access

// Module declarations
pub mod core;
pub mod management;
pub mod operations;
pub mod simd_helpers;

// Re-export main types
pub use core::SimdHotTier;
