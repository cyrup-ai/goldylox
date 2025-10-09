#![allow(dead_code)]
// Worker System - Complete worker types library with maintenance workers, canonical task integration, statistics tracking, atomic performance metrics, and comprehensive background cache maintenance coordination

//! Core types and data structures for cache maintenance worker
//!
//! This module defines the main data structures used for background cache
//! maintenance, task processing, and worker statistics tracking.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::JoinHandle;

use crossbeam_utils::CachePadded;

use crossbeam::channel::{Receiver, Sender};
use tokio::sync::mpsc;

use crate::cache::traits::types_and_enums::CacheOperationError;

/// Stats update messages sent from worker thread to manager
#[derive(Debug, Clone)]
pub enum StatUpdate {
    /// Task processed successfully
    TaskProcessed,
    /// Task added to queue
    TaskQueued,
    /// Entry promoted to higher tier
    Promotion,
    /// Entry demoted to lower tier
    Demotion,
    /// Cleanup operation completed
    Cleanup,
    /// Tier transition completed
    TierTransition,
    /// Number of entries cleaned
    EntriesCleaned(u64),
    /// Task processing time in nanoseconds
    TaskTime(u64),
    /// Set last maintenance timestamp
    SetLastMaintenance(u64),
    /// Set last tier check timestamp
    SetLastTierCheck(u64),
    /// Set last cleanup timestamp
    SetLastCleanup(u64),
    /// Update uptime in seconds
    UpdateUptime(u64),
}

/// Background maintenance worker for cache system
pub struct CacheMaintenanceWorker {
    /// Worker thread handle
    pub worker_handle: Option<JoinHandle<()>>,
    /// Shutdown signal
    pub shutdown: AtomicBool,
    /// Task channel sender (uses canonical MaintenanceTask) - async tokio channel
    pub task_sender: mpsc::UnboundedSender<MaintenanceTask>,
    /// Task channel receiver (moved to worker thread on start) - async tokio channel
    task_receiver: Option<mpsc::UnboundedReceiver<MaintenanceTask>>,
    /// Stats update sender for worker to send updates
    pub stat_sender: Sender<StatUpdate>,
    /// Stats update receiver for manager to process updates
    pub stat_receiver: Receiver<StatUpdate>,
    /// Worker statistics
    pub stats: WorkerStats,
    /// Cold tier coordinator for compaction operations
    pub cold_tier_coordinator: crate::cache::tier::cold::ColdTierCoordinator,
}

impl CacheMaintenanceWorker {
    /// Create a new maintenance worker
    pub fn new(cold_tier_coordinator: crate::cache::tier::cold::ColdTierCoordinator) -> Self {
        let (task_sender, task_receiver) = mpsc::unbounded_channel();
        let (stat_sender, stat_receiver) = crossbeam::channel::unbounded();
        Self {
            worker_handle: None,
            shutdown: AtomicBool::new(false),
            task_sender,
            task_receiver: Some(task_receiver),
            stat_sender,
            stat_receiver,
            stats: WorkerStats::default(),
            cold_tier_coordinator,
        }
    }

    /// Submit a canonical maintenance task
    pub fn submit_task(&self, task: MaintenanceTask) -> Result<(), CacheOperationError> {
        // Send stats update for task queued
        let _ = self.stat_sender.send(StatUpdate::TaskQueued);
        // Tokio mpsc send returns Result
        self.task_sender
            .send(task)
            .map_err(|_| CacheOperationError::OperationFailed)?;
        Ok(())
    }

    /// Get worker statistics
    pub fn stats(&mut self) -> WorkerStatsSnapshot {
        // Process any pending stat updates first
        self.process_stat_updates();
        self.stats.snapshot()
    }

