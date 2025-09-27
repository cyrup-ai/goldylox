//! SIMD-aligned memory pool implementation for hot tier cache
//!
//! This module provides the main MemoryPool struct with lock-free allocation,
//! slot management, and memory tracking capabilities.

use super::statistics::MemoryPoolStats;
use super::types::{CacheSlot, SlotMetadata};
use crate::cache::config::types::HotTierConfig;
use crate::cache::traits::{CacheKey, CacheValue};

/// SIMD-aligned memory pool for cache entries
#[derive(Debug)]
#[repr(align(64))]
pub struct MemoryPool<K: CacheKey, V: CacheValue> {
    /// Cache entries (SIMD-aligned for vectorized operations)
    pub entries: Vec<CacheSlot<K, V>>,
    /// Metadata array for SIMD parallel searches
    pub metadata: Vec<SlotMetadata>,
    /// Free slot stack for efficient allocation
    free_slots: Vec<usize>,
    /// Cache mask for fast modulo (capacity - 1)
    pub capacity_mask: usize,
    /// Memory usage tracking
    total_memory_usage: usize,
}

impl<K: CacheKey + Default, V: CacheValue> MemoryPool<K, V> {
    /// Create new SIMD-aligned memory pool
    pub fn new(config: &HotTierConfig) -> Self {
        // Ensure capacity is power of 2 for fast modulo
        let capacity = config.max_entries.next_power_of_two() as usize;

        // Initialize dynamic SIMD-aligned storage
        let mut entries = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            entries.push(CacheSlot::<K, V>::empty());
        }

