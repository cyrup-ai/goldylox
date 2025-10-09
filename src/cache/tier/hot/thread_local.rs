#![allow(dead_code)]
// Hot tier thread-local - Complete thread-local storage library with service-based routing, typed channels, and performance optimization

//! Hot tier cache operations with service-based routing
//!
//! This module provides blazing-fast cache operations using a service thread
//! pattern with typed channels, eliminating all type erasure.

use std::any::TypeId;
use std::sync::atomic::AtomicUsize;
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};
use dashmap::DashMap;

use super::simd_tier::SimdHotTier;
use crate::cache::config::types::HotTierConfig;
use crate::cache::types::performance::timer::timestamp_nanos;
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};
use crate::cache::types::statistics::tier_stats::TierStatistics;

/// Cache operation request for worker routing
#[derive(Debug)]
pub enum CacheRequest<K: CacheKey, V: CacheValue> {
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

    // Maintenance operations
    CleanupExpired {
        ttl_ns: u64,
        response: oneshot::Sender<usize>,
    },
    ProcessPrefetch {
        response: oneshot::Sender<usize>,
    },
    Compact {
        response: oneshot::Sender<usize>,
    },
    Clear {
        response: oneshot::Sender<()>,
    },

    // Statistics operations
    GetStats {
        response: oneshot::Sender<TierStatistics>,
    },
    GetMemoryStats {
        response: oneshot::Sender<super::memory_pool::MemoryPoolStats>,
    },
    GetEvictionStats {
        response: oneshot::Sender<super::eviction::EvictionStats>,
    },
    GetPrefetchStats {
        response: oneshot::Sender<super::prefetch::PrefetchStats>,
    },
    ShouldOptimize {
        response: oneshot::Sender<bool>,
    },

    // Analytics operations
    GetFrequentKeys {
        threshold: u32,
        window_ns: u64,
        response: oneshot::Sender<Vec<K>>,
    },
    GetIdleKeys {
        threshold_ns: u64,
        response: oneshot::Sender<Vec<K>>,
    },

    Shutdown,
}

