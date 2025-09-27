#![allow(dead_code)]
// Cold tier metadata index - Complete metadata indexing library with Bloom filters, fast existence checks, and cold storage management

//! Metadata index and bloom filter for fast key lookups
//!
//! This module provides fast key lookup capabilities using in-memory indexes
//! and bloom filters for negative lookup optimization in the cold tier cache.

use std::collections::HashMap;
use std::hash::Hasher;
use std::io;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use ahash::AHasher;

use super::data_structures::{BloomFilter, ColdCacheKey, IndexEntry, MetadataIndex};
use crate::cache::traits::CacheKey;

impl<K: CacheKey> ColdCacheKey<K> {
    /// Create cold cache key from original cache key (with hash-based serialization for non-bincode keys)
    pub fn from_cache_key(key: &K) -> Self {
        let mut hasher = AHasher::default();
        key.hash(&mut hasher);
        let key_hash = hasher.finish();

        // Use simplified cache_hash for consistent hashing
        let cache_hash = key.cache_hash();

        // Create a simple serialized representation using the cache hash
        let serialized_key = cache_hash.to_le_bytes().to_vec();

        Self {
            key_hash,
            serialized_key,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<K: CacheKey + bincode::Encode + bincode::Decode<()>> ColdCacheKey<K> {
    /// Create cold cache key from original cache key (with full serialization for bincode keys)
    pub fn from_cache_key_serialized(key: &K) -> Result<Self, String> {
        let mut hasher = AHasher::default();
        key.hash(&mut hasher);
        let key_hash = hasher.finish();

        // Serialize the actual key using bincode for full round-trip capability
        let serialized_key = bincode::encode_to_vec(key, bincode::config::standard())
            .map_err(|e| format!("Bincode serialization failed: {}", e))?;

        Ok(Self {
            key_hash,
            serialized_key,
            _phantom: std::marker::PhantomData,
        })
    }

    /// Convert cold cache key back to original cache key (requires bincode serialization)
    pub fn to_cache_key(&self) -> Result<K, String> {
        let (key, _len): (K, usize) =
            bincode::decode_from_slice(&self.serialized_key, bincode::config::standard())
                .map_err(|e| format!("Bincode deserialization failed: {}", e))?;
        Ok(key)
    }
}

impl<K: CacheKey> ColdCacheKey<K> {
    /// Get the hash value for fast comparison
    pub fn hash_value(&self) -> u64 {
        self.key_hash
    }

    /// Get serialized key data
    pub fn serialized_data(&self) -> &[u8] {
        &self.serialized_key
    }
}

impl<K: CacheKey> MetadataIndex<K> {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            key_index: HashMap::new(),
            bloom_filter: BloomFilter::new(10000, 3), // 10k capacity, 3 hash functions
            dirty_entries: AtomicU32::new(0),
            last_sync_ns: AtomicU64::new(0),
        })
    }

    /// Insert new index entry
    pub fn insert_entry(&mut self, key: ColdCacheKey<K>, entry: IndexEntry) {
        self.key_index.insert(key.clone(), entry);
        self.bloom_filter.insert(&key);
        self.dirty_entries.fetch_add(1, Ordering::Relaxed);
    }

    /// Get index entry for key
    pub fn get_entry(&self, key: &ColdCacheKey<K>) -> Option<IndexEntry> {
        self.key_index.get(key).cloned()
    }

    /// Remove index entry
    pub fn remove_entry(&mut self, key: &ColdCacheKey<K>) -> Option<IndexEntry> {
        let removed = self.key_index.remove(key);
        if removed.is_some() {
            self.dirty_entries.fetch_add(1, Ordering::Relaxed);
        }
        removed
    }

    /// Check if index contains key
    pub fn contains_key(&self, key: &ColdCacheKey<K>) -> bool {
        self.key_index.contains_key(key)
    }

    /// Get number of entries
    pub fn entry_count(&self) -> usize {
        self.key_index.len()
    }

