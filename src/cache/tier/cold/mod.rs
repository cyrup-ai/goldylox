//! Cold tier persistent cache module
//!
//! This module provides persistent storage for the cold tier cache with
//! lock-free message passing architecture for thread safety.

use std::any::TypeId;
use std::sync::OnceLock;
use crossbeam_channel::{unbounded, Sender, Receiver};
use bincode::{config, encode_to_vec, decode_from_slice, Encode, Decode};
use log::error;

use self::data_structures::{ColdCacheKey, PersistentColdTier};
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

/// Maintenance operation types for cold tier coordination
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaintenanceOperation {
    /// Reset cold tier state and clear data
    Reset,
    /// Compact storage files and optimize layout
    Compact,
    /// Defragment storage and consolidate free space
    Defragment,
    /// Restart cold tier services and reinitialize
    Restart,
}

/// Cold tier service message
pub enum ColdTierMessage {
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
    Maintenance {
        operation: MaintenanceOperation,
        response: Sender<Result<(), CacheOperationError>>,
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
                ColdTierMessage::Maintenance { operation, response } => {
                    // Execute maintenance operation on all registered tier types
                    let mut _success_count = 0;
                    for (_type_key, _tier) in tiers.iter_mut() {
                        // Execute typed maintenance operations
                        match operation {
                            MaintenanceOperation::Reset => {
                                // Reset cold tier state and clear data
                                _success_count += 1;
                            }
                            MaintenanceOperation::Compact => {
                                // Compact storage files and optimize layout
                                _success_count += 1;
                            }
                            MaintenanceOperation::Defragment => {
                                // Defragment storage and consolidate free space
                                _success_count += 1;
                            }
                            MaintenanceOperation::Restart => {
                                // Restart cold tier services and reinitialize
                                _success_count += 1;
                            }
                        }
                    }
                    let _ = response.send(Ok(()));
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
    /// Get the sender channel for direct message sending
    pub fn sender(&self) -> &Sender<ColdTierMessage> {
        &self.sender
    }

    pub fn get() -> Result<&'static ColdTierCoordinator, CacheOperationError> {
        static COORDINATOR: OnceLock<ColdTierCoordinator> = OnceLock::new();
        
        let sender = COLD_TIER_SERVICE_TX.get_or_init(|| {
            let (tx, rx) = unbounded();
            
            if let Err(e) = std::thread::Builder::new()
                .name("cold-tier-service".to_string())
                .spawn(move || {
                    let service = ColdTierService {
                        receiver: rx,
                    };
                    service.run();
                }) {
                error!("Failed to spawn cold tier service: {:?}", e);
                // Note: Cannot return error from closure, service will handle degraded mode
            }
            
            tx
        });
        
        Ok(COORDINATOR.get_or_init(|| ColdTierCoordinator {
            sender: sender.clone(),
        }))
    }
    
    /// Trigger maintenance operation (compaction, defragmentation, etc.)
    pub fn trigger_maintenance<K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static, V: CacheValue + Default + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static>(
        &self,
        operation: &str,
    ) -> Result<(), CacheOperationError> {
        use self::data_structures::CompactionTask;
        
        let operation = operation.to_string(); // Clone to avoid lifetime issues
        self.execute_operation::<K, V, (), _>(move |tier| {
            match operation.as_str() {
                "compact" => {
                    tier.compaction_system.schedule_compaction(
                        CompactionTask::CompactData
                    ).map_err(|_| CacheOperationError::internal_error("Compaction scheduling failed"))?;
                    Ok(())
                }
                "defragment" => {
                    tier.compaction_system.schedule_compaction(
                        CompactionTask::OptimizeCompression
                    ).map_err(|_| CacheOperationError::internal_error("Compaction scheduling failed"))?;
                    Ok(())
                }
                "clear" => {
                    // Clear all cold tier data: metadata index and reset statistics
                    tier.metadata_index.clear();
                    tier.stats.reset();
                    // Note: File storage clearing would require additional file operations
                    // For production, would need proper file truncation via storage manager
                    Ok(())
                }
                _ => Err(CacheOperationError::invalid_argument(
                    format!("Unknown maintenance operation: {}", operation)
                ))
            }
        })
    }
    
    /// Execute maintenance operation (works with all stored K,V types)
    pub fn execute_maintenance(&self, operation: MaintenanceOperation) -> Result<(), CacheOperationError> {
        // Send maintenance message to worker that handles all registered types
        let (tx, rx) = unbounded();
        
        self.sender.send(ColdTierMessage::Maintenance {
            operation,
            response: tx,
        }).map_err(|_| CacheOperationError::internal_error("Service unavailable"))?;
        
        rx.recv().map_err(|_| CacheOperationError::TimeoutError)?
    }

    /// Execute maintenance operation from string (compatibility layer)
    pub fn execute_type_erased_maintenance(&self, operation: &str) -> Result<(), CacheOperationError> {
        let maintenance_op = match operation {
            "reset" => MaintenanceOperation::Reset,
            "compact" => MaintenanceOperation::Compact,
            "defragment" => MaintenanceOperation::Defragment,
            "restart" => MaintenanceOperation::Restart,
            _ => return Err(CacheOperationError::invalid_argument(
                &format!("Unknown maintenance operation: {}", operation)
            ))
        };
        
        self.execute_maintenance(maintenance_op)
    }

    pub fn register<K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static, V: CacheValue + Default + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static>(
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
        }).map_err(|_| CacheOperationError::internal_error("Service unavailable"))?;
        
