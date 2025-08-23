//! Eviction candidate data structures
//!
//! This module provides core eviction candidate types with
//! rich metadata for decision making.

use std::marker::PhantomData;
use std::time::Instant;

use crate::cache::traits::types_and_enums::SelectionReason;
use crate::cache::traits::{CacheKey, CacheValue};
use crate::cache::types::timestamp_nanos;

/// Eviction candidate with rich decision metadata
#[derive(Debug, Clone)]
pub struct EvictionCandidate<K: CacheKey, V: CacheValue> {
    /// Key to evict
    key: K,
    /// Eviction score (higher = more likely to evict)
    score: f64,
    /// Reason for selection
    reason: SelectionReason,
    /// Confidence in decision (0.0 to 1.0)
    confidence: f32,
    /// Entry metadata
    metadata: CandidateMetadata,
    /// Type marker
    _phantom: PhantomData<V>,
}

impl<K: CacheKey, V: CacheValue> EvictionCandidate<K, V> {
    /// Create new eviction candidate
    #[inline(always)]
    pub fn new(
        key: K,
        score: f64,
        reason: SelectionReason,
        confidence: f32,
        _metadata: CandidateMetadata,
    ) -> Self {
        Self {
            key,
            score,
            reason,
            confidence,
            metadata: CandidateMetadata::default(),
            _phantom: PhantomData,
        }
    }

    /// Create LRU-based candidate
    #[inline(always)]
    pub fn lru(key: K, last_access_ns: u64, confidence: f32) -> Self {
        let now_ns = timestamp_nanos(Instant::now());
        let age_score = (now_ns.saturating_sub(last_access_ns)) as f64 / 1_000_000_000.0;

        Self::new(
            key,
            age_score,
            SelectionReason::LeastRecentlyUsed,
            confidence,
            CandidateMetadata::default(),
        )
    }

    /// Create LFU-based candidate
    #[inline(always)]
    pub fn lfu(key: K, access_count: u64, confidence: f32) -> Self {
        let frequency_score = 1.0 / (access_count as f64 + 1.0);

        Self::new(
            key,
            frequency_score,
            SelectionReason::LeastFrequentlyUsed,
            confidence,
            CandidateMetadata::default(),
        )
    }

    /// Create size-based candidate
    #[inline(always)]
    pub fn large_size(key: K, size_bytes: u64, confidence: f32) -> Self {
        let size_score = size_bytes as f64 / (1024.0 * 1024.0); // Score in MB

        Self::new(
            key,
            size_score,
            SelectionReason::LargeSize,
            confidence,
            CandidateMetadata::default(),
        )
    }

    /// Create candidate with full metadata
    #[inline(always)]
    pub fn with_metadata(
        key: K,
        score: f64,
        reason: SelectionReason,
        confidence: f32,
        metadata: CandidateMetadata,
    ) -> Self {
        Self {
            key,
            score,
            reason,
            confidence,
            metadata,
            _phantom: PhantomData,
        }
    }

    /// Update candidate metadata
    #[inline(always)]
    pub fn update_metadata(&mut self, metadata: CandidateMetadata) {
        self.metadata = metadata;
    }

    /// Get candidate metadata
    #[inline(always)]
    pub fn metadata(&self) -> &CandidateMetadata {
        &self.metadata
    }

    /// Calculate composite eviction score using multiple factors
    #[inline(always)]
    pub fn composite_score(&self) -> f64 {
        let age_weight = 0.4;
        let frequency_weight = 0.3;
        let size_weight = 0.3;

        let age_score = if self.metadata.age_ns > 0 {
            self.metadata.age_ns as f64 / 1_000_000_000.0 // Age in seconds
        } else {
            0.0
        };

        let frequency_score = 1.0 / (self.metadata.access_count as f64 + 1.0);

        let size_score = self.metadata.size_bytes as f64 / (1024.0 * 1024.0); // Size in MB

        (age_score * age_weight + frequency_score * frequency_weight + size_score * size_weight)
            * self.confidence as f64
    }

    /// Check if candidate should be evicted based on thresholds
    #[inline(always)]
    pub fn should_evict(&self, threshold: f64) -> bool {
        self.composite_score() > threshold && self.confidence > 0.5
    }

    /// Get confidence level
    #[inline(always)]
    pub fn confidence(&self) -> f32 {
        self.confidence
    }
}

impl<K, V> crate::cache::traits::entry_and_stats::EvictionCandidate<K, V> for EvictionCandidate<K, V>
where
    K: CacheKey + crate::cache::traits::core::CacheKey,
    V: CacheValue + crate::cache::traits::core::CacheValue,
{
    #[inline(always)]
    fn key(&self) -> &K {
        &self.key
    }

    #[inline(always)]
    fn eviction_score(&self) -> f64 {
        self.score
    }

    #[inline(always)]
    fn selection_reason(&self) -> SelectionReason {
        self.reason
    }

    #[inline(always)]
    fn confidence(&self) -> f32 {
        self.confidence
    }
}

/// Candidate metadata for eviction decisions
#[derive(Debug, Clone, Default)]
pub struct CandidateMetadata {
    /// Entry size in bytes
    pub size_bytes: u64,
    /// Last access timestamp
    pub last_access_ns: u64,
    /// Access frequency
    pub access_count: u64,
    /// Entry age
    pub age_ns: u64,
}

impl CandidateMetadata {
    /// Create metadata from access statistics
    #[inline(always)]
    pub fn from_access_stats(
        size_bytes: u64,
        last_access_ns: u64,
        access_count: u64,
        age_ns: u64,
    ) -> Self {
        Self {
            size_bytes,
            last_access_ns,
            access_count,
            age_ns,
        }
    }

    /// Calculate memory pressure contribution
    #[inline(always)]
    pub fn memory_pressure(&self) -> f64 {
        self.size_bytes as f64 / (self.access_count as f64 + 1.0)
    }

    /// Calculate temporal relevance score
    #[inline(always)]
    pub fn temporal_relevance(&self) -> f64 {
        let now_ns = timestamp_nanos(Instant::now());
        let recency = now_ns.saturating_sub(self.last_access_ns) as f64;
        1.0 / (recency / 1_000_000_000.0 + 1.0) // Inverse of age in seconds
    }

    /// Calculate access efficiency score
    #[inline(always)]
    pub fn access_efficiency(&self) -> f64 {
        if self.age_ns > 0 {
            self.access_count as f64 / (self.age_ns as f64 / 1_000_000_000.0)
        } else {
            self.access_count as f64
        }
    }
}

// Type aliases for common eviction configurations can be defined here
// Example: pub type MyEvictionCandidate<K, V> = EvictionCandidate<K, V>;
