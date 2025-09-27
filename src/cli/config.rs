//! CLI configuration management
//!
//! This module handles CLI configuration, including loading from files
//! and merging with command-line arguments.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::cache::config::CacheConfig;
use crate::cli::{errors::CliResult, output::OutputFormat};

/// CLI configuration structure
#[derive(Debug, Clone)]
pub struct CliConfig {
    /// Cache configuration file path
    pub config_file: Option<PathBuf>,
    /// Cache instance identifier
    pub cache_id: Option<String>,
    /// Hot tier memory limit in MB
    pub hot_tier_memory: Option<u32>,
    /// Warm tier memory limit in MB
    pub warm_tier_memory: Option<u32>,
    /// Cold tier size limit in MB
    pub cold_tier_size: Option<u64>,
    /// Compression level (0-9)
    pub compression_level: Option<u8>,
    /// Number of background worker threads
    pub workers: Option<u8>,
    /// Output format
    pub output_format: OutputFormat,
    /// Verbosity level
    pub verbose: u8,
    /// Quiet mode
    pub quiet: bool,
    /// Daemon endpoint
    pub daemon_endpoint: Option<String>,
    /// Daemon request timeout in milliseconds
    pub daemon_timeout_ms: Option<u64>,
    /// Auto-start daemon if not running
    pub auto_start_daemon: Option<bool>,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            config_file: None,
            cache_id: None,
            hot_tier_memory: None,
            warm_tier_memory: None,
            cold_tier_size: None,
            compression_level: None,
            workers: None,
            output_format: OutputFormat::Human,
            verbose: 0,
            quiet: false,
            daemon_endpoint: None,
            daemon_timeout_ms: None,
            auto_start_daemon: None,
        }
    }
}

/// Configuration file format
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ConfigFile {
    /// Cache configuration
    #[serde(default)]
    pub cache: CacheFileConfig,
    /// CLI configuration
    #[serde(default)]
    pub cli: CliFileConfig,
}

/// Cache configuration in file format
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CacheFileConfig {
    /// Cache instance ID
    pub cache_id: Option<String>,
    /// Hot tier configuration
    #[serde(default)]
    pub hot_tier: HotTierFileConfig,
    /// Warm tier configuration
    #[serde(default)]
    pub warm_tier: WarmTierFileConfig,
    /// Cold tier configuration
    #[serde(default)]
    pub cold_tier: ColdTierFileConfig,
    /// Worker configuration
    #[serde(default)]
    pub worker: WorkerFileConfig,
}

/// Hot tier configuration in file format
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct HotTierFileConfig {
    /// Maximum entries
    pub max_entries: Option<u32>,
    /// Memory limit in MB
    pub memory_limit_mb: Option<u32>,
}

/// Warm tier configuration in file format
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct WarmTierFileConfig {
    /// Maximum entries
    pub max_entries: Option<usize>,
    /// Maximum memory in bytes
    pub max_memory_bytes: Option<u64>,
}

/// Cold tier configuration in file format
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ColdTierFileConfig {
    /// Base directory
    pub base_dir: Option<String>,
    /// Maximum size in bytes
    pub max_size_bytes: Option<u64>,
    /// Compression level (0-9)
    pub compression_level: Option<u8>,
}

/// Worker configuration in file format
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct WorkerFileConfig {
    /// Thread pool size
    pub thread_pool_size: Option<u8>,
}

/// CLI configuration in file format
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CliFileConfig {
    /// Default output format
    pub output_format: Option<String>,
    /// Default verbosity
    pub verbose: Option<u8>,
    /// Default quiet mode
    pub quiet: Option<bool>,
}

