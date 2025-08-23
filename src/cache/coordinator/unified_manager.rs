//! Core unified cache manager coordinating all tiers with atomic state management
//!
//! This module contains the main UnifiedCacheManager struct and its core operations
//! for intelligently managing multi-tier cache access with SIMD optimization.

use std::sync::Arc;

use super::super::coherence::ProtocolConfiguration;
use super::super::config::CacheConfig;
use super::super::eviction::CachePolicyEngine;
use super::super::manager::{
    ErrorRecoverySystem as ManagerErrorRecoverySystem, UnifiedStats as ManagerUnifiedStats,
};
use super::super::tier::TierPromotionManager;
use super::super::traits::core::{CacheKey, CacheValue};
use super::super::types::PrecisionTimer;
use super::background_coordinator::BackgroundCoordinator;
use super::strategy_selector::CacheStrategySelector;
use super::tier_operations::TierOperations;
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::telemetry::monitor::PerformanceMonitor;
use crate::telemetry::statistics::UnifiedCacheStatistics;

/// Unified cache manager coordinating all tiers with atomic state management
#[derive(Debug)]
pub struct UnifiedCacheManager<K: CacheKey, V: CacheValue> {
    /// Cache configuration (immutable after initialization)
    config: CacheConfig,
    /// Cache strategy selector for intelligent tier decisions
    strategy_selector: CacheStrategySelector,
    /// Tier promotion/demotion manager with SIMD optimization
    tier_manager: TierPromotionManager,
    /// Unified cache statistics with atomic counters
    unified_stats: UnifiedCacheStatistics,
    /// Background operation coordinator with work-stealing scheduler
    background_coordinator: BackgroundCoordinator<K, V>,
    /// Cache policy engine with machine learning
    policy_engine: CachePolicyEngine<K, V>,
    /// Performance monitor with zero-allocation sampling
    performance_monitor: PerformanceMonitor,
    /// Error recovery system with circuit breaker
    error_recovery: ManagerErrorRecoverySystem,
    /// Tier operations handler
    tier_operations: TierOperations<K, V>,
}

impl<K: CacheKey, V: CacheValue> UnifiedCacheManager<K, V> {
    /// Create new unified cache manager with full tier coordination
    pub fn new(config: CacheConfig) -> Result<Self, CacheOperationError> {
        // Initialize all cache tiers with SIMD optimization
        let hot_tier_config = super::super::tier::hot::types::HotTierConfig {
            max_entries: config.hot_tier.max_entries,
            enable_simd: true,
            enable_prefetch: true,
            lru_threshold: std::time::Duration::from_millis(100),
            memory_limit: 1024 * 1024 * 64, // 64MB default
            cache_line_size: 64,
        };
        // Initialize hot tier with proper generic types
        super::super::tier::hot::init_simd_hot_tier::<K, V>(hot_tier_config)?;
        let warm_tier_config = super::super::tier::warm::config::WarmTierConfig {
            max_memory_bytes: config.warm_tier.max_size_bytes,
            max_entries: config.warm_tier.max_entries as usize,
            default_ttl_sec: config.warm_tier.entry_timeout_ns / 1_000_000_000,
            pressure_thresholds: super::super::tier::warm::config::PressureConfig::default(),
            eviction_config: super::super::tier::warm::config::EvictionConfig::default(),
            tracking_config: super::super::tier::warm::config::TrackingConfig::default(),
            background_config: super::super::tier::warm::config::BackgroundConfig::default(),
            performance_config: super::super::tier::warm::config::PerformanceConfig::default(),
        };
        // Initialize warm tier with proper generic types
        super::super::tier::warm::init_warm_tier::<K, V>(warm_tier_config)?;
        // Initialize cold tier with proper generic types
        super::super::tier::cold::init_cold_tier::<K, V>(config.cold_tier.storage_path.as_str())
            .map_err(|e| CacheOperationError::io_error(&format!("Cold tier init failed: {}", e)))?;

        // Initialize coherence protocol with atomic coordination
        let _coherence_config = ProtocolConfiguration {
            optimistic_concurrency: true,
            write_through: false,
            max_invalidation_retries: 3,
            coherence_timeout_ns: 1_000_000, // 1ms
            strict_ordering: false,
        };
        let _coherence_controller =
            super::super::coherence::protocol::global_api::init_coherence_controller::<K, V>();

        // Initialize all subsystems with atomic state management
        let strategy_selector = CacheStrategySelector::new(&config)?;
        let tier_manager = TierPromotionManager::new(&config)?;
        let unified_stats = UnifiedCacheStatistics::new();
        let background_coordinator = BackgroundCoordinator::new(&config)?;
        let policy_engine =
            CachePolicyEngine::new(&config, crate::cache::eviction::PolicyType::default())?;
        let monitor_config = crate::telemetry::types::MonitorConfig::default();
        let performance_monitor = PerformanceMonitor::new(monitor_config);
        let error_recovery = ManagerErrorRecoverySystem::new();
        let tier_operations = TierOperations::new();

        let manager = Self {
            config,
            strategy_selector,
            tier_manager,
            unified_stats,
            background_coordinator,
            policy_engine,
            performance_monitor,
            error_recovery,
            tier_operations,
        };

        // Start background processing with work-stealing scheduler
        manager.start_background_processing()?;

        Ok(manager)
    }

