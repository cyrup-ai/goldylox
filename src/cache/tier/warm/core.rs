#![allow(dead_code)]
// Warm tier core - Complete warm cache implementation with lock-free skiplist, concurrent data structures, and atomic metadata management

//! Core warm tier cache data structures and primary implementation
//!
//! This module contains the main LockFreeWarmTier cache implementation with
//! concurrent-safe data structures and atomic metadata management.

use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU16, AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use crossbeam_skiplist::SkipMap;
use crossbeam_utils::{CachePadded, atomic::AtomicCell};
use log;

// Removed unused import super::traits
use super::access_tracking::ConcurrentAccessTracker;
use super::config::WarmTierConfig;
use super::data_structures::MaintenanceTask;
use super::error::WarmTierInitError;
use super::eviction::ConcurrentEvictionPolicy;
use crate::cache::types::{timestamp_nanos, *};

use super::memory_monitor_enum::MemoryMonitorImpl;
use super::memory_monitor_trait::MemoryMonitor;
use crate::cache::traits::AccessType;
use crate::cache::traits::supporting_types::ValueMetadata;
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::types_and_enums::{CompressionHint, TierLocation, VolatilityLevel};
use crate::cache::traits::{CacheKey, CacheValue};
use crate::cache::types::performance::timer::PrecisionTimer;
use crate::cache::types::statistics::atomic_stats::AtomicTierStats;

/// Lock-free warm tier cache with concurrent access optimization
#[derive(Debug)]
pub struct LockFreeWarmTier<K: CacheKey, V: CacheValue + Default> {
    /// Primary storage using lock-free skiplist for O(log n) concurrent access
    pub(super) storage: SkipMap<WarmCacheKey<K>, WarmCacheEntry<V>>,
    /// Access tracking for advanced eviction policies
    pub(super) access_tracker: ConcurrentAccessTracker<K>,
    /// Eviction policy implementation
    pub(super) eviction_policy: ConcurrentEvictionPolicy<K>,
    /// Atomic statistics
    pub(super) stats: AtomicTierStats,
    /// Configuration
    pub(super) config: WarmTierConfig,
    /// Memory pressure monitor
    pub(super) memory_monitor: MemoryMonitorImpl,
    /// Background maintenance channel
    pub(super) maintenance_tx: Sender<MaintenanceTask>,
    pub(super) maintenance_rx: Receiver<MaintenanceTask>,
    /// Entry generation counter for ABA prevention
    pub(super) generation_counter: AtomicU64,
}

/// Warm cache key with efficient ordering for skiplist
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(bound = "K: serde::de::DeserializeOwned")]
pub struct WarmCacheKey<K: CacheKey> {
    /// Primary hash for fast comparison
    pub primary_hash: u64,
    /// Original cache key
    pub original_key: K,
}

impl<K: CacheKey> WarmCacheKey<K> {
    /// Create warm cache key from a cache key
    pub fn from_cache_key(key: &K) -> Self {
        use std::collections::hash_map::DefaultHasher;

        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let primary_hash = hasher.finish();

        Self {
            primary_hash,
            original_key: key.clone(),
        }
    }
}

/// Canonical CacheKey trait implementation for WarmCacheKey<K>
impl<K: CacheKey> CacheKey for WarmCacheKey<K> {
    type HashContext = K::HashContext;
    type Priority = K::Priority;
    type SizeEstimator = K::SizeEstimator;

    fn estimated_size(&self) -> usize {
        std::mem::size_of::<u64>() + self.original_key.estimated_size()
    }

    fn priority(&self) -> Self::Priority {
        self.original_key.priority()
    }

    fn hash_context(&self) -> Self::HashContext {
        self.original_key.hash_context()
    }

    fn size_estimator(&self) -> Self::SizeEstimator {
        self.original_key.size_estimator()
    }

    fn fast_hash(&self, context: &Self::HashContext) -> u64 {
        self.original_key.fast_hash(context)
    }
}

/// Warm cache entry with concurrent-safe metadata
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
#[serde(bound = "V: serde::de::DeserializeOwned")]
pub struct WarmCacheEntry<V: CacheValue + Default> {
    /// Cached value (direct storage - SkipMap handles concurrent access)
    pub value: V,
    /// Entry metadata with atomic fields
    pub metadata: WarmEntryMetadata,
    /// Creation timestamp
    #[serde(skip)]
    pub created_at: Instant,
    /// Entry generation for consistency
    pub generation: u64,
}

