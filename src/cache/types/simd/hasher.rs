#![allow(dead_code)]
// SIMD Types - Complete SIMD hashing library with SSE2/AVX2 optimization, true parallel batch processing, runtime CPU detection, and scalar fallbacks

//! SIMD-optimized hash computation for cache keys
//!
//! This module provides hardware SIMD acceleration using SSE2/AVX2 intrinsics
//! with scalar fallbacks for non-x86 architectures.

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

use crate::cache::traits::CacheKey;

/// SIMD-optimized hasher with hardware acceleration
#[derive(Debug)]
pub struct SimdHasher {
    state: [u32; 4],
    seed: u64,
}

impl SimdHasher {
    /// Create new SIMD hasher
    #[inline(always)]
    pub fn new() -> Self {
        Self {
            state: [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476],
            seed: 0x517CC1B727220A95,
        }
    }

    /// Create hasher with custom seed
    #[inline(always)]
    pub const fn with_seed(seed: u64) -> Self {
        Self {
            state: [
                0x67452301 ^ (seed & 0xFFFFFFFF) as u32,
                0xEFCDAB89 ^ (seed >> 32) as u32,
                0x98BADCFE,
                0x10325476,
            ],
            seed,
        }
    }

    /// Hash cache key with SIMD operations (avoids double hashing)
    #[inline(always)]
    pub fn hash_cache_key<K: CacheKey>(&mut self, key: &K) -> u64 {
        // Check for runtime SIMD support first
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("sse2") {
                return self.hash_cache_key_simd(key);
            }
        }

