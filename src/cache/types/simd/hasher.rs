//! SIMD-optimized hash computation for cache keys
//!
//! This module provides high-performance hash functions optimized
//! for cache key hashing with SIMD-style operations.

use crate::cache::traits::CacheKey;

/// SIMD-optimized hash computation for cache keys
#[derive(Debug, Clone)]
pub struct SimdHasher {
    seed: u64,
    state: [u64; 4], // SIMD-friendly state array
}

impl SimdHasher {
    /// Create new SIMD hasher with seed
    #[inline(always)]
    pub const fn new(seed: u64) -> Self {
        Self {
            seed,
            state: [
                seed,
                seed ^ 0xAAAAAAAAAAAAAAAA,
                seed ^ 0x5555555555555555,
                seed ^ 0xCCCCCCCCCCCCCCCC,
            ],
        }
    }

    /// Create hasher with default seed
    #[inline(always)]
    pub fn default() -> Self {
        Self::new(0x517CC1B727220A95)
    }

    /// Hash cache key with SIMD operations
    #[inline(always)]
    pub fn hash_cache_key<K: CacheKey>(&mut self, key: &K) -> u64 {
        // Use generic CacheKey hash method
        key.cache_hash()
    }

    /// Hash arbitrary byte slice with SIMD operations
    #[inline(always)]
    pub fn hash_bytes(&mut self, bytes: &[u8]) -> u64 {
        // Process bytes in 8-byte chunks for SIMD efficiency
        let chunks = bytes.chunks_exact(8);
        let remainder = chunks.remainder();

        for (i, chunk) in chunks.enumerate() {
            let value = u64::from_le_bytes(chunk.try_into().unwrap_or([0; 8]));
            let state_index = i % 4;
            self.state[state_index] = self.state[state_index]
                .wrapping_mul(0x9E3779B97F4A7C15_u64)
                .wrapping_add(value);
        }

        // Handle remaining bytes
        if !remainder.is_empty() {
            let mut padded = [0u8; 8];
            padded[..remainder.len()].copy_from_slice(remainder);
            let value = u64::from_le_bytes(padded);
            self.state[0] = self.state[0]
                .wrapping_mul(0x9E3779B97F4A7C15_u64)
                .wrapping_add(value);
        }

        self.finalize_hash()
    }

    /// Finalize hash computation with avalanche mixing
    #[inline(always)]
    fn finalize_hash(&self) -> u64 {
        // SIMD-style parallel XOR reduction
        let mut result = self.state[0] ^ self.state[1] ^ self.state[2] ^ self.state[3];

        // Avalanche mixing for better distribution
        result ^= result >> 33;
        result = result.wrapping_mul(0xff51afd7ed558ccd);
        result ^= result >> 33;
        result = result.wrapping_mul(0xc4ceb9fe1a85ec53);
        result ^= result >> 33;

        result
    }

    /// Reset hasher state
    #[inline(always)]
    pub fn reset(&mut self) {
        *self = Self::new(self.seed);
    }

    /// Hash multiple keys in parallel (simulated SIMD)
    pub fn hash_batch_keys<K: CacheKey>(&mut self, keys: &[K]) -> Vec<u64> {
        keys.iter().map(|key| self.hash_cache_key(key)).collect()
    }

    /// Hash string with SIMD operations
    #[inline(always)]
    pub fn hash_string(&mut self, text: &str) -> u64 {
        self.hash_bytes(text.as_bytes())
    }

    /// Get current seed value
    #[inline(always)]
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Create hasher with custom state for advanced use
    #[inline(always)]
    pub fn with_state(seed: u64, state: [u64; 4]) -> Self {
        Self { seed, state }
    }

    /// Get current state snapshot
    #[inline(always)]
    pub fn state(&self) -> [u64; 4] {
        self.state
    }

    /// Hash multiple byte arrays in batch
    pub fn hash_batch_bytes(&mut self, byte_arrays: &[&[u8]]) -> Vec<u64> {
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
}