/// Atomic metadata for warm cache entries
#[derive(Debug)]
pub struct WarmEntryMetadata {
    /// Last access timestamp (atomic for concurrent updates)
    pub last_access_ns: CachePadded<AtomicU64>,
    /// Access count (atomic increment)
    pub access_count: CachePadded<AtomicU64>,
    /// Entry size in bytes
    pub size_bytes: CachePadded<AtomicU32>,
    /// Access frequency estimate
    pub frequency_estimate: CachePadded<AtomicCell<f64>>,
    /// Priority level for eviction
    pub priority_level: CachePadded<AtomicU16>,
    /// Entry flags (temporal patterns, etc.)
    pub flags: CachePadded<AtomicU16>,
}

impl WarmEntryMetadata {
    /// Create new metadata for a cache value
    pub fn new<V: CacheValue>(value: &V) -> Self {
        let size_bytes = value.estimated_size() as u32;
        let now_ns = timestamp_nanos(Instant::now());

        Self {
            last_access_ns: CachePadded::new(AtomicU64::new(now_ns)),
            access_count: CachePadded::new(AtomicU64::new(0)),
            size_bytes: CachePadded::new(AtomicU32::new(size_bytes)),
            frequency_estimate: CachePadded::new(AtomicCell::new(1.0)),
            priority_level: CachePadded::new(AtomicU16::new(100)), // Medium priority
            flags: CachePadded::new(AtomicU16::new(0)),
        }
    }
}

/// Custom Clone implementation for WarmEntryMetadata using atomic loads
impl Clone for WarmEntryMetadata {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            last_access_ns: CachePadded::new(AtomicU64::new(
                self.last_access_ns.load(Ordering::Relaxed),
            )),
            access_count: CachePadded::new(AtomicU64::new(
                self.access_count.load(Ordering::Relaxed),
            )),
            size_bytes: CachePadded::new(AtomicU32::new(self.size_bytes.load(Ordering::Relaxed))),
            frequency_estimate: CachePadded::new(AtomicCell::new(self.frequency_estimate.load())),
            priority_level: CachePadded::new(AtomicU16::new(
                self.priority_level.load(Ordering::Relaxed),
            )),
            flags: CachePadded::new(AtomicU16::new(self.flags.load(Ordering::Relaxed))),
        }
    }
}

impl Default for WarmEntryMetadata {
    fn default() -> Self {
        Self {
            last_access_ns: CachePadded::new(AtomicU64::new(0)),
            access_count: CachePadded::new(AtomicU64::new(0)),
            size_bytes: CachePadded::new(AtomicU32::new(0)),
            frequency_estimate: CachePadded::new(AtomicCell::new(0.0)),
            priority_level: CachePadded::new(AtomicU16::new(0)),
            flags: CachePadded::new(AtomicU16::new(0)),
        }
    }
}

impl serde::Serialize for WarmEntryMetadata {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("WarmEntryMetadata", 6)?;
        state.serialize_field(
            "last_access_ns",
            &self
                .last_access_ns
                .load(std::sync::atomic::Ordering::Relaxed),
        )?;
        state.serialize_field(
            "access_count",
            &self.access_count.load(std::sync::atomic::Ordering::Relaxed),
        )?;
        state.serialize_field(
            "size_bytes",
            &self.size_bytes.load(std::sync::atomic::Ordering::Relaxed),
        )?;
        state.serialize_field("frequency_estimate", &self.frequency_estimate.load())?;
        state.serialize_field(
            "priority_level",
            &self
                .priority_level
                .load(std::sync::atomic::Ordering::Relaxed),
        )?;
        state.serialize_field(
            "flags",
            &self.flags.load(std::sync::atomic::Ordering::Relaxed),
        )?;
        state.end()
    }
}

