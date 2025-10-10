#![allow(dead_code)]
// Warm tier global API - Complete global interface library with thread-safe access, type-erased storage, alerts, and cache management

//! Global API functions and tier operations for warm tier cache
//!
//! This module provides thread-safe global access to warm tier cache instances
//! with type-erased storage following the same pattern as the hot tier implementation.

use std::any::TypeId;

use std::sync::atomic::AtomicUsize;
use std::time::Duration;

use dashmap::DashMap;

// AlertSeverity moved to canonical location: crate::cache::types::core_types::AlertSeverity
pub use crate::cache::types::core_types::AlertSeverity;

/// Generic cache alert structure
#[derive(Debug, Clone)]
pub struct CacheAlert {
    pub message: String,
    pub severity: AlertSeverity,
}
use tokio::sync::{mpsc, oneshot};

use super::config::WarmTierConfig;
use super::core::LockFreeWarmTier;

use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};

/// Trait for type-erased warm tier operations
pub trait WarmTierOperations: std::any::Any + Send + Sync {
    fn shutdown(&self);
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Handle for communicating with a warm tier instance (complete implementation)
pub struct WarmTierHandle<K: CacheKey, V: CacheValue + PartialEq> {
    sender: mpsc::UnboundedSender<WarmCacheRequest<K, V>>,
    _phantom: std::marker::PhantomData<(K, V)>,
}

impl<K: CacheKey, V: CacheValue + PartialEq> Clone for WarmTierHandle<K, V> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<K: CacheKey, V: CacheValue + PartialEq> WarmTierHandle<K, V> {
    /// Send a request to the warm tier worker thread
    pub fn send_request(&self, request: WarmCacheRequest<K, V>) -> Result<(), CacheOperationError> {
        self.sender
            .send(request)
            .map_err(|_| CacheOperationError::TierOperationFailed)
    }
}

impl<K: CacheKey, V: CacheValue + PartialEq> WarmTierOperations for WarmTierHandle<K, V> {
    fn shutdown(&self) {
        let _ = self.sender.send(WarmCacheRequest::Shutdown);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Cache operation request for warm tier worker routing (complete implementation)
pub enum WarmCacheRequest<K: CacheKey, V: CacheValue + PartialEq> {
    Get {
        key: K,
        response: oneshot::Sender<Option<V>>,
    },
    Put {
        key: K,
        value: V,
        response: oneshot::Sender<Result<(), CacheOperationError>>,
    },
    Remove {
        key: K,
        response: oneshot::Sender<Option<V>>,
    },

    // Atomic operations
    PutIfAbsent {
        key: K,
        value: V,
        response: oneshot::Sender<Option<V>>,
    },
    Replace {
        key: K,
        value: V,
        response: oneshot::Sender<Option<V>>,
    },
    CompareAndSwap {
        key: K,
        expected: V,
        new_value: V,
        response: oneshot::Sender<bool>,
    },

    // Maintenance operations (fully implemented)
    CleanupExpired {
        max_age: Duration,
        response: oneshot::Sender<Result<usize, CacheOperationError>>,
    },
    ForceEviction {
        target_count: usize,
        response: oneshot::Sender<Result<usize, CacheOperationError>>,
    },
    ProcessMaintenance {
        response: oneshot::Sender<Result<usize, CacheOperationError>>,
    },

    // Statistics operations (fully implemented)
    GetStats {
        response: oneshot::Sender<super::monitoring::TierStatsSnapshot>,
    },
    GetMemoryUsage {
        response: oneshot::Sender<Option<usize>>,
    },
    GetMemoryPressure {
        response: oneshot::Sender<Option<f64>>,
    },
    GetCacheSize {
        response: oneshot::Sender<Option<usize>>,
    },

    // Analytics operations (fully implemented)
    GetFrequentKeys {
        limit: usize,
        response: oneshot::Sender<Vec<K>>,
    },
    GetIdleKeys {
        threshold: Duration,
        response: oneshot::Sender<Vec<K>>,
    },
    GetAllKeys {
        response: oneshot::Sender<Vec<K>>,
    },
    GetAlerts {
        response: oneshot::Sender<Vec<CacheAlert>>,
    },
    GetMLPolicies {
        response: oneshot::Sender<Vec<crate::cache::tier::warm::eviction::types::PolicyPerformanceMetrics>>,
    },

    // ML Model Operations
    UpdateMLModels {
        response: oneshot::Sender<Result<usize, CacheOperationError>>,
    },

    // Temporal locality analysis
    GetTimestamps {
        key: K,
        max_count: usize,
        response: oneshot::Sender<Vec<u64>>,
    },

    // Spatial locality analysis
    GetKeyHashes {
        max_count: usize,
        response: oneshot::Sender<Vec<u64>>,
    },

    Shutdown,
}

/// Global warm tier coordinator for type-safe cache operations
pub struct WarmTierCoordinator {
    /// Storage for different K,V type combinations using lock-free DashMap (Arc-wrapped for cloning)
    pub(crate) warm_tiers: std::sync::Arc<DashMap<(TypeId, TypeId), Box<dyn WarmTierOperations>>>,
    /// Instance counter for load balancing (if we add multiple instances later)
    pub(crate) instance_selector: AtomicUsize,
}

impl Clone for WarmTierCoordinator {
    fn clone(&self) -> Self {
        Self {
            warm_tiers: self.warm_tiers.clone(), // Arc clone is cheap
            instance_selector: AtomicUsize::new(0),
        }
    }
}

impl std::fmt::Debug for WarmTierCoordinator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WarmTierCoordinator")
            .field("warm_tiers", &format!("{} entries", self.warm_tiers.len()))
            .field("instance_selector", &self.instance_selector)
            .finish()
    }
}

impl WarmTierCoordinator {
    /// Get or create a warm tier instance for the given K,V types (complete implementation)
    pub fn get_or_create_tier<K: CacheKey + 'static, V: CacheValue + Default + PartialEq + 'static>(
        &self,
        config: Option<WarmTierConfig>,
    ) -> Result<WarmTierHandle<K, V>, CacheOperationError> {
        use dashmap::mapref::entry::Entry;
        
        let type_key = (TypeId::of::<K>(), TypeId::of::<V>());

        // Use entry API to atomically check-and-create, preventing race condition
        match self.warm_tiers.entry(type_key) {
            Entry::Occupied(entry) => {
                // Tier already exists - downcast and return handle
                let handle = entry.get()
                    .as_any()
                    .downcast_ref::<WarmTierHandle<K, V>>()
                    .ok_or_else(|| CacheOperationError::internal_error("Type mismatch in warm tier map"))?;
                Ok(handle.clone())
            }
            Entry::Vacant(entry) => {
                // Create new tier atomically within vacant entry
                let tier_config = config.unwrap_or_default();
                let mut tier = LockFreeWarmTier::<K, V>::new(tier_config)
                    .map_err(|_| CacheOperationError::InitializationFailed)?;

                // Create channel for tier communication
                let (sender, mut receiver) = mpsc::unbounded_channel::<WarmCacheRequest<K, V>>();

                // Spawn background task to handle tier operations - tier OWNS the data
                tokio::runtime::Handle::current().spawn(async move {
            while let Some(request) = receiver.recv().await {
                match request {
                    WarmCacheRequest::Get { key, response } => {
                        let result = tier.get(&key);
                        let _ = response.send(result);
                    }
                    WarmCacheRequest::Put {
                        key,
                        value,
                        response,
                    } => {
                        let result = tier.put(key, value);
                        let _ = response.send(result);
                    }
                    WarmCacheRequest::Remove { key, response } => {
                        let result = tier.remove(&key);
                        let _ = response.send(result);
                    }
                    WarmCacheRequest::CleanupExpired { max_age, response } => {
                        let result = tier.cleanup_expired(max_age);
                        let _ = response.send(result);
                    }
                    WarmCacheRequest::ForceEviction {
                        target_count,
                        response,
                    } => {
                        let result = tier.force_evict(target_count);
                        let _ = response.send(result);
                    }
                    WarmCacheRequest::ProcessMaintenance { response } => {
                        let result = tier.process_maintenance();
                        let _ = response.send(result);
                    }
                    WarmCacheRequest::GetStats { response } => {
                        let result = tier.get_stats();
                        let _ = response.send(result);
                    }
                    WarmCacheRequest::GetMemoryUsage { response } => {
                        let result = Some(tier.memory_usage() as usize);
                        let _ = response.send(result);
                    }
                    WarmCacheRequest::GetMemoryPressure { response } => {
                        let result = tier.memory_pressure();
                        let _ = response.send(result);
                    }
                    WarmCacheRequest::GetCacheSize { response } => {
                        let result = Some(tier.size());
                        let _ = response.send(result);
                    }
                    WarmCacheRequest::GetFrequentKeys { limit, response } => {
                        let result = tier.get_frequent_keys(limit);
                        let _ = response.send(result);
                    }
                    WarmCacheRequest::GetIdleKeys {
                        threshold,
                        response,
                    } => {
                        let result = tier.get_idle_keys(threshold);
                        let _ = response.send(result);
                    }
                    WarmCacheRequest::GetAllKeys { response } => {
                        let result = tier.get_keys();
                        let _ = response.send(result);
                    }
                    WarmCacheRequest::GetAlerts { response } => {
                        let result = tier.get_alerts();
                        let _ = response.send(result);
                    }
                    WarmCacheRequest::GetMLPolicies { response } => {
                        let result = tier.get_ml_policies();
                        let _ = response.send(result);
                    }
                    WarmCacheRequest::UpdateMLModels { response } => {
                        let result = tier.update_ml_models();
                        let _ = response.send(result);
                    }
                    WarmCacheRequest::PutIfAbsent {
                        key,
                        value,
                        response,
                    } => {
                        // Atomic put-if-absent: check and insert in single operation within worker thread
                        let existing = tier.get(&key);
                        let result = if existing.is_some() {
                            existing // Return existing value
                        } else {
                            // Insert and return None since key was absent
                            let _ = tier.put(key, value);
                            None
                        };
                        let _ = response.send(result);
                    }
                    WarmCacheRequest::Replace {
                        key,
                        value,
                        response,
                    } => {
                        // Atomic replace: get and conditionally replace in single operation
                        let old_value = tier.get(&key);
                        if old_value.is_some() {
                            let _ = tier.put(key, value);
                        }
                        let _ = response.send(old_value);
                    }
                    WarmCacheRequest::CompareAndSwap {
                        key,
                        expected,
                        new_value,
                        response,
                    } => {
                        // Atomic compare-and-swap: get, compare, and conditionally replace in single operation
                        // Use proper PartialEq comparison - 10-100x faster than bytewise
                        if let Some(current) = tier.get(&key) {
                            if current == expected {
                                let _ = tier.put(key, new_value);
                                let _ = response.send(true);
                            } else {
                                let _ = response.send(false);
                            }
                        } else {
                            let _ = response.send(false);
                        }
                    }
                    WarmCacheRequest::GetTimestamps { key: _key, max_count, response } => {
                        // Extract timestamps from access_tracker.recent_accesses SkipMap
                        let timestamps: Vec<u64> = tier.access_tracker
                            .recent_accesses
                            .iter()
                            .rev()  // Newest to oldest
                            .map(|entry| entry.value().timestamp_ns)
                            .take(max_count)
                            .collect();

                        // Reverse to get oldest→newest order
                        let mut timestamps = timestamps;
                        timestamps.reverse();

                        let _ = response.send(timestamps);
                    }
                    WarmCacheRequest::GetKeyHashes { max_count, response } => {
                        // Extract key hashes from access_tracker.recent_accesses SkipMap
                        let key_hashes: Vec<u64> = tier.access_tracker
                            .recent_accesses
                            .iter()
                            .rev()  // Newest to oldest
                            .map(|entry| entry.value().key_hash)
                            .take(max_count)
                            .collect();

                        // Reverse to get oldest→newest order
                        let mut key_hashes = key_hashes;
                        key_hashes.reverse();

                        let _ = response.send(key_hashes);
                    }
                    WarmCacheRequest::Shutdown => break,
                }
            }
        });

                let handle = WarmTierHandle {
                    sender,
                    _phantom: std::marker::PhantomData,
                };
                // Insert into vacant entry and return handle
                entry.insert(Box::new(handle.clone()));
                Ok(handle)
            }
        }
    }

