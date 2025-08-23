//! High-precision performance tools and SIMD utilities
//!
//! This module provides timing utilities and SIMD-optimized helpers
//! for blazing-fast cache operations.

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

use super::core_types::timestamp_nanos;

/// RDTSC-based high-precision timing
#[derive(Debug)]
pub struct PrecisionTimer {
    /// Start timestamp
    start_ns: u64,
}

impl PrecisionTimer {
    /// Start precision timer
    #[inline(always)]
    pub fn start() -> Self {
        Self {
            start_ns: timestamp_nanos(),
        }
    }

    /// Get elapsed time in nanoseconds
    #[inline(always)]
    pub fn elapsed_ns(&self) -> u64 {
        timestamp_nanos().saturating_sub(self.start_ns)
    }

    /// Get elapsed time in microseconds
    #[inline(always)]
    pub fn elapsed_us(&self) -> f64 {
        self.elapsed_ns() as f64 / 1_000.0
    }

    /// Get elapsed time in milliseconds
    #[inline(always)]
    pub fn elapsed_ms(&self) -> f64 {
        self.elapsed_ns() as f64 / 1_000_000.0
    }

    /// Reset timer to current time
    #[inline(always)]
    pub fn reset(&mut self) {
        self.start_ns = timestamp_nanos();
    }
}

/// Memory-aligned cache line (64 bytes)
#[derive(Debug)]
#[repr(align(64))]
pub struct CacheLine<T> {
    data: T,
    _padding: [u8; 64],
}

impl<T> CacheLine<T> {
    /// Create new cache-aligned data
    #[inline(always)]
    pub fn new(data: T) -> Self {
        Self {
            data,
            _padding: [0; 64], // Fixed size padding, alignment handled by repr(align(64))
        }
    }

    /// Get reference to data
    #[inline(always)]
    pub fn get(&self) -> &T {
        &self.data
    }

    /// Get mutable reference to data
    #[inline(always)]
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.data
    }

    /// Convert into inner data
    #[inline(always)]
    pub fn into_inner(self) -> T {
        self.data
    }
}

/// SIMD-optimized hash computation for cache keys
#[derive(Debug)]
pub struct SimdHasher {
    state: [u32; 4],
}

impl SimdHasher {
    /// Create new SIMD hasher
    #[inline(always)]
    pub fn new() -> Self {
        Self {
            state: [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476],
        }
    }

    /// Hash bytes using SIMD operations
    #[cfg(target_arch = "x86_64")]
    #[inline(always)]
    pub fn hash_bytes(&mut self, bytes: &[u8]) -> u64 {
        unsafe {
            // Load state into SIMD register
            let mut state = _mm_loadu_si128(self.state.as_ptr() as *const __m128i);

            // Process bytes in 16-byte chunks
            let chunks = bytes.chunks_exact(16);
            let remainder = chunks.remainder();

            for chunk in chunks {
                let data = _mm_loadu_si128(chunk.as_ptr() as *const __m128i);
                state = _mm_xor_si128(state, data);
                state = self.simd_mix(state);
            }

            // Handle remainder
            if !remainder.is_empty() {
                let mut padded = [0u8; 16];
                padded[..remainder.len()].copy_from_slice(remainder);
                let data = _mm_loadu_si128(padded.as_ptr() as *const __m128i);
                state = _mm_xor_si128(state, data);
                state = self.simd_mix(state);
            }

            // Store result back to state
            _mm_storeu_si128(self.state.as_mut_ptr() as *mut __m128i, state);

            // Combine state elements into final hash
            (self.state[0] as u64) << 32 | (self.state[1] as u64)
        }
    }

    /// Hash bytes using scalar operations (fallback)
    #[cfg(not(target_arch = "x86_64"))]
    #[inline(always)]
    pub fn hash_bytes(&mut self, bytes: &[u8]) -> u64 {
        for &byte in bytes {
            self.state[0] = self.state[0].wrapping_mul(31).wrapping_add(byte as u32);
            self.state[1] = self.state[1].wrapping_mul(37).wrapping_add(byte as u32);
            self.state[2] = self.state[2].wrapping_mul(41).wrapping_add(byte as u32);
            self.state[3] = self.state[3].wrapping_mul(43).wrapping_add(byte as u32);
        }

        (self.state[0] as u64) << 32 | (self.state[1] as u64)
    }

    /// SIMD mixing function
    #[cfg(target_arch = "x86_64")]
    #[inline(always)]
    unsafe fn simd_mix(&self, state: __m128i) -> __m128i {
        // Rotate and mix using SIMD operations
        let rotated = _mm_or_si128(_mm_slli_epi32(state, 13), _mm_srli_epi32(state, 19));
        _mm_mullo_epi32(rotated, _mm_set1_epi32(0x5BD1E995))
    }

    /// Get final hash value
    #[inline(always)]
    pub fn finish(&self) -> u64 {
        (self.state[0] as u64) << 32 | (self.state[1] as u64)
    }

    /// Reset hasher state
    #[inline(always)]
    pub fn reset(&mut self) {
        self.state = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476];
    }
}

impl Default for SimdHasher {
    fn default() -> Self {
        Self::new()
    }
}

/// SIMD-optimized memory operations
pub struct SimdMemory;

impl SimdMemory {
    /// Copy memory using SIMD operations
    #[cfg(target_arch = "x86_64")]
    #[inline(always)]
    pub unsafe fn copy_simd(src: *const u8, dst: *mut u8, len: usize) {
        let chunks = len / 32;
        let remainder = len % 32;

        // Copy 32-byte chunks using AVX2
        for i in 0..chunks {
            let offset = i * 32;
            let data = _mm256_loadu_si256((src.add(offset)) as *const __m256i);
            _mm256_storeu_si256((dst.add(offset)) as *mut __m256i, data);
        }

        // Handle remainder
        if remainder > 0 {
            let src_remainder = src.add(chunks * 32);
            let dst_remainder = dst.add(chunks * 32);
            std::ptr::copy_nonoverlapping(src_remainder, dst_remainder, remainder);
        }
    }

    /// Copy memory using standard operations (fallback)
    #[cfg(not(target_arch = "x86_64"))]
    #[inline(always)]
    pub unsafe fn copy_simd(src: *const u8, dst: *mut u8, len: usize) {
        std::ptr::copy_nonoverlapping(src, dst, len);
    }

    /// Zero memory using SIMD operations
    #[cfg(target_arch = "x86_64")]
    #[inline(always)]
    pub unsafe fn zero_simd(dst: *mut u8, len: usize) {
        let chunks = len / 32;
        let remainder = len % 32;
        let zero = _mm256_setzero_si256();

        // Zero 32-byte chunks using AVX2
        for i in 0..chunks {
            let offset = i * 32;
            _mm256_storeu_si256((dst.add(offset)) as *mut __m256i, zero);
        }

        // Handle remainder
        if remainder > 0 {
            let dst_remainder = dst.add(chunks * 32);
            std::ptr::write_bytes(dst_remainder, 0, remainder);
        }
    }

    /// Zero memory using standard operations (fallback)
    #[cfg(not(target_arch = "x86_64"))]
    #[inline(always)]
    pub unsafe fn zero_simd(dst: *mut u8, len: usize) {
        std::ptr::write_bytes(dst, 0, len);
    }
}