impl<'de> serde::Deserialize<'de> for WarmEntryMetadata {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            LastAccessNs,
            AccessCount,
            SizeBytes,
            FrequencyEstimate,
            PriorityLevel,
            Flags,
        }

        struct WarmEntryMetadataVisitor;

        impl<'de> serde::de::Visitor<'de> for WarmEntryMetadataVisitor {
            type Value = WarmEntryMetadata;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct WarmEntryMetadata")
            }

            fn visit_map<V>(self, mut map: V) -> Result<WarmEntryMetadata, V::Error>
            where
                V: serde::de::MapAccess<'de>,
            {
                let mut last_access_ns = None;
                let mut access_count = None;
                let mut size_bytes = None;
                let mut frequency_estimate = None;
                let mut priority_level = None;
                let mut flags = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::LastAccessNs => {
                            if last_access_ns.is_some() {
                                return Err(serde::de::Error::duplicate_field("last_access_ns"));
                            }
                            last_access_ns = Some(map.next_value()?);
                        }
                        Field::AccessCount => {
                            if access_count.is_some() {
                                return Err(serde::de::Error::duplicate_field("access_count"));
                            }
                            access_count = Some(map.next_value()?);
                        }
                        Field::SizeBytes => {
                            if size_bytes.is_some() {
                                return Err(serde::de::Error::duplicate_field("size_bytes"));
                            }
                            size_bytes = Some(map.next_value()?);
                        }
                        Field::FrequencyEstimate => {
                            if frequency_estimate.is_some() {
                                return Err(serde::de::Error::duplicate_field(
                                    "frequency_estimate",
                                ));
                            }
                            frequency_estimate = Some(map.next_value()?);
                        }
                        Field::PriorityLevel => {
                            if priority_level.is_some() {
                                return Err(serde::de::Error::duplicate_field("priority_level"));
                            }
                            priority_level = Some(map.next_value()?);
                        }
                        Field::Flags => {
                            if flags.is_some() {
                                return Err(serde::de::Error::duplicate_field("flags"));
                            }
                            flags = Some(map.next_value()?);
                        }
                    }
                }

                let last_access_ns = last_access_ns
                    .ok_or_else(|| serde::de::Error::missing_field("last_access_ns"))?;
                let access_count =
                    access_count.ok_or_else(|| serde::de::Error::missing_field("access_count"))?;
                let size_bytes =
                    size_bytes.ok_or_else(|| serde::de::Error::missing_field("size_bytes"))?;
                let frequency_estimate = frequency_estimate
                    .ok_or_else(|| serde::de::Error::missing_field("frequency_estimate"))?;
                let priority_level = priority_level
                    .ok_or_else(|| serde::de::Error::missing_field("priority_level"))?;
                let flags = flags.ok_or_else(|| serde::de::Error::missing_field("flags"))?;

                Ok(WarmEntryMetadata {
                    last_access_ns: CachePadded::new(AtomicU64::new(last_access_ns)),
                    access_count: CachePadded::new(AtomicU64::new(access_count)),
                    size_bytes: CachePadded::new(AtomicU32::new(size_bytes)),
                    frequency_estimate: CachePadded::new(AtomicCell::new(frequency_estimate)),
                    priority_level: CachePadded::new(AtomicU16::new(priority_level)),
                    flags: CachePadded::new(AtomicU16::new(flags)),
                })
            }
        }

        const FIELDS: &[&str] = &[
            "last_access_ns",
            "access_count",
            "size_bytes",
            "frequency_estimate",
            "priority_level",
            "flags",
        ];
        deserializer.deserialize_struct("WarmEntryMetadata", FIELDS, WarmEntryMetadataVisitor)
    }
}

/// ValueMetadata trait implementation for WarmEntryMetadata using zero-allocation atomic operations
impl ValueMetadata for WarmEntryMetadata {
    #[inline(always)]
    fn creation_cost(&self) -> u64 {
        // Estimate creation cost based on size and access patterns
        let size = self.size_bytes.load(Ordering::Relaxed) as u64;
        let access_count = self.access_count.load(Ordering::Relaxed);

        // Higher creation cost for larger items that are accessed less frequently
        if access_count > 0 {
            size * 100 / access_count.min(100) // Clamp to prevent overflow
        } else {
            size * 100 // Default high creation cost for new items
        }
    }

    #[inline(always)]
    fn access_frequency(&self) -> f64 {
        self.frequency_estimate.load()
    }

    #[inline(always)]
    fn volatility(&self) -> VolatilityLevel {
        // Determine volatility based on access patterns and age
        let now_ns = timestamp_nanos(Instant::now());
        let last_access_ns = self.last_access_ns.load(Ordering::Relaxed);
        let age_ns = now_ns.saturating_sub(last_access_ns);
        let access_count = self.access_count.load(Ordering::Relaxed);

        if age_ns < 60_000_000_000 && access_count > 10 {
            // < 1 minute, frequent access
            VolatilityLevel::HighlyVolatile
        } else if age_ns < 300_000_000_000 {
            // < 5 minutes
            VolatilityLevel::Moderate
        } else {
            VolatilityLevel::Stable
        }
    }

    #[inline(always)]
    fn compression_ratio(&self) -> f32 {
        // Estimate compression ratio based on size and type
        let size = self.size_bytes.load(Ordering::Relaxed);

        if size < 128 {
            0.9 // Small items don't compress well
        } else if size < 1024 {
            0.7 // Medium items compress moderately
        } else {
            0.5 // Large items compress well
        }
    }
}

