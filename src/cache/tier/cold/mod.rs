//! Cold tier persistent cache module
//!
//! This module provides persistent storage for the cold tier cache with
//! comprehensive serialization, compression, synchronization, and optimization.
//! The implementation is decomposed into focused submodules for maintainability.

use std::any::TypeId;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use self::data_structures::{ColdCacheKey, IndexEntry, PersistentColdTier};
use crate::cache::config::types::ColdTierConfig;
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};

// Module declarations
pub mod compaction_system;
pub mod compression;
pub mod compression_engine;
pub mod core;
pub mod data_structures;
pub mod metadata_index;
pub mod optimization;
pub mod serialization;
pub mod storage;
pub mod storage_manager;
pub mod sync;

/// Cold tier coordinator for managing mutable operations
pub struct ColdTierCoordinator {
    tiers: Arc<Mutex<HashMap<(TypeId, TypeId), Arc<Mutex<Box<dyn std::any::Any + Send + Sync>>>>>>,
}

static COLD_TIER_COORDINATOR: OnceLock<ColdTierCoordinator> = OnceLock::new();

impl ColdTierCoordinator {
    pub fn get() -> Result<&'static ColdTierCoordinator, CacheOperationError> {
        COLD_TIER_COORDINATOR.get_or_init(|| ColdTierCoordinator {
            tiers: Arc::new(Mutex::new(HashMap::new())),
        });

        COLD_TIER_COORDINATOR.get().ok_or_else(|| {
            CacheOperationError::resource_exhausted("Cold tier coordinator not available")
        })
    }

    pub fn register<K: CacheKey + 'static, V: CacheValue + 'static>(
        &self,
        tier: PersistentColdTier<K, V>,
    ) -> Result<(), CacheOperationError> {
        let mut tiers = self
            .tiers
            .lock()
            .map_err(|_| CacheOperationError::invalid_state("Coordinator lock poisoned"))?;

        let type_key = (TypeId::of::<K>(), TypeId::of::<V>());
        let boxed_tier = Box::new(tier) as Box<dyn std::any::Any + Send + Sync>;
        tiers.insert(type_key, Arc::new(Mutex::new(boxed_tier)));

        Ok(())
    }

    pub fn execute_operation<K, V, R, F>(&self, operation: F) -> Result<R, CacheOperationError>
    where
        K: CacheKey + 'static,
        V: CacheValue + 'static,
        F: FnOnce(&mut PersistentColdTier<K, V>) -> Result<R, CacheOperationError>,
    {
        let tiers = self
            .tiers
            .lock()
            .map_err(|_| CacheOperationError::invalid_state("Coordinator lock poisoned"))?;

        let type_key = (TypeId::of::<K>(), TypeId::of::<V>());
        let tier_mutex = tiers.get(&type_key).ok_or_else(|| {
            CacheOperationError::resource_exhausted("Cold tier not initialized for type")
        })?;

        let mut tier_guard = tier_mutex
            .lock()
            .map_err(|_| CacheOperationError::invalid_state("Tier lock poisoned"))?;

        let tier = tier_guard
            .downcast_mut::<PersistentColdTier<K, V>>()
            .ok_or_else(|| CacheOperationError::invalid_state("Type mismatch"))?;

        operation(tier)
    }

    pub fn execute_read_operation<K, V, R, F>(&self, operation: F) -> Result<R, CacheOperationError>
    where
        K: CacheKey + 'static,
        V: CacheValue + 'static,
        F: FnOnce(&PersistentColdTier<K, V>) -> Result<R, CacheOperationError>,
    {
        let tiers = self
            .tiers
            .lock()
            .map_err(|_| CacheOperationError::invalid_state("Coordinator lock poisoned"))?;

        let type_key = (TypeId::of::<K>(), TypeId::of::<V>());
        let tier_mutex = tiers.get(&type_key).ok_or_else(|| {
            CacheOperationError::resource_exhausted("Cold tier not initialized for type")
        })?;

        let tier_guard = tier_mutex
            .lock()
            .map_err(|_| CacheOperationError::invalid_state("Tier lock poisoned"))?;

        let tier = tier_guard
            .downcast_ref::<PersistentColdTier<K, V>>()
            .ok_or_else(|| CacheOperationError::invalid_state("Type mismatch"))?;

        operation(tier)
    }
}

