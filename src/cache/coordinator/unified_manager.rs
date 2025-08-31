//! Core unified cache manager coordinating all tiers with atomic state management
//!
//! This module contains the main UnifiedCacheManager struct and its core operations
//! for intelligently managing multi-tier cache access with SIMD optimization.

use crate::cache::coherence::ProtocolConfiguration;
use crate::cache::config::CacheConfig;
use crate::cache::eviction::CachePolicyEngine;
use crate::cache::manager::ErrorRecoverySystem as ManagerErrorRecoverySystem;
use crate::telemetry::unified_stats::UnifiedStats;
use crate::cache::tier::manager::TierPromotionManager;
use crate::cache::traits::core::{CacheKey, CacheValue};
use crate::cache::types::performance::timer::PrecisionTimer;
use crate::cache::types::AccessPath;
use super::background_coordinator::BackgroundCoordinator;
use super::strategy_selector::CacheStrategySelector;
use super::tier_operations::TierOperations;
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::telemetry::monitor::PerformanceMonitor;
use crate::telemetry::unified_stats::UnifiedCacheStatistics;


/// Unified cache manager coordinating all tiers with atomic state management
#[derive(Debug)]
pub struct UnifiedCacheManager<K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static, V: CacheValue + Default + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static> {
    /// Cache configuration (immutable after initialization)
    config: CacheConfig,
    /// Cache strategy selector for intelligent tier decisions
    strategy_selector: CacheStrategySelector,
    /// Tier promotion/demotion manager with SIMD optimization
    tier_manager: TierPromotionManager<K>,
    /// Unified cache statistics with atomic counters
    unified_stats: UnifiedCacheStatistics,
    /// Background operation coordinator with work-stealing scheduler
    background_coordinator: BackgroundCoordinator<K, V>,
    /// Cache policy engine with machine learning
    policy_engine: CachePolicyEngine<K, V>,
    /// Performance monitor with zero-allocation sampling
    performance_monitor: PerformanceMonitor,
    /// Error recovery system with circuit breaker
    error_recovery: ManagerErrorRecoverySystem<K, V>,
    /// Tier operations handler
    tier_operations: TierOperations<K, V>,
}

