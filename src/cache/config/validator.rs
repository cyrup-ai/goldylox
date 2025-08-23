//! Fast configuration validation with zero allocation
//!
//! This module provides efficient validation of cache configurations
//! using stack-allocated error collections and inline validation.

use arrayvec::ArrayVec;

use super::types::{CacheConfig, ConfigError};

/// Fast configuration validation (stack allocated)
pub struct ConfigValidator;

impl ConfigValidator {
    /// Validate configuration (zero allocation)
    #[inline(always)]
    pub fn validate(config: &CacheConfig) -> Result<(), ArrayVec<ConfigError, 8>> {
        let mut errors = ArrayVec::new();

        // Validate hot tier
        if config.hot_tier.enabled && config.hot_tier.max_entries == 0 {
            let _ = errors.try_push(ConfigError::HotTierInvalid);
        }

        // Ensure power of 2
        if config.hot_tier.max_entries & (config.hot_tier.max_entries - 1) != 0 {
            let _ = errors.try_push(ConfigError::PowerOfTwoRequired);
        }

        // Validate warm tier
        if config.warm_tier.enabled && config.warm_tier.max_entries == 0 {
            let _ = errors.try_push(ConfigError::WarmTierInvalid);
        }

        // Ensure power of 2
        if config.warm_tier.max_entries & (config.warm_tier.max_entries - 1) != 0 {
            let _ = errors.try_push(ConfigError::PowerOfTwoRequired);
        }

        if config.warm_tier.entry_timeout_ns == 0 {
            let _ = errors.try_push(ConfigError::TimeoutInvalid);
        }

        // Validate cold tier
        if config.cold_tier.enabled && config.cold_tier.storage_path.is_empty() {
            let _ = errors.try_push(ConfigError::ColdTierPathRequired);
        }

        if config.cold_tier.compression_level > 9 {
            let _ = errors.try_push(ConfigError::CompressionLevelInvalid);
        }

        // Validate monitoring
        if config.monitoring.enabled && config.monitoring.sample_interval_ns == 0 {
            let _ = errors.try_push(ConfigError::MonitoringIntervalInvalid);
        }

        // Validate worker
        if config.worker.enabled && config.worker.thread_pool_size == 0 {
            let _ = errors.try_push(ConfigError::WorkerThreadsInvalid);
        }

        // Ensure power of 2
        if config.worker.task_queue_capacity & (config.worker.task_queue_capacity - 1) != 0 {
            let _ = errors.try_push(ConfigError::PowerOfTwoRequired);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Quick validation check (returns true if valid)
    #[inline(always)]
    pub fn is_valid(config: &CacheConfig) -> bool {
        Self::validate(config).is_ok()
    }

    /// Validate specific configuration section
    #[inline(always)]
    pub fn validate_hot_tier(config: &CacheConfig) -> Result<(), ConfigError> {
        if config.hot_tier.enabled && config.hot_tier.max_entries == 0 {
            return Err(ConfigError::HotTierInvalid);
        }

        if config.hot_tier.max_entries & (config.hot_tier.max_entries - 1) != 0 {
            return Err(ConfigError::PowerOfTwoRequired);
        }

        Ok(())
    }

    /// Validate warm tier configuration
    #[inline(always)]
    pub fn validate_warm_tier(config: &CacheConfig) -> Result<(), ConfigError> {
        if config.warm_tier.enabled && config.warm_tier.max_entries == 0 {
            return Err(ConfigError::WarmTierInvalid);
        }

        if config.warm_tier.max_entries & (config.warm_tier.max_entries - 1) != 0 {
            return Err(ConfigError::PowerOfTwoRequired);
        }

        if config.warm_tier.entry_timeout_ns == 0 {
            return Err(ConfigError::TimeoutInvalid);
        }

        Ok(())
    }

    /// Validate cold tier configuration
    #[inline(always)]
    pub fn validate_cold_tier(config: &CacheConfig) -> Result<(), ConfigError> {
        if config.cold_tier.enabled && config.cold_tier.storage_path.is_empty() {
            return Err(ConfigError::ColdTierPathRequired);
        }

        if config.cold_tier.compression_level > 9 {
            return Err(ConfigError::CompressionLevelInvalid);
        }

        Ok(())
    }

    /// Validate monitoring configuration
    #[inline(always)]
    pub fn validate_monitoring(config: &CacheConfig) -> Result<(), ConfigError> {
        if config.monitoring.enabled && config.monitoring.sample_interval_ns == 0 {
            return Err(ConfigError::MonitoringIntervalInvalid);
        }

        Ok(())
    }

    /// Validate worker configuration
    #[inline(always)]
    pub fn validate_worker(config: &CacheConfig) -> Result<(), ConfigError> {
        if config.worker.enabled && config.worker.thread_pool_size == 0 {
            return Err(ConfigError::WorkerThreadsInvalid);
        }

        if config.worker.task_queue_capacity & (config.worker.task_queue_capacity - 1) != 0 {
            return Err(ConfigError::PowerOfTwoRequired);
        }

        Ok(())
    }
}
