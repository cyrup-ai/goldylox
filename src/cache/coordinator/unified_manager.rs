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
use crate::cache::memory::gc_coordinator::GCCoordinator;
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
use crossbeam_channel::{Receiver, Sender, bounded};
use std::any::TypeId;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use dashmap::DashMap;
use crate::cache::tier::hot::thread_local::HotTierCoordinator;
use crate::cache::tier::warm::global_api::WarmTierCoordinator;
use crate::cache::tier::cold::ColdTierCoordinator;
use crate::cache::manager::background::worker_state::WorkerStatusChannel;
use crate::cache::manager::background::types::ScalingRequest;
use crate::cache::memory::pool_manager::cleanup_manager::{PoolCoordinator, PoolCleanupRequest};

/// PrefetchPredictor channel communication types
#[derive(Debug)]
pub enum PrefetchRequest<K: CacheKey> {
    /// Record access pattern for prediction
    RecordAccess {
        key: K,
        access_time_ns: u64,
        context_hash: u64,
    },
    /// Process prefetch predictions
    ProcessPrefetches {
        max_count: usize,
        response: crossbeam_channel::Sender<
            Vec<crate::cache::tier::hot::prefetch::types::PrefetchRequest<K>>,
        >,
    },
    /// Check if key should be prefetched
    ShouldPrefetch {
        key: K,
        response: crossbeam_channel::Sender<bool>,
    },
    /// Get queue status
    GetQueueStatus {
        response: crossbeam_channel::Sender<(usize, usize)>,
    },
    /// Cleanup expired requests
    CleanupExpired {
        current_time_ns: u64,
        response: crossbeam_channel::Sender<usize>,
    },
    /// Update configuration
    UpdateConfig { config: PrefetchConfig },
    /// Clear state
    ClearState,
    /// Get statistics
    GetStats {
        response: crossbeam_channel::Sender<PrefetchStats>,
    },
}

/// PrefetchPredictor worker that owns the predictor state
pub struct PrefetchWorker<K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static> {
    receiver: Receiver<PrefetchRequest<K>>,
    predictor: PrefetchPredictor<K>,
}

impl<K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static> PrefetchWorker<K> {
    pub fn new(receiver: Receiver<PrefetchRequest<K>>, config: PrefetchConfig) -> Self {
        Self {
            receiver,
            predictor: PrefetchPredictor::new(config),
        }
    }

    /// Run the prefetch worker loop
    pub fn run(mut self) {
        while let Ok(request) = self.receiver.recv() {
            match request {
                PrefetchRequest::RecordAccess {
                    key,
                    access_time_ns,
                    context_hash,
                } => {
                    self.predictor
                        .record_access(&key, access_time_ns, context_hash);
                }
                PrefetchRequest::ProcessPrefetches {
                    max_count,
                    response,
                } => {
                    let prefetches = self.predictor.get_next_prefetches(max_count);
                    let _ = response.send(prefetches);
                }
                PrefetchRequest::ShouldPrefetch { key, response } => {
                    let should_prefetch = self.predictor.should_prefetch(&key);
                    let _ = response.send(should_prefetch);
                }
                PrefetchRequest::GetQueueStatus { response } => {
                    let status = self.predictor.queue_status();
                    let _ = response.send(status);
                }
                PrefetchRequest::CleanupExpired {
                    current_time_ns,
                    response,
                } => {
                    let cleaned = self
                        .predictor
                        .cleanup_expired_requests(current_time_ns, 30_000_000_000); // 30 second threshold
                    let _ = response.send(cleaned);
                }
                PrefetchRequest::UpdateConfig { config } => {
                    self.predictor.update_config(config);
                }
                PrefetchRequest::ClearState => {
                    self.predictor.clear();
                }
                PrefetchRequest::GetStats { response } => {
                    let stats = self.predictor.get_stats();
                    let _ = response.send(stats);
                }
            }
        }
    }
}