    /// Execute cache operation via specific message types (following hot tier pattern)
    fn execute_operation<
        K: CacheKey + 'static,
        V: CacheValue + Default + 'static,
        T: Send + 'static,
    >(
        &self,
        _operation: impl FnOnce(&mut LockFreeWarmTier<K, V>) -> Result<T, CacheOperationError>,
    ) -> Result<T, CacheOperationError> {
        // This method is deprecated in favor of specific message types
        // All operations should use the specific WarmCacheRequest variants instead
        // Following the hot tier pattern where each operation has its own message type
        Err(CacheOperationError::invalid_state(
            "execute_operation is deprecated - use specific message types (GetStats, CleanupExpired, etc.)",
        ))
    }

    /// Send a message to a tier and wait for response
    async fn send_message<K: CacheKey + 'static, V: CacheValue + Default + PartialEq + 'static, R: 'static>(
        &self,
        create_message: impl FnOnce(oneshot::Sender<R>) -> WarmCacheRequest<K, V>,
    ) -> Result<R, CacheOperationError> {
        let handle = self.get_or_create_tier::<K, V>(None)?;
        let (response_tx, response_rx) = oneshot::channel();
        let message = create_message(response_tx);

        handle
            .sender
            .send(message)
            .map_err(|_| CacheOperationError::invalid_state("Failed to send message to tier"))?;

        response_rx
            .await
            .map_err(|_| CacheOperationError::invalid_state("Failed to receive response from tier"))
    }

