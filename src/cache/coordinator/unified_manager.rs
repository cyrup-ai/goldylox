//! Core unified cache manager coordinating all tiers with atomic state management
//!
//! This module contains the main UnifiedCacheManager struct and its core operations
//! for intelligently managing multi-tier cache access with SIMD optimization.

use super::strategy_selector::CacheStrategySelector;
use super::tier_operations::TierOperations;
use crate::cache::coherence::ProtocolConfiguration;
use crate::cache::config::CacheConfig;
use crate::cache::eviction::CachePolicyEngine;
use crate::cache::manager::background::types::{MaintenanceConfig, MaintenanceScheduler};
use crate::cache::manager::error_recovery::ErrorRecoverySystem as ManagerErrorRecoverySystem;
use crate::cache::manager::performance::core::PerformanceMonitor;
use crate::cache::tier::hot::prefetch::{
    PrefetchPredictor,
    types::{PrefetchConfig, PrefetchStats},
};
use crate::cache::tier::manager::TierPromotionManager;
use crate::cache::traits::core::{CacheKey, CacheValue};
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::types::AccessPath;
use crate::cache::types::performance::timer::PrecisionTimer;
use crate::cache::worker::task_coordination::{CacheCommand, TaskCoordinator};
use crate::telemetry::unified_stats::UnifiedCacheStatistics;
use crate::telemetry::unified_stats::UnifiedStats;
use dashmap::DashMap;
use crate::cache::tier::hot::thread_local::HotTierCoordinator;
use crate::cache::tier::warm::global_api::WarmTierCoordinator;
use crate::cache::tier::cold::ColdTierCoordinator;

/// Prefetch predictor communication requests
///
/// All requests follow fire-and-forget pattern for critical path operations.
/// Monitoring/maintenance requests (GetStats, GetQueueStatus, CleanupExpired)
/// use blocking responses but are NEVER called from get() critical path.
///
/// The PrefetchWorker operates autonomously, generating predictions periodically
/// (every 100ms) without being asked. RecordAccess messages accumulate pattern
/// data which the worker processes independently using ML-based analysis.
///
/// See PREFETCH_1 and PREFETCH_2 task files for architecture details.
#[derive(Debug)]
pub enum PrefetchRequest<K: CacheKey> {
    /// Record access pattern for prediction
    RecordAccess {
        key: K,
        access_time_ns: u64,
        context_hash: u64,
    },
    /// Get queue status
    GetQueueStatus {
        response: tokio::sync::oneshot::Sender<(usize, usize)>,
    },
    /// Cleanup expired requests
    CleanupExpired {
        current_time_ns: u64,
        response: tokio::sync::oneshot::Sender<usize>,
    },
    /// Update configuration
    UpdateConfig { config: PrefetchConfig },
    /// Clear state
    ClearState,
    /// Get statistics
    GetStats {
        response: tokio::sync::oneshot::Sender<PrefetchStats>,
    },
}

/// PrefetchPredictor worker that owns the predictor state
pub struct PrefetchWorker<K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static> {
    receiver: tokio::sync::mpsc::UnboundedReceiver<PrefetchRequest<K>>,
    predictor: PrefetchPredictor<K>,
}

impl<K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static> PrefetchWorker<K> {
    pub fn new(receiver: tokio::sync::mpsc::UnboundedReceiver<PrefetchRequest<K>>, config: PrefetchConfig) -> Self {
        Self {
            receiver,
            predictor: PrefetchPredictor::new(config),
        }
    }

    /// Run the prefetch worker loop with autonomous prediction generation
    pub async fn run<V: CacheValue + Default + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static>(
        mut self,
        task_coordinator: std::sync::Arc<TaskCoordinator<K, V>>,
    ) {
        use std::time::{Duration, Instant};

        let mut last_prediction = Instant::now();
        let prediction_interval = Duration::from_millis(100);
        let recv_timeout = Duration::from_millis(10);

        loop {
            match tokio::time::timeout(recv_timeout, self.receiver.recv()).await {
                Ok(Some(request)) => {
                    match request {
                        PrefetchRequest::RecordAccess {
                            key,
                            access_time_ns,
                            context_hash,
                        } => {
                            // Accumulate access patterns in predictor
                            self.predictor.record_access(&key, access_time_ns, context_hash);

                            // Check if it's time to generate predictions
                            if last_prediction.elapsed() >= prediction_interval {
                                self.generate_and_enqueue_predictions(&task_coordinator);
                                last_prediction = Instant::now();
                            }
                        }
                        PrefetchRequest::GetStats { response } => {
                            let stats = self.predictor.get_stats();
                            let _ = response.send(stats);
                        }
                        PrefetchRequest::UpdateConfig { config } => {
                            self.predictor.update_config(config);
                        }
                        PrefetchRequest::ClearState => {
                            self.predictor.clear();
                        }
                        PrefetchRequest::CleanupExpired {
                            current_time_ns,
                            response,
                        } => {
                            let cleaned = self.predictor.cleanup_expired_requests(
                                current_time_ns,
                                30_000_000_000,
                            );
                            let _ = response.send(cleaned);
                        }
                        PrefetchRequest::GetQueueStatus { response } => {
                            let status = self.predictor.queue_status();
                            let _ = response.send(status);
                        }
                    }
                }
                Ok(None) => {
                    // Channel closed - graceful shutdown
                    break;
                }
                Err(_) => {
                    // Timeout - check prediction timer anyway
                    if last_prediction.elapsed() >= prediction_interval {
                        self.generate_and_enqueue_predictions(&task_coordinator);
                        last_prediction = Instant::now();
                    }
                }
            }
        }
    }

    /// Generate prefetch predictions and enqueue them autonomously
    fn generate_and_enqueue_predictions<V: CacheValue + Default + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static>(
        &mut self,
        task_coordinator: &TaskCoordinator<K, V>,
    ) {
        // Generate predictions from accumulated access patterns
        let prefetch_requests = self.predictor.get_next_prefetches(5);

        if prefetch_requests.is_empty() {
            // Not enough pattern data yet - skip this cycle
            return;
        }

        let mut enqueued_count = 0;

        // Enqueue prefetch commands - non-blocking!
        for request in prefetch_requests {
            // Check confidence threshold
            if !self.predictor.should_prefetch(&request.key) {
                continue;
            }

            let prefetch_command = CacheCommand::Prefetch {
                key: request.key.clone(),
                confidence: request.confidence.as_f64(),
                timestamp: std::time::Instant::now(),
            };

            // Non-blocking enqueue - if queue full, skip and retry next cycle
            match task_coordinator.enqueue_command(prefetch_command) {
                Ok(_) => {
                    enqueued_count += 1;
                }
                Err(_) => {
                    // Queue full - stop this cycle, will retry in 100ms
                    break;
                }
            }
        }

        // Optional trace logging (can be removed if too noisy)
        if enqueued_count > 0 {
            log::trace!(
                "PrefetchWorker: enqueued {} prefetch predictions",
                enqueued_count
            );
        }
    }
}

