//! Cache configuration system with production-ready defaults
//!
//! This module provides comprehensive configuration for the hierarchical cache system
//! with lock-free data structures and atomic metadata tracking.

pub mod builder;
pub mod global;
pub mod loader;
pub mod presets;
pub mod types;
pub mod validator;

// Re-export all types for easy access
pub use builder::CacheConfigBuilder;
pub use global::{
    get_global_config, get_global_config_version, init_global_config, is_global_config_initialized,
    update_global_config,
};
pub use loader::ConfigLoader;
pub use presets::ConfigPresets;
pub use types::{
    AlertThresholdsConfig, AnalyzerConfig, CacheConfig, ColdTierConfig, ConfigError,
    HashFunction, HotTierConfig, MonitoringConfig, WorkerConfig,
};
// Import consolidated eviction types from canonical location
pub use crate::cache::tier::warm::eviction::types::EvictionPolicyType;
pub use crate::cache::tier::warm::config::{SkipMapConfig, WarmTierConfig};
pub use validator::ConfigValidator;

/// Initialize configuration with default settings
pub fn init_default() -> Result<(), ConfigError> {
    init_global_config(CacheConfig::default())
}

/// Initialize configuration with high-performance preset
pub fn init_high_performance() -> Result<(), ConfigError> {
    init_global_config(ConfigPresets::high_performance())
}

/// Initialize configuration with memory-efficient preset
pub fn init_memory_efficient() -> Result<(), ConfigError> {
    init_global_config(ConfigPresets::memory_efficient())
}

/// Initialize configuration with balanced preset
pub fn init_balanced() -> Result<(), ConfigError> {
    init_global_config(ConfigPresets::balanced())
}

/// Initialize configuration for development
pub fn init_development() -> Result<(), ConfigError> {
    init_global_config(ConfigPresets::development())
}

/// Initialize configuration for production
pub fn init_production() -> Result<(), ConfigError> {
    init_global_config(ConfigPresets::production())
}

/// Initialize configuration for testing
pub fn init_testing() -> Result<(), ConfigError> {
    init_global_config(ConfigPresets::testing())
}

/// Initialize configuration for embedded systems
pub fn init_embedded() -> Result<(), ConfigError> {
    init_global_config(ConfigPresets::embedded())
}

/// Initialize configuration for debugging
pub fn init_debug() -> Result<(), ConfigError> {
    init_global_config(ConfigPresets::debug())
}
