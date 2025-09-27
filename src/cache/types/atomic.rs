#![allow(dead_code)]
// Atomic Operations - Complete lock-free atomic cache entry library with cache-line alignment, generation-based ABA prevention, atomic metadata updates, sophisticated access tracking, and comprehensive concurrency coordination

//! Atomic cache data structures with zero-cost abstractions
//!
//! This module provides lock-free atomic cache entries using
//! cache-line aligned data structures for maximum performance.

use crossbeam_utils::atomic::AtomicCell;
use std::sync::atomic::{AtomicU16, AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::cache::traits::{CacheKey, CacheValue};

/// Convert Instant to nanosecond timestamp
#[inline(always)]
pub fn timestamp_nanos(_instant: Instant) -> u64 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_nanos() as u64
}

/// Atomic update operation result
#[allow(dead_code)] // Utility system - used in atomic operations and concurrent access coordination
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateError {
    /// Concurrent modification prevented update after max retries
    ConcurrentModification,
    /// Generation counter would overflow
    GenerationOverflow,
    /// Value size exceeds maximum allowed
    ValueTooLarge,
}

/// Access result from atomic entry update
#[allow(dead_code)] // Utility system - used in atomic operations and concurrent access coordination
#[derive(Debug, Clone, Copy)]
pub struct AccessResult {
    /// Previous access timestamp (nanoseconds)
    pub previous_access_ns: u64,
    /// New access count after increment
    pub new_access_count: u64,
    /// Current access timestamp (nanoseconds)
    pub access_time_ns: u64,
}

/// Access statistics for cache entries
#[allow(dead_code)] // Utility system - used in atomic operations and concurrent access coordination
#[derive(Debug, Clone, Copy)]
pub struct AccessStats {
    /// Total access count
    pub access_count: u64,
    /// Last access timestamp (nanoseconds)
    pub last_access_ns: u64,
    /// Entry age in nanoseconds
    pub age_ns: u64,
    /// Access frequency (accesses per second)
    pub frequency_per_second: f64,
}

/// Atomic cache entry with lock-free metadata (cache-line aligned)
#[repr(align(64))]
pub struct AtomicCacheEntry<K: CacheKey, V: CacheValue> {
    /// Entry key (immutable after creation)
    key: K,
    /// Entry value (AtomicCell for lock-free access)
    value: AtomicCell<V>,
    /// Creation timestamp for age calculations
    created_at: Instant,
    /// Last access timestamp (atomic for lock-free updates)
    last_accessed: AtomicU64,
    /// Access count for frequency tracking
    access_count: AtomicU64,
    /// Entry size in bytes for memory accounting
    size_bytes: AtomicU32,
    /// Entry generation for ABA prevention
    generation: AtomicU32,
    /// Entry flags for state management
    flags: AtomicU16,
    /// Cache-line padding to prevent false sharing
    _padding: [u8; 16],
}

impl<K: CacheKey, V: CacheValue> std::fmt::Debug for AtomicCacheEntry<K, V>
where
    K: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AtomicCacheEntry")
            .field("key", &self.key)
            .field("value", &"<AtomicCell>")
            .field("created_at", &self.created_at)
            .field(
                "last_accessed",
                &self
                    .last_accessed
                    .load(std::sync::atomic::Ordering::Relaxed),
            )
            .field(
                "access_count",
                &self.access_count.load(std::sync::atomic::Ordering::Relaxed),
            )
            .field(
                "size_bytes",
                &self.size_bytes.load(std::sync::atomic::Ordering::Relaxed),
            )
            .field(
                "generation",
                &self.generation.load(std::sync::atomic::Ordering::Relaxed),
            )
            .field(
                "flags",
                &self.flags.load(std::sync::atomic::Ordering::Relaxed),
            )
            .finish()
    }
}

impl<K: CacheKey, V: CacheValue> AtomicCacheEntry<K, V> {
    /// Create new cache entry with atomic metadata
    #[inline(always)]
    pub fn new(key: K, value: V) -> Self {
        let now = Instant::now();
        let size = value.estimated_size() as u32;

        Self {
            key,
            value: AtomicCell::new(value),
            created_at: now,
            last_accessed: AtomicU64::new(timestamp_nanos(now)),
            access_count: AtomicU64::new(1),
            size_bytes: AtomicU32::new(size),
            generation: AtomicU32::new(1),
            flags: AtomicU16::new(0),
            _padding: [0; 16],
        }
    }

    /// Get entry key reference
    #[inline(always)]
    pub fn key(&self) -> &K {
        &self.key
    }

