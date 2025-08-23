//! Initialization and constructor logic for unified cache manager
//!
//! This module implements the constructor and initialization logic for
//! the unified cache management system.

use super::super::super::config::CacheConfig;
use super::super::super::tier::hot::init_simd_hot_tier;
use super::super::super::traits::core::{CacheKey, CacheValue};
use crate::cache::background::coordinator::BackgroundCoordinator;
use super::super::error_recovery::ErrorRecoverySystem;
use super::super::performance::PerformanceMonitor;
use super::super::policy::engine::CachePolicyEngine;
use super::super::statistics::UnifiedCacheStatistics;
use super::super::strategy::CacheStrategySelector;
use super::super::tier_manager::TierPromotionManager;
use super::types::UnifiedCacheManager;
use crate::cache::traits::types_and_enums::CacheOperationError;

impl<K: CacheKey, V: CacheValue> UnifiedCacheManager<K, V> {
    /// Create new unified cache manager
    pub fn new(config: CacheConfig) -> Result<Self, CacheOperationError> {
        // Initialize all cache tiers
        // Convert config types and initialize hot tier
        let hot_tier_config = crate::cache::tier::hot::types::HotTierConfig::default();
        // Initialize hot tier with proper generic types
        crate::cache::tier::hot::thread_local::init_simd_hot_tier::<K, V>(hot_tier_config);
        
        // Initialize warm tier with existing sophisticated implementation
        crate::cache::tier::warm::api::global_functions::init_warm_tier::<K, V>(config.warm_tier.clone())?;
        
        // Initialize cold tier with existing persistent storage implementation  
        crate::cache::tier::cold::core::utilities::init_cold_tier::<K, V>(config.cold_tier.clone())?;

        // Initialize coherence protocol
        let _coherence_config = super::super::super::coherence::ProtocolConfiguration {
            optimistic_concurrency: true,
            write_through: false,
            max_invalidation_retries: 16,
            coherence_timeout_ns: 1_000_000, // 1ms
            strict_ordering: false,
        };
        let _coherence_controller =
            super::super::super::coherence::protocol::global_api::init_coherence_controller::<K, V>(
            );

        // Initialize all subsystems
        let strategy_selector = CacheStrategySelector::new();
        let tier_manager = TierPromotionManager::new();
        let unified_stats = UnifiedCacheStatistics::new();
        let background_coordinator = BackgroundCoordinator::new()?;
        let policy_engine =
            CachePolicyEngine::new(&config, crate::cache::eviction::PolicyType::default())?;
        let performance_monitor = PerformanceMonitor::new();
        let error_recovery = ErrorRecoverySystem::new();

        let manager = Self {
            config,
            strategy_selector,
            tier_manager,
            unified_stats,
            background_coordinator,
            policy_engine,
            performance_monitor,
            error_recovery,
            _phantom: std::marker::PhantomData,
        };

        // Start background processing
        manager.start_background_processing()?;

        Ok(manager)
    }

    /// Create cache manager with custom configuration
    pub fn with_config(config: CacheConfig) -> Result<Self, CacheOperationError> {
        Self::new(config)
    }

    /// Create cache manager with default configuration
    pub fn default() -> Result<Self, CacheOperationError> {
        let config = CacheConfig::default();
        Self::new(config)
    }

    /// Create cache manager optimized for high performance
    pub fn high_performance() -> Result<Self, CacheOperationError> {
        let config = CacheConfig::high_performance();
        Self::new(config)
    }

    /// Create cache manager optimized for low memory usage
    pub fn low_memory() -> Result<Self, CacheOperationError> {
        let config = CacheConfig::low_memory();
        Self::new(config)
    }

    /// Validate configuration before initialization
    pub fn validate_config(config: &CacheConfig) -> Result<(), CacheOperationError> {
        // Validate hot tier configuration
        if config.hot_tier.max_entries == 0 {
            return Err(CacheOperationError::InvalidConfiguration(
                "Hot tier max_entries must be greater than 0".to_string(),
            ));
        }

        // Validate warm tier configuration
        if config.warm_tier.max_size_bytes == 0 {
            return Err(CacheOperationError::InvalidConfiguration(
                "Warm tier max_size_bytes must be greater than 0".to_string(),
            ));
        }

        // Validate cold tier configuration
        if config.cold_tier.max_size_bytes == 0 {
            return Err(CacheOperationError::InvalidConfiguration(
                "Cold tier max_size_bytes must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }
}
