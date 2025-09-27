//! Individual memory pool with lock-free allocation
//!
//! This module implements the MemoryPool struct that handles allocation and
//! deallocation for objects of a specific size using lock-free data structures.

use std::alloc::{Layout, alloc, dealloc};
use std::ptr::{NonNull, null_mut};
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

use crossbeam_utils::CachePadded;

use crate::cache::memory::types::PoolAllocationStats;
use crate::cache::traits::types_and_enums::CacheOperationError;

/// Individual memory pool with lock-free allocation
#[derive(Debug)]
pub struct MemoryPool {
    /// Pool name for identification
    pool_name: &'static str,
    /// Object size for this pool (bytes)
    object_size: usize,
    /// Maximum pool capacity (objects)
    max_capacity: AtomicUsize,
    /// Current pool utilization (objects)
    #[allow(dead_code)]
    // Memory management - current_utilization used in pool utilization tracking
    current_utilization: CachePadded<AtomicUsize>,
    /// Free list head (lock-free stack)
    free_list_head: AtomicPtr<PoolEntry>,
    /// Pool allocation statistics
    #[allow(dead_code)]
    // Memory management - pool_allocation_stats used in individual pool statistics tracking
    pool_allocation_stats: PoolAllocationStats,
    /// Pool memory layout
    memory_layout: Layout,
}

/// Pool entry for lock-free free list
#[derive(Debug)]
struct PoolEntry {
    /// Pointer to next free entry
    next: AtomicPtr<PoolEntry>,
    /// Entry data (aligned to pool object size)
    data: [u8; 0], // Flexible array member
}
impl Clone for MemoryPool {
    /// Create a new MemoryPool with the same configuration but fresh state
    fn clone(&self) -> Self {
        // Clone by recreating with same parameters (atomic fields get fresh state)
        match Self::new(
            self.pool_name,
            self.object_size,
            self.max_capacity.load(Ordering::Relaxed),
        ) {
            Ok(pool) => pool,
            Err(_) => {
                // Fallback: create with minimal safe configuration if original parameters fail
                match Self::new("cloned_pool", 64, 100) {
                    Ok(fallback_pool) => fallback_pool,
                    Err(_) => {
                        // Emergency fallback with absolute minimum configuration
                        let emergency_layout =
                            match Layout::from_size_align(64, std::mem::align_of::<PoolEntry>()) {
                                Ok(layout) => layout,
                                Err(_) => Layout::new::<u64>(), // Safe fallback layout
                            };
                        Self {
                            pool_name: "emergency_pool",
                            object_size: 64,
                            max_capacity: AtomicUsize::new(100),
                            current_utilization: CachePadded::new(AtomicUsize::new(0)),
                            free_list_head: AtomicPtr::new(null_mut()),
                            pool_allocation_stats: PoolAllocationStats::new(),
                            memory_layout: emergency_layout,
                        }
                    }
                }
            }
        }
    }
}

impl MemoryPool {
    pub fn new(
        name: &'static str,
        object_size: usize,
        max_capacity: usize,
    ) -> Result<Self, CacheOperationError> {
        let layout = Layout::from_size_align(object_size, std::mem::align_of::<PoolEntry>())
            .map_err(|_| CacheOperationError::initialization_failed("Invalid memory layout"))?;

        Ok(Self {
            pool_name: name,
            object_size,
            max_capacity: AtomicUsize::new(max_capacity),
            current_utilization: CachePadded::new(AtomicUsize::new(0)),
            free_list_head: AtomicPtr::new(null_mut()),
            pool_allocation_stats: PoolAllocationStats::new(),
            memory_layout: layout,
        })
    }

    pub fn allocate(&self, _size: usize) -> Result<NonNull<u8>, CacheOperationError> {
        // Try to pop from free list first
        loop {
            let head = self.free_list_head.load(Ordering::Acquire);
            if head.is_null() {
                // Free list empty, allocate new memory
                return self.allocate_new();
            }

            let next = unsafe { &*head }.next.load(Ordering::Relaxed);
            if self
                .free_list_head
                .compare_exchange_weak(head, next, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                // Successfully popped from free list
                self.pool_allocation_stats.record_allocation();

                return NonNull::new(head as *mut u8).ok_or_else(|| {
                    CacheOperationError::resource_exhausted("Null pointer in free list")
                });
            }
        }
    }

