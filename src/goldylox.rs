//! Simple public API for Goldylox cache system
//! 
//! This provides a user-friendly interface while leveraging all the sophisticated
//! internal infrastructure (multi-tier, ML eviction, SIMD, coherence protocols).
//!
//! Users specify both key and value types `Goldylox<K, V>` for full type safety
//! and direct access to all advanced cache features.

use serde::{Serialize, de::DeserializeOwned};
use std::fmt::Debug;
use std::hash::Hash;

use crate::cache::coordinator::unified_manager::UnifiedCacheManager;
use crate::cache::serde::{SerdeCacheKey, SerdeCacheValue};
use crate::cache::config::CacheConfig;
use crate::cache::config::types::generate_storage_path;
use crate::cache::traits::types_and_enums::CacheOperationError;




/// Simple, user-friendly cache interface with homogeneous key-value storage
/// 
/// Users specify both key and value types for full type safety and direct access
/// to all sophisticated cache features: ML eviction, SIMD optimization, coherence protocols.
pub struct Goldylox<K, V> 
where 
    K: Serialize + DeserializeOwned + Clone + Hash + Eq + Ord + Send + Sync + Debug + Default + 'static,
    V: Serialize + DeserializeOwned + Clone + Send + Sync + Debug + Default + 'static
{
    // Direct connection to sophisticated UnifiedCacheManager with all advanced features
    manager: UnifiedCacheManager<SerdeCacheKey<K>, SerdeCacheValue<V>>,
}

