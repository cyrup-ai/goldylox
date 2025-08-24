//! Global configuration management with atomic operations
//!
//! This module provides thread-safe global configuration storage
//! with TRUE lock-free access using crossbeam epoch-based reclamation.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicPtr, AtomicU64, Ordering};
use crossbeam_epoch::{self as epoch, Atomic, Owned, Shared};

use super::types::{CacheConfig, ConfigError};

/// Global configuration storage using epoch-based memory reclamation
struct GlobalConfigStorage {
    /// Current config pointer (epoch-protected)
    config: Atomic<CacheConfig>,
    /// Version counter
    version: AtomicU64,
}

/// Global configuration storage instance
static GLOBAL_CONFIG: OnceLock<GlobalConfigStorage> = OnceLock::new();

/// Initialize global configuration (call once at startup)
#[inline]
pub fn init_global_config(config: CacheConfig) -> Result<(), ConfigError> {
    GLOBAL_CONFIG
        .set(GlobalConfigStorage {
            config: Atomic::new(config),
            version: AtomicU64::new(1),
        })
        .map_err(|_| ConfigError::InvalidValue("Config already initialized".into()))?;

    Ok(())
}

/// Get global configuration (lock-free, wait-free read)
#[inline(always)]
pub fn get_global_config() -> CacheConfig {
    match GLOBAL_CONFIG.get() {
        Some(storage) => {
            let guard = &epoch::pin();
            let config_ptr = storage.config.load(Ordering::Acquire, guard);
            
            // Safe: config is always valid while guard is held
            if config_ptr.is_null() {
                CacheConfig::default()
            } else {
                unsafe { (*config_ptr.as_raw()).clone() }
            }
        }
        None => CacheConfig::default(),
    }
}

/// Access global configuration without cloning (zero-copy for reads)
#[inline(always)]
pub fn with_global_config<F, R>(f: F) -> R
where
    F: FnOnce(&CacheConfig) -> R,
    R: Default,
{
    match GLOBAL_CONFIG.get() {
        Some(storage) => {
            let guard = &epoch::pin();
            let config_ptr = storage.config.load(Ordering::Acquire, guard);
            
            if config_ptr.is_null() {
                R::default()
            } else {
                // Safe: config is valid while guard is held
                unsafe { f(&*config_ptr.as_raw()) }
            }
        }
        None => R::default(),
    }
}

/// Update global configuration atomically (lock-free)
#[inline(always)]
pub fn update_global_config(config: CacheConfig) -> Result<(), ConfigError> {
    let storage = GLOBAL_CONFIG
        .get()
        .ok_or(ConfigError::InvalidValue("Config not initialized".into()))?;
    
    let guard = &epoch::pin();
    let new_config = Owned::new(config);
    
    // Atomically swap the config pointer
    let old = storage.config.swap(new_config, Ordering::Release, guard);
    
    // Defer deallocation of old config until safe
    if !old.is_null() {
        unsafe { guard.defer_destroy(old) };
    }
    
    storage.version.fetch_add(1, Ordering::Release);
    Ok(())
}

/// Get global configuration version
#[inline(always)]
pub fn get_global_config_version() -> u64 {
    GLOBAL_CONFIG
        .get()
        .map(|s| s.version.load(Ordering::Acquire))
        .unwrap_or(0)
}

/// Check if global configuration is initialized
#[inline(always)]
pub fn is_global_config_initialized() -> bool {
    GLOBAL_CONFIG.get().is_some()
}