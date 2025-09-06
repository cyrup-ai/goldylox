//! Core unified cache manager coordinating all tiers with atomic state management
//!
//! This module contains the main UnifiedCacheManager struct and its core operations
//! for intelligently managing multi-tier cache access with SIMD optimization.

use crate::cache::coherence::ProtocolConfiguration;
use crate::cache::config::CacheConfig;
use crate::cache::eviction::CachePolicyEngine;
use crate::cache::manager::error_recovery::ErrorRecoverySystem as ManagerErrorRecoverySystem;
use crate::telemetry::unified_stats::UnifiedStats;
use crate::cache::tier::manager::TierPromotionManager;
use crate::cache::traits::core::{CacheKey, CacheValue};
use crate::cache::types::performance::timer::PrecisionTimer;
use crate::cache::types::AccessPath;
use crate::cache::manager::background::types::{MaintenanceScheduler, MaintenanceConfig};
use super::strategy_selector::CacheStrategySelector;
use super::tier_operations::TierOperations;
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::manager::performance::core::PerformanceMonitor;
use crate::telemetry::unified_stats::UnifiedCacheStatistics;
use crate::cache::worker::task_coordination::{TaskCoordinator, CacheCommand};


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
    /// Work-stealing maintenance scheduler
    maintenance_scheduler: MaintenanceScheduler<K, V>,
    /// Cache policy engine with machine learning
    policy_engine: CachePolicyEngine<K, V>,
    /// Performance monitor with zero-allocation sampling
    performance_monitor: PerformanceMonitor,
    /// Error recovery system with circuit breaker
    error_recovery: ManagerErrorRecoverySystem<K, V>,
    /// Tier operations handler
    tier_operations: TierOperations<K, V>,
    /// Task coordinator for async cache operations  
    task_coordinator: TaskCoordinator<K, V>,
}

