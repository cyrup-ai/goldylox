//! Tier-specific coherence statistics
//!
//! This module provides statistics tracking specific to each cache tier
//! in the coherence protocol for detailed performance analysis.

use std::sync::atomic::{AtomicU64, Ordering};

/// Tier-specific coherence statistics
#[derive(Debug)]
pub struct TierCoherenceStats {
    /// Hot tier statistics
    pub hot_tier: TierSpecificStats,
    /// Warm tier statistics
    pub warm_tier: TierSpecificStats,
    /// Cold tier statistics
    pub cold_tier: TierSpecificStats,
}

/// Statistics specific to a cache tier
#[derive(Debug)]
pub struct TierSpecificStats {
    /// Cache hits in this tier
    pub cache_hits: AtomicU64,
    /// Cache misses in this tier
    pub cache_misses: AtomicU64,
    /// Invalidations sent from this tier
    pub invalidations_sent: AtomicU64,
    /// Invalidations received by this tier
    pub invalidations_received: AtomicU64,
    /// Data transfers initiated by this tier
    pub data_transfers_initiated: AtomicU64,
    /// Data transfers received by this tier
    pub data_transfers_received: AtomicU64,
    /// Write-backs from this tier
    pub writebacks_from_tier: AtomicU64,
    /// Write-backs to this tier
    pub writebacks_to_tier: AtomicU64,
}

impl TierSpecificStats {
    pub fn new() -> Self {
        Self {
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            invalidations_sent: AtomicU64::new(0),
            invalidations_received: AtomicU64::new(0),
            data_transfers_initiated: AtomicU64::new(0),
            data_transfers_received: AtomicU64::new(0),
            writebacks_from_tier: AtomicU64::new(0),
            writebacks_to_tier: AtomicU64::new(0),
        }
    }

    /// Calculate hit rate for this tier
    pub fn hit_rate(&self) -> f64 {
        let hits = self.cache_hits.load(Ordering::Relaxed);
        let misses = self.cache_misses.load(Ordering::Relaxed);
        let total = hits + misses;

        if total == 0 {
            0.0
        } else {
            (hits as f64 / total as f64) * 100.0
        }
    }
}

impl TierCoherenceStats {
    pub fn new() -> Self {
        Self {
            hot_tier: TierSpecificStats::new(),
            warm_tier: TierSpecificStats::new(),
            cold_tier: TierSpecificStats::new(),
        }
    }
}