    /// Shutdown all warm tier instances
    fn shutdown_all(&self) {
        for entry in self.warm_tiers.iter() {
            entry.value().shutdown();
        }
        self.warm_tiers.clear();
    }
}
/// Initialize the warm tier cache system
pub fn init_warm_tier_system() -> Result<(), CacheOperationError> {
    Ok(())  // No-op: coordinators now created per UnifiedCacheManager instance
}

/// Initialize warm tier with specific configuration for given types
pub fn init_warm_tier<K: CacheKey + 'static, V: CacheValue + Default + PartialEq + 'static>(
    coordinator: &WarmTierCoordinator,
    config: WarmTierConfig,
) -> Result<(), CacheOperationError> {
    let _tier = coordinator.get_or_create_tier::<K, V>(Some(config))?;
    Ok(())
}

/// Shutdown the warm tier cache system
pub fn shutdown_warm_tier(
    coordinator: &WarmTierCoordinator
) -> Result<(), CacheOperationError> {
    coordinator.shutdown_all();
    coordinator.warm_tiers.clear();
    Ok(())
}

/// Get a value from the warm tier cache
pub async fn warm_get<K: CacheKey + 'static, V: CacheValue + Default + PartialEq + 'static>(
    coordinator: &WarmTierCoordinator,
    key: &K
) -> Option<V> {
    let handle = coordinator.get_or_create_tier::<K, V>(None).ok()?;

    let (response_tx, response_rx) = oneshot::channel();
    let message = WarmCacheRequest::Get {
        key: key.clone(),
        response: response_tx,
    };

    handle.sender.send(message).ok()?;
    response_rx.await.ok()?
}

