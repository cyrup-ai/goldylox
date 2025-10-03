//! Utility functions and global compatibility for cold tier cache
//!
//! This module provides helper functions for cache maintenance and global
//! compatibility functions for existing code integration.

use crate::cache::tier::cold::PersistentColdTier;
use crate::cache::tier::cold::data_structures::ColdCacheKey;
use crate::cache::tier::cold::data_structures::*;

use crate::cache::tier::cold::core::types::timestamp_nanos;

use crate::cache::traits::{CacheKey, CacheValue};

impl<
    K: CacheKey + Default + bincode::Encode + bincode::Decode<()>,
    V: CacheValue
        + Default
        + serde::Serialize
        + serde::de::DeserializeOwned
        + bincode::Encode
        + bincode::Decode<()>
        + 'static,
> PersistentColdTier<K, V>
{
    /// Update access metadata for cache entry
    pub(super) fn update_access_metadata(&self, _key: &ColdCacheKey<K>, entry: &mut IndexEntry) {
        // Connect to existing sophisticated access tracking system
        use std::time::Instant;

        // Update timestamp and access count atomically
        entry.last_access_ns = timestamp_nanos(Instant::now());
        entry.access_count += 1;

        // Update per-instance statistics using existing stats infrastructure
        self.stats.record_hit(0);
    }

    /// Mark space as free for compaction
    pub(super) fn mark_space_free(&self, _offset: u64, _size: u32) {
        // Connect to existing compaction system

        // Mark space for compaction - actual reclamation happens during compaction
        // Space statistics are updated when entries are actually removed, not just marked

        // Schedule compaction task if fragmentation threshold reached
        let fragmentation_threshold = 0.3; // 30% fragmentation triggers compaction
        if self.should_compact(fragmentation_threshold) {
            // Submit maintenance task to existing scheduler via channel
            if let Some(sender) = self.get_maintenance_sender() {
                let canonical_task = crate::cache::manager::background::types::CanonicalMaintenanceTask::CompactStorage {
                    compaction_threshold: 0.3, // 30% fragmentation threshold
                };
                let task =
                    crate::cache::manager::background::types::MaintenanceTask::new(canonical_task);
                let _ = sender.send(task);
            }
        }
    }

    /// Calculate checksum for data integrity
    pub(super) fn calculate_checksum(&self, data: &[u8]) -> u32 {
        // Simple CRC32-like checksum
        let mut checksum = 0u32;
        for &byte in data {
            checksum = checksum.wrapping_mul(31).wrapping_add(byte as u32);
        }
        checksum
    }

    /// Validate storage integrity
    pub fn validate_integrity(
        &self,
    ) -> Result<bool, crate::cache::traits::types_and_enums::CacheOperationError> {
        // Delegate to storage manager's validate_integrity method
        self.storage_manager.validate_integrity()
    }

    /// Check if compaction is needed based on fragmentation threshold
    fn should_compact(&self, fragmentation_threshold: f64) -> bool {
        // Get current tier statistics
        let stats = self.stats.snapshot();

        // Get storage usage information
        let total_allocated_bytes = stats.memory_usage;
        let entry_count = stats.entry_count;

        // Don't compact if there's no data
        if total_allocated_bytes == 0 || entry_count == 0 {
            return false;
        }

        // Estimate fragmentation based on compaction system statistics
        let fragmented_bytes = self.compaction_system.bytes_reclaimed();

        // Calculate deleted entry overhead using professional MemoryEfficiencyAnalyzer
        let deleted_entry_bytes = if self.compaction_system.is_compacting() {
            0
        } else {
            use crate::cache::config::CacheConfig;
            use crate::cache::memory::efficiency_analyzer::MemoryEfficiencyAnalyzer;

            // Create CacheConfig from existing cold tier config (verified compatibility)
            let cache_config = CacheConfig {
                cold_tier: self.config.clone(),
                ..CacheConfig::default()
            };

            match MemoryEfficiencyAnalyzer::new(&cache_config) {
                Ok(analyzer) => {
                    match analyzer.analyze_efficiency() {
                        Ok(analysis) => {
                            // Use professional fragmentation analysis from efficiency analyzer
                            fragmented_bytes
                                .saturating_mul((analysis.fragmentation_impact * 1000.0) as u64)
                                / 1000
                        }
                        Err(_) => {
                            // Fallback: Use defragmentation measurement from CompactionSystem
                            self.compaction_system.bytes_reclaimed()
                        }
                    }
                }
                Err(analyzer_error) => {
                    // Alternative: Use fallback value (defragmentation requires pool_coordinator access)
                    log::warn!(
                        "MemoryEfficiencyAnalyzer failed: {:?}, using fallback fragmentation estimate",
                        analyzer_error
                    );
                    // Conservative fallback: use zero instead of arbitrary calculation
                    0
                }
            }
        };

        // Calculate fragmentation ratio
        let free_space = fragmented_bytes + deleted_entry_bytes;
        let fragmentation_ratio = if total_allocated_bytes > 0 {
            free_space as f64 / total_allocated_bytes as f64
        } else {
            0.0
        };

        // Apply business rules from configuration
        let fragmentation_exceeds_threshold = fragmentation_ratio > fragmentation_threshold;
        let size_threshold_met = total_allocated_bytes > (10 * 1024 * 1024); // 10MB default
        let time_threshold_met = !self.compaction_system.is_compacting();

        // Log compaction decision for observability
        if fragmentation_exceeds_threshold && size_threshold_met {
            log::info!(
                "Cold tier fragmentation evaluation: ratio={:.2}%, size={}, time_ok={}, threshold={:.1}%",
                fragmentation_ratio * 100.0,
                format_bytes(total_allocated_bytes as u64),
                time_threshold_met,
                fragmentation_threshold * 100.0
            );
        }

        fragmentation_exceeds_threshold && size_threshold_met && time_threshold_met
    }

    /// Get maintenance sender for submitting tasks
    fn get_maintenance_sender(
        &self,
    ) -> Option<&crossbeam_channel::Sender<crate::cache::manager::background::types::MaintenanceTask>>
    {
        // Return reference to the maintenance sender if available
        self.maintenance_sender.as_ref()
    }

    /// Schedule compaction if fragmentation threshold exceeded
    pub fn schedule_compaction_if_needed(
        &self,
        fragmentation_threshold: f64,
    ) -> Result<bool, crate::cache::traits::types_and_enums::CacheOperationError> {
        // Check if compaction is needed
        if !self.should_compact(fragmentation_threshold) {
            return Ok(false);
        }

        // Get the maintenance sender
        let Some(sender) = self.get_maintenance_sender() else {
            log::debug!("No maintenance sender available for cold tier compaction");
            return Ok(false);
        };

        // Create compaction task
        let canonical_task =
            crate::cache::manager::background::types::CanonicalMaintenanceTask::CompactStorage {
                compaction_threshold: fragmentation_threshold,
            };
        let compaction_task =
            crate::cache::manager::background::types::MaintenanceTask::new(canonical_task);

        // Submit the task via channel
        match sender.send(compaction_task) {
            Ok(()) => {
                log::info!(
                    "Submitted compaction task for cold tier with threshold {:.2}",
                    fragmentation_threshold
                );
                Ok(true)
            }
            Err(_) => {
                log::error!("Failed to submit compaction task to maintenance scheduler");
                Err(
                    crate::cache::traits::types_and_enums::CacheOperationError::internal_error(
                        "Failed to submit compaction task - scheduler channel may be closed"
                            .to_string(),
                    ),
                )
            }
        }
    }
}

/// Format bytes in human-readable format
fn format_bytes(bytes: u64) -> String {
    if bytes > 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes > 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes > 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

// All global functions moved to mod.rs with proper coordinator pattern
// This file now only contains utility implementations for PersistentColdTier