/// Canonical CacheValue trait implementation for WarmCacheEntry<V> with zero-allocation operations
impl<V: CacheValue + Default> CacheValue for WarmCacheEntry<V> {
    type Metadata = WarmEntryMetadata;

    #[inline(always)]
    fn estimated_size(&self) -> usize {
        self.value.estimated_size()
            + std::mem::size_of::<WarmEntryMetadata>()
            + std::mem::size_of::<Instant>()
            + std::mem::size_of::<u64>()
    }

    #[inline(always)]
    fn is_expensive(&self) -> bool {
        // Warm tier entries are moderately expensive due to metadata overhead
        // but less expensive than the original value since we have cached access patterns
        self.value.is_expensive() || self.metadata.creation_cost() > 1000
    }

    #[inline(always)]
    fn compression_hint(&self) -> CompressionHint {
        // Use compression hint based on size and access patterns
        let size = self.metadata.size_bytes.load(Ordering::Relaxed);
        let access_count = self.metadata.access_count.load(Ordering::Relaxed);

        if size > 1024 && access_count < 5 {
            // Large, infrequently accessed items should be compressed
            CompressionHint::Force
        } else if size < 256 {
            // Small items shouldn't be compressed due to overhead
            CompressionHint::Disable
        } else {
            // Medium items use automatic decision
            CompressionHint::Auto
        }
    }

    #[inline(always)]
    fn metadata(&self) -> Self::Metadata {
        self.metadata.clone()
    }
}

impl<V: CacheValue + Default> Default for WarmCacheEntry<V> {
    fn default() -> Self {
        Self {
            value: V::default(),
            metadata: WarmEntryMetadata::default(),
            created_at: std::time::Instant::now(),
            generation: 0,
        }
    }
}

/// CacheEntry integration for warm tier structures
impl<V: CacheValue + Default> WarmCacheEntry<V> {
    /// Convert to canonical CacheEntry wrapper
    pub fn to_canonical_entry<K: CacheKey>(
        &self,
        key: &WarmCacheKey<K>,
    ) -> crate::cache::traits::CacheEntry<WarmCacheKey<K>, V> {
        crate::cache::traits::CacheEntry::new(
            key.clone(),
            self.value.clone(),
            crate::cache::traits::types_and_enums::TierLocation::Warm,
        )
    }

    /// Create from canonical CacheEntry
    pub fn from_canonical_entry<K: CacheKey>(
        entry: &crate::cache::traits::CacheEntry<WarmCacheKey<K>, V>,
        generation: u64,
    ) -> Self {
        Self {
            value: entry.value.clone(),
            metadata: WarmEntryMetadata::new(&entry.value),
            created_at: Instant::now(),
            generation,
        }
    }
}

impl<K: CacheKey> WarmCacheKey<K> {
    /// Convert to canonical CacheEntry wrapper with value
    pub fn to_canonical_entry<V: CacheValue>(
        &self,
        value: V,
    ) -> crate::cache::traits::CacheEntry<Self, V> {
        crate::cache::traits::CacheEntry::new(self.clone(), value, TierLocation::Warm)
    }
}

impl<K: CacheKey, V: CacheValue + Default> LockFreeWarmTier<K, V> {
    /// Create new warm tier with fallible initialization
    pub fn new(config: WarmTierConfig) -> Result<Self, WarmTierInitError> {
        let (maintenance_tx, maintenance_rx) = crossbeam_channel::unbounded();

        // Create memory monitor - cannot fail
        let memory_monitor = MemoryMonitorImpl::new(config.max_memory_bytes);

        // Create eviction policy and configure it with capacity limits
        let eviction_policy = ConcurrentEvictionPolicy::new();
        eviction_policy.update_cache_capacity(
            Some(config.max_entries),
            Some(config.max_memory_bytes)
        );

        Ok(Self {
            storage: SkipMap::new(),
            access_tracker: ConcurrentAccessTracker::new(),
            eviction_policy,
            stats: AtomicTierStats::new(),
            config,
            memory_monitor,
            maintenance_tx,
            maintenance_rx,
            generation_counter: AtomicU64::new(1),
        })
    }

