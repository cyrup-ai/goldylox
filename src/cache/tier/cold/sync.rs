//! Background synchronization operations for cold tier cache
//!
//! This module handles background tasks including file syncing, cleanup,
//! and maintenance operations for the cold tier persistent storage.

use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{unbounded, Receiver, Sender};

use super::storage::ColdTierCache;
use crate::cache::traits::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};

/// Background sync task types
#[derive(Debug, Clone)]
pub enum SyncTask {
    /// Sync file to disk
    SyncToDisk,
    /// Cleanup expired entries
    CleanupExpired { max_age_sec: u64 },
    /// Compact storage file
    CompactStorage,
    /// Validate data integrity
    ValidateIntegrity,
    /// Emergency shutdown
    Shutdown,
}

/// Sync operation result
#[derive(Debug)]
pub struct SyncResult {
    /// Task that was executed
    pub task: SyncTask,
    /// Success status
    pub success: bool,
    /// Execution time in microseconds
    pub execution_time_us: u64,
    /// Optional error message
    pub error_message: Option<String>,
    /// Items processed (if applicable)
    pub items_processed: usize,
}

/// Background sync coordinator
#[derive(Debug)]
pub struct SyncCoordinator {
    /// Task sender
    task_sender: Sender<SyncTask>,
    /// Result receiver
    result_receiver: Receiver<SyncResult>,
    /// Background thread handle
    thread_handle: Option<thread::JoinHandle<()>>,
    /// Running flag
    running: Arc<AtomicBool>,
    /// Statistics
    stats: SyncStats,
}

/// Synchronization statistics
#[derive(Debug, Default)]
pub struct SyncStats {
    /// Total sync tasks executed
    pub total_sync_tasks: AtomicU64,
    /// Total cleanup operations
    pub total_cleanup_operations: AtomicU64,
    /// Total compaction operations
    pub total_compactions: AtomicU64,
    /// Failed operations
    pub failed_operations: AtomicU64,
    /// Items cleaned up
    pub items_cleaned_up: AtomicU64,
}

/// Sync configuration
#[derive(Debug, Clone)]
pub struct SyncConfig {
    /// Automatic sync interval in seconds
    pub auto_sync_interval_sec: u64,
    /// Cleanup interval in seconds
    pub cleanup_interval_sec: u64,
    /// Maximum entry age before cleanup in seconds
    pub max_entry_age_sec: u64,
    /// Enable automatic background operations
    pub enable_auto_operations: bool,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            auto_sync_interval_sec: 300, // 5 minutes
            cleanup_interval_sec: 3600,  // 1 hour
            max_entry_age_sec: 604800,   // 1 week
            enable_auto_operations: true,
        }
    }
}

impl SyncCoordinator {
    /// Create new sync coordinator
    pub fn new<K: CacheKey, V: CacheValue>(
        cache: Arc<ColdTierCache<K, V>>,
        config: SyncConfig,
    ) -> Result<Self, CacheOperationError> {
        let (task_sender, task_receiver) = unbounded();
        let (result_sender, result_receiver) = unbounded();
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        // Spawn background thread
        let thread_handle = thread::Builder::new()
            .name("cold-tier-sync".to_string())
            .spawn(move || {
                let mut worker =
                    SyncWorker::new(cache, config, task_receiver, result_sender, running_clone);
                worker.run();
            })
            .map_err(|e| {
                CacheOperationError::io_failed(format!("Failed to spawn sync thread: {}", e))
            })?;

        Ok(Self {
            task_sender,
            result_receiver,
            thread_handle: Some(thread_handle),
            running,
            stats: SyncStats::default(),
        })
    }

    /// Submit sync task
    pub fn submit_task(&self, task: SyncTask) -> Result<(), CacheOperationError> {
        self.task_sender
            .try_send(task)
            .map_err(|_| CacheOperationError::Corruption("Sync channel full".to_string()))
    }

    /// Try to receive sync result (non-blocking)
    pub fn try_receive_result(&mut self) -> Option<SyncResult> {
        if let Ok(result) = self.result_receiver.try_recv() {
            self.update_stats(&result);
            Some(result)
        } else {
            None
        }
    }

