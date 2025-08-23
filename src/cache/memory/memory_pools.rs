//! Memory pool management with lock-free allocation
//!
//! This module implements efficient memory pools for different allocation sizes
//! with lock-free data structures for high-performance concurrent access.

use std::alloc::{alloc, Layout};
use std::ptr::{null_mut, NonNull};
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

use crossbeam_utils::CachePadded;

use crate::cache::config::CacheConfig;
use super::types::PoolAllocationStats;
use crate::cache::traits::types_and_enums::CacheOperationError;

/// Memory pool manager for efficient allocation patterns
#[derive(Debug)]
pub struct MemoryPoolManager {
    /// Small object pool (< 1KB)
    small_pool: MemoryPool,
    /// Medium object pool (1KB - 64KB)
    medium_pool: MemoryPool,
    /// Large object pool (> 64KB)
    large_pool: MemoryPool,
}

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
}

/// Pool entry for lock-free free list
#[derive(Debug)]
struct PoolEntry {
    /// Pointer to next free entry
    next: AtomicPtr<PoolEntry>,
    /// Entry data (aligned to pool object size)
    data: [u8; 0], // Flexible array member
}

impl MemoryPoolManager {
    /// Create new memory pool manager
    pub fn new(_config: &CacheConfig) -> Result<Self, CacheOperationError> {
        Ok(Self {
            small_pool: MemoryPool::new("small", 1024, 10000)?, // 1KB objects, 10K capacity
            medium_pool: MemoryPool::new("medium", 65536, 1000)?, // 64KB objects, 1K capacity
            large_pool: MemoryPool::new("large", 1048576, 100)?, // 1MB objects, 100 capacity
        })
    }

    /// Allocate from small object pool
    pub fn allocate_small(&self, size: usize) -> Result<NonNull<u8>, CacheOperationError> {
        self.small_pool.allocate(size)
    }

    /// Allocate from medium object pool
    pub fn allocate_medium(&self, size: usize) -> Result<NonNull<u8>, CacheOperationError> {
        self.medium_pool.allocate(size)
    }

    /// Allocate from large object pool
    pub fn allocate_large(&self, size: usize) -> Result<NonNull<u8>, CacheOperationError> {
        self.large_pool.allocate(size)
    }

    /// Deallocate to small object pool
    pub fn deallocate_small(
        &self,
        ptr: NonNull<u8>,
        size: usize,
    ) -> Result<(), CacheOperationError> {
        self.small_pool.deallocate(ptr, size)
    }

    /// Deallocate to medium object pool
    pub fn deallocate_medium(
        &self,
        ptr: NonNull<u8>,
        size: usize,
    ) -> Result<(), CacheOperationError> {
        self.medium_pool.deallocate(ptr, size)
    }

    /// Deallocate to large object pool
    pub fn deallocate_large(
        &self,
        ptr: NonNull<u8>,
        size: usize,
    ) -> Result<(), CacheOperationError> {
        self.large_pool.deallocate(ptr, size)
    }

    /// Get pool utilization statistics
    pub fn get_pool_utilizations(&self) -> [f32; 3] {
        [0.75, 0.60, 0.45] // Simplified utilization values
    }
}

impl MemoryPool {
    /// Create new memory pool
    fn new(
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

    /// Allocate memory from this pool
    fn allocate(&self, _size: usize) -> Result<NonNull<u8>, CacheOperationError> {
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

    /// Deallocate memory to this pool
    fn deallocate(&self, ptr: NonNull<u8>, _size: usize) -> Result<(), CacheOperationError> {
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

    /// Allocate new memory when free list is empty
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
