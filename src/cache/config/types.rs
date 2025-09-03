//! Core configuration types and enums
//!
//! This module defines the fundamental data structures and enums used throughout
//! the cache configuration system, including tier configs and alert thresholds.

use arrayvec::ArrayString;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

// Import canonical types directly to avoid type identity conflicts
use crate::cache::tier::warm::config::WarmTierConfig;
use crate::cache::tier::warm::eviction::types::EvictionPolicyType;

/// Custom ArrayString serialization module
mod arraystring_serde {
    use super::*;

    pub fn serialize<S>(value: &ArrayString<256>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.as_str().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<ArrayString<256>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        ArrayString::from(&s).map_err(serde::de::Error::custom)
    }
}

/// Hash function types for cache optimization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HashFunction {
    #[serde(rename = "xxhash")]
    XxHash,
    #[serde(rename = "ahash")]
    AHash,
    #[serde(rename = "fnv")]
    FnvHash,
}

// EvictionPolicy moved to canonical location: crate::cache::tier::warm::eviction::types::EvictionPolicyType
// Use the comprehensive "best of best" implementation for all eviction policy needs

/// Hot tier configuration (cache-aligned, 64 bytes) - Unified canonical version
#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(align(64))]
pub struct HotTierConfig {
    pub max_entries: u32,
    pub enabled: bool,
    pub hash_function: HashFunction,
    pub eviction_policy: EvictionPolicyType,
    pub cache_line_size: u8,
    pub prefetch_distance: u8,
    /// Enable SIMD optimizations (from tier-specific version)
    pub enable_simd: bool,
    /// Enable prefetching (from tier-specific version)  
    pub enable_prefetch: bool,
    /// LRU eviction threshold in seconds (from tier-specific version)
    pub lru_threshold_secs: u32,
    /// Memory limit in megabytes (from tier-specific version)
    pub memory_limit_mb: u32,
    #[serde(skip)]
    pub _padding: [u8; 1],
}

// Removed re-export to eliminate type identity conflicts  
// Use direct imports: crate::cache::tier::warm::config::{WarmTierConfig, SkipMapConfig}

/// Cold tier configuration with persistent storage
#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(align(64))]
pub struct ColdTierConfig {
    pub enabled: bool,
    #[serde(with = "arraystring_serde")]
    pub storage_path: ArrayString<256>,
    pub max_size_bytes: u64,
    pub max_file_size: u64,
    pub compression_level: u8,
    pub auto_compact: bool,
    pub compact_interval_ns: u64,
    pub mmap_size: u64,
    pub write_buffer_size: u32,
    #[serde(skip)]
    pub _padding: [u8; 2],
}



/// Monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub enabled: bool,
    pub sample_interval_ns: u64,
    pub max_history_samples: u32,
    pub enable_alerts: bool,
    pub enable_tracing: bool,
    pub metrics_frequency_hz: u16,
    #[serde(skip)]
    pub _padding: [u8; 4],
}

/// Worker configuration for background tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    pub enabled: bool,
    pub thread_pool_size: u8,
    pub task_queue_capacity: u32,
    pub maintenance_interval_ns: u64,
    pub auto_tier_management: bool,
    pub cpu_affinity_mask: u64,
    pub priority_level: u8,
    pub batch_size: u16,
    #[serde(skip)]
    pub _padding: [u8; 4],
}

/// Configuration for access pattern analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzerConfig {
    /// Maximum number of keys to track in memory
    pub max_tracked_keys: usize,
    /// Time decay constant for frequency calculation (nanoseconds)
    pub frequency_decay_constant: f64,
    /// Half-life for recency calculation (nanoseconds)
    pub recency_half_life: f64,
    /// Age threshold for cleanup (nanoseconds)
    pub cleanup_age_threshold_ns: u64,
    /// Number of operations between cleanup cycles
    pub cleanup_interval: u64,
    /// Number of time buckets for sliding window frequency
    pub time_bucket_count: usize,
    /// Duration of each time bucket (nanoseconds)
    pub time_bucket_duration_ns: u64,
    /// Pattern detection window size
    pub pattern_analysis_window: usize,
}

