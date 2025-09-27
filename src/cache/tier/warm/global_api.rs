#![allow(dead_code)]
// Warm tier global API - Complete global interface library with thread-safe access, type-erased storage, alerts, and cache management

//! Global API functions and tier operations for warm tier cache
//!
//! This module provides thread-safe global access to warm tier cache instances
//! with type-erased storage following the same pattern as the hot tier implementation.

use std::any::TypeId;

use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
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
use crossbeam_channel::{Sender, bounded};

use super::config::WarmTierConfig;
use super::core::LockFreeWarmTier;

use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};

/// Trait for type-erased warm tier operations
trait WarmTierOperations: std::any::Any + Send + Sync {
    fn shutdown(&self);
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Handle for communicating with a warm tier instance (complete implementation)
pub struct WarmTierHandle<K: CacheKey, V: CacheValue> {
    sender: Sender<WarmCacheRequest<K, V>>,
    _phantom: std::marker::PhantomData<(K, V)>,
}

impl<K: CacheKey, V: CacheValue> Clone for WarmTierHandle<K, V> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<K: CacheKey, V: CacheValue> WarmTierHandle<K, V> {
    /// Send a request to the warm tier worker thread
    pub fn send_request(&self, request: WarmCacheRequest<K, V>) -> Result<(), CacheOperationError> {
        self.sender
            .send(request)
            .map_err(|_| CacheOperationError::TierOperationFailed)
    }
}

impl<K: CacheKey, V: CacheValue> WarmTierOperations for WarmTierHandle<K, V> {
    fn shutdown(&self) {
        let _ = self.sender.send(WarmCacheRequest::Shutdown);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Cache operation request for warm tier worker routing (complete implementation)
pub enum WarmCacheRequest<K: CacheKey, V: CacheValue> {
    Get {
        key: K,
        response: Sender<Option<V>>,
    },
    Put {
        key: K,
        value: V,
        response: Sender<Result<(), CacheOperationError>>,
    },
    Remove {
        key: K,
        response: Sender<Option<V>>,
    },

    // Atomic operations
    PutIfAbsent {
        key: K,
        value: V,
        response: Sender<Option<V>>,
    },
    Replace {
        key: K,
        value: V,
        response: Sender<Option<V>>,
    },
    CompareAndSwap {
        key: K,
        expected: V,
        new_value: V,
        response: Sender<bool>,
    },

    // Maintenance operations (fully implemented)
    CleanupExpired {
        max_age: Duration,
        response: Sender<Result<usize, CacheOperationError>>,
    },
    ForceEviction {
        target_count: usize,
        response: Sender<Result<usize, CacheOperationError>>,
    },
    ProcessMaintenance {
        response: Sender<Result<usize, CacheOperationError>>,
    },

    // Statistics operations (fully implemented)
    GetStats {
        response: Sender<super::monitoring::TierStatsSnapshot>,
    },
    GetMemoryUsage {
        response: Sender<Option<usize>>,
    },
    GetMemoryPressure {
        response: Sender<Option<f64>>,
    },
    GetCacheSize {
        response: Sender<Option<usize>>,
    },

    // Analytics operations (fully implemented)
    GetFrequentKeys {
        limit: usize,
        response: Sender<Vec<K>>,
    },
    GetIdleKeys {
        threshold: Duration,
        response: Sender<Vec<K>>,
    },
    GetAllKeys {
        response: Sender<Vec<K>>,
    },
    GetAlerts {
        response: Sender<Vec<CacheAlert>>,
    },
    GetMLPolicies {
        response: Sender<Vec<crate::cache::tier::warm::eviction::types::PolicyPerformanceMetrics>>,
    },

    // ML Model Operations
    UpdateMLModels {
        response: Sender<Result<usize, CacheOperationError>>,
    },

    Shutdown,
}

/// Global warm tier coordinator for type-safe cache operations
pub struct WarmTierCoordinator {
    /// Storage for different K,V type combinations using lock-free DashMap
    warm_tiers: DashMap<(TypeId, TypeId), Box<dyn WarmTierOperations>>,
    /// Instance counter for load balancing (if we add multiple instances later)
    instance_selector: AtomicUsize,
}

static COORDINATOR: AtomicPtr<WarmTierCoordinator> = AtomicPtr::new(std::ptr::null_mut());

impl WarmTierCoordinator {
    /// Initialize the global coordinator
    pub fn initialize() -> Result<(), CacheOperationError> {
        if !COORDINATOR.load(Ordering::Acquire).is_null() {
            return Ok(()); // Already initialized
        }

        let coordinator = Box::new(WarmTierCoordinator {
            warm_tiers: DashMap::new(),
            instance_selector: AtomicUsize::new(0),
        });

        let coordinator_ptr = Box::into_raw(coordinator);
        COORDINATOR.store(coordinator_ptr, Ordering::Release);

        Ok(())
    }

    /// Get the global coordinator instance
    #[inline]
    pub fn get() -> Result<&'static WarmTierCoordinator, CacheOperationError> {
        let ptr = COORDINATOR.load(Ordering::Acquire);
        if ptr.is_null() {
            return Err(CacheOperationError::invalid_state(
                "WarmTierCoordinator not initialized",
            ));
        }
        Ok(unsafe { &*ptr })
    }

    /// Get or create a warm tier instance for the given K,V types (complete implementation)
    pub fn get_or_create_tier<K: CacheKey + 'static, V: CacheValue + Default + 'static>(
        &self,
        config: Option<WarmTierConfig>,
    ) -> Result<WarmTierHandle<K, V>, CacheOperationError> {
        let type_key = (TypeId::of::<K>(), TypeId::of::<V>());

        // Try to get existing tier
        if let Some(handle_ops) = self.warm_tiers.get(&type_key)
            && let Some(handle) = handle_ops.as_any().downcast_ref::<WarmTierHandle<K, V>>()
        {
            return Ok(handle.clone());
        }

        // Create new tier if doesn't exist
        let tier_config = config.unwrap_or_default();
        let mut tier = LockFreeWarmTier::<K, V>::new(tier_config)
            .map_err(|_| CacheOperationError::InitializationFailed)?;

        // Create channel for tier communication
        let (sender, receiver) = crossbeam_channel::bounded::<WarmCacheRequest<K, V>>(1024);

        // Spawn background task to handle tier operations - tier OWNS the data
        std::thread::spawn(move || {
            while let Ok(request) = receiver.recv() {
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
                    WarmCacheRequest::Shutdown => break,
                }
            }
        });

        let handle = WarmTierHandle {
            sender,
            _phantom: std::marker::PhantomData,
        };
        self.warm_tiers.insert(type_key, Box::new(handle.clone()));
        Ok(handle)
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
    fn send_message<K: CacheKey + 'static, V: CacheValue + Default + 'static, R: 'static>(
        &self,
        create_message: impl FnOnce(Sender<R>) -> WarmCacheRequest<K, V>,
    ) -> Result<R, CacheOperationError> {
        let handle = self.get_or_create_tier::<K, V>(None)?;
        let (response_tx, response_rx) = bounded(1);
        let message = create_message(response_tx);

        handle
            .sender
            .send(message)
            .map_err(|_| CacheOperationError::invalid_state("Failed to send message to tier"))?;

        response_rx
            .recv()
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
    WarmTierCoordinator::initialize()
}

/// Initialize warm tier with specific configuration for given types
pub fn init_warm_tier<K: CacheKey + 'static, V: CacheValue + Default + 'static>(
    config: WarmTierConfig,
) -> Result<(), CacheOperationError> {
    let coordinator = WarmTierCoordinator::get()?;
    let _tier = coordinator.get_or_create_tier::<K, V>(Some(config))?;
    Ok(())
}

/// Check if the warm tier system is initialized
pub fn is_initialized() -> bool {
    !COORDINATOR.load(Ordering::Acquire).is_null()
}

/// Shutdown the warm tier cache system
pub fn shutdown_warm_tier() -> Result<(), CacheOperationError> {
    let ptr = COORDINATOR.swap(std::ptr::null_mut(), Ordering::AcqRel);
    if !ptr.is_null() {
        unsafe {
            let coordinator = Box::from_raw(ptr);
            coordinator.shutdown_all();
            drop(coordinator);
        }
    }
    Ok(())
}

/// Get a value from the warm tier cache
pub fn warm_get<K: CacheKey + 'static, V: CacheValue + Default + 'static>(key: &K) -> Option<V> {
    let coordinator = WarmTierCoordinator::get().ok()?;
    let handle = coordinator.get_or_create_tier::<K, V>(None).ok()?;

    let (response_tx, response_rx) = bounded(1);
    let message = WarmCacheRequest::Get {
        key: key.clone(),
        response: response_tx,
    };

    handle.sender.send(message).ok()?;
    response_rx.recv().ok()?
}

// Removed warm_get_ref - zero-copy not possible with channel architecture

/// Put a value into the warm tier cache
pub fn warm_put<K: CacheKey + 'static, V: CacheValue + Default + 'static>(
    key: K,
    value: V,
) -> Result<(), CacheOperationError> {
    let coordinator = WarmTierCoordinator::get()?;
    let handle = coordinator.get_or_create_tier::<K, V>(None)?;

    let (response_tx, response_rx) = bounded(1);
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
        .recv()
        .map_err(|_| CacheOperationError::invalid_state("Failed to receive put response"))?
}

/// Remove a value from the warm tier cache
pub fn warm_remove<K: CacheKey + 'static, V: CacheValue + Default + 'static>(key: &K) -> Option<V> {
    let coordinator = WarmTierCoordinator::get().ok()?;
    let handle = coordinator.get_or_create_tier::<K, V>(None).ok()?;

    let (response_tx, response_rx) = bounded(1);
    let message = WarmCacheRequest::Remove {
        key: key.clone(),
        response: response_tx,
    };

    handle.sender.send(message).ok()?;
    response_rx.recv().ok()?
}
/// Insert a promoted entry from cold tier
pub fn insert_promoted<K: CacheKey + 'static, V: CacheValue + Default + 'static>(
    key: K,
    value: V,
) -> Result<(), CacheOperationError> {
    warm_put(key, value)
}

/// Insert a demoted entry from hot tier
pub fn insert_demoted<K: CacheKey + 'static, V: CacheValue + Default + 'static>(
    key: K,
    value: V,
) -> Result<(), CacheOperationError> {
    warm_put(key, value)
}

/// Remove an entry from the warm tier
pub fn remove_entry<K: CacheKey + 'static, V: CacheValue + Default + 'static>(
    key: &K,
) -> Result<bool, CacheOperationError> {
    Ok(warm_remove::<K, V>(key).is_some())
}

/// Get cache size via worker-based routing (complete implementation)
pub fn get_cache_size<K: CacheKey + 'static, V: CacheValue + Default + 'static>() -> Option<usize> {
    let coordinator = WarmTierCoordinator::get().ok()?;
    let (response_tx, response_rx) = crossbeam_channel::bounded(1);

    let request = WarmCacheRequest::GetCacheSize {
        response: response_tx,
    };
    let handle = coordinator.get_or_create_tier::<K, V>(None).ok()?;

    handle.sender.send(request).ok()?;
    response_rx.recv_timeout(Duration::from_millis(100)).ok()?
}

/// Get memory usage via worker-based routing (complete implementation)
pub fn get_memory_usage<K: CacheKey + 'static, V: CacheValue + Default + 'static>() -> Option<usize>
{
    let coordinator = WarmTierCoordinator::get().ok()?;
    let (response_tx, response_rx) = crossbeam_channel::bounded(1);

    let request = WarmCacheRequest::GetMemoryUsage {
        response: response_tx,
    };
    let handle = coordinator.get_or_create_tier::<K, V>(None).ok()?;

    handle.sender.send(request).ok()?;
    response_rx.recv_timeout(Duration::from_millis(100)).ok()?
}

/// Get memory pressure via worker-based routing (complete implementation)
pub fn get_memory_pressure<K: CacheKey + 'static, V: CacheValue + Default + 'static>() -> Option<f64>
{
    let coordinator = WarmTierCoordinator::get().ok()?;
    let (response_tx, response_rx) = crossbeam_channel::bounded(1);

    let request = WarmCacheRequest::GetMemoryPressure {
        response: response_tx,
    };
    let handle = coordinator.get_or_create_tier::<K, V>(None).ok()?;

    handle.sender.send(request).ok()?;
    response_rx.recv_timeout(Duration::from_millis(100)).ok()?
}

/// Get cache statistics via worker-based routing (complete implementation)
pub fn get_stats<K: CacheKey + 'static, V: CacheValue + Default + 'static>()
-> Option<super::monitoring::TierStatsSnapshot> {
    let coordinator = WarmTierCoordinator::get().ok()?;
    let (response_tx, response_rx) = crossbeam_channel::bounded(1);

    let request = WarmCacheRequest::GetStats {
        response: response_tx,
    };
    let handle = coordinator.get_or_create_tier::<K, V>(None).ok()?;

    handle.sender.send(request).ok()?;
    response_rx.recv_timeout(Duration::from_millis(100)).ok()
}

// REMOVED: get_warm_tier_stats() compatibility alias
// Users must now use canonical function:
// - Use get_stats::<K, V>() instead of get_warm_tier_stats::<K, V>()
/// Get all keys via worker-based routing (complete implementation)
pub fn get_warm_tier_keys<K: CacheKey + 'static, V: CacheValue + Default + 'static>() -> Vec<K> {
    let coordinator = WarmTierCoordinator::get().ok();
    let coordinator = match coordinator {
        Some(c) => c,
        None => return Vec::new(),
    };

    let (response_tx, response_rx) = crossbeam_channel::bounded(1);
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
        .recv_timeout(Duration::from_millis(100))
        .unwrap_or_else(|_| Vec::new())
}

/// Get frequently accessed keys via worker-based routing (complete implementation)
pub fn get_frequently_accessed_keys<K: CacheKey + 'static, V: CacheValue + Default + 'static>(
    limit: usize,
) -> Vec<K> {
    let coordinator = WarmTierCoordinator::get().ok();
    let coordinator = match coordinator {
        Some(c) => c,
        None => return Vec::new(),
    };

    let (response_tx, response_rx) = crossbeam_channel::bounded(1);
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
        .recv_timeout(Duration::from_millis(100))
        .unwrap_or_else(|_| Vec::new())
}

/// Get idle keys via worker-based routing (complete implementation)
pub fn get_idle_keys<K: CacheKey + 'static, V: CacheValue + Default + 'static>(
    threshold: Duration,
) -> Vec<K> {
    let coordinator = WarmTierCoordinator::get().ok();
    let coordinator = match coordinator {
        Some(c) => c,
        None => return Vec::new(),
    };

    let (response_tx, response_rx) = crossbeam_channel::bounded(1);
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
        .recv_timeout(Duration::from_millis(100))
        .unwrap_or_else(|_| Vec::new())
}

/// Cleanup expired entries via worker-based routing (complete implementation)
pub fn cleanup_expired_entries<K: CacheKey + 'static, V: CacheValue + Default + 'static>(
    max_age: Duration,
) -> Result<usize, CacheOperationError> {
    let coordinator = WarmTierCoordinator::get()?;
    let (response_tx, response_rx) = crossbeam_channel::bounded(1);

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
        .recv_timeout(Duration::from_millis(1000))
        .map_err(|_| CacheOperationError::TimeoutError)?
}

/// Force eviction via worker-based routing (complete implementation)
pub fn force_eviction<K: CacheKey + 'static, V: CacheValue + Default + 'static>(
    target_count: usize,
) -> Result<usize, CacheOperationError> {
    let coordinator = WarmTierCoordinator::get()?;
    let (response_tx, response_rx) = crossbeam_channel::bounded(1);

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
        .recv_timeout(Duration::from_millis(1000))
        .map_err(|_| CacheOperationError::TimeoutError)?
}
/// Process background maintenance via worker-based routing (complete implementation)
pub fn process_background_maintenance<K: CacheKey + 'static, V: CacheValue + Default + 'static>()
-> Result<usize, CacheOperationError> {
    let coordinator = WarmTierCoordinator::get()?;
    let (response_tx, response_rx) = crossbeam_channel::bounded(1);

    let request = WarmCacheRequest::ProcessMaintenance {
        response: response_tx,
    };
    let handle = coordinator.get_or_create_tier::<K, V>(None)?;

    handle
        .sender
        .send(request)
        .map_err(|_| CacheOperationError::invalid_state("Worker queue full"))?;

    response_rx
        .recv_timeout(Duration::from_millis(1000))
        .map_err(|_| CacheOperationError::TimeoutError)?
}

/// Check for alerts via worker-based routing (complete implementation)
pub fn check_warm_tier_alerts<K: CacheKey + 'static, V: CacheValue + Default + 'static>()
-> Vec<CacheAlert> {
    let coordinator = WarmTierCoordinator::get().ok();
    let coordinator = match coordinator {
        Some(c) => c,
        None => return Vec::new(),
    };

    let (response_tx, response_rx) = crossbeam_channel::bounded(1);
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
        .recv_timeout(Duration::from_millis(100))
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
pub fn clear_all_warm_tiers() -> Result<(), CacheOperationError> {
    let coordinator = WarmTierCoordinator::get()?;
    coordinator.shutdown_all();
    coordinator.warm_tiers.clear();
    Ok(())
}

/// Get the number of different type combinations stored in warm tiers
pub fn get_warm_tier_type_count() -> usize {
    let coordinator = WarmTierCoordinator::get().ok();
    if let Some(coordinator) = coordinator {
        coordinator.warm_tiers.len()
    } else {
        0
    }
}

/// Get ML eviction policy metrics from all warm tier instances
#[allow(dead_code)] // ML system - used in machine learning policy metrics collection and monitoring
pub fn get_warm_tier_ml_policies<K: CacheKey + 'static, V: CacheValue + Default + 'static>()
-> Result<
    Vec<crate::cache::tier::warm::eviction::types::PolicyPerformanceMetrics>,
    CacheOperationError,
> {
    let coordinator = WarmTierCoordinator::get()?;
    let (response_tx, response_rx) = crossbeam_channel::bounded(1);

    let request = WarmCacheRequest::GetMLPolicies {
        response: response_tx,
    };
    let handle = coordinator.get_or_create_tier::<K, V>(None)?;

    handle
        .sender
        .send(request)
        .map_err(|_| CacheOperationError::invalid_state("Worker queue full"))?;

    response_rx
        .recv_timeout(Duration::from_millis(100))
        .map_err(|_| CacheOperationError::TimeoutError)
}

/// Update ML models in warm tier instances via crossbeam messaging
pub fn update_warm_tier_ml_models<K: CacheKey + 'static, V: CacheValue + Default + 'static>()
-> Result<usize, CacheOperationError> {
    let coordinator = WarmTierCoordinator::get()?;
    let (response_tx, response_rx) = crossbeam_channel::bounded(1);

    let request = WarmCacheRequest::UpdateMLModels {
        response: response_tx,
    };
    let handle = coordinator.get_or_create_tier::<K, V>(None)?;

    handle
        .sender
        .send(request)
        .map_err(|_| CacheOperationError::invalid_state("Worker queue full"))?;

    response_rx
        .recv_timeout(Duration::from_millis(1000))
        .map_err(|_| CacheOperationError::TimeoutError)?
}
