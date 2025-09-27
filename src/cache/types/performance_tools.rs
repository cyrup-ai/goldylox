//! High-precision performance tools and SIMD utilities
//!
//! This module provides timing utilities and SIMD-optimized helpers
//! for blazing-fast cache operations.

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

// PrecisionTimer moved to canonical location: crate::cache::types::performance::timer::PrecisionTimer

// CacheLine moved to canonical location: crate::cache::types::simd::memory::CacheLine

// SimdHasher moved to canonical location: crate::cache::types::simd::hasher::SimdHasher

/// SIMD-optimized memory operations
#[allow(dead_code)] // Utility system - used in SIMD optimization and performance tools
pub struct SimdMemory;

impl SimdMemory {
    /// Copy memory using SIMD operations
    #[allow(dead_code)] // Utility system - used in SIMD optimization and performance tools
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
    #[allow(dead_code)] // Hot tier SIMD - Memory copy fallback for non-x86_64 platforms
    pub unsafe fn copy_simd(src: *const u8, dst: *mut u8, len: usize) {
        unsafe {
            std::ptr::copy_nonoverlapping(src, dst, len);
        }
    }

    /// Zero memory using SIMD operations
    #[allow(dead_code)] // Utility system - used in SIMD optimization and performance tools
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
    #[allow(dead_code)] // Hot tier SIMD - Memory zeroing fallback for non-x86_64 platforms
    pub unsafe fn zero_simd(dst: *mut u8, len: usize) {
        unsafe {
            std::ptr::write_bytes(dst, 0, len);
        }
    }
}