impl<K, V> Goldylox<K, V> 
where 
    K: Serialize + DeserializeOwned + Clone + Hash + Eq + Ord + Send + Sync + Debug + Default + 'static,
    V: Serialize + DeserializeOwned + Clone + Send + Sync + Debug + Default + 'static
{
    /// Create new cache builder with fluent configuration
    pub fn builder() -> GoldyloxBuilder<K, V> {
        GoldyloxBuilder::new()
    }
    
    /// Create new cache with default configuration
    pub fn new() -> Result<Self, CacheOperationError> {
        Self::builder().build()
    }
    
    /// Store value in cache with direct access to all sophisticated features
    pub fn put(&self, key: K, value: V) -> Result<(), CacheOperationError> {
        let cache_key = SerdeCacheKey(key);
        let cache_value = SerdeCacheValue(value);
        
        self.manager.put(cache_key, cache_value)
    }
    
    /// Retrieve value from cache with full type safety
    pub fn get(&self, key: &K) -> Option<V> {
        let cache_key = SerdeCacheKey(key.clone());
        self.manager.get(&cache_key).map(|cache_value| cache_value.0)
    }
    
    /// Remove value from cache
    pub fn remove(&self, key: &K) -> bool {
        let cache_key = SerdeCacheKey(key.clone());
        self.manager.remove(&cache_key)
    }
    
    /// Clear all entries from cache
    pub fn clear(&self) -> Result<(), CacheOperationError> {
        self.manager.clear()
    }
    
    /// Get cache statistics as formatted string
    pub fn stats(&self) -> Result<String, CacheOperationError> {
        let stats = self.manager.stats();
        Ok(format!(
            "{{\"total_operations\":{},\"overall_hit_rate\":{:.2},\"ops_per_second\":{:.2},\"hot_tier_hits\":{},\"warm_tier_hits\":{},\"cold_tier_hits\":{},\"total_misses\":{},\"avg_access_latency_ns\":{},\"total_memory_usage\":{}}}",
            stats.total_operations,
            stats.overall_hit_rate,
            stats.ops_per_second,
            stats.hot_tier_hits,
            stats.warm_tier_hits,
            stats.cold_tier_hits,
            stats.total_misses,
            stats.avg_access_latency_ns,
            stats.total_memory_usage
        ))
    }
    
    /// Get detailed cache analytics including access pattern analysis
    pub fn detailed_analytics(&self) -> Result<String, CacheOperationError> {
        let (unified_stats, policy_stats, analyzer_stats) = self.manager.detailed_analytics();
        Ok(format!(
            "{{\"unified\":{},\"policy_hit_rates\":{:?},\"analyzer_tracked_keys\":{},\"analyzer_cleanup_cycles\":{},\"analyzer_memory_usage\":{}}}",
            serde_json::to_string(&unified_stats).unwrap_or_default(),
            policy_stats.hit_rates,
            analyzer_stats.tracked_keys,
            analyzer_stats.total_cleanup_cycles,
            analyzer_stats.memory_usage_bytes
        ))
    }
    
    // =======================================================================
    // CONCURRENT CACHE OPERATIONS (Java ConcurrentHashMap style)
    // =======================================================================
    
    /// Store value only if key is not already present
    /// Returns the previous value if key was present, None if key was absent
    pub fn put_if_absent(&self, key: K, value: V) -> Result<Option<V>, CacheOperationError> {
        let cache_key = SerdeCacheKey(key);
        let cache_value = SerdeCacheValue(value);
        
        self.manager.put_if_absent(cache_key, cache_value)
            .map(|opt| opt.map(|cache_value| cache_value.0))
    }
    
    /// Replace existing value with new value, returning the old value
    /// Returns None if key was not present
    pub fn replace(&self, key: K, value: V) -> Result<Option<V>, CacheOperationError> {
        let cache_key = SerdeCacheKey(key);
        let cache_value = SerdeCacheValue(value);
        
        self.manager.replace(cache_key, cache_value)
            .map(|opt| opt.map(|cache_value| cache_value.0))
    }
    
    /// Atomically replace value only if current value equals expected value
    /// Returns true if replacement occurred, false otherwise
    pub fn compare_and_swap(&self, key: K, expected: V, new_value: V) -> Result<bool, CacheOperationError>
    where 
        V: PartialEq
    {
        let cache_key = SerdeCacheKey(key);
        let expected_cache_value = SerdeCacheValue(expected);
        let new_cache_value = SerdeCacheValue(new_value);
        
        self.manager.compare_and_swap(cache_key, expected_cache_value, new_cache_value)
    }
    
    /// Get value or insert using factory function if key is absent
    /// Returns the existing value if present, or the newly inserted value
    pub fn get_or_insert<F>(&self, key: K, factory: F) -> Result<V, CacheOperationError>
    where 
        F: FnOnce() -> V
    {
        // Use atomic get-or-insert to avoid race conditions
        let cache_key = SerdeCacheKey(key);
        let result = self.manager.get_or_insert_atomic(cache_key, || {
            SerdeCacheValue(factory())
        })?;
        Ok(result.0)
    }
    
    /// Get value or insert using fallible factory function if key is absent
    /// Returns the existing value if present, or the newly inserted value
    pub fn get_or_insert_with<F, E>(&self, key: K, factory: F) -> Result<V, CacheOperationError>
    where 
        F: FnOnce() -> Result<V, E>,
        E: Into<CacheOperationError>
    {
        // First check if key exists (fast path)
        if let Some(existing_value) = self.get(&key) {
            return Ok(existing_value);
        }

        // Key doesn't exist, create value and use atomic insert
        let new_value = factory().map_err(|e| e.into())?;
        let cache_key = SerdeCacheKey(key);
        let cache_value = SerdeCacheValue(new_value.clone());
        
        match self.manager.put_if_absent(cache_key, cache_value)? {
            Some(existing) => Ok(existing.0), // Another thread inserted first
            None => Ok(new_value), // We successfully inserted
        }
    }
    
    /// Check if key exists in cache without retrieving the value
    pub fn contains_key(&self, key: &K) -> bool {
        let cache_key = SerdeCacheKey(key.clone());
        self.manager.contains_key(&cache_key)
    }
}

impl<K, V> std::fmt::Debug for Goldylox<K, V> 
where 
    K: Serialize + DeserializeOwned + Clone + Hash + Eq + Ord + Send + Sync + Debug + Default + 'static,
    V: Serialize + DeserializeOwned + Clone + Send + Sync + Debug + Default + 'static
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Goldylox")
            .field("cache_id", &"<cache>")
            .finish()
    }
}