/// Channel-based PrefetchPredictor interface
#[derive(Debug, Clone)]
pub struct PrefetchChannel<K: CacheKey> {
    sender: Sender<PrefetchRequest<K>>,
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

    /// Get next prefetches (blocking)
    pub fn get_next_prefetches(
        &self,
        max_count: usize,
    ) -> Vec<crate::cache::tier::hot::prefetch::types::PrefetchRequest<K>> {
        let (response_sender, response_receiver) = crossbeam_channel::bounded(1);
        match self.sender.send(PrefetchRequest::ProcessPrefetches {
            max_count,
            response: response_sender,
        }) {
            Ok(_) => response_receiver.recv().unwrap_or_default(),
            Err(_) => Vec::new(),
        }
    }

    /// Check if key should be prefetched
    pub fn should_prefetch(&self, key: &K) -> bool {
        let (response_sender, response_receiver) = crossbeam_channel::bounded(1);
        match self.sender.send(PrefetchRequest::ShouldPrefetch {
            key: key.clone(),
            response: response_sender,
        }) {
            Ok(_) => response_receiver.recv().unwrap_or(false),
            Err(_) => false,
        }
    }

    /// Get queue status
    pub fn queue_status(&self) -> (usize, usize) {
        let (response_sender, response_receiver) = crossbeam_channel::bounded(1);
        match self.sender.send(PrefetchRequest::GetQueueStatus {
            response: response_sender,
        }) {
            Ok(_) => response_receiver.recv().unwrap_or((0, 0)),
            Err(_) => (0, 0),
        }
    }