impl CliConfig {
    /// Load configuration from file and merge with CLI options
    pub fn load_and_merge(mut self) -> CliResult<(Self, CacheConfig)> {
        // Load file configuration if specified
        let file_config = if let Some(config_path) = &self.config_file {
            Some(Self::load_config_file(config_path)?)
        } else {
            None
        };

        // Merge file configuration with CLI options
        if let Some(file_config) = &file_config {
            // Merge CLI-specific options from file if not specified on command line
            if self.output_format == OutputFormat::Human
                && let Some(format_str) = &file_config.cli.output_format
            {
                self.output_format = match format_str.as_str() {
                    "json" => OutputFormat::Json,
                    "quiet" => OutputFormat::Quiet,
                    _ => OutputFormat::Human,
                };
            }

            if self.verbose == 0 {
                self.verbose = file_config.cli.verbose.unwrap_or(0);
            }

            if !self.quiet {
                self.quiet = file_config.cli.quiet.unwrap_or(false);
            }

            // Merge cache options
            if self.cache_id.is_none() {
                self.cache_id = file_config.cache.cache_id.clone();
            }

            if self.hot_tier_memory.is_none() {
                self.hot_tier_memory = file_config.cache.hot_tier.memory_limit_mb;
            }

            if self.warm_tier_memory.is_none() {
                self.warm_tier_memory = file_config
                    .cache
                    .warm_tier
                    .max_memory_bytes
                    .map(|bytes| (bytes / 1024 / 1024) as u32);
            }

            if self.cold_tier_size.is_none() {
                self.cold_tier_size = file_config.cache.cold_tier.max_size_bytes;
            }

            if self.compression_level.is_none() {
                self.compression_level = file_config.cache.cold_tier.compression_level;
            }

            if self.workers.is_none() {
                self.workers = file_config.cache.worker.thread_pool_size;
            }
        }

        // Build cache configuration
        let mut cache_config = CacheConfig::default();

        // Apply configuration overrides
        if let Some(cache_id) = &self.cache_id {
            cache_config.cache_id = cache_id.clone();
        }

        if let Some(hot_memory) = self.hot_tier_memory {
            cache_config.hot_tier.memory_limit_mb = hot_memory;
        }

        if let Some(warm_memory) = self.warm_tier_memory {
            cache_config.warm_tier.max_memory_bytes = (warm_memory as u64) * 1024 * 1024;
        }

        if let Some(cold_size) = self.cold_tier_size {
            cache_config.cold_tier.max_size_bytes = cold_size;
        }

        if let Some(compression) = self.compression_level {
            cache_config.cold_tier.compression_level = compression;
        }

        if let Some(workers) = self.workers {
            cache_config.worker.thread_pool_size = workers;
        }

        Ok((self, cache_config))
    }

    /// Load configuration from file
    fn load_config_file(path: &PathBuf) -> CliResult<ConfigFile> {
        let content = std::fs::read_to_string(path)?;

        // Determine format based on extension
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            Ok(serde_json::from_str(&content)?)
        } else {
            // Default to TOML
            Ok(toml::from_str(&content)?)
        }
    }

    /// Generate default configuration file
    pub fn generate_default_config() -> CliResult<ConfigFile> {
        Ok(ConfigFile {
            cache: CacheFileConfig {
                cache_id: Some("goldylox-default".to_string()),
                hot_tier: HotTierFileConfig {
                    max_entries: Some(10000),
                    memory_limit_mb: Some(64),
                },
                warm_tier: WarmTierFileConfig {
                    max_entries: Some(100000),
                    max_memory_bytes: Some(256 * 1024 * 1024), // 256MB
                },
                cold_tier: ColdTierFileConfig {
                    base_dir: Some("./goldylox-data".to_string()),
                    max_size_bytes: Some(1024 * 1024 * 1024), // 1GB
                    compression_level: Some(6),
                },
                worker: WorkerFileConfig {
                    thread_pool_size: Some(4),
                },
            },
            cli: CliFileConfig {
                output_format: Some("human".to_string()),
                verbose: Some(0),
                quiet: Some(false),
            },
        })
    }

    /// Save configuration to file
    pub fn save_config_file(config: &ConfigFile, path: &PathBuf) -> CliResult<()> {
        let content = if path.extension().and_then(|s| s.to_str()) == Some("json") {
            serde_json::to_string_pretty(config)?
        } else {
            toml::to_string_pretty(config).map_err(std::io::Error::other)?
        };

        std::fs::write(path, content)?;
        Ok(())
    }
}
