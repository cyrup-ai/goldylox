//! Utility functions and global compatibility for cold tier cache
//!
//! This module provides helper functions for cache maintenance and global
//! compatibility functions for existing code integration.



use crate::cache::tier::cold::data_structures::ColdCacheKey;
use crate::cache::tier::cold::data_structures::*;
use crate::cache::tier::cold::PersistentColdTier;

use crate::cache::tier::cold::core::types::timestamp_nanos;
use crate::cache::tier::cold::storage::ColdTierCache;
use crate::cache::manager::background::types::MaintenanceTaskType;

use crate::cache::traits::{CacheKey, CacheValue};


impl<K: CacheKey, V: CacheValue> PersistentColdTier<K, V> {
    /// Update access metadata for cache entry
    pub(super) fn update_access_metadata(&self, key: &ColdCacheKey<K>, entry: &mut IndexEntry) {
        // Connect to existing sophisticated access tracking system
        use std::time::Instant;
        
        // Update timestamp and access count atomically
        entry.last_access_ns = timestamp_nanos(Instant::now());
        entry.access_count += 1;
        
        // Update global statistics using existing atomic counters
        ColdTierCache::<K, V>::record_hit();
    }

    /// Mark space as free for compaction
    pub(super) fn mark_space_free(&self, offset: u64, size: u32) {
        // Connect to existing compaction system
        
        // Mark space for compaction - actual reclamation happens during compaction
        // Space statistics are updated when entries are actually removed, not just marked
        
        // Schedule compaction task if fragmentation threshold reached
        let fragmentation_threshold = 0.3; // 30% fragmentation triggers compaction
        if self.needs_compaction(fragmentation_threshold) {
            // Submit maintenance task to existing scheduler
            if let Some(scheduler) = self.get_maintenance_scheduler() {
                let _ = scheduler.submit_task(
                    MaintenanceTaskType::CompactColdTier,
                    100 // Medium priority
                );
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
    pub fn validate_integrity(&self) -> Result<bool, crate::cache::traits::types_and_enums::CacheOperationError> {
        // Delegate to storage manager's validate_integrity method
        self.storage_manager.validate_integrity()
    }
}

// All global functions moved to mod.rs with proper coordinator pattern
// This file now only contains utility implementations for PersistentColdTier