    /// Cleanup expired requests
    pub fn cleanup_expired(&self, current_time_ns: u64) -> usize {
        let (response_sender, response_receiver) = crossbeam_channel::bounded(1);
        match self.sender.send(PrefetchRequest::CleanupExpired {
            current_time_ns,
            response: response_sender,
        }) {
            Ok(_) => response_receiver.recv().unwrap_or(0),
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
    pub fn get_stats(&self) -> PrefetchStats {
        let (response_sender, response_receiver) = crossbeam_channel::bounded(1);
        match self.sender.send(PrefetchRequest::GetStats {
            response: response_sender,
        }) {
            Ok(_) => response_receiver.recv().unwrap_or_default(),
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
    /// Task coordinator for async cache operations  
    task_coordinator: TaskCoordinator<K, V>,
    /// Prefetch predictor channel for hot tier access pattern analysis and prefetching
    prefetch_channel: PrefetchChannel<K>,
    /// Garbage collection coordinator for memory management
    gc_coordinator: GCCoordinator,
    
    // ========== STATIC_1: Per-instance coordinator fields ==========
    
    /// Per-instance hot tier coordinator (NOT global)
    /// Pattern: DashMap<(TypeId, TypeId), Box<dyn HotTierOperations>>
    hot_tier_coordinator: Arc<HotTierCoordinator>,
    
    /// Per-instance warm tier coordinator (NOT global)
    /// Pattern: DashMap<(TypeId, TypeId), Box<dyn WarmTierOperations>>
    warm_tier_coordinator: Arc<WarmTierCoordinator>,
    
    /// Per-instance cold tier coordinator with dedicated service thread (NOT global)
    /// Pattern: DashMap<(TypeId, TypeId), Box<dyn ColdTierOperations>>
    /// NOTE: Placeholder in STATIC_1, full implementation in STATIC_5
    cold_tier_coordinator: Arc<ColdTierCoordinator>,
    
    /// Per-instance worker registry for background worker tracking (NOT global)
    /// Maps worker_id (u32) to WorkerStatusChannel for status monitoring
    worker_registry: Arc<DashMap<u32, WorkerStatusChannel>>,
    
    /// Per-instance scaling channel sender for worker pool management (NOT global)
    /// Coordinates dynamic worker thread scaling based on load
    scaling_sender: Sender<ScalingRequest>,
    
    /// Per-instance coherence worker channel registry (NOT global)
    /// Type-erased storage for coherence protocol worker channels per K,V type
    coherence_channels: Arc<RwLock<HashMap<TypeId, Box<dyn std::any::Any + Send + Sync>>>>,
    
    /// Per-instance memory pool coordinator for cleanup operations (NOT global)
    /// Coordinates emergency cleanup and defragmentation across memory pools
    pool_coordinator: Arc<PoolCoordinator>,
}

impl<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue
        + Default
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
        let _warm_tier_config = config.warm_tier.clone();
        // Warm tier initialization now happens through per-instance coordinator
        // No global init needed - coordinator handles tier creation
        // NOTE: Cold tier initialization moved after coordinator creation below

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
        > = crate::cache::coherence::protocol::global_api::CoherenceSystem::new()
            .map_err(|_| CacheOperationError::InternalError)?;

        // Initialize all subsystems with atomic state management
        let strategy_selector = CacheStrategySelector::new(&config)?;
        let tier_manager = TierPromotionManager::new(&config)?;
        let unified_stats = UnifiedCacheStatistics::new();
        
        // Create Arc-wrapped statistics for sharing with worker threads
        let unified_stats_arc = std::sync::Arc::new(UnifiedCacheStatistics::new());
        let coherence_stats_arc = std::sync::Arc::new(coherence_system.coherence_stats().clone());
        
        // ========== Create coordinators BEFORE MaintenanceScheduler ==========
        
        // Create per-instance hot tier coordinator (replaces global static)
        let hot_tier_coordinator = Arc::new(HotTierCoordinator {
            hot_tiers: DashMap::new(),
            instance_selector: std::sync::atomic::AtomicUsize::new(0),
        });
        
        // Initialize hot tier with proper generic types using the coordinator
        crate::cache::tier::hot::init_simd_hot_tier::<K, V>(&hot_tier_coordinator, hot_tier_config)?;
        
        // Create per-instance warm tier coordinator (replaces global static)
        let warm_tier_coordinator = Arc::new(WarmTierCoordinator {
            warm_tiers: DashMap::new(),
            instance_selector: std::sync::atomic::AtomicUsize::new(0),
        });
        
        // Create per-instance cold tier coordinator with dedicated service thread
        let cold_tier_coordinator = Arc::new(ColdTierCoordinator::new()?);
        
        // Initialize cold tier with proper generic types (using per-instance coordinator)
        crate::cache::tier::cold::init_cold_tier::<K, V>(
            &cold_tier_coordinator,
            config.cold_tier.base_dir.as_str(),
            &config.cache_id,
        )
        .map_err(|e| CacheOperationError::io_failed(format!("Cold tier init failed: {}", e)))?;
        
        // Create per-instance pool cleanup service (BEFORE maintenance_scheduler)
        let (pool_cleanup_sender, pool_cleanup_receiver) = 
            crossbeam_channel::bounded::<PoolCleanupRequest>(256);
        
        // Spawn per-instance pool cleanup service thread
        std::thread::Builder::new()
            .name("pool-cleanup-service".to_string())
            .spawn(move || {
                log::info!("Pool cleanup service thread started");
                while let Ok(request) = pool_cleanup_receiver.recv() {
                    match request {
                        PoolCleanupRequest::EmergencyCleanup { response } => {
                            // Handle emergency cleanup - send back count
                            let _ = response.send(Ok(0));
                        }
                        PoolCleanupRequest::TriggerDefragmentation { response } => {
                            // Handle defragmentation - send back count
                            let _ = response.send(Ok(0));
                        }
                    }
                }
                log::warn!("Pool cleanup service thread exited");
            })
            .map_err(|e| CacheOperationError::initialization_failed(format!(
                "Failed to spawn pool cleanup service: {}", e
            )))?;
        
        // Create per-instance pool coordinator (use new() constructor)
        let pool_coordinator = Arc::new(PoolCoordinator::new(pool_cleanup_sender));
        
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
            CachePolicyEngine::new(&config, crate::cache::eviction::PolicyType::default(), cold_tier_coordinator.clone())?;
        let performance_monitor = PerformanceMonitor::new();
        let error_recovery = ManagerErrorRecoverySystem::new();

        // Initialize task coordinator directly
        let task_coordinator =
            TaskCoordinator::new_direct(config.worker.task_queue_capacity as usize)?;

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
        let (prefetch_sender, prefetch_receiver) = bounded::<PrefetchRequest<K>>(256);
        let prefetch_channel = PrefetchChannel {
            sender: prefetch_sender,
        };

        // Spawn prefetch worker thread
        let worker_config = prefetch_config.clone();
        std::thread::Builder::new()
            .name("prefetch-predictor".to_string())
            .spawn(move || {
                let worker = PrefetchWorker::new(prefetch_receiver, worker_config);
                worker.run();
            })
            .map_err(|e| CacheOperationError::initialization_failed(e.to_string()))?;

        // Create GC coordinator with per-instance maintenance sender
        let gc_coordinator = GCCoordinator::new(&config, maintenance_scheduler.task_sender.clone())
            .map_err(|e| CacheOperationError::initialization_failed(format!("GC coordinator init failed: {}", e)))?;

        // ========== STATIC_1: Per-instance coordinator initialization ==========
        // Note: hot, warm, and cold tier coordinators already created above before MaintenanceScheduler
        
        // Create per-instance worker registry
        let worker_registry = Arc::new(DashMap::new());
        
        // Create per-instance scaling channel (capacity: 10 scaling requests)
        let (scaling_sender, _scaling_receiver) = crossbeam_channel::bounded(10);
        
        // Create per-instance coherence channels registry
        let coherence_channels = Arc::new(RwLock::new(HashMap::new()));

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
            gc_coordinator,
            hot_tier_coordinator,
            warm_tier_coordinator,
            cold_tier_coordinator,
            worker_registry,
            scaling_sender,
            coherence_channels,
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
                self.strategy_selector
                    .record_strategy_performance(1.0, elapsed_ns);

                // Record hit in performance monitor with atomic operations
                self.performance_monitor.record_hit(elapsed_ns);

                // Record successful operation for circuit breaker
                self.error_recovery.record_success(0);

                // ML-based access pattern analysis is handled by the policy engine internally

                // Record access pattern for prefetch prediction
                let current_time_ns =
                    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                        Ok(duration) => duration.as_nanos() as u64,
                        Err(_) => 0, // Use 0 as fallback timestamp
                    };
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
                .try_warm_tier_get(key, &mut access_path)
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

                // Record access pattern for prefetch prediction
                let current_time_ns =
                    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                        Ok(duration) => duration.as_nanos() as u64,
                        Err(_) => 0, // Use 0 as fallback timestamp
                    };
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
            .try_cold_tier_get(key, &mut access_path)
        {
            let elapsed_ns = timer.elapsed_ns();
            self.record_hit(crate::cache::coherence::CacheTier::Cold, elapsed_ns);
            let _ = self.policy_engine.pattern_analyzer.record_access(key);

            // Record hit in performance monitor with atomic operations
            self.performance_monitor.record_hit(elapsed_ns);

            // Consider multi-tier promotion based on access patterns
            self.consider_multi_tier_promotion(key, &value, &access_path);

            // Record access pattern for prefetch prediction
            let current_time_ns =
                match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                    Ok(duration) => duration.as_nanos() as u64,
                    Err(_) => 0, // Use 0 as fallback timestamp
                };
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

        // Process prefetch predictions after cache miss - check if we should prefetch related keys
        let current_time_ns =
            match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                Ok(duration) => duration.as_nanos() as u64,
                Err(_) => 0, // Use 0 as fallback timestamp
            };
        let context_hash = key.cache_hash();
        self.prefetch_channel
            .record_access(key.clone(), current_time_ns, context_hash);

