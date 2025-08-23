//! Tier transition logic for cache maintenance worker
//!
//! This module implements the logic for checking and performing automatic
//! tier promotions and demotions based on access patterns and thresholds.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::types::WorkerStats;
use crate::cache::tier::{cold, hot, warm};

/// Check for entries that need tier transitions with generic types
/// NOTE: This function is now generic and must be called with explicit type parameters
/// Example: check_tier_transitions::<MyKey, MyValue>(&stats)
pub fn check_tier_transitions<K: CacheKey + Clone, V: CacheValue + Clone>(
    stats: &Arc<Mutex<WorkerStats>>,
) {
    let now = Instant::now();

    // Define promotion/demotion thresholds
    const HOT_TO_WARM_IDLE_THRESHOLD: Duration = Duration::from_secs(300); // 5 minutes
    const WARM_TO_COLD_IDLE_THRESHOLD: Duration = Duration::from_secs(1800); // 30 minutes
    const COLD_TO_WARM_ACCESS_THRESHOLD: u32 = 5; // 5 accesses in recent window
    const WARM_TO_HOT_ACCESS_THRESHOLD: u32 = 10; // 10 accesses in recent window

    // Check warm tier utilization for promotion to hot tier
    if let Some(_warm_stats) = warm::get_stats::<K, V>() {
        // Promote frequently accessed warm entries to hot tier
        let frequently_accessed_keys: Vec<K> =
            warm::get_frequently_accessed_keys::<K, V>(WARM_TO_HOT_ACCESS_THRESHOLD as usize);

        let mut promotions = 0;
        for key in frequently_accessed_keys {
            if let Some(value) = warm::warm_get::<K, V>(&key) {
                if warm::remove_entry::<K, V>(&key).is_ok() {
                    if hot::insert_promoted(key, value).is_ok() {
                        promotions += 1;
                    }
                }
            }
        }

        // Update statistics with actual promotion count
        if let Ok(mut worker_stats) = stats.lock() {
            worker_stats.tier_transitions += promotions;
        }
    }

    // Check cold tier for promotion to warm tier
    if let Ok(_cold_stats) = cold::get_stats() {
        let frequently_accessed_keys: Vec<K> =
            cold::get_frequently_accessed_keys(COLD_TO_WARM_ACCESS_THRESHOLD);

        for key in frequently_accessed_keys {
            if let Ok(Some(value)) = cold::cold_get::<K, V>(&key) {
                if cold::remove_entry(&key).is_ok() {
                    if warm::insert_promoted(key, value).is_ok() {
                        if let Ok(mut worker_stats) = stats.lock() {
                            worker_stats.tier_transitions += 1;
                        }
                    }
                }
            }
        }
    }

    // Check for demotions - hot to warm
    let idle_hot_keys: Vec<K> = hot::get_idle_keys(HOT_TO_WARM_IDLE_THRESHOLD);
    for key in idle_hot_keys {
        if let Some(entry) = hot::remove_entry::<K, V>(&key) {
            if warm::insert_demoted(key, entry).is_ok() {
                if let Ok(mut worker_stats) = stats.lock() {
                    worker_stats.tier_transitions += 1;
                }
            }
        }
    }

    // Check for demotions - warm to cold
    let idle_warm_keys: Vec<K> = warm::get_idle_keys::<K, V>(WARM_TO_COLD_IDLE_THRESHOLD);
    for key in idle_warm_keys {
        if let Some(value) = warm::warm_get::<K, V>(&key) {
            if warm::remove_entry::<K, V>(&key).is_ok() {
                if cold::insert_demoted(key, value).is_ok() {
                    if let Ok(mut worker_stats) = stats.lock() {
                        worker_stats.tier_transitions += 1;
                    }
                }
            }
        }
    }

    // Update worker statistics - last tier check timestamp
    if let Ok(mut worker_stats) = stats.lock() {
        worker_stats.last_tier_check = Some(now);
    }
}
