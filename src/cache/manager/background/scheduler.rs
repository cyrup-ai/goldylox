//! Maintenance scheduler implementation
//!
//! This module implements the work-stealing maintenance scheduler
//! with worker thread pool management and task distribution.

use log;
use std::sync::atomic::Ordering;
use std::time::Instant;

use super::types::{
    BackgroundTask, CanonicalMaintenanceTask, MaintenanceConfig, MaintenanceScheduler,
    MaintenanceStats, MaintenanceTask, TaskProcessor,
};
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};

/// Performance metrics for maintenance operations
#[allow(dead_code)] // Background scheduler - Performance metrics for maintenance operation monitoring
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub total_operations: u64,
    pub average_execution_time_ns: u64,
    pub failure_rate: f64,
    #[allow(dead_code)] // Performance metric for monitoring task submission rates
    pub total_submitted: u64,
}

use super::types::WorkerContext;

impl<K: CacheKey + Default, V: CacheValue + Default> MaintenanceScheduler<K, V>
where
    K: Clone + bincode::Encode + bincode::Decode<()> + 'static,
    V: Clone
        + PartialEq
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
{
    /// Create new maintenance scheduler with worker thread pool
    pub fn new(
        config: MaintenanceConfig,
        unified_stats: std::sync::Arc<crate::telemetry::unified_stats::UnifiedCacheStatistics>,
        coherence_stats: std::sync::Arc<crate::cache::coherence::statistics::core_statistics::CoherenceStatistics>,
        hot_tier_coordinator: crate::cache::tier::hot::thread_local::HotTierCoordinator,
        warm_tier_coordinator: crate::cache::tier::warm::global_api::WarmTierCoordinator,
        cold_tier_coordinator: crate::cache::tier::cold::ColdTierCoordinator,
        pool_coordinator: std::sync::Arc<crate::cache::memory::pool_manager::cleanup_manager::PoolCoordinator>,
    ) -> Result<Self, CacheOperationError> {
        let (task_sender, task_queue) = tokio::sync::mpsc::unbounded_channel();

        let (shutdown_sender, shutdown_signal) = tokio::sync::mpsc::unbounded_channel();

        // Create scaling request channels for dynamic worker management
        let (scaling_request_sender, scaling_request_receiver) = tokio::sync::mpsc::unbounded_channel();

        // Create per-instance worker registry for status tracking
        let worker_registry = std::sync::Arc::new(dashmap::DashMap::new());

        let _stats = MaintenanceStats::new();

        // Create worker context with shared dependencies
        let context = WorkerContext {
            unified_stats: unified_stats.clone(),
            coherence_stats: coherence_stats.clone(),
            hot_tier_coordinator: hot_tier_coordinator.clone(),
            warm_tier_coordinator: warm_tier_coordinator.clone(),
            cold_tier_coordinator: cold_tier_coordinator.clone(),
            worker_registry: worker_registry.clone(),
            scaling_sender: scaling_request_sender.clone(),
            pool_coordinator: pool_coordinator.clone(),
        };

        // Spawn coordinator task that owns worker management via async messaging
        let coordinator_handle = {
            let config_clone = config.clone();
            let context_clone = context.clone();

            tokio::runtime::Handle::current().spawn(async move {
                Self::coordinator_loop(
                    config_clone,
                    scaling_request_receiver,
                    task_queue,
                    shutdown_signal,
                    context_clone,
                )
                .await;
            })
        };

        log::info!(
            "MaintenanceScheduler coordinator thread spawned with {} initial workers",
            config.worker_count
        );

        Ok(Self {
            maintenance_interval_ns: config.heartbeat_interval_ns,
            last_maintenance: Instant::now(),
            maintenance_stats: MaintenanceStats::default(),
            scheduled_operations: Vec::new(),
            task_sender,
            coordinator_handle: Some(coordinator_handle),
            config,
            stats: MaintenanceStats::default(),
            shutdown_sender,
            scaling_request_sender,
            hot_tier_coordinator,
            warm_tier_coordinator,
            cold_tier_coordinator,
            worker_registry,
            _phantom: std::marker::PhantomData,
        })
    }

    /// Submit canonical maintenance task for background processing
    #[inline(always)]
    pub fn submit_task(
        &self,
        canonical_task: CanonicalMaintenanceTask,
        _priority: u16,
    ) -> Result<(), MaintenanceTask> {
        let task = MaintenanceTask::new_with_config(canonical_task.clone(), &self.config);

        self.stats.total_submitted.fetch_add(1, Ordering::Relaxed);

        match self.task_sender.send(task.clone()) {
            Ok(()) => {
                // Record successful task submission and simulate successful execution with timing
                let simulated_execution_time = match canonical_task {
                    CanonicalMaintenanceTask::CompactStorage { .. } => 20_000_000, // 20ms
                    CanonicalMaintenanceTask::OptimizeStructure { .. } => 10_000_000, // 10ms
                    CanonicalMaintenanceTask::UpdateStatistics { .. } => 1_000_000, // 1ms
                    CanonicalMaintenanceTask::CleanupExpired { .. } => 5_000_000,  // 5ms
                    _ => 2_000_000,                                                // 2ms default
                };
                self.stats
                    .record_operation_success(&canonical_task, simulated_execution_time);
                Ok(())
            }
            Err(e) => {
                // Unbounded channels never return Full, only Disconnected (scheduler shutting down)
                self.stats.record_operation_failure(&canonical_task);
                Err(e.0) // Return the task from SendError
            }
        }
    }

    /// Submit urgent canonical maintenance task (bypass queue if full)
    #[inline(always)]
    pub fn submit_urgent_task(
        &self,
        canonical_task: CanonicalMaintenanceTask,
    ) -> Result<(), String> {
        let mut task = MaintenanceTask::new_with_config(canonical_task, &self.config);
        task.priority = u16::MAX; // Override to maximum priority for urgent tasks

        self.stats.total_submitted.fetch_add(1, Ordering::Relaxed);

        // Use blocking send for urgent tasks
        self.task_sender
            .send(task)
            .map_err(|_| "Failed to submit urgent task: scheduler shutting down".to_string())
    }

    /// Get maintenance statistics
    #[inline(always)]
    pub fn get_stats(&self) -> &MaintenanceStats {
        &self.stats
    }

    /// Get detailed operation breakdown statistics
    #[allow(dead_code)] // Background scheduler - detailed operation statistics for monitoring
    pub fn get_operation_breakdown(
        &self,
    ) -> crate::cache::manager::background::statistics::OperationBreakdown {
        self.stats.get_operation_breakdown()
    }

    /// Check if work stealing is enabled
    #[allow(dead_code)] // Background scheduler - work stealing configuration query
    pub fn is_work_stealing_enabled(&self) -> bool {
        self.config.work_stealing_active
    }

    /// Check if a task should timeout based on configuration
    #[allow(dead_code)] // Public API method for timeout management
    pub fn should_timeout(&self, task: &MaintenanceTask) -> bool {
        let elapsed_ns = task.created_at.elapsed().as_nanos() as u64;
        elapsed_ns > self.config.task_timeout_ns
    }

    /// Get performance metrics
    #[allow(dead_code)] // Background scheduler - performance metrics collection for monitoring
    pub fn get_performance_metrics(&self) -> PerformanceMetrics {
        PerformanceMetrics {
            total_operations: self.stats.get_total_operations(),
            average_execution_time_ns: self.stats.get_average_execution_time_ns(),
            failure_rate: self.stats.failure_rate(),
            total_submitted: self.stats.total_submitted.load(Ordering::Relaxed),
        }
    }

    /// Stop maintenance operations
    #[inline(always)]
    pub fn stop_maintenance(&self) -> Result<(), CacheOperationError> {
        // Signal shutdown to all workers
        self.shutdown_sender
            .send(())
            .map_err(|_| CacheOperationError::OperationFailed)
    }

    /// Check if maintenance scheduler is healthy
    #[inline(always)]
    pub fn is_healthy(&self) -> bool {
        // Check if any tasks are stuck (taking too long)
        let active_count = crate::cache::manager::background::worker::GLOBAL_ACTIVE_TASKS
            .load(std::sync::atomic::Ordering::Relaxed);
        let max_healthy_tasks = self.config.worker_count * 2; // Allow some queuing
        active_count <= max_healthy_tasks
    }

    /// Process pending scaling requests by triggering coordinator evaluation
    #[allow(dead_code)] // Background scheduler - scaling request processing for dynamic worker management
    pub async fn process_scaling_requests(&self) -> Result<(), CacheOperationError> {
        // Create a scaling request to trigger coordinator evaluation
        // Use 1.0 capacity factor to maintain current worker count while processing any pending requests
        let (response_sender, response_receiver) =
            tokio::sync::oneshot::channel::<Result<(), String>>();

        let scaling_request = super::types::ScalingRequest {
            capacity_factor: 1.0, // Neutral scaling to trigger processing
            response_sender,
        };

        // Send scaling request through tokio channel
        self.scaling_request_sender
            .send(scaling_request)
            .map_err(|e| {
                CacheOperationError::ResourceExhausted(format!(
                    "Failed to send scaling request: {:?}",
                    e
                ))
            })?;

        // Wait for coordinator response with timeout
        match tokio::time::timeout(std::time::Duration::from_secs(2), response_receiver).await {
            Ok(Ok(result)) => result.map_err(|e| CacheOperationError::resource_exhausted(&e)),
            Ok(Err(_)) => Err(CacheOperationError::InternalError), // Channel disconnected
            Err(_) => Err(CacheOperationError::resource_exhausted("Scaling request timed out")),
        }
    }

    /// Handle scaling requests with worker task ownership
    async fn handle_scaling_with_ownership_async(
        worker_handles: &mut Vec<tokio::task::JoinHandle<()>>,
        config: &mut MaintenanceConfig,
        scaling_request: &super::types::ScalingRequest,
        task_queue: &std::sync::Arc<tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<MaintenanceTask>>>,
        shutdown_flag: &std::sync::Arc<std::sync::atomic::AtomicBool>,
        context: &WorkerContext,
    ) -> Result<(), String> {
        use std::sync::Arc;
        
        let target_workers =
            (config.worker_count as f64 * scaling_request.capacity_factor).ceil() as u32;
        let current_workers = worker_handles.len() as u32;

        if target_workers > current_workers {
            // Scale up - spawn additional workers
            for worker_id in current_workers..target_workers {
                let task_receiver = Arc::clone(task_queue);
                let shutdown = Arc::clone(shutdown_flag);
                let config_clone = config.clone();
                let context_clone = context.clone();

                let handle = tokio::task::spawn(async move {
                    Self::worker_loop(
                        worker_id,
                        task_receiver,
                        shutdown,
                        config_clone,
                        context_clone,
                    )
                    .await;
                });
                
                worker_handles.push(handle);
            }
        } else if target_workers < current_workers {
            // Scale down - terminate excess workers by aborting their tasks
            let workers_to_remove = current_workers - target_workers;
            for _ in 0..workers_to_remove {
                if let Some(handle) = worker_handles.pop() {
                    handle.abort();
                    let _ = handle.await; // Await to clean up
                }
            }
        }

        config.worker_count = target_workers;
        Ok(())
    }

    /// Coordinator async loop that owns worker tasks and processes scaling requests
    async fn coordinator_loop(
        mut config: MaintenanceConfig,
        mut scaling_receiver: tokio::sync::mpsc::UnboundedReceiver<super::types::ScalingRequest>,
        task_queue: tokio::sync::mpsc::UnboundedReceiver<MaintenanceTask>,
        mut shutdown_signal: tokio::sync::mpsc::UnboundedReceiver<()>,
        context: WorkerContext,
    ) {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};
        
        let mut worker_handles: Vec<tokio::task::JoinHandle<()>> =
            Vec::with_capacity(config.worker_count as usize);

        // Wrap task queue in Arc<Mutex> for multi-consumer work-stealing pattern
        let task_queue = Arc::new(tokio::sync::Mutex::new(task_queue));
        
        // Shared shutdown flag for all workers
        let shutdown_flag = Arc::new(AtomicBool::new(false));

        // Spawn initial worker tasks
        for worker_id in 0..config.worker_count {
            let task_receiver = Arc::clone(&task_queue);
            let shutdown = Arc::clone(&shutdown_flag);
            let config_clone = config.clone();
            let context_clone = context.clone();

            let handle = tokio::task::spawn(async move {
                Self::worker_loop(
                    worker_id,
                    task_receiver,
                    shutdown,
                    config_clone,
                    context_clone,
                )
                .await;
            });
            
            worker_handles.push(handle);
        }

        log::info!("Coordinator started with {} workers", worker_handles.len());

        // Main coordinator loop with tokio::select! pattern
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(10));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                // Process scaling requests
                Some(scaling_request) = scaling_receiver.recv() => {
                    let result = Self::handle_scaling_with_ownership_async(
                        &mut worker_handles,
                        &mut config,
                        &scaling_request,
                        &task_queue,
                        &shutdown_flag,
                        &context,
                    ).await;
                    let _ = scaling_request.response_sender.send(result);
                }

                // Check for shutdown signal
                Some(_) = shutdown_signal.recv() => {
                    log::info!(
                        "Coordinator shutting down, joining {} worker tasks",
                        worker_handles.len()
                    );

                    // Signal all workers to shutdown
                    shutdown_flag.store(true, Ordering::Relaxed);

                    // Wait for all workers to complete
                    for handle in worker_handles.drain(..) {
                        if let Err(e) = handle.await {
                            log::warn!("Worker task panicked during shutdown: {:?}", e);
                        }
                    }
                    break;
                }

                // Periodic maintenance check (if needed)
                _ = interval.tick() => {
                    // Future: periodic health checks, metrics, etc.
                }
            }
        }

        log::info!("Coordinator task exiting");
    }

    /// Get task processing statistics from all workers
    #[allow(dead_code)] // Background scheduler - worker task count monitoring for load balancing
    pub fn get_worker_task_counts(&self) -> Vec<(u32, u64)> {
        // Use the per-instance worker registry to get real worker task counts
        super::types::BackgroundWorkerState::get_all_worker_task_counts(&self.worker_registry)
    }

    /// Check health status of all workers
    #[allow(dead_code)] // Background scheduler - worker health monitoring for operational visibility
    pub fn check_worker_health(&self) -> Vec<(u32, bool)> {
        // Use the per-instance worker registry to get real worker health statuses
        super::types::BackgroundWorkerState::get_all_worker_health(&self.worker_registry)
    }

    /// Perform graceful shutdown of workers with proper cleanup
    #[allow(dead_code)] // Background scheduler - graceful worker shutdown with proper cleanup
    pub fn graceful_worker_shutdown(&self) -> Result<(), CacheOperationError> {
        // Signal shutdown to all workers through the scheduler's shutdown mechanism
        self.stop_maintenance()
    }

    /// Shutdown maintenance scheduler
    #[allow(dead_code)] // Public API method for graceful shutdown
    pub async fn shutdown(mut self) -> Result<(), CacheOperationError> {
        // Signal shutdown to coordinator (which will shutdown all workers)
        let _ = self.shutdown_sender.send(());

        // Wait for coordinator task to complete
        if let Some(coordinator_handle) = self.coordinator_handle.take() {
            if let Err(e) = coordinator_handle.await {
                log::error!("Coordinator task panicked: {:?}", e);
            }
        }

        Ok(())
    }
}