    /// Update statistics with result
    fn update_stats(&mut self, result: &SyncResult) {
        match &result.task {
            SyncTask::SyncToDisk => {
                self.stats.total_sync_tasks.fetch_add(1, Ordering::Relaxed);
            }
            SyncTask::CleanupExpired { .. } => {
                self.stats
                    .total_cleanup_operations
                    .fetch_add(1, Ordering::Relaxed);
                self.stats
                    .items_cleaned_up
                    .fetch_add(result.items_processed as u64, Ordering::Relaxed);
            }
            SyncTask::CompactStorage => {
                self.stats.total_compactions.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }

        if !result.success {
            self.stats.failed_operations.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Get sync statistics snapshot
    pub fn get_stats(&self) -> SyncStatsSnapshot {
        SyncStatsSnapshot {
            total_sync_tasks: self.stats.total_sync_tasks.load(Ordering::Relaxed),
            total_cleanup_operations: self.stats.total_cleanup_operations.load(Ordering::Relaxed),
            total_compactions: self.stats.total_compactions.load(Ordering::Relaxed),
            failed_operations: self.stats.failed_operations.load(Ordering::Relaxed),
            items_cleaned_up: self.stats.items_cleaned_up.load(Ordering::Relaxed),
        }
    }

    /// Shutdown sync coordinator
    pub fn shutdown(&mut self) -> Result<(), CacheOperationError> {
        // Signal shutdown
        self.running.store(false, Ordering::Relaxed);

        // Send shutdown task
        let _ = self.task_sender.try_send(SyncTask::Shutdown);

        // Wait for thread to complete
        if let Some(handle) = self.thread_handle.take() {
            handle.join().map_err(|_| {
                CacheOperationError::Corruption("Failed to join sync thread".to_string())
            })?;
        }

        Ok(())
    }
}

/// Sync statistics snapshot
#[derive(Debug, Clone)]
pub struct SyncStatsSnapshot {
    pub total_sync_tasks: u64,
    pub total_cleanup_operations: u64,
    pub total_compactions: u64,
    pub failed_operations: u64,
    pub items_cleaned_up: u64,
}

/// Background sync worker
struct SyncWorker<K: CacheKey, V: CacheValue> {
    cache: Arc<ColdTierCache<K, V>>,
    config: SyncConfig,
    task_receiver: Receiver<SyncTask>,
    result_sender: Sender<SyncResult>,
    running: Arc<AtomicBool>,
    last_auto_sync: Instant,
    _phantom: PhantomData<V>,
}

impl<K: CacheKey, V: CacheValue> SyncWorker<K, V> {
    fn new(
        cache: Arc<ColdTierCache<K, V>>,
        config: SyncConfig,
        task_receiver: Receiver<SyncTask>,
        result_sender: Sender<SyncResult>,
        running: Arc<AtomicBool>,
    ) -> Self {
        Self {
            cache,
            config,
            task_receiver,
            result_sender,
            running,
            last_auto_sync: Instant::now(),
            _phantom: PhantomData,
        }
    }

    fn run(&mut self) {
        while self.running.load(Ordering::Relaxed) {
            // Check for incoming tasks
            if let Ok(task) = self.task_receiver.recv_timeout(Duration::from_millis(100)) {
                let result = self.execute_task(task);
                let _ = self.result_sender.try_send(result);
            }
        }
    }

    fn execute_task(&self, task: SyncTask) -> SyncResult {
        let start_time = Instant::now();

        let (success, error_message, items_processed) = match &task {
            SyncTask::SyncToDisk => match self.cache.as_ref().sync_to_disk() {
                Ok(_) => (true, None, 0),
                Err(e) => (false, Some(e.to_string()), 0),
            },
            SyncTask::CleanupExpired { max_age_sec: _ } => {
                match self.cache.as_ref().cleanup_expired() {
                    Ok(count) => (true, None, count),
                    Err(e) => (false, Some(e.to_string()), 0),
                }
            }
            SyncTask::CompactStorage => match self.cache.compact() {
                Ok(_) => (true, None, 0),
                Err(e) => (false, Some(e.to_string()), 0),
            },
            SyncTask::ValidateIntegrity => match self.cache.validate_integrity() {
                Ok(_) => (true, None, 0),
                Err(e) => (false, Some(e.to_string()), 0),
            },
            SyncTask::Shutdown => {
                self.running.store(false, Ordering::Relaxed);
                (true, None, 0)
            }
        };

        SyncResult {
            task,
            success,
            execution_time_us: start_time.elapsed().as_micros() as u64,
            error_message,
            items_processed,
        }
    }

    fn check_auto_operations(&mut self) {
        let now = Instant::now();

        // Auto sync
        if now.duration_since(self.last_auto_sync).as_secs() >= self.config.auto_sync_interval_sec {
            let _ = self
                .result_sender
                .try_send(self.execute_task(SyncTask::SyncToDisk));
            self.last_auto_sync = now;
        }
    }
}

impl Drop for SyncCoordinator {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
