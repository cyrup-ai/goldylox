//! Atomic coordination state for cache operations
//!
//! This module provides reader-writer locks and coordination primitives
//! for thread-safe cache access patterns.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Atomic coordination state for cache operations
#[derive(Debug)]
pub struct CoordinationState {
    /// Operation lock (0=free, 1=locked)
    operation_lock: AtomicU32,
    /// Active readers count
    active_readers: AtomicU32,
    /// Active writers count  
    active_writers: AtomicU32,
    /// Generation counter for ABA prevention
    generation: AtomicU64,
}

impl Default for CoordinationState {
    fn default() -> Self {
        Self::new()
    }
}

impl CoordinationState {
    /// Create new coordination state
    pub fn new() -> Self {
        Self {
            operation_lock: AtomicU32::new(0),
            active_readers: AtomicU32::new(0),
            active_writers: AtomicU32::new(0),
            generation: AtomicU64::new(0),
        }
    }

    /// Acquire read lock (non-blocking)
    pub fn try_read_lock(&self) -> bool {
        // Check if writers are active
        if self.active_writers.load(Ordering::Acquire) > 0 {
            return false;
        }

        // Increment reader count
        self.active_readers.fetch_add(1, Ordering::AcqRel);

        // Double-check no writers started
        if self.active_writers.load(Ordering::Acquire) > 0 {
            self.active_readers.fetch_sub(1, Ordering::AcqRel);
            return false;
        }

        true
    }

    /// Release read lock
    pub fn release_read_lock(&self) {
        self.active_readers.fetch_sub(1, Ordering::AcqRel);
    }

    /// Acquire write lock (non-blocking)
    pub fn try_write_lock(&self) -> bool {
        // Check if any readers or writers are active
        if self.active_readers.load(Ordering::Acquire) > 0
            || self.active_writers.load(Ordering::Acquire) > 0
        {
            return false;
        }

        // Try to acquire write lock
        match self
            .active_writers
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => {
                // Double-check no readers started
                if self.active_readers.load(Ordering::Acquire) > 0 {
                    self.active_writers.store(0, Ordering::Release);
                    return false;
                }
                true
            }
            Err(_) => false,
        }
    }

    /// Release write lock
    pub fn release_write_lock(&self) {
        self.active_writers.store(0, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Get current generation (for ABA prevention)
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Check if any operations are active
    pub fn is_active(&self) -> bool {
        self.active_readers.load(Ordering::Acquire) > 0
            || self.active_writers.load(Ordering::Acquire) > 0
    }
}

/// RAII guard for read lock
pub struct ReadGuard<'a> {
    state: &'a CoordinationState,
}

impl<'a> ReadGuard<'a> {
    /// Create new read guard
    pub fn new(state: &'a CoordinationState) -> Option<Self> {
        if state.try_read_lock() {
            Some(Self { state })
        } else {
            None
        }
    }
}

impl<'a> Drop for ReadGuard<'a> {
    fn drop(&mut self) {
        self.state.release_read_lock();
    }
}

/// RAII guard for write lock
pub struct WriteGuard<'a> {
    state: &'a CoordinationState,
}

impl<'a> WriteGuard<'a> {
    /// Create new write guard
    pub fn new(state: &'a CoordinationState) -> Option<Self> {
        if state.try_write_lock() {
            Some(Self { state })
        } else {
            None
        }
    }
}

impl<'a> Drop for WriteGuard<'a> {
    fn drop(&mut self) {
        self.state.release_write_lock();
    }
}
