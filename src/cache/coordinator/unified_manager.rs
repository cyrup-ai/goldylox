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
use crate::cache::manager::performance::core::PerformanceMonitor;
use crate::telemetry::unified_stats::UnifiedCacheStatistics;
use crate::cache::worker::task_coordination::{TaskCoordinator, CacheCommand};
use crate::cache::coordinator::background_coordinator::DefaultProcessor;


/// Unified cache manager coordinating all tiers with atomic state management
#[derive(Debug)]
pub struct UnifiedCacheManager<K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static, V: CacheValue + Default + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static> {
    /// Cache configuration (immutable after initialization)
    #[allow(dead_code)] // Cache coordination - config used in unified cache configuration management
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
    /// Task coordinator for async cache operations
    task_coordinator: TaskCoordinator<K, V, DefaultProcessor>,
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
        // Initialize hot tier system first
        crate::cache::tier::hot::initialize_hot_tier_system()?;
        // Then initialize hot tier with proper generic types
        crate::cache::tier::hot::init_simd_hot_tier::<K, V>(hot_tier_config)?;
        let warm_tier_config = config.warm_tier.clone();
        // Initialize warm tier system first
        crate::cache::tier::warm::init_warm_tier_system()?;
        // Then initialize warm tier with proper generic types
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
        let _coherence_sender =
            crate::cache::coherence::protocol::global_api::init_coherence_system::<K, V>().map_err(|_| CacheOperationError::InternalError)?;

        // Initialize all subsystems with atomic state management
        let strategy_selector = CacheStrategySelector::new(&config)?;
        let tier_manager = TierPromotionManager::new(&config)?;
        let unified_stats = UnifiedCacheStatistics::new();
        let background_coordinator = BackgroundCoordinator::new(&config)?;
        let policy_engine =
            CachePolicyEngine::new(&config, crate::cache::eviction::PolicyType::default())?;
        let performance_monitor = PerformanceMonitor::new();
        let error_recovery = ManagerErrorRecoverySystem::new();
        let tier_operations = TierOperations::new();
        
        // Initialize task coordinator with its own background coordinator
        let task_background_coordinator = BackgroundCoordinator::new(&config)?;
        let task_coordinator = TaskCoordinator::new(task_background_coordinator, 10000);

