//! High-precision timing utilities for performance measurement
//!
//! This module provides high-precision timing capabilities including RDTSC
//! for x86_64 platforms and fallback mechanisms for other architectures.

use std::time::Instant;

/// High-precision timer using RDTSC when available
pub struct PrecisionTimer {
    start_time: Instant,
    #[cfg(target_arch = "x86_64")]
    start_cycles: u64,
}

impl PrecisionTimer {
    /// Create a new precision timer starting now
    #[inline]
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            #[cfg(target_arch = "x86_64")]
            start_cycles: Self::read_tsc(),
        }
    }

    /// Get elapsed time in nanoseconds
    #[inline]
    pub fn elapsed_nanos(&self) -> u64 {
        self.start_time.elapsed().as_nanos() as u64
    }

    /// Get elapsed CPU cycles (x86_64 only, fallback to nanoseconds on other platforms)
    #[cfg(target_arch = "x86_64")]
    #[inline]
    pub fn elapsed_cycles(&self) -> u64 {
        Self::read_tsc() - self.start_cycles
    }

    /// Get elapsed CPU cycles (fallback for non-x86_64 platforms)
    #[cfg(not(target_arch = "x86_64"))]
    #[inline]
    pub fn elapsed_cycles(&self) -> u64 {
        // Fallback to nanoseconds for non-x86_64 platforms
        self.elapsed_nanos()
    }

    /// Read Time Stamp Counter (x86_64 only)
    #[cfg(target_arch = "x86_64")]
    #[inline]
    fn read_tsc() -> u64 {
        unsafe { core::arch::x86_64::_rdtsc() }
    }
}

impl Default for PrecisionTimer {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
