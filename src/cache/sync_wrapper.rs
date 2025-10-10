//! Synchronous wrappers for async cache operations
//! 
//! Use these ONLY if you cannot use async/await in your code.
//! These wrappers use tokio::runtime::Handle::current().block_on() internally.
//!
//! ## Requirements
//! 
//! - A tokio runtime must be running (e.g., #[tokio::main] or Runtime::new())
//! - Calling these methods blocks the current thread until async operation completes
//!
//! ## Example
//!
//! ```rust,ignore
//! use goldylox::cache::sync_wrapper::SyncCacheWrapper;
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() {
//!     let cache = Arc::new(UnifiedCacheManager::new(config).await.unwrap());
//!     let sync_cache = SyncCacheWrapper::new(cache);
//!     
//!     // Now can use blocking API in non-async code
//!     let value = sync_cache.get_blocking(&key);
//!     sync_cache.put_blocking(key, value)?;
//! }
//! ```

use tokio::runtime::Handle;
use std::sync::Arc;
use crate::cache::coordinator::unified_manager::UnifiedCacheManager;
use crate::cache::traits::core::{CacheKey, CacheValue};
use crate::cache::traits::types_and_enums::CacheOperationError;

/// Synchronous wrapper for UnifiedCacheManager
/// 
/// Provides blocking methods that internally use async operations via `Handle::current().block_on()`.
/// 
/// ## Warning
/// 
/// - Requires a tokio runtime to be running (will panic if no runtime exists)
/// - Blocks the calling thread (defeats the purpose of async)
/// - Only use during migration period or in environments without async support
/// - New code should use the async API directly via UnifiedCacheManager
pub struct SyncCacheWrapper<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue + Default + PartialEq + bincode::Encode + bincode::Decode<()> + 'static,
> {
    inner: Arc<UnifiedCacheManager<K, V>>,
}

impl<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue + Default + PartialEq + bincode::Encode + bincode::Decode<()> + 'static,
> SyncCacheWrapper<K, V> {
    /// Create a new synchronous wrapper
    /// 
    /// # Panics
    /// 
    /// Panics if called outside a tokio runtime context (no Handle::current())
    pub fn new(cache: Arc<UnifiedCacheManager<K, V>>) -> Self {
        // Validate runtime handle exists early
        let _ = Handle::try_current()
            .expect("SyncCacheWrapper requires a tokio runtime. Run inside #[tokio::main] or Runtime::new()");
        
        Self {
            inner: cache,
        }
    }
    
    /// Blocking get operation
    /// 
    /// Uses `Handle::current().block_on()` internally to wait for the async operation.
    /// 
    /// # Returns
    /// 
    /// - `Some(V)` if key exists in cache
    /// - `None` if key not found
    pub fn get_blocking(&self, key: &K) -> Option<V> {
        Handle::current().block_on(self.inner.get(key))
    }
    
    /// Blocking put operation
    /// 
    /// Uses `Handle::current().block_on()` internally to wait for the async operation.
    /// 
    /// # Returns
    /// 
    /// - `Ok(())` if value successfully stored
    /// - `Err(CacheOperationError)` if operation fails
    pub fn put_blocking(&self, key: K, value: V) -> Result<(), CacheOperationError> {
        Handle::current().block_on(self.inner.put(key, value))
    }
    
    /// Blocking remove operation
    /// 
    /// Uses `Handle::current().block_on()` internally to wait for the async operation.
    /// 
    /// # Returns
    /// 
    /// - `true` if key was present and removed
    /// - `false` if key was not found
    pub fn remove_blocking(&self, key: &K) -> bool {
        Handle::current().block_on(self.inner.remove(key))
    }
    
    /// Blocking contains_key operation
    /// 
    /// # Returns
    /// 
    /// - `true` if key exists in cache
    /// - `false` if key not found
    pub fn contains_key_blocking(&self, key: &K) -> bool {
        Handle::current().block_on(self.inner.contains_key(key))
    }
    
    /// Blocking clear operation
    /// 
    /// Clears all entries from all cache tiers.
    /// 
    /// # Returns
    /// 
    /// - `Ok(())` if cache successfully cleared
    /// - `Err(CacheOperationError)` if operation fails
    pub fn clear_blocking(&self) -> Result<(), CacheOperationError> {
        Handle::current().block_on(self.inner.clear())
    }
}

impl<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue + Default + PartialEq + bincode::Encode + bincode::Decode<()> + 'static,
> Clone for SyncCacheWrapper<K, V> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}