        let mut metadata = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            metadata.push(SlotMetadata::empty());
        }

        // Initialize free slots stack
        let free_slots: Vec<usize> = (0..capacity).collect();

        Self {
            entries,
            metadata,
            free_slots,
            capacity_mask: capacity - 1,
            total_memory_usage: 0,
        }
    }

    /// Allocate a cache slot (lock-free atomic operation)
    pub fn allocate_slot(&mut self) -> Option<usize> {
        self.free_slots.pop()
    }

    /// Deallocate a cache slot (return to free pool)
    pub fn deallocate_slot(&mut self, slot_idx: usize) {
        if slot_idx <= self.capacity_mask {
            // Clear the slot
            self.entries[slot_idx].clear();
            self.metadata[slot_idx].mark_empty();

            // Return to free pool
            self.free_slots.push(slot_idx);
        }
    }

    /// Get slot at index
    #[inline(always)]
    pub fn get_slot(&self, index: usize) -> Option<&CacheSlot<K, V>> {
        if index <= self.capacity_mask {
            Some(&self.entries[index])
        } else {
            None
        }
    }

    /// Get mutable slot at index
    #[inline(always)]
    pub fn get_slot_mut(&mut self, index: usize) -> Option<&mut CacheSlot<K, V>> {
        if index <= self.capacity_mask {
            Some(&mut self.entries[index])
        } else {
            None
        }
    }

    /// Get metadata at index
    #[inline(always)]
    pub fn get_metadata(&self, index: usize) -> Option<&SlotMetadata> {
        if index <= self.capacity_mask {
            Some(&self.metadata[index])
        } else {
            None
        }
    }

    /// Get mutable metadata at index
    #[inline(always)]
    pub fn get_metadata_mut(&mut self, index: usize) -> Option<&mut SlotMetadata> {
        if index <= self.capacity_mask {
            Some(&mut self.metadata[index])
        } else {
            None
        }
    }

    /// Insert entry at specific slot
    pub fn insert_at_slot(
        &mut self,
        slot_idx: usize,
        key: K,
        value: V,
        key_hash: u64,
        timestamp_ns: u64,
    ) {
        if slot_idx <= self.capacity_mask {
            let memory_size = value.estimated_size();

            self.entries[slot_idx] = CacheSlot::<K, V> {
                key_hash,
                key,
                value: Some(value),
                last_access_ns: timestamp_ns,
            };

            self.metadata[slot_idx] = SlotMetadata {
                state: 1, // Occupied
                access_count: 1,
                priority: 5, // Default priority
                _reserved: [0; 1],
                generation: 1,
                size_bytes: memory_size as u32,
                _padding: [0; 4],
            };

            self.total_memory_usage += memory_size;
        }
    }

    /// Update existing entry at slot
    pub fn update_at_slot(&mut self, slot_idx: usize, value: V, timestamp_ns: u64) {
        if slot_idx <= self.capacity_mask {
            let old_size = self.metadata[slot_idx].size_bytes as usize;
            let new_size = value.estimated_size();

            self.entries[slot_idx].value = Some(value);
            self.entries[slot_idx].last_access_ns = timestamp_ns;

            self.metadata[slot_idx].access_count =
                self.metadata[slot_idx].access_count.wrapping_add(1);
            self.metadata[slot_idx].size_bytes = new_size as u32;
            self.metadata[slot_idx].generation = self.metadata[slot_idx].generation.wrapping_add(1);

            // Update memory usage
            self.total_memory_usage = self
                .total_memory_usage
                .saturating_sub(old_size)
                .saturating_add(new_size);
        }
    }

    /// Clear slot and update memory tracking
    pub fn clear_slot(&mut self, slot_idx: usize) {
        if slot_idx <= self.capacity_mask {
            let old_size = self.metadata[slot_idx].size_bytes as usize;

            self.entries[slot_idx].clear();
            self.metadata[slot_idx].mark_empty();

            self.total_memory_usage = self.total_memory_usage.saturating_sub(old_size);
        }
    }

    /// Get available slot count
    pub fn available_slots(&self) -> usize {
        self.free_slots.len()
    }

    /// Get occupied slot count
    pub fn occupied_slots(&self) -> usize {
        (self.capacity_mask + 1) - self.free_slots.len()
    }

    /// Get total memory usage
    pub fn total_memory_usage(&self) -> usize {
        self.total_memory_usage
    }

    /// Get capacity
    pub fn capacity(&self) -> usize {
        self.capacity_mask + 1
    }

    /// Calculate slot index from hash
    #[inline(always)]
    pub fn slot_index_from_hash(&self, hash: u64) -> usize {
        (hash as usize) & self.capacity_mask
    }

    /// Compact memory pool by removing fragmentation
    pub fn compact(&mut self) -> usize {
        let mut compacted_count = 0;
        let mut write_idx = 0;

        // Compact occupied slots to beginning of array
        for read_idx in 0..=self.capacity_mask {
            if self.metadata[read_idx].is_occupied() {
                if read_idx != write_idx {
                    // Move slot to earlier position
                    self.entries[write_idx] = std::mem::take(&mut self.entries[read_idx]);
                    self.metadata[write_idx] = self.metadata[read_idx];

                    // Clear old position
                    self.entries[read_idx] = CacheSlot::<K, V>::empty();
                    self.metadata[read_idx] = SlotMetadata::empty();

                    compacted_count += 1;
                }
                write_idx += 1;
            }
        }

        // Rebuild free slots list
        self.free_slots.clear();
        for idx in write_idx..=self.capacity_mask {
            self.free_slots.push(idx);
        }

        compacted_count
    }

    /// Get memory pool statistics
    pub fn stats(&self) -> MemoryPoolStats {
        MemoryPoolStats {
            total_slots: self.capacity(),
            occupied_slots: self.occupied_slots(),
            available_slots: self.available_slots(),
            total_memory_usage: self.total_memory_usage,
            average_slot_size: if self.occupied_slots() > 0 {
                self.total_memory_usage / self.occupied_slots()
            } else {
                0
            },
            fragmentation_ratio: {
                let used_slots = self.occupied_slots();
                if used_slots > 0 {
                    1.0 - (used_slots as f64 / self.capacity() as f64)
                } else {
                    0.0
                }
            },
        }
    }
}