/// Fluent builder for Goldylox configuration
pub struct GoldyloxBuilder<K, V> 
where 
    K: Serialize + DeserializeOwned + Clone + Hash + Eq + Ord + Send + Sync + Debug + Default + 'static,
    V: Serialize + DeserializeOwned + Clone + Send + Sync + Debug + Default + 'static
{
    config: CacheConfig,
    _phantom: std::marker::PhantomData<(K, V)>,
}

impl<K, V> GoldyloxBuilder<K, V>
where 
    K: Serialize + DeserializeOwned + Clone + Hash + Eq + Ord + Send + Sync + Debug + Default + 'static,
    V: Serialize + DeserializeOwned + Clone + Send + Sync + Debug + Default + 'static
{
    /// Create new builder with default configuration
    pub fn new() -> Self {
        Self {
            config: CacheConfig::default(),
            _phantom: std::marker::PhantomData,
        }
    }
    
    /// Set hot tier maximum entries
    pub fn hot_tier_max_entries(mut self, max_entries: u32) -> Self {
        self.config.hot_tier.max_entries = max_entries;
        self
    }
    

    
    /// Set hot tier memory limit in MB
    pub fn hot_tier_memory_limit_mb(mut self, limit_mb: u32) -> Self {
        self.config.hot_tier.memory_limit_mb = limit_mb;
        self
    }
    

    
    /// Set warm tier maximum entries
    pub fn warm_tier_max_entries(mut self, max_entries: usize) -> Self {
        self.config.warm_tier.max_entries = max_entries;
        self
    }
    

    
    /// Set warm tier memory limit in bytes
    pub fn warm_tier_max_memory_bytes(mut self, max_bytes: u64) -> Self {
        self.config.warm_tier.max_memory_bytes = max_bytes;
        self
    }
    
    /// Set cold tier base directory
    pub fn cold_tier_base_dir<P: AsRef<str>>(mut self, path: P) -> Self {
        let path_str = path.as_ref();
        if path_str.len() <= 256 {
            // ArrayString<256> requires manual construction
            self.config.cold_tier.base_dir.clear();
            for ch in path_str.chars() {
                let _ = self.config.cold_tier.base_dir.try_push(ch);
            }
        }
        self
    }    

    
    /// Set cold tier maximum size in bytes
    pub fn cold_tier_max_size_bytes(mut self, max_size_bytes: u64) -> Self {
        self.config.cold_tier.max_size_bytes = max_size_bytes;
        self
    }
    
    /// Set cold tier compression level (0-9)
    pub fn compression_level(mut self, level: u8) -> Self {
        self.config.cold_tier.compression_level = level.min(9);
        self
    }
    
    /// Set cache ID (defaults to UUID if not specified)
    pub fn cache_id<S: Into<String>>(mut self, id: S) -> Self {
        self.config.cache_id = id.into();
        // Update base dir to use the new cache ID
        self.config.cold_tier.base_dir = generate_storage_path(&self.config.cache_id);
        self
    }
    
    /// Set number of background worker threads
    pub fn background_worker_threads(mut self, count: u8) -> Self {
        self.config.worker.thread_pool_size = count;
        self
    }
    
    
    /// Build the cache with the configured settings
    /// 
    /// This initializes all cache tiers, crossbeam channels, workers, etc.
    /// All complex initialization is handled by UnifiedCacheManager.
    pub fn build(self) -> Result<Goldylox<K, V>, CacheOperationError> {
        // Delegate all complex initialization to UnifiedCacheManager
        let manager = UnifiedCacheManager::new(self.config)?;
        
        Ok(Goldylox { 
            manager,
        })
    }
}

impl<K, V> Default for GoldyloxBuilder<K, V>
where 
    K: Serialize + DeserializeOwned + Clone + Hash + Eq + Ord + Send + Sync + Debug + Default + 'static,
    V: Serialize + DeserializeOwned + Clone + Send + Sync + Debug + Default + 'static
{
    fn default() -> Self {
        Self::new()
    }
}