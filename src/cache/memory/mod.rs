//! Memory management module with decomposed submodules
//!
//! This module coordinates memory allocation, pool management, garbage collection,
//! pressure monitoring, and efficiency analysis through specialized submodules.

use std::ptr::NonNull;
use log::{error, warn, info};

use super::config::CacheConfig;
use crate::cache::traits::types_and_enums::CacheOperationError;

// Submodule declarations
pub mod allocation_manager;
pub mod allocation_stats;
pub mod efficiency_analyzer;
pub mod gc_coordinator;
pub mod memory_pools;
pub mod pool_manager;
pub mod pressure_monitor;
pub mod types;

// Re-export key types and functions
pub use allocation_manager::*;
pub use allocation_stats::AllocationStatistics;
pub use efficiency_analyzer::MemoryEfficiencyAnalyzer;
pub use gc_coordinator::GCCoordinator;
pub use memory_pools::{MemoryPool, MemoryPoolManager};
pub use pool_manager::*;
pub use pressure_monitor::{
    MemoryPressureMonitor, MemoryAlert, PressureThresholds, MemoryAlertSystem,
    MemoryUsageHistory, TrendAnalysis, create_advanced_pressure_monitor, create_default_pressure_monitor,
    get_system_memory_with_fallback
};
pub use types::{
    EfficiencyAnalysisResult, GCTask, GCTaskType, MemoryError, MemoryStatistics, PressureSample,
};

use crate::cache::manager::background::types::{BackgroundTask, TaskProcessor};

/// Advanced memory manager with atomic allocation tracking
#[derive(Debug)]
pub struct MemoryManager {
    /// Global allocation statistics
    allocation_stats: AllocationStatistics,
    /// Memory pool manager for efficient allocation
    pool_manager: MemoryPoolManager,
    /// Memory pressure monitor with adaptive thresholds
    pressure_monitor: Option<MemoryPressureMonitor>,
    /// Garbage collection coordinator
    gc_coordinator: GCCoordinator,
    /// Memory efficiency analyzer
    efficiency_analyzer: MemoryEfficiencyAnalyzer,
}

impl MemoryManager {
    /// Create new memory manager with configuration
    pub fn new(config: &CacheConfig) -> Result<Self, CacheOperationError> {
        Ok(Self {
            allocation_stats: AllocationStatistics::new(),
            pool_manager: MemoryPoolManager::new(config)?,
            pressure_monitor: if config.memory_config.monitoring_enabled {
                Some(create_advanced_pressure_monitor(config)?)
            } else {
                None
            },
            gc_coordinator: GCCoordinator::new(config)?,
            efficiency_analyzer: MemoryEfficiencyAnalyzer::new(config)?,
        })
    }

    /// Allocate memory with optimal pool selection
    pub fn allocate(&self, size: usize) -> Result<NonNull<u8>, CacheOperationError> {
        let timer = std::time::Instant::now();

        // Update allocation statistics
        self.allocation_stats.record_allocation(size as u64);

        // Select appropriate pool based on size
        let allocation_result = if size < 1024 {
            self.pool_manager.allocate_small(size)
        } else if size < 65536 {
            self.pool_manager.allocate_medium(size)
        } else {
            self.pool_manager.allocate_large(size)
        };

        match allocation_result {
            Ok(ptr) => {
                // Update peak allocation if necessary
                self.allocation_stats.update_peak_allocation();

                // Record allocation in advanced pressure monitor
                if let Some(ref monitor) = self.pressure_monitor {
                    monitor.record_allocation(size);
                }

                // Record allocation latency
                let allocation_latency = timer.elapsed().as_nanos() as u64;
                self.efficiency_analyzer
                    .record_allocation_latency(allocation_latency, size);

                Ok(ptr)
            }
            Err(err) => {
                // Handle allocation failure
                self.allocation_stats.record_allocation_failure();
                Err(err)
            }
        }
    }

