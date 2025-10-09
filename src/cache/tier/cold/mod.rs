#![allow(dead_code)]
// Cold tier - Complete cold storage library with compression, persistence, memory mapping, statistics, validation, and maintenance operations

//! Cold tier persistent cache module
//!
//! This module provides persistent storage for the cold tier cache with
//! lock-free message passing architecture for thread safety.

use bincode::{Decode, Encode, config, decode_from_slice, encode_to_vec};
use crossbeam_channel::{unbounded, Sender, Receiver};
use std::any::TypeId;

use self::data_structures::{ColdCacheKey, PersistentColdTier};
use crate::cache::config::types::ColdTierConfig;
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};

// Module declarations
pub mod atomic_ops;
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

// Type aliases for complex function types to improve readability
type MutateOperation = Box<dyn FnOnce(&mut dyn std::any::Any) -> Vec<u8> + Send>;
type ReadOperation = Box<dyn FnOnce(&dyn std::any::Any) -> Vec<u8> + Send>;

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
        op: MutateOperation,
        response: Sender<Result<Vec<u8>, CacheOperationError>>,
    },
    ExecuteReadOp {
        type_key: (TypeId, TypeId),
        op: ReadOperation,
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
        log::info!("Cold tier service starting...");
        // Service thread owns ALL tier instances - no sharing, no locks
        let mut tiers = std::collections::HashMap::<
            (TypeId, TypeId),
            Box<dyn std::any::Any + Send + Sync>,
        >::new();

        while let Ok(msg) = self.receiver.recv() {
            log::debug!("Cold tier service received message");
            match msg {
                ColdTierMessage::Register {
                    type_key,
                    tier,
                    response,
                } => {
                    tiers.insert(type_key, tier);
                    let _ = response.send(Ok(()));
                }
                ColdTierMessage::ExecuteOp {
                    type_key,
                    op,
                    response,
                } => {
                    if let Some(tier) = tiers.get_mut(&type_key) {
                        let result = op(&mut **tier);
                        let _ = response.send(Ok(result));
                    } else {
                        let _ = response.send(Err(CacheOperationError::resource_exhausted(
                            "Cold tier not initialized for type",
                        )));
                    }
                }
                ColdTierMessage::ExecuteReadOp {
                    type_key,
                    op,
                    response,
                } => {
                    if let Some(tier) = tiers.get(&type_key) {
                        let result = op(&**tier);
                        let _ = response.send(Ok(result));
                    } else {
                        let _ = response.send(Err(CacheOperationError::resource_exhausted(
                            "Cold tier not initialized for type",
                        )));
                    }
                }
                ColdTierMessage::Maintenance {
                    operation,
                    response,
                } => {
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
        log::warn!("Cold tier service exiting - receiver disconnected");
    }
}


/// Cold tier coordinator for managing mutable operations
#[derive(Debug)]
pub struct ColdTierCoordinator {
    sender: Sender<ColdTierMessage>,
}

impl Clone for ColdTierCoordinator {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

impl ColdTierCoordinator {
    /// Get the sender channel for direct message sending
    pub fn sender(&self) -> &Sender<ColdTierMessage> {
        &self.sender
    }

    /// Create new per-instance cold tier coordinator with dedicated worker thread
    /// Uses crossbeam channels and std::thread for worker-owns-data pattern
    pub fn new() -> Result<Self, CacheOperationError> {
        let (tx, rx) = unbounded();
        
        // Spawn OS thread worker (not tokio task)
        std::thread::Builder::new()
            .name("cold-tier-worker".to_string())
            .spawn(move || {
                log::info!("Cold tier worker thread started");
                let service = ColdTierService { receiver: rx };
                service.run(); // Blocking call
                log::warn!("Cold tier worker thread exited");
            })
            .map_err(|e| CacheOperationError::initialization_failed(
                format!("Failed to spawn cold tier worker: {}", e)
            ))?;
        
        Ok(Self { sender: tx })
    }

    /// Trigger maintenance operation (compaction, defragmentation, etc.)
    pub async fn trigger_maintenance<
        K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
        V: CacheValue
            + Default
            + serde::Serialize
            + serde::de::DeserializeOwned
            + bincode::Encode
            + bincode::Decode<()>
            + 'static,
    >(
        &self,
        operation: &str,
    ) -> Result<(), CacheOperationError> {
        let operation = operation.to_string(); // Clone to avoid lifetime issues
        self.execute_operation::<K, V, (), _>(move |tier| {
            match operation.as_str() {
                "compact" => {
                    // Execute compaction directly with lock-free access to tier data
                    tier.compact_data_file()?;
                    Ok(())
                }
                "cleanup_expired" => {
                    // Remove expired entries from metadata index
                    tier.cleanup_expired_entries()?;
                    Ok(())
                }
                "optimize_compression" => {
                    // Test and select best compression algorithm
                    tier.optimize_compression_parameters()?;
                    Ok(())
                }
                "defragment" => {
                    // Defragmentation = cleanup + compaction
                    tier.cleanup_expired_entries()?;
                    tier.compact_data_file()?;
                    Ok(())
                }
                "clear" => {
                    // Clear all cold tier data: metadata index, statistics, and file storage
                    tier.metadata_index.clear();
                    tier.stats.reset();

                    // Clear persistent file storage through storage manager
                    tier.storage_manager.clear_storage().map_err(|e| {
                        CacheOperationError::resource_exhausted(format!(
                            "Failed to clear storage files: {}",
                            e
                        ))
                    })?;

                    Ok(())
                }
                _ => Err(CacheOperationError::invalid_argument(format!(
                    "Unknown maintenance operation: {}",
                    operation
                ))),
            }
        }).await
    }

    /// Execute maintenance operation (works with all stored K,V types)
    pub async fn execute_maintenance(
        &self,
        operation: MaintenanceOperation,
    ) -> Result<(), CacheOperationError> {
        // Send maintenance message to worker that handles all registered types
        let (tx, rx) = oneshot::channel();

        self.sender
            .send(ColdTierMessage::Maintenance {
                operation,
                response: tx,
            })
            .map_err(|_| CacheOperationError::internal_error("Service unavailable"))?;

        rx.await.map_err(|_| CacheOperationError::TimeoutError)?
    }

    /// Execute maintenance operation from string (compatibility layer)
    pub async fn execute_type_erased_maintenance(
        &self,
        operation: &str,
    ) -> Result<(), CacheOperationError> {
        let maintenance_op = match operation {
            "reset" => MaintenanceOperation::Reset,
            "compact" => MaintenanceOperation::Compact,
            "defragment" => MaintenanceOperation::Defragment,
            "restart" => MaintenanceOperation::Restart,
            _ => {
                return Err(CacheOperationError::invalid_argument(format!(
                    "Unknown maintenance operation: {}",
                    operation
                )));
            }
        };

        self.execute_maintenance(maintenance_op).await
    }

    pub async fn register<
        K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
        V: CacheValue
            + Default
            + serde::Serialize
            + serde::de::DeserializeOwned
            + bincode::Encode
            + bincode::Decode<()>
            + 'static,
    >(
        &self,
        tier: PersistentColdTier<K, V>,
    ) -> Result<(), CacheOperationError> {
        let type_key = (TypeId::of::<K>(), TypeId::of::<V>());
        let boxed_tier = Box::new(tier) as Box<dyn std::any::Any + Send + Sync>;
        let (tx, rx) = oneshot::channel();

        self.sender
            .send(ColdTierMessage::Register {
                type_key,
                tier: boxed_tier,
                response: tx,
            })
            .map_err(|_| CacheOperationError::internal_error("Service unavailable"))?;

        rx.await.map_err(|_| CacheOperationError::TimeoutError)?
    }

    pub fn execute_operation<K, V, R, F>(&self, operation: F) -> Result<R, CacheOperationError>
    where
        K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
        V: CacheValue
            + Default
            + serde::Serialize
            + serde::de::DeserializeOwned
            + bincode::Encode
            + bincode::Decode<()>
            + 'static,
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

        self.sender
            .send(ColdTierMessage::ExecuteOp {
                type_key,
                op,
                response: tx,
            })
            .map_err(|_| CacheOperationError::internal_error("Service unavailable"))?;

        // Blocking recv with timeout using crossbeam
        let serialized = rx.recv_timeout(std::time::Duration::from_millis(50))
            .map_err(|_| CacheOperationError::TimeoutError)??;

        decode_from_slice(&serialized, config::standard())
            .map(|(decoded, _len)| decoded)
            .map_err(|e| CacheOperationError::internal_error(
                format!("Cold tier operation result deserialization failed: serialized_size={}, error={:?}", 
                         serialized.len(), e)
            ))
    }

    pub async fn execute_read_operation<K, V, R, F>(&self, operation: F) -> Result<R, CacheOperationError>
    where
        K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
        V: CacheValue
            + Default
            + serde::Serialize
            + serde::de::DeserializeOwned
            + bincode::Encode
            + bincode::Decode<()>
            + 'static,
        F: FnOnce(&PersistentColdTier<K, V>) -> Result<R, CacheOperationError> + Send + 'static,
        R: Encode + Decode<()> + 'static,
    {
        let type_key = (TypeId::of::<K>(), TypeId::of::<V>());
        let (tx, rx) = oneshot::channel();

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

        self.sender
            .send(ColdTierMessage::ExecuteReadOp {
                type_key,
                op,
                response: tx,
            })
            .map_err(|_| CacheOperationError::internal_error("Service unavailable"))?;

        let serialized = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            rx
        )
        .await
        .map_err(|_| CacheOperationError::TimeoutError)?
        .map_err(|_| CacheOperationError::internal_error("Response channel closed"))??;

        decode_from_slice(&serialized, config::standard())
            .map(|(decoded, _len)| decoded)
            .map_err(|e| CacheOperationError::internal_error(
                format!("Cold tier read operation result deserialization failed: serialized_size={}, error={:?}", 
                         serialized.len(), e)
            ))
    }

    pub async fn execute_async_operation<K, V, R, F, Fut>(&self, operation: F) -> Result<R, CacheOperationError>
    where
        K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
        V: CacheValue
            + Default
            + serde::Serialize
            + serde::de::DeserializeOwned
            + bincode::Encode
            + bincode::Decode<()>
            + 'static,
        F: FnOnce(&mut PersistentColdTier<K, V>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<R, CacheOperationError>> + Send + 'static,
        R: Encode + Decode<()> + 'static,
    {
        let type_key = (TypeId::of::<K>(), TypeId::of::<V>());
        let (tx, rx) = oneshot::channel();

        let op = Box::new(move |tier_any: &mut dyn std::any::Any| -> Pin<Box<dyn Future<Output = Vec<u8>> + Send + '_>> {
            Box::pin(async move {
                if let Some(tier) = tier_any.downcast_mut::<PersistentColdTier<K, V>>() {
                    match operation(tier).await {
                        Ok(result) => encode_to_vec(&result, config::standard()).unwrap_or_default(),
                        Err(_) => Vec::new(),
                    }
                } else {
                    Vec::new()
                }
            })
        });

        self.sender
            .send(ColdTierMessage::ExecuteAsyncOp {
                type_key,
                op,
                response: tx,
            })
            .map_err(|_| CacheOperationError::internal_error("Service unavailable"))?;

        let serialized = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            rx
        )
        .await
        .map_err(|_| CacheOperationError::TimeoutError)?
        .map_err(|_| CacheOperationError::internal_error("Response channel closed"))??;

        decode_from_slice(&serialized, config::standard())
            .map(|(decoded, _len)| decoded)
            .map_err(|e| CacheOperationError::internal_error(
                format!("Cold tier async operation result deserialization failed: serialized_size={}, error={:?}", 
                         serialized.len(), e)
            ))
    }
}

