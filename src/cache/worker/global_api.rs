//! Worker API functions for cache maintenance operations
//!
//! This module provides generic worker API functions that eliminate the global singleton pattern.
//! All functions now require explicit type parameters and work with CacheMaintenanceWorker instances.

use std::sync::Arc;

use super::types::{CacheMaintenanceWorker, MaintenanceTask, WorkerStats};
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};

// REMOVED: Broken global singleton pattern
// - static mut MAINTENANCE_WORKER: Option<CacheMaintenanceWorker> = None;
// - init_maintenance_worker() function
// - submit_maintenance_task() function
// - maintenance_stats() function
// - schedule_maintenance() function
//
// These functions attempted to use a global singleton without generic type parameters,
// which is fundamentally incompatible with the generic architecture.
//
// Users must now create and manage CacheMaintenanceWorker<K, V> instances directly:
// - let worker = CacheMaintenanceWorker::<MyKey, MyValue>::new();
// - worker.submit_task(MaintenanceTask::Promote { key, value });
// - worker.stats();
// - worker.schedule_maintenance();

/// Create a new maintenance worker for specific key-value types
#[inline]
pub fn create_worker<K: CacheKey, V: CacheValue>() -> CacheMaintenanceWorker<K, V> {
    CacheMaintenanceWorker::new()
}

/// Promote entry to warm tier asynchronously
pub fn async_promote<K: CacheKey, V: CacheValue>(
    worker: &CacheMaintenanceWorker<K, V>,
    key: K,
    value: Arc<V>,
) -> Result<(), CacheOperationError> {
    worker.submit_task(MaintenanceTask::Promote { key, value })
}

/// Demote entry to cold tier asynchronously  
pub fn async_demote<K: CacheKey, V: CacheValue>(
    worker: &CacheMaintenanceWorker<K, V>,
    key: K,
    value: Arc<V>,
) -> Result<(), CacheOperationError> {
    worker.submit_task(MaintenanceTask::Demote { key, value })
}

/// Trigger cleanup asynchronously
pub fn async_cleanup<K: CacheKey, V: CacheValue>(
    worker: &CacheMaintenanceWorker<K, V>,
) -> Result<(), CacheOperationError> {
    worker.submit_task(MaintenanceTask::CleanupExpired)
}

/// Trigger compaction asynchronously
pub fn async_compact<K: CacheKey, V: CacheValue>(
    worker: &CacheMaintenanceWorker<K, V>,
) -> Result<(), CacheOperationError> {
    worker.submit_task(MaintenanceTask::CompactColdTier)
}

/// Schedule automatic maintenance for specific types
pub fn schedule_maintenance<K: CacheKey, V: CacheValue>(
    worker: &CacheMaintenanceWorker<K, V>,
) -> Result<(), CacheOperationError> {
    worker.schedule_maintenance()
}

/// Get worker statistics
pub fn get_worker_stats<K: CacheKey, V: CacheValue>(
    worker: &CacheMaintenanceWorker<K, V>,
) -> WorkerStats {
    worker.stats()
}