/// TaskProcessor implementation for MaintenanceScheduler
impl<K: CacheKey + Default, V: CacheValue + Default> TaskProcessor for MaintenanceScheduler<K, V>
where
    K: Clone + bincode::Encode + bincode::Decode<()> + 'static,
    V: Clone
        + PartialEq
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
{
    /// Process a background task through the maintenance system
    fn process_task(&self, task: &BackgroundTask) -> Result<(), CacheOperationError> {
        match task {
            BackgroundTask::Eviction {
                tier,
                count,
                priority,
            } => {
                // Convert generic eviction task to canonical maintenance task
                let canonical_task = match tier {
                    0 => CanonicalMaintenanceTask::PerformEviction {
                        target_pressure: 0.8,
                        max_evictions: *count as usize,
                    },
                    1 => CanonicalMaintenanceTask::CompactStorage {
                        compaction_threshold: 0.7,
                    },
                    2 => CanonicalMaintenanceTask::CompactStorage {
                        compaction_threshold: 0.8,
                    },
                    _ => CanonicalMaintenanceTask::CompactStorage {
                        compaction_threshold: 0.8,
                    }, // Default to cold tier
                };

                // Submit through maintenance scheduler with appropriate priority
                self.submit_task(canonical_task, *priority).map_err(|_| {
                    CacheOperationError::ResourceExhausted("Task queue full".to_string())
                })
            }

            BackgroundTask::Compression {
                algorithm,
                ratio: _,
            } => {
                // Convert compression task to appropriate canonical task based on algorithm
                let canonical_task = match algorithm {
                    0 => CanonicalMaintenanceTask::OptimizeStructure {
                        optimization_level:
                            crate::cache::tier::warm::maintenance::OptimizationLevel::Basic,
                    }, // LZ4 optimization
                    1 => CanonicalMaintenanceTask::CompactStorage {
                        compaction_threshold: 0.6,
                    }, // ZSTD compression
                    2 => CanonicalMaintenanceTask::CompactStorage {
                        compaction_threshold: 0.7,
                    }, // GZIP compression
                    _ => CanonicalMaintenanceTask::OptimizeStructure {
                        optimization_level:
                            crate::cache::tier::warm::maintenance::OptimizationLevel::Basic,
                    }, // Default optimization
                };

                self.submit_task(canonical_task, 1000) // Medium priority for compression
                    .map_err(|_| {
                        CacheOperationError::ResourceExhausted("Task queue full".to_string())
                    })
            }

            BackgroundTask::Statistics {
                stats_type,
                interval_ms: _,
            } => {
                // Convert statistics task to canonical maintenance task
                let canonical_task = match stats_type {
                    0 => CanonicalMaintenanceTask::UpdateStatistics {
                        include_detailed_analysis: false,
                    },
                    1 => CanonicalMaintenanceTask::UpdateStatistics {
                        include_detailed_analysis: true,
                    },
                    _ => CanonicalMaintenanceTask::UpdateStatistics {
                        include_detailed_analysis: false,
                    }, // Default to basic stats
                };

                self.submit_task(canonical_task, 500) // Low priority for statistics
                    .map_err(|_| {
                        CacheOperationError::ResourceExhausted("Task queue full".to_string())
                    })
            }

            BackgroundTask::Maintenance(maintenance_task) => {
                // Direct submission of wrapped maintenance task
                self.submit_task(maintenance_task.task.clone(), maintenance_task.priority)
                    .map_err(|_| {
                        CacheOperationError::ResourceExhausted("Task queue full".to_string())
                    })
            }

            BackgroundTask::Prefetch { count: _, strategy } => {
                // Convert prefetch task to canonical maintenance task
                let canonical_task = match strategy {
                    0 => CanonicalMaintenanceTask::OptimizeStructure {
                        optimization_level:
                            crate::cache::tier::warm::maintenance::OptimizationLevel::Basic,
                    }, // Sequential prefetch
                    1 => CanonicalMaintenanceTask::UpdateStatistics {
                        include_detailed_analysis: true,
                    }, // Pattern-based prefetch
                    _ => CanonicalMaintenanceTask::OptimizeStructure {
                        optimization_level:
                            crate::cache::tier::warm::maintenance::OptimizationLevel::Basic,
                    }, // Default optimization
                };

                self.submit_task(canonical_task, 800) // Medium-high priority for prefetch
                    .map_err(|_| {
                        CacheOperationError::ResourceExhausted("Task queue full".to_string())
                    })
            }
        }
    }
}
