//! Configuration loading and serialization utilities
//!
//! This module provides zero-allocation configuration loading from files,
//! environment variables, and binary serialization with memory-mapped I/O.

use std::fs::File;
use std::mem::size_of;

use memmap2::MmapOptions;
use serde_json;
use toml;

use super::builder::CacheConfigBuilder;
use super::types::{CacheConfig, ConfigError};

/// Zero-allocation configuration loader using memory-mapped files
pub struct ConfigLoader;

impl ConfigLoader {
    /// Load configuration from memory-mapped binary file (fastest)
    pub fn from_binary_file(path: &str) -> Result<CacheConfig, ConfigError> {
        let file = File::open(path).map_err(|e| ConfigError::IoError(e.kind()))?;

        let mmap =
            unsafe { MmapOptions::new().map(&file) }.map_err(|e| ConfigError::IoError(e.kind()))?;

        if mmap.len() < size_of::<CacheConfig>() {
            return Err(ConfigError::InvalidFormat);
        }

        let config_ptr = mmap.as_ptr() as *const CacheConfig;
        let config = unsafe { config_ptr.read_unaligned() };

        // Validate configuration
        Self::validate_config(&config)?;

        Ok(config)
    }

    /// Save configuration to binary file (fastest)
    pub fn to_binary_file(config: &CacheConfig, path: &str) -> Result<(), ConfigError> {
        let bytes = unsafe {
            std::slice::from_raw_parts(
                config as *const CacheConfig as *const u8,
                size_of::<CacheConfig>(),
            )
        };

        std::fs::write(path, bytes).map_err(|e| ConfigError::IoError(e.kind()))
    }

    /// Load configuration from environment variables (zero allocation)
    pub fn from_env() -> Result<CacheConfig, ConfigError> {
        let mut builder = CacheConfigBuilder::new();

        // Hot tier settings
        if let Ok(capacity_str) = std::env::var("CACHE_HOT_CAPACITY") {
            if let Ok(capacity) = capacity_str.parse::<u32>() {
                builder = builder.hot_tier_capacity(capacity);
            }
        }

        if let Ok(enabled_str) = std::env::var("CACHE_HOT_ENABLED") {
            if let Ok(enabled) = enabled_str.parse::<bool>() {
                builder = builder.hot_tier_enabled(enabled);
            }
        }

        // Warm tier settings
        if let Ok(capacity_str) = std::env::var("CACHE_WARM_CAPACITY") {
            if let Ok(capacity) = capacity_str.parse::<u32>() {
                builder = builder.warm_tier_capacity(capacity);
            }
        }

        if let Ok(timeout_str) = std::env::var("CACHE_WARM_TIMEOUT_NS") {
            if let Ok(timeout_ns) = timeout_str.parse::<u64>() {
                builder = builder.warm_tier_timeout_ns(timeout_ns);
            }
        }

        if let Ok(enabled_str) = std::env::var("CACHE_WARM_ENABLED") {
            if let Ok(enabled) = enabled_str.parse::<bool>() {
                builder = builder.warm_tier_enabled(enabled);
            }
        }

        // Cold tier settings
        if let Ok(path) = std::env::var("CACHE_COLD_PATH") {
            builder = builder
                .cold_tier_storage(&path)
                .map_err(|_| ConfigError::InvalidFormat)?;
        }

        if let Ok(compression_str) = std::env::var("CACHE_COLD_COMPRESSION") {
            if let Ok(compression) = compression_str.parse::<u8>() {
                builder = builder.cold_tier_compression(compression);
            }
        }

        // Monitoring settings
        if let Ok(enabled_str) = std::env::var("CACHE_MONITORING_ENABLED") {
            if let Ok(enabled) = enabled_str.parse::<bool>() {
                builder = builder.monitoring_enabled(enabled);
            }
        }

        if let Ok(interval_str) = std::env::var("CACHE_MONITORING_INTERVAL_NS") {
            if let Ok(interval_ns) = interval_str.parse::<u64>() {
                builder = builder.monitoring_interval_ns(interval_ns);
            }
        }

        // Worker settings
        if let Ok(enabled_str) = std::env::var("CACHE_WORKER_ENABLED") {
            if let Ok(enabled) = enabled_str.parse::<bool>() {
                builder = builder.worker_enabled(enabled);
            }
        }

        if let Ok(threads_str) = std::env::var("CACHE_WORKER_THREADS") {
            if let Ok(threads) = threads_str.parse::<u8>() {
                builder = builder.worker_threads(threads);
            }
        }

        // Analyzer settings
        if let Ok(max_keys_str) = std::env::var("CACHE_ANALYZER_MAX_KEYS") {
            if let Ok(max_keys) = max_keys_str.parse::<usize>() {
                builder = builder.analyzer_max_keys(max_keys);
            }
        }

        if let Ok(decay_str) = std::env::var("CACHE_ANALYZER_FREQUENCY_DECAY") {
            if let Ok(decay_ns) = decay_str.parse::<f64>() {
                builder = builder.analyzer_frequency_decay(decay_ns);
            }
        }

        if let Ok(half_life_str) = std::env::var("CACHE_ANALYZER_RECENCY_HALF_LIFE") {
            if let Ok(half_life_ns) = half_life_str.parse::<f64>() {
                builder = builder.analyzer_recency_half_life(half_life_ns);
            }
        }

        Ok(builder.build())
    }

