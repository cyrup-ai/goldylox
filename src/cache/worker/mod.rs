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
use std::sync::Arc;

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
) -> Result<Arc<TaskCoordinator<K, V>>, CacheOperationError> {
    let mut background_coordinator = BackgroundCoordinator::new(config)?;
    
    // Create and wire up memory monitoring processor if enabled
    if config.memory_config.monitoring_enabled {
        let memory_manager = Arc::new(MemoryManager::new(config)?);
        let processor = Arc::new(MemoryMonitoringProcessor::new(memory_manager));
        background_coordinator.set_task_processor(processor);
    }
    
    let background_coordinator = Arc::new(background_coordinator);
    background_coordinator.start_worker_threads()?;
    
    // Start periodic memory monitoring if enabled
    if config.memory_config.monitoring_enabled {
        let monitoring_task = crate::cache::manager::background::types::BackgroundTask::Statistics {
            stats_type: 2, // Memory monitoring type
            interval_ms: config.memory_config.sample_interval_ms,
        };
        background_coordinator.submit_task(monitoring_task)?;
    }
    
    let coordinator = Arc::new(TaskCoordinator::new(background_coordinator, max_command_queue_size));
    Ok(coordinator)
}


/// Shutdown the worker system gracefully
pub fn shutdown_worker_system<K: CacheKey, V: CacheValue>(
    coordinator: Arc<TaskCoordinator<K, V>>,
) -> Result<(), CacheOperationError> {
    coordinator.shutdown()
}
