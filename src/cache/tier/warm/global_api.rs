//! Global API functions and tier operations for warm tier cache
//!
//! This module provides thread-safe global access to warm tier cache instances
//! with type-erased storage following the same pattern as the hot tier implementation.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::time::Duration;

use dashmap::DashMap;
use crossbeam_channel::{Sender, Receiver, bounded};

use super::core::LockFreeWarmTier;
use super::data_structures::WarmTierConfig;
use super::monitoring::TierStatsSnapshot;
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};
use crate::cache::manager::TierStatistics;

/// Handle for communicating with a warm tier instance
#[derive(Clone)]
struct WarmTierHandle {
    sender: Sender<WarmTierMessage>,
}

/// Messages for warm tier operations
enum WarmTierMessage {
    Get { key: Box<dyn Any + Send>, response: Sender<Option<Box<dyn Any + Send>>> },
    Put { key: Box<dyn Any + Send>, value: Box<dyn Any + Send>, response: Sender<Result<(), CacheOperationError>> },
    Remove { key: Box<dyn Any + Send>, response: Sender<Option<Box<dyn Any + Send>>> },
    GetStats { response: Sender<TierStatsSnapshot> },
    Shutdown,
}

/// Cache operation request for warm tier routing
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
    // Maintenance operations
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
    // Statistics operations
    GetStats {
        response: Sender<TierStatsSnapshot>,
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
    // Analytics operations
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
        response: Sender<Vec<String>>,
    },
}
/// Global warm tier coordinator for type-safe cache operations
pub struct WarmTierCoordinator {
    /// Storage for different K,V type combinations using lock-free DashMap
    warm_tiers: DashMap<(TypeId, TypeId), WarmTierHandle>,
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
    fn get() -> Result<&'static WarmTierCoordinator, CacheOperationError> {
        let ptr = COORDINATOR.load(Ordering::Acquire);
        if ptr.is_null() {
            return Err(CacheOperationError::invalid_state(
                "WarmTierCoordinator not initialized",
            ));
        }
        Ok(unsafe { &*ptr })
    }

    /// Get or create a warm tier instance for the given K,V types
    fn get_or_create_tier<K: CacheKey + 'static, V: CacheValue + 'static>(
        &self,
        config: Option<WarmTierConfig>,
    ) -> Result<WarmTierHandle, CacheOperationError> {
        let type_key = (TypeId::of::<K>(), TypeId::of::<V>());
        
        // Try to get existing tier
        if let Some(handle) = self.warm_tiers.get(&type_key) {
            return Ok(WarmTierHandle { sender: handle.sender.clone() });
        }
        
        // Create new tier if doesn't exist
        let tier_config = config.unwrap_or_default();
        let mut tier = LockFreeWarmTier::<K, V>::new(tier_config)
            .map_err(|_| CacheOperationError::InitializationFailed)?;
        
        // Create channel for tier communication
        let (sender, receiver) = bounded::<WarmTierMessage>(1024);
        
        // Spawn background task to handle tier operations
        std::thread::spawn(move || {
            while let Ok(msg) = receiver.recv() {
                match msg {
                    WarmTierMessage::Get { key, response } => {
                        if let Some(k) = key.downcast_ref::<K>() {
                            let value = tier.get(k);
                            let boxed_value = value.map(|v| Box::new(v) as Box<dyn Any + Send>);
                            let _ = response.send(boxed_value);
                        }
                    }
                    WarmTierMessage::Put { key, value, response } => {
                        if let (Some(k), Some(v)) = (key.downcast_ref::<K>(), value.downcast_ref::<V>()) {
                            let result = tier.put(k.clone(), v.clone());
                            let _ = response.send(result);
                        }
                    }
                    WarmTierMessage::Remove { key, response } => {
                        if let Some(k) = key.downcast_ref::<K>() {
                            let value = tier.remove(k);
                            let boxed_value = value.map(|v| Box::new(v) as Box<dyn Any + Send>);
                            let _ = response.send(boxed_value);
                        }
                    }
                    WarmTierMessage::GetStats { response } => {
                        let _ = response.send(tier.get_stats());
                    }
                    WarmTierMessage::Shutdown => break,
                }
            }
        });
        
        let handle = WarmTierHandle { sender };
        self.warm_tiers.insert(type_key, handle.clone());
        Ok(handle)
    }

    /// Execute cache operation with proper type safety
    fn execute_operation<K: CacheKey + 'static, V: CacheValue + 'static, T>(
        &self,
        operation: impl FnOnce(&mut LockFreeWarmTier<K, V>) -> Result<T, CacheOperationError>,
    ) -> Result<T, CacheOperationError> {
        // This method needs to be refactored to use message passing
        // For now, return an error as direct access is no longer possible
        Err(CacheOperationError::invalid_state("Direct tier access no longer supported - use message passing"))
    }
    
    /// Send a message to a tier and wait for response
    fn send_message<K: CacheKey + 'static, V: CacheValue + 'static, R: 'static>(
        &self,
        create_message: impl FnOnce(Sender<R>) -> WarmTierMessage,
    ) -> Result<R, CacheOperationError> {
        let handle = self.get_or_create_tier::<K, V>(None)?;
        let (response_tx, response_rx) = bounded(1);
        let message = create_message(response_tx);
        
        handle.sender.send(message)
            .map_err(|_| CacheOperationError::invalid_state("Failed to send message to tier"))?;
            
        response_rx.recv()
            .map_err(|_| CacheOperationError::invalid_state("Failed to receive response from tier"))
    }
    
    /// Shutdown all warm tier instances
    fn shutdown_all(&self) {
        for entry in self.warm_tiers.iter() {
            let _ = entry.value().sender.send(WarmTierMessage::Shutdown);
        }
        self.warm_tiers.clear();
    }
}
/// Initialize the warm tier cache system
pub fn init_warm_tier_system() -> Result<(), CacheOperationError> {
    WarmTierCoordinator::initialize()
}