    /// Deallocate memory with pool coordination
    pub fn deallocate(&self, ptr: NonNull<u8>, size: usize) -> Result<(), CacheOperationError> {
        // Update deallocation statistics
        self.allocation_stats.record_deallocation(size as u64);

        // Record deallocation in advanced pressure monitor
        if let Some(ref monitor) = self.pressure_monitor {
            monitor.record_deallocation(size);
        }

        // Return memory to appropriate pool
        if size < 1024 {
            self.pool_manager.deallocate_small(ptr, size)
        } else if size < 65536 {
            self.pool_manager.deallocate_medium(ptr, size)
        } else {
            self.pool_manager.deallocate_large(ptr, size)
        }
    }

    /// Get current memory statistics
    pub fn get_memory_stats(&self) -> MemoryStatistics {
        self.allocation_stats.get_statistics()
    }

    /// Monitor memory pressure and trigger appropriate actions
    pub fn monitor_memory_pressure(&self) -> Result<(), CacheOperationError> {
        let Some(ref monitor) = self.pressure_monitor else {
            return Ok(()); // Monitoring disabled
        };
        
        let current_usage = self.get_current_memory_usage();
        monitor.update_memory_usage(current_usage)?;

        // Check pressure level and trigger appropriate actions
        let pressure = monitor.get_pressure();
        if pressure >= 0.95 {
            // Critical pressure - trigger emergency GC
            self.gc_coordinator.trigger_emergency_gc()?;
        } else if monitor.should_evict() {
            // High pressure - trigger normal GC
            self.gc_coordinator.schedule_normal_gc()?;
        }

        // Process any memory alerts
        self.process_memory_alerts();

        Ok(())
    }

    /// Analyze memory efficiency and provide optimization recommendations
    pub fn analyze_efficiency(&self) -> EfficiencyAnalysisResult {
        self.efficiency_analyzer
            .analyze_current_efficiency(&self.allocation_stats, &self.pool_manager)
    }

    /// Get current memory usage in bytes
    fn get_current_memory_usage(&self) -> u64 {
        let stats = self.allocation_stats.get_statistics();
        stats.total_allocated
    }

    /// Process memory alerts from the advanced monitoring system
    fn process_memory_alerts(&self) {
        if let Some(ref monitor) = self.pressure_monitor {
            while let Some(alert) = monitor.try_receive_alert() {
                match alert {
                    MemoryAlert::CriticalPressure { recommended_action, .. } => {
                        error!("Critical memory pressure detected: {}", recommended_action);
                        if let Err(e) = self.gc_coordinator.trigger_emergency_gc() {
                            error!("Emergency GC failed during critical pressure: {:?}", e);
                            // Log failure but continue processing other alerts
                        }
                    },
                    MemoryAlert::HighPressure { time_to_limit_sec, .. } => {
                        warn!("High memory pressure - time to limit: {:.1}s", time_to_limit_sec);
                        if let Err(e) = self.gc_coordinator.schedule_normal_gc() {
                            warn!("Failed to schedule GC during high pressure: {:?}", e);
                            // Attempt emergency GC as fallback
                            if let Err(emergency_err) = self.gc_coordinator.trigger_emergency_gc() {
                                error!("Emergency GC fallback also failed: {:?}", emergency_err);
                            }
                        }
                    },
                    MemoryAlert::MemoryLeak { growth_rate_mb_per_min, .. } => {
                        warn!("Memory leak detected: {:.1} MB/min growth rate", growth_rate_mb_per_min);
                    },
                    _ => {
                        info!("Memory monitoring alert: {:?}", alert);
                    }
                }
            }
        }
    }
}

/// Memory monitoring task processor using channel communication
pub struct MemoryMonitoringProcessor {
    task_sender: crossbeam_channel::Sender<MemoryMonitoringTask>,
}

impl MemoryMonitoringProcessor {
    pub fn new(task_sender: crossbeam_channel::Sender<MemoryMonitoringTask>) -> Self {
        Self { task_sender }
    }
}

/// Memory monitoring task
#[derive(Debug, Clone)]
pub enum MemoryMonitoringTask {
    MonitorPressure,
}

impl TaskProcessor for MemoryMonitoringProcessor {
    fn process_task(&self, task: &BackgroundTask) -> Result<(), CacheOperationError> {
        // Send task via channel instead of direct access
        self.task_sender.send(MemoryMonitoringTask::MonitorPressure)
            .map_err(|_| CacheOperationError::Internal("Failed to send monitoring task".into()))
    }
}