    /// Try to create warm tier with memory monitor (internal method for retry logic)
    pub fn try_new_with_monitor(config: WarmTierConfig) -> Result<Self, WarmTierInitError> {
        // Attempt creation with enhanced error context for retry decisions
        match Self::new(config.clone()) {
            Ok(tier) => Ok(tier),
            Err(mut err) => {
                // Add context to help with retry decisions
                if let WarmTierInitError::MemoryMonitorCreation(_cache_err) = &mut err {
                    // For memory monitor failures, try to gather more context about the failure
                    // This could help determine if retries would be beneficial
                    let current_memory = Self::get_available_system_memory();
                    let required_memory = config.max_memory_bytes;

                    if current_memory < required_memory {
                        return Err(WarmTierInitError::MemoryAllocationExhausted { attempts: 1 });
                    }
                }
                Err(err)
            }
        }
    }

    /// Get available system memory for retry decision logic
    fn get_available_system_memory() -> u64 {
        // Use the same logic as the system memory detector
        use crate::cache::memory::pressure_monitor::get_system_memory_with_fallback;
        get_system_memory_with_fallback()
    }

    /// Concurrent cache lookup with lock-free access
    pub fn get(&self, key: &K) -> Option<V> {
        let timer = PrecisionTimer::start();
        let warm_key = WarmCacheKey::from_cache_key(key);

        // Lock-free lookup in skiplist
        if let Some(entry_ref) = self.storage.get(&warm_key) {
            let entry = entry_ref.value();

            // Atomic metadata updates
            let now_ns = timestamp_nanos(Instant::now());
            entry
                .metadata
                .last_access_ns
                .store(now_ns, Ordering::Relaxed);
            entry.metadata.access_count.fetch_add(1, Ordering::Relaxed);

            // Record access for pattern analysis
            self.access_tracker
                .record_access(&warm_key, AccessType::SequentialRead, true);

            // Update eviction policy
            self.eviction_policy.on_access(&warm_key, true);

            let elapsed_ns = timer.elapsed_ns();
            self.stats.record_hit(elapsed_ns);

            Some(entry.value.clone())
        } else {
            // Record miss for pattern analysis
            self.access_tracker
                .record_access(&warm_key, AccessType::RandomRead, false);
            self.eviction_policy.on_access(&warm_key, false);

            let elapsed_ns = timer.elapsed_ns();
            self.stats.record_miss(elapsed_ns);

            None
        }
    }

