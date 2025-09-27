//! Global interface functions and static management
//!
//! This module provides the global API for the unified cache manager,
//! including static initialization and thread-safe access patterns.

// Removed unused imports - file contains only documentation

/// Global unified cache manager instance with atomic initialization
/// This is now generic and must be initialized with specific key/value types
#[allow(dead_code)] // Cache coordination - GLOBAL_CACHE_MANAGER used in global cache management (deprecated in favor of explicit generic types)
static GLOBAL_CACHE_MANAGER: std::sync::OnceLock<Box<dyn std::any::Any + Send + Sync>> =
    std::sync::OnceLock::new();

// REMOVED: String hardcoded init_cache_manager() and init_cache_manager_generic()
// These functions forced String, Arc<Vec<u8>> types and hid the generic architecture
// Users must now initialize with explicit generic types:
// - Create UnifiedCacheManager::<K, V>::new(config) directly with their chosen types
// - No global manager - use explicit manager instances for type safety

// REMOVED: All global cache functions (cache_clear, cache_stats, is_cache_initialized, with_cache_manager)
// These functions depended on broken global manager with hardcoded String types
// Users must now manage UnifiedCacheManager::<K, V> instances directly:
// - manager.clear() instead of cache_clear()
// - manager.stats() instead of cache_stats()
// - Check manager existence instead of is_cache_initialized()
// - Use manager directly instead of with_cache_manager()
// This eliminates global state and enforces proper generic type usage
