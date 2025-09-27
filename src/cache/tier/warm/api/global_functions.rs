//! Global API functions for warm tier cache access
//!
//! This module provides the public global API functions that use a static
//! warm tier instance for cache operations across the application.

use std::sync::OnceLock;
use std::time::Duration;

use super::builder::WarmTierBuilder;
use super::core::LockFreeWarmTier;
use super::data_structures::WarmTierConfig;
use super::monitoring::{MemoryAlert, TierStatsSnapshot};
use super::maintenance::MaintenanceTask;
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};

// Global warm tier no longer uses Arc - handled by channel-based architecture in global_api.rs

/// Initialize global warm tier cache
pub fn init_warm_tier<K: CacheKey + 'static, V: CacheValue + 'static>(config: WarmTierConfig) -> Result<(), CacheOperationError> {
    super::global_api::init_warm_tier::<K, V>(config)
}

/// Get entry from warm tier cache
pub fn warm_get<K: CacheKey + 'static, V: CacheValue + 'static>(key: &K) -> Option<V> {
    super::global_api::warm_get(key)
}

/// Put entry into warm tier cache
pub fn warm_put<K: CacheKey + 'static, V: CacheValue + 'static>(key: K, value: V) -> Result<(), CacheOperationError> {
    super::global_api::warm_put(key, value)
}

/// Remove entry from warm tier cache
pub fn warm_remove<K: CacheKey + 'static, V: CacheValue + 'static>(key: &K) -> Option<V> {
    super::global_api::warm_remove(key)
}


/// Get memory pressure level
pub fn get_memory_pressure<K: CacheKey + 'static, V: CacheValue + 'static>() -> Option<f64> {
    super::global_api::get_memory_pressure::<K, V>()
}

/// Cleanup expired entries from warm tier
pub fn cleanup_expired_entries<K: CacheKey + 'static, V: CacheValue + 'static>(max_age: Duration) -> Result<usize, CacheOperationError> {
    super::global_api::cleanup_expired_entries(max_age)
}

/// Get all cache keys currently in warm tier
pub fn get_warm_tier_keys<K: CacheKey + 'static, V: CacheValue + 'static>() -> Vec<K> {
    super::global_api::get_warm_tier_keys()
}

/// Get idle cache keys (not accessed recently)
pub fn get_idle_keys<K: CacheKey + 'static, V: CacheValue + 'static>(idle_threshold: Duration) -> Vec<K> {
    super::global_api::get_idle_keys(idle_threshold)
}

/// Insert entry promoted from cold tier
pub fn insert_promoted<K: CacheKey + 'static, V: CacheValue + 'static>(
    key: K,
    value: V,
) -> Result<(), CacheOperationError> {
    super::global_api::insert_promoted(key, value)
}

/// Insert entry demoted from hot tier
pub fn insert_demoted<K: CacheKey + 'static, V: CacheValue + 'static>(
    key: K,
    value: V,
) -> Result<(), CacheOperationError> {
    super::global_api::insert_demoted(key, value)
}

/// Process background maintenance for warm tier
pub fn process_background_maintenance<K: CacheKey + 'static, V: CacheValue + 'static>() -> Result<usize, CacheOperationError> {
    super::global_api::process_background_maintenance::<K, V>()
}

/// Check for memory alerts in warm tier
pub fn check_warm_tier_alerts<K: CacheKey + 'static, V: CacheValue + 'static>() -> Vec<super::global_api::CacheAlert> {
    super::global_api::check_warm_tier_alerts::<K, V>()
}

/// Force eviction to reach target memory pressure
pub fn force_eviction<K: CacheKey + 'static, V: CacheValue + 'static>(target_count: usize) -> Result<usize, CacheOperationError> {
    super::global_api::force_eviction::<K, V>(target_count)
}

/// Get current cache size (number of entries)
pub fn get_cache_size<K: CacheKey + 'static, V: CacheValue + 'static>() -> Option<usize> {
    super::global_api::get_cache_size::<K, V>()
}

/// Get estimated memory usage in bytes
pub fn get_memory_usage<K: CacheKey + 'static, V: CacheValue + 'static>() -> Option<usize> {
    super::global_api::get_memory_usage::<K, V>()
}

/// Check if warm tier is initialized
pub fn is_initialized() -> bool {
    super::global_api::is_initialized()
}

// REMOVED: get_stats() backwards compatibility alias
// Users must call get_warm_tier_stats::<K, V>() directly with explicit type parameters

/// Get frequently accessed keys for promotion analysis
pub fn get_frequently_accessed_keys<K: CacheKey + 'static, V: CacheValue + 'static>(count: usize) -> Vec<K> {
    super::global_api::get_frequently_accessed_keys(count)
}

// REMOVED: remove_entry() backwards compatibility alias
// Users must call warm_remove::<K, V>() directly with explicit type parameters

/// Shutdown warm tier (cleanup resources)
pub fn shutdown_warm_tier() -> Result<(), CacheOperationError> {
    super::global_api::shutdown_warm_tier()
}