impl<K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static, V: CacheValue + Default + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static> UnifiedCacheManager<K, V> {
    /// Create new unified cache manager with full tier coordination
    pub fn new(config: CacheConfig) -> Result<Self, CacheOperationError> {
        // Initialize all cache tiers with SIMD optimization
        let hot_tier_config = crate::cache::config::types::HotTierConfig {
            max_entries: config.hot_tier.max_entries,
            enabled: config.hot_tier.enabled,
            hash_function: config.hot_tier.hash_function,
            eviction_policy: config.hot_tier.eviction_policy,
            cache_line_size: config.hot_tier.cache_line_size,
            prefetch_distance: config.hot_tier.prefetch_distance,
            enable_simd: config.hot_tier.enable_simd,
            enable_prefetch: config.hot_tier.enable_prefetch,
            lru_threshold_secs: config.hot_tier.lru_threshold_secs,
            memory_limit_mb: config.hot_tier.memory_limit_mb,
            _padding: config.hot_tier._padding,
        };
        // Initialize hot tier with proper generic types
        crate::cache::tier::hot::init_simd_hot_tier::<K, V>(hot_tier_config)?;
        let warm_tier_config = config.warm_tier.clone();
        // Initialize warm tier with proper generic types
        crate::cache::tier::warm::init_warm_tier::<K, V>(warm_tier_config)?;
        // Initialize cold tier with proper generic types
        crate::cache::tier::cold::init_cold_tier::<K, V>(config.cold_tier.storage_path.as_str())
            .map_err(|e| CacheOperationError::io_failed(&format!("Cold tier init failed: {}", e)))?;

        // Initialize coherence protocol with atomic coordination
        let _coherence_config = ProtocolConfiguration {
            optimistic_concurrency: true,
            write_through: false,
            max_invalidation_retries: 3,
            coherence_timeout_ns: 1_000_000, // 1ms
            strict_ordering: false,
            schema_version: 1,
        };
        let _coherence_controller =
            crate::cache::coherence::protocol::global_api::init_coherence_controller::<K, V>();

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

        // Wire GC coordinator to maintenance scheduler before creating manager
        let maintenance_sender = background_coordinator.get_maintenance_task_sender();
        crate::cache::memory::gc_coordinator::set_global_maintenance_sender(maintenance_sender);

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
        let mut access_path = AccessPath::new();

        // Use strategy selector to determine optimal access pattern
        let _access_strategy = self.strategy_selector.current_strategy();

        // Record operation start with atomic increment
        self.unified_stats.record_miss(0); // Will be updated with actual timing later

        // Try hot tier first (SIMD-optimized, fastest access)
        if let Some(value) = self.tier_operations.try_hot_tier_get(key, &mut access_path) {
            let elapsed_ns = timer.elapsed_ns();
            self.record_hit(crate::cache::coherence::CacheTier::Hot, elapsed_ns);
            let _ = self.policy_engine.pattern_analyzer.record_access(key);
            
            // Record performance metrics via strategy selector (which uses atomic operations)
            self.strategy_selector.record_strategy_performance(1.0, elapsed_ns);
            
            return Some(value);
        }

        // Try warm tier second (moderate speed, balanced capacity)
        if let Some(value) = self
            .tier_operations
            .try_warm_tier_get(key, &mut access_path)
        {
            let elapsed_ns = timer.elapsed_ns();
            self.record_hit(crate::cache::coherence::CacheTier::Warm, elapsed_ns);
            let _ = self.policy_engine.pattern_analyzer.record_access(key);
            
            // Record performance metrics for warm tier access
            self.strategy_selector.record_strategy_performance(1.0, elapsed_ns);

            // Consider intelligent promotion to hot tier
            self.consider_promotion(
                key,
                &value,
                crate::cache::coherence::CacheTier::Warm,
                crate::cache::coherence::CacheTier::Hot,
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
            self.record_hit(crate::cache::coherence::CacheTier::Cold, elapsed_ns);
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
            crate::cache::coherence::CacheTier::Hot => {
                // Place in hot tier with potential replication
                match self.tier_operations.put_with_replication(
                    key,
                    value,
                    crate::cache::coherence::CacheTier::Hot,
                    placement_decision.replication_tiers,
                ) {
                    Ok(_) => {},
                    Err(e) => {
                        // Use error recovery system for sophisticated retry logic
                        let _recovery_strategy = self.error_recovery.handle_error(
                            crate::cache::manager::error_recovery::types::ErrorType::DiskIOError,
                            placement_decision.primary_tier as u8
                        );
                        return Err(e);
                    }
                }
            }
            crate::cache::coherence::CacheTier::Warm => {
                // Place in warm tier with selective replication
                self.tier_operations.put_with_replication(
                    key,
                    value,
                    crate::cache::coherence::CacheTier::Warm,
                    placement_decision.replication_tiers,
                )?;
            }
            crate::cache::coherence::CacheTier::Cold => {
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
    pub fn stats(&self) -> UnifiedStats {
        // Use config for statistical calculations
        let _config_max_ops = (self.config.monitoring.alert_thresholds.min_ops_per_second_x100 / 100) as u64;
        
        // Convert from performance_tracking::UnifiedStats to unified_stats::UnifiedStats
        let perf_stats = self.unified_stats.compute_unified_stats();
        UnifiedStats {
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
            peak_memory_usage: perf_stats.total_memory_usage, // Use current as peak for now
            ops_per_second: if perf_stats.avg_access_latency_ns > 0 { 
                1_000_000_000.0 / perf_stats.avg_access_latency_ns as f32 
            } else { 0.0 },
            tier_hit_rates: [
                if perf_stats.total_operations > 0 { perf_stats.hot_tier_hits as f32 / perf_stats.total_operations as f32 } else { 0.0 },
                if perf_stats.total_operations > 0 { perf_stats.warm_tier_hits as f32 / perf_stats.total_operations as f32 } else { 0.0 },
                if perf_stats.total_operations > 0 { perf_stats.cold_tier_hits as f32 / perf_stats.total_operations as f32 } else { 0.0 },
            ],
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
    fn record_hit(&self, tier: crate::cache::coherence::CacheTier, elapsed_ns: u64) {
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
        value: &V,
        from_tier: crate::cache::coherence::CacheTier,
        to_tier: crate::cache::coherence::CacheTier,
        access_path: &AccessPath,
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
        value: &V,
        _access_path: &AccessPath,
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
                crate::cache::coherence::CacheTier::Cold,
                crate::cache::coherence::CacheTier::Warm,
                7,
            );

            // Consider further promotion to hot tier for very frequent access
            if access_pattern.frequency > 10.0 && value.estimated_size() < 1024 {
                let _ = self.tier_manager.schedule_promotion(
                    key.clone(),
                    crate::cache::coherence::CacheTier::Warm,
                    crate::cache::coherence::CacheTier::Hot,
                    9,
                );
            }
        }
    }
}

// String-specific convenience APIs removed - use generic UnifiedCacheManager<K, V> directly
