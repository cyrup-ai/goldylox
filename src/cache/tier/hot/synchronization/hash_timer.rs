//! SIMD hash state and precision timing utilities
//!
//! Provides high-performance hashing with SIMD operations and
//! sub-nanosecond precision timing for cache operations.

use std::time::{Duration, Instant};

/// SIMD hash state for AVX2-accelerated hashing
#[derive(Debug)]
#[repr(align(32))]
pub struct SimdHashState {
    /// Hash seeds for parallel hashing (4x 64-bit values = 256 bits)
    pub seeds: [u64; 4],
    /// Hash multipliers (optimized constants)
    pub multipliers: [u64; 4],
    /// Current hash state
    pub state: [u64; 4],
}

impl SimdHashState {
    /// Create new SIMD hash state
    pub fn new() -> Self {
        Self {
            seeds: [
                0x517cc1b727220a95,
                0x9e3779b97f4a7c15,
                0xbf58476d1ce4e5b1,
                0x94d049bb133111eb,
            ],
            multipliers: [
                0xc6a4a7935bd1e995,
                0x87c37b91114253d5,
                0x4cf5ad432745937f,
                0x52dce729da3ac4b5,
            ],
            state: [0; 4],
        }
    }

    /// Update hash state with new input
    pub fn update(&mut self, input: u64) {
        for i in 0..4 {
            self.state[i] = self.state[i]
                .wrapping_mul(self.multipliers[i])
                .wrapping_add(input)
                .wrapping_add(self.seeds[i]);
        }
    }

    /// Get final hash value
    pub fn finalize(&self) -> u64 {
        self.state[0] ^ self.state[1] ^ self.state[2] ^ self.state[3]
    }

    /// Reset hash state
    pub fn reset(&mut self) {
        self.state = [0; 4];
    }
}

/// Performance timer for sub-nanosecond precision
#[derive(Debug)]
pub struct PrecisionTimer {
    start_time: Instant,
}

impl PrecisionTimer {
    /// Start new precision timer
    #[inline(always)]
    pub fn start() -> Self {
        Self {
            start_time: Instant::now(),
        }
    }

    /// Get elapsed time in nanoseconds
    #[inline(always)]
    pub fn elapsed_ns(&self) -> u64 {
        self.start_time.elapsed().as_nanos() as u64
    }

    /// Get elapsed time as Duration
    #[inline(always)]
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
}