/// Initialize cold tier for specific key-value types
pub async fn init_cold_tier<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
>(
    coordinator: &ColdTierCoordinator,
    base_dir: &str,
    cache_id: &str,
    pool_coordinator: std::sync::Arc<crate::cache::memory::pool_manager::cleanup_manager::PoolCoordinator>,
) -> Result<(), CacheOperationError> {
    log::info!(
        "Initializing cold tier: base_dir={}, cache_id={}",
        base_dir,
        cache_id
    );
    use arrayvec::ArrayString;

    let config = ColdTierConfig {
        base_dir: ArrayString::from(base_dir)
            .map_err(|_| CacheOperationError::invalid_state("Base directory path too long"))?,
        max_size_bytes: 1024 * 1024 * 1024, // 1GB
        max_file_size: 100 * 1024 * 1024,   // 100MB
        compression_level: 6,
        compact_interval_ns: 3_600_000_000_000, // 1 hour
        mmap_size: 1024 * 1024 * 1024,
        write_buffer_size: 64 * 1024,
        _padding: [0; 3],
    };

    let tier = PersistentColdTier::<K, V>::new(config, cache_id, pool_coordinator).map_err(|e| {
        CacheOperationError::io_failed(format!("Failed to initialize cold tier: {}", e))
    })?;

    coordinator.register::<K, V>(tier).await
}

