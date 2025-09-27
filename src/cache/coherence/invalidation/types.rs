//! Core types for invalidation management
//!
//! This module defines the fundamental data structures used for cache invalidation
//! including requests, priorities, and result types.

// Internal invalidation architecture - components may not be used in minimal API

use std::time::Instant;

use crate::cache::coherence::data_structures::{CacheTier, CoherenceKey, InvalidationReason};
use crate::cache::traits::CacheKey;

/// Invalidation request with retry logic
#[derive(Debug, Clone)]
pub struct InvalidationRequest<K: CacheKey> {
    /// Target coherence key
    pub key: CoherenceKey<K>,
    /// Target tier for invalidation
    pub target_tier: CacheTier,
    /// Invalidation reason
    pub reason: InvalidationReason,
    /// Sequence number for ordering
    pub sequence: u32,
    /// Creation timestamp
    pub created_at: Instant,
    /// Number of retry attempts
    pub retry_count: u32,
    /// Last attempt timestamp
    pub last_attempt: Option<Instant>,
    /// Priority level
    pub priority: InvalidationPriority,
}

/// Priority levels for invalidation requests
/// Internal invalidation API - variants used in priority-based processing
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[allow(dead_code)]
pub enum InvalidationPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// Result of invalidation processing
/// Internal invalidation API - variants used in invalidation result handling
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum InvalidationResult {
    /// Invalidation completed successfully
    Success,
    /// Invalidation failed, should retry
    Retry { delay_ms: u64 },
    /// Invalidation failed permanently
    Failed { reason: String },
    /// Invalidation timed out
    TimedOut,
}