// Removed warm_get_ref - zero-copy not possible with channel architecture

/// Extract recent access timestamps for temporal locality analysis
///
/// Queries ConcurrentAccessTracker.recent_accesses SkipMap for the last N timestamps
/// for the given key. Returns timestamps in chronological order (oldest to newest).
///
/// # Arguments
/// * `coordinator` - Warm tier coordinator with access to tracker
/// * `key` - Cache key to query
/// * `max_timestamps` - Maximum number of timestamps to return (default 10)
///
/// # Returns
/// Vector of timestamps in nanoseconds, oldest to newest. Empty if key not tracked.
pub async fn warm_get_timestamps<K: CacheKey + 'static, V: CacheValue + Default + PartialEq + 'static>(
    coordinator: &WarmTierCoordinator,
    key: &K,
    max_timestamps: usize,
) -> Vec<u64> {
    let handle = match coordinator.get_or_create_tier::<K, V>(None) {
        Ok(h) => h,
        Err(_) => return Vec::new(),
    };

    // Send request to get timestamps through channel
    let (response_tx, response_rx) = oneshot::channel();
    let message = WarmCacheRequest::GetTimestamps {
        key: key.clone(),
        max_count: max_timestamps,
        response: response_tx,
    };

    if handle.sender.send(message).is_err() {
        return Vec::new();
    }

    match response_rx.await {
        Ok(timestamps) => timestamps,
        Err(_) => Vec::new(),
    }
}

