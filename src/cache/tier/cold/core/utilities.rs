//! Utility functions and global compatibility for cold tier cache
//!
//! This module provides helper functions for cache maintenance and global
//! compatibility functions for existing code integration.

use std::any::TypeId;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use super::super::data_structures::ColdCacheKey;
use super::super::data_structures::*;
use super::types::ColdTierStats;
use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};

// Registry for PersistentColdTier instances by type combination
static COLD_TIER_REGISTRY: OnceLock<
    HashMap<(TypeId, TypeId), Box<dyn std::any::Any + Send + Sync>>,
> = OnceLock::new();

impl<K: CacheKey, V: CacheValue> super::super::PersistentColdTier<K, V> {
    /// Update access metadata for cache entry
    pub(super) fn update_access_metadata(&self, key: &ColdCacheKey<K>, entry: &mut IndexEntry) {
        // Connect to existing sophisticated access tracking system
        use std::time::Instant;
        
        // Update timestamp and access count atomically
        entry.last_access_ns = crate::cache::tier::warm::timing::timestamp_nanos(Instant::now());
        entry.access_count += 1;
        
        // Update global statistics using existing atomic counters
        use std::sync::atomic::Ordering;
        super::super::storage::COLD_READS.fetch_add(1, Ordering::Relaxed);
    }

    /// Mark space as free for compaction
    pub(super) fn mark_space_free(&self, offset: u64, size: u32) {
        // Connect to existing compaction system
        use std::sync::atomic::Ordering;
        
        // Update space tracking for compaction coordination
        super::super::storage::COLD_STORAGE_BYTES.fetch_sub(size as u64, Ordering::Relaxed);
        
        // Schedule compaction task if fragmentation threshold reached
        let fragmentation_threshold = 0.3; // 30% fragmentation triggers compaction
        if self.needs_compaction(fragmentation_threshold) {
            // Submit maintenance task to existing scheduler
            if let Some(scheduler) = self.get_maintenance_scheduler() {
                let _ = scheduler.submit_task(
                    super::super::super::manager::background::types::MaintenanceTaskType::CompactColdTier,
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
}

// All global functions moved to mod.rs with proper coordinator pattern
// This file now only contains utility implementations for PersistentColdTier
