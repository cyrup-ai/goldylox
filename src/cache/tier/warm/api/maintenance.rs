//! Background maintenance operations for warm tier cache
//!
//! This module handles all background maintenance tasks using the CANONICAL
//! MaintenanceTask definition - NO MORE DUPLICATES!

use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};

use crate::cache::traits::AccessType;
use super::access_tracking::{AccessContext, ConcurrentAccessTracker};
use super::core::{LockFreeWarmTier, WarmCacheKey};
use super::eviction::{ConcurrentEvictionPolicy, EvictionPolicyType};
use super::monitoring::{MemoryAlert, MemoryPressureMonitor};
use crate::cache::types::statistics::atomic_stats::AtomicTierStats;
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::CacheKey;

// Use ONLY the canonical MaintenanceTask - no duplicates!
pub use crate::cache::tier::warm::maintenance::{
    MaintenanceTask,
    OptimizationLevel,
    AnalysisDepth,
    SyncDirection,
    ConsistencyLevel,
    ValidationLevel,
    ModelComplexity,
    TaskPriority,
};

impl LockFreeWarmTier {
    /// Check for expired entries and clean them up
    pub fn cleanup_expired(&self, ttl: Duration) -> Result<usize, CacheOperationError> {
        let mut cleaned_count = 0;
        let mut to_remove = Vec::new();

        // Collect expired keys
        for entry in self.storage.iter() {
            let key = entry.key();
            let warm_entry = entry.value();
            if warm_entry.last_access() > ttl {
                to_remove.push(key.clone());
            }
        }

        // Remove expired entries
        for key in to_remove {
            if let Some(entry) = self.storage.remove(&key) {
                let entry_size = entry.value().estimated_size() as u64;

                // Update memory usage
                let current_usage = self.memory_monitor.get_current_usage();
                let _ = self
                    .memory_monitor
                    .update_memory_usage(current_usage - entry_size);

                // Update statistics
                self.stats.update_memory_usage(-(entry_size as i64));
                self.stats.update_entry_count(-1);

                cleaned_count += 1;
            }
        }

        Ok(cleaned_count)
    }

    /// Trigger eviction to reach target memory pressure
    pub fn trigger_eviction(&self, target_pressure: f64) -> Result<(), CacheOperationError> {
        let current_usage = self.memory_monitor.get_current_usage();
        let memory_limit = self.memory_monitor.memory_limit;
        let target_usage = (memory_limit as f64 * target_pressure) as u64;

        if current_usage <= target_usage {
            return Ok(()); // Already below target
        }

        let bytes_to_evict = current_usage - target_usage;
        let mut bytes_evicted = 0u64;
        let mut eviction_candidates = Vec::new();

        // Collect eviction candidates
        for entry in self.storage.iter() {
            eviction_candidates.push(entry.key().clone());
            if eviction_candidates.len() >= 100 {
                break; // Limit candidate set size
            }
        }

        // Select victims using eviction policy
        while bytes_evicted < bytes_to_evict && !eviction_candidates.is_empty() {
            let cache_pressure = self.memory_monitor.get_pressure_level();

            if let Some(victim) = self
                .eviction_policy
                .select_eviction_candidate(&eviction_candidates, cache_pressure)
            {
                // Remove victim from candidates
                eviction_candidates.retain(|k| k != &victim);

                // Evict the victim
                if let Some(entry) = self.storage.remove(&victim) {
                    let evicted_size = entry.value().estimated_size() as u64;
                    bytes_evicted += evicted_size;

                    // Update statistics
                    self.stats.update_memory_usage(-(evicted_size as i64));
                    self.stats.update_entry_count(-1);

                    // Record successful eviction
                    self.eviction_policy.record_eviction_outcome(
                        &victim,
                        EvictionPolicyType::Adaptive,
                        true,
                        None,
                    );
                }
            } else {
                break; // No more candidates
            }
        }

        // Update memory usage
        let new_usage = current_usage - bytes_evicted;
        self.memory_monitor.update_memory_usage(new_usage)?;

        Ok(())
    }

    /// Process background maintenance tasks
    pub fn process_maintenance_tasks(&self) -> Result<usize, CacheOperationError> {
        let mut tasks_processed = 0;

        // Process up to 10 tasks per call to avoid blocking
        while tasks_processed < 10 {
            match self.maintenance_rx.try_recv() {
                Ok(task) => {
                    self.handle_maintenance_task(task)?;
                    tasks_processed += 1;
                }
                Err(_) => break, // No more tasks
            }
        }

        Ok(tasks_processed)
    }

    /// Handle canonical maintenance task directly - no more adapters!
    pub fn handle_maintenance_task(&self, task: MaintenanceTask) -> Result<(), CacheOperationError> {
        match task {
            MaintenanceTask::CleanupExpired { ttl, .. } => {
                self.cleanup_expired(ttl)?;
            }
            MaintenanceTask::PerformEviction { target_pressure, .. } => {
                self.trigger_eviction(target_pressure)?;
            }
            MaintenanceTask::Evict { target_count, .. } => {
                // Convert target count to pressure - rough estimate
                let target_pressure = 1.0 - (target_count as f64 / 1000.0);
                self.trigger_eviction(target_pressure.max(0.1))?;
            }
            MaintenanceTask::UpdateStatistics { .. } => {
                // Statistics are updated automatically, but we could do deeper analysis here
            }
            MaintenanceTask::OptimizeStructure { .. } => {
                // Could implement skiplist optimization here
            }
            MaintenanceTask::CompactStorage { .. } => {
                // Could implement storage compaction here
            }
            MaintenanceTask::AnalyzePatterns { .. } => {
                // Trigger pattern analysis
                let _ = self
                    .access_tracker
                    .cleanup_old_records(Duration::from_secs(300));
            }
            MaintenanceTask::SyncTiers { .. } => {
                // Could implement tier synchronization here
            }
            MaintenanceTask::ValidateIntegrity { .. } => {
                // Could implement integrity validation here
            }
            MaintenanceTask::DefragmentMemory { .. } => {
                // Could implement memory defragmentation here
            }
            MaintenanceTask::UpdateMLModels { .. } => {
                // Could implement ML model updates here
            }
        }

        Ok(())
    }

    /// Check for memory alerts
    pub fn check_memory_alerts(&self) -> Vec<MemoryAlert> {
        let mut alerts = Vec::new();

        // Check for standard pressure alerts
        while let Some(alert) = self.memory_monitor.try_receive_alert() {
            alerts.push(alert);
        }

        // Check for memory leak detection
        if let Some(leak_alert) = self.memory_monitor.detect_memory_leaks() {
            alerts.push(leak_alert);
        }

        alerts
    }
}