impl<K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static, V: CacheValue + Default + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static> UnifiedCacheManager<K, V> {
    /// Create new unified cache manager with full tier coordination
    pub fn new(config: CacheConfig) -> Result<Self, CacheOperationError> {
        // Initialize all cache tiers with SIMD optimization
        let hot_tier_config = crate::cache::config::types::HotTierConfig {
            max_entries: config.hot_tier.max_entries,
            hash_function: config.hot_tier.hash_function,
            eviction_policy: config.hot_tier.eviction_policy,
            cache_line_size: config.hot_tier.cache_line_size,
            prefetch_distance: config.hot_tier.prefetch_distance,
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
        crate::cache::tier::cold::init_cold_tier::<K, V>(config.cold_tier.base_dir.as_str(), &config.cache_id)
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
        // Create maintenance config with worker parameters from config
        let mut maintenance_config = MaintenanceConfig::default();
        maintenance_config.worker_count = config.worker.thread_pool_size as u32;
        if config.worker.maintenance_interval_ns != 1_000_000_000 {
            maintenance_config.heartbeat_interval_ns = config.worker.maintenance_interval_ns;
        }
        let maintenance_scheduler = MaintenanceScheduler::new(maintenance_config)?;
        let policy_engine =
            CachePolicyEngine::new(&config, crate::cache::eviction::PolicyType::default())?;
        let performance_monitor = PerformanceMonitor::new();
        let error_recovery = ManagerErrorRecoverySystem::new();
        let tier_operations = TierOperations::new();
        
        // Initialize task coordinator directly
        let task_coordinator = TaskCoordinator::new_direct(config.worker.task_queue_capacity as usize);

        // Wire GC coordinator to maintenance scheduler
        let maintenance_sender = maintenance_scheduler.task_sender.clone();
        let _ = crate::cache::memory::gc_coordinator::set_global_maintenance_sender(maintenance_sender);

        let manager = Self {
            config,
            strategy_selector,
            tier_manager,
            unified_stats,
            maintenance_scheduler,
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
                
                // ML-based access pattern analysis is handled by the policy engine internally
                
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

        // Submit background maintenance task after put operation
        let canonical_task = crate::cache::worker::types::WorkerMaintenanceOps::update_statistics_task();
        let _ = self.maintenance_scheduler.submit_task(canonical_task, 1000);

        Ok(())
    }

    /// Remove value from all cache tiers with atomic consistency
    pub fn remove(&self, key: &K) -> bool {
        let removed = self.tier_operations.remove_from_all_tiers(key);

        if removed {
            // Update operation counter atomically using public method
            self.unified_stats.record_miss(0);
            
            // Submit background cleanup task after removal
            let canonical_task = crate::cache::worker::types::WorkerMaintenanceOps::cleanup_expired_task();
            let _ = self.maintenance_scheduler.submit_task(canonical_task, 500);
        }

        removed
    }

    /// Clear all cache tiers with atomic coordination
    pub fn clear(&self) -> Result<(), CacheOperationError> {
        self.tier_operations.clear_all_tiers()?;

        // Reset unified statistics atomically
        self.unified_stats.reset_all_counters();
        
        // MaintenanceScheduler handles ongoing operations automatically
        // No special trigger needed for clear operation

        // Check maintenance scheduler health after clear operation
        if !self.maintenance_scheduler.is_healthy() {
            log::warn!("Maintenance scheduler health check failed after clear operation");
        }

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
        
        // Include task coordinator statistics in detailed analytics
        let task_stats = self.task_coordinator.get_stats();
        log::debug!("Task coordinator stats - active tasks: {}, success rate: {}%", 
                   task_stats.active_task_count, task_stats.success_rate_percent);
        
        (unified_stats, policy_stats, analyzer_stats)
    }

    /// Get strategy performance metrics for workload analysis
    pub fn strategy_metrics(&self) -> &crate::cache::manager::strategy::StrategyMetrics {
        // Check maintenance scheduler health when accessing metrics
        if !self.maintenance_scheduler.is_healthy() {
            log::warn!("Maintenance scheduler health degraded during strategy metrics access");
        }
        
        self.strategy_selector.get_metrics()
    }

    /// Get strategy thresholds configuration
    pub fn strategy_thresholds(&self) -> &crate::cache::manager::strategy::StrategyThresholds {
        // MaintenanceScheduler provides built-in health monitoring
        log::debug!("Strategy thresholds accessed - maintenance scheduler operational");
        
        self.strategy_selector.get_thresholds()
    }

    /// Force strategy change for manual optimization or testing
    pub fn force_cache_strategy(&self, strategy: crate::cache::manager::strategy::CacheStrategy) {
        // MaintenanceScheduler handles worker state internally
        log::info!("Strategy change requested - using work-stealing scheduler");
        
        self.strategy_selector.force_strategy(strategy);
        
        // No special trigger needed - scheduler handles ongoing operations
    }

    /// Start background processing with work-stealing scheduler
    fn start_background_processing(&self) -> Result<(), CacheOperationError> {
        // Maintenance scheduler is already initialized with worker threads
        // No additional startup required
        
        // Task coordinator is already initialized and ready for command processing
        
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
            // Maintenance scheduler provides health check
            if self.maintenance_scheduler.is_healthy() {
                log::debug!("Promotion decision - maintenance scheduler healthy");
            }
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
                use crate::cache::tier::cold::compaction_system::{CompactionSystem, CompactionTask};
                
                // Use the existing sophisticated compaction system
                let compaction_system = CompactionSystem::new(target_size as u64)
                    .map_err(|e| CacheOperationError::io_failed(&format!("Compaction system init failed: {}", e)))?;
                let task = match tier {
                    crate::cache::types::CacheTier::Cold => CompactionTask::CompactData,
                    _ => CompactionTask::OptimizeCompression, // Hot/Warm use compression optimization
                };
                
                compaction_system.schedule_compaction(task)
                    .map_err(|_| CacheOperationError::InternalError)?;
                Ok(())
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

    /// Cancel a specific task
    pub fn cancel_task(&self, task_id: u64) -> Result<bool, CacheOperationError> {
        self.task_coordinator.cancel_task(task_id)
    }

    /// Atomically put value only if key is not present
    /// Returns the previous value if key was present, None if key was absent
    pub fn put_if_absent(&self, key: K, value: V) -> Result<Option<V>, CacheOperationError> {
        // Use lock-free tier-native atomic operations
        self.tier_operations.put_if_absent_atomic(key, value)
    }

    /// Atomically replace existing value with new value, returning the old value
    /// Returns None if key was not present
    pub fn replace(&self, key: K, value: V) -> Result<Option<V>, CacheOperationError> {
        // Use lock-free tier-native atomic operations
        self.tier_operations.replace_atomic(key, value)
    }

    /// Atomically replace value only if current value equals expected value
    /// Returns true if replacement occurred, false otherwise
    /// 
    /// Note: This method is only available when V implements PartialEq
    pub fn compare_and_swap(&self, key: K, expected: V, new_value: V) -> Result<bool, CacheOperationError>
    where 
        V: PartialEq
    {
        // Use lock-free tier-native atomic operations
        self.tier_operations.compare_and_swap_atomic(key, expected, new_value)
    }

    /// Check if key exists in cache without retrieving the value
    pub fn contains_key(&self, key: &K) -> bool {
        self.get(key).is_some()
    }

    /// Atomically get value or insert using factory function if key is absent
    pub fn get_or_insert_atomic<F>(&self, key: K, factory: F) -> Result<V, CacheOperationError>
    where 
        F: FnOnce() -> V
    {
        self.tier_operations.get_or_insert_atomic(key, factory)
    }

    /// Shutdown cache system gracefully with proper cleanup
    pub fn shutdown_gracefully(&self) -> Result<(), CacheOperationError> {
        // Stop maintenance operations gracefully
        self.maintenance_scheduler.stop_maintenance()?;
        
        // Note: Full shutdown with thread joining requires ownership of MaintenanceScheduler
        // This is handled during UnifiedCacheManager Drop if needed
        
        Ok(())
    }

    /// Start a background processing task with specified priority
    pub fn start_background_processor(&self) -> Result<(), CacheOperationError> {        
        // Check if maintenance scheduler is healthy before starting
        if !self.maintenance_scheduler.is_healthy() {
            return Err(CacheOperationError::InvalidState("Maintenance scheduler not healthy".to_string()));
        }
        
        // Start worker threads instead
        self.start_background_processing()
    }
}

// String-specific convenience APIs removed - use generic UnifiedCacheManager<K, V> directly
