//! Cache entry and statistics traits for runtime monitoring and access
//!
//! This module defines traits for cache entry access, statistics collection,
//! access pattern tracking, eviction candidate selection, and error handling.

#![allow(dead_code)] // Cache traits - Entry and statistics trait definitions for zero-copy monitoring

// Internal trait definitions - may not be used in minimal API

use std::fmt::Debug;
use std::time::Instant;

use super::core::{CacheKey, CacheValue};
use super::types_and_enums::{
    AccessType, ErrorCategory, RecoveryHint, SelectionReason, TemporalPattern, TierLocation,
};

/// Cache entry trait with zero-copy access and efficient metadata
pub trait CacheEntry<Key, Value> {
    /// Entry key reference type
    type Key;
    /// Entry value reference type
    type Value;

    /// Get key reference with zero-copy access
    fn key(&self) -> &Self::Key;

    /// Get value reference with zero-copy access
    fn value(&self) -> &Self::Value;

    /// Get creation timestamp
    fn created_at(&self) -> Instant;

    /// Entry age since creation (calculated from created_at)
    fn age(&self) -> std::time::Duration {
        self.created_at().elapsed()
    }

    /// Access count for frequency tracking (atomic load)
    fn access_count(&self) -> u64;

    /// Last access timestamp (atomic load)
    fn last_access(&self) -> Instant;

    /// Entry size in bytes (cached value)
    fn size(&self) -> usize;

    /// Entry tier location (no allocation)
    fn tier(&self) -> TierLocation;

    /// Check if entry is hot (frequently accessed)
    fn is_hot(&self) -> bool {
        self.access_count() > 10 && self.age().as_secs() < 60
    }

    /// Check if entry is cold (rarely accessed)
    fn is_cold(&self) -> bool {
        self.access_count() <= 2 || self.age().as_secs() > 3600
    }

    /// Get access frequency per hour
    fn access_frequency_per_hour(&self) -> f64 {
        let age_hours = self.age().as_secs_f64() / 3600.0;
        if age_hours > 0.0 {
            self.access_count() as f64 / age_hours
        } else {
            self.access_count() as f64
        }
    }
}

/// Tier statistics trait for atomic lock-free monitoring
pub trait TierStats: Send + Sync + Debug {
    /// Number of entries (atomic load)
    fn entry_count(&self) -> usize;

    /// Memory usage in bytes (atomic load)
    fn memory_usage(&self) -> usize;

    /// Hit rate (0.0 to 1.0, cached calculation)
    fn hit_rate(&self) -> f64;

    /// Average access time in nanoseconds (exponential moving average)
    fn avg_access_time_ns(&self) -> u64;

    /// Operations per second (sliding window calculation)
    fn ops_per_second(&self) -> f64;

    /// Error rate (0.0 to 1.0, cached calculation)
    fn error_rate(&self) -> f64;

    /// Memory capacity in bytes
    fn memory_capacity(&self) -> usize;

    /// Total cache hits (atomic load)
    fn total_hits(&self) -> u64;

    /// Total cache misses (atomic load)
    fn total_misses(&self) -> u64;

    /// Peak access latency in nanoseconds
    fn peak_access_latency_ns(&self) -> u64;

    /// Memory utilization percentage (0.0 to 1.0)
    fn memory_utilization(&self) -> f64 {
        let usage = self.memory_usage() as f64;
        let capacity = self.memory_capacity() as f64;
        if capacity > 0.0 {
            usage / capacity
        } else {
            0.0
        }
    }

    /// Cache efficiency score (0.0 to 1.0, weighted metric)
    fn efficiency_score(&self) -> f64 {
        let hit_rate_weight = 0.4;
        let latency_weight = 0.3;
        let utilization_weight = 0.3;

        let hit_rate_score = self.hit_rate();

        // Normalize latency (lower is better, cap at 10ms)
        let avg_latency_ms = self.avg_access_time_ns() as f64 / 1_000_000.0;
        let latency_score = (10.0 - avg_latency_ms.min(10.0)) / 10.0;

        // Optimal utilization around 70-80%
        let utilization = self.memory_utilization();
        let utilization_score = if utilization <= 0.8 {
            utilization / 0.8
        } else {
            2.0 - (utilization / 0.8)
        }
        .clamp(0.0, 1.0);

        (hit_rate_score * hit_rate_weight)
            + (latency_score * latency_weight)
            + (utilization_score * utilization_weight)
    }

    /// Throughput score based on operations per second
    #[inline(always)]
    fn throughput_score(&self) -> f64 {
        let ops = self.ops_per_second();
        // Normalize based on expected high performance (10k ops/sec reference)
        (ops / 10000.0).min(1.0)
    }

    /// Overall performance score combining efficiency and throughput
    #[inline(always)]
    fn performance_score(&self) -> f64 {
        let efficiency = self.efficiency_score();
        let throughput = self.throughput_score();
        (efficiency * 0.7) + (throughput * 0.3)
    }

    /// Check if performance is optimal
    #[inline(always)]
    fn is_performing_optimally(&self) -> bool {
        self.performance_score() > 0.8 && self.hit_rate() > 0.7 && self.error_rate() < 0.01
    }

    /// Check if cache needs attention
    #[inline(always)]
    fn needs_attention(&self) -> bool {
        self.performance_score() < 0.5
            || self.hit_rate() < 0.4
            || self.error_rate() > 0.05
            || self.memory_utilization() > 0.95
    }
}

/// Access pattern tracker with advanced analytics
pub trait AccessTracker<K: CacheKey, V: CacheValue>: Send + Sync + Debug {
    /// Record access event
    fn record_access(&mut self, key: &K, timestamp: Instant, access_type: AccessType);

    /// Get access frequency for key
    fn access_frequency(&self, key: &K) -> f64;

    /// Get recency score for key
    fn recency_score(&self, key: &K) -> f64;

    /// Predict future access probability
    fn predict_access(&self, key: &K) -> f64;

    /// Get temporal access pattern
    fn temporal_pattern(&self, key: &K) -> TemporalPattern;
}

/// Eviction candidate with rich decision data
pub trait EvictionCandidate<K: CacheKey, V: CacheValue>: Send + Sync + Debug {
    /// Candidate key
    fn key(&self) -> &K;

    /// Eviction score (higher = more likely to evict)
    fn eviction_score(&self) -> f64;

    /// Reason for selection
    fn selection_reason(&self) -> SelectionReason;

    /// Confidence in decision (0.0 to 1.0)
    fn confidence(&self) -> f32;
}

/// Cache error trait with rich error information
pub trait CacheError: std::error::Error + Send + Sync + Debug {
    /// Error category
    fn category(&self) -> ErrorCategory;

    /// Retry possibility
    fn is_retryable(&self) -> bool;

    /// Recovery suggestions
    fn recovery_hint(&self) -> RecoveryHint;
}
