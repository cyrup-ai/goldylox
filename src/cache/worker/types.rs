//! Core types and data structures for cache maintenance worker
//!
//! This module defines the main data structures used for background cache
//! maintenance, task processing, and worker statistics tracking.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;

use crossbeam::channel::Sender;

use crate::cache::traits::{CacheKey, CacheValue};

/// Background maintenance worker for cache system
pub struct CacheMaintenanceWorker<K: CacheKey, V: CacheValue> {
    /// Worker thread handle
    pub worker_handle: Option<JoinHandle<()>>,
    /// Shutdown signal
    pub shutdown: Arc<AtomicBool>,
    /// Task channel sender
    pub task_sender: Sender<MaintenanceTask<K, V>>,
    /// Worker statistics
    pub stats: Arc<Mutex<WorkerStats>>,
}

impl<K: CacheKey, V: CacheValue> CacheMaintenanceWorker<K, V> {
    /// Create a new maintenance worker
    pub fn new() -> Self {
        let (task_sender, _) = crossbeam::channel::unbounded();
        Self {
            worker_handle: None,
            shutdown: Arc::new(AtomicBool::new(false)),
            task_sender,
            stats: Arc::new(Mutex::new(WorkerStats::default())),
        }
    }

    /// Submit a maintenance task
    pub fn submit_task(
        &self,
        task: MaintenanceTask<K, V>,
    ) -> Result<(), crate::cache::types::CacheOperationError> {
        self.task_sender
            .send(task)
            .map_err(|_| crate::cache::types::CacheOperationError::OperationFailed)
    }

    /// Get worker statistics
    pub fn stats(&self) -> WorkerStats {
        match self.stats.lock() {
            Ok(stats) => stats.clone(),
            Err(poisoned) => {
                // Handle mutex poisoning gracefully - return recovered data or default
                eprintln!("Warning: Worker stats mutex poisoned, recovering data");
                poisoned.into_inner().clone()
            }
        }
    }

    /// Schedule automatic maintenance
    pub fn schedule_maintenance(&self) -> Result<(), crate::cache::types::CacheOperationError> {
        // Submit various maintenance tasks
        self.submit_task(MaintenanceTask::CleanupExpired)?;
        self.submit_task(MaintenanceTask::UpdateStatistics)?;
        Ok(())
    }
}

/// Maintenance task types
#[derive(Debug, Clone)]
pub enum MaintenanceTask<K: CacheKey, V: CacheValue> {
    /// Promote entry from cold to warm tier
    Promote { key: K, value: Arc<V> },
    /// Demote entry from warm to cold tier
    Demote { key: K, value: Arc<V> },
    /// Cleanup expired entries
    CleanupExpired,
    /// Compact cold tier storage
    CompactColdTier,
    /// Optimize cache layout
    OptimizeLayout,
    /// Update cache statistics
    UpdateStatistics,
}

/// Worker performance statistics
#[derive(Debug, Clone)]
pub struct WorkerStats {
    /// Total tasks processed
    pub tasks_processed: u64,
    /// Tasks currently queued
    pub tasks_queued: u64,
    /// Average task processing time
    pub avg_task_time_ns: u64,
    /// Worker uptime
    pub uptime_seconds: u64,
    /// Last maintenance run
    pub last_maintenance: Option<Instant>,
    /// Total promotions performed
    pub promotions: u64,
    /// Total demotions performed
    pub demotions: u64,
    /// Total cleanup operations
    pub cleanups: u64,
    /// Total tier transitions performed
    pub tier_transitions: u64,
    /// Last tier transition check
    pub last_tier_check: Option<Instant>,
    /// Total entries cleaned across all operations
    pub entries_cleaned: u64,
    /// Last cleanup operation timestamp
    pub last_cleanup: Option<Instant>,
}

impl Default for WorkerStats {
    fn default() -> Self {
        Self {
            tasks_processed: 0,
            tasks_queued: 0,
            avg_task_time_ns: 0,
            uptime_seconds: 0,
            last_maintenance: None,
            promotions: 0,
            demotions: 0,
            cleanups: 0,
            tier_transitions: 0,
            last_tier_check: None,
            entries_cleaned: 0,
            last_cleanup: None,
        }
    }
}