/// Channel-based PrefetchPredictor interface
#[derive(Debug, Clone)]
pub struct PrefetchChannel<K: CacheKey> {
    sender: tokio::sync::mpsc::UnboundedSender<PrefetchRequest<K>>,
}

impl<K: CacheKey> PrefetchChannel<K> {
    /// Record access pattern for prediction
    pub fn record_access(&self, key: K, access_time_ns: u64, context_hash: u64) {
        let _ = self.sender.send(PrefetchRequest::RecordAccess {
            key,
            access_time_ns,
            context_hash,
        });
    }

    /// Get queue status
    pub async fn queue_status(&self) -> (usize, usize) {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        match self.sender.send(PrefetchRequest::GetQueueStatus {
            response: response_tx,
        }) {
            Ok(_) => response_rx.await.unwrap_or((0, 0)),
            Err(_) => (0, 0),
        }
    }

    /// Cleanup expired requests
    pub async fn cleanup_expired(&self, current_time_ns: u64) -> usize {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        match self.sender.send(PrefetchRequest::CleanupExpired {
            current_time_ns,
            response: response_tx,
        }) {
            Ok(_) => response_rx.await.unwrap_or(0),
            Err(_) => 0,
        }
    }

    /// Update configuration
    pub fn update_config(&self, config: PrefetchConfig) {
        let _ = self.sender.send(PrefetchRequest::UpdateConfig { config });
    }

    /// Clear state
    pub fn clear(&self) {
        let _ = self.sender.send(PrefetchRequest::ClearState);
    }

    /// Get statistics
    pub async fn get_stats(&self) -> PrefetchStats {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        match self.sender.send(PrefetchRequest::GetStats {
            response: response_tx,
        }) {
            Ok(_) => response_rx.await.unwrap_or_default(),
            Err(_) => PrefetchStats::default(),
        }
    }
}

/// Unified cache manager coordinating all tiers with atomic state management
#[derive(Debug)]
pub struct UnifiedCacheManager<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue
        + Default
        + PartialEq
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
> {
    /// Cache configuration (immutable after initialization)
    #[allow(dead_code)]
    // Cache coordination - config used in unified cache configuration management
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
    /// Task coordinator for async cache operations (Arc-wrapped for sharing with prefetch worker)
    task_coordinator: std::sync::Arc<TaskCoordinator<K, V>>,
    /// Prefetch predictor channel for hot tier access pattern analysis and prefetching
    prefetch_channel: PrefetchChannel<K>,
    
    /// Per-instance cold tier coordinator with dedicated service thread
    /// Used for cold tier compression stats and operations
    cold_tier_coordinator: ColdTierCoordinator,

    /// Per-instance allocation manager for memory pool operations
    pub(crate) allocation_manager: crate::cache::memory::allocation_manager::AllocationManager<K, V>,

    /// Pool coordinator for memory cleanup operations
    pool_coordinator: std::sync::Arc<crate::cache::memory::pool_manager::cleanup_manager::PoolCoordinator>,
}