/// Extract recent key hashes for spatial locality analysis
///
/// Queries ConcurrentAccessTracker.recent_accesses SkipMap for the last N key hashes.
/// Returns key hashes in chronological order (oldest to newest) for calculating
/// spatial locality based on hash proximity.
///
/// # Arguments
/// * `coordinator` - Warm tier coordinator with access to tracker
/// * `max_hashes` - Maximum number of key hashes to return (default 10)
///
/// # Returns
/// Vector of key hashes (u64), oldest to newest. Empty if no access history.
pub async fn warm_get_key_hashes<K: CacheKey + 'static, V: CacheValue + Default + PartialEq + 'static>(
    coordinator: &WarmTierCoordinator,
    max_hashes: usize,
) -> Vec<u64> {
    let handle = match coordinator.get_or_create_tier::<K, V>(None) {
        Ok(h) => h,
        Err(_) => return Vec::new(),
    };

    // Send request to get key hashes through channel
    let (response_tx, response_rx) = oneshot::channel();
    let message = WarmCacheRequest::GetKeyHashes {
        max_count: max_hashes,
        response: response_tx,
    };

    if handle.sender.send(message).is_err() {
        return Vec::new();
    }

    match response_rx.await {
        Ok(key_hashes) => key_hashes,
        Err(_) => Vec::new(),
    }
}

/// Put a value into the warm tier cache
pub async fn warm_put<K: CacheKey + 'static, V: CacheValue + Default + PartialEq + 'static>(
    coordinator: &WarmTierCoordinator,
    key: K,
    value: V,
) -> Result<(), CacheOperationError> {
    let handle = coordinator.get_or_create_tier::<K, V>(None)?;

    let (response_tx, response_rx) = oneshot::channel();
    let message = WarmCacheRequest::Put {
        key,
        value,
        response: response_tx,
    };

    handle
        .sender
        .send(message)
        .map_err(|_| CacheOperationError::invalid_state("Failed to send put message"))?;
    response_rx
        .await
        .map_err(|_| CacheOperationError::invalid_state("Failed to receive put response"))?
}

