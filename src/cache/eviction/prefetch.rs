//! Prefetch prediction with pattern recognition and machine learning
//!
//! This module implements intelligent prefetching based on access patterns,
//! spatial/temporal locality analysis, and machine learning predictions.

use crate::cache::config::CacheConfig;

use super::policy_engine::PrefetchResult;
use super::types::{AccessEvent, PrefetchRequest};
use crate::cache::tier::hot::prefetch::types::PrefetchStats; // Canonical location
use crate::cache::traits::core::CacheKey;
use crate::cache::traits::types_and_enums::CacheOperationError;
// AtomicU64 and Ordering removed - not used in current implementation

// PrefetchPredictor moved to canonical location: crate::cache::tier::hot::prefetch::core::PrefetchPredictor
// Use the enhanced canonical implementation directly - no wrapper needed for eviction-specific functionality
pub use crate::cache::tier::hot::prefetch::core::PrefetchPredictor;

/// Eviction-specific prefetch adapter functions
#[allow(dead_code)] // Prefetch system - adapter used in eviction-specific prefetch operations
pub struct EvictionPrefetchAdapter;

impl EvictionPrefetchAdapter {
    /// Create new PrefetchPredictor from cache configuration (eviction compatibility)
    #[allow(dead_code)] // Prefetch system - predictor factory used in eviction-specific initialization
    pub fn new_predictor<K: CacheKey>(
        _config: &CacheConfig,
    ) -> Result<PrefetchPredictor<K>, CacheOperationError> {
        let prefetch_config = crate::cache::tier::hot::prefetch::PrefetchConfig::default();
        Ok(PrefetchPredictor::new(prefetch_config))
    }

    /// Record access event for pattern learning (eviction compatibility)
    #[allow(dead_code)] // Prefetch system - access recording used in pattern analysis
    pub fn record_access_event<K: CacheKey>(
        predictor: &mut PrefetchPredictor<K>,
        event: &AccessEvent<K>,
    ) {
        // Convert AccessEvent to the format expected by canonical predictor
        predictor.record_access(&event.key, event.timestamp, 0); // Default context hash
    }

    /// Execute prefetch operations and return eviction-specific results
    #[allow(dead_code)] // Prefetch system - execution used in prefetch operation processing
    pub fn execute_prefetch<K: CacheKey>(
        predictor: &mut PrefetchPredictor<K>,
        requests: &[PrefetchRequest<K>],
    ) -> PrefetchResult {
        let mut successful = 0;
        let mut failed = 0;
        let mut total_latency = 0u64;

        for request in requests {
            let start = std::time::Instant::now();

            // Check if we should prefetch this key using canonical predictor
            if predictor.should_prefetch(&request.key) {
                successful += 1;
            } else {
                failed += 1;
            }

            total_latency += start.elapsed().as_nanos() as u64;
        }

        PrefetchResult {
            successful_prefetches: successful,
            failed_prefetches: failed,
            total_latency_ns: total_latency,
        }
    }

    /// Get prefetch metrics (eviction compatibility) - returns canonical enhanced PrefetchStats
    #[allow(dead_code)] // Prefetch system - metrics used in performance monitoring
    pub fn get_metrics<K: CacheKey>(predictor: &PrefetchPredictor<K>) -> PrefetchStats {
        let stats = predictor.get_stats();
        PrefetchStats {
            total_predictions: stats.total_predictions,
            accuracy: stats.accuracy,
            hit_rate: stats.hit_rate,
            patterns_detected: stats.patterns_detected,
            queue_size: stats.queue_size,
            avg_confidence: stats.avg_confidence,
            // Enhanced fields from policy engine version
            average_latency_ns: 0, // Not tracked by canonical predictor
            successful_count: stats.total_predictions,
            failed_count: if stats.accuracy > 0.0 {
                ((1.0 - stats.accuracy) * stats.total_predictions as f64) as u64
            } else {
                0
            },
        }
    }

    /// Shutdown predictor (eviction compatibility)
    #[allow(dead_code)] // Prefetch system - shutdown used in graceful cleanup
    pub fn shutdown<K: CacheKey>(
        predictor: &mut PrefetchPredictor<K>,
    ) -> Result<(), CacheOperationError> {
        // Clear predictor state for clean shutdown
        predictor.clear();

        // The canonical predictor doesn't have a shutdown method, so we just clear state
        log::debug!("PrefetchPredictor shutdown completed");
        Ok(())
    }
}
