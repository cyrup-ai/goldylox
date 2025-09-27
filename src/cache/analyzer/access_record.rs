//! Access record for individual cache keys with atomic concurrent access
//!
//! This module provides thread-safe access tracking for cache keys using
//! cache-line aligned atomic operations for optimal performance.

// Internal analyzer architecture - components may not be used in minimal API

use std::sync::atomic::{AtomicU8, AtomicU32, AtomicU64, Ordering};

use crossbeam_utils::CachePadded;

use super::types::AccessPatternType;

/// Access record for individual cache keys with atomic concurrent access
#[derive(Debug)]
#[repr(align(64))] // Cache-line aligned for performance
pub struct AccessRecord {
    /// Total number of accesses
    total_accesses: CachePadded<AtomicU64>,
    /// Timestamp of last access (nanoseconds)
    last_access_ns: CachePadded<AtomicU64>,
    /// Creation timestamp (nanoseconds)
    creation_ns: u64,
    /// Access count buckets for sliding window frequency calculation
    access_buckets: Box<[AtomicU32]>,
    /// Cached pattern type hint (updated periodically)
    pattern_hint: AtomicU8,
    /// Generation counter for ABA prevention
    generation: AtomicU32,
    /// Estimated memory usage
    memory_usage: AtomicU32,
}

impl AccessRecord {
    pub fn new(now_ns: u64, bucket_count: usize) -> Self {
        let mut buckets = Vec::with_capacity(bucket_count);
        for _ in 0..bucket_count {
            buckets.push(AtomicU32::new(0));
        }

        Self {
            total_accesses: CachePadded::new(AtomicU64::new(1)),
            last_access_ns: CachePadded::new(AtomicU64::new(now_ns)),
            creation_ns: now_ns,
            access_buckets: buckets.into_boxed_slice(),
            pattern_hint: AtomicU8::new(AccessPatternType::Random as u8),
            generation: AtomicU32::new(1),
            memory_usage: AtomicU32::new(std::mem::size_of::<Self>() as u32),
        }
    }

    /// Record a new access atomically
    #[inline(always)]
    pub fn record_access(&self, now_ns: u64, bucket_index: usize) {
        self.total_accesses.fetch_add(1, Ordering::Relaxed);
        self.last_access_ns.store(now_ns, Ordering::Relaxed);

        // Update time bucket for frequency tracking
        if bucket_index < self.access_buckets.len() {
            self.access_buckets[bucket_index].fetch_add(1, Ordering::Relaxed);
        }

        // Increment generation for ABA prevention
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    /// Get total access count
    #[inline(always)]
    pub fn total_accesses(&self) -> u64 {
        self.total_accesses.load(Ordering::Relaxed)
    }

    /// Get last access timestamp
    #[inline(always)]
    pub fn last_access_ns(&self) -> u64 {
        self.last_access_ns.load(Ordering::Relaxed)
    }

    /// Get creation timestamp
    #[inline(always)]
    pub fn creation_ns(&self) -> u64 {
        self.creation_ns
    }

    /// Get access buckets for frequency calculation
    #[inline(always)]
    pub fn access_buckets(&self) -> &[AtomicU32] {
        &self.access_buckets
    }

    /// Get memory usage estimate
    #[inline(always)]
    pub fn memory_usage(&self) -> u32 {
        self.memory_usage.load(Ordering::Relaxed)
    }

    /// Get cached pattern hint
    #[inline(always)]
    pub fn pattern_hint(&self) -> AccessPatternType {
        match self.pattern_hint.load(Ordering::Relaxed) {
            0 => AccessPatternType::Sequential,
            1 => AccessPatternType::Temporal,
            2 => AccessPatternType::Spatial,
            _ => AccessPatternType::Random,
        }
    }

    /// Update pattern hint based on access analysis
    pub fn update_pattern_hint(&self, pattern: AccessPatternType) {
        self.pattern_hint.store(pattern as u8, Ordering::Relaxed);
    }
}
