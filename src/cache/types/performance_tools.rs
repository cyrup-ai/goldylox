//! High-precision performance tools and SIMD utilities
//!
//! This module provides timing utilities and SIMD-optimized helpers
//! for blazing-fast cache operations.

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

use super::core_types::timestamp_nanos;

// PrecisionTimer moved to canonical location: crate::cache::types::performance::timer::PrecisionTimer

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

// SimdHasher moved to canonical location: crate::cache::types::simd::hasher::SimdHasher

/// SIMD-optimized memory operations
pub struct SimdMemory;

impl SimdMemory {
    /// Copy memory using SIMD operations
    #[cfg(target_arch = "x86_64")]
    #[inline(always)]
    pub unsafe fn copy_simd(src: *const u8, dst: *mut u8, len: usize) {
        // Runtime CPU feature detection
        if !is_x86_feature_detected!("avx2") {
            std::ptr::copy_nonoverlapping(src, dst, len);
            return;
        }

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
        unsafe {
            std::ptr::copy_nonoverlapping(src, dst, len);
        }
    }

    /// Zero memory using SIMD operations
    #[cfg(target_arch = "x86_64")]
    #[inline(always)]
    pub unsafe fn zero_simd(dst: *mut u8, len: usize) {
        // Runtime CPU feature detection
        if !is_x86_feature_detected!("avx2") {
            std::ptr::write_bytes(dst, 0, len);
            return;
        }

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
        unsafe {
            std::ptr::write_bytes(dst, 0, len);
        }
    }
}