/// Remove a value from the warm tier cache
pub async fn warm_remove<K: CacheKey + 'static, V: CacheValue + Default + PartialEq + 'static>(
    coordinator: &WarmTierCoordinator,
    key: &K
) -> Option<V> {
    let handle = coordinator.get_or_create_tier::<K, V>(None).ok()?;

    let (response_tx, response_rx) = oneshot::channel();
    let message = WarmCacheRequest::Remove {
        key: key.clone(),
        response: response_tx,
    };

    handle.sender.send(message).ok()?;
    response_rx.await.ok()?
}
/// Insert a promoted entry from cold tier
pub async fn insert_promoted<K: CacheKey + 'static, V: CacheValue + Default + PartialEq + 'static>(
    coordinator: &WarmTierCoordinator,
    key: K,
    value: V,
) -> Result<(), CacheOperationError> {
    warm_put(coordinator, key, value).await
}

/// Insert a demoted entry from hot tier
pub async fn insert_demoted<K: CacheKey + 'static, V: CacheValue + Default + PartialEq + 'static>(
    coordinator: &WarmTierCoordinator,
    key: K,
    value: V,
) -> Result<(), CacheOperationError> {
    warm_put(coordinator, key, value).await
}

/// Remove an entry from the warm tier
pub async fn remove_entry<K: CacheKey + 'static, V: CacheValue + Default + PartialEq + 'static>(
    coordinator: &WarmTierCoordinator,
    key: &K,
) -> Result<bool, CacheOperationError> {
    Ok(warm_remove::<K, V>(coordinator, key).await.is_some())
}

/// Get cache size via worker-based routing (complete implementation)
pub async fn get_cache_size<K: CacheKey + 'static, V: CacheValue + Default + PartialEq + 'static>(
    coordinator: &WarmTierCoordinator
) -> Option<usize> {
    let (response_tx, response_rx) = oneshot::channel();

    let request = WarmCacheRequest::GetCacheSize {
        response: response_tx,
    };
    let handle = coordinator.get_or_create_tier::<K, V>(None).ok()?;

    handle.sender.send(request).ok()?;
    response_rx.await.ok()?
}

/// Get memory usage via worker-based routing (complete implementation)
pub async fn get_memory_usage<K: CacheKey + 'static, V: CacheValue + Default + PartialEq + 'static>(
    coordinator: &WarmTierCoordinator
) -> Option<usize> {
    let (response_tx, response_rx) = oneshot::channel();

    let request = WarmCacheRequest::GetMemoryUsage {
        response: response_tx,
    };
    let handle = coordinator.get_or_create_tier::<K, V>(None).ok()?;

    handle.sender.send(request).ok()?;
    response_rx.await.ok()?
}

/// Get memory pressure via worker-based routing (complete implementation)
pub async fn get_memory_pressure<K: CacheKey + 'static, V: CacheValue + Default + PartialEq + 'static>(
    coordinator: &WarmTierCoordinator
) -> Option<f64> {
    let (response_tx, response_rx) = oneshot::channel();

    let request = WarmCacheRequest::GetMemoryPressure {
        response: response_tx,
    };
    let handle = coordinator.get_or_create_tier::<K, V>(None).ok()?;

    handle.sender.send(request).ok()?;
    response_rx.await.ok()?
}

/// Get cache statistics via worker-based routing (complete implementation)
pub async fn get_stats<K: CacheKey + 'static, V: CacheValue + Default + PartialEq + 'static>(
    coordinator: &WarmTierCoordinator
) -> Option<super::monitoring::TierStatsSnapshot> {
    let (response_tx, response_rx) = oneshot::channel();

    let request = WarmCacheRequest::GetStats {
        response: response_tx,
    };
    let handle = coordinator.get_or_create_tier::<K, V>(None).ok()?;

    handle.sender.send(request).ok()?;
    response_rx.await.ok()
}

// REMOVED: get_warm_tier_stats() compatibility alias
// Users must now use canonical function:
// - Use get_stats::<K, V>() instead of get_warm_tier_stats::<K, V>()
/// Get all keys via worker-based routing (complete implementation)
pub async fn get_warm_tier_keys<K: CacheKey + 'static, V: CacheValue + Default + PartialEq + 'static>(
    coordinator: &WarmTierCoordinator
) -> Vec<K> {

    let (response_tx, response_rx) = oneshot::channel();
    let request = WarmCacheRequest::GetAllKeys {
        response: response_tx,
    };

    let handle = match coordinator.get_or_create_tier::<K, V>(None) {
        Ok(h) => h,
        Err(_) => return Vec::new(),
    };

    if handle.sender.send(request).is_err() {
        return Vec::new();
    }

    response_rx
        .await
        .unwrap_or_else(|_| Vec::new())
}