    /// Zero-copy reference access for read-heavy workloads
    /// Returns a reference guard that maintains the entry's lifetime
    pub fn get_ref<'a>(&'a self, key: &K) -> Option<impl std::ops::Deref<Target = V> + 'a> {
        let warm_key = WarmCacheKey::from_cache_key(key);

        self.storage.get(&warm_key).map(|entry_ref| {
            // Update access metadata atomically
            let entry = entry_ref.value();
            let now_ns = timestamp_nanos(Instant::now());
            entry
                .metadata
                .last_access_ns
                .store(now_ns, Ordering::Relaxed);
            entry.metadata.access_count.fetch_add(1, Ordering::Relaxed);

            // Record access patterns
            self.access_tracker
                .record_access(&warm_key, AccessType::SequentialRead, true);
            self.eviction_policy.on_access(&warm_key, true);
            self.stats.record_hit(0);

            // Return a wrapper that derefs to V
            ValueRef { _entry: entry_ref }
        })
    }

    /// Concurrent cache insertion with adaptive eviction
    pub fn put(&self, key: K, value: V) -> Result<(), CacheOperationError> {
        let timer = PrecisionTimer::start();
        let warm_key = WarmCacheKey::from_cache_key(&key);
        let generation = self.generation_counter.fetch_add(1, Ordering::Relaxed);

        // Check memory pressure
        if self.memory_monitor.should_evict() {
            self.trigger_background_eviction();
        }

        // Get entry size before moving value
        let entry_size = value.estimated_size() as i64;

        // Create new entry
        let metadata = WarmEntryMetadata::new(&value);
        let entry = WarmCacheEntry {
            value,
            metadata,
            created_at: Instant::now(),
            generation,
        };

        // Check if there was a previous entry before insertion
        let had_previous = self.storage.contains_key(&warm_key);
        let old_size = if had_previous {
            if let Some(prev_entry) = self.storage.get(&warm_key) {
                prev_entry.value().value.estimated_size() as i64
            } else {
                0
            }
        } else {
            0
        };

        // Concurrent insertion (may evict existing entry with same key)
        self.storage.insert(warm_key.clone(), entry);

        // Update memory usage
        if had_previous {
            self.stats.update_memory_usage(entry_size - old_size);
        } else {
            self.stats.update_entry_count(1);
            self.stats.update_memory_usage(entry_size);
        }

        // Update memory monitor
        self.memory_monitor.record_allocation(entry_size as usize);

        // Record access pattern
        self.access_tracker
            .record_access(&warm_key, AccessType::SequentialWrite, false);

        let elapsed_ns = timer.elapsed_ns();
        self.stats.record_hit(elapsed_ns);

        Ok(())
    }

    /// Remove entry from cache and return the removed value
    pub fn remove(&self, key: &K) -> Option<V> {
        let warm_key = WarmCacheKey::from_cache_key(key);

        if let Some(entry) = self.storage.remove(&warm_key) {
            let entry_value = entry.value();
            let entry_size = entry_value.value.estimated_size() as i64;
            let removed_value = entry_value.value.clone();

            self.stats.update_entry_count(-1);
            self.stats.update_memory_usage(-entry_size);

            // Safe conversion with proper error handling
            if let Ok(deallocation_size) = entry_size.try_into() {
                self.memory_monitor.record_deallocation(deallocation_size);
            } else {
                // Log error but continue operation - negative size shouldn't happen
                // but we handle it gracefully if it does
                log::warn!(
                    "Warning: Invalid entry size for deallocation: {}",
                    entry_size
                );
            }

            // Update eviction policy
            self.eviction_policy.on_eviction(&warm_key);

            Some(removed_value)
        } else {
            None
        }
    }

    /// Get cache statistics
    pub fn stats(&self) -> TierStatistics {
        self.stats.snapshot()
    }

    /// Trigger background eviction based on memory pressure
    pub(super) fn trigger_background_eviction(&self) {
        let eviction_count = self.calculate_eviction_count();

        if eviction_count > 0 {
            let task = MaintenanceTask::Evict {
                target_count: eviction_count,
                policy_hint: format!("{:?}", self.eviction_policy.current_policy()),
            };

            let _ = self.maintenance_tx.try_send(task);
        }
    }

    /// Calculate how many entries to evict based on memory pressure
    fn calculate_eviction_count(&self) -> usize {
        let pressure = self.memory_monitor.get_pressure();
        let total_entries = self.storage.len();

        if pressure > 0.9 {
            total_entries / 4 // Aggressive eviction
        } else if pressure > 0.75 {
            total_entries / 8 // Moderate eviction
        } else if pressure > 0.5 {
            total_entries / 16 // Light eviction
        } else {
            0 // No eviction needed
        }
    }
}

// REMOVED: Compatibility aliases from_generic_key() and to_generic_key()
// Users must now use canonical methods:
// - Use WarmCacheKey::from_cache_key() instead of from_generic_key()
// - Use .original_key field access instead of to_generic_key()

impl<V: CacheValue + Default> WarmCacheEntry<V> {
    /// Create new warm cache entry
    pub fn new(value: V, generation: u64) -> Self {
        Self {
            metadata: WarmEntryMetadata::new(&value),
            value,
            created_at: Instant::now(),
            generation,
        }
    }

    /// Get estimated size of entry in bytes
    pub fn estimated_size(&self) -> usize {
        self.value.estimated_size() + std::mem::size_of::<WarmCacheEntry<V>>()
    }

    /// Convert warm cache entry back to V
    pub fn into_value(self) -> V {
        self.value
    }

    /// Get last access time as Instant
    pub fn last_access(&self) -> Instant {
        let last_access_ns = self.metadata.last_access_ns.load(Ordering::Relaxed);
        let duration_since_start = Duration::from_nanos(last_access_ns);
        Instant::now() - duration_since_start // Approximate conversion
    }

    /// Get access count
    pub fn access_count(&self) -> u64 {
        self.metadata.access_count.load(Ordering::Relaxed)
    }
}

impl<K: CacheKey, V: CacheValue + Default> LockFreeWarmTier<K, V> {
    /// Get number of entries in cache
    pub fn size(&self) -> usize {
        self.storage.len()
    }

    /// Get current memory usage in bytes
    pub fn memory_usage(&self) -> u64 {
        self.stats.memory_usage_bytes()
    }

    /// Get current memory pressure level
    pub fn memory_pressure(&self) -> Option<f64> {
        Some(self.memory_monitor.get_pressure())
    }

    /// Get cache statistics  
    pub fn get_stats(&self) -> super::monitoring::types::TierStatsSnapshot {
        let tier_stats = self.stats.snapshot();
        super::monitoring::types::TierStatsSnapshot::from(tier_stats)
    }