    /// Process pending stat updates from worker thread
    pub fn process_stat_updates(&mut self) {
        while let Ok(update) = self.stat_receiver.try_recv() {
            match update {
                StatUpdate::TaskProcessed => {
                    self.stats.tasks_processed.fetch_add(1, Ordering::Relaxed);
                    self.stats.tasks_queued.fetch_sub(1, Ordering::Relaxed);
                }
                StatUpdate::TaskQueued => {
                    self.stats.tasks_queued.fetch_add(1, Ordering::Relaxed);
                }
                StatUpdate::Promotion => {
                    self.stats.promotions.fetch_add(1, Ordering::Relaxed);
                }
                StatUpdate::Demotion => {
                    self.stats.demotions.fetch_add(1, Ordering::Relaxed);
                }
                StatUpdate::Cleanup => {
                    self.stats.cleanups.fetch_add(1, Ordering::Relaxed);
                }
                StatUpdate::TierTransition => {
                    self.stats.tier_transitions.fetch_add(1, Ordering::Relaxed);
                }
                StatUpdate::EntriesCleaned(count) => {
                    self.stats
                        .entries_cleaned
                        .fetch_add(count, Ordering::Relaxed);
                }
                StatUpdate::TaskTime(ns) => {
                    self.stats.update_avg_task_time(ns);
                }
                StatUpdate::SetLastMaintenance(ns) => {
                    self.stats.last_maintenance_ns.store(ns, Ordering::Relaxed);
                }
                StatUpdate::SetLastTierCheck(ns) => {
                    self.stats.last_tier_check_ns.store(ns, Ordering::Relaxed);
                }
                StatUpdate::SetLastCleanup(ns) => {
                    self.stats.last_cleanup_ns.store(ns, Ordering::Relaxed);
                }
                StatUpdate::UpdateUptime(seconds) => {
                    self.stats.uptime_seconds.store(seconds, Ordering::Relaxed);
                }
            }
        }
    }

    /// Schedule automatic maintenance using canonical tasks
    pub fn schedule_maintenance(&self) -> Result<(), CacheOperationError> {
        // Submit various canonical maintenance tasks
        self.submit_task(WorkerMaintenanceOps::cleanup_expired_task())?;
        self.submit_task(WorkerMaintenanceOps::update_statistics_task())?;
        Ok(())
    }
}

/// CANONICAL MaintenanceTask is used directly - NO LOCAL DUPLICATES
/// All worker operations now use the canonical maintenance task definition.
// Import canonical MaintenanceTask and supporting types
pub use crate::cache::tier::warm::maintenance::{MaintenanceTask, OptimizationLevel};

/// Worker-specific maintenance operations using canonical tasks
pub struct WorkerMaintenanceOps;

impl WorkerMaintenanceOps {
    /// Create promote operation using canonical cleanup task
    pub fn promote_task() -> MaintenanceTask {
        // Worker promotions are implemented as cleanup + optimization
        MaintenanceTask::CleanupExpired {
            ttl: std::time::Duration::from_secs(300),
            batch_size: 100,
        }
    }

    /// Create demote operation using canonical eviction task
    pub fn demote_task() -> MaintenanceTask {
        MaintenanceTask::PerformEviction {
            target_pressure: 0.7,
            max_evictions: 50,
        }
    }

    /// Create cleanup task
    pub fn cleanup_expired_task() -> MaintenanceTask {
        MaintenanceTask::CleanupExpired {
            ttl: std::time::Duration::from_secs(600),
            batch_size: 200,
        }
    }

    /// Create compact cold tier task
    pub fn compact_cold_tier_task() -> MaintenanceTask {
        MaintenanceTask::CompactStorage {
            compaction_threshold: 0.7,
        }
    }

    /// Create optimize layout task
    pub fn optimize_layout_task() -> MaintenanceTask {
        MaintenanceTask::OptimizeStructure {
            optimization_level: OptimizationLevel::Standard,
        }
    }

    /// Create update statistics task
    pub fn update_statistics_task() -> MaintenanceTask {
        MaintenanceTask::UpdateStatistics {
            include_detailed_analysis: true,
        }
    }
}

/// Worker performance statistics with lock-free atomic fields
#[derive(Debug)]
pub struct WorkerStats {
    /// Total tasks processed
    pub tasks_processed: CachePadded<AtomicU64>,
    /// Tasks currently queued
    pub tasks_queued: CachePadded<AtomicU64>,
    /// Average task processing time
    pub avg_task_time_ns: CachePadded<AtomicU64>,
    /// Worker uptime
    pub uptime_seconds: CachePadded<AtomicU64>,

