//! Global interface functions and static management
//!
//! This module provides the global API for the unified cache manager,
//! including static initialization and thread-safe access patterns.

use std::sync::Arc;

use crate::cache::config::CacheConfig;
use crate::cache::manager::UnifiedStats as ManagerUnifiedStats;
use super::unified_manager::UnifiedCacheManager;
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};

/// Global unified cache manager instance with atomic initialization
/// This is now generic and must be initialized with specific key/value types
static GLOBAL_CACHE_MANAGER: std::sync::OnceLock<Box<dyn std::any::Any + Send + Sync>> =
    std::sync::OnceLock::new();

// REMOVED: String hardcoded init_cache_manager() and init_cache_manager_generic()
// These functions forced String, Arc<Vec<u8>> types and hid the generic architecture
// Users must now initialize with explicit generic types:
// - Create UnifiedCacheManager::<K, V>::new(config) directly with their chosen types
// - No global manager - use explicit manager instances for type safety

// REMOVED: cache_clear_generic() compatibility alias
// Users must call cache_clear() directly

// REMOVED: Broken cache_get(), cache_put(), cache_remove() functions
// These functions pretended to be generic but were non-functional stubs
// Users must now use UnifiedCacheManager::<K, V> instances directly:
// - manager.cache_get(&key) instead of cache_get(&key)
// - manager.cache_put(key, value) instead of cache_put(key, value)
// - manager.cache_remove(&key) instead of cache_remove(&key)
// This forces proper type safety and eliminates global state dependencies

// REMOVED: All global cache functions (cache_clear, cache_stats, is_cache_initialized, with_cache_manager)
// These functions depended on broken global manager with hardcoded String types
// Users must now manage UnifiedCacheManager::<K, V> instances directly:
// - manager.clear() instead of cache_clear()
// - manager.stats() instead of cache_stats()
// - Check manager existence instead of is_cache_initialized()
// - Use manager directly instead of with_cache_manager()
// This eliminates global state and enforces proper generic type usage