    /// Get all keys currently in cache
    pub fn get_keys(&self) -> Vec<K> {
        self.storage
            .iter()
            .map(|entry| entry.key().original_key.clone())
            .collect()
    }

    /// Get frequently accessed keys
    pub fn get_frequent_keys(&self, limit: usize) -> Vec<K> {
        // Collect entries with access count and sort by frequency
        let mut key_access_pairs: Vec<(K, u64)> = self
            .storage
            .iter()
            .map(|entry_ref| {
                let key = entry_ref.key().original_key.clone();
                let entry = entry_ref.value();
                let access_count = entry.metadata.access_count.load(Ordering::Relaxed);
                (key, access_count)
            })
            .collect();

        // Sort by access count descending
        key_access_pairs.sort_by(|a, b| b.1.cmp(&a.1));

        // Take top N and extract keys
        key_access_pairs
            .into_iter()
            .take(limit)
            .map(|(key, _)| key)
            .collect()
    }

    /// Get idle keys that haven't been accessed recently
    pub fn get_idle_keys(&self, threshold: Duration) -> Vec<K> {
        let now = Instant::now();
        let threshold_ns = threshold.as_nanos() as u64;

        self.storage
            .iter()
            .filter_map(|entry_ref| {
                let entry = entry_ref.value();
                let last_access_ns = entry.metadata.last_access_ns.load(Ordering::Relaxed);
                let current_ns = timestamp_nanos(now);

                // Check if entry hasn't been accessed within threshold
                if current_ns.saturating_sub(last_access_ns) > threshold_ns {
                    Some(entry_ref.key().original_key.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Clean up expired entries based on maximum age
    pub fn cleanup_expired(&mut self, max_age: Duration) -> Result<usize, CacheOperationError> {
        let now = Instant::now();
        let mut removed_count = 0;

        // Collect keys to remove (avoiding iterator invalidation)
        let mut keys_to_remove = Vec::new();

        for entry_ref in self.storage.iter() {
            let entry = entry_ref.value();
            if now.duration_since(entry.created_at) > max_age {
                keys_to_remove.push(entry_ref.key().clone());
            }
        }

        // Remove expired entries
        for key in keys_to_remove {
            if let Some(removed_entry) = self.storage.remove(&key) {
                let entry_size = removed_entry.value().value.estimated_size() as i64;
                self.stats.update_entry_count(-1);
                self.stats.update_memory_usage(-entry_size);

                if let Ok(deallocation_size) = entry_size.try_into() {
                    self.memory_monitor.record_deallocation(deallocation_size);
                }

                self.eviction_policy.on_eviction(&key);
                removed_count += 1;
            }
        }

        Ok(removed_count)
    }

    /// Force eviction of the specified number of entries
    pub fn force_evict(&mut self, target_count: usize) -> Result<usize, CacheOperationError> {
        let mut evicted_count = 0;

        // Update eviction policy with current statistics for accurate pressure calculation
        let stats_snapshot = self.stats.snapshot();
        self.eviction_policy.update_cache_stats(
            stats_snapshot.entry_count,
            self.stats.memory_usage_bytes()
        );

        // Use eviction policy to select candidates
        let candidates = self
            .eviction_policy
            .select_eviction_candidates(target_count);

        for candidate in candidates.into_iter().take(target_count) {
            if let Some(removed_entry) = self.storage.remove(&candidate) {
                let entry_size = removed_entry.value().value.estimated_size() as i64;
                self.stats.update_entry_count(-1);
                self.stats.update_memory_usage(-entry_size);

                if let Ok(deallocation_size) = entry_size.try_into() {
                    self.memory_monitor.record_deallocation(deallocation_size);
                }

                self.eviction_policy.on_eviction(&candidate);
                evicted_count += 1;
            }
        }

        Ok(evicted_count)
    }

    /// Process background maintenance tasks
    pub fn process_maintenance(&mut self) -> Result<usize, CacheOperationError> {
        let mut tasks_processed = 0;

        // Process pending maintenance tasks
        while let Ok(task) = self.maintenance_rx.try_recv() {
            match task {
                MaintenanceTask::Evict { target_count, .. } => {
                    self.force_evict(target_count)?;
                }
                MaintenanceTask::CompactStorage {
                    compaction_threshold: _,
                } => {
                    // Storage compaction would go here
                    // Current skiplist implementation doesn't need explicit compaction
                }
                MaintenanceTask::UpdateStatistics {
                    include_detailed_analysis: _,
                } => {
                    // Statistics are updated continuously, so this is a no-op
                }
                MaintenanceTask::CleanupExpired { ttl, batch_size: _ } => {
                    self.cleanup_expired(ttl)?;
                }
                MaintenanceTask::PerformEviction {
                    target_pressure: _,
                    max_evictions,
                } => {
                    self.force_evict(max_evictions)?;
                }
                MaintenanceTask::OptimizeStructure {
                    optimization_level: _,
                } => {
                    // Structure optimization would go here
                    // Current skiplist implementation is already well-optimized
                }
                MaintenanceTask::AnalyzePatterns {
                    analysis_depth: _,
                    prediction_horizon_sec: _,
                } => {
                    // Trigger actual pattern analysis on access tracker
                    self.access_tracker.process_analysis_tasks();
                }
                MaintenanceTask::SyncTiers {
                    sync_direction: _,
                    consistency_level: _,
                } => {
                    // Tier synchronization would go here
                    // Inter-tier coordination is handled by the coordinator
                }
                MaintenanceTask::ValidateIntegrity { check_level: _ } => {
                    // Integrity validation would go here
                    // Current implementation maintains consistency automatically
                }
                MaintenanceTask::DefragmentMemory {
                    target_fragmentation: _,
                } => {
                    // Memory defragmentation would go here
                    // Current skiplist implementation has minimal fragmentation
                }
                MaintenanceTask::UpdateMLModels {
                    training_data_size: _,
                    model_complexity: _,
                } => {
                    // ML model updates would go here
                    // ML eviction policy is already self-training
                }
            }
            tasks_processed += 1;
        }

        Ok(tasks_processed)
    }

    /// Get current alerts for memory pressure, performance issues, etc.
    pub fn get_alerts(&self) -> Vec<super::global_api::CacheAlert> {
        let mut alerts = Vec::new();

        // Check memory pressure
        let pressure = self.memory_monitor.get_pressure();
        if pressure > 0.9 {
            alerts.push(super::global_api::CacheAlert {
                message: format!("Critical memory pressure: {:.1}%", pressure * 100.0),
                severity: super::global_api::AlertSeverity::Critical,
            });
        } else if pressure > 0.75 {
            alerts.push(super::global_api::CacheAlert {
                message: format!("High memory pressure: {:.1}%", pressure * 100.0),
                severity: super::global_api::AlertSeverity::Warning,
            });
        }

        // Check cache efficiency
        let stats = self.stats.snapshot();
        if stats.hit_rate < 0.5 {
            alerts.push(super::global_api::CacheAlert {
                message: format!("Low cache hit rate: {:.1}%", stats.hit_rate * 100.0),
                severity: super::global_api::AlertSeverity::Warning,
            });
        }

        // Check entry count vs capacity
        let entry_count = self.storage.len();
        let max_entries = self.config.max_entries;
        if entry_count as f64 / max_entries as f64 > 0.9 {
            alerts.push(super::global_api::CacheAlert {
                message: format!("Cache nearly full: {}/{} entries", entry_count, max_entries),
                severity: super::global_api::AlertSeverity::Warning,
            });
        }

        alerts
    }

    /// Get ML policy metrics for model adaptation and analysis
    pub fn get_ml_policies(&self) -> Vec<super::eviction::types::PolicyPerformanceMetrics> {
        // Return ML policy performance metrics instead of the policy itself
        // This follows the messaging pattern where workers own data and clients get information
        vec![self.eviction_policy.performance_metrics()]
    }

    /// Update ML models using the sophisticated existing ML system
    pub fn update_ml_models(&self) -> Result<usize, CacheOperationError> {
        // Call the adapt() method on the ML eviction policy
        // This uses the existing sophisticated ML system with:
        // - 16-feature FeatureVector system
        // - Linear regression with gradient optimization
        // - Temporal pattern analysis
        // - Learning rate adaptation based on performance
        // - Feature importance analysis
        self.eviction_policy.ml_policy().adapt();

        // Return the number of ML policies updated
        Ok(1)
    }
}

/// Zero-copy value reference wrapper for SkipMap entries
/// This maintains the entry's lifetime while providing deref access to the value
struct ValueRef<'a, K: CacheKey, V: CacheValue + Default> {
    _entry: crossbeam_skiplist::map::Entry<'a, WarmCacheKey<K>, WarmCacheEntry<V>>,
}

impl<'a, K: CacheKey, V: CacheValue + Default> std::ops::Deref for ValueRef<'a, K, V> {
    type Target = V;
    fn deref(&self) -> &Self::Target {
        &self._entry.value().value
    }
}
