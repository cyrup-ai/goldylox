//! Simple public API for Goldylox cache system
//!
//! This provides a user-friendly interface while leveraging all the sophisticated
//! internal infrastructure (multi-tier, ML eviction, SIMD, coherence protocols).
//!
//! Users specify both key and value types `Goldylox<K, V>` for full type safety
//! and direct access to all advanced cache features.

use serde::{Serialize, de::DeserializeOwned};
use std::fmt::Debug;
use std::hash::Hash;

use crate::cache::config::CacheConfig;
use crate::cache::config::types::generate_storage_path;
use crate::cache::coordinator::unified_manager::UnifiedCacheManager;
use crate::cache::serde::{SerdeCacheKey, SerdeCacheValue};
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};

/// Summary of batch operation results with timing and success metrics
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BatchOperationSummary<T> {
    /// Successfully retrieved/processed items
    pub successful_results: Vec<T>,
    /// Number of failed operations
    pub failed_count: usize,
    /// Total number of operations attempted
    pub total_operations: usize,
    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,
    /// Total time for all operations in nanoseconds
    pub total_time_ns: u64,
    /// Average latency per operation in nanoseconds
    pub avg_latency_ns: u64,
    /// Minimum operation latency in nanoseconds
    pub min_latency_ns: u64,
    /// Maximum operation latency in nanoseconds
    pub max_latency_ns: u64,
}

impl<T> BatchOperationSummary<T> {
    /// Get throughput in operations per second
    pub fn throughput_ops_per_sec(&self) -> f64 {
        if self.total_time_ns > 0 {
            (self.total_operations as f64) / (self.total_time_ns as f64 / 1_000_000_000.0)
        } else {
            0.0
        }
    }

    /// Get latency statistics in milliseconds
    pub fn latency_stats_ms(&self) -> (f64, f64, f64) {
        (
            self.avg_latency_ns as f64 / 1_000_000.0,
            self.min_latency_ns as f64 / 1_000_000.0,
            self.max_latency_ns as f64 / 1_000_000.0,
        )
    }

    /// Check if all operations succeeded
    pub fn all_succeeded(&self) -> bool {
        self.success_rate == 1.0
    }

    /// Check if any operations succeeded
    pub fn any_succeeded(&self) -> bool {
        self.success_rate > 0.0
    }
}

/// Task coordination statistics for background operations
#[derive(Debug, Clone)]
pub struct TaskCoordinatorStats {
    /// Number of active background tasks
    pub active_tasks: usize,
    /// Total tasks completed
    pub completed_tasks: u64,
    /// Total tasks failed
    pub failed_tasks: u64,
    /// Average task execution time in milliseconds
    pub avg_execution_time_ms: f64,
    /// Queue depth
    pub queue_depth: usize,
}

/// Represents an active background task
#[derive(Debug, Clone)]
pub struct ActiveTask {
    /// Task identifier
    pub id: u64,
    /// Task type description
    pub task_type: String,
    /// When the task was started
    pub started_at: u64,
    /// Task status
    pub status: String,
    /// Estimated completion percentage (0.0-1.0)
    pub progress: f64,
}

impl ActiveTask {
    /// Get task ID
    pub fn task_id(&self) -> u64 {
        self.id
    }
}

/// Maintenance operation breakdown statistics
#[derive(Debug, Clone)]
pub struct MaintenanceBreakdown {
    /// Time spent on compaction in milliseconds
    pub compaction_time_ms: u64,
    /// Time spent on eviction in milliseconds
    pub eviction_time_ms: u64,
    /// Time spent on statistics collection in milliseconds
    pub stats_collection_time_ms: u64,
    /// Time spent on memory management in milliseconds
    pub memory_management_time_ms: u64,
    /// Total maintenance cycles completed
    pub total_cycles: u64,
    /// Last maintenance timestamp
    pub last_maintenance_ns: u64,
}

/// Simple, user-friendly cache interface with homogeneous key-value storage
///
/// Users specify both key and value types for full type safety and direct access
/// to all sophisticated cache features: ML eviction, SIMD optimization, coherence protocols.
pub struct Goldylox<K, V>
where
    K: Serialize
        + DeserializeOwned
        + Clone
        + Hash
        + Eq
        + Ord
        + Send
        + Sync
        + Debug
        + Default
        + 'static,
    V: Serialize + DeserializeOwned + Clone + Send + Sync + Debug + Default + 'static,
{
    // Arc-wrapped manager for cheap cloning without spawning new threads
    manager: std::sync::Arc<UnifiedCacheManager<SerdeCacheKey<K>, SerdeCacheValue<V>>>,
}