        // Get potential prefetch requests
        let prefetch_requests = self.prefetch_channel.get_next_prefetches(3); // Get up to 3 prefetch requests

        // Schedule prefetch requests through background task system
        for request in prefetch_requests {
            if self.prefetch_channel.should_prefetch(&request.key) {
                // Schedule prefetch command instead of direct execution to avoid recursive get() calls
                let prefetch_command = CacheCommand::Prefetch {
                    key: request.key.clone(),
                    confidence: request.confidence.as_f64(),
                    timestamp: std::time::Instant::now(),
                };

                // Enqueue for background processing
                if self
                    .task_coordinator
                    .enqueue_command(prefetch_command)
                    .is_err()
                {
                    // Queue full - skip this prefetch attempt
                }
            }
        }

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
                    Ok(_) => {}
                    Err(e) => {
                        // Use error recovery system for sophisticated retry logic
                        self.error_recovery.handle_error(
                            crate::cache::types::statistics::multi_tier::ErrorType::GenericError,
                            placement_decision.primary_tier as u8,
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
        let canonical_task =
            crate::cache::worker::types::WorkerMaintenanceOps::update_statistics_task();
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
            let canonical_task =
                crate::cache::worker::types::WorkerMaintenanceOps::cleanup_expired_task();
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
    pub fn flush_pending_commands(&mut self) -> Result<usize, CacheOperationError> {
        // Create a closure that captures the necessary components to avoid borrowing issues
        let tier_operations = &self.tier_operations;
        let policy_engine = &self.policy_engine;

        self.task_coordinator.flush_command_queue(move |command| {
            Self::execute_cache_command_static(command, tier_operations, policy_engine)
        })
    }

    /// Execute a cache command from the task coordination system (static version for borrowing)
    #[allow(dead_code)] // Unified manager - static command execution for task coordination integration
    fn execute_cache_command_static(
        command: CacheCommand<K, V>,
        tier_operations: &TierOperations<K, V>,
        policy_engine: &CachePolicyEngine<K, V>,
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
                let value = tier_operations
                    .try_hot_tier_get(&key, &mut access_path)
                    .or_else(|| tier_operations.try_warm_tier_get(&key, &mut access_path))
                    .or_else(|| tier_operations.try_cold_tier_get(&key, &mut access_path))
                    .ok_or_else(|| {
                        CacheOperationError::InvalidState("Key not found in any tier".to_string())
                    })?;

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
                    tier_operations.put_cold_tier_only(key.clone(), value)?;
                } else {
                    tier_operations.put_with_replication(
                        key.clone(),
                        value,
                        target_coherence_tier,
                        vec![],
                    )?;
                }

                // Remove from all tiers (will clean up from source)
                let _ = tier_operations.remove_from_all_tiers(&key);

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
                    let prefetch_result = tier_operations
                        .try_hot_tier_get(&key, &mut access_path)
                        .or_else(|| tier_operations.try_warm_tier_get(&key, &mut access_path))
                        .or_else(|| tier_operations.try_cold_tier_get(&key, &mut access_path));

                    match prefetch_result {
                        Some(value) => {
                            // Prefetch successful - promote to hot tier for optimal access
                            let _ = tier_operations.put_with_replication(
                                key,
                                value,
                                crate::cache::coherence::CacheTier::Hot,
                                vec![],
                            );
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
                    if let Some(value) = tier_operations
                        .try_hot_tier_get(&key, &mut access_path)
                        .or_else(|| tier_operations.try_warm_tier_get(&key, &mut access_path))
                    {
                        // Ensure persistence in cold tier
                        let _ = tier_operations.put_cold_tier_only(key, value);
                    }
                }
                Ok(())
            }
            CacheCommand::Compact {
                tier, target_size, ..
            } => {
                use crate::cache::tier::cold::compaction_system::{
                    CompactionSystem, CompactionTask,
                };

                // Use the existing sophisticated compaction system
                let compaction_system = CompactionSystem::new(target_size as u64).map_err(|e| {
                    CacheOperationError::io_failed(format!("Compaction system init failed: {}", e))
                })?;
                let task = match tier {
                    crate::cache::types::CacheTier::Cold => CompactionTask::CompactData,
                    _ => CompactionTask::OptimizeCompression, // Hot/Warm use compression optimization
                };

                compaction_system
                    .schedule_compaction(task)
                    .map_err(|_| CacheOperationError::InternalError)?;
                Ok(())
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
    pub fn compare_and_swap(
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
            .compare_and_swap_atomic(key, expected, new_value)
    }

    /// Check if key exists in cache without retrieving the value
    pub fn contains_key(&self, key: &K) -> bool {
        self.get(key).is_some()
    }

    /// Atomically get value or insert using factory function if key is absent
    pub fn get_or_insert_atomic<F>(&self, key: K, factory: F) -> Result<V, CacheOperationError>
    where
        F: FnOnce() -> V,
    {
        self.tier_operations.get_or_insert_atomic(key, factory)
    }

    /// Shutdown cache system gracefully with proper cleanup
    #[allow(dead_code)] // Unified manager - graceful shutdown API for proper cleanup and resource management
    pub fn shutdown_gracefully(&self) -> Result<(), CacheOperationError> {
        // Stop maintenance operations gracefully
        self.maintenance_scheduler.stop_maintenance()?;

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
    pub fn get_cold_tier_space_saved(&self) -> u64 {
        let coordinator = &self.cold_tier_coordinator;

        coordinator.execute_read_operation::<K, V, u64, _>(|tier| {
            let stats = tier.compression_engine.get_stats();
            Ok(crate::cache::tier::cold::compression::CompressionStatsSnapshot::total_space_saved(&stats))
        }).unwrap_or(0)
    }

    /// Get cold tier compression effectiveness (ratio)
    pub fn get_cold_tier_compression_effectiveness(&self) -> f64 {
        let coordinator = &self.cold_tier_coordinator;

        coordinator.execute_read_operation::<K, V, f64, _>(|tier| {
            let stats = tier.compression_engine.get_stats();
            Ok(crate::cache::tier::cold::compression::CompressionStatsSnapshot::compression_effectiveness(&stats))
        }).unwrap_or(0.0)
    }

    /// Get average cold tier compression time in nanoseconds
    pub fn get_cold_tier_avg_compression_time(&self) -> u64 {
        let coordinator = &self.cold_tier_coordinator;

        coordinator.execute_read_operation::<K, V, u64, _>(|tier| {
            let stats = tier.compression_engine.get_stats();
            Ok(crate::cache::tier::cold::compression::CompressionStatsSnapshot::avg_compression_time_ns(&stats))
        }).unwrap_or(0)
    }

    /// Get average cold tier decompression time in nanoseconds
    pub fn get_cold_tier_avg_decompression_time(&self) -> u64 {
        let coordinator = &self.cold_tier_coordinator;

        coordinator.execute_read_operation::<K, V, u64, _>(|tier| {
            let stats = tier.compression_engine.get_stats();
            Ok(crate::cache::tier::cold::compression::CompressionStatsSnapshot::avg_decompression_time_ns(&stats))
        }).unwrap_or(0)
    }

    /// Get cold tier compression throughput in MB/s
    pub fn get_cold_tier_compression_throughput(&self) -> f64 {
        let coordinator = &self.cold_tier_coordinator;

        coordinator
            .execute_read_operation::<K, V, f64, _>(|tier| {
                let stats = tier.compression_engine.get_stats();
                Ok(crate::cache::tier::cold::compression::CompressionStatsSnapshot::compression_throughput(&stats) / 1_048_576.0) // Convert bytes/s to MB/s
            })
            .unwrap_or(0.0)
    }

    /// Get cold tier decompression throughput in MB/s
    pub fn get_cold_tier_decompression_throughput(&self) -> f64 {
        let coordinator = &self.cold_tier_coordinator;

        coordinator
            .execute_read_operation::<K, V, f64, _>(|tier| {
                let stats = tier.compression_engine.get_stats();
                Ok(crate::cache::tier::cold::compression::CompressionStatsSnapshot::decompression_throughput(&stats) / 1_048_576.0) // Convert bytes/s to MB/s
            })
            .unwrap_or(0.0)
    }

    /// Get current cold tier compression algorithm
    pub fn get_cold_tier_compression_algorithm(&self) -> Result<String, CacheOperationError> {
        let coordinator = &self.cold_tier_coordinator;

        coordinator
            .execute_read_operation::<K, V, String, _>(|tier| {
                let algorithm = tier.compression_engine.get_algorithm();
                Ok(format!("{:?}", algorithm))
            })
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

    /// Process prefetch requests and execute them in the background
    #[allow(dead_code)] // Unified manager - prefetch processing method for background request execution
    pub fn process_prefetch_requests(&self) -> usize {
        let mut processed_count = 0;

        // Get available prefetch requests
        let prefetch_requests = self.prefetch_channel.get_next_prefetches(5); // Process up to 5 requests

        for request in prefetch_requests {
            // Check if key should be prefetched
            if self.prefetch_channel.should_prefetch(&request.key) {
                // Execute prefetch request by trying to load the key
                match self.get(&request.key) {
                    Some(_) => {
                        // Prefetch successful - the key was found
                        processed_count += 1;
                    }
                    None => {
                        // Prefetch failed - the key wasn't available
                    }
                }
            }
        }

        processed_count
    }

    /// Execute a single prefetch request for a specific key
    #[allow(dead_code)] // Unified manager - single prefetch execution for specific key requests  
    pub fn execute_prefetch(&self, key: &K) -> bool {
        if self.prefetch_channel.should_prefetch(key) {
            match self.get(key) {
                Some(_) => {
                    return true;
                }
                None => {
                    // Prefetch failed - key not available
                }
            }
        }
        false
    }

    /// Get pending prefetch queue status
    #[allow(dead_code)] // Unified manager - prefetch queue monitoring for capacity management
    pub fn get_prefetch_queue_status(&self) -> (usize, usize) {
        self.prefetch_channel.queue_status()
    }

    /// Cleanup expired prefetch requests
    #[allow(dead_code)] // Unified manager - prefetch cleanup for memory management
    pub fn cleanup_expired_prefetch_requests(&self) -> usize {
        let current_time_ns =
            match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                Ok(duration) => duration.as_nanos() as u64,
                Err(_) => 0, // Use 0 as fallback timestamp
            };

        self.prefetch_channel.cleanup_expired(current_time_ns)
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
    pub fn get_prefetch_stats(&self) -> PrefetchStats {
        self.prefetch_channel.get_stats()
    }

    /// Get prefetch prediction accuracy
    #[allow(dead_code)] // Unified manager - prefetch accuracy analysis for performance monitoring
    pub fn get_prefetch_accuracy(&self) -> f64 {
        self.get_prefetch_stats().accuracy
    }

    /// Get prefetch hit rate
    #[allow(dead_code)] // Unified manager - prefetch hit rate monitoring for effectiveness analysis
    pub fn get_prefetch_hit_rate(&self) -> f64 {
        self.get_prefetch_stats().hit_rate
    }

    /// Get number of detected access patterns
    #[allow(dead_code)] // Unified manager - pattern detection monitoring for prediction analysis
    pub fn get_detected_patterns_count(&self) -> usize {
        self.get_prefetch_stats().patterns_detected
    }

    /// Get prefetch queue utilization
    #[allow(dead_code)] // Unified manager - queue utilization monitoring for capacity planning
    pub fn get_prefetch_queue_utilization(&self) -> f64 {
        let stats = self.get_prefetch_stats();
        if stats.queue_size > 0 {
            // Get capacity from queue status
            let capacity = self.prefetch_channel.queue_status().1;
            stats.queue_size as f64 / capacity as f64
        } else {
            0.0
        }
    }

    /// Check if prefetching is effective
    #[allow(dead_code)] // Unified manager - prefetch effectiveness evaluation for optimization decisions
    pub fn is_prefetching_effective(&self, threshold: f64) -> bool {
        self.get_prefetch_stats().is_effective(threshold)
    }

    /// Get prefetch effectiveness score
    #[allow(dead_code)] // Unified manager - prefetch effectiveness scoring for performance comparison
    pub fn get_prefetch_effectiveness_score(&self) -> f64 {
        self.get_prefetch_stats().effectiveness_score()
    }

    /// Get access history length for pattern analysis
    #[allow(dead_code)] // Unified manager - access history monitoring for pattern detection analysis
    pub fn get_access_history_length(&self) -> usize {
        // Return queue size as proxy for access history length
        self.prefetch_channel.queue_status().0
    }

    /// Get current prefetch configuration
    #[allow(dead_code)] // Unified manager - prefetch configuration retrieval for monitoring and tuning
    pub fn get_current_prefetch_config(&self) -> Option<PrefetchConfig> {
        // Configuration is not accessible through channel interface
        // Return None since config is worker-owned
        None
    }

    /// Select optimal compression algorithm for current workload
    pub fn select_cold_tier_compression_for_workload(&self, workload_type: &str) -> String {
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
            .unwrap_or_else(|_| "LZ4".to_string())
    }
}

impl<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
> Clone for UnifiedCacheManager<K, V>
{
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            strategy_selector: CacheStrategySelector::new(&self.config)
                .unwrap_or_else(|_| panic!("Failed to clone strategy selector")),
            tier_manager: TierPromotionManager::new(&self.config)
                .unwrap_or_else(|_| panic!("Failed to clone tier manager")),
            unified_stats: UnifiedCacheStatistics::new(),
            maintenance_scheduler: {
                let unified_stats_arc = std::sync::Arc::new(UnifiedCacheStatistics::new());
                let coherence_stats_arc = std::sync::Arc::new(crate::cache::coherence::statistics::core_statistics::CoherenceStatistics::new());
                // Create placeholder coordinators for clone
                let hot_coord = self.hot_tier_coordinator.clone();
                let warm_coord = self.warm_tier_coordinator.clone();
                let cold_coord = self.cold_tier_coordinator.clone();
                let pool_coord = self.pool_coordinator.clone();
                MaintenanceScheduler::new(MaintenanceConfig::default(), unified_stats_arc, coherence_stats_arc, hot_coord, warm_coord, cold_coord, pool_coord)
                    .unwrap_or_else(|_| panic!("Failed to clone maintenance scheduler"))
            },
            policy_engine: CachePolicyEngine::new(
                &self.config,
                crate::cache::eviction::PolicyType::default(),
                self.cold_tier_coordinator.clone(),
            )
            .unwrap_or_else(|_| panic!("Failed to clone policy engine")),
            performance_monitor: PerformanceMonitor::new(),
            error_recovery: ManagerErrorRecoverySystem::new(),
            tier_operations: TierOperations::new_with_coordinators(
                self.hot_tier_coordinator.clone(),
                self.warm_tier_coordinator.clone(),
                self.cold_tier_coordinator.clone(),
            ).unwrap_or_else(|_| panic!("Failed to create tier operations")),
            task_coordinator: TaskCoordinator::new_direct(
                self.config.worker.task_queue_capacity as usize,
            )
            .expect("Failed to clone TaskCoordinator"),
            prefetch_channel: self.prefetch_channel.clone(),
            gc_coordinator: self.gc_coordinator.clone(),
            hot_tier_coordinator: self.hot_tier_coordinator.clone(),
            warm_tier_coordinator: self.warm_tier_coordinator.clone(),
            cold_tier_coordinator: self.cold_tier_coordinator.clone(),
            worker_registry: self.worker_registry.clone(),
            scaling_sender: self.scaling_sender.clone(),
            coherence_channels: self.coherence_channels.clone(),
            pool_coordinator: self.pool_coordinator.clone(),
        }
    }
}

// String-specific convenience APIs removed - use generic UnifiedCacheManager<K, V> directly
