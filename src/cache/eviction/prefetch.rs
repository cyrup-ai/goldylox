//! Prefetch prediction with pattern recognition and machine learning
//!
//! This module implements intelligent prefetching based on access patterns,
//! spatial/temporal locality analysis, and machine learning predictions.


use crate::cache::config::CacheConfig;
use crate::cache::tier::hot::prefetch::types::AccessSequence;
use super::policy_engine::{PrefetchResult, PrefetchStats};
use super::types::{AccessEvent, PrefetchRequest};
use crate::cache::traits::core::CacheKey;
use crate::cache::traits::types_and_enums::CacheOperationError;
// AtomicU64 and Ordering removed - not used in current implementation

/// Wrapper around hot tier's PrefetchPredictor with eviction-specific extensions
#[derive(Debug)]
pub struct PrefetchPredictor<K: CacheKey> {
    inner: crate::cache::tier::hot::prefetch::PrefetchPredictor<K>,
    stats: PrefetchStats,
}

impl<K: CacheKey> PrefetchPredictor<K> {
    pub fn new(config: &CacheConfig) -> Result<Self, CacheOperationError> {
        let prefetch_config = crate::cache::tier::hot::prefetch::PrefetchConfig::default();
        Ok(Self {
            inner: crate::cache::tier::hot::prefetch::PrefetchPredictor::new(prefetch_config),
            stats: PrefetchStats::default(),
        })
    }

    /// Record an access event for pattern learning
    pub fn record_access(&mut self, event: &AccessEvent<K>) {
        // Convert AccessEvent to AccessSequence for hot tier predictor
        let sequence = AccessSequence {
            key: event.key.clone(),
            timestamp: event.timestamp,
            context_hash: 0, // Default context
        };
        
        // Hot tier predictor expects timestamp and context_hash separately
        self.inner.record_access(&event.key, event.timestamp, 0);
    }

    /// Get next prefetch requests
    pub fn get_next_prefetches(&mut self, count: usize) -> Vec<crate::cache::tier::hot::prefetch::types::PrefetchRequest<K>> {
        self.inner.get_next_prefetches(count)
    }

    /// Execute prefetch operations
    pub fn execute_prefetch(&mut self, requests: &[PrefetchRequest<K>]) -> PrefetchResult {
        let mut successful = 0;
        let mut failed = 0;
        let mut total_latency = 0u64;

        for request in requests {
            // Record prefetch attempt
            let start = std::time::Instant::now();
            
            // Check if we should prefetch this key
            if self.inner.should_prefetch(&request.key) {
                successful += 1;
                self.stats.successful_count += 1;
            } else {
                failed += 1;
                self.stats.failed_count += 1;
            }
            
            total_latency += start.elapsed().as_nanos() as u64;
        }

        // Update hit rate
        let total = self.stats.successful_count + self.stats.failed_count;
        if total > 0 {
            self.stats.hit_rate = self.stats.successful_count as f64 / total as f64;
            self.stats.average_latency_ns = total_latency / requests.len() as u64;
        }

        PrefetchResult {
            successful_prefetches: successful,
            failed_prefetches: failed,
            total_latency_ns: total_latency,
        }
    }

    /// Get prefetch metrics
    pub fn get_metrics(&self) -> PrefetchStats {
        self.stats.clone()
    }

    /// Shutdown predictor
    pub fn shutdown(&mut self) -> Result<(), CacheOperationError> {
        // Hot tier predictor doesn't need explicit shutdown
        Ok(())
    }
}