impl<K, V> Clone for Goldylox<K, V>
where
    K: Serialize
        + DeserializeOwned
        + Clone
        + Hash
        + Eq
        + Ord
        + Send
        + Sync
        + Debug
        + Default
        + 'static,
    V: Serialize + DeserializeOwned + Clone + Send + Sync + Debug + Default + 'static,
{
    fn clone(&self) -> Self {
        Self {
            manager: self.manager.clone(), // Just Arc clone - cheap!
        }
    }
}

impl<K, V> Goldylox<K, V>
where
    K: Serialize
        + DeserializeOwned
        + Clone
        + Hash
        + Eq
        + Ord
        + Send
        + Sync
        + Debug
        + Default
        + CacheKey
        + 'static,
    V: Serialize + DeserializeOwned + Clone + Send + Sync + Debug + Default + CacheValue + 'static,
{
    /// Create new cache builder with fluent configuration
    pub fn builder() -> GoldyloxBuilder<K, V> {
        GoldyloxBuilder::new()
    }

    /// Create new cache with default configuration
    pub async fn new() -> Result<Self, CacheOperationError> {
        Self::builder().build().await
    }

    /// Store value in cache with direct access to all sophisticated features
    pub async fn put(&self, key: K, value: V) -> Result<(), CacheOperationError> {
        let cache_key = SerdeCacheKey(key);
        let cache_value = SerdeCacheValue(value);

        self.manager.put(cache_key, cache_value).await
    }

    /// Retrieve value from cache with full type safety
    pub async fn get(&self, key: &K) -> Option<V> {
        let cache_key = SerdeCacheKey(key.clone());
        self.manager
            .get(&cache_key).await
            .map(|cache_value| cache_value.0)
    }

    /// Remove value from cache
    pub async fn remove(&self, key: &K) -> bool {
        let cache_key = SerdeCacheKey(key.clone());
        self.manager.remove(&cache_key).await
    }

    /// Clear all entries from cache
    pub async fn clear(&self) -> Result<(), CacheOperationError> {
        self.manager.clear().await
    }

    /// Get the hash value for a key (for testing and debugging)
    pub fn hash_key(&self, key: &K) -> u64 {
        key.cache_hash()
    }

    /// Get cache statistics as formatted string
    pub fn stats(&self) -> Result<String, CacheOperationError> {
        let stats = self.manager.stats();
        Ok(format!(
            "{{\"total_operations\":{},\"overall_hit_rate\":{:.2},\"ops_per_second\":{:.2},\"hot_tier_hits\":{},\"warm_tier_hits\":{},\"cold_tier_hits\":{},\"total_misses\":{},\"avg_access_latency_ns\":{},\"total_memory_usage\":{}}}",
            stats.total_operations,
            stats.overall_hit_rate,
            stats.ops_per_second,
            stats.hot_tier_hits,
            stats.warm_tier_hits,
            stats.cold_tier_hits,
            stats.total_misses,
            stats.avg_access_latency_ns,
            stats.total_memory_usage
        ))
    }

    /// Get detailed cache analytics including access pattern analysis
    pub fn detailed_analytics(&self) -> Result<String, CacheOperationError> {
        let (unified_stats, policy_stats, analyzer_stats) = self.manager.detailed_analytics();
        Ok(format!(
            "{{\"unified\":{},\"policy_hit_rates\":{:?},\"analyzer_tracked_keys\":{},\"analyzer_cleanup_cycles\":{},\"analyzer_memory_usage\":{}}}",
            serde_json::to_string(&unified_stats).unwrap_or_default(),
            policy_stats.hit_rates,
            analyzer_stats.tracked_keys,
            analyzer_stats.total_cleanup_cycles,
            analyzer_stats.memory_usage_bytes
        ))
    }

    /// Get reference to unified cache statistics for per-instance monitoring
    ///
    /// Returns a reference to the UnifiedCacheStatistics instance for this cache,
    /// enabling per-instance statistics tracking and monitoring.
    pub fn get_unified_stats(&self) -> &crate::telemetry::unified_stats::UnifiedCacheStatistics {
        self.manager.get_unified_stats()
    }

    // =======================================================================
    // CONCURRENT CACHE OPERATIONS (Java ConcurrentHashMap style)
    // =======================================================================

    /// Store value only if key is not already present
    /// Returns the previous value if key was present, None if key was absent
    pub async fn put_if_absent(&self, key: K, value: V) -> Result<Option<V>, CacheOperationError> {
        let cache_key = SerdeCacheKey(key);
        let cache_value = SerdeCacheValue(value);

        self.manager
            .put_if_absent(cache_key, cache_value).await
            .map(|opt| opt.map(|cache_value| cache_value.0))
    }

    /// Replace existing value with new value, returning the old value
    /// Returns None if key was not present
    pub async fn replace(&self, key: K, value: V) -> Result<Option<V>, CacheOperationError> {
        let cache_key = SerdeCacheKey(key);
        let cache_value = SerdeCacheValue(value);

        self.manager
            .replace(cache_key, cache_value).await
            .map(|opt| opt.map(|cache_value| cache_value.0))
    }

    /// Atomically replace value only if current value equals expected value
    /// Returns true if replacement occurred, false otherwise
    pub async fn compare_and_swap(
        &self,
        key: K,
        expected: V,
        new_value: V,
    ) -> Result<bool, CacheOperationError>
    where
        V: PartialEq,
    {
        let cache_key = SerdeCacheKey(key);
        let expected_cache_value = SerdeCacheValue(expected);
        let new_cache_value = SerdeCacheValue(new_value);

        self.manager
            .compare_and_swap(cache_key, expected_cache_value, new_cache_value).await
    }

    /// Get value or insert using factory function if key is absent
    /// Returns the existing value if present, or the newly inserted value
    pub async fn get_or_insert<F>(&self, key: K, factory: F) -> Result<V, CacheOperationError>
    where
        F: FnOnce() -> V,
    {
        // Use atomic get-or-insert to avoid race conditions
        let cache_key = SerdeCacheKey(key);
        let result = self
            .manager
            .get_or_insert_atomic(cache_key, || SerdeCacheValue(factory())).await?;
        Ok(result.0)
    }

    /// Get value or insert using fallible factory function if key is absent
    /// Returns the existing value if present, or the newly inserted value
    pub async fn get_or_insert_with<F, E>(&self, key: K, factory: F) -> Result<V, CacheOperationError>
    where
        F: FnOnce() -> Result<V, E>,
        E: Into<CacheOperationError>,
    {
        // First check if key exists (fast path)
        if let Some(existing_value) = self.get(&key).await {
            return Ok(existing_value);
        }

        // Key doesn't exist, create value and use atomic insert
        let new_value = factory().map_err(|e| e.into())?;
        let cache_key = SerdeCacheKey(key);
        let cache_value = SerdeCacheValue(new_value.clone());

        match self.manager.put_if_absent(cache_key, cache_value).await? {
            Some(existing) => Ok(existing.0), // Another thread inserted first
            None => Ok(new_value),            // We successfully inserted
        }
    }

    /// Check if key exists in cache without retrieving the value
    pub async fn contains_key(&self, key: &K) -> bool {
        let cache_key = SerdeCacheKey(key.clone());
        self.manager.contains_key(&cache_key).await
    }

    // =======================================================================
    // COLD TIER COMPRESSION STATISTICS API
    // =======================================================================

    /// Get total space saved by cold tier compression
    pub async fn get_cold_tier_space_saved(&self) -> u64 {
        self.manager.get_cold_tier_space_saved().await
    }

    /// Get cold tier compression effectiveness (ratio)
    pub async fn get_cold_tier_compression_effectiveness(&self) -> f64 {
        self.manager.get_cold_tier_compression_effectiveness().await
    }

    /// Get average cold tier compression time in nanoseconds
    pub async fn get_cold_tier_avg_compression_time(&self) -> u64 {
        self.manager.get_cold_tier_avg_compression_time().await
    }

    /// Get average cold tier decompression time in nanoseconds
    pub async fn get_cold_tier_avg_decompression_time(&self) -> u64 {
        self.manager.get_cold_tier_avg_decompression_time().await
    }

    /// Get cold tier compression throughput in MB/s
    pub async fn get_cold_tier_compression_throughput(&self) -> f64 {
        self.manager.get_cold_tier_compression_throughput().await
    }

    /// Get cold tier decompression throughput in MB/s
    pub async fn get_cold_tier_decompression_throughput(&self) -> f64 {
        self.manager.get_cold_tier_decompression_throughput().await
    }

    /// Get current cold tier compression algorithm
    pub async fn get_cold_tier_compression_algorithm(&self) -> Result<String, CacheOperationError> {
        self.manager.get_cold_tier_compression_algorithm().await
    }

    /// Update cold tier compression thresholds
    pub fn update_cold_tier_compression_thresholds(
        &self,
        min_size: u32,
        max_ratio: f32,
        speed_threshold: f64,
    ) -> Result<(), CacheOperationError> {
        self.manager.update_cold_tier_compression_thresholds(
            min_size as usize,
            max_ratio as f64,
            speed_threshold,
        );
        Ok(())
    }

    /// Adapt cold tier compression algorithm based on workload
    pub fn adapt_cold_tier_compression(&self) -> Result<(), CacheOperationError> {
        self.manager.adapt_cold_tier_compression();
        Ok(())
    }

    /// Select optimal compression algorithm for current workload
    pub async fn select_cold_tier_compression_for_workload(
        &self,
        workload_type: &str,
    ) -> Result<String, CacheOperationError> {
        Ok(self.manager.select_cold_tier_compression_for_workload(workload_type).await)
    }

    // =======================================================================
    // STRATEGY MANAGEMENT API
    // =======================================================================

    /// Get strategy performance metrics for workload analysis
    pub fn strategy_metrics(&self) -> &crate::cache::manager::strategy::StrategyMetrics {
        self.manager.strategy_metrics()
    }

    /// Get strategy thresholds configuration
    pub fn strategy_thresholds(&self) -> &crate::cache::manager::strategy::StrategyThresholds {
        self.manager.strategy_thresholds()
    }

    /// Force strategy change for manual optimization or testing
    pub fn force_cache_strategy(&self, strategy: crate::cache::manager::strategy::CacheStrategy) {
        self.manager.force_cache_strategy(strategy)
    }

    // =======================================================================
    // BATCH OPERATIONS API
    // =======================================================================

    /// Execute batch get operations with comprehensive timing and statistics
    ///
    /// Returns simplified batch result with success/failure counts and timing data
    pub async fn batch_get(&self, keys: Vec<K>) -> BatchOperationSummary<V> {
        let mut successful_results = Vec::new();
        let mut failed_keys = Vec::new();
        let mut total_time_ns = 0u64;
        let mut individual_times = Vec::new();

        for key in keys {
            let start = std::time::Instant::now();
            match self.get(&key).await {
                Some(value) => {
                    let elapsed_ns = start.elapsed().as_nanos() as u64;
                    successful_results.push(value);
                    individual_times.push(elapsed_ns);
                    total_time_ns += elapsed_ns;
                }
                None => {
                    let elapsed_ns = start.elapsed().as_nanos() as u64;
                    failed_keys.push(key);
                    individual_times.push(elapsed_ns);
                    total_time_ns += elapsed_ns;
                }
            }
        }

        let total_operations = successful_results.len() + failed_keys.len();
        let success_rate = if total_operations > 0 {
            successful_results.len() as f64 / total_operations as f64
        } else {
            0.0
        };
        let avg_latency_ns = if total_operations > 0 {
            total_time_ns / total_operations as u64
        } else {
            0
        };

        BatchOperationSummary {
            successful_results,
            failed_count: failed_keys.len(),
            total_operations,
            success_rate,
            total_time_ns,
            avg_latency_ns,
            min_latency_ns: individual_times.iter().min().copied().unwrap_or(0),
            max_latency_ns: individual_times.iter().max().copied().unwrap_or(0),
        }
    }

    /// Execute batch put operations with comprehensive timing and statistics
    ///
    /// Returns simplified batch result with success/failure counts and timing data
    pub async fn batch_put(&self, entries: Vec<(K, V)>) -> BatchOperationSummary<()> {
        let mut successful_operations = 0;
        let mut failed_operations = 0;
        let mut total_time_ns = 0u64;
        let mut individual_times = Vec::new();

        for (key, value) in entries {
            let start = std::time::Instant::now();
            match self.put(key, value).await {
                Ok(()) => {
                    let elapsed_ns = start.elapsed().as_nanos() as u64;
                    successful_operations += 1;
                    individual_times.push(elapsed_ns);
                    total_time_ns += elapsed_ns;
                }
                Err(_) => {
                    let elapsed_ns = start.elapsed().as_nanos() as u64;
                    failed_operations += 1;
                    individual_times.push(elapsed_ns);
                    total_time_ns += elapsed_ns;
                }
            }
        }

        let total_operations = successful_operations + failed_operations;
        let success_rate = if total_operations > 0 {
            successful_operations as f64 / total_operations as f64
        } else {
            0.0
        };
        let avg_latency_ns = if total_operations > 0 {
            total_time_ns / total_operations as u64
        } else {
            0
        };

        BatchOperationSummary {
            successful_results: if successful_operations > 0 {
                vec![(); successful_operations]
            } else {
                vec![]
            },
            failed_count: failed_operations,
            total_operations,
            success_rate,
            total_time_ns,
            avg_latency_ns,
            min_latency_ns: individual_times.iter().min().copied().unwrap_or(0),
            max_latency_ns: individual_times.iter().max().copied().unwrap_or(0),
        }
    }

    /// Execute batch remove operations with comprehensive timing and statistics
    ///
    /// Returns simplified batch result with success/failure counts and timing data
    pub async fn batch_remove(&self, keys: Vec<K>) -> BatchOperationSummary<bool> {
        let mut successful_results = Vec::new();
        let mut failed_operations = 0;
        let mut total_time_ns = 0u64;
        let mut individual_times = Vec::new();

        for key in keys {
            let start = std::time::Instant::now();
            let removed = self.remove(&key).await;
            let elapsed_ns = start.elapsed().as_nanos() as u64;

            if removed {
                successful_results.push(true);
            } else {
                failed_operations += 1;
            }
            individual_times.push(elapsed_ns);
            total_time_ns += elapsed_ns;
        }

        let total_operations = successful_results.len() + failed_operations;
        let success_rate = if total_operations > 0 {
            successful_results.len() as f64 / total_operations as f64
        } else {
            0.0
        };
        let avg_latency_ns = if total_operations > 0 {
            total_time_ns / total_operations as u64
        } else {
            0
        };

        BatchOperationSummary {
            successful_results,
            failed_count: failed_operations,
            total_operations,
            success_rate,
            total_time_ns,
            avg_latency_ns,
            min_latency_ns: individual_times.iter().min().copied().unwrap_or(0),
            max_latency_ns: individual_times.iter().max().copied().unwrap_or(0),
        }
    }

    // =======================================================================
    // TASK COORDINATION AND BACKGROUND PROCESSING API
    // =======================================================================

    /// Get task coordinator statistics for background operations
    pub fn get_task_coordinator_stats(&self) -> TaskCoordinatorStats {
        // Call underlying UnifiedCacheManager's real implementation and convert
        let snapshot = self.manager.get_task_coordinator_stats();
        TaskCoordinatorStats {
            active_tasks: snapshot.active_task_count,
            completed_tasks: snapshot.total_tasks,
            failed_tasks: (snapshot.total_tasks as f64
                * (1.0 - snapshot.success_rate_percent / 100.0)) as u64,
            avg_execution_time_ms: (snapshot.avg_task_duration_ns as f64 / 1_000_000.0),
            queue_depth: snapshot.command_queue_stats.queued_commands,
        }
    }

    /// Get list of active background tasks
    pub fn get_active_tasks(&self) -> Vec<ActiveTask> {
        // Convert from internal TaskInfo to public ActiveTask format
        self.manager
            .get_active_tasks()
            .into_iter()
            .map(|task_info| ActiveTask {
                id: task_info.task_id(),
                task_type: task_info.task_type().to_string(),
                started_at: task_info.start_timestamp(),
                status: task_info.status().as_str().to_string(),
                progress: task_info.progress() as f64,
            })
            .collect()
    }

    /// Cancel a background task by ID
    pub fn cancel_task(&self, task_id: u64) -> Result<bool, CacheOperationError> {
        self.manager.cancel_task(task_id)
    }

    /// Get maintenance operation breakdown
    pub fn get_maintenance_breakdown(&self) -> MaintenanceBreakdown {
        self.manager.get_maintenance_stats()
    }

    /// Get maintenance configuration information
    pub fn get_maintenance_config_info(&self) -> String {
        let breakdown = self.manager.get_maintenance_stats();
        let total_time_ms = breakdown.compaction_time_ms
            + breakdown.eviction_time_ms
            + breakdown.stats_collection_time_ms
            + breakdown.memory_management_time_ms;

        format!(
            "MaintenanceConfig {{ enabled: true, total_cycles: {}, total_time_ms: {}, avg_time_ms: {:.2}, last_maintenance_ago_ms: {} }}",
            breakdown.total_cycles,
            total_time_ms,
            if breakdown.total_cycles > 0 {
                total_time_ms as f64 / breakdown.total_cycles as f64
            } else {
                0.0
            },
            (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64)
                .saturating_sub(breakdown.last_maintenance_ns)
                / 1_000_000
        )
    }

    /// Start background processor for maintenance tasks
    pub fn start_background_processor(&self) -> Result<(), CacheOperationError> {
        self.manager.start_background_processor()
    }

    /// Schedule an async operation with proper task coordination
    pub async fn schedule_async_operation<F, T, C>(
        &self,
        operation: F,
        context_name: String,
        _context_data: Vec<C>,
    ) -> Result<T, CacheOperationError>
    where
        F: FnOnce(&str) -> Result<T, CacheOperationError> + Send + 'static,
        T: Send + 'static,
        C: Send + 'static,
    {
        // Input validation
        if context_name.is_empty() {
            return Err(CacheOperationError::InvalidArgument(
                "Context name cannot be empty".to_string(),
            ));
        }
        if context_name.len() > 256 {
            return Err(CacheOperationError::InvalidArgument(
                "Context name too long (max 256 chars)".to_string(),
            ));
        }

        // Create result tracker and get task ID first for proper coordination
        use crate::cache::worker::task_coordination::TaskResultTracker;
        use std::time::Duration;

        let result_tracker = TaskResultTracker::<T>::new();

        // Generate task ID first using TaskCoordinator's ID generator
        let task_id = self.manager.task_coordinator().next_task_id();
        let result_sender = result_tracker.register_task_result(task_id);

        // Adapt operation to work with enhanced TaskExecutionContext
        let context_name_clone = context_name.clone();
        let enhanced_operation =
            move |_task_context: crate::cache::worker::task_coordination::TaskExecutionContext<
                SerdeCacheKey<K>,
                SerdeCacheValue<V>,
                T,
            >| {
                // Call the original operation with the context name
                operation(&context_name_clone)
            };

        // Schedule with result coordination using the same task_id
        let scheduled_task_id = self.manager.schedule_operation_with_task_id(
            enhanced_operation,
            context_name,
            Vec::<SerdeCacheKey<K>>::new(),
            Some(result_sender),
            task_id, // Use the same task_id for coordination
        )?;

        // Verify task IDs match (production safety check)
        if task_id != scheduled_task_id {
            return Err(CacheOperationError::InvalidState(format!(
                "Task ID mismatch: expected {}, got {}",
                task_id, scheduled_task_id
            )));
        }

        // Wait for result with the correct task_id
        result_tracker.wait_for_result(task_id, Duration::from_secs(30))
    }

    /// Shutdown the policy engine gracefully
    pub async fn shutdown_policy_engine(&self) -> Result<(), CacheOperationError> {
        // Policy engine shutdown is handled as part of graceful system shutdown
        // The sophisticated policy engine cleanup is integrated into the unified shutdown process
        self.manager.shutdown_gracefully().await
    }

    /// Shutdown the cache system gracefully
    pub async fn shutdown_gracefully(&self) -> Result<(), CacheOperationError> {
        self.manager.shutdown_gracefully().await
    }
}