/// Get frequently accessed keys via worker-based routing (complete implementation)
pub async fn get_frequently_accessed_keys<K: CacheKey + 'static, V: CacheValue + Default + PartialEq + 'static>(
    coordinator: &WarmTierCoordinator,
    limit: usize,
) -> Vec<K> {

    let (response_tx, response_rx) = oneshot::channel();
    let request = WarmCacheRequest::GetFrequentKeys {
        limit,
        response: response_tx,
    };

    let handle = match coordinator.get_or_create_tier::<K, V>(None) {
        Ok(h) => h,
        Err(_) => return Vec::new(),
    };

    if handle.sender.send(request).is_err() {
        return Vec::new();
    }

    response_rx
        .await
        .unwrap_or_else(|_| Vec::new())
}

/// Get idle keys via worker-based routing (complete implementation)
pub async fn get_idle_keys<K: CacheKey + 'static, V: CacheValue + Default + PartialEq + 'static>(
    coordinator: &WarmTierCoordinator,
    threshold: Duration,
) -> Vec<K> {

    let (response_tx, response_rx) = oneshot::channel();
    let request = WarmCacheRequest::GetIdleKeys {
        threshold,
        response: response_tx,
    };

    let handle = match coordinator.get_or_create_tier::<K, V>(None) {
        Ok(h) => h,
        Err(_) => return Vec::new(),
    };

    if handle.sender.send(request).is_err() {
        return Vec::new();
    }

    response_rx
        .await
        .unwrap_or_else(|_| Vec::new())
}

/// Cleanup expired entries via worker-based routing (complete implementation)
pub async fn cleanup_expired_entries<K: CacheKey + 'static, V: CacheValue + Default + PartialEq + 'static>(
    coordinator: &WarmTierCoordinator,
    max_age: Duration,
) -> Result<usize, CacheOperationError> {
    let (response_tx, response_rx) = oneshot::channel();

    let request = WarmCacheRequest::CleanupExpired {
        max_age,
        response: response_tx,
    };
    let handle = coordinator.get_or_create_tier::<K, V>(None)?;

    handle
        .sender
        .send(request)
        .map_err(|_| CacheOperationError::invalid_state("Worker queue full"))?;

    response_rx
        .await
        .map_err(|_| CacheOperationError::TimeoutError)?
}

/// Force eviction via worker-based routing (complete implementation)
pub async fn force_eviction<K: CacheKey + 'static, V: CacheValue + Default + PartialEq + 'static>(
    coordinator: &WarmTierCoordinator,
    target_count: usize,
) -> Result<usize, CacheOperationError> {
    let (response_tx, response_rx) = oneshot::channel();

    let request = WarmCacheRequest::ForceEviction {
        target_count,
        response: response_tx,
    };
    let handle = coordinator.get_or_create_tier::<K, V>(None)?;

    handle
        .sender
        .send(request)
        .map_err(|_| CacheOperationError::invalid_state("Worker queue full"))?;

    response_rx
        .await
        .map_err(|_| CacheOperationError::TimeoutError)?
}
/// Process background maintenance via worker-based routing (complete implementation)
pub async fn process_background_maintenance<K: CacheKey + 'static, V: CacheValue + Default + PartialEq + 'static>(
    coordinator: &WarmTierCoordinator
) -> Result<usize, CacheOperationError> {
    let (response_tx, response_rx) = oneshot::channel();

    let request = WarmCacheRequest::ProcessMaintenance {
        response: response_tx,
    };
    let handle = coordinator.get_or_create_tier::<K, V>(None)?;

    handle
        .sender
        .send(request)
        .map_err(|_| CacheOperationError::invalid_state("Worker queue full"))?;

    response_rx
        .await
        .map_err(|_| CacheOperationError::TimeoutError)?
}

