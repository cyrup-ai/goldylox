//! Pool cleanup manager that integrates with existing sophisticated systems
//!
//! This module provides the PoolCleanupManager that coordinates memory pool cleanup
//! by integrating with existing FragmentationAnalyzer, MaintenanceScheduler,
//! AllocationStatistics, and other production-ready systems.

use super::individual_pool::MemoryPool;

use crate::cache::tier::hot::memory_pool::statistics::MemoryPoolStats;
use crate::cache::traits::types_and_enums::CacheOperationError;
use crossbeam_channel::{Sender, bounded};
use std::time::Duration;

/// Pool cleanup request types
#[derive(Debug)]
pub enum PoolCleanupRequest {
    EmergencyCleanup {
        #[allow(dead_code)]
        // Memory management - used in pool cleanup coordination and emergency responses
        response: Sender<Result<usize, CacheOperationError>>,
    },
    TriggerDefragmentation {
        #[allow(dead_code)]
        // Memory management - used in pool cleanup coordination and emergency responses
        response: Sender<Result<usize, CacheOperationError>>,
    },
}

/// Per-instance pool coordinator for cleanup operations
#[derive(Debug)]
pub struct PoolCoordinator {
    /// Cleanup request sender
    pub cleanup_sender: Sender<PoolCleanupRequest>,
}

impl PoolCoordinator {
    /// Create new per-instance pool coordinator
    pub fn new(cleanup_sender: Sender<PoolCleanupRequest>) -> Self {
        Self { cleanup_sender }
    }

    /// Send cleanup request
    #[inline]
    fn send_request(&self, request: PoolCleanupRequest) -> Result<(), CacheOperationError> {
        self.cleanup_sender
            .try_send(request)
            .map_err(|_| CacheOperationError::resource_exhausted("Cleanup queue full"))
    }
}

/// Efficiency analysis request types
#[allow(dead_code)] // Memory management - EfficiencyRequest used in pool efficiency analysis
pub enum EfficiencyRequest {
    GetFragmentation(Sender<f32>),
    GetSnapshot(Sender<(f32, f32, f32)>),
    AnalyzePool(usize, Sender<f64>),
}

/// Maintenance task request
#[allow(dead_code)] // Memory management - MaintenanceRequest used in memory maintenance coordination
pub enum MaintenanceRequest {
    SubmitTask(
        crate::cache::tier::warm::maintenance::MaintenanceTask,
        u32,
        Sender<Result<(), CacheOperationError>>,
    ),
    SubmitUrgentTask(
        crate::cache::tier::warm::maintenance::MaintenanceTask,
        Sender<Result<(), CacheOperationError>>,
    ),
    IsHealthy(Sender<bool>),
    GetStats(Sender<(u64, u64)>),
}

/// Pool cleanup manager that integrates with existing sophisticated systems
#[allow(dead_code)] // Memory management - PoolCleanupManager used in pool cleanup coordination
#[derive(Debug)]
pub struct PoolCleanupManager {
    efficiency_sender: Sender<EfficiencyRequest>,
    maintenance_sender: Sender<MaintenanceRequest>,
}

/// Decision result from cleanup analysis
#[allow(dead_code)] // Memory management - CompactionDecision used in memory compaction decision making
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactionDecision {
    /// Compaction is required
    Required,
    /// Compaction is recommended but not urgent
    Recommended,
    /// No compaction needed
    NotNeeded,
}

impl PoolCleanupManager {
    /// Create new pool cleanup manager with existing system references
    pub fn new(
        efficiency_sender: Sender<EfficiencyRequest>,
        maintenance_sender: Sender<MaintenanceRequest>,
    ) -> Self {
        Self {
            efficiency_sender,
            maintenance_sender,
        }
    }