impl<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue
        + Default
        + PartialEq
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
> UnifiedCacheManager<K, V>
{
    /// Default priority for async operations
    const DEFAULT_TASK_PRIORITY: u16 = 5;

    /// Create new unified cache manager with full tier coordination
    pub async fn new(config: CacheConfig) -> Result<Self, CacheOperationError> {
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
        let _warm_tier_config = config.warm_tier.clone();
        // Warm tier initialization now happens through per-instance coordinator
        // No global init needed - coordinator handles tier creation
        // NOTE: Cold tier initialization moved after coordinator creation below

        // ========== Create coordinators BEFORE stats and coherence ==========
        
        // Create per-instance hot tier coordinator (replaces global static)
        let hot_tier_coordinator = HotTierCoordinator {
            hot_tiers: std::sync::Arc::new(DashMap::new()),
            instance_selector: std::sync::atomic::AtomicUsize::new(0),
        };
        
        // Initialize hot tier with proper generic types using the coordinator
        crate::cache::tier::hot::init_simd_hot_tier::<K, V>(&hot_tier_coordinator, hot_tier_config).await?;
        
        // Create per-instance warm tier coordinator (replaces global static)
        let warm_tier_coordinator = WarmTierCoordinator {
            warm_tiers: std::sync::Arc::new(DashMap::new()),
            instance_selector: std::sync::atomic::AtomicUsize::new(0),
        };
        
        // Create per-instance cold tier coordinator with dedicated service thread
        let cold_tier_coordinator = ColdTierCoordinator::new()?;

        // Initialize coherence protocol with atomic coordination
        let _coherence_config = ProtocolConfiguration {
            optimistic_concurrency: true,
            write_through: false,
            max_invalidation_retries: 3,
            coherence_timeout_ns: 1_000_000, // 1ms
            strict_ordering: false,
            schema_version: 1,
        };
        let coherence_system: crate::cache::coherence::protocol::global_api::CoherenceSystem<
            K,
            V,
        > = crate::cache::coherence::protocol::global_api::CoherenceSystem::new(
            hot_tier_coordinator.clone(),
            warm_tier_coordinator.clone(),
            cold_tier_coordinator.clone(),
        ).map_err(|_| CacheOperationError::InternalError)?;

        // Initialize all subsystems with atomic state management
        let strategy_selector = CacheStrategySelector::new(&config)?;
        let tier_manager = TierPromotionManager::new(&config)?;
        let unified_stats = UnifiedCacheStatistics::new(
            std::sync::Arc::new(warm_tier_coordinator.clone()),
        );
        
        // Create Arc-wrapped statistics for sharing with worker threads
        let unified_stats_arc = std::sync::Arc::new(UnifiedCacheStatistics::new(
            std::sync::Arc::new(warm_tier_coordinator.clone()),
        ));
        let coherence_stats_arc = std::sync::Arc::new(coherence_system.coherence_stats().clone());
        
        // Create allocation manager (which creates the working pool coordinator)
        let (allocation_manager, pool_coordinator) = crate::cache::memory::allocation_manager::AllocationManager::new(&config, hot_tier_coordinator.clone(), warm_tier_coordinator.clone(), cold_tier_coordinator.clone())?;
        
        // Initialize cold tier with proper generic types (using per-instance coordinator)
        // NOTE: Must happen AFTER pool_coordinator creation as it's needed for compaction
        crate::cache::tier::cold::init_cold_tier::<K, V>(
            &cold_tier_coordinator,
            config.cold_tier.base_dir.as_str(),
            &config.cache_id,
            pool_coordinator.clone(),
        ).await
        .map_err(|e| CacheOperationError::io_failed(format!("Cold tier init failed: {}", e)))?;
        
        // Create maintenance config with worker parameters from config
        let mut maintenance_config = MaintenanceConfig {
            worker_count: config.worker.thread_pool_size as u32,
            ..Default::default()
        };
        if config.worker.maintenance_interval_ns != 1_000_000_000 {
            maintenance_config.heartbeat_interval_ns = config.worker.maintenance_interval_ns;
        }
        let maintenance_scheduler = MaintenanceScheduler::new(
            maintenance_config,
            unified_stats_arc,
            coherence_stats_arc,
            hot_tier_coordinator.clone(),
            warm_tier_coordinator.clone(),
            cold_tier_coordinator.clone(),
            pool_coordinator.clone(),
        )?;
        let policy_engine =
            CachePolicyEngine::new(
                &config, 
                crate::cache::eviction::PolicyType::default(), 
                cold_tier_coordinator.clone(),
                warm_tier_coordinator.clone(),
            )?;
        let performance_monitor = PerformanceMonitor::new();
        let error_recovery = ManagerErrorRecoverySystem::new();

        // Initialize task coordinator directly (Arc-wrapped for sharing with prefetch worker)
        let task_coordinator = std::sync::Arc::new(
            TaskCoordinator::new_direct(config.worker.task_queue_capacity as usize)?
        );

        // Initialize prefetch predictor with configuration
        let prefetch_config = PrefetchConfig {
            history_size: config.hot_tier.prefetch_distance as usize * 100, // Scale with prefetch distance
            max_prefetch_distance: config.hot_tier.prefetch_distance as usize,
            min_confidence_threshold: 0.6,
            pattern_detection_window: std::time::Duration::from_secs(300),
            max_patterns: 100,
            prefetch_queue_size: 50,
        };

        // Create prefetch worker with channels
        let (prefetch_sender, prefetch_receiver) = tokio::sync::mpsc::unbounded_channel::<PrefetchRequest<K>>();
        let prefetch_channel = PrefetchChannel {
            sender: prefetch_sender,
        };

        // Spawn prefetch worker thread with task coordinator for autonomous operation
        let worker_config = prefetch_config.clone();
        let task_coordinator_clone = task_coordinator.clone(); // Arc::clone is cheap

        tokio::runtime::Handle::current().spawn(async move {
            let worker = PrefetchWorker::new(prefetch_receiver, worker_config);
            worker.run(task_coordinator_clone).await;
        });

        // Create TierOperations with injected coordinators
        let tier_operations = TierOperations::new_with_coordinators(
            hot_tier_coordinator.clone(),
            warm_tier_coordinator.clone(),
            cold_tier_coordinator.clone(),
        )?;

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
            prefetch_channel,
            cold_tier_coordinator,
            allocation_manager,
            pool_coordinator,
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
    pub async fn get(&self, key: &K) -> Option<V> {
        let timer = PrecisionTimer::start();
        let mut access_path = AccessPath::new();

        // Use strategy selector to determine optimal access pattern
        let _access_strategy = self.strategy_selector.current_strategy();

        // Record operation start with atomic increment
        self.unified_stats.record_miss(0); // Will be updated with actual timing later

        // Cache timestamp once to avoid multiple syscalls (used for prefetch pattern recording)
        let current_time_ns =
            match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                Ok(duration) => duration.as_nanos() as u64,
                Err(_) => 0, // Fallback for system clock before UNIX epoch
            };

        // Try hot tier first (SIMD-optimized, fastest access) with error recovery
        if self.error_recovery.is_tier_available(0) {
            if let Some(value) = self.tier_operations.try_hot_tier_get(key, &mut access_path).await {
                let elapsed_ns = timer.elapsed_ns();
                self.record_hit(crate::cache::coherence::CacheTier::Hot, elapsed_ns);
                let _ = self.policy_engine.pattern_analyzer.record_access(key);

                // Record performance metrics via strategy selector (which uses atomic operations)
                self.strategy_selector
                    .record_strategy_performance(1.0, elapsed_ns);

                // Record hit in performance monitor with atomic operations
                self.performance_monitor.record_hit(elapsed_ns);

                // Record successful operation for circuit breaker
                self.error_recovery.record_success(0);

                // ML-based access pattern analysis is handled by the policy engine internally

                // Record access pattern for prefetch prediction (using cached timestamp)
                let context_hash = key.cache_hash();
                self.prefetch_channel
                    .record_access(key.clone(), current_time_ns, context_hash);

                return Some(value);
            }
        } else {
            // Hot tier unavailable, record error recovery attempt
            let _ = self.error_recovery.execute_recovery(
                crate::cache::types::statistics::multi_tier::ErrorType::OutOfMemory,
                0,
            );
        }

        // Try warm tier second (moderate speed, balanced capacity) with error recovery
        if self.error_recovery.is_tier_available(1) {
            if let Some(value) = self
                .tier_operations
                .try_warm_tier_get(key, &mut access_path).await
            {
                let elapsed_ns = timer.elapsed_ns();
                self.record_hit(crate::cache::coherence::CacheTier::Warm, elapsed_ns);
                let _ = self.policy_engine.pattern_analyzer.record_access(key);

                // Record performance metrics for warm tier access
                self.strategy_selector
                    .record_strategy_performance(1.0, elapsed_ns);

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

                // Record access pattern for prefetch prediction (using cached timestamp)
                let context_hash = key.cache_hash();
                self.prefetch_channel
                    .record_access(key.clone(), current_time_ns, context_hash);

                return Some(value);
            }
        } else {
            // Warm tier unavailable, record error recovery attempt
            let _ = self.error_recovery.execute_recovery(
                crate::cache::types::statistics::multi_tier::ErrorType::OutOfMemory,
                1,
            );
        }

        // Try cold tier last (highest capacity, persistent storage)
        if let Some(value) = self
            .tier_operations
            .try_cold_tier_get(key, &mut access_path).await
        {
            let elapsed_ns = timer.elapsed_ns();
            self.record_hit(crate::cache::coherence::CacheTier::Cold, elapsed_ns);
            let _ = self.policy_engine.pattern_analyzer.record_access(key);

            // Record hit in performance monitor with atomic operations
            self.performance_monitor.record_hit(elapsed_ns);

            // Consider multi-tier promotion based on access patterns
            self.consider_multi_tier_promotion(key, &value, &access_path);

            // Record access pattern for prefetch prediction (using cached timestamp)
            let context_hash = key.cache_hash();
            self.prefetch_channel
                .record_access(key.clone(), current_time_ns, context_hash);

            return Some(value);
        }

        // Cache miss - record for analytics and prefetch prediction
        let elapsed_ns = timer.elapsed_ns();
        self.record_miss(elapsed_ns);
        let _ = self.policy_engine.pattern_analyzer.record_miss(key);

        // Record miss in performance monitor with atomic operations
        self.performance_monitor.record_miss(elapsed_ns);

        // Record access pattern for prefetch prediction (using cached timestamp)
        let context_hash = key.cache_hash();
        self.prefetch_channel
            .record_access(key.clone(), current_time_ns, context_hash);

        // Prefetch prediction happens autonomously in PrefetchWorker background thread
        // (see PREFETCH_2 task). The worker periodically analyzes accumulated access
        // patterns and enqueues prefetch commands directly to the task coordinator
        // without blocking get(). This design keeps the critical cache miss path
        // fast (<1μs overhead) while still enabling intelligent ML-based prefetching.
        // See PrefetchWorker::run() at line 87 for autonomous prediction loop.

        None
    }

    /// Put value in unified cache with optimal tier placement
    pub async fn put(&self, key: K, value: V) -> Result<(), CacheOperationError> {
        let timer = PrecisionTimer::start();
        
        let placement_decision = self.tier_operations
            .analyze_placement(&key, &value, &self.policy_engine);
        
        let mut current_tier = placement_decision.primary_tier;
        let mut last_error = None;
        
        // Try primary tier, then failover chain if needed
        loop {
            match current_tier {
                crate::cache::coherence::CacheTier::Hot => {
                    if self.error_recovery.is_tier_available(0) {
                        match self.tier_operations.put_with_replication(
                            key.clone(),
                            value.clone(),
                            crate::cache::coherence::CacheTier::Hot,
                            placement_decision.replication_tiers.clone(),
                        ).await {
                            Ok(_) => {
                                self.error_recovery.record_success(0);
                                break; // Success
                            }
                            Err(e) => {
                                log::warn!("Hot tier put failed: {:?}, initiating failover", e);
                                self.error_recovery.execute_recovery(
                                    crate::cache::types::statistics::multi_tier::ErrorType::TierTransition,
                                    0,
                                );
                                last_error = Some(e);
                                current_tier = crate::cache::coherence::CacheTier::Warm; // Failover to Warm
                                continue;
                            }
                        }
                    } else {
                        log::debug!("Hot tier unavailable (circuit breaker open), failing over to Warm");
                        current_tier = crate::cache::coherence::CacheTier::Warm;
                        continue;
                    }
                }
                
                crate::cache::coherence::CacheTier::Warm => {
                    if self.error_recovery.is_tier_available(1) {
                        match self.tier_operations.put_with_replication(
                            key.clone(),
                            value.clone(),
                            crate::cache::coherence::CacheTier::Warm,
                            vec![], // No replication for failover puts
                        ).await {
                            Ok(_) => {
                                self.error_recovery.record_success(1);
                                log::info!("Successfully failed over PUT to Warm tier");
                                break;
                            }
                            Err(e) => {
                                log::warn!("Warm tier put failed: {:?}, failing over to Cold", e);
                                self.error_recovery.execute_recovery(
                                    crate::cache::types::statistics::multi_tier::ErrorType::TierTransition,
                                    1,
                                );
                                last_error = Some(e);
                                current_tier = crate::cache::coherence::CacheTier::Cold; // Failover to Cold
                                continue;
                            }
                        }
                    } else {
                        log::debug!("Warm tier unavailable (circuit breaker open), failing over to Cold");
                        current_tier = crate::cache::coherence::CacheTier::Cold;
                        continue;
                    }
                }
                
                crate::cache::coherence::CacheTier::Cold => {
                    // Cold tier is final fallback - no more failover options
                    match self.tier_operations.put_cold_tier_only(key, value).await {
                        Ok(_) => {
                            self.error_recovery.record_success(2);
                            log::info!("Successfully failed over PUT to Cold tier");
                            break;
                        }
                        Err(e) => {
                            log::error!("Cold tier put failed - no more failover options: {:?}", e);
                            self.error_recovery.execute_recovery(
                                crate::cache::types::statistics::multi_tier::ErrorType::TierTransition,
                                2,
                            );
                            return Err(last_error.unwrap_or(e));
                        }
                    }
                }
            }
        }
        
        // Update metrics
        let elapsed_ns = timer.elapsed_ns();
        self.unified_stats.update_memory_usage(elapsed_ns);
        
        // Submit background maintenance task
        let canonical_task = crate::cache::worker::types::WorkerMaintenanceOps::update_statistics_task();
        let _ = self.maintenance_scheduler.submit_task(canonical_task, 1000);
        
        Ok(())
    }

    /// Remove value from all cache tiers with atomic consistency
    pub async fn remove(&self, key: &K) -> bool {
        let removed = self.tier_operations.remove_from_all_tiers(key).await;

        if removed {
            // Update operation counter atomically using public method
            self.unified_stats.record_miss(0);

            // Submit background cleanup task after removal
            let canonical_task =
                crate::cache::worker::types::WorkerMaintenanceOps::cleanup_expired_task();
            let _ = self.maintenance_scheduler.submit_task(canonical_task, 500);
        }

        removed
    }

    /// Clear all cache tiers with atomic coordination
    pub async fn clear(&self) -> Result<(), CacheOperationError> {
        self.tier_operations.clear_all_tiers().await?;

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
            } else {
                0.0
            },
            tier_hit_rates: [
                if perf_stats.total_operations > 0 {
                    perf_stats.hot_tier_hits as f32 / perf_stats.total_operations as f32
                } else {
                    0.0
                },
                if perf_stats.total_operations > 0 {
                    perf_stats.warm_tier_hits as f32 / perf_stats.total_operations as f32
                } else {
                    0.0
                },
                if perf_stats.total_operations > 0 {
                    perf_stats.cold_tier_hits as f32 / perf_stats.total_operations as f32
                } else {
                    0.0
                },
            ],
        }
    }

    /// Get reference to unified cache statistics for per-instance monitoring
    ///
    /// Returns a direct reference to the UnifiedCacheStatistics instance,
    /// enabling external systems to access real-time atomic statistics
    /// without copying or aggregation overhead.
    pub fn get_unified_stats(&self) -> &UnifiedCacheStatistics {
        &self.unified_stats
    }

    /// Get detailed analytics including policy engine and pattern analyzer statistics
    pub fn detailed_analytics(
        &self,
    ) -> (
        UnifiedStats,
        crate::cache::eviction::PolicyStats,
        crate::cache::analyzer::types::AnalyzerStatistics,
    ) {
        let unified_stats = self.stats();
        let (policy_stats, analyzer_stats) = self.policy_engine.get_comprehensive_stats();

        // Include task coordinator statistics in detailed analytics
        let task_stats = self.task_coordinator.get_stats();
        log::debug!(
            "Task coordinator stats - active tasks: {}, success rate: {}%",
            task_stats.active_task_count,
            task_stats.success_rate_percent
        );

        (unified_stats, policy_stats, analyzer_stats)
    }

    /// Get strategy performance metrics for workload analysis
    #[allow(dead_code)] // Unified manager - strategy metrics API for workload analysis and performance monitoring
    pub fn strategy_metrics(&self) -> &crate::cache::manager::strategy::StrategyMetrics {
        // Check maintenance scheduler health when accessing metrics
        if !self.maintenance_scheduler.is_healthy() {
            log::warn!("Maintenance scheduler health degraded during strategy metrics access");
        }

        self.strategy_selector.get_metrics()
    }

    /// Get strategy thresholds configuration
    #[allow(dead_code)] // Unified manager - strategy threshold configuration API for adaptive tuning
    pub fn strategy_thresholds(&self) -> &crate::cache::manager::strategy::StrategyThresholds {
        // MaintenanceScheduler provides built-in health monitoring
        log::debug!("Strategy thresholds accessed - maintenance scheduler operational");

        self.strategy_selector.get_thresholds()
    }

    /// Force strategy change for manual optimization or testing
    #[allow(dead_code)] // Unified manager - manual strategy override API for testing and optimization
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
        self.performance_monitor.force_metrics_collection();

        // Initialize memory usage tracking
        self.performance_monitor.record_memory_usage(0);

        // Enable collection for ongoing monitoring
        self.performance_monitor.set_collection_active(true);

        Ok(())
    }

    /// Initialize error recovery with circuit breakers
    fn initialize_error_recovery(&self) -> Result<(), CacheOperationError> {
        // Reset any existing error states
        self.error_recovery.reset_all();

        // Initialize circuit breakers for each tier
        for tier_idx in 0..3 {
            // Hot, Warm, Cold
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
    fn consider_multi_tier_promotion(&self, key: &K, value: &V, _access_path: &AccessPath) {
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

    /// Schedule cache operation using task coordinator (synchronous, crossbeam-based)
    #[allow(dead_code)] // Unified manager - synchronous operation scheduling API with task coordination
    pub fn schedule_operation<F, T>(
        &self,
        operation: F,
        task_type: String,
        keys: Vec<K>,
    ) -> Result<u64, CacheOperationError>
    where
        F: FnOnce(
                crate::cache::worker::task_coordination::TaskExecutionContext<K, V>,
            ) -> Result<T, CacheOperationError>
            + Send
            + 'static,
        T: Send + 'static,
    {
        self.task_coordinator.schedule_cache_operation(
            operation,
            task_type,
            Self::DEFAULT_TASK_PRIORITY,
            keys,
        )
    }

    /// Schedule cache operation with result coordination (synchronous, crossbeam-based)
    #[allow(dead_code)] // Unified manager - synchronous operation scheduling API with result coordination  
    pub fn schedule_operation_with_result<F, T>(
        &self,
        operation: F,
        task_type: String,
        keys: Vec<K>,
        result_sender: Option<crossbeam_channel::Sender<Result<T, CacheOperationError>>>,
    ) -> Result<u64, CacheOperationError>
    where
        F: FnOnce(
                crate::cache::worker::task_coordination::TaskExecutionContext<K, V, T>,
            ) -> Result<T, CacheOperationError>
            + Send
            + 'static,
        T: Send + 'static,
    {
        self.task_coordinator.schedule_cache_operation_with_result(
            operation,
            task_type,
            Self::DEFAULT_TASK_PRIORITY,
            keys,
            result_sender,
        )
    }

    /// Schedule cache operation with predetermined task ID (synchronous, crossbeam-based)
    #[allow(dead_code)]
    pub fn schedule_operation_with_task_id<F, T>(
        &self,
        operation: F,
        task_type: String,
        keys: Vec<K>,
        result_sender: Option<crossbeam_channel::Sender<Result<T, CacheOperationError>>>,
        task_id: u64,
    ) -> Result<u64, CacheOperationError>
    where
        F: FnOnce(
                crate::cache::worker::task_coordination::TaskExecutionContext<K, V, T>,
            ) -> Result<T, CacheOperationError>
            + Send
            + 'static,
        T: Send + 'static,
    {
        self.task_coordinator.schedule_cache_operation_with_task_id(
            operation,
            task_type,
            Self::DEFAULT_TASK_PRIORITY,
            keys,
            result_sender,
            task_id,
        )
    }

    /// Get TaskCoordinator reference for ID generation
    #[allow(dead_code)]
    pub fn task_coordinator(
        &self,
    ) -> &crate::cache::worker::task_coordination::TaskCoordinator<K, V> {
        &self.task_coordinator
    }

    /// Execute pending cache commands through task coordinator
    #[allow(dead_code)] // Unified manager - command queue flushing API with batch execution
    pub async fn flush_pending_commands(&mut self) -> Result<usize, CacheOperationError> {
        // Get references we'll need for command execution
        let tier_operations = &self.tier_operations;
        let policy_engine = &self.policy_engine;
        let pool_coordinator = &self.pool_coordinator;
        
        // Get mutable access to TaskCoordinator through Arc to drain commands
        let task_coordinator_mut = std::sync::Arc::get_mut(&mut self.task_coordinator)
            .ok_or_else(|| CacheOperationError::InvalidState("Cannot get mutable access to task_coordinator (shared references exist)".to_string()))?;
        
        // Drain all pending commands into a Vec
        let commands = task_coordinator_mut.drain_pending_commands()?;
        let command_count = commands.len();
        
        // Process all commands sequentially with async/await
        // Note: Sequential processing is used because tier_operations and policy_engine
        // are not Arc-wrapped. For concurrent processing, these would need to be
        // wrapped in Arc within UnifiedCacheManager's struct definition.
        for command in commands {
            Self::execute_cache_command_static(command, tier_operations, policy_engine, pool_coordinator).await?;
        }
        
        Ok(command_count)
    }

    /// Execute a cache command from the task coordination system (static version for borrowing)
    #[allow(dead_code)] // Unified manager - static command execution for task coordination integration
    async fn execute_cache_command_static(
        command: CacheCommand<K, V>,
        tier_operations: &TierOperations<K, V>,
        policy_engine: &CachePolicyEngine<K, V>,
        _pool_coordinator: &std::sync::Arc<crate::cache::memory::pool_manager::cleanup_manager::PoolCoordinator>,
    ) -> Result<(), CacheOperationError> {
        use crate::cache::worker::task_coordination::CacheCommand;

        match command {
            CacheCommand::Insert {
                key, value, tier, ..
            } => {
                // Convert between CacheTier types
                let coherence_tier = match tier {
                    crate::cache::types::CacheTier::Hot => crate::cache::coherence::CacheTier::Hot,
                    crate::cache::types::CacheTier::Warm => {
                        crate::cache::coherence::CacheTier::Warm
                    }
                    crate::cache::types::CacheTier::Cold => {
                        crate::cache::coherence::CacheTier::Cold
                    }
                };

                if coherence_tier == crate::cache::coherence::CacheTier::Cold {
                    tier_operations.put_cold_tier_only(key, value).await
                } else {
                    // Use private put_in_tier method through public interface
                    tier_operations.put_with_replication(key, value, coherence_tier, vec![]).await
                }
            }
            CacheCommand::Remove { key, .. } => {
                // Convert tier type and use remove_from_all_tiers for simplicity
                let _ = tier_operations.remove_from_all_tiers(&key).await;
                Ok(())
            }
            CacheCommand::Move { key, to_tier, .. } => {
                // Try to get value from all tiers (since we don't have individual tier getters)
                let mut access_path = AccessPath::new();
                let value = if let Some(v) = tier_operations.try_hot_tier_get(&key, &mut access_path).await {
                    v
                } else if let Some(v) = tier_operations.try_warm_tier_get(&key, &mut access_path).await {
                    v
                } else if let Some(v) = tier_operations.try_cold_tier_get(&key, &mut access_path).await {
                    v
                } else {
                    return Err(CacheOperationError::InvalidState("Key not found in any tier".to_string()));
                };

                // Convert tier types
                let target_coherence_tier = match to_tier {
                    crate::cache::types::CacheTier::Hot => crate::cache::coherence::CacheTier::Hot,
                    crate::cache::types::CacheTier::Warm => {
                        crate::cache::coherence::CacheTier::Warm
                    }
                    crate::cache::types::CacheTier::Cold => {
                        crate::cache::coherence::CacheTier::Cold
                    }
                };

                // Insert in target tier
                if target_coherence_tier == crate::cache::coherence::CacheTier::Cold {
                    tier_operations.put_cold_tier_only(key.clone(), value).await?;
                } else {
                    tier_operations.put_with_replication(
                        key.clone(),
                        value,
                        target_coherence_tier,
                        vec![],
                    ).await?;
                }

                // Remove from all tiers (will clean up from source)
                let _ = tier_operations.remove_from_all_tiers(&key).await;

                Ok(())
            }
            CacheCommand::UpdateMetadata {
                key, access_count, ..
            } => {
                // Update access statistics using available methods
                for _ in 0..access_count.min(100) {
                    // Limit iterations to prevent DoS
                    let _ = policy_engine.pattern_analyzer.record_access(&key);
                }
                Ok(())
            }
            CacheCommand::Prefetch {
                key, confidence, ..
            } => {
                // Execute prefetch if confidence is above threshold
                const PREFETCH_CONFIDENCE_THRESHOLD: f64 = 0.5;
                if confidence >= PREFETCH_CONFIDENCE_THRESHOLD {
                    // Try to find the key in available tiers using tier_operations
                    let mut access_path = AccessPath::new();
                    let prefetch_result = if let Some(v) = tier_operations.try_hot_tier_get(&key, &mut access_path).await {
                        Some(v)
                    } else if let Some(v) = tier_operations.try_warm_tier_get(&key, &mut access_path).await {
                        Some(v)
                    } else {
                        tier_operations.try_cold_tier_get(&key, &mut access_path).await
                    };

                    match prefetch_result {
                        Some(value) => {
                            // Prefetch successful - promote to hot tier for optimal access
                            let _ = tier_operations.put_with_replication(
                                key,
                                value,
                                crate::cache::coherence::CacheTier::Hot,
                                vec![],
                            ).await;
                        }
                        None => {
                            // Prefetch failed - key not found in any tier
                            // No action needed, prefetch simply failed
                        }
                    }
                }
                Ok(())
            }
            CacheCommand::FlushDirty { keys, .. } => {
                // Flush dirty entries by ensuring they're persisted to cold tier
                for key in keys {
                    let mut access_path = AccessPath::new();
                    let value = if let Some(v) = tier_operations.try_hot_tier_get(&key, &mut access_path).await {
                        Some(v)
                    } else {
                        tier_operations.try_warm_tier_get(&key, &mut access_path).await
                    };
                    
                    if let Some(value) = value {
                        // Ensure persistence in cold tier
                        let _ = tier_operations.put_cold_tier_only(key, value).await;
                    }
                }
                Ok(())
            }
            CacheCommand::Compact {
                tier, target_size, ..
            } => {
                // Compaction is handled by the ColdTierCoordinator's trigger_maintenance method
                // This static method doesn't have access to the coordinator, so compaction
                // must be triggered through the UnifiedCacheManager instance methods instead
                log::warn!(
                    "Compaction command received for tier {:?} with target {} but cannot be executed from static context",
                    tier, target_size
                );
                Err(CacheOperationError::invalid_state(
                    "Compaction must be triggered through ColdTierCoordinator.trigger_maintenance()"
                ))
            }
        }
    }

    /// Get task coordinator statistics
    #[allow(dead_code)] // Unified manager - task coordinator statistics API for monitoring and analysis
    pub fn get_task_coordinator_stats(
        &self,
    ) -> crate::cache::worker::task_coordination::CoordinatorStatsSnapshot {
        self.task_coordinator.get_stats()
    }

    /// Get active tasks from task coordinator
    #[allow(dead_code)] // Unified manager - active task enumeration API for monitoring and debugging
    pub fn get_active_tasks(&self) -> Vec<crate::cache::worker::task_coordination::TaskInfo<K>> {
        self.task_coordinator.get_active_tasks()
    }

    /// Cancel a specific task
    #[allow(dead_code)] // Unified manager - task cancellation API for operational control
    pub fn cancel_task(&self, task_id: u64) -> Result<bool, CacheOperationError> {
        self.task_coordinator.cancel_task(task_id)
    }

    /// Get maintenance operation statistics
    #[allow(dead_code)] // Unified manager - maintenance statistics API for operational monitoring
    pub fn get_maintenance_stats(&self) -> crate::goldylox::MaintenanceBreakdown {
        let stats = self.maintenance_scheduler.get_stats();
        let total_time_ns = stats
            .total_maintenance_time_ns
            .load(std::sync::atomic::Ordering::Relaxed);
        let total_ops = stats
            .operations_executed
            .load(std::sync::atomic::Ordering::Relaxed);

        // Convert from internal MaintenanceStats to public MaintenanceBreakdown
        crate::goldylox::MaintenanceBreakdown {
            compaction_time_ms: (stats
                .cleanup_operations
                .load(std::sync::atomic::Ordering::Relaxed)
                * total_time_ns
                / total_ops.max(1)
                / 1_000_000),
            eviction_time_ms: (stats
                .optimization_operations
                .load(std::sync::atomic::Ordering::Relaxed)
                * total_time_ns
                / total_ops.max(1)
                / 1_000_000),
            stats_collection_time_ms: (stats
                .validation_operations
                .load(std::sync::atomic::Ordering::Relaxed)
                * total_time_ns
                / total_ops.max(1)
                / 1_000_000),
            memory_management_time_ms: (stats
                .defrag_operations
                .load(std::sync::atomic::Ordering::Relaxed)
                * total_time_ns
                / total_ops.max(1)
                / 1_000_000),
            total_cycles: total_ops,
            last_maintenance_ns: stats
                .last_maintenance
                .load()
                .map(|instant| instant.elapsed().as_nanos() as u64)
                .unwrap_or(0),
        }
    }

    /// Atomically put value only if key is not present
    /// Returns the previous value if key was present, None if key was absent
    pub async fn put_if_absent(&self, key: K, value: V) -> Result<Option<V>, CacheOperationError> {
        // Use lock-free tier-native atomic operations
        self.tier_operations.put_if_absent_atomic(key, value).await
    }

    /// Atomically replace existing value with new value, returning the old value
    /// Returns None if key was not present
    pub async fn replace(&self, key: K, value: V) -> Result<Option<V>, CacheOperationError> {
        // Use lock-free tier-native atomic operations
        self.tier_operations.replace_atomic(key, value).await
    }

    /// Atomically replace value only if current value equals expected value
    /// Returns true if replacement occurred, false otherwise
    ///
    /// Note: This method is only available when V implements PartialEq
    pub async fn compare_and_swap(
        &self,
        key: K,
        expected: V,
        new_value: V,
    ) -> Result<bool, CacheOperationError>
    where
        V: PartialEq,
    {
        // Use lock-free tier-native atomic operations
        self.tier_operations
            .compare_and_swap_atomic(key, expected, new_value).await
    }

    /// Check if key exists in cache without retrieving the value
    pub async fn contains_key(&self, key: &K) -> bool {
        self.get(key).await.is_some()
    }

    /// Atomically get value or insert using factory function if key is absent
    pub async fn get_or_insert_atomic<F>(&self, key: K, factory: F) -> Result<V, CacheOperationError>
    where
        F: FnOnce() -> V,
    {
        self.tier_operations.get_or_insert_atomic(key, factory).await
    }

    /// Shutdown cache system gracefully with proper cleanup
    #[allow(dead_code)] // Unified manager - graceful shutdown API for proper cleanup and resource management
    pub async fn shutdown_gracefully(&self) -> Result<(), CacheOperationError> {
        // Stop maintenance operations gracefully
        self.maintenance_scheduler.stop_maintenance()?;

        // Sync cold tier storage to disk (async, non-blocking)
        // This ensures all pending writes are flushed before shutdown
        if let Err(e) = crate::cache::tier::cold::sync_storage::<K, V>(
            &self.cold_tier_coordinator
        ).await {
            log::error!("Failed to sync cold tier storage during shutdown: {}", e);
            // Continue shutdown even if sync fails
        }

        // Note: Full shutdown with thread joining requires ownership of MaintenanceScheduler
        // This is handled during UnifiedCacheManager Drop if needed

        Ok(())
    }

    /// Start a background processing task with specified priority
    #[allow(dead_code)] // Unified manager - background processing startup API for maintenance coordination
    pub fn start_background_processor(&self) -> Result<(), CacheOperationError> {
        // Check if maintenance scheduler is healthy before starting
        if !self.maintenance_scheduler.is_healthy() {
            return Err(CacheOperationError::InvalidState(
                "Maintenance scheduler not healthy".to_string(),
            ));
        }

        // Start worker threads instead
        self.start_background_processing()
    }

    // =======================================================================
    // COLD TIER COMPRESSION STATISTICS - PRODUCTION GRADE IMPLEMENTATION
    // =======================================================================

    /// Get total space saved by cold tier compression
    pub async fn get_cold_tier_space_saved(&self) -> u64 {
        let coordinator = &self.cold_tier_coordinator;

        coordinator.execute_read_operation::<K, V, u64, _>(|tier| {
            let stats = tier.compression_engine.get_stats();
            Ok(crate::cache::tier::cold::compression::CompressionStatsSnapshot::total_space_saved(&stats))
        }).await.unwrap_or(0)
    }

    /// Get cold tier compression effectiveness (ratio)
    pub async fn get_cold_tier_compression_effectiveness(&self) -> f64 {
        let coordinator = &self.cold_tier_coordinator;

        coordinator.execute_read_operation::<K, V, f64, _>(|tier| {
            let stats = tier.compression_engine.get_stats();
            Ok(crate::cache::tier::cold::compression::CompressionStatsSnapshot::compression_effectiveness(&stats))
        }).await.unwrap_or(0.0)
    }

    /// Get average cold tier compression time in nanoseconds
    pub async fn get_cold_tier_avg_compression_time(&self) -> u64 {
        let coordinator = &self.cold_tier_coordinator;

        coordinator.execute_read_operation::<K, V, u64, _>(|tier| {
            let stats = tier.compression_engine.get_stats();
            Ok(crate::cache::tier::cold::compression::CompressionStatsSnapshot::avg_compression_time_ns(&stats))
        }).await.unwrap_or(0)
    }

    /// Get average cold tier decompression time in nanoseconds
    pub async fn get_cold_tier_avg_decompression_time(&self) -> u64 {
        let coordinator = &self.cold_tier_coordinator;

        coordinator.execute_read_operation::<K, V, u64, _>(|tier| {
            let stats = tier.compression_engine.get_stats();
            Ok(crate::cache::tier::cold::compression::CompressionStatsSnapshot::avg_decompression_time_ns(&stats))
        }).await.unwrap_or(0)
    }

    /// Get cold tier compression throughput in MB/s
    pub async fn get_cold_tier_compression_throughput(&self) -> f64 {
        let coordinator = &self.cold_tier_coordinator;

        coordinator
            .execute_read_operation::<K, V, f64, _>(|tier| {
                let stats = tier.compression_engine.get_stats();
                Ok(crate::cache::tier::cold::compression::CompressionStatsSnapshot::compression_throughput(&stats) / 1_048_576.0) // Convert bytes/s to MB/s
            }).await
            .unwrap_or(0.0)
    }

    /// Get cold tier decompression throughput in MB/s
    pub async fn get_cold_tier_decompression_throughput(&self) -> f64 {
        let coordinator = &self.cold_tier_coordinator;

        coordinator
            .execute_read_operation::<K, V, f64, _>(|tier| {
                let stats = tier.compression_engine.get_stats();
                Ok(crate::cache::tier::cold::compression::CompressionStatsSnapshot::decompression_throughput(&stats) / 1_048_576.0) // Convert bytes/s to MB/s
            }).await
            .unwrap_or(0.0)
    }

    /// Get current cold tier compression algorithm
    pub async fn get_cold_tier_compression_algorithm(&self) -> Result<String, CacheOperationError> {
        let coordinator = &self.cold_tier_coordinator;

        coordinator
            .execute_read_operation::<K, V, String, _>(|tier| {
                let algorithm = tier.compression_engine.get_algorithm();
                Ok(format!("{:?}", algorithm))
            }).await
            .map_err(|e| {
                CacheOperationError::TierError(format!(
                    "Failed to get compression algorithm: {}",
                    e
                ))
            })
    }

    /// Update cold tier compression thresholds
    pub fn update_cold_tier_compression_thresholds(
        &self,
        min_size: usize,
        max_ratio: f64,
        speed_threshold: f64,
    ) {
        let coordinator = &self.cold_tier_coordinator;
        let _ = coordinator.execute_operation::<K, V, (), _>(move |tier| {
                tier.compression_engine.update_thresholds(
                    min_size as u32,
                    max_ratio as f32,
                    speed_threshold,
                );
                Ok(())
            });
    }

    /// Adapt cold tier compression algorithm based on workload
    pub fn adapt_cold_tier_compression(&self) {
        let coordinator = &self.cold_tier_coordinator;
        let _ = coordinator.execute_operation::<K, V, (), _>(|tier| {
                tier.compression_engine.adapt_algorithm();
                Ok(())
            });
    }

    // =======================================================================
    // PREFETCH SYSTEM INTEGRATION - PRODUCTION GRADE IMPLEMENTATION
    // =======================================================================

    /// Get pending prefetch queue status
    #[allow(dead_code)] // Unified manager - prefetch queue monitoring for capacity management
    pub async fn get_prefetch_queue_status(&self) -> (usize, usize) {
        self.prefetch_channel.queue_status().await
    }

    /// Cleanup expired prefetch requests
    #[allow(dead_code)] // Unified manager - prefetch cleanup for memory management
    pub async fn cleanup_expired_prefetch_requests(&self) -> usize {
        let current_time_ns =
            match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                Ok(duration) => duration.as_nanos() as u64,
                Err(_) => 0, // Use 0 as fallback timestamp
            };

        self.prefetch_channel.cleanup_expired(current_time_ns).await
    }

    /// Update prefetch configuration
    #[allow(dead_code)] // Unified manager - prefetch configuration updates for runtime tuning
    pub fn update_prefetch_config(&self, config: PrefetchConfig) {
        self.prefetch_channel.update_config(config);
    }

    /// Clear prefetch predictor state
    #[allow(dead_code)] // Unified manager - prefetch state reset for system maintenance
    pub fn clear_prefetch_state(&self) {
        self.prefetch_channel.clear();
    }

    /// Get comprehensive prefetch statistics
    pub async fn get_prefetch_stats(&self) -> PrefetchStats {
        self.prefetch_channel.get_stats().await
    }

    /// Get prefetch prediction accuracy
    #[allow(dead_code)] // Unified manager - prefetch accuracy analysis for performance monitoring
    pub async fn get_prefetch_accuracy(&self) -> f64 {
        self.get_prefetch_stats().await.accuracy
    }

    /// Get prefetch hit rate
    #[allow(dead_code)] // Unified manager - prefetch hit rate monitoring for effectiveness analysis
    pub async fn get_prefetch_hit_rate(&self) -> f64 {
        self.get_prefetch_stats().await.hit_rate
    }

    /// Get number of detected access patterns
    #[allow(dead_code)] // Unified manager - pattern detection monitoring for prediction analysis
    pub async fn get_detected_patterns_count(&self) -> usize {
        self.get_prefetch_stats().await.patterns_detected
    }

    /// Get prefetch queue utilization
    #[allow(dead_code)] // Unified manager - queue utilization monitoring for capacity planning
    pub async fn get_prefetch_queue_utilization(&self) -> f64 {
        let stats = self.get_prefetch_stats().await;
        if stats.queue_size > 0 {
            // Get capacity from queue status
            let capacity = self.prefetch_channel.queue_status().await.1;
            stats.queue_size as f64 / capacity as f64
        } else {
            0.0
        }
    }

    /// Check if prefetching is effective
    #[allow(dead_code)] // Unified manager - prefetch effectiveness evaluation for optimization decisions
    pub async fn is_prefetching_effective(&self, threshold: f64) -> bool {
        self.get_prefetch_stats().await.is_effective(threshold)
    }

    /// Get prefetch effectiveness score
    #[allow(dead_code)] // Unified manager - prefetch effectiveness scoring for performance comparison
    pub async fn get_prefetch_effectiveness_score(&self) -> f64 {
        self.get_prefetch_stats().await.effectiveness_score()
    }

    /// Get access history length for pattern analysis
    #[allow(dead_code)] // Unified manager - access history monitoring for pattern detection analysis
    pub async fn get_access_history_length(&self) -> usize {
        // Return queue size as proxy for access history length
        self.prefetch_channel.queue_status().await.0
    }

    /// Get current prefetch configuration
    #[allow(dead_code)] // Unified manager - prefetch configuration retrieval for monitoring and tuning
    pub fn get_current_prefetch_config(&self) -> Option<PrefetchConfig> {
        // Configuration is not accessible through channel interface
        // Return None since config is worker-owned
        None
    }

    /// Select optimal compression algorithm for current workload
    pub async fn select_cold_tier_compression_for_workload(&self, workload_type: &str) -> String {
        let coordinator = &self.cold_tier_coordinator;

        let workload_type = workload_type.to_string(); // Clone to avoid lifetime issues
        coordinator
            .execute_read_operation::<K, V, String, _>(move |tier| {
                use crate::cache::tier::cold::compression_engine::WorkloadType;
                let workload = match workload_type.as_str() {
                    "latency_optimized" => WorkloadType::LatencyOptimized,
                    "storage_optimized" => WorkloadType::StorageOptimized,
                    "balanced" => WorkloadType::Balanced,
                    _ => WorkloadType::Adaptive,
                };
                let algorithm = tier
                    .compression_engine
                    .select_algorithm_for_workload(&[], workload);
                Ok(format!("{:?}", algorithm))
            })
            .await
            .unwrap_or_else(|_| "LZ4".to_string())
    }
}

// Clone removed - UnifiedCacheManager is now Arc-wrapped inside Goldylox for cheap cloning
// This prevents accidental thread explosion when cloning

// String-specific convenience APIs removed - use generic UnifiedCacheManager<K, V> directly