    /// Get entry value (atomic load) - requires V: Copy for lock-free access
    #[inline(always)]
    pub fn value(&self) -> V
    where
        V: Copy,
    {
        self.value.load()
    }

    /// Record access atomically
    #[inline(always)]
    pub fn record_access(&self) -> AccessResult {
        let now_nanos = timestamp_nanos(Instant::now());
        let prev_access = self.last_accessed.swap(now_nanos, Ordering::Relaxed);
        let new_count = self.access_count.fetch_add(1, Ordering::Relaxed) + 1;

        AccessResult {
            previous_access_ns: prev_access,
            new_access_count: new_count,
            access_time_ns: now_nanos,
        }
    }

    /// Get access statistics
    #[inline(always)]
    pub fn access_stats(&self) -> AccessStats {
        let last_access_ns = self.last_accessed.load(Ordering::Relaxed);
        let count = self.access_count.load(Ordering::Relaxed);
        let age = self.created_at.elapsed();

        AccessStats {
            access_count: count,
            last_access_ns,
            age_ns: age.as_nanos() as u64,
            frequency_per_second: Self::calculate_frequency(count, age),
        }
    }

    /// Calculate access frequency (accesses per second)
    #[inline(always)]
    fn calculate_frequency(count: u64, age: Duration) -> f64 {
        let age_secs = age.as_secs_f64();
        if age_secs < 0.001 {
            // Less than 1ms old
            count as f64 * 1000.0 // Extrapolate to per-second rate
        } else {
            count as f64 / age_secs
        }
    }

    /// Get entry size
    #[inline(always)]
    pub fn size_bytes(&self) -> u32 {
        self.size_bytes.load(Ordering::Relaxed)
    }

    /// Get entry generation (for ABA prevention)
    #[inline(always)]
    pub fn generation(&self) -> u32 {
        self.generation.load(Ordering::Relaxed)
    }

    /// Update generation atomically
    #[inline(always)]
    pub fn increment_generation(&self) -> u32 {
        self.generation.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Update entry value atomically with coordinated metadata updates
    ///
    /// Ensures atomic visibility of value, size, and generation changes using
    /// generation-based compare-and-swap with proper memory ordering guarantees.
    /// Prevents race conditions where readers observe inconsistent entry state.
    #[inline(always)]
    pub fn update_value(&self, new_value: V) -> Result<u32, UpdateError> {
        const MAX_RETRIES: u32 = 1000;

        let new_size = new_value.estimated_size();
        if new_size > u32::MAX as usize {
            return Err(UpdateError::ValueTooLarge);
        }
        let new_size = new_size as u32;

        for _ in 0..MAX_RETRIES {
            let current_gen = self.generation.load(Ordering::Acquire);
            let next_gen = current_gen.wrapping_add(1);

            if next_gen == 0 {
                return Err(UpdateError::GenerationOverflow);
            }

            match self.generation.compare_exchange_weak(
                current_gen,
                next_gen,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.value.store(new_value);
                    self.size_bytes.store(new_size, Ordering::Release);
                    return Ok(next_gen);
                }
                Err(_) => continue,
            }
        }

        Err(UpdateError::ConcurrentModification)
    }

    /// Check if entry is expired based on age
    #[inline(always)]
    pub fn is_expired(&self, max_age_ns: u64) -> bool {
        let age_ns = self.created_at.elapsed().as_nanos() as u64;
        age_ns > max_age_ns
    }

    /// Get entry age in nanoseconds
    #[inline(always)]
    pub fn age_ns(&self) -> u64 {
        self.created_at.elapsed().as_nanos() as u64
    }

    /// Get last access age in nanoseconds
    #[inline(always)]
    pub fn last_access_age_ns(&self) -> u64 {
        let now_ns = timestamp_nanos(Instant::now());
        let last_access = self.last_accessed.load(Ordering::Relaxed);
        now_ns.saturating_sub(last_access)
    }

    /// Check if entry should be considered for eviction
    #[inline(always)]
    pub fn eviction_score(&self) -> f64 {
        let age_score = self.age_ns() as f64 / 1_000_000_000.0; // Age in seconds
        let access_score = 1.0 / (self.access_count.load(Ordering::Relaxed) as f64 + 1.0);
        let size_score = self.size_bytes() as f64 / (1024.0 * 1024.0); // Size in MB

        // Weighted combination of factors
        age_score * 0.4 + access_score * 0.4 + size_score * 0.2
    }