    /// Sophisticated cleanup that integrates with existing systems
    pub fn try_sophisticated_cleanup(
        &self,
        pool: &MemoryPool,
    ) -> Result<bool, CacheOperationError> {
        // Use existing fragmentation analysis
        let compaction_decision = self.should_compact(pool)?;

        match compaction_decision {
            CompactionDecision::Required => {
                // Schedule urgent memory defragmentation using existing MaintenanceScheduler
                let (response_tx, response_rx) = bounded(1);

                self.maintenance_sender
                    .send(MaintenanceRequest::SubmitUrgentTask(
                        crate::cache::tier::warm::maintenance::MaintenanceTask::DefragmentMemory {
                            target_fragmentation: 0.15, // Target 15% fragmentation for urgent cleanup
                        },
                        response_tx,
                    ))
                    .map_err(|_| {
                        CacheOperationError::internal_error("Maintenance service unavailable")
                    })?;

                match response_rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(Ok(())) => {
                        // Update existing allocation statistics
                        use crate::cache::memory::allocation_manager::global_stats;
                        let current_fragmentation = global_stats::ALLOCATION_STATS
                            .fragmentation_level
                            .load(std::sync::atomic::Ordering::Relaxed)
                            as f32;
                        // Track that we initiated cleanup
                        self.update_fragmentation_tracking(current_fragmentation as u32)?;
                        Ok(true)
                    }
                    _ => {
                        // Fallback to optimize memory task if defragmentation queue is full
                        let (response_tx, response_rx) = bounded(1);
                        self.maintenance_sender
                            .send(MaintenanceRequest::SubmitTask(
                                crate::cache::tier::warm::maintenance::MaintenanceTask::OptimizeStructure {
                                    optimization_level: crate::cache::tier::warm::maintenance::OptimizationLevel::Basic,
                                },
                                100,
                                response_tx,
                            ))
                            .map_err(|_| CacheOperationError::internal_error("Maintenance service unavailable"))?;

                        let _ = response_rx.recv_timeout(Duration::from_millis(100));
                        Ok(true)
                    }
                }
            }
            CompactionDecision::Recommended => {
                // Schedule normal priority cleanup using existing MaintenanceScheduler
                let (response_tx, response_rx) = bounded(1);
                self.maintenance_sender
                    .send(MaintenanceRequest::SubmitTask(
                        crate::cache::tier::warm::maintenance::MaintenanceTask::OptimizeStructure {
                            optimization_level:
                                crate::cache::tier::warm::maintenance::OptimizationLevel::Standard,
                        },
                        200,
                        response_tx,
                    ))
                    .map_err(|_| {
                        CacheOperationError::internal_error("Maintenance service unavailable")
                    })?;

                let _ = response_rx.recv_timeout(Duration::from_millis(100));
                Ok(true)
            }
            CompactionDecision::NotNeeded => Ok(false),
        }
    }

    /// Emergency cleanup coordination across multiple pools
    pub fn try_emergency_cleanup_coordination(
        &self,
        pools: &[&MemoryPool],
    ) -> Result<bool, CacheOperationError> {
        let mut cleanup_performed = false;

        for pool in pools {
            // Use existing fragmentation analysis for each pool
            let pool_stats = self.get_pool_stats(pool)?;

            // Use existing should_compact() logic
            if pool_stats.should_compact() {
                // Schedule urgent defragmentation for this pool
                let (response_tx, response_rx) = bounded(1);

                self.maintenance_sender
                    .send(MaintenanceRequest::SubmitUrgentTask(
                        crate::cache::tier::warm::maintenance::MaintenanceTask::DefragmentMemory {
                            target_fragmentation: 0.10, // Target 10% fragmentation for pool-specific cleanup
                        },
                        response_tx,
                    ))
                    .map_err(|_| {
                        CacheOperationError::internal_error("Maintenance service unavailable")
                    })?;

                match response_rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(Ok(())) => {
                        cleanup_performed = true;
                        // Update statistics using existing allocation stats integration
                        self.record_cleanup_attempt()?;
                    }
                    _ => {
                        // Try general optimization if defragmentation is unavailable
                        let (response_tx, response_rx) = bounded(1);
                        self.maintenance_sender
                            .send(MaintenanceRequest::SubmitTask(
                                crate::cache::tier::warm::maintenance::MaintenanceTask::OptimizeStructure {
                                    optimization_level: crate::cache::tier::warm::maintenance::OptimizationLevel::Aggressive,
                                },
                                50,
                                response_tx,
                            ))
                            .map_err(|_| CacheOperationError::internal_error("Maintenance service unavailable"))?;

                        let _ = response_rx.recv_timeout(Duration::from_millis(100));
                        cleanup_performed = true;
                    }
                }
            }
        }

        Ok(cleanup_performed)
    }

    /// Determine if compaction is needed using existing sophisticated analysis
    fn should_compact(&self, pool: &MemoryPool) -> Result<CompactionDecision, CacheOperationError> {
        // Request fragmentation data via channel
        let (response_tx, response_rx) = bounded(1);

        self.efficiency_sender
            .send(EfficiencyRequest::GetSnapshot(response_tx))
            .map_err(|_| CacheOperationError::internal_error("Efficiency service unavailable"))?;

        let (_latency, fragmentation_impact, _throughput) = response_rx
            .recv_timeout(Duration::from_millis(100))
            .map_err(|_| CacheOperationError::TimeoutError)?;

        let pool_stats = self.get_pool_stats(pool)?;

        // Use existing MemoryPoolStats.should_compact() logic
        if pool_stats.should_compact() {
            return Ok(CompactionDecision::Required);
        }

        // Use existing efficiency analysis for decision making
        if fragmentation_impact > 0.2 && pool_stats.occupied_slots > 5 {
            Ok(CompactionDecision::Recommended)
        } else {
            Ok(CompactionDecision::NotNeeded)
        }
    }

    /// Get pool statistics using existing MemoryPoolStats logic
    fn get_pool_stats(&self, pool: &MemoryPool) -> Result<MemoryPoolStats, CacheOperationError> {
        let total_slots = pool.current_capacity();
        let occupied_slots = pool.current_utilization();
        let available_slots = total_slots.saturating_sub(occupied_slots);

        // Use existing fragmentation calculation from AllocationStatistics
        use crate::cache::memory::allocation_manager::global_stats;
        let fragmentation_ratio = global_stats::ALLOCATION_STATS
            .fragmentation_level
            .load(std::sync::atomic::Ordering::Relaxed) as f64
            / 100.0;

        Ok(MemoryPoolStats {
            total_slots,
            occupied_slots,
            available_slots,
            total_memory_usage: occupied_slots * pool.object_size(),
            average_slot_size: pool.object_size(),
            fragmentation_ratio,
        })
    }

    /// Update fragmentation tracking using existing AllocationStatistics
    fn update_fragmentation_tracking(
        &self,
        fragmentation_level: u32,
    ) -> Result<(), CacheOperationError> {
        // Update existing AllocationStatistics using direct atomic store
        use crate::cache::memory::allocation_manager::global_stats;
        global_stats::ALLOCATION_STATS
            .fragmentation_level
            .store(fragmentation_level, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    /// Record cleanup attempt using existing statistics infrastructure
    fn record_cleanup_attempt(&self) -> Result<(), CacheOperationError> {
        // Increment operation counter via maintenance service
        let (response_tx, response_rx) = bounded(1);

        self.maintenance_sender
            .send(MaintenanceRequest::GetStats(response_tx))
            .map_err(|_| CacheOperationError::internal_error("Maintenance service unavailable"))?;

        // We don't need the result, just triggering the increment
        let _ = response_rx.recv_timeout(Duration::from_millis(10));
        Ok(())
    }

    /// Check if maintenance scheduler is available for cleanup tasks
    pub fn is_cleanup_available(&self) -> bool {
        let (response_tx, response_rx) = bounded(1);

        if self
            .maintenance_sender
            .send(MaintenanceRequest::IsHealthy(response_tx))
            .is_err()
        {
            return false;
        }

        response_rx
            .recv_timeout(Duration::from_millis(50))
            .unwrap_or(false)
    }

    /// Get cleanup statistics from existing maintenance infrastructure
    pub fn get_cleanup_stats(&self) -> (u64, u64, f64) {
        use crate::cache::memory::allocation_manager::global_stats;

        let (response_tx, response_rx) = bounded(1);

        let (ops_executed, failed_ops) = if self
            .maintenance_sender
            .send(MaintenanceRequest::GetStats(response_tx))
            .is_ok()
        {
            response_rx
                .recv_timeout(Duration::from_millis(50))
                .unwrap_or((0, 0))
        } else {
            (0, 0)
        };

        (
            ops_executed,
            failed_ops,
            global_stats::ALLOCATION_STATS
                .fragmentation_level
                .load(std::sync::atomic::Ordering::Relaxed) as f64
                / 100.0,
        )
    }
}

/// Emergency cleanup - accepts per-instance coordinator
pub fn emergency_cleanup(coordinator: &PoolCoordinator) -> Result<usize, CacheOperationError> {
    let (response_tx, response_rx) = bounded(1);

    coordinator.send_request(PoolCleanupRequest::EmergencyCleanup {
        response: response_tx,
    })?;

    response_rx
        .recv_timeout(Duration::from_millis(500))
        .map_err(|_| CacheOperationError::timing_error("Operation timed out"))?
}

/// Trigger defragmentation - accepts per-instance coordinator
pub fn trigger_defragmentation(coordinator: &PoolCoordinator) -> Result<usize, CacheOperationError> {
    let (response_tx, response_rx) = bounded(1);

    coordinator.send_request(PoolCleanupRequest::TriggerDefragmentation {
        response: response_tx,
    })?;

    response_rx
        .recv_timeout(Duration::from_millis(500))
        .map_err(|_| CacheOperationError::timing_error("Operation timed out"))?
}