    /// Load configuration from file with automatic format detection
    pub fn from_file(path: &str) -> Result<CacheConfig, ConfigError> {
        use std::path::Path;

        let path = Path::new(path);

        // Validate file exists and is readable
        if !path.exists() {
            return Err(ConfigError::FileNotFound(
                path.to_string_lossy().to_string(),
            ));
        }

        if !path.is_file() {
            return Err(ConfigError::NotAFile(path.to_string_lossy().to_string()));
        }

        // Read file contents
        let contents = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::FileReadError(format!("{}: {}", path.display(), e)))?;

        // Parse based on file extension
        match path.extension().and_then(|s| s.to_str()) {
            Some("toml") => Self::from_toml(&contents),
            Some("json") => Self::from_json(&contents),
            Some(ext) => Err(ConfigError::UnsupportedFormat(ext.to_string())),
            None => Err(ConfigError::UnsupportedFormat("no extension".to_string())),
        }
    }

    /// Save configuration to file with format based on extension
    pub fn to_file(config: &CacheConfig, path: &str) -> Result<(), ConfigError> {
        use std::path::Path;

        let path = Path::new(path);

        let contents = match path.extension().and_then(|s| s.to_str()) {
            Some("toml") => Self::to_toml(config)?,
            Some("json") => Self::to_json(config)?,
            Some(ext) => return Err(ConfigError::UnsupportedFormat(ext.to_string())),
            None => return Err(ConfigError::UnsupportedFormat("no extension".to_string())),
        };

        std::fs::write(path, contents)
            .map_err(|e| ConfigError::FileReadError(format!("{}: {}", path.display(), e)))
    }

    /// Load configuration from file with environment variable overrides
    pub fn from_file_with_env_overrides(path: &str) -> Result<CacheConfig, ConfigError> {
        let mut config = Self::from_file(path)?;
        Self::apply_env_overrides(&mut config)?;
        Self::validate_config(&config)?;
        Ok(config)
    }

    /// Apply environment variable overrides to configuration
    fn apply_env_overrides(config: &mut CacheConfig) -> Result<(), ConfigError> {
        use arrayvec::ArrayString;

        // Hot tier overrides
        if let Ok(val) = std::env::var("CACHE_HOT_MAX_ENTRIES") {
            config.hot_tier.max_entries =
                val.parse().map_err(|_| ConfigError::InvalidFieldValue {
                    field: "CACHE_HOT_MAX_ENTRIES".to_string(),
                    value: val,
                    reason: "must be a valid positive integer".to_string(),
                })?;
        }

        if let Ok(val) = std::env::var("CACHE_HOT_ENABLED") {
            config.hot_tier.enabled = val.parse().map_err(|_| ConfigError::InvalidFieldValue {
                field: "CACHE_HOT_ENABLED".to_string(),
                value: val,
                reason: "must be 'true' or 'false'".to_string(),
            })?;
        }

        if let Ok(val) = std::env::var("CACHE_HOT_CACHE_LINE_SIZE") {
            config.hot_tier.cache_line_size =
                val.parse().map_err(|_| ConfigError::InvalidFieldValue {
                    field: "CACHE_HOT_CACHE_LINE_SIZE".to_string(),
                    value: val,
                    reason: "must be a valid positive integer".to_string(),
                })?;
        }

        if let Ok(val) = std::env::var("CACHE_HOT_PREFETCH_DISTANCE") {
            config.hot_tier.prefetch_distance =
                val.parse().map_err(|_| ConfigError::InvalidFieldValue {
                    field: "CACHE_HOT_PREFETCH_DISTANCE".to_string(),
                    value: val,
                    reason: "must be a valid positive integer".to_string(),
                })?;
        }

        // Warm tier overrides
        if let Ok(val) = std::env::var("CACHE_WARM_MAX_ENTRIES") {
            config.warm_tier.max_entries =
                val.parse().map_err(|_| ConfigError::InvalidFieldValue {
                    field: "CACHE_WARM_MAX_ENTRIES".to_string(),
                    value: val,
                    reason: "must be a valid positive integer".to_string(),
                })?;
        }

        if let Ok(val) = std::env::var("CACHE_WARM_MAX_SIZE_BYTES") {
            config.warm_tier.max_memory_bytes =
                val.parse().map_err(|_| ConfigError::InvalidFieldValue {
                    field: "CACHE_WARM_MAX_SIZE_BYTES".to_string(),
                    value: val,
                    reason: "must be a valid positive integer".to_string(),
                })?;
        }

        if let Ok(val) = std::env::var("CACHE_WARM_ENABLED") {
            config.warm_tier.enabled = val.parse().map_err(|_| ConfigError::InvalidFieldValue {
                field: "CACHE_WARM_ENABLED".to_string(),
                value: val,
                reason: "must be 'true' or 'false'".to_string(),
            })?;
        }

        if let Ok(val) = std::env::var("CACHE_WARM_ENTRY_TIMEOUT_NS") {
            let timeout_ns: u64 = val.parse().map_err(|_| ConfigError::InvalidFieldValue {
                field: "CACHE_WARM_ENTRY_TIMEOUT_NS".to_string(),
                value: val,
                reason: "must be a valid positive integer".to_string(),
            })?;
            config.warm_tier.default_ttl_sec = timeout_ns / 1_000_000_000;
        }

        // Cold tier overrides
        if let Ok(val) = std::env::var("CACHE_COLD_STORAGE_PATH") {
            config.cold_tier.storage_path =
                ArrayString::from(&val).map_err(|_| ConfigError::InvalidFieldValue {
                    field: "CACHE_COLD_STORAGE_PATH".to_string(),
                    value: val.clone(),
                    reason: "path too long (max 256 characters)".to_string(),
                })?;
        }

        if let Ok(val) = std::env::var("CACHE_COLD_ENABLED") {
            config.cold_tier.enabled = val.parse().map_err(|_| ConfigError::InvalidFieldValue {
                field: "CACHE_COLD_ENABLED".to_string(),
                value: val,
                reason: "must be 'true' or 'false'".to_string(),
            })?;
        }

        if let Ok(val) = std::env::var("CACHE_COLD_COMPRESSION_LEVEL") {
            config.cold_tier.compression_level =
                val.parse().map_err(|_| ConfigError::InvalidFieldValue {
                    field: "CACHE_COLD_COMPRESSION_LEVEL".to_string(),
                    value: val,
                    reason: "must be an integer between 0 and 9".to_string(),
                })?;
        }

        // Worker overrides
        if let Ok(val) = std::env::var("CACHE_WORKER_ENABLED") {
            config.worker.enabled = val.parse().map_err(|_| ConfigError::InvalidFieldValue {
                field: "CACHE_WORKER_ENABLED".to_string(),
                value: val,
                reason: "must be 'true' or 'false'".to_string(),
            })?;
        }

        if let Ok(val) = std::env::var("CACHE_WORKER_THREAD_POOL_SIZE") {
            config.worker.thread_pool_size =
                val.parse().map_err(|_| ConfigError::InvalidFieldValue {
                    field: "CACHE_WORKER_THREAD_POOL_SIZE".to_string(),
                    value: val,
                    reason: "must be a valid positive integer".to_string(),
                })?;
        }

        if let Ok(val) = std::env::var("CACHE_WORKER_TASK_QUEUE_CAPACITY") {
            config.worker.task_queue_capacity =
                val.parse().map_err(|_| ConfigError::InvalidFieldValue {
                    field: "CACHE_WORKER_TASK_QUEUE_CAPACITY".to_string(),
                    value: val,
                    reason: "must be a valid positive integer".to_string(),
                })?;
        }

        // Monitoring overrides
        if let Ok(val) = std::env::var("CACHE_MONITORING_ENABLED") {
            config.monitoring.enabled =
                val.parse().map_err(|_| ConfigError::InvalidFieldValue {
                    field: "CACHE_MONITORING_ENABLED".to_string(),
                    value: val,
                    reason: "must be 'true' or 'false'".to_string(),
                })?;
        }

        if let Ok(val) = std::env::var("CACHE_MONITORING_SAMPLE_INTERVAL_NS") {
            config.monitoring.sample_interval_ns =
                val.parse().map_err(|_| ConfigError::InvalidFieldValue {
                    field: "CACHE_MONITORING_SAMPLE_INTERVAL_NS".to_string(),
                    value: val,
                    reason: "must be a valid positive integer".to_string(),
                })?;
        }

        Ok(())
    }

    /// Load configuration from JSON string (allocating)
    pub fn from_json(json_str: &str) -> Result<CacheConfig, ConfigError> {
        let config: CacheConfig = serde_json::from_str(json_str)
            .map_err(|e| ConfigError::JsonParseError(e.to_string()))?;
        Self::validate_config(&config)?;
        Ok(config)
    }

    /// Save configuration to JSON string (allocating)
    pub fn to_json(config: &CacheConfig) -> Result<String, ConfigError> {
        Self::validate_config(config)?;
        serde_json::to_string_pretty(config)
            .map_err(|e| ConfigError::JsonParseError(format!("Serialization failed: {}", e)))
    }

    /// Load configuration from TOML string (allocating)
    pub fn from_toml(toml_str: &str) -> Result<CacheConfig, ConfigError> {
        let config: CacheConfig =
            toml::from_str(toml_str).map_err(|e| ConfigError::TomlParseError(e.to_string()))?;
        Self::validate_config(&config)?;
        Ok(config)
    }

    /// Save configuration to TOML string (allocating)
    pub fn to_toml(config: &CacheConfig) -> Result<String, ConfigError> {
        Self::validate_config(config)?;
        toml::to_string_pretty(config)
            .map_err(|e| ConfigError::TomlParseError(format!("Serialization failed: {}", e)))
    }

    /// Validate configuration for consistency
    fn validate_config(config: &CacheConfig) -> Result<(), ConfigError> {
        // Hot tier validation
        if config.hot_tier.enabled && config.hot_tier.max_entries == 0 {
            return Err(ConfigError::InvalidFieldValue {
                field: "hot_tier.max_entries".to_string(),
                value: "0".to_string(),
                reason: "must be greater than 0 when hot tier is enabled".to_string(),
            });
        }

        // Ensure hot tier capacity is power of 2
        if config.hot_tier.max_entries > 0
            && config.hot_tier.max_entries & (config.hot_tier.max_entries - 1) != 0
        {
            return Err(ConfigError::InvalidFieldValue {
                field: "hot_tier.max_entries".to_string(),
                value: config.hot_tier.max_entries.to_string(),
                reason: "must be a power of 2".to_string(),
            });
        }

        // Cache line size validation
        if config.hot_tier.cache_line_size == 0
            || config.hot_tier.cache_line_size & (config.hot_tier.cache_line_size - 1) != 0
        {
            return Err(ConfigError::InvalidFieldValue {
                field: "hot_tier.cache_line_size".to_string(),
                value: config.hot_tier.cache_line_size.to_string(),
                reason: "must be a power of 2 and greater than 0".to_string(),
            });
        }

        // Warm tier validation
        if config.warm_tier.enabled && config.warm_tier.max_entries == 0 {
            return Err(ConfigError::InvalidFieldValue {
                field: "warm_tier.max_entries".to_string(),
                value: "0".to_string(),
                reason: "must be greater than 0 when warm tier is enabled".to_string(),
            });
        }

        // Ensure warm tier capacity is power of 2
        if config.warm_tier.max_entries > 0
            && config.warm_tier.max_entries & (config.warm_tier.max_entries - 1) != 0
        {
            return Err(ConfigError::InvalidFieldValue {
                field: "warm_tier.max_entries".to_string(),
                value: config.warm_tier.max_entries.to_string(),
                reason: "must be a power of 2".to_string(),
            });
        }

        // Timeout validation
        if config.warm_tier.enabled && config.warm_tier.default_ttl_sec == 0 {
            return Err(ConfigError::InvalidFieldValue {
                field: "warm_tier.default_ttl_sec".to_string(),
                value: "0".to_string(),
                reason: "must be greater than 0 when warm tier is enabled".to_string(),
            });
        }

        // Cold tier validation
        if config.cold_tier.enabled && config.cold_tier.storage_path.is_empty() {
            return Err(ConfigError::MissingRequiredField(
                "cold_tier.storage_path".to_string(),
            ));
        }

        // Compression level validation
        if config.cold_tier.compression_level > 9 {
            return Err(ConfigError::InvalidFieldValue {
                field: "cold_tier.compression_level".to_string(),
                value: config.cold_tier.compression_level.to_string(),
                reason: "must be between 0 and 9".to_string(),
            });
        }

        // Worker validation
        if config.worker.enabled && config.worker.thread_pool_size == 0 {
            return Err(ConfigError::InvalidFieldValue {
                field: "worker.thread_pool_size".to_string(),
                value: "0".to_string(),
                reason: "must be greater than 0 when worker is enabled".to_string(),
            });
        }

        // Task queue capacity validation
        if config.worker.task_queue_capacity > 0
            && config.worker.task_queue_capacity & (config.worker.task_queue_capacity - 1) != 0
        {
            return Err(ConfigError::InvalidFieldValue {
                field: "worker.task_queue_capacity".to_string(),
                value: config.worker.task_queue_capacity.to_string(),
                reason: "must be a power of 2".to_string(),
            });
        }

        // Monitoring validation
        if config.monitoring.enabled && config.monitoring.sample_interval_ns == 0 {
            return Err(ConfigError::InvalidFieldValue {
                field: "monitoring.sample_interval_ns".to_string(),
                value: "0".to_string(),
                reason: "must be greater than 0 when monitoring is enabled".to_string(),
            });
        }

        // Cross-tier consistency validation
        if config.hot_tier.enabled && config.warm_tier.enabled {
            if config.hot_tier.max_entries as usize >= config.warm_tier.max_entries {
                return Err(ConfigError::ValidationError(
                    "Hot tier max_entries should be smaller than warm tier max_entries for optimal performance".to_string()
                ));
            }
        }

        // Memory usage estimation validation
        let estimated_hot_memory = config.hot_tier.max_entries as u64 * 1024; // Rough estimate
        let estimated_warm_memory = config.warm_tier.max_entries as u64 * 512; // Rough estimate
        let total_estimated = estimated_hot_memory + estimated_warm_memory;

        if total_estimated > 1_073_741_824 {
            // 1GB limit for safety
            return Err(ConfigError::ValidationError(format!(
                "Estimated memory usage ({} bytes) exceeds recommended limit (1GB)",
                total_estimated
            )));
        }

        Ok(())
    }
}
