//! Pool cleanup manager that integrates with existing sophisticated systems
//!
//! This module provides the PoolCleanupManager that coordinates memory pool cleanup
//! by integrating with existing FragmentationAnalyzer, MaintenanceScheduler,
//! AllocationStatistics, and other production-ready systems.

use std::sync::Arc;

use super::individual_pool::MemoryPool;
use crate::cache::manager::background::types::{MaintenanceScheduler, MaintenanceTaskType};
use crate::cache::memory::allocation_manager::AllocationStatistics;
use crate::cache::memory::efficiency_analyzer::MemoryEfficiencyAnalyzer;
use crate::cache::tier::hot::memory_pool::statistics::MemoryPoolStats;
use crate::cache::traits::types_and_enums::CacheOperationError;

/// Pool cleanup manager that integrates with existing sophisticated systems
#[derive(Debug)]
pub struct PoolCleanupManager {
    /// Use existing fragmentation analyzer (don't duplicate)
    fragmentation_analyzer: Arc<MemoryEfficiencyAnalyzer>,
    /// Leverage existing maintenance task system
    maintenance_scheduler: Arc<MaintenanceScheduler>,
    /// Integration with allocation statistics
    allocation_stats: Arc<AllocationStatistics>,
}

/// Decision result from cleanup analysis
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
        fragmentation_analyzer: Arc<MemoryEfficiencyAnalyzer>,
        maintenance_scheduler: Arc<MaintenanceScheduler>,
        allocation_stats: Arc<AllocationStatistics>,
    ) -> Self {
        Self {
            fragmentation_analyzer,
            maintenance_scheduler,
            allocation_stats,
        }
    }

    /// Sophisticated cleanup that integrates with existing systems
    pub fn try_sophisticated_cleanup(&self, pool: &MemoryPool) -> Result<bool, CacheOperationError> {
        // Use existing fragmentation analysis
        let compaction_decision = self.should_compact(pool)?;

        match compaction_decision {
            CompactionDecision::Required => {
                // Schedule urgent memory defragmentation using existing MaintenanceScheduler
                match self.maintenance_scheduler.submit_urgent_task(MaintenanceTaskType::MemoryDefragmentation) {
                    Ok(()) => {
                        // Update existing allocation statistics
                        let current_fragmentation = self.allocation_stats.fragmentation_level() * 100.0;
                        // Track that we initiated cleanup
                        self.update_fragmentation_tracking(current_fragmentation as u32)?;
                        Ok(true)
                    }
                    Err(_) => {
                        // Fallback to optimize memory task if defragmentation queue is full
                        self.maintenance_scheduler
                            .submit_task(MaintenanceTaskType::OptimizeMemory, 100)?;
                        Ok(true)
                    }
                }
            }
            CompactionDecision::Recommended => {
                // Schedule normal priority cleanup using existing MaintenanceScheduler
                self.maintenance_scheduler
                    .submit_task(MaintenanceTaskType::OptimizeMemory, 200)?;
                Ok(true)
            }
            CompactionDecision::NotNeeded => Ok(false),
        }
    }

    /// Emergency cleanup coordination across multiple pools
    pub fn try_emergency_cleanup_coordination(&self, pools: &[&MemoryPool]) -> Result<bool, CacheOperationError> {
        let mut cleanup_performed = false;

        for pool in pools {
            // Use existing fragmentation analysis for each pool
            let pool_stats = self.get_pool_stats(pool)?;
            
            // Use existing should_compact() logic
            if pool_stats.should_compact() {
                // Schedule urgent defragmentation for this pool
                match self.maintenance_scheduler.submit_urgent_task(MaintenanceTaskType::MemoryDefragmentation) {
                    Ok(()) => {
                        cleanup_performed = true;
                        // Update statistics using existing allocation stats integration
                        self.record_cleanup_attempt()?;
                    }
                    Err(_) => {
                        // Try general optimization if defragmentation is unavailable
                        self.maintenance_scheduler
                            .submit_task(MaintenanceTaskType::OptimizeMemory, 50)?;
                        cleanup_performed = true;
                    }
                }
            }
        }

        Ok(cleanup_performed)
    }

    /// Determine if compaction is needed using existing sophisticated analysis
    fn should_compact(&self, pool: &MemoryPool) -> Result<CompactionDecision, CacheOperationError> {
        // Use existing FragmentationAnalyzer methods (don't reimplement)
        let fragmentation_impact = self.fragmentation_analyzer
            .get_efficiency_snapshot().1; // Gets fragmentation from existing analyzer

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
        let fragmentation_ratio = self.allocation_stats.fragmentation_level() as f64;

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
    fn update_fragmentation_tracking(&self, fragmentation_level: u32) -> Result<(), CacheOperationError> {
        // Update existing AllocationStatistics using public method
        let level_as_f32 = fragmentation_level as f32 / 100.0;
        self.allocation_stats.update_fragmentation_level(level_as_f32);
        Ok(())
    }

    /// Record cleanup attempt using existing statistics infrastructure
    fn record_cleanup_attempt(&self) -> Result<(), CacheOperationError> {
        // Update existing MaintenanceStats operations_executed counter
        self.maintenance_scheduler.get_stats()
            .operations_executed
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    /// Check if maintenance scheduler is available for cleanup tasks
    pub fn is_cleanup_available(&self) -> bool {
        self.maintenance_scheduler.is_healthy()
    }

    /// Get cleanup statistics from existing maintenance infrastructure
    pub fn get_cleanup_stats(&self) -> (u64, u64, f64) {
        let maintenance_stats = self.maintenance_scheduler.get_stats();
        (
            maintenance_stats.operations_executed.load(std::sync::atomic::Ordering::Relaxed),
            maintenance_stats.failed_operations.load(std::sync::atomic::Ordering::Relaxed),
            self.allocation_stats.fragmentation_level() as f64,
        )
    }
}