    /// Total promotions performed
    pub promotions: CachePadded<AtomicU64>,
    /// Total demotions performed
    pub demotions: CachePadded<AtomicU64>,
    /// Total tier transitions performed
    pub tier_transitions: CachePadded<AtomicU64>,

    /// Total cleanup operations
    pub cleanups: CachePadded<AtomicU64>,
    /// Total entries cleaned across all operations
    pub entries_cleaned: CachePadded<AtomicU64>,

    /// Last maintenance run (nanos since epoch)
    pub last_maintenance_ns: AtomicU64,
    /// Last tier transition check (nanos since epoch)
    pub last_tier_check_ns: AtomicU64,
    /// Last cleanup operation timestamp (nanos since epoch)
    pub last_cleanup_ns: AtomicU64,
}

impl WorkerStats {
    /// Atomic snapshot of all statistics
    pub fn snapshot(&self) -> WorkerStatsSnapshot {
        WorkerStatsSnapshot {
            tasks_processed: self.tasks_processed.load(Ordering::Relaxed),
            tasks_queued: self.tasks_queued.load(Ordering::Relaxed),
            avg_task_time_ns: self.avg_task_time_ns.load(Ordering::Relaxed),
            uptime_seconds: self.uptime_seconds.load(Ordering::Relaxed),
            promotions: self.promotions.load(Ordering::Relaxed),
            demotions: self.demotions.load(Ordering::Relaxed),
            tier_transitions: self.tier_transitions.load(Ordering::Relaxed),
            cleanups: self.cleanups.load(Ordering::Relaxed),
            entries_cleaned: self.entries_cleaned.load(Ordering::Relaxed),
            last_maintenance_ns: self.last_maintenance_ns.load(Ordering::Relaxed),
            last_tier_check_ns: self.last_tier_check_ns.load(Ordering::Relaxed),
            last_cleanup_ns: self.last_cleanup_ns.load(Ordering::Relaxed),
        }
    }

    /// Update average task time using atomic CAS loop
    pub fn update_avg_task_time(&self, new_time_ns: u64) {
        let count = self.tasks_processed.load(Ordering::Relaxed);
        if count == 0 {
            self.avg_task_time_ns.store(new_time_ns, Ordering::Relaxed);
            return;
        }

        // Atomic update of running average
        let mut current = self.avg_task_time_ns.load(Ordering::Relaxed);
        loop {
            let new_avg = (current * (count - 1) + new_time_ns) / count;
            match self.avg_task_time_ns.compare_exchange_weak(
                current,
                new_avg,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
    }
}

/// Snapshot for cloning stats without locks
#[derive(Debug, Clone)]
pub struct WorkerStatsSnapshot {
    pub tasks_processed: u64,
    pub tasks_queued: u64,
    pub avg_task_time_ns: u64,
    pub uptime_seconds: u64,
    pub promotions: u64,
    pub demotions: u64,
    pub tier_transitions: u64,
    pub cleanups: u64,
    pub entries_cleaned: u64,
    pub last_maintenance_ns: u64,
    pub last_tier_check_ns: u64,
    pub last_cleanup_ns: u64,
}

impl Default for WorkerStats {
    fn default() -> Self {
        Self {
            tasks_processed: CachePadded::new(AtomicU64::new(0)),
            tasks_queued: CachePadded::new(AtomicU64::new(0)),
            avg_task_time_ns: CachePadded::new(AtomicU64::new(0)),
            uptime_seconds: CachePadded::new(AtomicU64::new(0)),
            promotions: CachePadded::new(AtomicU64::new(0)),
            demotions: CachePadded::new(AtomicU64::new(0)),
            tier_transitions: CachePadded::new(AtomicU64::new(0)),
            cleanups: CachePadded::new(AtomicU64::new(0)),
            entries_cleaned: CachePadded::new(AtomicU64::new(0)),
            last_maintenance_ns: AtomicU64::new(0),
            last_tier_check_ns: AtomicU64::new(0),
            last_cleanup_ns: AtomicU64::new(0),
        }
    }
}
