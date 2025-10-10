//! Worker task implementation
//!
//! This module implements the async worker task main loop and task processing
//! logic for background maintenance operations.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::types::{
    BackgroundWorkerState, CanonicalMaintenanceTask, MaintenanceConfig, MaintenanceTask,
    WorkerStatus,
};
use crate::cache::traits::{CacheKey, CacheValue};

/// Global active task counter
pub static GLOBAL_ACTIVE_TASKS: AtomicU32 = AtomicU32::new(0);

impl<K: CacheKey + Default, V: CacheValue + Default> super::types::MaintenanceScheduler<K, V>
where
    K: Clone + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: Clone
        + PartialEq
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
{
    /// Worker async task main loop
    pub async fn worker_loop(
        worker_id: u32,
        task_receiver: Arc<tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<MaintenanceTask>>>,
        shutdown: Arc<AtomicBool>,
        _config: MaintenanceConfig,
        context: super::types::WorkerContext,
    ) {
        let worker_state = BackgroundWorkerState::new(worker_id);

        // Create channel for worker status updates to monitoring system
        let (status_sender, status_receiver) = crossbeam_channel::unbounded();

        // Register worker status channel in per-instance registry for monitoring
        BackgroundWorkerState::register_worker_channel(&context.worker_registry, worker_id, status_receiver);

        // Send initial status update
        let _ = status_sender.try_send(worker_state.create_status_snapshot());

        // Periodic status update interval
        let mut status_interval = tokio::time::interval(Duration::from_millis(100));
        status_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            // Check shutdown flag
            if shutdown.load(Ordering::Relaxed) {
                worker_state.status.store(WorkerStatus::Shutdown);
                let _ = status_sender.try_send(worker_state.create_status_snapshot());
                break;
            }

            // Try to receive a task with async lock and receive
            let task_opt = {
                let mut receiver = task_receiver.lock().await;
                receiver.recv().await
            };

            match task_opt {
                Some(task) => {
                    worker_state.status.store(WorkerStatus::Processing);
                    worker_state
                        .current_task_discriminant
                        .store(task.priority as u8, Ordering::Relaxed);
                    let _ = status_sender.try_send(worker_state.create_status_snapshot());
                    GLOBAL_ACTIVE_TASKS.fetch_add(1, Ordering::Relaxed);

                    let start_time = Instant::now();

                    // Process the task with error handling - DIRECT AWAIT (NO BLOCKING!)
                    match Self::try_process_maintenance_task(&task, &worker_state, &context).await {
                        Ok(_) => {
                            let processing_time = start_time.elapsed().as_nanos() as u64;
                            worker_state
                                .total_processing_time
                                .fetch_add(processing_time, Ordering::Relaxed);
                            worker_state.tasks_processed.fetch_add(1, Ordering::Relaxed);

                            // If worker was in error state, attempt recovery
                            if matches!(worker_state.status(), WorkerStatus::Error) {
                                worker_state.attempt_recovery();
                                let _ =
                                    status_sender.try_send(worker_state.create_status_snapshot());
                            }
                        }
                        Err(e) => {
                            // Record error and update status
                            worker_state.record_error(&format!("Task processing failed: {:?}", e));
                            let _ = status_sender.try_send(worker_state.create_status_snapshot());
                        }
                    }

                    GLOBAL_ACTIVE_TASKS.fetch_sub(1, Ordering::Relaxed);
                    worker_state
                        .current_task_discriminant
                        .store(0, Ordering::Relaxed);

                    // Only set to Idle if not in Error state
                    if !matches!(worker_state.status(), WorkerStatus::Error) {
                        worker_state.status.store(WorkerStatus::Idle);
                        let _ = status_sender.try_send(worker_state.create_status_snapshot());
                    }

                    // Update heartbeat after processing
                    worker_state.last_heartbeat.store(Instant::now());
                }
                None => {
                    // Channel closed, shutdown
                    break;
                }
            }

            // Send periodic status update
            if worker_state
                .tasks_processed
                .load(Ordering::Relaxed)
                .is_multiple_of(10)
            {
                let _ = status_sender.try_send(worker_state.create_status_snapshot());
            }

            // Brief yield to allow other tasks to run
            tokio::task::yield_now().await;
        }

        // Unregister worker state from per-instance registry on shutdown
        BackgroundWorkerState::unregister_worker(&context.worker_registry, worker_id);
    }

    /// Process canonical maintenance task with error handling
    pub async fn try_process_maintenance_task(
        task: &MaintenanceTask,
        worker_state: &BackgroundWorkerState,
        context: &super::types::WorkerContext,
    ) -> Result<(), crate::cache::traits::types_and_enums::CacheOperationError> {
        // Check for task timeout
        if task.created_at.elapsed().as_secs() > 30 {
            // 30 second timeout
            return Err(crate::cache::traits::types_and_enums::CacheOperationError::TimeoutError);
        }

        // Process the task
        Self::process_maintenance_task(task, worker_state, context).await;
        Ok(())
    }

    /// Process canonical maintenance task
    pub async fn process_maintenance_task(
        task: &MaintenanceTask,
        worker_state: &BackgroundWorkerState,
        context: &super::types::WorkerContext,
    ) {
        match &task.task {
            CanonicalMaintenanceTask::CompactStorage { .. } => {
                // Connect to cold tier service via message passing
                if let Err(e) = context.cold_tier_coordinator.trigger_maintenance::<K, V>("compact").await {
                    log::error!("Failed to trigger compaction: {:?}", e);
                } else {
                    log::debug!("Cold tier compaction requested via service");
                }

                worker_state.tasks_processed.fetch_add(1, Ordering::Relaxed);
            }
            CanonicalMaintenanceTask::CleanupExpired { ttl, .. } => {
                // Connect to tier-specific cleanup implementations
                use crate::cache::tier::{hot, warm};

                let mut total_cleaned = 0;

                // Clean hot tier
                total_cleaned += hot::cleanup_expired_entries::<K, V>(&context.hot_tier_coordinator, *ttl).await;

                // Clean warm tier
                if let Ok(cleaned) = warm::cleanup_expired_entries::<K, V>(&context.warm_tier_coordinator, *ttl).await {
                    total_cleaned += cleaned;
                }

                // Clean cold tier using existing sophisticated maintenance system
                if let Ok(()) = context.cold_tier_coordinator.trigger_maintenance::<K, V>("compact").await {
                    total_cleaned += 1; // Cold tier maintenance triggered successfully
                }

                log::debug!("Cleaned {} expired entries across all tiers", total_cleaned);
                worker_state.tasks_processed.fetch_add(1, Ordering::Relaxed);
            }
            CanonicalMaintenanceTask::SyncTiers { .. } => {
                worker_state.tasks_processed.fetch_add(1, Ordering::Relaxed);
            }
            CanonicalMaintenanceTask::UpdateStatistics { .. } => {
                // Update unified statistics using per-instance statistics
                let _metrics = context.unified_stats.get_performance_metrics();
                log::debug!("Updated unified cache statistics");

                // Update coherence statistics using per-instance statistics
                let snapshot = context.coherence_stats.get_snapshot();

                // Use CoherenceStatistics methods for real-time calculation
                let live_success_rate = context.coherence_stats.success_rate();
                let live_invalidation_efficiency =
                    context.coherence_stats.invalidation_efficiency();

                // Log comprehensive coherence telemetry using all CoherenceStatisticsSnapshot fields
                log::info!(
                    "Coherence telemetry - Total ops: {}, Success rate: {:.1}%, Violations: {} ({:.1}%), Overhead: {}ns/op",
                    snapshot.total_operations(),
                    snapshot.success_rate(),
                    snapshot.protocol_violations,
                    snapshot.violation_rate(),
                    snapshot.avg_overhead_per_operation_ns()
                );
                log::debug!(
                    "Coherence details - Transitions: {}, Invalidations sent/rcv: {}/{}, Writebacks: {}, Avg latency: {}ns, Peak concurrent: {}",
                    snapshot.total_transitions,
                    snapshot.invalidations_sent,
                    snapshot.invalidations_received,
                    snapshot.writebacks_performed,
                    snapshot.avg_operation_latency_ns,
                    snapshot.peak_concurrent_operations
                );
                log::debug!(
                    "Live coherence metrics - Success rate: {:.2}%, Invalidation efficiency: {:.2}%",
                    live_success_rate,
                    live_invalidation_efficiency
                );
                log::debug!("Updated coherence statistics");
                log::debug!("Updated cache statistics");
                worker_state.tasks_processed.fetch_add(1, Ordering::Relaxed);
            }

            CanonicalMaintenanceTask::AnalyzePatterns { .. } => {
                // Connect to warm tier pattern analysis system using existing public API
                use crate::cache::tier::warm::global_api;

                // Trigger pattern analysis for all active warm tier instances
                match global_api::process_background_maintenance::<K, V>(&context.warm_tier_coordinator).await {
                    Ok(maintenance_count) => {
                        log::debug!(
                            "Warm tier pattern analysis completed, processed {} items",
                            maintenance_count
                        );
                    }
                    Err(e) => {
                        log::error!("Failed to trigger warm tier pattern analysis: {:?}", e);
                    }
                }

                worker_state.tasks_processed.fetch_add(1, Ordering::Relaxed);
            }

            CanonicalMaintenanceTask::OptimizeStructure { .. } => {
                // Connect to performance monitoring and optimization
                use crate::cache::manager::performance::core::PerformanceMonitor;

                let monitor = PerformanceMonitor::new();
                let snapshot = monitor.collect_metrics();
                let alerts = monitor.check_alerts(&snapshot);

                for alert in alerts {
                    log::warn!("Performance alert: {:?}", alert);
                }

                log::debug!("Performance optimization check completed");
                worker_state.tasks_processed.fetch_add(1, Ordering::Relaxed);
            }

            CanonicalMaintenanceTask::Evict { .. } => {
                // Connect to memory pool optimization system
                use crate::cache::memory::pool_manager::cleanup_manager;

                match cleanup_manager::trigger_defragmentation(&context.pool_coordinator) {
                    Ok(reclaimed) => {
                        log::debug!("Memory optimization reclaimed {} bytes", reclaimed);
                    }
                    Err(e) => {
                        log::warn!("Memory optimization failed: {:?}", e);
                    }
                }

                worker_state.tasks_processed.fetch_add(1, Ordering::Relaxed);
            }
            CanonicalMaintenanceTask::PerformEviction { .. } => {
                // Connect to warm tier pattern analysis system for eviction-based pattern optimization
                use crate::cache::tier::warm::global_api;

                // Trigger pattern analysis to optimize eviction decisions
                match global_api::process_background_maintenance::<K, V>(&context.warm_tier_coordinator).await {
                    Ok(maintenance_count) => {
                        log::debug!(
                            "Pattern analysis for eviction optimization completed, processed {} items",
                            maintenance_count
                        );
                    }
                    Err(e) => {
                        log::error!(
                            "Failed to trigger warm tier pattern analysis for eviction optimization: {:?}",
                            e
                        );
                    }
                }

                worker_state.tasks_processed.fetch_add(1, Ordering::Relaxed);
            }
            CanonicalMaintenanceTask::ValidateIntegrity { .. } => {
                // Connect to cold tier integrity validation
                match context.cold_tier_coordinator.execute_read_operation::<K, V, bool, _>(|tier| {
                    tier.validate_integrity()
                }).await {
                    Ok(valid) => {
                        if !valid {
                            log::warn!("Cold tier integrity validation failed");
                        } else {
                            log::debug!("Cold tier integrity validation passed");
                        }
                    }
                    Err(e) => {
                        log::error!("Cold tier integrity check error: {:?}", e);
                    }
                }

                worker_state.tasks_processed.fetch_add(1, Ordering::Relaxed);
            }
            CanonicalMaintenanceTask::DefragmentMemory {
                target_fragmentation,
            } => {
                // Connect to memory pool defragmentation system
                use crate::cache::memory::pool_manager::cleanup_manager;

                match cleanup_manager::trigger_defragmentation(&context.pool_coordinator) {
                    Ok(reclaimed_bytes) => {
                        log::debug!(
                            "Memory defragmentation completed, reclaimed {} bytes (target fragmentation: {})",
                            reclaimed_bytes,
                            target_fragmentation
                        );
                    }
                    Err(e) => {
                        log::error!("Failed to trigger memory defragmentation: {:?}", e);
                    }
                }

                worker_state.tasks_processed.fetch_add(1, Ordering::Relaxed);
            }
            CanonicalMaintenanceTask::UpdateMLModels { .. } => {
                log::debug!(
                    "ML model update requested - triggering warm tier ML eviction policy adaptation via crossbeam messaging"
                );

                // Use proper crossbeam messaging to update ML models
                // The warm tier worker owns the ML policy data and processes the update
                // This calls the sophisticated existing ML system with:
                // 1. MachineLearningEvictionPolicy.adapt() method
                // 2. 16-feature FeatureVector system with feature importance analysis
                // 3. Linear regression with gradient optimization
                // 4. Temporal pattern analysis with time-of-day calculations
                // 5. Learning rate adaptation based on current model performance
                // 6. Aging applied to feature vectors using exponential decay
                // 7. Statistics recording using existing infrastructure

                match crate::cache::tier::warm::global_api::update_warm_tier_ml_models::<K, V>(&context.warm_tier_coordinator).await {
                    Ok(updated_count) => {
                        log::info!(
                            "ML model adaptation completed via crossbeam messaging: {} ML policies updated using existing sophisticated system",
                            updated_count
                        );
                    }
                    Err(e) => {
                        log::error!("ML model adaptation failed: {:?}", e);
                    }
                }

                worker_state.tasks_processed.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}
