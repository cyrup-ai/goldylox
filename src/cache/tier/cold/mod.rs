//! Cold tier persistent cache module
//!
//! This module provides persistent storage for the cold tier cache with
//! lock-free message passing architecture for thread safety.

use std::any::TypeId;
use std::sync::OnceLock;
use crossbeam_channel::{unbounded, Sender, Receiver};
use bincode::{config, encode_to_vec, decode_from_slice};

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

/// Cold tier service message
enum ColdTierMessage {
    Register {
        type_key: (TypeId, TypeId),
        tier: Box<dyn std::any::Any + Send + Sync>,
        response: Sender<Result<(), CacheOperationError>>,
    },
    ExecuteOp {
        type_key: (TypeId, TypeId),
        op: Box<dyn FnOnce(&mut dyn std::any::Any) -> Vec<u8> + Send>,
        response: Sender<Result<Vec<u8>, CacheOperationError>>,
    },
    ExecuteReadOp {
        type_key: (TypeId, TypeId),
        op: Box<dyn FnOnce(&dyn std::any::Any) -> Vec<u8> + Send>,
        response: Sender<Result<Vec<u8>, CacheOperationError>>,
    },
}

/// Cold tier service that owns all state (no locks needed)
struct ColdTierService {
    receiver: Receiver<ColdTierMessage>,
}

impl ColdTierService {
    fn run(self) {
        // Service thread owns ALL tier instances - no sharing, no locks
        let mut tiers = std::collections::HashMap::<
            (TypeId, TypeId),
            Box<dyn std::any::Any + Send + Sync>
        >::new();
        
        while let Ok(msg) = self.receiver.recv() {
            match msg {
                ColdTierMessage::Register { type_key, tier, response } => {
                    tiers.insert(type_key, tier);
                    let _ = response.send(Ok(()));
                }
                ColdTierMessage::ExecuteOp { type_key, op, response } => {
                    if let Some(tier) = tiers.get_mut(&type_key) {
                        let result = op(&mut **tier);
                        let _ = response.send(Ok(result));
                    } else {
                        let _ = response.send(Err(CacheOperationError::resource_exhausted(
                            "Cold tier not initialized for type"
                        )));
                    }
                }
                ColdTierMessage::ExecuteReadOp { type_key, op, response } => {
                    if let Some(tier) = tiers.get(&type_key) {
                        let result = op(&**tier);
                        let _ = response.send(Ok(result));
                    } else {
                        let _ = response.send(Err(CacheOperationError::resource_exhausted(
                            "Cold tier not initialized for type"
                        )));
                    }
                }
            }
        }
    }
}

static COLD_TIER_SERVICE_TX: OnceLock<Sender<ColdTierMessage>> = OnceLock::new();

/// Cold tier coordinator for managing mutable operations
pub struct ColdTierCoordinator {
    sender: Sender<ColdTierMessage>,
}

impl ColdTierCoordinator {
    pub fn get() -> Result<&'static ColdTierCoordinator, CacheOperationError> {
        static COORDINATOR: OnceLock<ColdTierCoordinator> = OnceLock::new();
        
        let sender = COLD_TIER_SERVICE_TX.get_or_init(|| {
            let (tx, rx) = unbounded();
            
            std::thread::Builder::new()
                .name("cold-tier-service".to_string())
                .spawn(move || {
                    let service = ColdTierService {
                        receiver: rx,
                    };
                    service.run();
                })
                .expect("Failed to spawn cold tier service");
            
            tx
        });
        
        Ok(COORDINATOR.get_or_init(|| ColdTierCoordinator {
            sender: sender.clone(),
        }))
    }

    pub fn register<K: CacheKey + 'static, V: CacheValue + 'static>(
        &self,
        tier: PersistentColdTier<K, V>,
    ) -> Result<(), CacheOperationError> {
        let type_key = (TypeId::of::<K>(), TypeId::of::<V>());
        let boxed_tier = Box::new(tier) as Box<dyn std::any::Any + Send + Sync>;
        let (tx, rx) = unbounded();
        
        self.sender.send(ColdTierMessage::Register {
            type_key,
            tier: boxed_tier,
            response: tx,
        }).map_err(|_| CacheOperationError::InternalError("Service unavailable".to_string()))?;
        
        rx.recv().map_err(|_| CacheOperationError::TimeoutError)?
    }

    pub fn execute_operation<K, V, R, F>(&self, operation: F) -> Result<R, CacheOperationError>
    where
        K: CacheKey + 'static,
        V: CacheValue + 'static,
        F: FnOnce(&mut PersistentColdTier<K, V>) -> Result<R, CacheOperationError>,
        R: bincode::Encode + bincode::Decode + 'static,
    {
        let type_key = (TypeId::of::<K>(), TypeId::of::<V>());
        let (tx, rx) = unbounded();
        
        let op = Box::new(move |tier_any: &mut dyn std::any::Any| -> Vec<u8> {
            if let Some(tier) = tier_any.downcast_mut::<PersistentColdTier<K, V>>() {
                match operation(tier) {
                    Ok(result) => encode_to_vec(&result, config::standard()).unwrap_or_default(),
                    Err(_) => Vec::new(),
                }
            } else {
                Vec::new()
            }
        });
        
        self.sender.send(ColdTierMessage::ExecuteOp {
            type_key,
            op,
            response: tx,
        }).map_err(|_| CacheOperationError::InternalError("Service unavailable".to_string()))?;
        
        let serialized = rx.recv().map_err(|_| CacheOperationError::TimeoutError)??;
        
        decode_from_slice(&serialized, config::standard())
            .map(|(decoded, _len)| decoded)
            .map_err(|_| CacheOperationError::InternalError("Deserialization failed".to_string()))
    }

    pub fn execute_read_operation<K, V, R, F>(&self, operation: F) -> Result<R, CacheOperationError>
    where
        K: CacheKey + 'static,
        V: CacheValue + 'static,
        F: FnOnce(&PersistentColdTier<K, V>) -> Result<R, CacheOperationError>,
        R: bincode::Encode + bincode::Decode + 'static,
    {
        let type_key = (TypeId::of::<K>(), TypeId::of::<V>());
        let (tx, rx) = unbounded();
        
        let op = Box::new(move |tier_any: &dyn std::any::Any| -> Vec<u8> {
            if let Some(tier) = tier_any.downcast_ref::<PersistentColdTier<K, V>>() {
                match operation(tier) {
                    Ok(result) => encode_to_vec(&result, config::standard()).unwrap_or_default(),
                    Err(_) => Vec::new(),
                }
            } else {
                Vec::new()
            }
        });
        
        self.sender.send(ColdTierMessage::ExecuteReadOp {
            type_key,
            op,
            response: tx,
        }).map_err(|_| CacheOperationError::InternalError("Service unavailable".to_string()))?;
        
        let serialized = rx.recv().map_err(|_| CacheOperationError::TimeoutError)??;
        
        decode_from_slice(&serialized, config::standard())
            .map(|(decoded, _len)| decoded)
            .map_err(|_| CacheOperationError::InternalError("Deserialization failed".to_string()))
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
) -> Result<Option<V>, CacheOperationError> {
    let coordinator = ColdTierCoordinator::get()?;
    coordinator.execute_read_operation::<K, V, Option<V>, _>(|tier| Ok(tier.get(key)))
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
    value: V,
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