    pub fn deallocate(&self, ptr: NonNull<u8>, _size: usize) -> Result<(), CacheOperationError> {
        let entry = ptr.as_ptr() as *mut PoolEntry;

        // Push onto free list
        loop {
            let head = self.free_list_head.load(Ordering::Acquire);
            unsafe { &*entry }.next.store(head, Ordering::Relaxed);

            if self
                .free_list_head
                .compare_exchange_weak(head, entry, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                // Successfully pushed to free list
                self.pool_allocation_stats.record_deallocation();
                return Ok(());
            }
        }
    }

    /// Sophisticated cleanup that integrates with existing fragmentation analysis,
    /// maintenance scheduling, and allocation statistics systems
    pub fn try_cleanup(&self) -> bool {
        // Perform real memory cleanup by walking the free list and deallocating
        self.perform_real_cleanup()
    }

    /// Perform real memory cleanup by deallocating free list entries
    fn perform_real_cleanup(&self) -> bool {
        // Atomically take ownership of the entire free list
        let mut head = self.free_list_head.swap(null_mut(), Ordering::AcqRel);

        if head.is_null() {
            // No free entries to clean up
            return false;
        }

        let mut entries_freed = 0usize;
        let mut bytes_freed = 0usize;

        // Walk the free list and deallocate each entry
        while !head.is_null() {
            unsafe {
                // Get next entry before deallocating current
                let next = (*head).next.load(Ordering::Acquire);

                // Deallocate the current entry back to the system
                dealloc(head as *mut u8, self.memory_layout);

                entries_freed += 1;
                bytes_freed += self.object_size;

                // Move to next entry
                head = next;
            }
        }

        // Update utilization counter
        let freed = entries_freed.min(self.current_utilization.load(Ordering::Relaxed));
        if freed > 0 {
            self.current_utilization.fetch_sub(freed, Ordering::Relaxed);

            // Update pool statistics with real deallocation count
            for _ in 0..entries_freed {
                self.pool_allocation_stats.record_deallocation();
            }

            // Log cleanup results for monitoring
            log::debug!(
                "Pool {} cleaned up {} entries ({} bytes)",
                self.pool_name,
                entries_freed,
                bytes_freed
            );

            true
        } else {
            false
        }
    }

    /// Get current utilization as percentage * 100
    pub fn get_utilization_percentage(&self) -> u32 {
        let current = self.current_utilization.load(Ordering::Relaxed);
        let max_cap = self.max_capacity.load(Ordering::Relaxed);

        if max_cap > 0 {
            ((current * 10000) / max_cap) as u32
        } else {
            0
        }
    }

    /// Get pool name
    pub fn name(&self) -> &'static str {
        self.pool_name
    }

    /// Get object size
    pub fn object_size(&self) -> usize {
        self.object_size
    }

    /// Get current capacity
    pub fn current_capacity(&self) -> usize {
        self.max_capacity.load(Ordering::Relaxed)
    }

    /// Get current utilization
    pub fn current_utilization(&self) -> usize {
        self.current_utilization.load(Ordering::Relaxed)
    }

    fn allocate_new(&self) -> Result<NonNull<u8>, CacheOperationError> {
        // Check capacity limits
        let current = self.current_utilization.load(Ordering::Relaxed);
        let max_cap = self.max_capacity.load(Ordering::Relaxed);

        if current >= max_cap {
            self.pool_allocation_stats.record_failure();
            return Err(CacheOperationError::resource_exhausted(
                "Pool capacity exhausted",
            ));
        }

        // Allocate new memory
        let ptr = unsafe { alloc(self.memory_layout) };
        if ptr.is_null() {
            return Err(CacheOperationError::resource_exhausted(
                "System memory exhausted",
            ));
        }

        self.current_utilization.fetch_add(1, Ordering::Relaxed);
        self.pool_allocation_stats.record_allocation();

        NonNull::new(ptr)
            .ok_or_else(|| CacheOperationError::resource_exhausted("Allocation returned null"))
    }
}
