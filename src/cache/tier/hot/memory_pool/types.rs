//! Core data structures for SIMD-aligned memory pool
//!
//! This module defines the fundamental types used in the hot tier memory pool,
//! including cache slots and metadata structures optimized for SIMD operations.

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

use crate::cache::traits::{CacheKey, CacheValue};

/// Cache slot with SIMD-friendly layout (32 bytes, fits 2 per cache line)
#[derive(Debug)]
#[repr(align(32))]
pub struct CacheSlot<K: CacheKey, V: CacheValue> {
    /// Key hash (primary identifier for SIMD comparison)
    pub key_hash: u64,
    /// Full key (for collision resolution)
    pub key: K,
    /// Cached value
    pub value: Option<V>,
    /// Access timestamp (for LRU)
    pub last_access_ns: u64,
}

impl<K: CacheKey, V: CacheValue> CacheSlot<K, V> {
    /// Create empty cache slot with provided key
    pub fn empty_with_key(key: K) -> Self {
        Self {
            key_hash: 0,
            key,
            value: None,
            last_access_ns: 0,
        }
    }

    /// Create empty cache slot with default key
    pub fn empty() -> Self
    where
        K: Default,
    {
        Self {
            key_hash: 0,
            key: K::default(),
            value: None,
            last_access_ns: 0,
        }
    }
}

impl<K: CacheKey + Default, V: CacheValue> Default for CacheSlot<K, V> {
    fn default() -> Self {
        Self::empty()
    }
}

impl<K: CacheKey, V: CacheValue> CacheSlot<K, V> {
    /// Check if slot is occupied
    #[inline(always)]
    pub fn is_occupied(&self) -> bool {
        self.value.is_some()
    }

    /// Check if slot is empty
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.value.is_none()
    }

    /// Get estimated memory usage of stored value
    pub fn memory_usage(&self) -> usize {
        if let Some(ref value) = self.value {
            value.estimated_size()
        } else {
            0
        }
    }

    /// Clear the slot contents
    pub fn clear(&mut self)
    where
        K: Default,
    {
        self.key_hash = 0;
        self.key = K::default();
        self.value = None;
        self.last_access_ns = 0;
    }
}

/// Slot metadata for SIMD parallel operations (16 bytes)
#[derive(Debug, Clone, Copy)]
#[repr(align(16))]
pub struct SlotMetadata {
    /// Slot occupancy (0=empty, 1=occupied, 2=tombstone)
    pub state: u8,
    /// Access count (overflow wraps)
    pub access_count: u8,
    /// Priority level (for eviction decisions)
    pub priority: u8,
    /// Reserved for alignment
    pub _reserved: [u8; 1],
    /// Generation counter (ABA prevention)
    pub generation: u32,
    /// Entry size in bytes
    pub size_bytes: u32,
    /// Reserved padding
    pub _padding: [u8; 4],
}

impl SlotMetadata {
    /// Create empty metadata
    pub const fn empty() -> Self {
        Self {
            state: 0,
            access_count: 0,
            priority: 0,
            _reserved: [0; 1],
            generation: 0,
            size_bytes: 0,
            _padding: [0; 4],
        }
    }

    /// Check if slot is occupied
    #[inline(always)]
    pub fn is_occupied(&self) -> bool {
        self.state == 1
    }

    /// Check if slot is empty
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.state == 0
    }

    /// Check if slot is tombstone
    #[inline(always)]
    pub fn is_tombstone(&self) -> bool {
        self.state == 2
    }

    /// Mark slot as occupied
    pub fn mark_occupied(&mut self, size: u32) {
        self.state = 1;
        self.size_bytes = size;
        self.generation = self.generation.wrapping_add(1);
    }

    /// Mark slot as empty
    pub fn mark_empty(&mut self) {
        self.state = 0;
        self.access_count = 0;
        self.priority = 0;
        self.size_bytes = 0;
    }

    /// Mark slot as tombstone (for deletion)
    pub fn mark_tombstone(&mut self) {
        self.state = 2;
    }

    /// Record access to this slot
    pub fn record_access(&mut self) {
        self.access_count = self.access_count.wrapping_add(1);
    }
}
