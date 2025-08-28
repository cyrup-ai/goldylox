//! Advanced configuration builder methods
//!
//! This module provides builder methods for advanced cache configuration options
//! including hash functions, eviction policies, and version management.

use crate::cache::config::types::HashFunction;
use crate::cache::tier::warm::eviction::types::EvictionPolicyType;
use super::core::CacheConfigBuilder;

impl CacheConfigBuilder {
    /// Set hash function
    #[inline(always)]
    pub const fn hash_function(mut self, hash_fn: HashFunction) -> Self {
        self.config.hot_tier.hash_function = hash_fn;
        self
    }

    /// Set eviction policy
    #[inline(always)]
    pub const fn eviction_policy(mut self, policy: EvictionPolicyType) -> Self {
        self.config.hot_tier.eviction_policy = policy;
        self
    }

    /// Set configuration version
    #[inline(always)]
    pub const fn version(mut self, version: u32) -> Self {
        self.config.version = version;
        self
    }
}