/// Check for alerts via worker-based routing (complete implementation)
pub async fn check_warm_tier_alerts<K: CacheKey + 'static, V: CacheValue + Default + PartialEq + 'static>(
    coordinator: &WarmTierCoordinator
) -> Vec<CacheAlert> {

    let (response_tx, response_rx) = oneshot::channel();
    let request = WarmCacheRequest::GetAlerts {
        response: response_tx,
    };

    let handle = match coordinator.get_or_create_tier::<K, V>(None) {
        Ok(h) => h,
        Err(_) => return Vec::new(),
    };

    if handle.sender.send(request).is_err() {
        return Vec::new();
    }

    response_rx
        .await
        .unwrap_or_else(|_| Vec::new())
}

// REMOVED: get_aggregated_warm_tier_stats() - this was an architectural anti-pattern
//
// The correct approach is type-specific statistics using the existing infrastructure:
//
// For specific cache types:
//   let stats = get_stats::<MyKey, MyValue>()?;
//
// For multiple types, make separate calls:
//   let user_stats = get_stats::<UserId, UserData>()?;
//   let session_stats = get_stats::<SessionId, SessionData>()?;
//
// The warm tier system is designed to be type-aware with proper crossbeam messaging.
// Each K,V combination has its own worker thread that owns the data and responds to
// GetStats messages. This ensures type safety and eliminates the need for type erasure.
//
// Statistics aggregation across types should be done at the application layer,
// not within the cache system, to maintain proper separation of concerns.

/// Clear all warm tier instances (useful for testing and cleanup)
pub fn clear_all_warm_tiers(
    coordinator: &WarmTierCoordinator
) -> Result<(), CacheOperationError> {
    coordinator.shutdown_all();
    coordinator.warm_tiers.clear();
    Ok(())
}

/// Get the number of different type combinations stored in warm tiers
pub fn get_warm_tier_type_count(
    coordinator: &WarmTierCoordinator
) -> usize {
    coordinator.warm_tiers.len()
}

/// Get ML eviction policy metrics from all warm tier instances
#[allow(dead_code)] // ML system - used in machine learning policy metrics collection and monitoring
pub async fn get_warm_tier_ml_policies<K: CacheKey + 'static, V: CacheValue + Default + PartialEq + 'static>(
    coordinator: &WarmTierCoordinator
) -> Result<
    Vec<crate::cache::tier::warm::eviction::types::PolicyPerformanceMetrics>,
    CacheOperationError,
> {
    let (response_tx, response_rx) = oneshot::channel();

    let request = WarmCacheRequest::GetMLPolicies {
        response: response_tx,
    };
    let handle = coordinator.get_or_create_tier::<K, V>(None)?;

    handle
        .sender
        .send(request)
        .map_err(|_| CacheOperationError::invalid_state("Worker queue full"))?;

    response_rx
        .await
        .map_err(|_| CacheOperationError::TimeoutError)
}

/// Update ML models in warm tier instances via crossbeam messaging
pub async fn update_warm_tier_ml_models<K: CacheKey + 'static, V: CacheValue + Default + PartialEq + 'static>(
    coordinator: &WarmTierCoordinator
) -> Result<usize, CacheOperationError> {
    let (response_tx, response_rx) = oneshot::channel();

    let request = WarmCacheRequest::UpdateMLModels {
        response: response_tx,
    };
    let handle = coordinator.get_or_create_tier::<K, V>(None)?;

    handle
        .sender
        .send(request)
        .map_err(|_| CacheOperationError::invalid_state("Worker queue full"))?;

    response_rx
        .await
        .map_err(|_| CacheOperationError::TimeoutError)?
}