impl<K, V> std::fmt::Debug for Goldylox<K, V>
where
    K: Serialize
        + DeserializeOwned
        + Clone
        + Hash
        + Eq
        + Ord
        + Send
        + Sync
        + Debug
        + Default
        + 'static,
    V: Serialize + DeserializeOwned + Clone + Send + Sync + Debug + Default + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Goldylox")
            .field("cache_id", &"<cache>")
            .finish()
    }
}

/// Fluent builder for Goldylox configuration
pub struct GoldyloxBuilder<K, V>
where
    K: Serialize
        + DeserializeOwned
        + Clone
        + Hash
        + Eq
        + Ord
        + Send
        + Sync
        + Debug
        + Default
        + CacheKey
        + 'static,
    V: Serialize + DeserializeOwned + Clone + Send + Sync + Debug + Default + CacheValue + 'static,
{
    config: CacheConfig,
    _phantom: std::marker::PhantomData<(K, V)>,
}

impl<K, V> GoldyloxBuilder<K, V>
where
    K: Serialize
        + DeserializeOwned
        + Clone
        + Hash
        + Eq
        + Ord
        + Send
        + Sync
        + Debug
        + Default
        + CacheKey
        + 'static,
    V: Serialize + DeserializeOwned + Clone + Send + Sync + Debug + Default + CacheValue + 'static,
{
    /// Create new builder with default configuration
    pub fn new() -> Self {
        Self {
            config: CacheConfig::default(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Set hot tier maximum entries
    pub fn hot_tier_max_entries(mut self, max_entries: u32) -> Self {
        self.config.hot_tier.max_entries = max_entries;
        self
    }

    /// Set hot tier memory limit in MB
    pub fn hot_tier_memory_limit_mb(mut self, limit_mb: u32) -> Self {
        self.config.hot_tier.memory_limit_mb = limit_mb;
        self
    }

    /// Set warm tier maximum entries
    pub fn warm_tier_max_entries(mut self, max_entries: usize) -> Self {
        self.config.warm_tier.max_entries = max_entries;
        self
    }

    /// Set warm tier memory limit in bytes
    pub fn warm_tier_max_memory_bytes(mut self, max_bytes: u64) -> Self {
        self.config.warm_tier.max_memory_bytes = max_bytes;
        self
    }

    /// Set cold tier base directory
    pub fn cold_tier_base_dir<P: AsRef<str>>(mut self, path: P) -> Self {
        let path_str = path.as_ref();
        if path_str.len() <= 256 {
            // ArrayString<256> requires manual construction
            self.config.cold_tier.base_dir.clear();
            for ch in path_str.chars() {
                let _ = self.config.cold_tier.base_dir.try_push(ch);
            }
        }
        self
    }

    /// Set cold tier maximum size in bytes
    pub fn cold_tier_max_size_bytes(mut self, max_size_bytes: u64) -> Self {
        self.config.cold_tier.max_size_bytes = max_size_bytes;
        self
    }

    /// Set cold tier compression level (0-9)
    pub fn compression_level(mut self, level: u8) -> Self {
        self.config.cold_tier.compression_level = level.min(9);
        self
    }

    /// Set cache ID (defaults to UUID if not specified)
    pub fn cache_id<S: Into<String>>(mut self, id: S) -> Self {
        self.config.cache_id = id.into();
        // Update base dir to use the new cache ID
        self.config.cold_tier.base_dir = generate_storage_path(&self.config.cache_id);
        self
    }

    /// Set number of background worker threads
    pub fn background_worker_threads(mut self, count: u8) -> Self {
        self.config.worker.thread_pool_size = count;
        self
    }

    /// Build the cache with the configured settings
    ///
    /// This initializes all cache tiers, crossbeam channels, workers, etc.
    /// All complex initialization is handled by UnifiedCacheManager.
    /// Spawns ~18-24 background worker threads.
    pub async fn build(self) -> Result<Goldylox<K, V>, CacheOperationError> {
        // Delegate all complex initialization to UnifiedCacheManager
        let manager = UnifiedCacheManager::new(self.config).await?;

        // Arc-wrap for cheap cloning without spawning new threads
        Ok(Goldylox { 
            manager: std::sync::Arc::new(manager)
        })
    }
}

impl<K, V> Default for GoldyloxBuilder<K, V>
where
    K: Serialize
        + DeserializeOwned
        + Clone
        + Hash
        + Eq
        + Ord
        + Send
        + Sync
        + Debug
        + Default
        + CacheKey
        + 'static,
    V: Serialize + DeserializeOwned + Clone + Send + Sync + Debug + Default + CacheValue + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}