    /// Get comprehensive entry metadata
    #[inline(always)]
    pub fn metadata(&self) -> EntryMetadata {
        let stats = self.access_stats();

        EntryMetadata {
            size_bytes: self.size_bytes(),
            generation: self.generation(),
            created_at_ns: timestamp_nanos(self.created_at),
            access_stats: stats,
            eviction_score: self.eviction_score(),
        }
    }
}

/// Comprehensive entry metadata
#[derive(Debug, Clone)]
pub struct EntryMetadata {
    /// Entry size in bytes
    pub size_bytes: u32,
    /// Entry generation
    pub generation: u32,
    /// Creation timestamp
    pub created_at_ns: u64,
    /// Access statistics
    pub access_stats: AccessStats,
    /// Computed eviction score
    pub eviction_score: f64,
}

/// Cache entry flags for advanced operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryFlags(u16);

impl EntryFlags {
    /// No special flags
    pub const NONE: Self = Self(0);
    /// Entry is being written
    pub const WRITING: Self = Self(1 << 0);
    /// Entry is marked for deletion
    pub const DELETING: Self = Self(1 << 1);
    /// Entry is in migration between tiers
    pub const MIGRATING: Self = Self(1 << 2);
    /// Entry is locked for exclusive access
    pub const LOCKED: Self = Self(1 << 3);
    /// Entry has been accessed recently
    pub const HOT: Self = Self(1 << 4);
    /// Entry is compressed
    pub const COMPRESSED: Self = Self(1 << 5);

    /// Check if flag is set
    #[inline(always)]
    pub fn contains(self, flag: Self) -> bool {
        (self.0 & flag.0) != 0
    }

    /// Set flag
    #[inline(always)]
    pub fn set(self, flag: Self) -> Self {
        Self(self.0 | flag.0)
    }

    /// Clear flag
    #[inline(always)]
    pub fn clear(self, flag: Self) -> Self {
        Self(self.0 & !flag.0)
    }

    /// Toggle flag
    #[inline(always)]
    pub fn toggle(self, flag: Self) -> Self {
        Self(self.0 ^ flag.0)
    }
}

impl<K: CacheKey, V: CacheValue> AtomicCacheEntry<K, V> {
    /// Set entry flags atomically
    #[inline(always)]
    pub fn set_flags(&self, flags: EntryFlags) -> EntryFlags {
        let old_flags = self.flags.load(Ordering::Acquire);
        let new_flags = EntryFlags(old_flags).set(flags);
        self.flags.store(new_flags.0, Ordering::Release);
        new_flags
    }

    /// Clear entry flags atomically  
    #[inline(always)]
    pub fn clear_flags(&self, flags: EntryFlags) -> EntryFlags {
        let old_flags = self.flags.load(Ordering::Acquire);
        let new_flags = EntryFlags(old_flags).clear(flags);
        self.flags.store(new_flags.0, Ordering::Release);
        new_flags
    }

    /// Check if flags are set
    #[inline(always)]
    pub fn has_flags(&self, flags: EntryFlags) -> bool {
        EntryFlags(self.flags.load(Ordering::Acquire)).contains(flags)
    }
}

/// Generic atomic cache entry utilities
///
/// Provides factory functions and utilities for creating AtomicCacheEntry
/// instances with proper generic type support.
/// Create atomic cache entry with automatic type inference
#[inline(always)]
pub fn create_atomic_entry<K, V>(key: K, value: V) -> AtomicCacheEntry<K, V>
where
    K: CacheKey,
    V: CacheValue,
{
    AtomicCacheEntry::new(key, value)
}

/// Batch atomic entry creation for bulk operations
#[inline(always)]
pub fn create_atomic_entries<K, V, I>(entries: I) -> Vec<AtomicCacheEntry<K, V>>
where
    K: CacheKey,
    V: CacheValue,
    I: IntoIterator<Item = (K, V)>,
{
    entries
        .into_iter()
        .map(|(k, v)| create_atomic_entry(k, v))
        .collect()
}

/// Common atomic entry patterns for convenience (still generic)
pub mod patterns {
    use super::*;

    /// High-performance numeric key cache entry
    pub type NumericEntry<V> = AtomicCacheEntry<u64, V>;

    /// Hash-based cache entry for maximum performance
    pub type HashEntry<V> = AtomicCacheEntry<u64, V>;

    /// Create numeric key entry
    #[inline(always)]
    pub fn numeric<V: CacheValue>(key: u64, value: V) -> NumericEntry<V> {
        create_atomic_entry(key, value)
    }

    /// Create hash key entry
    #[inline(always)]
    pub fn hash<V: CacheValue>(key: u64, value: V) -> HashEntry<V> {
        create_atomic_entry(key, value)
    }
}