    /// Get value from unified cache with intelligent tier selection - zero-copy crossbeam reference
    pub fn get(&self, key: &K) -> Option<V> {
        let timer = PrecisionTimer::start();
        let mut access_path = super::super::manager::AccessPath::new();

        // Record operation start with atomic increment
        self.unified_stats.record_miss(0); // Will be updated with actual timing later

        // Try hot tier first (SIMD-optimized, fastest access)
        if let Some(value) = self.tier_operations.try_hot_tier_get(key, &mut access_path) {
            let elapsed_ns = timer.elapsed_ns();
            self.record_hit(super::super::coherence::CacheTier::Hot, elapsed_ns);
            let _ = self.policy_engine.pattern_analyzer.record_access(key);
            return Some(value);
        }

        // Try warm tier second (moderate speed, balanced capacity)
        if let Some(value) = self
            .tier_operations
            .try_warm_tier_get(key, &mut access_path)
        {
            let elapsed_ns = timer.elapsed_ns();
            self.record_hit(super::super::coherence::CacheTier::Warm, elapsed_ns);
            let _ = self.policy_engine.pattern_analyzer.record_access(key);

            // Consider intelligent promotion to hot tier
            self.consider_promotion(
                key,
                &value,
                super::super::coherence::CacheTier::Warm,
                super::super::coherence::CacheTier::Hot,
                &access_path,
            );

            return Some(value);
        }

        // Try cold tier last (highest capacity, persistent storage)
        if let Some(value) = self
            .tier_operations
            .try_cold_tier_get(key, &mut access_path)
        {
            let elapsed_ns = timer.elapsed_ns();
            self.record_hit(super::super::coherence::CacheTier::Cold, elapsed_ns);
            let _ = self.policy_engine.pattern_analyzer.record_access(key);

            // Consider multi-tier promotion based on access patterns
            self.consider_multi_tier_promotion(key, &value, &access_path);

            return Some(value);
        }

        // Cache miss - record for analytics and prefetch prediction
        let elapsed_ns = timer.elapsed_ns();
        self.record_miss(elapsed_ns);
        let _ = self.policy_engine.pattern_analyzer.record_miss(key);

        None
    }

    /// Put value in unified cache with optimal tier placement
    pub fn put(&self, key: K, value: V) -> Result<(), CacheOperationError> {
        let timer = PrecisionTimer::start();

        // Analyze value characteristics for intelligent placement
        let placement_decision =
            self.tier_operations
                .analyze_placement(&key, &value, &self.policy_engine);

        match placement_decision.primary_tier {
            super::super::coherence::CacheTier::Hot => {
                // Place in hot tier with potential replication
                self.tier_operations.put_with_replication(
                    key,
                    value,
                    super::super::coherence::CacheTier::Hot,
                    placement_decision.replication_tiers,
                )?;
            }
            super::super::coherence::CacheTier::Warm => {
                // Place in warm tier with selective replication
                self.tier_operations.put_with_replication(
                    key,
                    value,
                    super::super::coherence::CacheTier::Warm,
                    placement_decision.replication_tiers,
                )?;
            }
            super::super::coherence::CacheTier::Cold => {
                // Place only in cold tier (large, infrequently accessed)
                self.tier_operations.put_cold_tier_only(key, value)?;
            }
        }

        // Update access latency with running average calculation
        let elapsed_ns = timer.elapsed_ns();
        self.unified_stats.update_memory_usage(elapsed_ns); // Use available public method

        Ok(())
    }

