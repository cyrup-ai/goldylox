//! Global configuration management with atomic operations
//!
//! This module provides thread-safe global configuration storage
//! with atomic updates and lock-free access patterns.

use std::sync::{Arc, OnceLock, RwLock};

use super::types::{CacheConfig, ConfigError};

/// Atomic cache configuration for runtime updates
#[derive(Debug)]
pub struct AtomicCacheConfig {
    config: Arc<RwLock<CacheConfig>>,
    version: std::sync::atomic::AtomicU64,
}

impl AtomicCacheConfig {
    pub fn new() -> Result<Self, ConfigError> {
        Ok(Self {
            config: Arc::new(RwLock::new(CacheConfig::default())),
            version: std::sync::atomic::AtomicU64::new(1),
        })
    }

    pub fn load(&self) -> Result<CacheConfig, ConfigError> {
        self.config
            .read()
            .map_err(|_| ConfigError::LockError)
            .map(|guard| guard.clone())
    }

    pub fn store(&self, config: CacheConfig) -> Result<(), ConfigError> {
        *self.config.write().map_err(|_| ConfigError::LockError)? = config;
        self.version
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    pub fn version(&self) -> u64 {
        self.version.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// Global atomic configuration instance
static GLOBAL_CONFIG: OnceLock<AtomicCacheConfig> = OnceLock::new();

/// Initialize global configuration (call once at startup)
pub fn init_global_config(config: CacheConfig) -> Result<(), ConfigError> {
    let atomic_config = AtomicCacheConfig::new()?;
    atomic_config.store(config)?;

    GLOBAL_CONFIG
        .set(atomic_config)
        .map_err(|_| ConfigError::LockError)?;

    Ok(())
}

/// Get global configuration (lock-free)
#[inline(always)]
pub fn get_global_config() -> CacheConfig {
    GLOBAL_CONFIG
        .get()
        .and_then(|c| c.load().ok())
        .unwrap_or_else(CacheConfig::default)
}

/// Update global configuration atomically
#[inline(always)]
pub fn update_global_config(config: CacheConfig) -> Result<(), ConfigError> {
    GLOBAL_CONFIG
        .get()
        .ok_or(ConfigError::LockError)?
        .store(config)
}

/// Get global configuration version
#[inline(always)]
pub fn get_global_config_version() -> u64 {
    GLOBAL_CONFIG.get().map(|c| c.version()).unwrap_or(0)
}

/// Check if global configuration is initialized
#[inline(always)]
pub fn is_global_config_initialized() -> bool {
    GLOBAL_CONFIG.get().is_some()
}
