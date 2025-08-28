//! Tier configuration builder methods
//!
//! This module provides builder methods for configuring hot, warm, and cold tiers
//! of the cache system with fluent API patterns.

use arrayvec::ArrayString;

use crate::cache::config::types::ConfigError;
use crate::cache::tier::warm::config::SkipMapConfig;
use super::core::CacheConfigBuilder;

impl CacheConfigBuilder {
    /// Set hot tier capacity (must be power of 2)
    #[inline(always)]
    pub const fn hot_tier_capacity(mut self, capacity: u32) -> Self {
        self.config.hot_tier.max_entries = capacity;
        self
    }

    /// Enable or disable hot tier
    #[inline(always)]
    pub const fn hot_tier_enabled(mut self, enabled: bool) -> Self {
        self.config.hot_tier.enabled = enabled;
        self
    }

    /// Set cache line size for hot tier
    #[inline(always)]
    pub const fn cache_line_size(mut self, size: u8) -> Self {
        self.config.hot_tier.cache_line_size = size;
        self
    }

    /// Set prefetch distance for hot tier
    #[inline(always)]
    pub const fn prefetch_distance(mut self, distance: u8) -> Self {
        self.config.hot_tier.prefetch_distance = distance;
        self
    }

    /// Set warm tier capacity (must be power of 2)
    #[inline(always)]
    pub const fn warm_tier_capacity(mut self, capacity: u32) -> Self {
        self.config.warm_tier.max_entries = capacity as usize;
        self
    }

    /// Set warm tier timeout in nanoseconds
    #[inline(always)]
    pub const fn warm_tier_timeout_ns(mut self, timeout_ns: u64) -> Self {
        self.config.warm_tier.default_ttl_sec = timeout_ns / 1_000_000_000;
        self
    }

    /// Enable or disable warm tier
    #[inline(always)]
    pub const fn warm_tier_enabled(mut self, enabled: bool) -> Self {
        self.config.warm_tier.enabled = enabled;
        self
    }

    /// Set skip map configuration for warm tier
    #[inline(always)]
    pub const fn skip_map_config(mut self, config: SkipMapConfig) -> Self {
        self.config.warm_tier.skip_map = config;
        self
    }

    /// Set promotion threshold for warm tier
    #[inline(always)]
    pub const fn promotion_threshold(mut self, threshold: u16) -> Self {
        self.config.warm_tier.promotion_threshold = threshold;
        self
    }

    /// Set demotion age threshold in nanoseconds
    #[inline(always)]
    pub const fn demotion_age_threshold_ns(mut self, threshold_ns: u64) -> Self {
        self.config.warm_tier.demotion_age_threshold_ns = threshold_ns;
        self
    }


    /// Enable cold tier with storage path (fixed-size string)
    #[inline(always)]
    pub fn cold_tier_storage(mut self, path: &str) -> Result<Self, ConfigError> {
        self.config.cold_tier.enabled = true;
        self.config.cold_tier.storage_path = ArrayString::from(path)
            .map_err(|_| ConfigError::InvalidValue("Storage path too long".to_string()))?;
        Ok(self)
    }

    /// Set cold tier compression level (0-9)
    #[inline(always)]
    pub const fn cold_tier_compression(mut self, level: u8) -> Self {
        self.config.cold_tier.compression_level = if level > 9 { 9 } else { level };
        self
    }

    /// Set cold tier max file size
    #[inline(always)]
    pub const fn max_file_size(mut self, size: u64) -> Self {
        self.config.cold_tier.max_file_size = size;
        self
    }

    /// Enable or disable auto compaction
    #[inline(always)]
    pub const fn auto_compact(mut self, enabled: bool) -> Self {
        self.config.cold_tier.auto_compact = enabled;
        self
    }

    /// Set compact interval in nanoseconds
    #[inline(always)]
    pub const fn compact_interval_ns(mut self, interval_ns: u64) -> Self {
        self.config.cold_tier.compact_interval_ns = interval_ns;
        self
    }

    /// Set memory map size
    #[inline(always)]
    pub const fn mmap_size(mut self, size: u64) -> Self {
        self.config.cold_tier.mmap_size = size;
        self
    }

    /// Set write buffer size
    #[inline(always)]
    pub const fn write_buffer_size(mut self, size: u32) -> Self {
        self.config.cold_tier.write_buffer_size = size;
        self
    }
}