    /// Get dirty entry count
    pub fn dirty_count(&self) -> u32 {
        self.dirty_entries.load(Ordering::Relaxed)
    }

    /// Mark index as synced
    pub fn mark_synced(&self) {
        self.dirty_entries.store(0, Ordering::Relaxed);
        let now_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        self.last_sync_ns.store(now_ns, Ordering::Relaxed);
    }

    /// Check if index needs sync
    pub fn needs_sync(&self) -> bool {
        self.dirty_entries.load(Ordering::Relaxed) > 100 // Sync after 100 changes
    }

    /// Get all keys (for iteration)
    pub fn keys(&self) -> Vec<ColdCacheKey<K>> {
        self.key_index.keys().cloned().collect()
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.key_index.clear();
        self.bloom_filter.clear();
        self.dirty_entries.store(0, Ordering::Relaxed);
    }
}

impl<K: CacheKey> BloomFilter<K> {
    pub fn new(capacity: u32, hash_count: u32) -> Self {
        let bit_count = (capacity * 8) / 64; // 8 bits per item, packed in u64s
        let bits = (0..bit_count).map(|_| AtomicU64::new(0)).collect();

        Self {
            bits,
            hash_count,
            capacity,
            item_count: AtomicU32::new(0),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Insert key into bloom filter
    pub fn insert(&self, key: &ColdCacheKey<K>) {
        let hash = key.hash_value();
        let bit_count = self.bits.len() * 64;

        for i in 0..self.hash_count {
            let bit_index = self.hash_function(hash, i) % (bit_count as u64);
            let word_index = (bit_index / 64) as usize;
            let bit_offset = bit_index % 64;

            if word_index < self.bits.len() {
                self.bits[word_index].fetch_or(1u64 << bit_offset, Ordering::Relaxed);
            }
        }

        self.item_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Check if key might be in the filter
    pub fn might_contain(&self, key: &ColdCacheKey<K>) -> bool {
        let hash = key.hash_value();
        let bit_count = self.bits.len() * 64;

        for i in 0..self.hash_count {
            let bit_index = self.hash_function(hash, i) % (bit_count as u64);
            let word_index = (bit_index / 64) as usize;
            let bit_offset = bit_index % 64;

            if word_index >= self.bits.len() {
                return false;
            }

            let word = self.bits[word_index].load(Ordering::Relaxed);
            if (word & (1u64 << bit_offset)) == 0 {
                return false;
            }
        }

        true
    }

    /// Clear the bloom filter
    pub fn clear(&self) {
        for bit_word in &self.bits {
            bit_word.store(0, Ordering::Relaxed);
        }
        self.item_count.store(0, Ordering::Relaxed);
    }

    /// Get current item count
    pub fn item_count(&self) -> u32 {
        self.item_count.load(Ordering::Relaxed)
    }

    /// Get filter capacity
    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    /// Calculate false positive probability
    pub fn false_positive_probability(&self) -> f64 {
        let items = self.item_count.load(Ordering::Relaxed) as f64;
        let bits = (self.bits.len() * 64) as f64;
        let hashes = self.hash_count as f64;

        if items == 0.0 {
            return 0.0;
        }

        // Standard bloom filter false positive formula
        let exponent = -hashes * items / bits;
        (1.0 - exponent.exp()).powf(hashes)
    }

    /// Hash function for bloom filter
    fn hash_function(&self, hash: u64, i: u32) -> u64 {
        // Simple hash function combining the base hash with the iteration
        hash.wrapping_mul(0x9e3779b97f4a7c15u64)
            .wrapping_add(i as u64)
    }
}

/// Index statistics
#[derive(Debug, Clone)]
pub struct IndexStats {
    pub entry_count: usize,
    pub dirty_count: u32,
    pub bloom_item_count: u32,
    pub bloom_false_positive_rate: f64,
    pub last_sync_ns: u64,
}
