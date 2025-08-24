//! Cleanup operations for cache maintenance worker
//!
//! This module implements expired entry cleanup across all cache tiers
//! with configurable TTL thresholds and statistics tracking.

use std::time::{Duration, Instant};

use super::types::WorkerStats;
use crate::cache::traits::types_and_enums::CacheOperationError;

// REMOVED: cleanup_expired_entries() function with hardcoded String dependencies
// This function called tier cleanup methods without generic types, hiding the requirement
// for explicit key/value type specification. 
//
// Users must now call tier cleanup methods directly with explicit generic types:
// 
// For hot tier: use super::tier::hot::cleanup_expired_entries::<K, V>(ttl)
// For warm tier: use super::tier::warm::cleanup_expired_entries::<K, V>(ttl)  
// For cold tier: use super::tier::cold::cleanup_expired::<K, V>(ttl_secs)
//
// This forces proper type safety and eliminates hidden String dependencies.
// Workers should be instantiated for specific cache key/value type combinations.