/// Memory pressure monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Maximum memory usage limit in bytes (None = auto-detect)
    pub max_memory_usage: Option<u64>,
    /// Memory pressure monitoring enabled
    pub monitoring_enabled: bool,
    /// Low pressure threshold (0.0-1.0)
    pub low_pressure_threshold: f64,
    /// Medium pressure threshold (0.0-1.0)
    pub medium_pressure_threshold: f64,
    /// High pressure threshold (0.0-1.0)
    pub high_pressure_threshold: f64,
    /// Critical pressure threshold (0.0-1.0)
    pub critical_pressure_threshold: f64,
    /// Memory leak detection enabled
    pub leak_detection_enabled: bool,
    /// Alert cooldown period in milliseconds
    pub alert_cooldown_ms: u64,
    /// Sample collection interval in milliseconds
    pub sample_interval_ms: u64,
    /// Maximum samples to keep in history
    pub max_history_samples: usize,
}

/// Main cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub hot_tier: HotTierConfig,
    pub warm_tier: WarmTierConfig,
    pub cold_tier: ColdTierConfig,
    pub monitoring: MonitoringConfig,
    pub worker: WorkerConfig,
    pub analyzer: AnalyzerConfig,
    pub memory_config: MemoryConfig,
    pub version: u32,
}

/// Configuration error types
#[allow(dead_code)] // Configuration system - used in config validation and error handling
#[derive(Debug, Clone)]
pub enum ConfigError {
    InvalidValue(String),
    IoError(std::io::ErrorKind),
    LockError,
    InvalidFormat,

    // File-related errors
    FileNotFound(String),
    NotAFile(String),
    FileReadError(String),
    TomlParseError(String),
    JsonParseError(String),
    UnsupportedFormat(String),

    // Enhanced validation errors
    ValidationError(String),
    MissingRequiredField(String),
    InvalidFieldValue {
        field: String,
        value: String,
        reason: String,
    },

    // Existing specific errors
    HotTierInvalid,
    WarmTierInvalid,
    PowerOfTwoRequired,
    TimeoutInvalid,
    ColdTierPathRequired,
    CompressionLevelInvalid,
    MonitoringIntervalInvalid,
    WorkerThreadsInvalid,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::InvalidValue(msg) => write!(f, "Invalid configuration value: {}", msg),
            ConfigError::IoError(kind) => write!(f, "IO error: {:?}", kind),
            ConfigError::LockError => write!(f, "Lock acquisition failed"),
            ConfigError::InvalidFormat => write!(f, "Invalid configuration format"),

            // File-related errors
            ConfigError::FileNotFound(path) => write!(f, "Configuration file not found: {}", path),
            ConfigError::NotAFile(path) => write!(f, "Path is not a file: {}", path),
            ConfigError::FileReadError(msg) => {
                write!(f, "Failed to read configuration file: {}", msg)
            }
            ConfigError::TomlParseError(msg) => write!(f, "TOML parsing error: {}", msg),
            ConfigError::JsonParseError(msg) => write!(f, "JSON parsing error: {}", msg),
            ConfigError::UnsupportedFormat(ext) => write!(f, "Unsupported file format: {}", ext),

            // Enhanced validation errors
            ConfigError::ValidationError(msg) => {
                write!(f, "Configuration validation error: {}", msg)
            }
            ConfigError::MissingRequiredField(field) => {
                write!(f, "Missing required field: {}", field)
            }
            ConfigError::InvalidFieldValue {
                field,
                value,
                reason,
            } => {
                write!(
                    f,
                    "Invalid value '{}' for field '{}': {}",
                    value, field, reason
                )
            }

            // Existing specific errors
            ConfigError::HotTierInvalid => write!(f, "Hot tier configuration is invalid"),
            ConfigError::WarmTierInvalid => write!(f, "Warm tier configuration is invalid"),
            ConfigError::PowerOfTwoRequired => write!(f, "Value must be a power of two"),
            ConfigError::TimeoutInvalid => write!(f, "Timeout value is invalid"),
            ConfigError::ColdTierPathRequired => write!(f, "Cold tier storage path is required"),
            ConfigError::CompressionLevelInvalid => write!(f, "Compression level must be 0-9"),
            ConfigError::MonitoringIntervalInvalid => write!(f, "Monitoring interval is invalid"),
            ConfigError::WorkerThreadsInvalid => write!(f, "Worker thread count is invalid"),
        }
    }
}

impl std::error::Error for ConfigError {}

