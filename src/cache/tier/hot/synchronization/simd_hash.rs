//! SIMD hash state for AVX2-accelerated hashing
//!
//! This module provides parallel hashing capabilities using SIMD instructions
//! for high-performance hash computation.

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

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

impl Default for SimdHashState {
    fn default() -> Self {
        Self::new()
    }
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

    /// Update hash state with new input using SIMD acceleration
    pub fn update(&mut self, input: u64) {
        #[cfg(target_arch = "x86_64")]
        {
            unsafe {
                // Load current state, multipliers, and seeds into SIMD registers
                let state_vec = _mm256_loadu_si256(self.state.as_ptr() as *const __m256i);
                let mult_vec = _mm256_loadu_si256(self.multipliers.as_ptr() as *const __m256i);
                let seeds_vec = _mm256_loadu_si256(self.seeds.as_ptr() as *const __m256i);

                // Broadcast input to all 4 lanes
                let input_vec = _mm256_set1_epi64x(input as i64);

                // Vectorized operations: state * multipliers + input + seeds
                let mult_result = _mm256_mullo_epi64(state_vec, mult_vec);
                let add_input = _mm256_add_epi64(mult_result, input_vec);
                let final_result = _mm256_add_epi64(add_input, seeds_vec);

                // Store result back to state
                _mm256_storeu_si256(self.state.as_mut_ptr() as *mut __m256i, final_result);
            }
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            // Fallback scalar implementation for non-x86_64 architectures
            for i in 0..4 {
                self.state[i] = self.state[i]
                    .wrapping_mul(self.multipliers[i])
                    .wrapping_add(input)
                    .wrapping_add(self.seeds[i]);
            }
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