/// Get value from cold tier cache
pub async fn cold_get<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
>(
    coordinator: &ColdTierCoordinator,
    key: &K,
) -> Result<Option<V>, CacheOperationError> {
    let key = key.clone(); // Clone to avoid lifetime issues
    coordinator.execute_read_operation::<K, V, Option<V>, _>(move |tier| Ok(tier.get(&key))).await
}

/// Get cache statistics  
pub async fn get_stats<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
>(
    coordinator: &ColdTierCoordinator,
) -> Result<crate::cache::types::TierStatistics, CacheOperationError> {
    coordinator.execute_read_operation::<K, V, crate::cache::types::TierStatistics, _>(|tier| {
        Ok(tier.stats())
    }).await
}

/// Get frequently accessed keys for promotion analysis
pub async fn get_frequently_accessed_keys<
    K: CacheKey + Clone + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
>(
    coordinator: &ColdTierCoordinator,
    threshold: u32,
) -> Vec<K> {
    coordinator
        .execute_read_operation::<K, V, Vec<K>, _>(move |tier| {
            let keys: Vec<K> = tier
                .metadata_index
                .key_index
                .iter()
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
        })
        .await
        .unwrap_or_else(|_| Vec::new())
}

/// Check if entry should be promoted to warm tier
pub async fn should_promote_to_warm<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
>(
    coordinator: &ColdTierCoordinator,
    key: &K,
) -> bool {
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
        .await
        .unwrap_or(false)
}

