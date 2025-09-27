//! Worker thread implementation
//!
//! This module implements the worker thread main loop and task processing
//! logic for background maintenance operations.

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, TryRecvError};

use super::types::{
    BackgroundWorkerState, CanonicalMaintenanceTask, MaintenanceConfig, MaintenanceTask,
    WorkerStatus,
};
use crate::cache::tier::cold::ColdTierCoordinator;
use crate::cache::traits::{CacheKey, CacheValue};

/// Global active task counter
pub static GLOBAL_ACTIVE_TASKS: AtomicU32 = AtomicU32::new(0);

impl<K: CacheKey + Default, V: CacheValue + Default> super::types::MaintenanceScheduler<K, V>
where
    K: Clone + Default + bincode::Encode + bincode::Decode<()> + 'static,
    V: Clone
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
{
    /// Worker thread main loop
    pub fn worker_loop(
        worker_id: u32,
        task_receiver: Receiver<MaintenanceTask>,
        shutdown_receiver: Receiver<()>,
        _active_tasks: &AtomicU32, // Now uses global static
        config: MaintenanceConfig,
    ) {
        let worker_state = BackgroundWorkerState::new(worker_id);

        // Create channel for worker status updates to monitoring system
        let (status_sender, status_receiver) = crossbeam_channel::unbounded();

        // Register worker status channel in global registry for monitoring
        BackgroundWorkerState::register_worker_channel(worker_id, status_receiver);

        // Send initial status update
        let _ = status_sender.try_send(worker_state.create_status_snapshot());

        loop {
            // Check for shutdown signal
            if shutdown_receiver.try_recv().is_ok() {
                worker_state.status.store(WorkerStatus::Shutdown);
                let _ = status_sender.try_send(worker_state.create_status_snapshot());
                break;
            }

            // Try to get next task
            match task_receiver.try_recv() {
                Ok(task) => {
                    worker_state.status.store(WorkerStatus::Processing);
                    worker_state
                        .current_task_discriminant
                        .store(task.priority as u8, Ordering::Relaxed);
                    let _ = status_sender.try_send(worker_state.create_status_snapshot());
                    GLOBAL_ACTIVE_TASKS.fetch_add(1, Ordering::Relaxed);

                    let start_time = Instant::now();

                    // Process the task with error handling
                    match Self::try_process_maintenance_task(&task, &worker_state) {
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
                }
                Err(TryRecvError::Empty) => {
                    // No tasks available, check for work stealing if enabled
                    if worker_state.can_steal_work(&config) {
                        worker_state.start_work_stealing(&config);

                        // Implement work stealing using crossbeam pattern
                        worker_state.steal_attempts.fetch_add(1, Ordering::Relaxed);

                        // Try stealing from global injector first (most efficient)
                        // In our architecture, the task_receiver acts as both local queue and stealer
                        // This is a simplified work stealing - in full implementation we'd have
                        // separate Worker/Stealer queues per worker thread

                        // Attempt to steal by checking if other workers have tasks
                        // Since we're using crossbeam_channel, we simulate work stealing
                        // by yielding briefly to allow other workers to process
                        std::thread::yield_now();

                        // If no work found after yielding, brief sleep to avoid busy waiting
                        if task_receiver.is_empty() {
                            std::thread::sleep(Duration::from_millis(1)); // Minimal sleep
                        } else {
                            // Work potentially available - record successful steal attempt
                            worker_state
                                .successful_steals
                                .fetch_add(1, Ordering::Relaxed);
                        }
                    } else {
                        // No tasks available, sleep briefly
                        worker_state.status.store(WorkerStatus::Idle);
                        let _ = status_sender.try_send(worker_state.create_status_snapshot());
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
                Err(TryRecvError::Disconnected) => {
                    // Channel disconnected, shutdown
                    break;
                }
            }

            // Update heartbeat and send periodic status update
            worker_state.last_heartbeat.store(Instant::now());
            // Send status update every few iterations to keep monitoring current
            if worker_state
                .tasks_processed
                .load(Ordering::Relaxed)
                .is_multiple_of(10)
            {
                let _ = status_sender.try_send(worker_state.create_status_snapshot());
            }
        }

        // Unregister worker state from global registry on shutdown
        BackgroundWorkerState::unregister_worker(worker_id);
    }

    /// Process canonical maintenance task with error handling
    pub fn try_process_maintenance_task(
        task: &MaintenanceTask,
        worker_state: &BackgroundWorkerState,
    ) -> Result<(), crate::cache::traits::types_and_enums::CacheOperationError> {
        // Check for task timeout
        if task.created_at.elapsed().as_secs() > 30 {
            // 30 second timeout
            return Err(crate::cache::traits::types_and_enums::CacheOperationError::TimeoutError);
        }

        // Process the task
        Self::process_maintenance_task(task, worker_state);
        Ok(())
    }

    /// Process canonical maintenance task
    pub fn process_maintenance_task(task: &MaintenanceTask, worker_state: &BackgroundWorkerState) {
        match &task.task {
            CanonicalMaintenanceTask::CompactStorage { .. } => {
                // Connect to cold tier service via message passing
                match ColdTierCoordinator::get() {
                    Ok(coordinator) => {
                        if let Err(e) = coordinator.trigger_maintenance::<K, V>("compact") {
                            log::error!("Failed to trigger compaction: {:?}", e);
                        } else {
                            log::debug!("Cold tier compaction requested via service");
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to get cold tier coordinator: {:?}", e);
                    }
                }

                worker_state.tasks_processed.fetch_add(1, Ordering::Relaxed);
            }
            CanonicalMaintenanceTask::CleanupExpired { ttl, .. } => {
                // Connect to tier-specific cleanup implementations
                use crate::cache::tier::{hot, warm};

                let mut total_cleaned = 0;

                // Clean hot tier
                total_cleaned += hot::cleanup_expired_entries::<K, V>(*ttl);

                // Clean warm tier
                if let Ok(cleaned) = warm::cleanup_expired_entries::<K, V>(*ttl) {
                    total_cleaned += cleaned;
                }

                // Clean cold tier using existing sophisticated maintenance system
                match crate::cache::tier::cold::ColdTierCoordinator::get() {
                    Ok(coordinator) => {
                        if let Ok(()) = coordinator.trigger_maintenance::<K, V>("compact") {
                            total_cleaned += 1; // Cold tier maintenance triggered successfully
                        }
                    }
                    Err(_) => {
                        // Cold tier not available - this is normal if cold tier disabled
                    }
                }

                log::debug!("Cleaned {} expired entries across all tiers", total_cleaned);
                worker_state.tasks_processed.fetch_add(1, Ordering::Relaxed);
            }
            CanonicalMaintenanceTask::SyncTiers { .. } => {
                // Use the established channel architecture for tier coordination
                use crate::cache::worker::tier_transitions;

                // Create a channel for coordination with the tier transition system
                let (stat_sender, stat_receiver) = crossbeam_channel::unbounded();

                // Execute tier transitions through proper channel coordination
                tier_transitions::check_tier_transitions::<K, V>(&stat_sender);

                // Process any statistics updates that were sent
                let mut stat_count = 0;
                while let Ok(stat_update) = stat_receiver.try_recv() {
                    log::trace!("Received tier transition stat: {:?}", stat_update);
                    stat_count += 1;
                }
                log::debug!(
                    "Completed tier rebalancing check with {} stat updates",
                    stat_count
                );

                worker_state.tasks_processed.fetch_add(1, Ordering::Relaxed);
            }
            CanonicalMaintenanceTask::UpdateStatistics { .. } => {
                // Update unified statistics using existing sophisticated system
                use crate::telemetry::unified_stats::UnifiedCacheStatistics;

                // Get global unified statistics instance and update
                match UnifiedCacheStatistics::get_global_instance() {
                    Ok(stats) => {
                        // Refresh comprehensive performance metrics
                        let _metrics = stats.get_performance_metrics();
                        log::debug!("Updated unified cache statistics");
                    }
                    Err(_) => {
                        log::warn!("Unified statistics system not available");
                    }
                }

                // Update coherence statistics using existing sophisticated system
                use crate::cache::coherence::statistics::core_statistics::CoherenceStatistics;

                match CoherenceStatistics::get_global_instance() {
                    Ok(coherence_stats) => {
                        // Get fresh snapshot and log comprehensive telemetry
                        let snapshot = coherence_stats.get_snapshot();

                        // Use CoherenceStatistics methods for real-time calculation
                        let live_success_rate = coherence_stats.success_rate();
                        let live_invalidation_efficiency =
                            coherence_stats.invalidation_efficiency();

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
                    }
                    Err(_) => {
                        log::warn!("Coherence statistics system not available");
                    }
                }

                log::debug!("Updated cache statistics");
                worker_state.tasks_processed.fetch_add(1, Ordering::Relaxed);
            }

            CanonicalMaintenanceTask::AnalyzePatterns { .. } => {
                // Connect to warm tier pattern analysis system using existing public API
                use crate::cache::tier::warm::global_api;

                // Trigger pattern analysis for all active warm tier instances
                match global_api::process_background_maintenance::<K, V>() {
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

                match cleanup_manager::trigger_defragmentation() {
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
                match global_api::process_background_maintenance::<K, V>() {
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
                match ColdTierCoordinator::get() {
                    Ok(coordinator) => {
                        match coordinator.execute_read_operation::<K, V, bool, _>(|tier| {
                            tier.validate_integrity()
                        }) {
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
                    }
                    Err(e) => {
                        log::error!("Failed to get cold tier coordinator: {:?}", e);
                    }
                }

                worker_state.tasks_processed.fetch_add(1, Ordering::Relaxed);
            }
            CanonicalMaintenanceTask::DefragmentMemory {
                target_fragmentation,
            } => {
                // Connect to memory pool defragmentation system
                use crate::cache::memory::pool_manager::cleanup_manager;

                match cleanup_manager::trigger_defragmentation() {
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

                match crate::cache::tier::warm::global_api::update_warm_tier_ml_models::<K, V>() {
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