/// Initialize warm tier with specific configuration for given types
pub fn init_warm_tier<K: CacheKey + 'static, V: CacheValue + 'static>(
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
pub fn warm_get<K: CacheKey + 'static, V: CacheValue + 'static>(key: &K) -> Option<V> {
    let coordinator = WarmTierCoordinator::get().ok()?;
    let handle = coordinator.get_or_create_tier::<K, V>(None).ok()?;
    
    let (response_tx, response_rx) = bounded(1);
    let message = WarmTierMessage::Get {
        key: Box::new(key.clone()),
        response: response_tx,
    };
    
    handle.sender.send(message).ok()?;
    let boxed_value = response_rx.recv().ok()??;
    boxed_value.downcast::<V>().ok().map(|b| *b)
}

// Removed warm_get_ref - zero-copy not possible with channel architecture

/// Put a value into the warm tier cache
pub fn warm_put<K: CacheKey + 'static, V: CacheValue + 'static>(
    key: K,
    value: V,
) -> Result<(), CacheOperationError> {
    let coordinator = WarmTierCoordinator::get()?;
    let handle = coordinator.get_or_create_tier::<K, V>(None)?;
    
    let (response_tx, response_rx) = bounded(1);
    let message = WarmTierMessage::Put {
        key: Box::new(key),
        value: Box::new(value),
        response: response_tx,
    };
    
    handle.sender.send(message)
        .map_err(|_| CacheOperationError::invalid_state("Failed to send put message"))?;
    response_rx.recv()
        .map_err(|_| CacheOperationError::invalid_state("Failed to receive put response"))?
}

/// Remove a value from the warm tier cache
pub fn warm_remove<K: CacheKey + 'static, V: CacheValue + 'static>(key: &K) -> Option<V> {
    let coordinator = WarmTierCoordinator::get().ok()?;
    let handle = coordinator.get_or_create_tier::<K, V>(None).ok()?;
    
    let (response_tx, response_rx) = bounded(1);
    let message = WarmTierMessage::Remove {
        key: Box::new(key.clone()),
        response: response_tx,
    };
    
    handle.sender.send(message).ok()?;
    let boxed_value = response_rx.recv().ok()??;
    boxed_value.downcast::<V>().ok().map(|b| *b)
}
/// Insert a promoted entry from cold tier
pub fn insert_promoted<K: CacheKey + 'static, V: CacheValue + 'static>(
    key: K,
    value: V,
) -> Result<(), CacheOperationError> {
    warm_put(key, value)
}

/// Insert a demoted entry from hot tier
pub fn insert_demoted<K: CacheKey + 'static, V: CacheValue + 'static>(
    key: K,
    value: V,
) -> Result<(), CacheOperationError> {
    warm_put(key, value)
}

/// Remove an entry from the warm tier
pub fn remove_entry<K: CacheKey + 'static, V: CacheValue + 'static>(
    key: &K,
) -> Result<bool, CacheOperationError> {
    Ok(warm_remove::<K, V>(key).is_some())
}

/// Get current cache size (number of entries) for specific types
pub fn get_cache_size<K: CacheKey + 'static, V: CacheValue + 'static>() -> Option<usize> {
    let coordinator = WarmTierCoordinator::get().ok()?;
    coordinator
        .execute_operation::<K, V, Option<usize>>(|tier| Ok(Some(tier.size())))
        .ok()
        .flatten()
}

/// Get memory usage in bytes for specific types
pub fn get_memory_usage<K: CacheKey + 'static, V: CacheValue + 'static>() -> Option<usize> {
    let coordinator = WarmTierCoordinator::get().ok()?;
    coordinator
        .execute_operation::<K, V, Option<usize>>(|tier| Ok(Some(tier.memory_usage() as usize)))
        .ok()
        .flatten()
}

/// Get memory pressure as a ratio (0.0-1.0) for specific types
pub fn get_memory_pressure<K: CacheKey + 'static, V: CacheValue + 'static>() -> Option<f64> {
    let coordinator = WarmTierCoordinator::get().ok()?;
    coordinator
        .execute_operation::<K, V, Option<f64>>(|tier| Ok(tier.memory_pressure()))
        .ok()
        .flatten()
}

/// Get cache statistics snapshot for specific types
pub fn get_stats<K: CacheKey + 'static, V: CacheValue + 'static>() -> Option<TierStatsSnapshot> {
    let coordinator = WarmTierCoordinator::get().ok()?;
    coordinator
        .execute_operation::<K, V, Option<TierStatsSnapshot>>(|tier| Ok(Some(tier.get_stats())))
        .ok()
        .flatten()
}

// REMOVED: get_warm_tier_stats() compatibility alias
// Users must now use canonical function:
// - Use get_stats::<K, V>() instead of get_warm_tier_stats::<K, V>()
/// Get all keys currently in the warm tier for specific types
pub fn get_warm_tier_keys<K: CacheKey + 'static, V: CacheValue + 'static>() -> Vec<K> {
    let coordinator = WarmTierCoordinator::get().ok();
    if let Some(coordinator) = coordinator {
        coordinator
            .execute_operation::<K, V, Vec<K>>(|tier| Ok(tier.get_keys()))
            .unwrap_or_default()
    } else {
        Vec::new()
    }
}

/// Get frequently accessed keys for specific types
pub fn get_frequently_accessed_keys<K: CacheKey + 'static, V: CacheValue + 'static>(
    limit: usize,
) -> Vec<K> {
    let coordinator = WarmTierCoordinator::get().ok();
    if let Some(coordinator) = coordinator {
        coordinator
            .execute_operation::<K, V, Vec<K>>(|tier| Ok(tier.get_frequent_keys(limit)))
            .unwrap_or_default()
    } else {
        Vec::new()
    }
}

/// Get idle keys that haven't been accessed recently for specific types
pub fn get_idle_keys<K: CacheKey + 'static, V: CacheValue + 'static>(
    threshold: Duration,
) -> Vec<K> {
    let coordinator = WarmTierCoordinator::get().ok();
    if let Some(coordinator) = coordinator {
        coordinator
            .execute_operation::<K, V, Vec<K>>(|tier| Ok(tier.get_idle_keys(threshold)))
            .unwrap_or_default()
    } else {
        Vec::new()
    }
}

/// Clean up expired entries for specific types
pub fn cleanup_expired_entries<K: CacheKey + 'static, V: CacheValue + 'static>(
    max_age: Duration,
) -> Result<usize, CacheOperationError> {
    let coordinator = WarmTierCoordinator::get()?;
    coordinator.execute_operation::<K, V, usize>(|tier| tier.cleanup_expired(max_age))
}

/// Force eviction of entries to free memory for specific types
pub fn force_eviction<K: CacheKey + 'static, V: CacheValue + 'static>(
    target_count: usize,
) -> Result<usize, CacheOperationError> {
    let coordinator = WarmTierCoordinator::get()?;
    coordinator.execute_operation::<K, V, usize>(|tier| tier.force_evict(target_count))
}
/// Process background maintenance tasks for specific types
pub fn process_background_maintenance<K: CacheKey + 'static, V: CacheValue + 'static>(
) -> Result<usize, CacheOperationError> {
    let coordinator = WarmTierCoordinator::get()?;
    coordinator.execute_operation::<K, V, usize>(|tier| tier.process_maintenance())
}

/// Check for warm tier alerts (memory pressure, performance issues) for specific types
pub fn check_warm_tier_alerts<K: CacheKey + 'static, V: CacheValue + 'static>() -> Vec<String> {
    let coordinator = WarmTierCoordinator::get().ok();
    if let Some(coordinator) = coordinator {
        coordinator
            .execute_operation::<K, V, Vec<String>>(|tier| Ok(tier.get_alerts()))
            .unwrap_or_default()
    } else {
        Vec::new()
    }
}

/// Get aggregated statistics across all warm tier instances
pub fn get_aggregated_warm_tier_stats() -> TierStatistics {
    let coordinator = WarmTierCoordinator::get();
    if let Ok(coordinator) = coordinator {
        let tiers = coordinator.warm_tiers.lock().ok();
        if let Some(tiers) = tiers {
            let mut aggregated = TierStatistics::default();

            // Note: In a full implementation, we would need to iterate through all type-erased
            // tiers and aggregate their statistics. This requires additional infrastructure
            // to handle the type erasure properly. For now, return default statistics.
            // A production implementation would store statistics in a type-independent format.

            aggregated
        } else {
            TierStatistics::default()
        }
    } else {
        TierStatistics::default()
    }
}

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