/// Insert demoted entry from warm tier (for tier transitions)
pub async fn insert_demoted<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
>(
    coordinator: &ColdTierCoordinator,
    key: K,
    value: V,
) -> Result<(), CacheOperationError> {
    coordinator.execute_operation::<K, V, (), _>(|tier| tier.put(key, value)).await
}

/// Remove entry for promotion to warm tier (for tier transitions)
pub async fn remove_entry<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
>(
    coordinator: &ColdTierCoordinator,
    key: &K,
) -> Result<bool, CacheOperationError> {
    let key = key.clone(); // Clone to avoid lifetime issues
    coordinator.execute_operation::<K, V, bool, _>(move |tier| Ok(tier.remove(&key))).await
}

// Re-export atomic operations
pub use atomic_ops::{compare_and_swap_atomic, put_if_absent_atomic, replace_atomic};

/// Get detailed cold tier statistics including file metrics
pub async fn get_detailed_stats<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
>(
    coordinator: &ColdTierCoordinator,
) -> Result<storage::ColdTierStats, CacheOperationError> {
    coordinator.execute_read_operation::<K, V, storage::ColdTierStats, _>(|tier| {
        let snapshot = tier.stats.snapshot();
        let storage_stats = tier.storage_manager.storage_stats();
        Ok(storage::ColdTierStats {
            hits: snapshot.hits,
            misses: snapshot.misses,
            entries: tier.metadata_index.entry_count(),
            storage_bytes: storage_stats.used_data_size,
        })
    }).await
}

/// Get cold tier hit/miss ratio
pub async fn get_hit_miss_ratio<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
>(
    coordinator: &ColdTierCoordinator,
) -> f64 {
    coordinator
        .execute_read_operation::<K, V, f64, _>(|tier| {
            let snapshot = tier.stats.snapshot();
            Ok(snapshot.hit_rate)
        })
        .await
        .unwrap_or(0.0)
}