/// Initialize cold tier for specific key-value types
pub fn init_cold_tier<K: CacheKey + 'static, V: CacheValue + 'static>(
    storage_path: &str,
) -> Result<(), CacheOperationError> {
    use arrayvec::ArrayString;

    let config = ColdTierConfig {
        enabled: true,
        storage_path: ArrayString::from(storage_path)
            .map_err(|_| CacheOperationError::invalid_state("Storage path too long"))?,
        max_size_bytes: 1024 * 1024 * 1024, // 1GB
        max_file_size: 100 * 1024 * 1024,   // 100MB
        compression_level: 6,
        auto_compact: true,
        compact_interval_ns: 3_600_000_000_000, // 1 hour
        mmap_size: 1024 * 1024 * 1024,
        write_buffer_size: 64 * 1024,
        _padding: [0; 2],
    };

    let tier = PersistentColdTier::<K, V>::new(config).map_err(|e| {
        CacheOperationError::io_failed(format!("Failed to initialize cold tier: {}", e))
    })?;

    let coordinator = ColdTierCoordinator::get()?;
    coordinator.register::<K, V>(tier)
}

/// Get value from cold tier cache
pub fn cold_get<K: CacheKey + 'static, V: CacheValue + serde::de::DeserializeOwned + 'static>(
    key: &K,
) -> Result<Option<Arc<V>>, CacheOperationError> {
    let coordinator = ColdTierCoordinator::get()?;
    coordinator.execute_read_operation::<K, V, Option<Arc<V>>, _>(|tier| Ok(tier.get(key)))
}

/// Get cache statistics  
pub fn get_stats<K: CacheKey + 'static, V: CacheValue + 'static>(
) -> Result<crate::cache::types::TierStatistics, CacheOperationError> {
    let coordinator = ColdTierCoordinator::get()?;
    coordinator.execute_read_operation::<K, V, crate::cache::types::TierStatistics, _>(|tier| {
        Ok(tier.stats())
    })
}

/// Get frequently accessed keys for promotion analysis
pub fn get_frequently_accessed_keys<K: CacheKey + Clone + 'static>(threshold: u32) -> Vec<K> {
    // For now return empty - implementation would require type parameter V
    // Real implementation would iterate metadata_index with proper type handling
    Vec::new()
}

/// Check if entry should be promoted to warm tier
pub fn should_promote_to_warm<K: CacheKey + 'static, V: CacheValue + 'static>(key: &K) -> bool {
    let coordinator = ColdTierCoordinator::get();
    if let Ok(coordinator) = coordinator {
        coordinator
            .execute_read_operation::<K, V, bool, _>(|tier| {
                let cold_key = ColdCacheKey::from_cache_key(key);
                if let Some(entry) = tier.metadata_index.get_entry(&cold_key) {
                    Ok(entry.access_count >= 5) // Promotion threshold
                } else {
                    Ok(false)
                }
            })
            .unwrap_or(false)
    } else {
        false
    }
}

/// Insert demoted entry from warm tier (for tier transitions)
pub fn insert_demoted<K: CacheKey + 'static, V: CacheValue + serde::Serialize + 'static>(
    key: K,
    value: Arc<V>,
) -> Result<(), CacheOperationError> {
    let coordinator = ColdTierCoordinator::get()?;
    coordinator.execute_operation::<K, V, (), _>(|tier| tier.put(key, value))
}

/// Remove entry for promotion to warm tier (for tier transitions)
pub fn remove_entry<K: CacheKey + 'static, V: CacheValue + 'static>(
    key: &K,
) -> Result<bool, CacheOperationError> {
    let coordinator = ColdTierCoordinator::get()?;
    coordinator.execute_operation::<K, V, bool, _>(|tier| Ok(tier.remove(key)))
}
