//! Prefetch prediction with pattern recognition and machine learning
//!
//! This module implements intelligent prefetching based on access patterns,
//! spatial/temporal locality analysis, and machine learning predictions.


use crate::cache::config::CacheConfig;
use crate::cache::tier::hot::prefetch::core::PrefetchPredictor as HotTierPrefetchPredictor;
use crate::cache::tier::hot::prefetch::types::PrefetchConfig;
use crate::cache::manager::background::types::BackgroundTask;
use crate::cache::coordinator::background_coordinator::BackgroundCoordinator;
use super::policy_engine::{PrefetchResult, PrefetchStats};
use super::types::{AccessEvent, PrefetchRequest};
use crate::cache::traits::core::CacheKey;
use crate::cache::traits::types_and_enums::CacheOperationError;
use std::sync::atomic::{AtomicU64, Ordering};

/// Prefetch predictor using production ML system
pub type PrefetchPredictor<K> = crate::cache::tier::hot::prefetch::PrefetchPredictor<K>;


impl<K: CacheKey> PrefetchPredictor<K> {
    pub fn new(config: &CacheConfig) -> Result<Self, CacheOperationError> {
        let prefetch_config = crate::cache::tier::hot::prefetch::PrefetchConfig::default();
        Ok(crate::cache::tier::hot::prefetch::PrefetchPredictor::new(prefetch_config))
    }

}