/// Default implementations for all config types

impl Default for HotTierConfig {
    fn default() -> Self {
        Self {
            max_entries: 128,
            enabled: true,
            hash_function: HashFunction::AHash,
            eviction_policy: EvictionPolicyType::Lru,
            cache_line_size: 64,
            prefetch_distance: 2,
            enable_simd: cfg!(target_arch = "x86_64"),
            enable_prefetch: true,
            lru_threshold_secs: 300, // 5 minutes
            memory_limit_mb: 64,     // 64MB
            _padding: [0; 1],
        }
    }
}



impl Default for ColdTierConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            storage_path: ArrayString::new(),
            max_size_bytes: 1024 * 1024 * 1024, // 1GB for cold tier
            max_file_size: 100 * 1024 * 1024,
            compression_level: 6,
            auto_compact: true,
            compact_interval_ns: 3_600_000_000_000,
            mmap_size: 1024 * 1024 * 1024,
            write_buffer_size: 64 * 1024,
            _padding: [0; 2],
        }
    }
}



impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sample_interval_ns: 10_000_000_000,
            max_history_samples: 1024,
            enable_alerts: true,
            enable_tracing: false,
            metrics_frequency_hz: 100,
            _padding: [0; 4],
        }
    }
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            thread_pool_size: 2,
            task_queue_capacity: 1024,
            maintenance_interval_ns: 60_000_000_000,
            auto_tier_management: true,
            cpu_affinity_mask: 0,
            priority_level: 10,
            batch_size: 32,
            _padding: [0; 4],
        }
    }
}

impl Default for AnalyzerConfig {
    fn default() -> Self {
        Self {
            max_tracked_keys: 10_000,
            frequency_decay_constant: 1_000_000_000.0, // 1 second
            recency_half_life: 300_000_000_000.0,      // 5 minutes
            cleanup_age_threshold_ns: 3_600_000_000_000, // 1 hour
            cleanup_interval: 1000,
            time_bucket_count: 60, // 1 minute of buckets at 1 second each
            time_bucket_duration_ns: 1_000_000_000, // 1 second
            pattern_analysis_window: 100,
        }
    }
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            max_memory_usage: None, // Auto-detect system memory
            monitoring_enabled: true,
            low_pressure_threshold: 0.6,
            medium_pressure_threshold: 0.75,
            high_pressure_threshold: 0.9,
            critical_pressure_threshold: 0.98,
            leak_detection_enabled: true,
            alert_cooldown_ms: 30000,     // 30 seconds
            sample_interval_ms: 1000,     // 1 second  
            max_history_samples: 256,     // ~4 minutes of history
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            hot_tier: HotTierConfig::default(),
            warm_tier: WarmTierConfig::default(),
            cold_tier: ColdTierConfig::default(),
            monitoring: MonitoringConfig::default(),
            worker: WorkerConfig::default(),
            analyzer: AnalyzerConfig::default(),
            memory_config: MemoryConfig::default(),
            version: 1,
        }
    }
}

impl MemoryConfig {
    /// Validate memory configuration parameters
    #[allow(dead_code)] // Configuration system - used in config validation and error handling
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate threshold ordering and ranges
        if !(0.0..=1.0).contains(&self.low_pressure_threshold) {
            return Err(ConfigError::InvalidFieldValue {
                field: "low_pressure_threshold".to_string(),
                value: self.low_pressure_threshold.to_string(),
                reason: "Must be between 0.0 and 1.0".to_string(),
            });
        }

        if !(0.0..=1.0).contains(&self.medium_pressure_threshold) {
            return Err(ConfigError::InvalidFieldValue {
                field: "medium_pressure_threshold".to_string(),
                value: self.medium_pressure_threshold.to_string(),
                reason: "Must be between 0.0 and 1.0".to_string(),
            });
        }

        if !(0.0..=1.0).contains(&self.high_pressure_threshold) {
            return Err(ConfigError::InvalidFieldValue {
                field: "high_pressure_threshold".to_string(),
                value: self.high_pressure_threshold.to_string(),
                reason: "Must be between 0.0 and 1.0".to_string(),
            });
        }

