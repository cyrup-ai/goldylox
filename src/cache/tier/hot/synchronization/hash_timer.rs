//! Precision timing utilities for cache operations
//!
//! Provides sub-nanosecond precision timing utilities. 
//! SIMD hash functionality is available from the canonical location.

use std::time::{Duration, Instant};

// Import SIMD hash state from canonical location
pub use super::simd_hash::SimdHashState;

// PrecisionTimer moved to canonical location: crate::cache::types::performance::timer::PrecisionTimer
