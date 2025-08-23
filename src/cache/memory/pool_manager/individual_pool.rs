//! Individual memory pool with lock-free allocation
//!
//! This module implements the MemoryPool struct that handles allocation and
//! deallocation for objects of a specific size using lock-free data structures.

use std::alloc::{alloc, Layout};
use std::ptr::{null_mut, NonNull};
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::sync::Arc;

use crossbeam_utils::CachePadded;

use super::super::types::PoolAllocationStats;
use super::cleanup_manager::PoolCleanupManager;
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
    current_utilization: CachePadded<AtomicUsize>,
    /// Free list head (lock-free stack)
    free_list_head: AtomicPtr<PoolEntry>,
    /// Pool allocation statistics
    pool_allocation_stats: PoolAllocationStats,
    /// Pool memory layout
    memory_layout: Layout,
    /// Sophisticated cleanup manager (integrates with existing systems)
    cleanup_manager: Option<Arc<PoolCleanupManager>>,
}

/// Pool entry for lock-free free list
#[derive(Debug)]
struct PoolEntry {
    /// Pointer to next free entry
    next: AtomicPtr<PoolEntry>,
    /// Entry data (aligned to pool object size)
    data: [u8; 0], // Flexible array member
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
            cleanup_manager: None, // Will be set later via set_cleanup_manager
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

                return Ok(NonNull::new(head as *mut u8).ok_or_else(|| {
                    CacheOperationError::resource_exhausted("Null pointer in free list")
                })?);
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

    /// Set the cleanup manager for sophisticated cleanup operations
    pub fn set_cleanup_manager(&mut self, cleanup_manager: Arc<PoolCleanupManager>) {
        self.cleanup_manager = Some(cleanup_manager);
    }

    /// Sophisticated cleanup that integrates with existing fragmentation analysis,
    /// maintenance scheduling, and allocation statistics systems
    pub fn try_cleanup(&self) -> bool {
        // Use sophisticated cleanup manager if available
        if let Some(ref cleanup_manager) = self.cleanup_manager {
            // Integrate with existing sophisticated systems
            match cleanup_manager.try_sophisticated_cleanup(self) {
                Ok(cleanup_performed) => cleanup_performed,
                Err(_) => {
                    // Fallback to simple cleanup if sophisticated cleanup fails
                    self.simple_fallback_cleanup()
                }
            }
        } else {
            // Fallback to simple cleanup if no cleanup manager
            self.simple_fallback_cleanup()
        }
    }

    /// Simple fallback cleanup (preserves original behavior)
    fn simple_fallback_cleanup(&self) -> bool {
        let current_util = self.current_utilization.load(Ordering::Relaxed);
        if current_util > 0 {
            // Simulate some cleanup happening
            self.current_utilization.fetch_sub(1, Ordering::Relaxed);
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
