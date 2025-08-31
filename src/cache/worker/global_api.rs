//! Worker API functions for cache maintenance operations
//!
//! This module provides worker API functions for maintenance operations.
//! Functions work directly with tier transition APIs for promotion/demotion.

use super::types::{CacheMaintenanceWorker, MaintenanceTask, WorkerStatsSnapshot};
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};

/// Create a new maintenance worker
#[inline]
pub fn create_worker() -> CacheMaintenanceWorker {
    CacheMaintenanceWorker::new()
}

/// Promote entry to hot tier directly using tier API
pub fn async_promote<K: CacheKey + Default + 'static, V: CacheValue + 'static>(
    key: K,
    value: V,
) -> Result<(), CacheOperationError> {
    // Use hot tier API to put the value directly
    crate::cache::tier::hot::thread_local::simd_hot_put(key, value)
}

/// Demote entry to cold tier directly using tier API  
pub fn async_demote<K: CacheKey + bincode::Encode, V: CacheValue + bincode::Decode<()> + bincode::Encode + serde::de::DeserializeOwned>(
    key: &K,
    value: V,
) -> Result<(), CacheOperationError> {
    // Use cold tier storage API directly
    use crate::cache::tier::cold::storage::ColdTierCache;
    use crate::cache::traits::CompressionAlgorithm;
    use std::path::Path;
    
    // Create a temporary cold tier cache to store the value
    let cold_cache = ColdTierCache::<K, V>::new(
        Path::new("/tmp/cold_storage.dat"),
        CompressionAlgorithm::None,
    )?;
    
    cold_cache.put(key.clone(), value)?;
    Ok(())
}

/// Schedule maintenance task with worker
pub fn schedule_maintenance(
    worker: &CacheMaintenanceWorker,
    task: MaintenanceTask,
) -> Result<(), CacheOperationError> {
    worker.submit_task(task)
}

/// Trigger cleanup of expired entries
pub fn async_cleanup(
    worker: &CacheMaintenanceWorker,
    ttl: std::time::Duration,
) -> Result<(), CacheOperationError> {
    worker.submit_task(MaintenanceTask::CleanupExpired { 
        ttl, 
        batch_size: 100 
    })
}

/// Trigger compaction
pub fn async_compact(
    worker: &CacheMaintenanceWorker,
    threshold: f64,
) -> Result<(), CacheOperationError> {
    worker.submit_task(MaintenanceTask::CompactStorage { 
        compaction_threshold: threshold 
    })
}

/// Get worker statistics
pub fn get_worker_stats(
    worker: &mut CacheMaintenanceWorker,
) -> WorkerStatsSnapshot {
    worker.stats()
}