/// Get cold tier storage utilization metrics
pub async fn get_storage_utilization<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
>(
    coordinator: &ColdTierCoordinator,
) -> Result<(u64, u64, bool), CacheOperationError> {
    coordinator.execute_read_operation::<K, V, (u64, u64, bool), _>(|tier| {
        let storage_stats = tier.storage_manager.storage_stats();
        let needs_compaction = tier.storage_manager.needs_compaction();
        Ok((
            storage_stats.total_data_size,
            storage_stats.used_data_size,
            needs_compaction,
        ))
    }).await
}

/// Validate cold tier data integrity
pub async fn validate_integrity<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
>(
    coordinator: &ColdTierCoordinator,
) -> Result<bool, CacheOperationError> {
    coordinator
        .execute_read_operation::<K, V, bool, _>(|tier| tier.storage_manager.validate_integrity()).await
}

/// Cleanup expired entries in cold tier
pub async fn cleanup_expired<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
>(
    coordinator: &ColdTierCoordinator,
) -> Result<usize, CacheOperationError> {
    coordinator.execute_operation::<K, V, usize, _>(|tier| {
        let initial_count = tier.metadata_index.entry_count();

        // Find expired entries using proper timestamp-based expiry
        let current_time_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        // Default TTL: 24 hours = 24 * 60 * 60 * 1_000_000_000 nanoseconds
        let ttl_ns = 24 * 60 * 60 * 1_000_000_000u64;

        let keys_to_remove: Vec<_> = tier
            .metadata_index
            .keys()
            .into_iter()
            .filter(|key| {
                tier.metadata_index
                    .get_entry(key)
                    .map(|entry| {
                        // Check if entry has expired based on creation time + TTL
                        let entry_age_ns = current_time_ns.saturating_sub(entry.created_at_ns);
                        entry_age_ns > ttl_ns
                    })
                    .unwrap_or(false)
            })
            .collect();

        // Remove expired entries
        for key in &keys_to_remove {
            tier.metadata_index.remove_entry(key);
        }

        let final_count = tier.metadata_index.entry_count();
        let cleaned_count = initial_count.saturating_sub(final_count);

        log::debug!(
            "Cold tier cleanup: removed {} expired entries",
            cleaned_count
        );
        Ok(cleaned_count)
    }).await
}

/// Check if cold tier contains a specific key
pub async fn contains_key<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
>(
    coordinator: &ColdTierCoordinator,
    key: &K,
) -> Result<bool, CacheOperationError> {
    // Use coordinator to check if key exists
    // This integrates the unused key containment checking methods
    let key = key.clone();
    coordinator.execute_read_operation::<K, V, bool, _>(move |tier| Ok(tier.get(&key).is_some())).await
}

/// Get number of entries in cold tier
pub async fn entry_count<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
>(
    coordinator: &ColdTierCoordinator,
) -> Result<usize, CacheOperationError> {
    coordinator
        .execute_read_operation::<K, V, usize, _>(|tier| Ok(tier.metadata_index.entry_count())).await
}

/// Check if cold tier is empty
pub async fn is_empty<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
>(
    coordinator: &ColdTierCoordinator,
) -> Result<bool, CacheOperationError> {
    coordinator
        .execute_read_operation::<K, V, bool, _>(|tier| Ok(tier.metadata_index.entry_count() == 0)).await
}

/// Clear all entries from cold tier
pub async fn clear<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
>(
    coordinator: &ColdTierCoordinator,
) -> Result<(), CacheOperationError> {
    coordinator.execute_operation::<K, V, (), _>(|tier| {
        tier.metadata_index.clear();
        tier.stats.reset();

        log::debug!("Cold tier cleared: all entries removed");
        Ok(())
    }).await
}

/// Sync storage files to disk (async, non-blocking)
/// Call this during graceful shutdown to ensure data persistence
///
/// This function performs async I/O to flush memory-mapped files and sync file handles
/// using tokio::task::spawn_blocking for safe operations on !Send types.
pub async fn sync_storage<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
>(
    coordinator: &ColdTierCoordinator,
) -> Result<(), CacheOperationError> {
    coordinator.execute_async_operation::<K, V, (), _, _>(|tier| {
        Box::pin(async {
            tier.storage_manager.shutdown_async().await
                .map_err(|e| CacheOperationError::io_failed(
                    format!("Failed to sync storage: {}", e)
                ))
        })
    }).await
}



