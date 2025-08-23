//! Cache worker coordination and async infrastructure
//!
//! This module provides high-performance async worker infrastructure for cache operations,
//! including work-stealing task pools, command queues, and NUMA-aware coordination.

// async_infrastructure module deleted - use BackgroundCoordinator directly
pub mod global_api;
pub mod task_coordination;
pub mod tier_transitions;
pub mod types;

// Re-export key types for convenience
use log::{info, debug};

// NO RE-EXPORTS
// Applications must use direct imports:
// use crate::cache::coordinator::background_coordinator::BackgroundCoordinator;
// use crate::cache::manager::background::types::{BackgroundTask, MaintenanceTask};
use crate::cache::memory::{MemoryManager, MemoryMonitoringProcessor};
pub use task_coordination::{
    CacheCommand, CacheCommandQueue, CommandQueueStatsSnapshot, CoordinatorStatsSnapshot,
    TaskCoordinator, TaskExecutionContext,
};
// Note: GlobalCacheWorker and WorkerConfiguration don't exist in global_api
// Note: TierTransitionManager and TransitionStrategy don't exist in tier_transitions
// These modules only export functions, not types
pub use types::{CacheMaintenanceWorker, MaintenanceTask, WorkerStats};

use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};
use crate::cache::config::types::CacheConfig;
use crate::cache::coordinator::background_coordinator::BackgroundCoordinator;

/// Initialize the global cache worker system with configuration
pub fn initialize_worker_system_with_config<K: CacheKey, V: CacheValue>(
    config: &CacheConfig,
    max_command_queue_size: usize,
) -> Result<TaskCoordinator<K, V>, CacheOperationError> {
    info!("Initializing worker system");
    
    // Create appropriate coordinator based on monitoring config
    if config.memory_config.monitoring_enabled {
        info!("Memory monitoring enabled, creating monitoring processor");
        
        // Create coordinator with memory monitoring processor
        let mut background_coordinator = BackgroundCoordinator::<K, V, MemoryMonitoringProcessor>::new(config)?;
        
        // Create processor that will be OWNED by processor thread
        let memory_manager = MemoryManager::new(config)?;
        let processor = MemoryMonitoringProcessor::new(memory_manager);
        
        // Start processor thread that OWNS the processor
        background_coordinator.start_processor(processor)?;
        background_coordinator.start_worker_threads()?;
        
        // Start periodic memory monitoring
        let monitoring_task = crate::cache::manager::background::types::BackgroundTask::Statistics {
            stats_type: 2, // Memory monitoring type
            interval_ms: config.memory_config.sample_interval_ms,
        };
        
        debug!("Submitting periodic memory monitoring task");
        background_coordinator.submit_task(monitoring_task)?;
        
        Ok(TaskCoordinator::new(background_coordinator, max_command_queue_size))
    } else {
        info!("Memory monitoring disabled, creating basic coordinator");
        
        // Create coordinator without processor
        let background_coordinator = BackgroundCoordinator::<K, V>::new(config)?;
        background_coordinator.start_worker_threads()?;
        Ok(TaskCoordinator::new(background_coordinator, max_command_queue_size))
    }
}