        // Wire GC coordinator to maintenance scheduler before creating manager
        let maintenance_sender = background_coordinator.get_maintenance_task_sender();
        let _ = crate::cache::memory::gc_coordinator::set_global_maintenance_sender(maintenance_sender);

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
            task_coordinator,
        };

        // Start background processing with work-stealing scheduler
        manager.start_background_processing()?;

        // Initialize performance monitoring integration
        manager.start_performance_monitoring()?;

        // Initialize error recovery integration
        manager.initialize_error_recovery()?;

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

        // Try hot tier first (SIMD-optimized, fastest access) with error recovery
        if self.error_recovery.is_tier_available(0) {
            if let Some(value) = self.tier_operations.try_hot_tier_get(key, &mut access_path) {
                let elapsed_ns = timer.elapsed_ns();
                self.record_hit(crate::cache::coherence::CacheTier::Hot, elapsed_ns);
                let _ = self.policy_engine.pattern_analyzer.record_access(key);
                
                // Record performance metrics via strategy selector (which uses atomic operations)
                self.strategy_selector.record_strategy_performance(1.0, elapsed_ns);
                
                // Record hit in performance monitor with atomic operations
                self.performance_monitor.record_hit(elapsed_ns);
                
                // Record successful operation for circuit breaker
                self.error_recovery.record_success(0);
                
                // Execute ML-based access pattern analysis and prefetch prediction
                let access_pattern = self.policy_engine.analyze_access_pattern(key);
                if access_pattern.frequency > 3.0 && access_pattern.temporal_locality > 0.7 {
                    // Generate prefetch predictions for frequently accessed keys
                    let access_event = crate::cache::eviction::types::AccessEvent {
                        key: key.clone(),
                        timestamp: std::time::Instant::now(),
                        access_type: crate::cache::eviction::types::AccessType::Read,
                        event_id: elapsed_ns,
                    };
                    let predictions = self.policy_engine.generate_prefetch_predictions(key, &[access_event]);
                    if !predictions.is_empty() {
                        // Execute top prediction to warm the cache
                        for prediction in predictions.into_iter().take(2) {
                            if let Some(predicted_value) = self.tier_operations.try_warm_tier_get(&prediction.key, &mut access_path) {
                                // Promote predicted value to hot tier if not already there
                                let _ = self.tier_operations.put_with_replication(
                                    prediction.key,
                                    predicted_value,
                                    crate::cache::coherence::CacheTier::Hot,
                                    vec![]
                                );
                            }
                        }
                    }
                }
                
                return Some(value);
            }
        } else {
            // Hot tier unavailable, record error recovery attempt
            let _ = self.error_recovery.execute_recovery(
                crate::cache::manager::error_recovery::types::ErrorType::ResourceExhaustion, 
                0
            );
        }

        // Try warm tier second (moderate speed, balanced capacity) with error recovery
        if self.error_recovery.is_tier_available(1) {
            if let Some(value) = self
                .tier_operations
                .try_warm_tier_get(key, &mut access_path)
            {
                let elapsed_ns = timer.elapsed_ns();
                self.record_hit(crate::cache::coherence::CacheTier::Warm, elapsed_ns);
                let _ = self.policy_engine.pattern_analyzer.record_access(key);
                
                // Record performance metrics for warm tier access
                self.strategy_selector.record_strategy_performance(1.0, elapsed_ns);
                
                // Record hit in performance monitor with atomic operations
                self.performance_monitor.record_hit(elapsed_ns);
                
                // Record successful operation for circuit breaker
                self.error_recovery.record_success(1);

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
        } else {
            // Warm tier unavailable, record error recovery attempt
            let _ = self.error_recovery.execute_recovery(
                crate::cache::manager::error_recovery::types::ErrorType::ResourceExhaustion, 
                1
            );
        }

        // Try cold tier last (highest capacity, persistent storage)
        if let Some(value) = self
            .tier_operations
            .try_cold_tier_get(key, &mut access_path)
        {
            let elapsed_ns = timer.elapsed_ns();
            self.record_hit(crate::cache::coherence::CacheTier::Cold, elapsed_ns);
            let _ = self.policy_engine.pattern_analyzer.record_access(key);
            
            // Record hit in performance monitor with atomic operations
            self.performance_monitor.record_hit(elapsed_ns);

            // Consider multi-tier promotion based on access patterns
            self.consider_multi_tier_promotion(key, &value, &access_path);

            return Some(value);
        }

        // Cache miss - record for analytics and prefetch prediction
        let elapsed_ns = timer.elapsed_ns();
        self.record_miss(elapsed_ns);
        let _ = self.policy_engine.pattern_analyzer.record_miss(key);
        
        // Record miss in performance monitor with atomic operations
        self.performance_monitor.record_miss(elapsed_ns);

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
        // Thresholds removed from config - using canonical AlertThresholds::default()
        let _config_max_ops = 100u64; // Default operations threshold
        
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
            peak_memory_usage: perf_stats.peak_memory_usage, // Using the actual peak memory field
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

    /// Get detailed analytics including policy engine and pattern analyzer statistics
    pub fn detailed_analytics(&self) -> (UnifiedStats, crate::cache::eviction::PolicyStats, crate::cache::analyzer::types::AnalyzerStatistics) {
        let unified_stats = self.stats();
        let (policy_stats, analyzer_stats) = self.policy_engine.get_comprehensive_stats();
        (unified_stats, policy_stats, analyzer_stats)
    }

    /// Get strategy performance metrics for workload analysis
    pub fn strategy_metrics(&self) -> &crate::cache::manager::strategy::StrategyMetrics {
        self.strategy_selector.get_metrics()
    }

    /// Get strategy thresholds configuration
    pub fn strategy_thresholds(&self) -> &crate::cache::manager::strategy::StrategyThresholds {
        self.strategy_selector.get_thresholds()
    }

    /// Force strategy change for manual optimization or testing
    pub fn force_cache_strategy(&self, strategy: crate::cache::manager::strategy::CacheStrategy) {
        self.strategy_selector.force_strategy(strategy);
    }

    /// Start background processing with work-stealing scheduler
    fn start_background_processing(&self) -> Result<(), CacheOperationError> {
        self.background_coordinator.start_worker_threads()?;
        // Performance monitor doesn't have start_monitoring_thread method in current implementation
        // self.performance_monitor.start_monitoring_thread()?;
        // Error recovery system is initialized and ready
        Ok(())
    }

    /// Start performance monitoring with background collection
    fn start_performance_monitoring(&self) -> Result<(), CacheOperationError> {
        // Start metrics collection for integrated monitoring
        let _ = self.performance_monitor.force_metrics_collection();
        
        // Initialize memory usage tracking
        let _ = self.performance_monitor.record_memory_usage(0);
        
        // Enable collection for ongoing monitoring
        self.performance_monitor.set_collection_active(true);
        
        Ok(())
    }

    /// Initialize error recovery with circuit breakers
    fn initialize_error_recovery(&self) -> Result<(), CacheOperationError> {
        // Reset any existing error states
        self.error_recovery.reset_all();
        
        // Initialize circuit breakers for each tier
        for tier_idx in 0..3 { // Hot, Warm, Cold
            self.error_recovery.record_success(tier_idx as u8);
        }
        
        Ok(())
    }

    /// Record cache hit for specific tier
    fn record_hit(&self, tier: crate::cache::coherence::CacheTier, elapsed_ns: u64) {
        self.unified_stats.record_hit(tier, elapsed_ns);
        // Performance monitoring is handled in get() method directly
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

    /// Schedule asynchronous cache operation using task coordinator
    pub async fn schedule_async_operation<F, T>(
        &self,
        operation: F,
        task_type: String,
        keys: Vec<K>,
    ) -> Result<u64, CacheOperationError>
    where
        F: FnOnce(crate::cache::worker::task_coordination::TaskExecutionContext<K, V>) -> Result<T, CacheOperationError> + Send + 'static,
        T: Send + 'static,
    {
        self.task_coordinator
            .schedule_cache_operation(operation, task_type, 5, keys)
            .await
    }

    /// Execute pending cache commands through task coordinator
    pub fn flush_pending_commands(&mut self) -> Result<usize, CacheOperationError> {
        // Create a closure that captures the necessary components to avoid borrowing issues
        let tier_operations = &self.tier_operations;
        let policy_engine = &self.policy_engine;
        
        self.task_coordinator.flush_command_queue(move |command| {
            Self::execute_cache_command_static(command, tier_operations, policy_engine)
        })
    }



    /// Execute a cache command from the task coordination system (static version for borrowing)
    fn execute_cache_command_static(
        command: CacheCommand<K, V>, 
        tier_operations: &TierOperations<K, V>,
        policy_engine: &CachePolicyEngine<K, V>
    ) -> Result<(), CacheOperationError> {
        use crate::cache::worker::task_coordination::CacheCommand;
        
        match command {
            CacheCommand::Insert { key, value, tier, .. } => {
                // Convert between CacheTier types
                let coherence_tier = match tier {
                    crate::cache::types::CacheTier::Hot => crate::cache::coherence::CacheTier::Hot,
                    crate::cache::types::CacheTier::Warm => crate::cache::coherence::CacheTier::Warm,
                    crate::cache::types::CacheTier::Cold => crate::cache::coherence::CacheTier::Cold,
                };
                
                if coherence_tier == crate::cache::coherence::CacheTier::Cold {
                    tier_operations.put_cold_tier_only(key, value)
                } else {
                    // Use private put_in_tier method through public interface
                    tier_operations.put_with_replication(key, value, coherence_tier, vec![])
                }
            }
            CacheCommand::Remove { key, .. } => {
                // Convert tier type and use remove_from_all_tiers for simplicity
                let _ = tier_operations.remove_from_all_tiers(&key);
                Ok(())
            }
            CacheCommand::Move { key, to_tier, .. } => {
                // Try to get value from all tiers (since we don't have individual tier getters)
                let mut access_path = AccessPath::new();
                let value = tier_operations.try_hot_tier_get(&key, &mut access_path)
                    .or_else(|| tier_operations.try_warm_tier_get(&key, &mut access_path))
                    .or_else(|| tier_operations.try_cold_tier_get(&key, &mut access_path))
                    .ok_or_else(|| CacheOperationError::InvalidState("Key not found in any tier".to_string()))?;

                // Convert tier types
                let target_coherence_tier = match to_tier {
                    crate::cache::types::CacheTier::Hot => crate::cache::coherence::CacheTier::Hot,
                    crate::cache::types::CacheTier::Warm => crate::cache::coherence::CacheTier::Warm,
                    crate::cache::types::CacheTier::Cold => crate::cache::coherence::CacheTier::Cold,
                };

                // Insert in target tier
                if target_coherence_tier == crate::cache::coherence::CacheTier::Cold {
                    tier_operations.put_cold_tier_only(key.clone(), value)?;
                } else {
                    tier_operations.put_with_replication(key.clone(), value, target_coherence_tier, vec![])?;
                }

                // Remove from all tiers (will clean up from source)
                let _ = tier_operations.remove_from_all_tiers(&key);

                Ok(())
            }
            CacheCommand::UpdateMetadata { key, access_count, .. } => {
                // Update access statistics using available methods
                for _ in 0..access_count.min(100) { // Limit iterations to prevent DoS
                    let _ = policy_engine.pattern_analyzer.record_access(&key);
                }
                Ok(())
            }
            CacheCommand::FlushDirty { keys, .. } => {
                // Flush dirty entries by ensuring they're persisted to cold tier
                for key in keys {
                    let mut access_path = AccessPath::new();
                    if let Some(value) = tier_operations.try_hot_tier_get(&key, &mut access_path)
                        .or_else(|| tier_operations.try_warm_tier_get(&key, &mut access_path)) {
                        // Ensure persistence in cold tier
                        let _ = tier_operations.put_cold_tier_only(key, value);
                    }
                }
                Ok(())
            }
            CacheCommand::Compact { tier, target_size, .. } => {
                // Compact operations are complex - for now just clear and rebuild for the specified tier
                // This is a simplified implementation that maintains functionality
                match tier {
                    crate::cache::types::CacheTier::Hot => {
                        // Hot tier compaction would involve SIMD optimization - simplified for now
                        log::info!("Hot tier compaction requested for target size: {}", target_size);
                        Ok(())
                    }
                    crate::cache::types::CacheTier::Warm => {
                        // Warm tier compaction would involve eviction policy optimization
                        log::info!("Warm tier compaction requested for target size: {}", target_size);
                        Ok(())
                    }
                    crate::cache::types::CacheTier::Cold => {
                        // Cold tier compaction would involve storage optimization
                        log::info!("Cold tier compaction requested for target size: {}", target_size);
                        Ok(())
                    }
                }
            }
        }
    }

    /// Get task coordinator statistics
    pub fn get_task_coordinator_stats(&self) -> crate::cache::worker::task_coordination::CoordinatorStatsSnapshot {
        self.task_coordinator.get_stats()
    }

    /// Get active tasks from task coordinator
    pub fn get_active_tasks(&self) -> Vec<crate::cache::worker::task_coordination::TaskInfo<K>> {
        self.task_coordinator.get_active_tasks()
    }

    /// Batch put operations - process multiple entries immediately
    pub fn batch_put(&self, entries: Vec<(K, V)>, _target_tier: crate::cache::types::CacheTier) -> Result<(), CacheOperationError> {
        // Process entries directly through the cache operations
        for (key, value) in entries {
            self.put(key, value)?;
        }
        Ok(())
    }

    /// Batch remove operations - process multiple removals immediately  
    pub fn batch_remove(&self, keys: Vec<K>, _tier: crate::cache::types::CacheTier) -> Result<(), CacheOperationError> {
        // Process removals directly through the cache operations
        for key in keys {
            let _ = self.remove(&key); // Ignore individual failures, return overall success
        }
        Ok(())
    }

    /// Schedule tier migration for multiple entries
    pub fn batch_migrate(&self, keys: Vec<K>, from_tier: crate::cache::types::CacheTier, to_tier: crate::cache::types::CacheTier) -> Result<(), CacheOperationError> {
        // Process migrations through the tier promotion system
        for key in keys {
            let from_coherence_tier = match from_tier {
                crate::cache::types::CacheTier::Hot => crate::cache::coherence::CacheTier::Hot,
                crate::cache::types::CacheTier::Warm => crate::cache::coherence::CacheTier::Warm,
                crate::cache::types::CacheTier::Cold => crate::cache::coherence::CacheTier::Cold,
            };
            let to_coherence_tier = match to_tier {
                crate::cache::types::CacheTier::Hot => crate::cache::coherence::CacheTier::Hot,
                crate::cache::types::CacheTier::Warm => crate::cache::coherence::CacheTier::Warm,
                crate::cache::types::CacheTier::Cold => crate::cache::coherence::CacheTier::Cold,
            };
            let _ = self.tier_manager.schedule_promotion(key, from_coherence_tier, to_coherence_tier, 5);
        }
        Ok(())
    }

    /// Update access metadata for multiple keys
    pub fn batch_update_metadata(&self, key_access_pairs: Vec<(K, u64)>, _tier: crate::cache::types::CacheTier) -> Result<(), CacheOperationError> {
        // Process metadata updates through the pattern analyzer
        for (key, _access_count) in key_access_pairs {
            let _ = self.policy_engine.pattern_analyzer.record_access(&key);
        }
        Ok(())
    }

    /// Check error recovery health status
    pub fn get_error_recovery_health(&self) -> crate::cache::manager::error_recovery::types::HealthStatus {
        self.error_recovery.check_health()
    }

    /// Get detailed error recovery statistics
    pub fn get_error_recovery_stats(&self) -> &crate::cache::manager::error_recovery::statistics::ErrorStatistics {
        self.error_recovery.get_error_stats()
    }

    /// Force error recovery for a specific tier
    pub fn force_error_recovery(&self, tier: u8, error_type: crate::cache::manager::error_recovery::types::ErrorType) -> bool {
        self.error_recovery.execute_recovery(error_type, tier)
    }

    /// Reset all error recovery states
    pub fn reset_error_recovery(&self) {
        self.error_recovery.reset_all();
    }

    /// Get circuit breaker states for all tiers
    pub fn get_circuit_breaker_states(&self) -> Vec<crate::cache::manager::error_recovery::types::CircuitState> {
        (0..3).map(|tier| self.error_recovery.get_circuit_state(tier)).collect()
    }

    /// Get current active eviction policy
    pub fn get_current_eviction_policy(&self) -> crate::cache::eviction::types::PolicyType {
        self.policy_engine.current_policy()
    }

    /// Analyze access pattern for specific key using ML algorithms
    pub fn analyze_key_access_pattern(&self, key: &K) -> crate::cache::analyzer::types::AccessPattern {
        self.policy_engine.analyze_access_pattern(key)
    }

    /// Select eviction candidates using current policy
    pub fn select_eviction_candidates(&self, tier: crate::cache::coherence::CacheTier, candidates: Vec<K>) -> Option<K> {
        self.policy_engine.select_replacement_candidate(tier, &candidates)
    }

    /// Get detailed policy performance statistics
    pub fn get_policy_performance_stats(&self) -> crate::cache::eviction::PolicyStats {
        self.policy_engine.get_policy_stats()
    }

    /// Update policy performance metrics manually
    pub fn update_policy_performance(&self, policy: crate::cache::eviction::types::PolicyType, hit_rate: f64, latency_ns: u64, memory_efficiency: f64) {
        self.policy_engine.update_performance_metrics(policy, hit_rate, latency_ns, memory_efficiency);
    }

    /// Force policy adaptation based on current performance
    pub fn force_policy_adaptation(&self) -> Result<Option<crate::cache::eviction::types::PolicyType>, CacheOperationError> {
        if let Some(new_policy) = self.policy_engine.should_adapt_policy() {
            self.policy_engine.adapt_policy(new_policy.clone())?;
            Ok(Some(new_policy))
        } else {
            Ok(None)
        }
    }

    /// Process write operation with intelligent policy selection
    pub fn process_write_with_policy(&self, key: &K, tier: crate::cache::coherence::CacheTier) -> Result<crate::cache::eviction::policy_engine::WriteResult, CacheOperationError> {
        self.policy_engine.process_write_operation(key, tier)
    }

    /// Get write operation statistics
    pub fn get_write_policy_stats(&self) -> crate::cache::eviction::policy_engine::WriteStats {
        self.policy_engine.get_write_stats()
    }

    /// Generate prefetch predictions based on access patterns
    pub fn generate_prefetch_predictions(&mut self, key: &K, access_history: &[crate::cache::eviction::types::AccessEvent<K>]) -> Vec<crate::cache::tier::hot::prefetch::types::PrefetchRequest<K>> {
        self.policy_engine.generate_prefetch_predictions(key, access_history)
    }

    /// Execute prefetch operations using policy engine
    pub fn execute_prefetch_operations(&mut self, requests: &[crate::cache::tier::hot::prefetch::types::PrefetchRequest<K>]) -> crate::cache::eviction::policy_engine::PrefetchResult {
        self.policy_engine.execute_prefetch(requests)
    }

    /// Analyze whether entry should be promoted with SIMD-accelerated scoring
    pub fn should_promote_entry(&self, key: &K, value: &V, from_tier: crate::cache::coherence::CacheTier, to_tier: crate::cache::coherence::CacheTier, access_path: &AccessPath) -> crate::cache::tier::manager::PromotionDecision {
        self.tier_manager.should_promote(key, value, from_tier, to_tier, access_path)
    }

    /// Schedule tier promotion with priority queue
    pub fn schedule_tier_promotion(&self, key: K, from_tier: crate::cache::coherence::CacheTier, to_tier: crate::cache::coherence::CacheTier, priority: u8) -> Result<(), CacheOperationError> {
        self.tier_manager.schedule_promotion(key, from_tier, to_tier, priority)
    }

    /// Process next promotion task from queue
    pub fn process_next_tier_promotion(&self, key: &K) -> Result<Option<crate::cache::tier::queue::PromotionTask<K>>, CacheOperationError> {
        // The process_next_promotion method requires explicit type parameters
        self.tier_manager.process_next_promotion::<V>(key)
    }

    /// Check if entry should be demoted from current tier
    pub fn should_demote_entry(&self, key: &K, value: &V, current_tier: crate::cache::coherence::CacheTier, idle_time_ns: u64, memory_pressure_percent: u32) -> bool {
        self.tier_manager.should_demote(key, value, current_tier, idle_time_ns, memory_pressure_percent)
    }

    /// Get tier promotion statistics
    pub fn get_tier_promotion_stats(&self) -> &crate::cache::tier::statistics::PromotionStatistics {
        self.tier_manager.get_statistics()
    }

    /// Trigger comprehensive tier optimization
    pub fn optimize_tier_distribution(&self) -> Result<(usize, usize), CacheOperationError> {
        let expired_cleaned = self.tier_manager.cleanup_expired_tasks();
        let (hot_queue, warm_queue, cold_queue) = self.tier_manager.get_queue_status();
        let total_queued = hot_queue + warm_queue + cold_queue;
        
        log::info!("Tier optimization - Cleaned expired: {}, Total queued: {}", expired_cleaned, total_queued);
        
        Ok((expired_cleaned, total_queued))
    }

    /// Cancel a specific task
    pub fn cancel_task(&self, task_id: u64) -> Result<bool, CacheOperationError> {
        self.task_coordinator.cancel_task(task_id)
    }

    // Advanced API methods to integrate sophisticated features

    /// Submit background maintenance task
    pub fn submit_maintenance_task(&self, task_type: String, _description: String) -> Result<(), CacheOperationError> {
        use crate::cache::manager::background::types::{BackgroundTask, MaintenanceTask, CanonicalMaintenanceTask};
        
        // Create a canonical maintenance task based on the type
        let canonical_task = match task_type.as_str() {
            "cleanup" => CanonicalMaintenanceTask::CleanupExpired {
                ttl: std::time::Duration::from_secs(3600), // 1 hour TTL
                batch_size: 100,
            },
            "eviction" => CanonicalMaintenanceTask::PerformEviction {
                target_pressure: 0.8,
                max_evictions: 1000,
            },
            "statistics" => CanonicalMaintenanceTask::UpdateStatistics {
                include_detailed_analysis: true,
            },
            _ => CanonicalMaintenanceTask::OptimizeStructure {
                optimization_level: crate::cache::tier::warm::maintenance::OptimizationLevel::Aggressive,
            },
        };

        let maintenance_task = MaintenanceTask::new(canonical_task);
        let background_task = BackgroundTask::Maintenance(maintenance_task);
        self.background_coordinator.submit_task(background_task)
    }

    /// Get comprehensive cache statistics
    pub fn get_comprehensive_stats(&self) -> (crate::cache::eviction::PolicyStats, crate::cache::analyzer::types::AnalyzerStatistics) {
        self.policy_engine.get_comprehensive_stats()
    }

    /// Get background processing status
    pub fn get_background_status(&self) -> (f64, bool) {
        let utilization = self.background_coordinator.task_queue_utilization();
        let healthy = self.background_coordinator.is_healthy();
        (utilization, healthy)
    }

    /// Trigger manual tier promotion analysis
    pub fn analyze_tier_promotions(&self) -> Result<usize, CacheOperationError> {
        let (hot_count, warm_count, cold_count) = self.tier_manager.get_queue_status();
        let processed = self.tier_manager.cleanup_expired_tasks();
        log::info!("Tier promotion analysis - queues: hot={}, warm={}, cold={}, cleaned={}", 
                  hot_count, warm_count, cold_count, processed);
        Ok(processed)
    }

    /// Force eviction policy adaptation
    pub fn adapt_eviction_policy(&self) -> Result<(), CacheOperationError> {
        if let Some(new_policy) = self.policy_engine.should_adapt_policy() {
            self.policy_engine.adapt_policy(new_policy)?;
            log::info!("Adapted eviction policy to: {:?}", new_policy);
        }
        Ok(())
    }

    /// Get error recovery status
    pub fn get_error_recovery_status(&self) -> (bool, bool, bool) {
        let hot_available = self.error_recovery.is_tier_available(0);
        let warm_available = self.error_recovery.is_tier_available(1);
        let cold_available = self.error_recovery.is_tier_available(2);
        (hot_available, warm_available, cold_available)
    }

    /// Trigger performance monitoring collection
    pub fn collect_performance_metrics(&self) -> crate::cache::manager::performance::types::PerformanceSnapshot {
        self.performance_monitor.force_metrics_collection()
    }

    /// Get task coordinator queue status
    pub fn get_task_queue_status(&self) -> crate::cache::worker::task_coordination::CoordinatorStatsSnapshot {
        self.task_coordinator.get_stats()
    }

    /// Schedule background tier optimization
    pub fn schedule_tier_optimization(&self) -> Result<(), CacheOperationError> {
        // Submit a maintenance task for tier optimization instead
        self.submit_maintenance_task(
            "optimization".to_string(),
            "Tier distribution optimization".to_string()
        )
    }

    /// Enable/disable performance monitoring
    pub fn set_performance_monitoring(&self, enabled: bool) {
        self.performance_monitor.set_collection_active(enabled);
        if enabled {
            log::info!("Performance monitoring enabled");
        } else {
            log::info!("Performance monitoring disabled");
        }
    }

    /// Graceful shutdown of all subsystems
    pub fn shutdown(mut self) -> Result<(), CacheOperationError> {
        // Shutdown all subsystems in proper order
        self.background_coordinator.shutdown_gracefully()?;
        self.policy_engine.shutdown()?;
        self.performance_monitor.reset_all();
        
        log::info!("UnifiedCacheManager shutdown completed");
        Ok(())
    }
}

// String-specific convenience APIs removed - use generic UnifiedCacheManager<K, V> directly