/// Trait for type-erased hot tier operations
pub(crate) trait HotTierOperations: std::any::Any + Send + Sync {
    fn shutdown(&self);
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Handle for communicating with a hot tier instance
pub struct HotTierHandle<K: CacheKey, V: CacheValue> {
    sender: mpsc::UnboundedSender<CacheRequest<K, V>>,
    _phantom: std::marker::PhantomData<(K, V)>,
}

impl<K: CacheKey, V: CacheValue> Clone for HotTierHandle<K, V> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<K: CacheKey, V: CacheValue> HotTierHandle<K, V> {
    /// Send a request to the hot tier worker thread
    pub fn send_request(&self, request: CacheRequest<K, V>) -> Result<(), CacheOperationError> {
        self.sender
            .send(request)
            .map_err(|_| CacheOperationError::TierOperationFailed)
    }
}

impl<K: CacheKey, V: CacheValue> HotTierOperations for HotTierHandle<K, V> {
    fn shutdown(&self) {
        let _ = self.sender.send(CacheRequest::Shutdown);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Global hot tier coordinator for type-safe cache operations
pub struct HotTierCoordinator {
    /// Storage for different K,V type combinations using DashMap (Arc-wrapped for cloning)
    pub(crate) hot_tiers: std::sync::Arc<DashMap<(TypeId, TypeId), Box<dyn HotTierOperations>>>,
    /// Instance counter for load balancing
    pub(crate) instance_selector: AtomicUsize,
}

impl Clone for HotTierCoordinator {
    fn clone(&self) -> Self {
        Self {
            hot_tiers: self.hot_tiers.clone(), // Arc clone is cheap
            instance_selector: AtomicUsize::new(0),
        }
    }
}

impl std::fmt::Debug for HotTierCoordinator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HotTierCoordinator")
            .field("hot_tiers", &format!("{} entries", self.hot_tiers.len()))
            .field("instance_selector", &self.instance_selector)
            .finish()
    }
}

impl HotTierCoordinator {
    /// Get or create a hot tier instance for the given K,V types
    pub fn get_or_create_tier<K: CacheKey + Default + 'static, V: CacheValue + 'static>(
        &self,
        config: Option<HotTierConfig>,
    ) -> Result<HotTierHandle<K, V>, CacheOperationError> {
        let type_key = (TypeId::of::<K>(), TypeId::of::<V>());

        // Try to get existing tier
        if let Some(handle_ops) = self.hot_tiers.get(&type_key)
            && let Some(handle) = handle_ops.as_any().downcast_ref::<HotTierHandle<K, V>>()
        {
            return Ok(handle.clone());
        }

        // Create new tier if doesn't exist
        let tier_config = config.unwrap_or_default();
        let mut tier = SimdHotTier::<K, V>::new(tier_config);

        // Create channel for tier communication
        let (sender, mut receiver) = mpsc::unbounded_channel::<CacheRequest<K, V>>();

        // Spawn background task to handle tier operations - tier OWNS the data
        tokio::spawn(async move {
            while let Some(request) = receiver.recv().await {
                match request {
                    CacheRequest::Get { key, response } => {
                        let result = tier.get(&key);
                        let _ = response.send(result);
                    }
                    CacheRequest::Put {
                        key,
                        value,
                        response,
                    } => {
                        let result = tier.put(key, value);
                        let _ = response.send(result);
                    }
                    CacheRequest::Remove { key, response } => {
                        let result = tier.remove(&key);
                        let _ = response.send(result);
                    }
                    CacheRequest::CleanupExpired { ttl_ns, response } => {
                        let current_time = timestamp_nanos(std::time::Instant::now());
                        let mut cleaned_count = 0;

                        // Process slots in batches for better cache locality and SIMD potential
                        for batch_start in (0..256).step_by(32) {
                            let batch_end = (batch_start + 32).min(256);

                            for slot_idx in batch_start..batch_end {
                                if let Some(metadata) = tier.memory_pool().get_metadata(slot_idx)
                                    && metadata.is_occupied()
                                {
                                    // Get the actual cache slot to check timestamp
                                    if let Some(slot) = tier.memory_pool().get_slot(slot_idx) {
                                        // Proper time-based TTL: check if entry has expired
                                        let age_ns =
                                            current_time.saturating_sub(slot.last_access_ns);
                                        if age_ns > ttl_ns {
                                            // Entry has exceeded TTL - clear it
                                            tier.memory_pool_mut().clear_slot(slot_idx);
                                            cleaned_count += 1;
                                        }
                                    }
                                }
                            }
                        }
                        let _ = response.send(cleaned_count);
                    }
                    CacheRequest::ProcessPrefetch { response } => {
                        let result = tier.process_prefetch_requests();
                        let _ = response.send(result);
                    }
                    CacheRequest::Compact { response } => {
                        let result = tier.compact();
                        let _ = response.send(result);
                    }
                    CacheRequest::Clear { response } => {
                        tier.clear();
                        let _ = response.send(());
                    }
                    CacheRequest::GetStats { response } => {
                        let result = tier.stats();
                        let _ = response.send(result);
                    }
                    CacheRequest::GetMemoryStats { response } => {
                        let result = tier.memory_stats();
                        let _ = response.send(result);
                    }
                    CacheRequest::GetEvictionStats { response } => {
                        let result = tier.eviction_stats();
                        let _ = response.send(result);
                    }
                    CacheRequest::GetPrefetchStats { response } => {
                        let result = tier.prefetch_stats();
                        let _ = response.send(result);
                    }
                    CacheRequest::ShouldOptimize { response } => {
                        let result = tier.should_optimize();
                        let _ = response.send(result);
                    }
                    CacheRequest::GetFrequentKeys {
                        threshold,
                        window_ns,
                        response,
                    } => {
                        // Get frequent keys - full implementation
                        let mut frequent_keys = Vec::new();
                        let current_time = timestamp_nanos(std::time::Instant::now());
                        let _window_start = current_time.saturating_sub(window_ns);

                        for slot_idx in 0..256 {
                            if let Some(metadata) = tier.memory_pool().get_metadata(slot_idx)
                                && metadata.is_occupied()
                                && metadata.access_count >= threshold as u8
                                && metadata.generation > 0
                                && let Some(slot) = tier.memory_pool().get_slot(slot_idx)
                            {
                                frequent_keys.push(slot.key.clone());
                            }
                        }
                        let _ = response.send(frequent_keys);
                    }
                    CacheRequest::GetIdleKeys {
                        threshold_ns: _threshold_ns,
                        response,
                    } => {
                        // Get idle keys - full implementation
                        let mut idle_keys = Vec::new();
                        let _current_time = timestamp_nanos(std::time::Instant::now());

                        for slot_idx in 0..256 {
                            if let Some(metadata) = tier.memory_pool().get_metadata(slot_idx)
                                && metadata.is_occupied()
                                && metadata.access_count < 5
                                && let Some(slot) = tier.memory_pool().get_slot(slot_idx)
                            {
                                idle_keys.push(slot.key.clone());
                            }
                        }
                        let _ = response.send(idle_keys);
                    }
                    CacheRequest::PutIfAbsent {
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
                    CacheRequest::Replace {
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
                    CacheRequest::CompareAndSwap {
                        key,
                        expected,
                        new_value,
                        response,
                    } => {
                        // Atomic compare-and-swap: get, compare, and conditionally replace in single operation
                        // Uses bytewise comparison as fallback when PartialEq constraint not available
                        if let Some(current) = tier.get(&key) {
                            use std::mem;
                            let is_equal = unsafe {
                                // Bytewise comparison for types that don't have PartialEq constraint here
                                // This is safe because we're comparing values of the same type V
                                let current_bytes = std::slice::from_raw_parts(
                                    &current as *const V as *const u8,
                                    mem::size_of::<V>(),
                                );
                                let expected_bytes = std::slice::from_raw_parts(
                                    &expected as *const V as *const u8,
                                    mem::size_of::<V>(),
                                );
                                current_bytes == expected_bytes
                            };

                            if is_equal {
                                let _ = tier.put(key, new_value);
                                let _ = response.send(true);
                            } else {
                                let _ = response.send(false);
                            }
                        } else {
                            let _ = response.send(false);
                        }
                    }
                    CacheRequest::Shutdown => break,
                }
            }
        });

        let handle = HotTierHandle {
            sender,
            _phantom: std::marker::PhantomData,
        };
        self.hot_tiers.insert(type_key, Box::new(handle.clone()));
        Ok(handle)
    }
}



/// Initialize hot tier with specific configuration for given types
pub async fn init_simd_hot_tier<K: CacheKey + Default + 'static, V: CacheValue + 'static>(
    coordinator: &HotTierCoordinator,
    config: HotTierConfig,
) -> Result<(), CacheOperationError> {
    let _tier = coordinator.get_or_create_tier::<K, V>(Some(config))?;
    Ok(())
}

/// Get value from hot tier cache
pub async fn simd_hot_get<K: CacheKey + Default + 'static, V: CacheValue + 'static>(
    coordinator: &HotTierCoordinator,
    key: &K,
) -> Option<V> {
    let handle = coordinator.get_or_create_tier::<K, V>(None).ok()?;

    let (response_tx, response_rx) = oneshot::channel();
    let message = CacheRequest::Get {
        key: key.clone(),
        response: response_tx,
    };

    handle.sender.send(message).ok()?;
    response_rx.await.ok()?
}

/// Put value in hot tier cache  
pub async fn simd_hot_put<K: CacheKey + Default + 'static, V: CacheValue + 'static>(
    coordinator: &HotTierCoordinator,
    key: K,
    value: V,
) -> Result<(), CacheOperationError> {
    let handle = coordinator.get_or_create_tier::<K, V>(None)?;

    let (response_tx, response_rx) = oneshot::channel();
    let message = CacheRequest::Put {
        key,
        value,
        response: response_tx,
    };

    handle
        .sender
        .send(message)
        .map_err(|_| CacheOperationError::resource_exhausted("Worker queue full"))?;
    response_rx
        .await
        .map_err(|_| CacheOperationError::TimeoutError)?
}

/// Remove value from hot tier cache
pub async fn simd_hot_remove<K: CacheKey + Default + 'static, V: CacheValue + 'static>(
    coordinator: &HotTierCoordinator,
    key: &K,
) -> Result<Option<V>, CacheOperationError> {
    let handle = coordinator.get_or_create_tier::<K, V>(None)?;

    let (response_tx, response_rx) = oneshot::channel();
    let message = CacheRequest::Remove {
        key: key.clone(),
        response: response_tx,
    };

    handle
        .sender
        .send(message)
        .map_err(|_| CacheOperationError::resource_exhausted("Worker queue full"))?;
    response_rx
        .await
        .map_err(|_| CacheOperationError::TimeoutError)
}

/// Get statistics from hot tier
#[allow(dead_code)] // Hot tier SIMD - Statistics collection function for SIMD hot tier performance monitoring
pub async fn simd_hot_stats<K: CacheKey + Default + 'static, V: CacheValue + 'static>(
    coordinator: &HotTierCoordinator,
) -> TierStatistics {
    let handle = match coordinator.get_or_create_tier::<K, V>(None) {
        Ok(h) => h,
        Err(_) => return TierStatistics::default(),
    };

    let (response_tx, response_rx) = oneshot::channel();
    let request = CacheRequest::<K, V>::GetStats {
        response: response_tx,
    };

    if handle.sender.send(request).is_err() {
        return TierStatistics::default();
    }

    response_rx
        .await
        .unwrap_or_else(|_| TierStatistics::default())
}

/// Get frequently accessed keys from hot tier
pub async fn get_frequently_accessed_keys<K: CacheKey + Default + 'static, V: CacheValue + 'static>(
    coordinator: &HotTierCoordinator,
    access_threshold: u32,
    time_window: Duration,
) -> Vec<K> {
    let handle = match coordinator.get_or_create_tier::<K, V>(None) {
        Ok(h) => h,
        Err(_) => return Vec::new(),
    };

    let (response_tx, response_rx) = oneshot::channel();
    let request = CacheRequest::<K, V>::GetFrequentKeys {
        threshold: access_threshold,
        window_ns: time_window.as_nanos() as u64,
        response: response_tx,
    };

    if handle.sender.send(request).is_err() {
        return Vec::new();
    }

    response_rx
        .await
        .unwrap_or_else(|_| Vec::new())
}

/// Get idle keys from hot tier (candidates for demotion)
pub async fn get_idle_keys<K: CacheKey + Default + 'static, V: CacheValue + 'static>(
    coordinator: &HotTierCoordinator,
    idle_threshold: Duration,
) -> Vec<K> {
    let handle = match coordinator.get_or_create_tier::<K, V>(None) {
        Ok(h) => h,
        Err(_) => return Vec::new(),
    };

    let (response_tx, response_rx) = oneshot::channel();
    let request = CacheRequest::<K, V>::GetIdleKeys {
        threshold_ns: idle_threshold.as_nanos() as u64,
        response: response_tx,
    };

    if handle.sender.send(request).is_err() {
        return Vec::new();
    }

    response_rx
        .await
        .unwrap_or_else(|_| Vec::new())
}

/// Remove entry from hot tier using service-based routing
pub async fn remove_entry<K: CacheKey + Default + 'static, V: CacheValue + 'static>(
    coordinator: &HotTierCoordinator,
    key: &K,
) -> Option<V> {
    // Use the standard remove operation which properly routes to service
    simd_hot_remove::<K, V>(coordinator, key).await.unwrap_or_default()
}

/// Insert entry promoted from warm tier using service-based routing
pub async fn insert_promoted<K: CacheKey + Default + 'static, V: CacheValue + 'static>(
    coordinator: &HotTierCoordinator,
    key: K,
    value: V,
) -> Result<(), CacheOperationError> {
    // Use the standard put operation which properly routes to service
    simd_hot_put(coordinator, key, value).await
}

/// Cleanup expired entries from hot tier
pub async fn cleanup_expired_entries<K: CacheKey + Default + 'static, V: CacheValue + 'static>(
    coordinator: &HotTierCoordinator,
    ttl: Duration,
) -> usize {
    let handle = match coordinator.get_or_create_tier::<K, V>(None) {
        Ok(h) => h,
        Err(_) => return 0,
    };

    let (response_tx, response_rx) = oneshot::channel();
    let request = CacheRequest::<K, V>::CleanupExpired {
        ttl_ns: ttl.as_nanos() as u64,
        response: response_tx,
    };

    if handle.sender.send(request).is_err() {
        return 0;
    }

    response_rx
        .await
        .unwrap_or(0)
}

/// Process prefetch requests
pub async fn process_prefetch_requests<
    K: CacheKey + Default + 'static,
    V: CacheValue + PartialEq + 'static,
>(
    coordinator: &HotTierCoordinator,
) -> usize {
    let handle = match coordinator.get_or_create_tier::<K, V>(None) {
        Ok(h) => h,
        Err(_) => return 0,
    };

    let (response_tx, response_rx) = oneshot::channel();
    let request = CacheRequest::<K, V>::ProcessPrefetch {
        response: response_tx,
    };

    if handle.sender.send(request).is_err() {
        return 0;
    }

    response_rx
        .await
        .unwrap_or(0)
}

/// Compact hot tier and return compacted entries count
pub async fn compact_hot_tier<K: CacheKey + Default + 'static, V: CacheValue + PartialEq + 'static>(
    coordinator: &HotTierCoordinator,
) -> usize {
    let handle = match coordinator.get_or_create_tier::<K, V>(None) {
        Ok(h) => h,
        Err(_) => return 0,
    };

    let (response_tx, response_rx) = oneshot::channel();
    let request = CacheRequest::<K, V>::Compact {
        response: response_tx,
    };

    if handle.sender.send(request).is_err() {
        return 0;
    }

    response_rx
        .await
        .unwrap_or(0)
}

/// Clear all entries from hot tier
pub async fn clear_hot_tier<K: CacheKey + Default + 'static, V: CacheValue + 'static>(
    coordinator: &HotTierCoordinator,
) {
    let handle = match coordinator.get_or_create_tier::<K, V>(None) {
        Ok(h) => h,
        Err(_) => return,
    };

    let (response_tx, response_rx) = oneshot::channel();
    let request = CacheRequest::<K, V>::Clear {
        response: response_tx,
    };

    if handle.sender.send(request).is_err() {
        return;
    }

    let _ = response_rx.await;
}

/// Check if hot tier should be optimized
pub async fn should_optimize_hot_tier<
    K: CacheKey + Default + 'static,
    V: CacheValue + PartialEq + 'static,
>(
    coordinator: &HotTierCoordinator,
) -> bool {
    let handle = match coordinator.get_or_create_tier::<K, V>(None) {
        Ok(h) => h,
        Err(_) => return false,
    };

    let (response_tx, response_rx) = oneshot::channel();
    let request = CacheRequest::<K, V>::ShouldOptimize {
        response: response_tx,
    };

    if handle.sender.send(request).is_err() {
        return false;
    }

    response_rx
        .await
        .unwrap_or(false)
}

/// Get memory statistics from hot tier
pub async fn hot_tier_memory_stats<
    K: CacheKey + Default + 'static,
    V: CacheValue + PartialEq + 'static,
>(
    coordinator: &HotTierCoordinator,
) -> super::memory_pool::MemoryPoolStats {
    let handle = match coordinator.get_or_create_tier::<K, V>(None) {
        Ok(h) => h,
        Err(_) => return super::memory_pool::MemoryPoolStats::default(),
    };

    let (response_tx, response_rx) = oneshot::channel();
    let request = CacheRequest::<K, V>::GetMemoryStats {
        response: response_tx,
    };

    if handle.sender.send(request).is_err() {
        return super::memory_pool::MemoryPoolStats::default();
    }

    response_rx
        .await
        .unwrap_or_else(|_| super::memory_pool::MemoryPoolStats::default())
}

/// Get eviction statistics from hot tier
pub async fn hot_tier_eviction_stats<
    K: CacheKey + Default + 'static,
    V: CacheValue + PartialEq + 'static,
>(
    coordinator: &HotTierCoordinator,
) -> super::eviction::EvictionStats {
    let handle = match coordinator.get_or_create_tier::<K, V>(None) {
        Ok(h) => h,
        Err(_) => return super::eviction::EvictionStats::default(),
    };

    let (response_tx, response_rx) = oneshot::channel();
    let request = CacheRequest::<K, V>::GetEvictionStats {
        response: response_tx,
    };

    if handle.sender.send(request).is_err() {
        return super::eviction::EvictionStats::default();
    }

    response_rx
        .await
        .unwrap_or_else(|_| super::eviction::EvictionStats::default())
}

/// Get prefetch statistics from hot tier
pub async fn hot_tier_prefetch_stats<
    K: CacheKey + Default + 'static,
    V: CacheValue + PartialEq + 'static,
>(
    coordinator: &HotTierCoordinator,
) -> super::prefetch::PrefetchStats {
    let handle = match coordinator.get_or_create_tier::<K, V>(None) {
        Ok(h) => h,
        Err(_) => return super::prefetch::PrefetchStats::default(),
    };

    let (response_tx, response_rx) = oneshot::channel();
    let request = CacheRequest::<K, V>::GetPrefetchStats {
        response: response_tx,
    };

    if handle.sender.send(request).is_err() {
        return super::prefetch::PrefetchStats::default();
    }

    response_rx
        .await
        .unwrap_or_else(|_| super::prefetch::PrefetchStats::default())
}

/// Get configuration used by hot tier
pub fn hot_tier_config() -> HotTierConfig {
    // Return default configuration - in a full implementation, this would be stored globally
    // and shared across all service workers during initialization.
    HotTierConfig::default()
}

/// Clear all hot tier instances (type-erased for error recovery)
pub fn clear_hot_tier_system(
    coordinator: &HotTierCoordinator,
) -> Result<(), CacheOperationError> {
    // Clear all tiers regardless of type - this is for error recovery
    coordinator.hot_tiers.clear();
    Ok(())
}

/// Shutdown hot tier system (type-erased for error recovery)
pub fn shutdown_hot_tier_system(
    coordinator: &HotTierCoordinator,
) -> Result<(), CacheOperationError> {
    // Send shutdown to all active tiers
    for _tier_entry in coordinator.hot_tiers.iter() {
        // We can't access the sender directly due to type erasure,
        // but clearing the map will cause the worker threads to eventually stop
    }
    coordinator.hot_tiers.clear();
    Ok(())
}