        if !(0.0..=1.0).contains(&self.critical_pressure_threshold) {
            return Err(ConfigError::InvalidFieldValue {
                field: "critical_pressure_threshold".to_string(),
                value: self.critical_pressure_threshold.to_string(),
                reason: "Must be between 0.0 and 1.0".to_string(),
            });
        }

        // Validate threshold ordering
        if self.low_pressure_threshold >= self.medium_pressure_threshold {
            return Err(ConfigError::ValidationError(
                "low_pressure_threshold must be less than medium_pressure_threshold".to_string(),
            ));
        }

        if self.medium_pressure_threshold >= self.high_pressure_threshold {
            return Err(ConfigError::ValidationError(
                "medium_pressure_threshold must be less than high_pressure_threshold".to_string(),
            ));
        }

        if self.high_pressure_threshold >= self.critical_pressure_threshold {
            return Err(ConfigError::ValidationError(
                "high_pressure_threshold must be less than critical_pressure_threshold".to_string(),
            ));
        }

        // Validate timing parameters
        if self.alert_cooldown_ms == 0 {
            return Err(ConfigError::InvalidFieldValue {
                field: "alert_cooldown_ms".to_string(),
                value: "0".to_string(),
                reason: "Alert cooldown must be greater than 0".to_string(),
            });
        }

        if self.sample_interval_ms == 0 {
            return Err(ConfigError::InvalidFieldValue {
                field: "sample_interval_ms".to_string(),
                value: "0".to_string(),
                reason: "Sample interval must be greater than 0".to_string(),
            });
        }

        if self.max_history_samples == 0 {
            return Err(ConfigError::InvalidFieldValue {
                field: "max_history_samples".to_string(),
                value: "0".to_string(),
                reason: "Max history samples must be greater than 0".to_string(),
            });
        }

        // Validate memory limit if specified
        if let Some(max_memory) = self.max_memory_usage {
            if max_memory < 1024 * 1024 {  // 1MB minimum
                return Err(ConfigError::InvalidFieldValue {
                    field: "max_memory_usage".to_string(),
                    value: max_memory.to_string(),
                    reason: "Memory limit must be at least 1MB".to_string(),
                });
            }
        }

        Ok(())
    }
}

impl CacheConfig {
    /// Validate entire cache configuration
    #[allow(dead_code)] // Configuration system - used in config validation and error handling
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate memory configuration
        self.memory_config.validate()?;
        
        // Additional validation can be added here for other components
        
        Ok(())
    }

    /// Create high-performance configuration
    #[allow(dead_code)] // Configuration system - used in config validation and error handling
    pub fn high_performance() -> Self {
        Self {
            hot_tier: HotTierConfig {
                max_entries: 1024,
                enabled: true,
                hash_function: HashFunction::XxHash,
                eviction_policy: EvictionPolicyType::Lru,
                cache_line_size: 64,
                prefetch_distance: 8,
                enable_simd: true,
                enable_prefetch: true,
                lru_threshold_secs: 180, // 3 minutes for high performance
                memory_limit_mb: 128,    // 128MB for high performance
                _padding: [0; 1],
            },
            warm_tier: WarmTierConfig::default(),
            cold_tier: ColdTierConfig::default(),
            monitoring: MonitoringConfig::default(),
            worker: WorkerConfig::default(),
            analyzer: AnalyzerConfig::default(),
            memory_config: MemoryConfig::default(),
            version: 1,
        }
    }

    /// Create low-memory configuration
    #[allow(dead_code)] // Configuration system - used in config validation and error handling
    pub fn low_memory() -> Self {
        Self {
            hot_tier: HotTierConfig {
                max_entries: 64,
                enabled: true,
                hash_function: HashFunction::XxHash,
                eviction_policy: EvictionPolicyType::Lru,
                cache_line_size: 32,
                prefetch_distance: 2,
                enable_simd: false,       // Disabled for low memory
                enable_prefetch: false,   // Disabled for low memory
                lru_threshold_secs: 600,  // 10 minutes for low memory
                memory_limit_mb: 16,      // 16MB for low memory
                _padding: [0; 1],
            },
            warm_tier: WarmTierConfig::default(),
            cold_tier: ColdTierConfig::default(),
            monitoring: MonitoringConfig::default(),
            worker: WorkerConfig::default(),
            analyzer: AnalyzerConfig::default(),
            memory_config: MemoryConfig::default(),
            version: 1,
        }
    }
}