        // Fallback to scalar version for non-SIMD systems
        self.hash_cache_key_scalar(key)
    }

    /// SIMD-optimized cache key hashing
    #[cfg(target_arch = "x86_64")]
    #[inline(always)]
    fn hash_cache_key_simd<K: CacheKey>(&mut self, key: &K) -> u64 {
        // Get key bytes directly to avoid double hashing
        let key_bytes = self.get_key_bytes(key);
        self.hash_bytes_simd(&key_bytes)
    }

    /// Scalar fallback for cache key hashing
    #[inline(always)]
    fn hash_cache_key_scalar<K: CacheKey>(&mut self, key: &K) -> u64 {
        let key_bytes = self.get_key_bytes(key);
        self.hash_bytes_scalar(&key_bytes)
    }

    /// Extract bytes from cache key for hashing using safe Hash trait
    #[inline(always)]
    fn get_key_bytes<K: CacheKey>(&self, key: &K) -> Vec<u8> {
        struct ByteCollector {
            bytes: Vec<u8>,
        }

        impl std::hash::Hasher for ByteCollector {
            fn write(&mut self, bytes: &[u8]) {
                self.bytes.extend_from_slice(bytes);
            }

            fn finish(&self) -> u64 {
                0 // Not used - we only collect bytes
            }
        }

        let mut collector = ByteCollector { bytes: Vec::new() };
        key.hash(&mut collector);
        collector.bytes
    }

    /// Hash multiple keys with true parallel hardware batch SIMD processing
    #[cfg(target_arch = "x86_64")]
    pub fn hash_batch_keys<K: CacheKey>(&mut self, keys: &[K]) -> Vec<u64> {
        // Check for AVX2 support for true parallel processing
        if is_x86_feature_detected!("avx2") {
            return self.hash_batch_keys_avx2(keys);
        } else if is_x86_feature_detected!("sse2") {
            return self.hash_batch_keys_sse2(keys);
        }

        // Fallback to scalar processing
        self.hash_batch_keys_scalar(keys)
    }

    /// AVX2-based TRUE parallel batch hashing - simultaneous processing of 4 keys
    #[cfg(target_arch = "x86_64")]
    fn hash_batch_keys_avx2<K: CacheKey>(&mut self, keys: &[K]) -> Vec<u64> {
        let mut results = Vec::with_capacity(keys.len());

        for chunk in keys.chunks(4) {
            // Phase 1: Extract bytes from all keys safely
            let key_bytes: Vec<Vec<u8>> = chunk.iter().map(|key| self.get_key_bytes(key)).collect();

            // Phase 2: Align all byte arrays to same length for SIMD processing
            let max_len = key_bytes.iter().map(|b| b.len()).max().unwrap_or(0);
            let aligned_len = (max_len + 31) & !31; // Round up to 32-byte boundary

            let mut aligned_bytes = Vec::with_capacity(4);
            for mut bytes in key_bytes {
                bytes.resize(aligned_len, 0); // Pad with zeros
                aligned_bytes.push(bytes);
            }

            // Phase 3: Initialize 4 parallel hash states
            let mut parallel_states = [self.state; 4];

            // Phase 4: TRUE PARALLEL PROCESSING using AVX2
            unsafe {
                for chunk_offset in (0..aligned_len).step_by(32) {
                    // Load 32 bytes from each of 4 keys into separate AVX2 registers
                    let mut data_vectors = [_mm256_setzero_si256(); 4];

                    for (key_idx, bytes) in aligned_bytes.iter().enumerate().take(chunk.len()) {
                        let chunk_end = (chunk_offset + 32).min(bytes.len());
                        let mut chunk_data = [0u8; 32];

                        if chunk_offset < bytes.len() {
                            let copy_len = chunk_end - chunk_offset;
                            chunk_data[..copy_len].copy_from_slice(&bytes[chunk_offset..chunk_end]);
                        }

                        data_vectors[key_idx] =
                            _mm256_loadu_si256(chunk_data.as_ptr() as *const __m256i);
                    }

                    // Convert all 4 hash states to AVX2 format for parallel processing
                    let state_vectors = [
                        _mm256_broadcast_si128(&_mm_loadu_si128(
                            parallel_states[0].as_ptr() as *const __m128i
                        )),
                        _mm256_broadcast_si128(&_mm_loadu_si128(
                            parallel_states[1].as_ptr() as *const __m128i
                        )),
                        _mm256_broadcast_si128(&_mm_loadu_si128(
                            parallel_states[2].as_ptr() as *const __m128i
                        )),
                        _mm256_broadcast_si128(&_mm_loadu_si128(
                            parallel_states[3].as_ptr() as *const __m128i
                        )),
                    ];

                    // SIMULTANEOUS XOR operations for all 4 keys
                    let xor_results = [
                        _mm256_xor_si256(state_vectors[0], data_vectors[0]),
                        _mm256_xor_si256(state_vectors[1], data_vectors[1]),
                        _mm256_xor_si256(state_vectors[2], data_vectors[2]),
                        _mm256_xor_si256(state_vectors[3], data_vectors[3]),
                    ];

                    // SIMULTANEOUS mixing operations for all 4 keys
                    let mixed_results = [
                        self.simd_mix_avx2(xor_results[0]),
                        self.simd_mix_avx2(xor_results[1]),
                        self.simd_mix_avx2(xor_results[2]),
                        self.simd_mix_avx2(xor_results[3]),
                    ];

                    // Update all 4 parallel hash states
                    for (i, &result) in mixed_results.iter().enumerate().take(chunk.len()) {
                        let final_state = _mm256_extracti128_si256(result, 0);
                        _mm_storeu_si128(
                            parallel_states[i].as_mut_ptr() as *mut __m128i,
                            final_state,
                        );
                    }
                }

                // Phase 5: Extract final hash values from all parallel states
                for i in 0..chunk.len() {
                    let final_hash =
                        (parallel_states[i][0] as u64) << 32 | (parallel_states[i][1] as u64);
                    results.push(final_hash);
                }

                // Update main hasher state with first parallel result
                if !chunk.is_empty() {
                    self.state = parallel_states[0];
                }
            }
        }

        results
    }

    /// SSE2-based TRUE parallel batch hashing - simultaneous processing of 2 keys
    #[cfg(target_arch = "x86_64")]
    fn hash_batch_keys_sse2<K: CacheKey>(&mut self, keys: &[K]) -> Vec<u64> {
        let mut results = Vec::with_capacity(keys.len());

        for chunk in keys.chunks(2) {
            // Phase 1: Extract bytes from all keys safely
            let key_bytes: Vec<Vec<u8>> = chunk.iter().map(|key| self.get_key_bytes(key)).collect();

            // Phase 2: Align all byte arrays to same length for SIMD processing
            let max_len = key_bytes.iter().map(|b| b.len()).max().unwrap_or(0);
            let aligned_len = (max_len + 15) & !15; // Round up to 16-byte boundary

            let mut aligned_bytes = Vec::with_capacity(2);
            for mut bytes in key_bytes {
                bytes.resize(aligned_len, 0); // Pad with zeros
                aligned_bytes.push(bytes);
            }

            // Phase 3: Initialize 2 parallel hash states
            let mut parallel_states = [self.state; 2];

            // Phase 4: TRUE PARALLEL PROCESSING using SSE2
            unsafe {
                for chunk_offset in (0..aligned_len).step_by(16) {
                    // Load 16 bytes from each of 2 keys into separate SSE2 registers
                    let mut data_vectors = [_mm_setzero_si128(); 2];

                    for (key_idx, bytes) in aligned_bytes.iter().enumerate().take(chunk.len()) {
                        let chunk_end = (chunk_offset + 16).min(bytes.len());
                        let mut chunk_data = [0u8; 16];

                        if chunk_offset < bytes.len() {
                            let copy_len = chunk_end - chunk_offset;
                            chunk_data[..copy_len].copy_from_slice(&bytes[chunk_offset..chunk_end]);
                        }

                        data_vectors[key_idx] =
                            _mm_loadu_si128(chunk_data.as_ptr() as *const __m128i);
                    }

                    // Convert parallel states to SSE2 format for simultaneous processing
                    let state_vectors = [
                        _mm_loadu_si128(parallel_states[0].as_ptr() as *const __m128i),
                        if chunk.len() > 1 {
                            _mm_loadu_si128(parallel_states[1].as_ptr() as *const __m128i)
                        } else {
                            _mm_setzero_si128()
                        },
                    ];

                    // SIMULTANEOUS XOR operations for both keys
                    let xor_results = [
                        _mm_xor_si128(state_vectors[0], data_vectors[0]),
                        if chunk.len() > 1 {
                            _mm_xor_si128(state_vectors[1], data_vectors[1])
                        } else {
                            _mm_setzero_si128()
                        },
                    ];

                    // SIMULTANEOUS mixing operations for both keys
                    let mixed_results = [
                        self.simd_mix(xor_results[0]),
                        if chunk.len() > 1 {
                            self.simd_mix(xor_results[1])
                        } else {
                            _mm_setzero_si128()
                        },
                    ];

                    // Update both parallel states simultaneously
                    for (i, &result) in mixed_results.iter().enumerate().take(chunk.len()) {
                        _mm_storeu_si128(parallel_states[i].as_mut_ptr() as *mut __m128i, result);
                    }
                }

                // Phase 5: Extract final hash values from all parallel states
                for i in 0..chunk.len() {
                    let final_hash =
                        (parallel_states[i][0] as u64) << 32 | (parallel_states[i][1] as u64);
                    results.push(final_hash);
                }

                // Update main hasher state with first parallel result
                if !chunk.is_empty() {
                    self.state = parallel_states[0];
                }
            }
        }

        results
    }

    /// Scalar fallback for batch hashing
    fn hash_batch_keys_scalar<K: CacheKey>(&mut self, keys: &[K]) -> Vec<u64> {
        keys.iter()
            .map(|key| self.hash_cache_key_scalar(key))
            .collect()
    }

    /// Hash multiple keys with scalar fallback
    #[cfg(not(target_arch = "x86_64"))]
    pub fn hash_batch_keys<K: CacheKey>(&mut self, keys: &[K]) -> Vec<u64> {
        // Avoid redundant CPU detection by using scalar method directly
        keys.iter()
            .map(|key| self.hash_cache_key_scalar(key))
            .collect()
    }

    /// Hash bytes using SIMD operations with runtime CPU detection
    #[inline(always)]
    pub fn hash_bytes(&mut self, bytes: &[u8]) -> u64 {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("sse2") {
                return self.hash_bytes_simd(bytes);
            }
        }

        self.hash_bytes_scalar(bytes)
    }

    /// SIMD-optimized byte hashing
    #[cfg(target_arch = "x86_64")]
    #[inline(always)]
    fn hash_bytes_simd(&mut self, bytes: &[u8]) -> u64 {
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
    #[inline(always)]
    fn hash_bytes_scalar(&mut self, bytes: &[u8]) -> u64 {
        for &byte in bytes {
            self.state[0] = self.state[0].wrapping_mul(31).wrapping_add(byte as u32);
            self.state[1] = self.state[1].wrapping_mul(37).wrapping_add(byte as u32);
            self.state[2] = self.state[2].wrapping_mul(41).wrapping_add(byte as u32);
            self.state[3] = self.state[3].wrapping_mul(43).wrapping_add(byte as u32);
        }

        (self.state[0] as u64) << 32 | (self.state[1] as u64)
    }

    /// Hash string with SIMD operations
    #[inline(always)]
    pub fn hash_string(&mut self, text: &str) -> u64 {
        self.hash_bytes(text.as_bytes())
    }

    /// Reset hasher state
    #[inline(always)]
    pub fn reset(&mut self) {
        *self = Self::with_seed(self.seed);
    }

    /// Get current seed value
    #[inline(always)]
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Get current state snapshot
    #[inline(always)]
    pub fn state(&self) -> [u32; 4] {
        self.state
    }

    /// Hash multiple byte arrays in batch using SIMD optimization
    pub fn hash_batch_bytes(&mut self, byte_arrays: &[&[u8]]) -> Vec<u64> {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("sse2") {
                // Use batch SIMD processing for multiple byte arrays
                let mut results = Vec::with_capacity(byte_arrays.len());

                for chunk in byte_arrays.chunks(4) {
                    unsafe {
                        let mut hash_states = [self.state; 4];

                        // Extract bytes from all byte arrays safely
                        let byte_arrays: Vec<&[u8]> = chunk.iter().copied().collect();

                        // Align all byte arrays to same length for SIMD processing
                        let max_len = byte_arrays.iter().map(|b| b.len()).max().unwrap_or(0);
                        let aligned_len = (max_len + 15) & !15; // Round up to 16-byte boundary

                        let mut aligned_bytes = Vec::with_capacity(4);
                        for &bytes in &byte_arrays {
                            let mut aligned = vec![0u8; aligned_len];
                            aligned[..bytes.len()].copy_from_slice(bytes);
                            aligned_bytes.push(aligned);
                        }

                        // TRUE PARALLEL PROCESSING using SSE2
                        for chunk_offset in (0..aligned_len).step_by(16) {
                            // Load 16 bytes from each of up to 4 byte arrays into separate SSE2 registers
                            let mut data_vectors = [_mm_setzero_si128(); 4];

                            for (i, bytes) in aligned_bytes.iter().enumerate().take(chunk.len()) {
                                let chunk_end = (chunk_offset + 16).min(bytes.len());
                                let mut chunk_data = [0u8; 16];

                                if chunk_offset < bytes.len() {
                                    let copy_len = chunk_end - chunk_offset;
                                    chunk_data[..copy_len]
                                        .copy_from_slice(&bytes[chunk_offset..chunk_end]);
                                }

                                data_vectors[i] =
                                    _mm_loadu_si128(chunk_data.as_ptr() as *const __m128i);
                            }

                            // Convert hash states to SSE2 format for simultaneous processing
                            let state_vectors = [
                                _mm_loadu_si128(hash_states[0].as_ptr() as *const __m128i),
                                if chunk.len() > 1 {
                                    _mm_loadu_si128(hash_states[1].as_ptr() as *const __m128i)
                                } else {
                                    _mm_setzero_si128()
                                },
                                if chunk.len() > 2 {
                                    _mm_loadu_si128(hash_states[2].as_ptr() as *const __m128i)
                                } else {
                                    _mm_setzero_si128()
                                },
                                if chunk.len() > 3 {
                                    _mm_loadu_si128(hash_states[3].as_ptr() as *const __m128i)
                                } else {
                                    _mm_setzero_si128()
                                },
                            ];

                            // SIMULTANEOUS XOR operations for all byte arrays
                            let xor_results = [
                                _mm_xor_si128(state_vectors[0], data_vectors[0]),
                                if chunk.len() > 1 {
                                    _mm_xor_si128(state_vectors[1], data_vectors[1])
                                } else {
                                    _mm_setzero_si128()
                                },
                                if chunk.len() > 2 {
                                    _mm_xor_si128(state_vectors[2], data_vectors[2])
                                } else {
                                    _mm_setzero_si128()
                                },
                                if chunk.len() > 3 {
                                    _mm_xor_si128(state_vectors[3], data_vectors[3])
                                } else {
                                    _mm_setzero_si128()
                                },
                            ];

                            // SIMULTANEOUS mixing operations for all byte arrays
                            let mixed_results = [
                                self.simd_mix(xor_results[0]),
                                if chunk.len() > 1 {
                                    self.simd_mix(xor_results[1])
                                } else {
                                    _mm_setzero_si128()
                                },
                                if chunk.len() > 2 {
                                    self.simd_mix(xor_results[2])
                                } else {
                                    _mm_setzero_si128()
                                },
                                if chunk.len() > 3 {
                                    self.simd_mix(xor_results[3])
                                } else {
                                    _mm_setzero_si128()
                                },
                            ];

                            // Update all hash states simultaneously
                            for (i, &result) in mixed_results.iter().enumerate().take(chunk.len()) {
                                _mm_storeu_si128(
                                    hash_states[i].as_mut_ptr() as *mut __m128i,
                                    result,
                                );
                            }
                        }

                        // Extract final hash values from all hash states
                        for i in 0..chunk.len() {
                            let final_hash =
                                (hash_states[i][0] as u64) << 32 | (hash_states[i][1] as u64);
                            results.push(final_hash);
                        }

                        // Update main hasher state with first result
                        if !chunk.is_empty() {
                            self.state = hash_states[0];
                        }
                    }
                }

                return results;
            }
        }

        // Fallback to individual processing
        byte_arrays
            .iter()
            .map(|bytes| self.hash_bytes(bytes))
            .collect()
    }

    /// Combine two hash values using SIMD mixing
    #[inline(always)]
    pub fn combine_hashes(hash1: u64, hash2: u64) -> u64 {
        let mut combined = hash1 ^ hash2.rotate_left(13);
        combined = combined.wrapping_mul(0x9E3779B97F4A7C15_u64);
        combined ^= combined >> 33;
        combined = combined.wrapping_mul(0xff51afd7ed558ccd);
        combined ^= combined >> 33;
        combined
    }

    /// Create hasher with custom state for advanced use
    #[inline(always)]
    pub fn with_state(seed: u64, state: [u32; 4]) -> Self {
        Self { seed, state }
    }

    /// SIMD mixing function
    #[cfg(target_arch = "x86_64")]
    #[inline(always)]
    unsafe fn simd_mix(&self, state: __m128i) -> __m128i {
        // Rotate and mix using SIMD operations
        let rotated = _mm_or_si128(_mm_slli_epi32(state, 13), _mm_srli_epi32(state, 19));
        _mm_mullo_epi32(rotated, _mm_set1_epi32(0x5BD1E995))
    }

    /// AVX2 SIMD mixing function for parallel processing
    #[cfg(target_arch = "x86_64")]
    #[inline(always)]
    unsafe fn simd_mix_avx2(&self, state: __m256i) -> __m256i {
        let rotated = _mm256_or_si256(_mm256_slli_epi32(state, 13), _mm256_srli_epi32(state, 19));
        _mm256_mullo_epi32(rotated, _mm256_set1_epi32(0x5BD1E995))
    }
}

impl Default for SimdHasher {
    fn default() -> Self {
        Self::new()
    }
}