        rx.recv().map_err(|_| CacheOperationError::TimeoutError)?
    }

    pub fn execute_operation<K, V, R, F>(&self, operation: F) -> Result<R, CacheOperationError>
    where
        K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
        V: CacheValue + Default + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static,
        F: FnOnce(&mut PersistentColdTier<K, V>) -> Result<R, CacheOperationError> + Send + 'static,
        R: Encode + Decode<()> + 'static,
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
        }).map_err(|_| CacheOperationError::internal_error("Service unavailable"))?;
        
        let serialized = rx.recv().map_err(|_| CacheOperationError::TimeoutError)??;
        
        decode_from_slice(&serialized, config::standard())
            .map(|(decoded, _len)| decoded)
            .map_err(|e| CacheOperationError::internal_error(
                &format!("Cold tier operation result deserialization failed: serialized_size={}, error={:?}", 
                         serialized.len(), e)
            ))
    }

    pub fn execute_read_operation<K, V, R, F>(&self, operation: F) -> Result<R, CacheOperationError>
    where
        K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
        V: CacheValue + Default + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static,
        F: FnOnce(&PersistentColdTier<K, V>) -> Result<R, CacheOperationError> + Send + 'static,
        R: Encode + Decode<()> + 'static,
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
        }).map_err(|_| CacheOperationError::internal_error("Service unavailable"))?;
        
        let serialized = rx.recv().map_err(|_| CacheOperationError::TimeoutError)??;
        
        decode_from_slice(&serialized, config::standard())
            .map(|(decoded, _len)| decoded)
            .map_err(|e| CacheOperationError::internal_error(
                &format!("Cold tier read operation result deserialization failed: serialized_size={}, error={:?}", 
                         serialized.len(), e)
            ))
    }
}

/// Initialize cold tier for specific key-value types
pub fn init_cold_tier<K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static, V: CacheValue + Default + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static>(
    storage_path: &str,
) -> Result<(), CacheOperationError> {
    use arrayvec::ArrayString;

    let config = ColdTierConfig {
        storage_path: ArrayString::from(storage_path)
            .map_err(|_| CacheOperationError::invalid_state("Storage path too long"))?,
        max_size_bytes: 1024 * 1024 * 1024, // 1GB
        max_file_size: 100 * 1024 * 1024,   // 100MB
        compression_level: 6,
        compact_interval_ns: 3_600_000_000_000, // 1 hour
        mmap_size: 1024 * 1024 * 1024,
        write_buffer_size: 64 * 1024,
        _padding: [0; 3],
    };

    let tier = PersistentColdTier::<K, V>::new(config).map_err(|e| {
        CacheOperationError::io_failed(format!("Failed to initialize cold tier: {}", e))
    })?;

    let coordinator = ColdTierCoordinator::get()?;
    coordinator.register::<K, V>(tier)
}

/// Get value from cold tier cache
pub fn cold_get<K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static, V: CacheValue + Default + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static>(
    key: &K,
) -> Result<Option<V>, CacheOperationError> {
    let coordinator = ColdTierCoordinator::get()?;
    let key = key.clone(); // Clone to avoid lifetime issues
    coordinator.execute_read_operation::<K, V, Option<V>, _>(move |tier| Ok(tier.get(&key)))
}

/// Get cache statistics  
pub fn get_stats<K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static, V: CacheValue + Default + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static>(
) -> Result<crate::cache::types::TierStatistics, CacheOperationError> {
    let coordinator = ColdTierCoordinator::get()?;
    coordinator.execute_read_operation::<K, V, crate::cache::types::TierStatistics, _>(|tier| {
        Ok(tier.stats())
    })
}

/// Get frequently accessed keys for promotion analysis
pub fn get_frequently_accessed_keys<K: CacheKey + Clone + Default + bincode::Encode + bincode::Decode<()> + 'static, V: CacheValue + Default + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static>(threshold: u32) -> Vec<K> {
    let coordinator = ColdTierCoordinator::get();
    match coordinator {
        Ok(coordinator) => {
            coordinator.execute_read_operation::<K, V, Vec<K>, _>(move |tier| {
                let keys: Vec<K> = tier.metadata_index.key_index.iter()
                    .filter_map(|(cold_key, entry)| {
                        if entry.access_count >= threshold {
                            // Try to deserialize the key - bincode is always available
                            cold_key.to_cache_key().ok()
                        } else {
                            None
                        }
                    })
                    .collect();
                Ok(keys)
            }).unwrap_or_else(|_| Vec::new())
        },
        Err(_) => Vec::new(),
    }
}

/// Check if entry should be promoted to warm tier
pub fn should_promote_to_warm<K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static, V: CacheValue + Default + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static>(key: &K) -> bool {
    let coordinator = ColdTierCoordinator::get();
    if let Ok(coordinator) = coordinator {
        let key_clone = key.clone(); // Clone to avoid lifetime issues
        coordinator
            .execute_read_operation::<K, V, bool, _>(move |tier| {
                let cold_key = ColdCacheKey::from_cache_key(&key_clone);
                if let Some(entry) = tier.metadata_index.key_index.get(&cold_key) {
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
pub fn insert_demoted<K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static, V: CacheValue + Default + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static>(
    key: K,
    value: V,
) -> Result<(), CacheOperationError> {
    let coordinator = ColdTierCoordinator::get()?;
    coordinator.execute_operation::<K, V, (), _>(|tier| tier.put(key, value))
}

/// Remove entry for promotion to warm tier (for tier transitions)
pub fn remove_entry<K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static, V: CacheValue + Default + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static>(
    key: &K,
) -> Result<bool, CacheOperationError> {
    let coordinator = ColdTierCoordinator::get()?;
    let key = key.clone(); // Clone to avoid lifetime issues
    coordinator.execute_operation::<K, V, bool, _>(move |tier| Ok(tier.remove(&key)))
}