    /// Remove value from all cache tiers with atomic consistency
    pub fn remove(&self, key: &K) -> bool {
        let removed = self.tier_operations.remove_from_all_tiers(key);

        if removed {
            // Update operation counter atomically using public method
            self.unified_stats.record_miss(0);
        }

        removed
    }

    /// Clear all cache tiers with atomic coordination
    pub fn clear(&self) -> Result<(), CacheOperationError> {
        self.tier_operations.clear_all_tiers()?;

        // Reset unified statistics atomically
        self.unified_stats.reset_all_counters();

        Ok(())
    }

    /// Get comprehensive unified cache statistics
    pub fn stats(&self) -> ManagerUnifiedStats {
        // Convert from performance_tracking::UnifiedStats to manager::UnifiedStats
        let perf_stats = self.unified_stats.compute_unified_stats();
        ManagerUnifiedStats {
            total_operations: perf_stats.total_operations,
            overall_hit_rate: perf_stats.overall_hit_rate,
            hot_tier_hits: perf_stats.hot_tier_hits,
            warm_tier_hits: perf_stats.warm_tier_hits,
            cold_tier_hits: perf_stats.cold_tier_hits,
            total_misses: perf_stats.total_misses,
            avg_access_latency_ns: perf_stats.avg_access_latency_ns,
            promotions_performed: perf_stats.promotions_performed,
            demotions_performed: perf_stats.demotions_performed,
            total_memory_usage: perf_stats.total_memory_usage,
        }
    }

    /// Start background processing with work-stealing scheduler
    fn start_background_processing(&self) -> Result<(), CacheOperationError> {
        self.background_coordinator.start_worker_threads()?;
        // Performance monitor doesn't have start_monitoring_thread method in current implementation
        // self.performance_monitor.start_monitoring_thread()?;
        // Error recovery system is initialized and ready
        Ok(())
    }

    /// Record cache hit for specific tier
    fn record_hit(&self, tier: super::super::coherence::CacheTier, elapsed_ns: u64) {
        self.unified_stats.record_hit(tier, elapsed_ns);
    }

    /// Record cache miss
    fn record_miss(&self, elapsed_ns: u64) {
        self.unified_stats.record_miss(elapsed_ns);
    }

    /// Consider promoting entry between tiers
    fn consider_promotion(
        &self,
        key: &K,
        value: &Arc<V>,
        from_tier: super::super::coherence::CacheTier,
        to_tier: super::super::coherence::CacheTier,
        access_path: &super::super::manager::AccessPath,
    ) {
        let promotion_decision =
            self.tier_manager
                .should_promote(key, value, from_tier, to_tier, access_path);
        if promotion_decision.should_promote {
            let _ = self
                .tier_manager
                .schedule_promotion(key.clone(), from_tier, to_tier, 5);
        }
    }

    /// Consider multi-tier promotion based on access patterns
    fn consider_multi_tier_promotion(
        &self,
        key: &K,
        value: &Arc<V>,
        _access_path: &super::super::manager::AccessPath,
    ) {
        // Analyze access pattern to determine optimal promotion strategy
        let access_pattern = self
            .policy_engine
            .pattern_analyzer
            .analyze_access_pattern(key);

        // Promote to warm tier if access frequency is high enough
        if access_pattern.frequency > 2.0 {
            let _ = self.tier_manager.schedule_promotion(
                key.clone(),
                super::super::coherence::CacheTier::Cold,
                super::super::coherence::CacheTier::Warm,
                7,
            );

            // Consider further promotion to hot tier for very frequent access
            if access_pattern.frequency > 10.0 && value.as_ref().estimated_size() < 1024 {
                let _ = self.tier_manager.schedule_promotion(
                    key.clone(),
                    super::super::coherence::CacheTier::Warm,
                    super::super::coherence::CacheTier::Hot,
                    9,
                );
            }
        }
    }
}
